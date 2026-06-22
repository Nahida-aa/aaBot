use std::convert::Infallible;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use aa_core::llm::{
    Message, ModelConfig, ModelProvider, ModelRequest, ProviderId, Role, StreamEvent, ToolCall,
    ToolCallFunction,
};
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, Response, StatusCode},
    response::Json,
    routing::{get, post},
    Router,
};
use futures_util::Stream;
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use utoipa::{OpenApi, ToSchema};

type Registry = Arc<RwLock<aa_kernel::ToolRegistry>>;

#[derive(Clone)]
struct AppState {
    registry: Registry,
}

#[derive(Serialize, ToSchema)]
struct ToolInfo {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Serialize, ToSchema)]
struct HealthResponse {
    status: String,
    tool_count: usize,
}

#[derive(Deserialize, ToSchema)]
struct ToolCallRequest {
    arguments: serde_json::Value,
}

#[derive(Serialize, ToSchema)]
struct ToolResult {
    content: String,
    is_error: bool,
    metadata: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// AG-UI wire format types
// ---------------------------------------------------------------------------

#[derive(Deserialize, Clone, ToSchema)]
struct ChatMsg {
    role: String,
    #[serde(default)]
    content: Option<serde_json::Value>,
    #[serde(default)]
    tool_calls: Option<Vec<ToolCallWire>>,
    #[serde(default)]
    tool_call_id: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    #[allow(dead_code)]
    id: Option<String>,
}

#[derive(Deserialize, Clone, ToSchema)]
struct ToolCallWire {
    id: String,
    #[serde(rename = "type")]
    call_type: String,
    function: ToolCallFuncWire,
}

#[derive(Deserialize, Clone, ToSchema)]
struct ToolCallFuncWire {
    name: String,
    arguments: String,
}

#[derive(Deserialize, Clone, ToSchema)]
struct ToolDef {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Deserialize, ToSchema)]
struct AGUIChatRequest {
    #[serde(default)]
    thread_id: Option<String>,
    #[serde(default)]
    run_id: Option<String>,
    messages: Vec<ChatMsg>,
    tools: Vec<ToolDef>,
}

