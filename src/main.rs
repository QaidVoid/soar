#![allow(clippy::needless_return)]

use soar_cli::init;
use tracing::error;

#[tokio::main]
async fn main() {
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }

    if let Err(e) = init().await {
        error!("{}", e);
    }
}
