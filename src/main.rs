use axum::{Router, routing::get};
use tokio::net::TcpListener;
use tracing_subscriber::fmt::init;

#[tokio::main]
async fn main() {
    init();

    let app = Router::new().route("/health", get(health_check));

    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();

    tracing::info!("Listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();
}

async fn health_check() -> &'static str {
    "OK"
}
