use std::sync::Arc;

use clap::{Parser, Subcommand};

mod clipboard;
mod markdown;
mod run;
mod tui;

#[derive(Parser)]
#[command(name = "aa", version, about = "aaBot - 个人 AI 助手")]
struct Cli {
    #[command(subcommand)]
    command: Option<Command>,

    /// LLM provider
    #[arg(long, global = true)]
    pub provider: Option<String>,
    /// Model name
    #[arg(long, global = true)]
    pub model: Option<String>,
    /// API base URL
    #[arg(long, global = true)]
    pub base_url: Option<String>,
    /// Working directory
    #[arg(long, global = true)]
    pub working_dir: Option<String>,
}

#[derive(Subcommand)]
enum Command {
    /// 行模式对话
    Run(run::RunArgs),
    /// 工具管理
    Tool(ToolArgs),
    /// 扩展管理
    Extension(ExtensionArgs),
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
    tracing_subscriber::fmt::init();
    let args = Cli::parse();

    let working_dir = args.working_dir.as_deref().unwrap_or(".");

    match args.command {
        None => tui::run_tui(args.provider.as_deref(), args.model.as_deref(), args.base_url.as_deref(), working_dir),
        Some(Command::Run(ref run_args)) => {
            let rt = tokio::runtime::Runtime::new().expect("tokio rt");
            rt.block_on(async {
                let kernel = build_kernel();
                let registry = build_registry(&kernel, working_dir);
                run::cmd_run(run_args, &kernel, &registry).await;
            });
        }
        Some(Command::Tool(ref tool_args)) => {
            let rt = tokio::runtime::Runtime::new().expect("tokio rt");
            rt.block_on(cmd_tool(tool_args));
        }
        Some(Command::Extension(ref _ext_args)) => {
            println!("No extensions loaded (built-in only)");
        }
    }
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

pub(crate) fn build_kernel() -> aa_kernel::Kernel {
    aa_kernel::Kernel::builder()
        .with_tool_provider(Arc::new(aa_function_tools::FsToolProvider))
        .build()
}

pub(crate) fn build_registry(
    kernel: &aa_kernel::Kernel,
    working_dir: &str,
) -> aa_kernel::ToolRegistry {
    let scope = aa_kernel::ToolProviderScope::new(working_dir);
    kernel.build_tool_registry(&scope)
}
