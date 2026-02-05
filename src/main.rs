use clap::Parser;
use zorvia::cli::Cli;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    zorvia::run(cli).await
}
