use std::sync::Arc;

use aa_core::llm::*;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "aa", version, about = "aaBot - 个人 AI 助手")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    Run(RunArgs),
    Tool(ToolArgs),
    Extension(ExtensionArgs),
}

#[derive(clap::Args)]
struct RunArgs {
    #[arg(short, long, default_value = ".")]
    working_dir: String,
    #[arg(long)]
    no_llm: bool,
    #[arg(long, default_value = "deepseek-chat")]
    model: String,
    #[arg(long, default_value = "https://api.deepseek.com")]
    base_url: String,
    #[arg(long, default_value = "openai")]
    provider: String,
}

#[derive(clap::Args)]
struct ToolArgs {
    #[command(subcommand)]
    command: ToolCommand,
}

#[derive(Subcommand)]
enum ToolCommand {
    List,
    Call { name: String, arguments: String },
}

#[derive(clap::Args)]
struct ExtensionArgs {
    #[command(subcommand)]
    command: ExtensionCommand,
}

#[derive(Subcommand)]
enum ExtensionCommand {
    List,
}

fn main() {
    let args = Cli::parse();
    let rt = tokio::runtime::Runtime::new().expect("Failed to create tokio runtime");
    tracing_subscriber::fmt::init();

    match args.command {
        Command::Run(ref run_args) => rt.block_on(cmd_run(run_args)),
        Command::Tool(ref tool_args) => rt.block_on(cmd_tool(tool_args)),
        Command::Extension(ref ext_args) => rt.block_on(cmd_extension(ext_args)),
    }
}

async fn cmd_run(args: &RunArgs) {
    let kernel = build_kernel();
    let registry = build_registry(&kernel, &args.working_dir);
    let tools = registry.all_definitions();

    println!("aaBot ready | tools: {}", tools.len());

    if args.no_llm {
        for t in &tools {
            println!("  {} - {}", t.name, t.description);
        }
        return;
    }

    let provider: Arc<dyn ModelProvider> = match args.provider.as_str() {
        "ollama" => {
            let config = aa_ollama::OllamaConfig {
                base_url: args.base_url.clone(),
                default_model: args.model.clone(),
            };
            Arc::new(aa_ollama::OllamaProvider::new(config))
        }
        _ => {
            let config = aa_llm::OpenAiConfig {
                base_url: args.base_url.clone(),
                api_key: std::env::var("AA_API_KEY").unwrap_or_default(),
                default_model: args.model.clone(),
            };
            Arc::new(aa_llm::OpenAiCompatibleProvider::new(config))
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

        loop {
            let request = ModelRequest {
                messages: messages.clone(),
                tools: tools.iter().map(|t| serde_json::to_value(t).unwrap()).collect(),
                config: ModelConfig {
                    provider: provider.id(),
                    model: args.model.clone(),
                    temperature: None,
                    max_tokens: None,
                    top_p: None,
                },
            };
            let req = request.clone();

            match provider.chat(req).await {

                Ok(response) => {
                    if let Some(tcs) = &response.message.tool_calls {
                        let wd = args.working_dir.clone();
                        for tc in tcs {
                            print_tool_call(&tc);
                            let tool_args: serde_json::Value =
                                serde_json::from_str(&tc.function.arguments)
                                    .unwrap_or(serde_json::Value::Null);
                            let ctx = aa_kernel::tool_provider::ToolExecutionContext {
                                session_id: "cli".into(),
                                working_dir: wd.clone(),
                            };

                            match registry.find(&tc.function.name) {
                                Some(tool) => match tool.execute(tool_args, &ctx).await {
                                    Ok(result) => {
                                        messages.push(Message {
                                            role: Role::Tool,
                                            content: result.content,
                                            tool_calls: None,
                                            tool_call_id: Some(tc.id.clone()),
                                            name: Some(tc.function.name.clone()),
                                        });
                                        let status = if result.is_error { "error" } else { "ok" };
                                        println!("  → {status}");
                                    }
                                    Err(e) => {
                                        messages.push(Message {
                                            role: Role::Tool,
                                            content: format!("Error: {e}"),
                                            tool_calls: None,
                                            tool_call_id: Some(tc.id.clone()),
                                            name: Some(tc.function.name.clone()),
                                        });
                                        println!("  → error: {e}");
                                    }
                                },
                                None => {
                                    messages.push(Message {
                                        role: Role::Tool,
                                        content: format!("Tool '{}' not found", tc.function.name),
                                        tool_calls: None,
                                        tool_call_id: Some(tc.id.clone()),
                                        name: Some(tc.function.name.clone()),
                                    });
                                    println!("  → tool not found");
                                }
                            }
                        }
                    } else {
                        let content = response.message.content.clone();
                        messages.push(response.message);
                        println!("\n{content}\n");
                        break;
                    }
                }
                Err(e) => {
                    eprintln!("LLM error: {e}");
                    break;
                }
            }
        }
    }
}

fn print_tool_call(tc: &ToolCall) {
    println!("\n  tool: {} ({})", tc.function.name, tc.id);
}

async fn cmd_tool(args: &ToolArgs) {
    let kernel = build_kernel();
    let registry = build_registry(&kernel, ".");

    match &args.command {
        ToolCommand::List => {
            for t in registry.all_definitions() {
                println!("{}: {}", t.name, t.description);
            }
        }
        ToolCommand::Call { name, arguments } => {
            let args: serde_json::Value =
                serde_json::from_str(arguments).unwrap_or(serde_json::Value::Null);
            let ctx = aa_kernel::tool_provider::ToolExecutionContext {
                session_id: "cli".into(),
                working_dir: ".".into(),
            };

            match registry.find(name) {
                Some(tool) => match tool.execute(args, &ctx).await {
                    Ok(result) => println!("{}", result.content),
                    Err(e) => eprintln!("Error: {e}"),
                },
                None => eprintln!("Tool '{name}' not found"),
            }
        }
    }
}

async fn cmd_extension(_args: &ExtensionArgs) {
    println!("No extensions loaded (built-in only)");
}

fn build_kernel() -> aa_kernel::Kernel {
    aa_kernel::Kernel::builder()
        .with_tool_provider(Arc::new(aa_function_tools::FsToolProvider))
        .build()
}

fn build_registry(
    kernel: &aa_kernel::Kernel,
    working_dir: &str,
) -> aa_kernel::ToolRegistry {
    let scope = aa_kernel::ToolProviderScope::new(working_dir);
    kernel.build_tool_registry(&scope)
}
