#[derive(Debug, Clone)]
pub struct OllamaConfig {
    pub base_url: String,
    pub default_model: String,
}

impl Default for OllamaConfig {
    fn default() -> Self {
        Self {
            base_url: "http://localhost:11434".into(),
            default_model: "llama3.2".into(),
        }
    }
}

#[derive(serde::Serialize)]
pub(crate) struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tools: Option<Vec<Tool>>,
    pub stream: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<Options>,
}

#[derive(serde::Serialize)]
pub(crate) struct Options {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub num_predict: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
}

#[derive(serde::Serialize)]
pub(crate) struct Message {
    pub role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub images: Option<Vec<String>>,
}

#[derive(serde::Serialize)]
pub(crate) struct Tool {
    #[serde(rename = "type")]
    pub tool_type: String,
    pub function: ToolFunction,
}

#[derive(serde::Serialize)]
pub(crate) struct ToolFunction {
    pub name: String,
    pub description: String,
    pub parameters: serde_json::Value,
}

#[derive(serde::Serialize)]
pub(crate) struct ToolCall {
    pub function: ToolCallFunction,
}

#[derive(serde::Serialize)]
pub(crate) struct ToolCallFunction {
    pub name: String,
    pub arguments: serde_json::Value,
}

// Response types

#[derive(serde::Deserialize)]
#[allow(dead_code)]
pub(crate) struct ChatResponse {
    pub model: String,
    pub message: ResponseMessage,
    pub done: bool,
    #[serde(default)]
    pub done_reason: Option<String>,
    #[serde(default)]
    pub total_duration: Option<u64>,
    #[serde(default)]
    pub prompt_eval_count: Option<u32>,
    #[serde(default)]
    pub eval_count: Option<u32>,
}

#[derive(serde::Deserialize)]
pub(crate) struct ResponseMessage {
    pub role: String,
    #[serde(default)]
    pub content: String,
    #[serde(default)]
    pub tool_calls: Vec<ResponseToolCall>,
}

#[derive(serde::Deserialize)]
pub(crate) struct ResponseToolCall {
    pub function: ResponseToolCallFunction,
}

#[derive(serde::Deserialize)]
pub(crate) struct ResponseToolCallFunction {
    pub name: String,
    pub arguments: serde_json::Value,
}
