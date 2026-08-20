use axum::{
    Json,
    http::StatusCode,
    response::{IntoResponse, Response},
};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("Document not found")]
    NotFound,

    #[error("Invalid input: {0}")]
    Validation(String),

    #[error("Embedding generation failed: {0}")]
    Embedding(String),

    #[error("Database error: {0}")]
    Database(#[from] sqlx::Error),

    #[error("LLM error: {0}")]
    Llm(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            AppError::NotFound => (StatusCode::NOT_FOUND, self.to_string()),
            AppError::Validation(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            _ => (
                StatusCode::INTERNAL_SERVER_ERROR,
                self.to_string(),
            ),
        };
        (status, Json(json!({ "error": message }))).into_response()
    }
}
