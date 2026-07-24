use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{AppState, db};

#[derive(Deserialize)]
pub struct CreateDocumentPayload {
    title: String,
    content: String,
}

#[derive(Serialize)]
pub struct DocumentResponse {
    id: Uuid,
    title: String,
    content: String,
}

pub async fn health_check_handler() -> &'static str {
    "OK"
}

pub async fn create_document_handler(
    State(state): State<AppState>,
    Json(payload): Json<CreateDocumentPayload>,
) -> impl IntoResponse {
    match db::create_document(&state.pool, &payload.title, &payload.content).await {
        Ok(doc) => (
            StatusCode::CREATED,
            Json(DocumentResponse {
                id: doc.id,
                title: doc.title,
                content: doc.content,
            }),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to create document: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to create document",
            )
                .into_response()
        }
    }
}

pub async fn get_document_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> impl IntoResponse {
    match db::get_document(&state.pool, id).await {
        Ok(Some(doc)) => (
            StatusCode::OK,
            Json(DocumentResponse {
                id: doc.id,
                title: doc.title,
                content: doc.content,
            }),
        )
            .into_response(),
        Ok(None) => (StatusCode::NOT_FOUND, "Document not found").into_response(),
        Err(e) => {
            tracing::error!("Failed to get document: {:?}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to get document").into_response()
        }
    }
}
