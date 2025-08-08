#![warn(clippy::print_stdout, clippy::print_stderr, clippy::unwrap_used)]

#[allow(unused_imports)]
#[macro_use]
extern crate tracing;

#[tokio::main]
async fn main() {
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_env_filter("none,battlesnakes=trace")
        .finish();
    tracing::subscriber::set_global_default(subscriber).expect("failed set up tracing");
}
