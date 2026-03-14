use clap::Parser;
use ferrobus_server::{ServerCli, init_tracing, run_server};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_tracing();
    let cli = ServerCli::parse();
    run_server(cli).await
}
