use std::sync::Arc;

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

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let kernel = aa_kernel::Kernel::builder()
        .with_tool_pack(std::sync::Arc::new(aa_extension_fs::FsToolPack))
        .build();
    let scope = aa_kernel::ToolPackScope::new(".");
    let registry = Arc::new(RwLock::new(kernel.build_tool_registry(&scope)));

    let state = AppState { registry };

    let app = Router::new()
        .route("/health", get(health))
        .route("/tools", get(list_tools))
        .route("/tools/{name}", post(call_tool))
        .with_state(state);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("Failed to bind");

    println!("aaServer listening on http://0.0.0.0:3000");
    axum::serve(listener, app)
        .await
        .expect("Server failed");
}
