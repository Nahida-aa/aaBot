use std::convert::Infallible;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use aa_core::llm::{
    Message, ModelProvider, Role, ToolCall,
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
    resolved: aa_config::ResolvedConfig,
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
    provider: String,
    model: String,
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
#[allow(dead_code)]
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
    #[serde(default)]
    #[allow(dead_code)]
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

async fn send_json(tx: &tokio::sync::mpsc::Sender<String>, value: serde_json::Value) {
    if let Ok(json) = serde_json::to_string(&value) {
        let _ = tx.send(json).await;
    }
}

// ---------------------------------------------------------------------------
// SSE stream adapter
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

    let resolved = state.resolved.clone();

    tokio::spawn(async move {
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

        let provider: std::sync::Arc<dyn ModelProvider> = match resolved.provider.as_str() {
            "ollama" => {
                std::sync::Arc::new(aa_ollama::OllamaProvider::new(aa_ollama::OllamaConfig {
                    base_url: resolved.base_url.clone(),
                    default_model: resolved.model.clone(),
                }))
            }
            _ => {
                std::sync::Arc::new(aa_llm::OpenAiCompatibleProvider::new(aa_llm::OpenAiConfig {
                    base_url: resolved.base_url.clone(),
                    api_key: resolved.api_key.clone(),
                    default_model: resolved.model.clone(),
                }))
            }
        };

        let mut messages: Vec<Message> = req.messages.iter().map(chat_msg_to_message).collect();

        // ── Load previous session messages ────────────────
        // If thread_id looks like a UUID and has a session file, load it
        if uuid::Uuid::parse_str(&thread_id).is_ok() {
            if let Ok(previous) = aa_session::storage::load(&thread_id) {
                // Keep request messages (user's latest input) + prepend history
                let user_msg = messages.clone();
                messages = previous;
                messages.extend(user_msg);
            }
        }

        let tools = registry.read().await.all_tools();

        let (session_tx, mut session_rx) = tokio::sync::mpsc::channel::<aa_session::SessionEvent>(64);

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

        // --- Spawn session turn ---
        let turn_input = aa_session::TurnInput {
            messages,
            provider,
            tools,
            model: resolved.model.clone(),
            working_dir: ".".into(),
            session_id: thread_id.clone(),
        };

        let turn_handle = tokio::spawn(aa_session::run_turn(turn_input, session_tx));

        // --- Convert SessionEvent → AG-UI SSE ---
        let mut in_message = false;
        let mut msg_counter = 0u32;
        let mut pending_tool_calls: Vec<ToolCall> = Vec::new();

        while let Some(event) = session_rx.recv().await {
            match event {
                aa_session::SessionEvent::Token(text) => {
                    if !in_message {
                        msg_counter += 1;
                        send_json(
                            &tx,
                            serde_json::json!({
                                "type": "TEXT_MESSAGE_START",
                                "messageId": format!("msg_{msg_counter}"),
                            }),
                        )
                        .await;
                        in_message = true;
                    }
                    send_json(
                        &tx,
                        serde_json::json!({
                            "type": "TEXT_MESSAGE_CONTENT",
                            "delta": text,
                            "messageId": format!("msg_{msg_counter}"),
                        }),
                    )
                    .await;
                }
                aa_session::SessionEvent::ToolCall(tc) => {
                    // End current text message before tool calls
                    if in_message {
                        send_json(
                            &tx,
                            serde_json::json!({
                                "type": "TEXT_MESSAGE_END",
                                "messageId": format!("msg_{msg_counter}"),
                            }),
                        )
                        .await;
                        in_message = false;
                    }

                    // Save raw ToolCall for matching with ToolResult
                    pending_tool_calls.push(tc.clone());

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
                }
                aa_session::SessionEvent::ToolResult { content, .. } => {
                    // Match with pending tool call (FIFO as session executes sequentially)
                    if let Some(tc) = pending_tool_calls.first().cloned() {
                        pending_tool_calls.remove(0);

                        let parsed_args: serde_json::Value =
                            serde_json::from_str(&tc.function.arguments)
                                .unwrap_or(serde_json::Value::Null);

                        send_json(
                            &tx,
                            serde_json::json!({
                                "type": "TOOL_CALL_END",
                                "toolCallId": tc.id,
                                "input": parsed_args,
                                "result": content,
                            }),
                        )
                        .await;
                    }
                }
                aa_session::SessionEvent::Done { .. } => {
                    if in_message {
                        send_json(
                            &tx,
                            serde_json::json!({
                                "type": "TEXT_MESSAGE_END",
                                "messageId": format!("msg_{msg_counter}"),
                            }),
                        )
                        .await;
                    }

                    // ── Persist session ──────────────────────────
                    if let Ok(result) = turn_handle.await {
                        let model = &result.model;
                        let _ = aa_session::storage::save(
                            &thread_id,
                            &result.messages,
                            model,
                            &result.provider.id().0,
                        );
                    }

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
                aa_session::SessionEvent::Error(msg) => {
                    if in_message {
                        send_json(
                            &tx,
                            serde_json::json!({
                                "type": "TEXT_MESSAGE_END",
                                "messageId": format!("msg_{msg_counter}"),
                            }),
                        )
                        .await;
                    }
                    send_json(
                        &tx,
                        serde_json::json!({
                            "type": "RUN_ERROR",
                            "threadId": thread_id,
                            "runId": run_id,
                            "message": msg,
                        }),
                    )
                    .await;
                    break;
                }
            }
        }
    });

    Response::builder()
        .header(header::CONTENT_TYPE, "text/event-stream")
        .header(header::CACHE_CONTROL, "no-cache")
        .body(Body::from_stream(SseRx(rx)))
        .unwrap()
}

// ---------------------------------------------------------------------------
// Session endpoints
// ---------------------------------------------------------------------------

#[derive(Serialize, ToSchema)]
struct SessionSummary {
    session_id: String,
    model: String,
    provider: String,
    message_count: usize,
    updated_at: String,
    created_at: String,
}

#[derive(Serialize)]
struct SessionDetail {
    session_id: String,
    model: String,
    provider: String,
    messages: Vec<aa_core::llm::Message>,
    created_at: String,
    updated_at: String,
}

#[utoipa::path(
    get,
    path = "/sessions",
    responses(
        (status = 200, description = "List saved sessions", body = Vec<SessionSummary>)
    ),
    tag = "aaBot"
)]
/// List saved sessions (most recent first).
async fn list_sessions() -> Json<Vec<SessionSummary>> {
    let sessions = aa_session::storage::list().unwrap_or_default();
    Json(
        sessions
            .into_iter()
            .map(|s| SessionSummary {
                session_id: s.session_id,
                model: s.model,
                provider: s.provider,
                message_count: s.messages.len(),
                updated_at: s.updated_at,
                created_at: s.created_at,
            })
            .collect(),
    )
}

