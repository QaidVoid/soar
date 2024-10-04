use soar::init;

use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    init().await
}
