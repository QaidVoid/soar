#![allow(clippy::needless_return)]

use soar::init;

#[tokio::main]
async fn main() {
    if let Err(e) = init().await {
        eprintln!("{}", e);
    }
}
