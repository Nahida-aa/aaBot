use aa_core::llm::*;

pub enum UiEvent {
    Token(String),
    ToolCall(ToolCall),
    ToolResult { name: String, content: String, is_error: bool },
    Done { usage: Option<Usage> },
    Error(String),
    Ready { tools: usize, model: String },
}

#[derive(Clone)]
pub enum MsgKind {
    User,
    Assistant,
    ToolResult { content: String, is_error: bool },
    Error(String),
}

pub struct DisplayMessage {
    pub kind: MsgKind,
    pub content: String,
}

pub enum WorkerCommand {
    Submit { messages: Vec<Message> },
}
