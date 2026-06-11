//! LLM 抽象。
//!
//! 提供多 Provider 调用抽象。内置 Provider 实现可能在 aa-extensions 中，
//! 但此接口定义在 aa-core 中以供平台层依赖。

use serde::{Deserialize, Serialize};

/// LLM 提供者标识。
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ProviderId(pub String);

/// LLM 调用配置。
#[derive(Debug, Clone)]
pub struct ModelConfig {
    pub provider: ProviderId,
    pub model: String,
    pub temperature: Option<f32>,
    pub max_tokens: Option<u32>,
    pub top_p: Option<f32>,
}

/// LLM 调用请求。
#[derive(Debug, Clone)]
pub struct ModelRequest {
    pub messages: Vec<Message>,
    pub tools: Vec<serde_json::Value>,
    pub config: ModelConfig,
}

/// LLM 调用响应。
#[derive(Debug, Clone)]
pub struct ModelResponse {
    pub message: Message,
    pub tool_calls: Vec<ToolCall>,
    pub usage: Option<Usage>,
    pub provider: ProviderId,
}

/// 消息。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: Role,
    pub content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

/// 角色。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    System,
    User,
    Assistant,
    Tool,
}

/// LLM 工具调用。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub call_type: String,
    pub function: ToolCallFunction,
}

/// 工具调用函数。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String,
}

/// Token 用量。
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

/// 流式 LLM 响应。
#[derive(Debug, Clone)]
pub enum StreamEvent {
    Chunk(String),
    ToolCall(ToolCall),
    Done(Usage),
    Error(String),
}

/// LLM 调用结果。
#[derive(Debug)]
pub enum ModelResult {
    Sync(ModelResponse),
    Stream(tokio::sync::mpsc::Receiver<StreamEvent>),
}

/// LLM Provider 接口。
#[async_trait::async_trait]
pub trait ModelProvider: Send + Sync {
    fn id(&self) -> ProviderId;
    async fn chat(&self, request: ModelRequest) -> Result<ModelResponse, ModelError>;
    async fn chat_stream(
        &self,
        request: ModelRequest,
    ) -> Result<tokio::sync::mpsc::Receiver<StreamEvent>, ModelError>;
}

/// LLM 调用错误。
#[derive(Debug, thiserror::Error)]
pub enum ModelError {
    #[error("Provider error: {0}")]
    Provider(String),
    #[error("Rate limited, retry after {0}s")]
    RateLimited(u64),
    #[error("Context length exceeded: requested {requested}, allowed {max}")]
    ContextLengthExceeded { requested: u32, max: u32 },
    #[error("Model not found: {0}")]
    ModelNotFound(String),
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}
