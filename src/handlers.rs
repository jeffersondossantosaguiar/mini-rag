use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{AppState, chunking, db};

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

pub async fn ingest_document_handler(
    State(state): State<AppState>,
    Json(payload): Json<CreateDocumentPayload>,
) -> impl IntoResponse {
    let chunk_contents = chunking::chunk_text(&payload.content, &state.embedder);

    if chunk_contents.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            "Documentm content produced no chunks",
        )
            .into_response();
    }

    let mut chunks_with_embeddings = Vec::with_capacity(chunk_contents.len());

    for content in chunk_contents {
        match state.embedder.embed(content.clone()).await {
            Ok(embedding) => chunks_with_embeddings.push((content, embedding)),
            Err(e) => {
                tracing::error!("Failed to generate embedding for chunk: {:?}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "Failed to generate embedding for chunk",
                )
                    .into_response();
            }
        }
    }

    match db::create_document_with_chunks(
        &state.pool,
        &payload.title,
        &payload.content,
        &chunks_with_embeddings,
    )
    .await
    {
        Ok((document, chunks)) => (
            StatusCode::CREATED,
            Json(serde_json::json!({
                "id": document.id,
                "title": document.title,
                "chunks_created": chunks.len(),
            })),
        )
            .into_response(),
        Err(e) => {
            tracing::error!("Failed to ingest document: {:?}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Failed to ingest document",
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