// ---------------------------------------------------------------------------
// Message Part types (for TUI PromptInfo)
// ---------------------------------------------------------------------------

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct TextPart {
    id: String,
    session_id: String,
    message_id: String,
    r#type: String,
    text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    synthetic: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ignored: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    time: Option<PartTime>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    metadata: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct FilePart {
    id: String,
    session_id: String,
    message_id: String,
    r#type: String,
    mime: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    filename: Option<String>,
    url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source: Option<FilePartSource>,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct FilePartSource {
    text: PartTextRange,
    r#type: String,
    path: String,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct PartTextRange {
    value: String,
    start: usize,
    end: usize,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct AgentPart {
    id: String,
    session_id: String,
    message_id: String,
    r#type: String,
    name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source: Option<AgentPartSource>,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct AgentPartSource {
    value: String,
    start: usize,
    end: usize,
}

#[derive(Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
struct PartTime {
    start: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    end: Option<f64>,
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn extract_text(content: &Option<serde_json::Value>) -> String {
    match content {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Array(arr)) => arr
            .iter()
            .filter_map(|part| part.get("text").and_then(|t| t.as_str()))
            .collect::<Vec<_>>()
            .join(""),
        _ => String::new(),
    }
}

fn chat_msg_to_message(msg: &ChatMsg) -> Message {
    let role = match msg.role.as_str() {
        "assistant" => Role::Assistant,
        "tool" => Role::Tool,
        "system" => Role::System,
        _ => Role::User,
    };

    let tool_calls = msg.tool_calls.as_ref().map(|tcs| {
        tcs.iter()
            .map(|tc| ToolCall {
                id: tc.id.clone(),
                call_type: tc.call_type.clone(),
                function: ToolCallFunction {
                    name: tc.function.name.clone(),
                    arguments: tc.function.arguments.clone(),
                },
            })
            .collect()
    });

    Message {
        role,
        content: extract_text(&msg.content),
        tool_calls,
        tool_call_id: msg.tool_call_id.clone(),
        name: msg.name.clone(),
    }
}

fn create_provider(cfg: &aa_config::ResolvedConfig) -> Box<dyn ModelProvider> {
    match cfg.provider.as_str() {
        "ollama" => {
            Box::new(aa_ollama::OllamaProvider::new(aa_ollama::OllamaConfig {
                base_url: cfg.base_url.clone(),
                default_model: cfg.model.clone(),
            }))
        }
        _ => {
            Box::new(aa_llm::OpenAiCompatibleProvider::new(aa_llm::OpenAiConfig {
                base_url: cfg.base_url.clone(),
                api_key: cfg.api_key.clone(),
                default_model: cfg.model.clone(),
            }))
        }
    }
}

async fn send_json(tx: &tokio::sync::mpsc::Sender<String>, value: serde_json::Value) {
    if let Ok(json) = serde_json::to_string(&value) {
        let _ = tx.send(json).await;
    }
}

// ---------------------------------------------------------------------------
// SSE stream adapter (tokio::mpsc::Receiver → SSE-formatted Stream for Body)
// ---------------------------------------------------------------------------

struct SseRx(tokio::sync::mpsc::Receiver<String>);

impl Stream for SseRx {
    type Item = Result<String, Infallible>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        match self.get_mut().0.poll_recv(cx) {
            Poll::Ready(Some(json)) => Poll::Ready(Some(Ok(format!("data: {json}\n\n")))),
            Poll::Ready(None) => Poll::Ready(None),
            Poll::Pending => Poll::Pending,
        }
    }
}

// ---------------------------------------------------------------------------
// SSE chat endpoint
// ---------------------------------------------------------------------------

async fn chat_sse(
    State(state): State<AppState>,
    Json(req): Json<AGUIChatRequest>,
) -> Response<Body> {
    let (tx, rx) = tokio::sync::mpsc::channel::<String>(64);
    let registry = state.registry.clone();
    let thread_id = req
        .thread_id
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let run_id = req
        .run_id
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let history = req.messages;
    let tool_defs: Vec<serde_json::Value> = req
        .tools
        .iter()
        .map(|t| {
            serde_json::json!({
                "name": t.name,
                "description": t.description,
                "parameters": t.parameters,
            })
        })
        .collect();

    let config = aa_config::Config::load();
    let resolved = config.resolve(None, None, None);

    tokio::spawn(async move {
        // --- API key check ---
        if resolved.provider != "ollama" && resolved.api_key.is_empty() {
            send_json(
                &tx,
                serde_json::json!({
                    "type": "RUN_ERROR",
                    "threadId": thread_id,
                    "runId": run_id,
                    "message": "AA_LLM_API_KEY not set",
                }),
            )
            .await;
            return;
        }

        let provider = create_provider(&resolved);

        // --- RUN_STARTED ---
        send_json(
            &tx,
            serde_json::json!({
                "type": "RUN_STARTED",
                "threadId": thread_id,
                "runId": run_id,
            }),
        )
        .await;

        let mut history = history;
        let mut iteration = 0u32;
        const MAX_ITERATIONS: u32 = 10;

        loop {
            if iteration >= MAX_ITERATIONS {
                send_json(
                    &tx,
                    serde_json::json!({
                        "type": "RUN_FINISHED",
                        "threadId": thread_id,
                        "runId": run_id,
                        "finishReason": "stop",
                    }),
                )
                .await;
                break;
            }
            iteration += 1;

            let msg_id = format!("msg_{iteration}");

            // --- Build LLM request ---
            let llm_messages: Vec<Message> = history.iter().map(chat_msg_to_message).collect();

            let request = ModelRequest {
                messages: llm_messages,
                tools: tool_defs.clone(),
                config: ModelConfig {
                    provider: ProviderId("server".into()),
                    model: resolved.model.clone(),
                    temperature: None,
                    max_tokens: Some(4096),
                    top_p: None,
                },
            };

            // --- TEXT_MESSAGE_START ---
            send_json(
                &tx,
                serde_json::json!({
                    "type": "TEXT_MESSAGE_START",
                    "messageId": msg_id,
                }),
            )
            .await;

            // --- Stream LLM ---
            let mut text_content = String::new();
            let mut tool_calls: Vec<ToolCall> = Vec::new();

            match provider.chat_stream(request).await {
                Ok(mut stream) => {
                    let mut done = false;
                    while let Some(event) = stream.recv().await {
                        match event {
                            StreamEvent::Chunk(text) => {
                                text_content += &text;
                                send_json(
                                    &tx,
                                    serde_json::json!({
                                        "type": "TEXT_MESSAGE_CONTENT",
                                        "delta": text,
                                        "messageId": msg_id,
                                    }),
                                )
                                .await;
                            }
                            StreamEvent::ToolCall(tc) => {
                                tool_calls.push(tc);
                            }
                            StreamEvent::Done(_usage) => {
                                done = true;
                                break;
                            }
                            StreamEvent::Error(err) => {
                                send_json(
                                    &tx,
                                    serde_json::json!({
                                        "type": "RUN_ERROR",
                                        "threadId": thread_id,
                                        "runId": run_id,
                                        "message": err,
                                    }),
                                )
                                .await;
                                return;
                            }
                        }
                    }
                    if !done {
                        send_json(
                            &tx,
                            serde_json::json!({
                                "type": "RUN_ERROR",
                                "threadId": thread_id,
                                "runId": run_id,
                                "message": "LLM stream ended unexpectedly",
                            }),
                        )
                        .await;
                        return;
                    }
                }
                Err(e) => {
                    send_json(
                        &tx,
                        serde_json::json!({
                            "type": "RUN_ERROR",
                            "threadId": thread_id,
                            "runId": run_id,
                            "message": e.to_string(),
                        }),
                    )
                    .await;
                    return;
                }
            }

            // --- Add assistant message to history ---
            let assistant_tool_calls: Option<Vec<ToolCallWire>> =
                if tool_calls.is_empty() {
                    None
                } else {
                    Some(
                        tool_calls
                            .iter()
                            .map(|tc| ToolCallWire {
                                id: tc.id.clone(),
                                call_type: tc.call_type.clone(),
                                function: ToolCallFuncWire {
                                    name: tc.function.name.clone(),
                                    arguments: tc.function.arguments.clone(),
                                },
                            })
                            .collect(),
                    )
                };

            history.push(ChatMsg {
                role: "assistant".into(),
                content: Some(serde_json::Value::String(text_content)),
                tool_calls: assistant_tool_calls,
                tool_call_id: None,
                name: None,
                id: None,
            });

            // --- No tool calls: finish ---
            if tool_calls.is_empty() {
                send_json(
                    &tx,
                    serde_json::json!({
                        "type": "TEXT_MESSAGE_END",
                        "messageId": msg_id,
                    }),
                )
                .await;

                send_json(
                    &tx,
                    serde_json::json!({
                        "type": "RUN_FINISHED",
                        "threadId": thread_id,
                        "runId": run_id,
                        "finishReason": "stop",
                    }),
                )
                .await;
                break;
            }

            // --- Execute tool calls ---
            for tc in &tool_calls {
                send_json(
                    &tx,
                    serde_json::json!({
                        "type": "TOOL_CALL_START",
                        "toolCallId": tc.id,
                        "toolCallName": tc.function.name,
                    }),
                )
                .await;

                send_json(
                    &tx,
                    serde_json::json!({
                        "type": "TOOL_CALL_ARGS",
                        "toolCallId": tc.id,
                        "delta": tc.function.arguments,
                    }),
                )
                .await;

                let parsed_args: serde_json::Value =
                    serde_json::from_str(&tc.function.arguments)
                        .unwrap_or(serde_json::Value::Null);

                let ctx = aa_kernel::tool_provider::ToolExecutionContext {
                    session_id: "http".into(),
                    working_dir: ".".into(),
                };

                let tool_result = {
                    let registry = registry.read().await;
                    registry.find(&tc.function.name)
                };

                let result_content = match tool_result {
                    Some(tool) => match tool.execute(parsed_args.clone(), &ctx).await {
                        Ok(res) => res.content,
                        Err(e) => format!("Error: {e}"),
                    },
                    None => format!("Error: tool '{}' not found", tc.function.name),
                };

                send_json(
                    &tx,
                    serde_json::json!({
                        "type": "TOOL_CALL_END",
                        "toolCallId": tc.id,
                        "input": parsed_args,
                        "result": result_content,
                    }),
                )
                .await;

                history.push(ChatMsg {
                    role: "tool".into(),
                    content: Some(serde_json::Value::String(result_content)),
                    tool_calls: None,
                    tool_call_id: Some(tc.id.clone()),
                    name: Some(tc.function.name.clone()),
                    id: None,
                });
            }

            // --- TEXT_MESSAGE_END ---
            send_json(
                &tx,
                serde_json::json!({
                    "type": "TEXT_MESSAGE_END",
                    "messageId": msg_id,
                }),
            )
            .await;
        }
    });

    Response::builder()
        .header(header::CONTENT_TYPE, "text/event-stream")
        .header(header::CACHE_CONTROL, "no-cache")
        .body(Body::from_stream(SseRx(rx)))
        .unwrap()
}

// ---------------------------------------------------------------------------
// Legacy endpoints (unchanged)
// ---------------------------------------------------------------------------

#[utoipa::path(
    get,
    path = "/health",
    responses(
        (status = 200, description = "Server health status", body = HealthResponse)
    ),
    tag = "aaBot"
)]
async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    let count = state.registry.read().await.len();
    Json(HealthResponse {
        status: "ok".to_string(),
        tool_count: count,
    })
}

#[utoipa::path(
    get,
    path = "/tools",
    responses(
        (status = 200, description = "List all available tools", body = Vec<ToolInfo>)
    ),
    tag = "aaBot"
)]
async fn list_tools(State(state): State<AppState>) -> Json<Vec<ToolInfo>> {
    let registry = state.registry.read().await;
    let tools = registry
        .all_definitions()
        .into_iter()
        .map(|d| ToolInfo {
            name: d.name,
            description: d.description,
            parameters: d.parameters,
        })
        .collect();
    Json(tools)
}

