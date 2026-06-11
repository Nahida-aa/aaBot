use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "aa", version, about = "aaBot - 个人 AI 助手")]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// 启动交互式对话
    Run(RunArgs),
    /// 工具相关操作
    Tool(ToolArgs),
    /// 扩展相关操作
    Extension(ExtensionArgs),
}

#[derive(clap::Args)]
struct RunArgs {
    /// 工作目录
    #[arg(short, long, default_value = ".")]
    working_dir: String,
    /// 是否接入 LLM（否则只连接工具）
    #[arg(long)]
    no_llm: bool,
}

#[derive(clap::Args)]
struct ToolArgs {
    #[command(subcommand)]
    command: ToolCommand,
}

#[derive(Subcommand)]
enum ToolCommand {
    List,
    Call {
        name: String,
        arguments: String,
    },
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
        Command::Run(run_args) => {
            rt.block_on(cmd_run(&run_args));
        }
        Command::Tool(tool_args) => {
            rt.block_on(cmd_tool(&tool_args));
        }
        Command::Extension(ext_args) => {
            rt.block_on(cmd_extension(&ext_args));
        }
    }
}

async fn cmd_run(args: &RunArgs) {
    let kernel = build_kernel(args.working_dir.as_str());
    let registry = build_registry(&kernel, args.working_dir.as_str());

    let tools = registry.all_definitions();
    println!("aaBot ready | tools: {} | llm: {}", tools.len(), !args.no_llm);

    for t in &tools {
        println!("  tool: {} - {}", t.name, t.description);
    }

    if !args.no_llm {
        println!("LLM session mode (not yet implemented)");
    }
}

async fn cmd_tool(args: &ToolArgs) {
    let kernel = build_kernel(".");
    let registry = build_registry(&kernel, ".");

    match &args.command {
        ToolCommand::List => {
            let tools = registry.all_definitions();
            for t in &tools {
                println!("{}: {}", t.name, t.description);
            }
        }
        ToolCommand::Call {
            name,
            arguments,
        } => {
            let args: serde_json::Value =
                serde_json::from_str(arguments).unwrap_or(serde_json::Value::Null);
            let scope = aa_kernel::ToolPackScope::new(".");
            let ctx = aa_kernel::tool_pack::ToolExecutionContext {
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

async fn cmd_extension(args: &ExtensionArgs) {
    match &args.command {
        ExtensionCommand::List => {
            println!("No extensions loaded (built-in only)");
        }
    }
}

fn build_kernel(working_dir: &str) -> aa_kernel::Kernel {
    let mut builder = aa_kernel::Kernel::builder();
    builder = builder.with_tool_pack(std::sync::Arc::new(aa_extension_fs::FsToolPack));
    builder.build()
}

fn build_registry(
    kernel: &aa_kernel::Kernel,
    working_dir: &str,
) -> aa_kernel::ToolRegistry {
    let scope = aa_kernel::ToolPackScope::new(working_dir);
    kernel.build_tool_registry(&scope)
}
