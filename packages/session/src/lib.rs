use std::sync::Arc;

use aa_core::llm::*;
use aa_kernel::tool_provider::{Tool, ToolExecutionContext};

// ── Events ──────────────────────────────────────────────────

/// 对话轮次中发送给 UI 的实时事件。
#[derive(Debug, Clone)]
pub enum SessionEvent {
    Token(String),
    ToolCall(ToolCall),
    ToolResult { name: String, content: String, is_error: bool },
    Done { usage: Option<Usage> },
    Error(String),
}

// ── Turn ─────────────────────────────────────────────────────

/// 一次对话轮次的输入。
pub struct TurnInput {
    pub messages: Vec<Message>,
    pub provider: Arc<dyn ModelProvider>,
    pub tools: Vec<Arc<dyn Tool>>,
    pub model: String,
    pub working_dir: String,
    pub session_id: String,
}

/// 运行一次完整的对话轮次（包含工具调用循环）。
///
/// 消费 `input`，通过 `tx` 发送实时事件，返回更新后的消息列表。
pub async fn run_turn(
    input: TurnInput,
    tx: std::sync::mpsc::Sender<SessionEvent>,
) -> TurnInput {
    let TurnInput { mut messages, provider, tools, model, working_dir, session_id } = input;

    let tool_defs: Vec<serde_json::Value> = tools
        .iter()
        .map(|t| serde_json::to_value(t.definition()).unwrap())
        .collect();

    loop {
        let request = ModelRequest {
            messages: messages.clone(),
            tools: tool_defs.clone(),
            config: ModelConfig {
                provider: provider.id(),
                model: model.clone(),
                temperature: None,
                max_tokens: Some(4096),
                top_p: None,
            },
        };

        let mut stream_rx = match provider.chat_stream(request).await {
            Ok(rx) => rx,
            Err(e) => {
                let _ = tx.send(SessionEvent::Error(format!("{e}")));
                return TurnInput { messages, provider, tools, model, working_dir, session_id };
            }
        };

        let mut assistant_text = String::new();
        let mut tool_calls: Vec<ToolCall> = Vec::new();

        while let Some(event) = stream_rx.recv().await {
            match event {
                StreamEvent::Chunk(text) => {
                    assistant_text.push_str(&text);
                    let _ = tx.send(SessionEvent::Token(text));
                }
                StreamEvent::ToolCall(tc) => {
                    let _ = tx.send(SessionEvent::ToolCall(tc.clone()));
                    tool_calls.push(tc);
                }
                StreamEvent::Done(usage) => {
                    messages.push(Message {
                        role: Role::Assistant,
                        content: assistant_text.clone(),
                        tool_calls: None,
                        tool_call_id: None,
                        name: None,
                    });

                    if tool_calls.is_empty() {
                        let _ = tx.send(SessionEvent::Done { usage: Some(usage) });
                        return TurnInput { messages, provider, tools, model, working_dir, session_id };
                    }

                    for tc in &tool_calls {
                        let args: serde_json::Value =
                            serde_json::from_str(&tc.function.arguments)
                                .unwrap_or(serde_json::Value::Null);
                        let ctx = ToolExecutionContext {
                            session_id: session_id.clone(),
                            working_dir: working_dir.clone(),
                        };

                        match tools.iter().find(|t| t.definition().name == tc.function.name) {
                            Some(tool) => match tool.execute(args, &ctx).await {
                                Ok(result) => {
                                    let content = result.content;
                                    messages.push(Message {
                                        role: Role::Tool,
                                        content: content.clone(),
                                        tool_calls: None,
                                        tool_call_id: Some(tc.id.clone()),
                                        name: Some(tc.function.name.clone()),
                                    });
                                    let _ = tx.send(SessionEvent::ToolResult {
                                        name: tc.function.name.clone(),
                                        content,
                                        is_error: result.is_error,
                                    });
                                }
                                Err(e) => {
                                    let err = format!("Error: {e}");
                                    messages.push(Message {
                                        role: Role::Tool,
                                        content: err.clone(),
                                        tool_calls: None,
                                        tool_call_id: Some(tc.id.clone()),
                                        name: Some(tc.function.name.clone()),
                                    });
                                    let _ = tx.send(SessionEvent::ToolResult {
                                        name: tc.function.name.clone(),
                                        content: err,
                                        is_error: true,
                                    });
                                }
                            },
                            None => {
                                let err = format!("Tool '{}' not found", tc.function.name);
                                messages.push(Message {
                                    role: Role::Tool,
                                    content: err.clone(),
                                    tool_calls: None,
                                    tool_call_id: Some(tc.id.clone()),
                                    name: Some(tc.function.name.clone()),
                                });
                                let _ = tx.send(SessionEvent::ToolResult {
                                    name: tc.function.name.clone(),
                                    content: err,
                                    is_error: true,
                                });
                            }
                        }
                    }
                    tool_calls.clear();
                }
                StreamEvent::Error(msg) => {
                    let _ = tx.send(SessionEvent::Error(msg));
                    return TurnInput { messages, provider, tools, model, working_dir, session_id };
                }
            }
        }
    }
}
