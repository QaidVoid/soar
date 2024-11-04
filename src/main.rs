#![allow(clippy::needless_return)]

use soar_cli::{core::color::Color, core::color::ColorExt, error, init};

#[tokio::main]
async fn main() {
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }

    if let Err(e) = init().await {
        error!("{}", e);
    }
}
