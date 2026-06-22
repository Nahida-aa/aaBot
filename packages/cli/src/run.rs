use std::sync::Arc;

use aa_core::llm::{Message, ModelProvider, Role};
use clap::Args;

#[derive(Args)]
pub struct RunArgs {
    #[arg(short, long, default_value = ".")]
    pub working_dir: String,
    #[arg(long)]
    pub no_llm: bool,
    #[arg(long)]
    pub model: Option<String>,
    #[arg(long)]
    pub base_url: Option<String>,
    #[arg(long)]
    pub provider: Option<String>,
}

pub async fn cmd_run(
    args: &RunArgs,
    _kernel: &aa_kernel::Kernel,
    registry: &aa_kernel::ToolRegistry,
) {
    let tool_defs = registry.all_definitions();
    let tool_instances = registry.all_tools();

    println!("aaBot ready | tools: {}", tool_defs.len());

    if args.no_llm {
        for t in &tool_defs {
            println!("  {} - {}", t.name, t.description);
        }
        return;
    }

    let cfg = aa_config::Config::load();
    let resolved = cfg.resolve(
        args.provider.as_deref(),
        args.model.as_deref(),
        args.base_url.as_deref(),
    );

    let provider: Arc<dyn ModelProvider> = match resolved.provider.as_str() {
        "ollama" => {
            Arc::new(aa_ollama::OllamaProvider::new(aa_ollama::OllamaConfig {
                base_url: resolved.base_url,
                default_model: resolved.model.clone(),
            }))
        }
        _ => {
            Arc::new(aa_llm::OpenAiCompatibleProvider::new(aa_llm::OpenAiConfig {
                base_url: resolved.base_url,
                api_key: resolved.api_key,
                default_model: resolved.model.clone(),
            }))
        }
    };

    let mut messages: Vec<Message> = vec![Message {
        role: Role::System,
        content: "You are aaBot, an AI assistant. You have access to filesystem tools. Help the user with their tasks.".into(),
        tool_calls: None,
        tool_call_id: None,
        name: None,
    }];

    println!("Type your message (Ctrl+C to exit)\n");

    loop {
        let mut input = String::new();
        if std::io::stdin().read_line(&mut input).is_err() || input.trim().is_empty() {
            break;
        }
        let input = input.trim().to_owned();

        messages.push(Message {
            role: Role::User,
            content: input,
            tool_calls: None,
            tool_call_id: None,
            name: None,
        });

        let (tx, rx) = std::sync::mpsc::channel();

        let input = aa_session::TurnInput {
            messages,
            provider: provider.clone(),
            tools: tool_instances.clone(),
            model: resolved.model.clone(),
            working_dir: args.working_dir.clone(),
            session_id: "cli".into(),
        };

        let handle = tokio::spawn(aa_session::run_turn(input, tx));

        while let Ok(event) = rx.recv() {
            match event {
                aa_session::SessionEvent::Token(text) => {
                    print!("{text}");
                    use std::io::Write;
                    std::io::stdout().flush().ok();
                }
                aa_session::SessionEvent::ToolCall(tc) => {
                    println!("\n  tool: {} ({}) ...", tc.function.name, tc.id);
                }
                aa_session::SessionEvent::ToolResult { is_error, .. } => {
                    let status = if is_error { "error" } else { "ok" };
                    println!("  → {status}");
                }
                aa_session::SessionEvent::Done { usage } => {
                    println!();
                    if let Some(u) = usage {
                        eprintln!(
                            "  tokens: {} prompt + {} completion = {} total",
                            u.prompt_tokens, u.completion_tokens, u.total_tokens
                        );
                    }
                }
                aa_session::SessionEvent::Error(msg) => {
                    eprintln!("\nError: {msg}");
                }
            }
        }

        let result = handle.await.expect("turn task panicked");
        messages = result.messages;
    }
}
