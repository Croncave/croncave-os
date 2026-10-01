use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::try_from_env("CRONCAVE_AGENT_LOG").unwrap_or_else(|_| EnvFilter::new("info")))
        .with_writer(std::io::stderr)
        .init();

    let mut args = std::env::args().skip(1);
    match args.next().as_deref() {
        // The stand-in for a provider's coding agent tool, run as a child of a Code task.
        Some("mock-coder") => croncave_agent::coder::mock_main(args.collect()).await,
        Some("--version") => {
            println!("croncave-agent {}", croncave_agent::VERSION);
            Ok(())
        }
        _ => croncave_agent::run(croncave_agent::AgentConfig::from_env()?).await,
    }
}
