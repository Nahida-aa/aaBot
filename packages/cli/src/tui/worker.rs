use std::sync::{Arc, mpsc};

use super::types::{UiEvent, WorkerCommand};

pub async fn worker_main(
    worker_rx: mpsc::Receiver<WorkerCommand>,
    ui_tx: mpsc::Sender<UiEvent>,
    provider_name: Option<&str>,
    model_name: Option<&str>,
    base_url: Option<&str>,
    working_dir: &str,
) -> anyhow::Result<()> {
    let cfg = aa_config::Config::load();
    let resolved = cfg.resolve(provider_name, model_name, base_url);
    let effective_model = resolved.model.clone();

    let provider: Arc<dyn aa_core::llm::ModelProvider> = match resolved.provider.as_str() {
        "ollama" => Arc::new(aa_ollama::OllamaProvider::new(aa_ollama::OllamaConfig {
            base_url: resolved.base_url,
            default_model: resolved.model,
        })),
        _ => Arc::new(aa_llm::OpenAiCompatibleProvider::new(aa_llm::OpenAiConfig {
            base_url: resolved.base_url,
            api_key: resolved.api_key,
            default_model: resolved.model,
        })),
    };

    let kernel = aa_kernel::Kernel::builder()
        .with_tool_provider(Arc::new(aa_function_tools::FsToolProvider))
        .build();
    let scope = aa_kernel::ToolProviderScope::new(working_dir);
    let registry = kernel.build_tool_registry(&scope);
    let tools = registry.all_tools();
    let tool_count = tools.len();

    let _ = ui_tx.send(UiEvent::Ready { tools: tool_count, model: effective_model.clone() });

    let mut messages: Vec<aa_core::llm::Message> = vec![aa_core::llm::Message {
        role: aa_core::llm::Role::System,
        content: "You are aaBot, an AI assistant. You have access to filesystem tools. \
                  Help the user with their tasks.".into(),
        tool_calls: None,
        tool_call_id: None,
        name: None,
    }];

    while let Ok(cmd) = worker_rx.recv() {
        match cmd {
            WorkerCommand::Submit { messages: new_msgs } => {
                messages.extend(new_msgs);
            }
        }

        let (session_tx, session_rx) = mpsc::channel::<aa_session::SessionEvent>();

        let input = aa_session::TurnInput {
            messages,
            provider: provider.clone(),
            tools: tools.clone(),
            model: effective_model.clone(),
            working_dir: working_dir.into(),
            session_id: "tui".into(),
        };

        let handle = tokio::spawn(aa_session::run_turn(input, session_tx));

        while let Ok(event) = session_rx.recv() {
            match event {
                aa_session::SessionEvent::Token(text) => { let _ = ui_tx.send(UiEvent::Token(text)); }
                aa_session::SessionEvent::ToolCall(tc) => { let _ = ui_tx.send(UiEvent::ToolCall(tc)); }
                aa_session::SessionEvent::ToolResult { name, content, is_error } => {
                    let _ = ui_tx.send(UiEvent::ToolResult { name, content, is_error });
                }
                aa_session::SessionEvent::Done { usage } => { let _ = ui_tx.send(UiEvent::Done { usage }); }
                aa_session::SessionEvent::Error(msg) => { let _ = ui_tx.send(UiEvent::Error(msg)); }
            }
        }

        let result = handle.await?;
        messages = result.messages;
    }

    Ok(())
}
