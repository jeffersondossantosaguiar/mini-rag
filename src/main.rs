use std::{env, net::SocketAddr, time::Duration};

use axum::{
    Router,
    routing::{get, post},
};
use dotenvy::dotenv;
use sqlx::{PgPool, postgres::PgPoolOptions};
use tokio::net::TcpListener;
use tower_governor::{governor::GovernorConfigBuilder, GovernorLayer};
use tracing_subscriber::fmt;

use crate::{
    embedding::Embedder,
    handlers::{
        get_document_handler, health_check_handler, ingest_document_handler, query_handler,
    },
    llm::LlmClient,
};

mod chunking;
mod db;
mod embedding;
mod error;
mod handlers;
mod llm;

#[derive(Clone)]
struct AppState {
    pool: PgPool,
    embedder: Embedder,
    llm: LlmClient,
}

#[tokio::main]
async fn main() {
    fmt::init();
    dotenv().ok();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let llm_base_url = env::var("LLM_BASE_URL").expect("LLM_BASE_URL must be set");
    let llm_model = env::var("LLM_MODEL").expect("LLM_MODEL must be set");

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .expect("failed to connect to database");

    let embedder = Embedder::new().expect("failed to load embedding model");

    let llm = LlmClient::new(&llm_base_url, &llm_model);

    let state = AppState {
        pool,
        embedder,
        llm,
    };

    let governor_conf = GovernorConfigBuilder::default()
        .per_second(1)
        .burst_size(10)
        .finish()
        .unwrap();

    let governor_limiter = governor_conf.limiter().clone();
    std::thread::spawn(move || {
        loop {
            std::thread::sleep(Duration::from_secs(60));
            governor_limiter.retain_recent();
        }
    });

    let app = Router::new()
        .route("/health", get(health_check_handler))
        .route("/documents", post(ingest_document_handler))
        .route("/documents/{id}", get(get_document_handler))
        .route("/query", post(query_handler))
        .layer(GovernorLayer::new(governor_conf))
        .with_state(state);

    let listener = TcpListener::bind("0.0.0.0:3000").await.unwrap();

    tracing::info!("Listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .await
        .unwrap();
}