#[utoipa::path(
    post,
    path = "/tools/{name}",
    params(
        ("name" = String, Path, description = "Tool name")
    ),
    request_body = ToolCallRequest,
    responses(
        (status = 200, description = "Tool execution result", body = ToolResult),
        (status = 404, description = "Tool not found", body = String),
        (status = 500, description = "Tool execution failed", body = String),
    ),
    tag = "aaBot"
)]
async fn call_tool(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(req): Json<ToolCallRequest>,
) -> Result<Json<ToolResult>, (StatusCode, String)> {
    let ctx = aa_kernel::tool_provider::ToolExecutionContext {
        session_id: "http".into(),
        working_dir: ".".into(),
    };

    let tool = {
        let registry = state.registry.read().await;
        registry.find(&name).map(|t| t)
    }
    .ok_or_else(|| (StatusCode::NOT_FOUND, format!("Tool '{name}' not found")))?;

    let result = tool.execute(req.arguments, &ctx).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Tool execution failed: {e}"),
        )
    })?;

    Ok(Json(ToolResult {
        content: result.content,
        is_error: result.is_error,
        metadata: result.metadata,
    }))
}

// ---------------------------------------------------------------------------
// OpenAPI
// ---------------------------------------------------------------------------

#[derive(OpenApi)]
#[openapi(
    paths(health, list_tools, call_tool),
    components(schemas(
        HealthResponse,
        ToolInfo,
        ToolCallRequest,
        ToolResult,
        AGUIChatRequest,
        ChatMsg,
        ToolDef,
        ToolCallWire,
        ToolCallFuncWire,
        TextPart,
        FilePart,
        FilePartSource,
        PartTextRange,
        AgentPart,
        AgentPartSource,
        PartTime
    )),
    tags(
        (name = "aaBot", description = "aaBot API")
    )
)]
struct ApiDoc;

async fn openapi_json() -> Json<serde_json::Value> {
    Json(serde_json::to_value(ApiDoc::openapi()).unwrap())
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let kernel = aa_kernel::Kernel::builder()
        .with_tool_provider(std::sync::Arc::new(aa_function_tools::FsToolProvider))
        .build();
    let scope = aa_kernel::ToolProviderScope::new(".");
    let registry = Arc::new(RwLock::new(kernel.build_tool_registry(&scope)));

    let state = AppState { registry };

    let app = Router::new()
        .route("/health", get(health))
        .route("/tools", get(list_tools))
        .route("/tools/{name}", post(call_tool))
        .route("/chat", post(chat_sse))
        .route("/openapi.json", get(openapi_json))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("Failed to bind");

    println!("aaServer listening on http://0.0.0.0:3000");
    axum::serve(listener, app)
        .await
        .expect("Server failed");
}
