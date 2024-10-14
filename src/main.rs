#![allow(clippy::needless_return)]

use soar::{core::color::Color, core::color::ColorExt, error, init};

#[tokio::main]
async fn main() {
    if let Err(e) = init().await {
        error!("{}", e);
    }
}
