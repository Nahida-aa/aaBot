use aa_core::llm::{
    Message as CoreMessage, ModelError, ModelProvider, ModelRequest, ModelResponse, ProviderId,
    Role, StreamEvent, ToolCall as CoreToolCall, ToolCallFunction as CoreFunction,
    Usage as CoreUsage,
};
use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;

use crate::types;

pub struct OllamaProvider {
    client: Client,
    config: types::OllamaConfig,
}

impl OllamaProvider {
    pub fn new(config: types::OllamaConfig) -> Self {
        Self {
            client: Client::new(),
            config,
        }
    }
}

#[async_trait]
impl ModelProvider for OllamaProvider {
    fn id(&self) -> ProviderId {
        ProviderId("ollama".into())
    }

    async fn chat(&self, request: ModelRequest) -> Result<ModelResponse, ModelError> {
        let body = build_request(&request, &self.config, false);
        let url = format!("{}/api/chat", self.config.base_url);

        let resp = self
            .client
            .post(&url)
            .json(&body)
            .send()
            .await
            .map_err(|e| ModelError::Provider(format!("request: {e}")))?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return Err(ModelError::Provider(format!("HTTP {status}: {text}")));
        }

        let data: types::ChatResponse = resp
            .json()
            .await
            .map_err(|e| ModelError::Provider(format!("parse: {e}")))?;

        let mut tool_calls = Vec::new();
        for (i, tc) in data.message.tool_calls.iter().enumerate() {
            let args_str = serde_json::to_string(&tc.function.arguments).unwrap_or_default();
            tool_calls.push(CoreToolCall {
                id: format!("ollama_tc_{i}"),
                call_type: "function".into(),
                function: CoreFunction {
                    name: tc.function.name.clone(),
                    arguments: args_str,
                },
            });
        }

        let usage = data
            .prompt_eval_count
            .zip(data.eval_count)
            .map(|(p, c)| CoreUsage {
                prompt_tokens: p,
                completion_tokens: c,
                total_tokens: p + c,
            });

        Ok(ModelResponse {
            message: CoreMessage {
                role: role_from_str(&data.message.role),
                content: data.message.content,
                tool_calls: None,
                tool_call_id: None,
                name: None,
            },
            tool_calls,
            usage,
            provider: ProviderId("ollama".into()),
        })
    }

    async fn chat_stream(
        &self,
        request: ModelRequest,
    ) -> Result<tokio::sync::mpsc::Receiver<StreamEvent>, ModelError> {
        let body = build_request(&request, &self.config, true);
        let url = format!("{}/api/chat", self.config.base_url);
        let (tx, rx) = tokio::sync::mpsc::channel(64);

        let client = self.client.clone();
        tokio::spawn(async move {
            if let Err(e) = stream_worker(client, &url, body, tx).await {
                tracing::error!("ollama stream worker: {e}");
            }
        });

        Ok(rx)
    }
}

async fn stream_worker(
    client: Client,
    url: &str,
    body: types::ChatRequest,
    tx: tokio::sync::mpsc::Sender<StreamEvent>,
) -> Result<(), ModelError> {
    let resp = client
        .post(url)
        .json(&body)
        .send()
        .await
        .map_err(|e| ModelError::Provider(format!("request: {e}")))?;

    let status = resp.status();
    if !status.is_success() {
        let text = resp.text().await.unwrap_or_default();
        let _ = tx
            .send(StreamEvent::Error(format!("HTTP {status}: {text}")))
            .await;
        return Err(ModelError::Provider(format!("HTTP {status}: {text}")));
    }

    let mut final_usage: Option<CoreUsage> = None;
    let mut stream = resp.bytes_stream();
    let mut buf = String::new();

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.map_err(|e| ModelError::Provider(format!("stream: {e}")))?;
        buf.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(line_end) = buf.find('\n') {
            let line = buf[..line_end].trim().to_owned();
            buf = buf[line_end + 1..].to_owned();

            if line.is_empty() {
                continue;
            }

            let data: types::ChatResponse = match serde_json::from_str(&line) {
                Ok(d) => d,
                Err(_) => continue,
            };

            if !data.message.content.is_empty() {
                let _ = tx.send(StreamEvent::Chunk(data.message.content)).await;
            }

            if data.done {
                if !data.message.tool_calls.is_empty() {
                    for (i, tc) in data.message.tool_calls.iter().enumerate() {
                        let args_str =
                            serde_json::to_string(&tc.function.arguments).unwrap_or_default();
                        let _ = tx
                            .send(StreamEvent::ToolCall(CoreToolCall {
                                id: format!("ollama_tc_{i}"),
                                call_type: "function".into(),
                                function: CoreFunction {
                                    name: tc.function.name.clone(),
                                    arguments: args_str,
                                },
                            }))
                            .await;
                    }
                }

                final_usage = data
                    .prompt_eval_count
                    .zip(data.eval_count)
                    .map(|(p, c)| CoreUsage {
                        prompt_tokens: p,
                        completion_tokens: c,
                        total_tokens: p + c,
                    });

                break;
            }
        }
    }

    let usage = final_usage.unwrap_or(CoreUsage {
        prompt_tokens: 0,
        completion_tokens: 0,
        total_tokens: 0,
    });
    let _ = tx.send(StreamEvent::Done(usage)).await;
    Ok(())
}

fn build_request(
    req: &ModelRequest,
    config: &types::OllamaConfig,
    stream: bool,
) -> types::ChatRequest {
    let model = if req.config.model.is_empty() {
        config.default_model.clone()
    } else {
        req.config.model.clone()
    };

    let messages: Vec<types::Message> = req
        .messages
        .iter()
        .map(|m| {
            let role = match m.role {
                Role::System => "system",
                Role::User => "user",
                Role::Assistant => "assistant",
                Role::Tool => "tool",
            };

            let tool_calls = m.tool_calls.as_ref().map(|calls| {
                calls
                    .iter()
                    .map(|c| {
                        let args: serde_json::Value = serde_json::from_str(&c.function.arguments)
                            .unwrap_or(serde_json::Value::Null);
                        types::ToolCall {
                            function: types::ToolCallFunction {
                                name: c.function.name.clone(),
                                arguments: args,
                            },
                        }
                    })
                    .collect()
            });

            let is_tool_call_msg = m.role == Role::Assistant && tool_calls.is_some();
            let content = if is_tool_call_msg && m.content.is_empty() {
                None
            } else {
                Some(m.content.clone())
            };

            types::Message {
                role: role.to_owned(),
                content,
                tool_calls,
                images: None,
            }
        })
        .collect();

    let tools = if req.tools.is_empty() {
        None
    } else {
        Some(
            req.tools
                .iter()
                .map(|t| types::Tool {
                    tool_type: "function".into(),
                    function: types::ToolFunction {
                        name: t["name"].as_str().unwrap_or("").to_owned(),
                        description: t["description"].as_str().unwrap_or("").to_owned(),
                        parameters: t
                            .get("parameters")
                            .cloned()
                            .unwrap_or(serde_json::Value::Null),
                    },
                })
                .collect(),
        )
    };

    types::ChatRequest {
        model,
        messages,
        tools,
        stream,
        options: Some(types::Options {
            temperature: req.config.temperature,
            num_predict: req.config.max_tokens,
            top_p: req.config.top_p,
        }),
    }
}

fn role_from_str(s: &str) -> Role {
    match s {
        "system" => Role::System,
        "assistant" => Role::Assistant,
        "tool" => Role::Tool,
        _ => Role::User,
    }
}
