use soar_cli::init;

#[tokio::main]
async fn main() {
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }

    init().await;
}
