use std::env;

use axum::{
    Router,
    routing::{get, post},
};
use dotenvy::dotenv;
use sqlx::{PgPool, postgres::PgPoolOptions};
use tokio::net::TcpListener;
use tracing_subscriber::fmt;

use crate::handlers::{get_document_handler, health_check_handler, ingest_document_handler};

mod chunking;
mod db;
mod handlers;

#[derive(Clone)]
struct AppState {
    pool: PgPool,
}

#[tokio::main]
async fn main() {
    fmt::init();
    dotenv().ok();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("failed to connect to database");

    let state = AppState { pool };

    let app = Router::new()
        .route("/health", get(health_check_handler))
        .route("/documents", post(ingest_document_handler))
        .route("/documents/{id}", get(get_document_handler))
        .with_state(state);

    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();

    tracing::info!("Listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.unwrap();
}
