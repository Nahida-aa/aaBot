use std::sync::Arc;

use aa_core::llm::{ModelConfig, ModelProvider, ModelRequest, ProviderId, Role, Message};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::Json,
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

type Registry = Arc<RwLock<aa_kernel::ToolRegistry>>;

#[derive(Clone)]
struct AppState {
    registry: Registry,
}

#[derive(Serialize)]
struct ToolInfo {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    tool_count: usize,
}

#[derive(Deserialize)]
struct ToolCallRequest {
    arguments: serde_json::Value,
}

#[derive(Deserialize)]
struct ChatRequest {
    messages: Vec<ChatMsg>,
    tools: Vec<ToolDef>,
}

#[derive(Deserialize, Serialize, Clone)]
struct ChatMsg {
    role: String,
    #[serde(default)]
    content: String,
}

#[derive(Deserialize, Clone)]
struct ToolDef {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Serialize)]
struct ChatResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<serde_json::Value>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    let count = state.registry.read().await.len();
    Json(HealthResponse {
        status: "ok",
        tool_count: count,
    })
}

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

async fn call_tool(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(req): Json<ToolCallRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let ctx = aa_kernel::tool_pack::ToolExecutionContext {
        session_id: "http".into(),
        working_dir: ".".into(),
    };

    let tool = {
        let registry = state.registry.read().await;
        registry.find(&name).map(|t| t)
    }
    .ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("Tool '{name}' not found"),
        )
    })?;

    let result = tool.execute(req.arguments, &ctx).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("Tool execution failed: {e}"),
        )
    })?;

    Ok(Json(serde_json::json!({
        "content": result.content,
        "is_error": result.is_error,
        "metadata": result.metadata,
    })))
}

async fn chat(
    State(state): State<AppState>,
    Json(req): Json<ChatRequest>,
) -> Json<ChatResponse> {
    let api_key = std::env::var("AA_LLM_API_KEY").unwrap_or_default();
    if api_key.is_empty() {
        return Json(ChatResponse {
            content: None,
            tool_calls: None,
            error: Some("AA_LLM_API_KEY not set".into()),
        });
    }

    let provider = aa_llm::OpenAiCompatibleProvider::new(aa_llm::OpenAiConfig {
        base_url: std::env::var("AA_LLM_BASE_URL")
            .unwrap_or_else(|_| "https://api.openai.com/v1".into()),
        api_key,
        default_model: std::env::var("AA_LLM_MODEL")
            .unwrap_or_else(|_| "gpt-4o-mini".into()),
    });

    let tool_defs: Vec<serde_json::Value> = req.tools.iter().map(|t| {
        serde_json::json!({
            "name": t.name,
            "description": t.description,
            "parameters": t.parameters,
        })
    }).collect();

    let mut history: Vec<ChatMsg> = req.messages.clone();

    for _ in 0..10 {
        let messages: Vec<Message> = history.iter().map(|m| {
            let role = match m.role.as_str() {
                "assistant" => Role::Assistant,
                "tool" => Role::Tool,
                "system" => Role::System,
                _ => Role::User,
            };
            Message {
                role,
                content: m.content.clone(),
                tool_calls: None,
                tool_call_id: None,
                name: None,
            }
        }).collect();

        let request = ModelRequest {
            messages,
            tools: tool_defs.clone(),
            config: ModelConfig {
                provider: ProviderId("openai".into()),
                model: std::env::var("AA_LLM_MODEL")
                    .unwrap_or_else(|_| "gpt-4o-mini".into()),
                temperature: None,
                max_tokens: Some(4096),
                top_p: None,
            },
        };

        match provider.chat(request).await {
            Ok(response) => {
                if response.tool_calls.is_empty() {
                    return Json(ChatResponse {
                        content: Some(response.message.content),
                        tool_calls: None,
                        error: None,
                    });
                }

                let tcs: Vec<serde_json::Value> = response.tool_calls.iter().map(|tc| {
                    serde_json::json!({
                        "id": tc.id,
                        "type": "function",
                        "function": {
                            "name": tc.function.name,
                            "arguments": tc.function.arguments,
                        }
                    })
                }).collect();

                history.push(ChatMsg {
                    role: "assistant".into(),
                    content: String::new(),
                });

                for tc in &response.tool_calls {
                    let args: serde_json::Value =
                        serde_json::from_str(&tc.function.arguments)
                            .unwrap_or(serde_json::Value::Null);

                    let ctx = aa_kernel::tool_pack::ToolExecutionContext {
                        session_id: "http".into(),
                        working_dir: ".".into(),
                    };

                    let result = {
                        let registry = state.registry.read().await;
                        registry.find(&tc.function.name)
                    };

                    match result {
                        Some(tool) => {
                            match tool.execute(args, &ctx).await {
                                Ok(res) => {
                                    history.push(ChatMsg {
                                        role: "tool".into(),
                                        content: res.content,
                                    });
                                }
                                Err(e) => {
                                    history.push(ChatMsg {
                                        role: "tool".into(),
                                        content: format!("Error: {e}"),
                                    });
                                }
                            }
                        }
                        None => {
                            history.push(ChatMsg {
                                role: "tool".into(),
                                content: format!("Error: tool '{}' not found", tc.function.name),
                            });
                        }
                    }
                }

                // Return tool calls to frontend for display, then continue loop
                if tcs.len() == 1 {
                    // Single step: continue automatically
                    continue;
                }
                // Multiple tool calls: return to frontend for display
                return Json(ChatResponse {
                    content: None,
                    tool_calls: Some(tcs),
                    error: None,
                });
            }
            Err(e) => {
                return Json(ChatResponse {
                    content: None,
                    tool_calls: None,
                    error: Some(e.to_string()),
                });
            }
        }
    }

    Json(ChatResponse {
        content: Some("Max iterations reached".into()),
        tool_calls: None,
        error: None,
    })
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let kernel = aa_kernel::Kernel::builder()
        .with_tool_pack(std::sync::Arc::new(aa_function_tools::FsToolPack))
        .build();
    let scope = aa_kernel::ToolPackScope::new(".");
    let registry = Arc::new(RwLock::new(kernel.build_tool_registry(&scope)));

    let state = AppState { registry };

    let app = Router::new()
        .route("/health", get(health))
        .route("/tools", get(list_tools))
        .route("/tools/{name}", post(call_tool))
        .route("/chat", post(chat))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("Failed to bind");

    println!("aaServer listening on http://0.0.0.0:3000");
    axum::serve(listener, app)
        .await
        .expect("Server failed");
}