/// Get full session detail including messages.
async fn get_session(Path(id): Path<String>) -> Result<Json<SessionDetail>, (StatusCode, String)> {
    let file = aa_session::storage::load_file(&id)
        .map_err(|e| (StatusCode::NOT_FOUND, format!("Session '{id}' not found: {e}")))?;
    Ok(Json(SessionDetail {
        session_id: file.session_id,
        model: file.model,
        provider: file.provider,
        messages: file.messages,
        created_at: file.created_at,
        updated_at: file.updated_at,
    }))
}

// ---------------------------------------------------------------------------
// Legacy endpoints
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
        provider: state.resolved.provider.clone(),
        model: state.resolved.model.clone(),
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
    paths(health, list_tools, call_tool, list_sessions),
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
        PartTime,
        SessionSummary
    )),
    tags(
        (name = "aaBot", description = "aaBot API")
    )
)]
pub(crate) struct ApiDoc;

async fn openapi_json() -> Json<serde_json::Value> {
    Json(serde_json::to_value(ApiDoc::openapi()).unwrap())
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

fn build_app(registry: Registry, resolved: aa_config::ResolvedConfig) -> Router {
    let state = AppState { registry, resolved };

    Router::new()
        .route("/health", get(health))
        .route("/tools", get(list_tools))
        .route("/tools/{name}", post(call_tool))
        .route("/chat", post(chat_sse))
        .route("/sessions", get(list_sessions))
        .route("/sessions/{id}", get(get_session))
        .route("/openapi.json", get(openapi_json))
        .with_state(state)
}

/// Build the kernel with built-in tool providers and optional MCP extensions.
fn build_kernel(config: &aa_config::Config) -> aa_kernel::Kernel {
    let mut builder = aa_kernel::Kernel::builder()
        .with_tool_provider(std::sync::Arc::new(aa_function_tools::FsToolProvider));

    if let Some(mcp_json) = config.mcp_servers_json() {
        builder = builder.with_tool_provider(
            std::sync::Arc::new(aa_extension_mcp::McpToolProvider::from_json(mcp_json)),
        );
    }

    builder.build()
}

/// Start the aa server on the given port.
///
/// Optional config overrides (provider, model, base_url) take highest priority.
pub async fn serve(
    port: u16,
    cli_provider: Option<&str>,
    cli_model: Option<&str>,
    cli_base_url: Option<&str>,
) -> anyhow::Result<()> {
    let config = aa_config::Config::load();
    let kernel = build_kernel(&config);
    let scope = aa_kernel::ToolProviderScope::new(".");
    let registry = Arc::new(RwLock::new(kernel.build_tool_registry(&scope)));

    let resolved = config.resolve(cli_provider, cli_model, cli_base_url);

    let app = build_app(registry, resolved);

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{port}"))
        .await
        .expect("Failed to bind");

    tracing::info!("aaServer listening on http://0.0.0.0:{port}");
    axum::serve(listener, app).await?;
    Ok(())
}
