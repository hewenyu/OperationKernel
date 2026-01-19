use anyhow::Result;

/// Main entry point
#[tokio::main]
async fn main() -> Result<()> {
    ok::cli::run().await
}
