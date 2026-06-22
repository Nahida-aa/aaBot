#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    aa_server::serve(3000, None, None, None).await
}
