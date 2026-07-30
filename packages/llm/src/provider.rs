use std::collections::HashMap;

use aa_core::llm::{
    Message as CoreMessage, ModelError, ModelProvider, ModelRequest, ModelResponse, ProviderId,
    Role, StreamEvent, ToolCall as CoreToolCall, ToolCallFunction as CoreFunction,
    Usage as CoreUsage,
};
use async_trait::async_trait;
use futures_util::StreamExt;
use reqwest::Client;

use crate::types;

pub struct OpenAiCompatibleProvider {
    client: Client,
    config: types::OpenAiConfig,
}

impl OpenAiCompatibleProvider {
    pub fn new(config: types::OpenAiConfig) -> Self {
        Self {
            client: Client::new(),
            config,
        }
    }
}

#[async_trait]
impl ModelProvider for OpenAiCompatibleProvider {
    fn id(&self) -> ProviderId {
        ProviderId("openai-compatible".into())
    }

    async fn chat(&self, request: ModelRequest) -> Result<ModelResponse, ModelError> {
        let body = build_request(&request, &self.config, false);
        let url = format!("{}/chat/completions", self.config.base_url);

        let resp = self
            .client
            .post(&url)
            .header("Authorization", format!("Bearer {}", self.config.api_key))
            .json(&body)
            .send()
            .await
            .map_err(|e| ModelError::Provider(format!("request: {e}")))?;

        let status = resp.status();
        if !status.is_success() {
            let text = resp.text().await.unwrap_or_default();
            return if status.as_u16() == 429 {
                Err(ModelError::RateLimited(30))
            } else {
                Err(ModelError::Provider(format!("HTTP {status}: {text}")))
            };
        }

        let data: types::ChatResponse = resp
            .json()
            .await
            .map_err(|e| ModelError::Provider(format!("parse: {e}")))?;

        let choice = data
            .choices
            .into_iter()
            .next()
            .ok_or_else(|| ModelError::Provider("no choices".into()))?;

        Ok(ModelResponse {
            message: CoreMessage {
                role: role_from_str(&choice.message.role),
                content: choice.message.content.unwrap_or_default(),
                tool_calls: choice.message.tool_calls.map(|calls| {
                    calls
                        .into_iter()
                        .map(|c| CoreToolCall {
                            id: c.id,
                            call_type: c.call_type,
                            function: CoreFunction {
                                name: c.function.name,
                                arguments: c.function.arguments,
                            },
                        })
                        .collect()
                }),
                tool_call_id: None,
                name: None,
            },
            tool_calls: vec![],
            usage: data.usage.map(|u| CoreUsage {
                prompt_tokens: u.prompt_tokens,
                completion_tokens: u.completion_tokens,
                total_tokens: u.total_tokens,
            }),
            provider: ProviderId("openai-compatible".into()),
        })
    }

    async fn chat_stream(
        &self,
        request: ModelRequest,
    ) -> Result<tokio::sync::mpsc::Receiver<StreamEvent>, ModelError> {
        let body = build_request(&request, &self.config, true);
        let url = format!("{}/chat/completions", self.config.base_url);
        let api_key = self.config.api_key.clone();
        let (tx, rx) = tokio::sync::mpsc::channel(64);

        let client = self.client.clone();
        tokio::spawn(async move {
            if let Err(e) = stream_worker(client, &url, &api_key, body, tx).await {
                tracing::error!("stream worker: {e}");
            }
        });

        Ok(rx)
    }
}

async fn stream_worker(
    client: Client,
    url: &str,
    api_key: &str,
    body: types::ChatRequest,
    tx: tokio::sync::mpsc::Sender<StreamEvent>,
) -> Result<(), ModelError> {
    let resp = client
        .post(url)
        .header("Authorization", format!("Bearer {api_key}"))
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
        return if status.as_u16() == 429 {
            Err(ModelError::RateLimited(30))
        } else {
            Err(ModelError::Provider(format!("HTTP {status}: {text}")))
        };
    }

    let mut tool_buffers: HashMap<u64, (String, String, String)> = HashMap::new();
    let mut final_usage: Option<CoreUsage> = None;

    let mut stream = resp.bytes_stream();
    let mut buf = String::new();

    while let Some(chunk_result) = stream.next().await {
        let chunk = chunk_result.map_err(|e| ModelError::Provider(format!("stream: {e}")))?;
        buf.push_str(&String::from_utf8_lossy(&chunk));

        while let Some(line_end) = buf.find('\n') {
            let line = buf[..line_end].trim().to_owned();
            buf = buf[line_end + 1..].to_owned();

            if line.is_empty() || line.starts_with(':') {
                continue;
            }

            let json_str = if let Some(data) = line.strip_prefix("data: ") {
                data
            } else {
                continue;
            };

            if json_str == "[DONE]" {
                continue;
            }

            let chunk: types::StreamChunk = match serde_json::from_str(json_str) {
                Ok(c) => c,
                Err(_) => continue,
            };

            if let Some(ref u) = chunk.usage {
                final_usage = Some(CoreUsage {
                    prompt_tokens: u.prompt_tokens,
                    completion_tokens: u.completion_tokens,
                    total_tokens: u.total_tokens,
                });
            }

            for choice in chunk.choices {
                if let Some(ref text) = choice.delta.content {
                    if !text.is_empty() {
                        let _ = tx.send(StreamEvent::Chunk(text.clone())).await;
                    }
                }

                if let Some(ref tool_calls) = choice.delta.tool_calls {
                    for tc in tool_calls {
                        let entry = tool_buffers
                            .entry(tc.index)
                            .or_insert_with(|| (String::new(), String::new(), String::new()));
                        if let Some(ref id) = tc.id {
                            entry.0 = id.clone();
                        }
                        if let Some(name) = tc.function.as_ref().and_then(|f| f.name.as_ref()) {
                            entry.1 = name.to_string();
                        }
                        if let Some(ref args) =
                            tc.function.as_ref().and_then(|f| f.arguments.as_ref())
                        {
                            entry.2.push_str(args);
                        }
                    }
                }

                if let Some(ref reason) = choice.finish_reason {
                    if reason == "tool_calls" {
                        for (_idx, (id, name, args)) in tool_buffers.drain() {
                            let _ = tx
                                .send(StreamEvent::ToolCall(CoreToolCall {
                                    id,
                                    call_type: "function".into(),
                                    function: CoreFunction {
                                        name,
                                        arguments: args,
                                    },
                                }))
                                .await;
                        }
                    }
                }
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
    config: &types::OpenAiConfig,
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
                    .map(|c| types::ToolCall {
                        id: c.id.clone(),
                        call_type: c.call_type.clone(),
                        function: types::ToolCallFunction {
                            name: c.function.name.clone(),
                            arguments: c.function.arguments.clone(),
                        },
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
                tool_call_id: m.tool_call_id.clone(),
                name: m.name.clone(),
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
        temperature: req.config.temperature,
        max_tokens: req.config.max_tokens,
        top_p: req.config.top_p,
        stream: if stream { Some(true) } else { None },
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
