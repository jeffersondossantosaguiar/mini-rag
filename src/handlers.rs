use axum::{
    Json,
    extract::{Path, State},
};
use pgvector::Vector;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{AppState, LlmClient, chunking, db, error::AppError};

#[derive(Deserialize)]
pub struct CreateDocumentPayload {
    title: String,
    content: String,
}

impl CreateDocumentPayload {
    pub fn validate(&self) -> Result<(), AppError> {
        if self.title.trim().is_empty() {
            return Err(AppError::Validation("title must not be empty".into()));
        }
        if self.title.len() > 200 {
            return Err(AppError::Validation("title must be 200 chars or less".into()));
        }
        if self.content.trim().is_empty() {
            return Err(AppError::Validation("content must not be empty".into()));
        }
        if self.content.len() > 100_000 {
            return Err(AppError::Validation("content must be 100,000 chars or less".into()));
        }
        Ok(())
    }
}

#[derive(Serialize)]
pub struct DocumentResponse {
    id: Uuid,
    title: String,
    content: String,
}

#[derive(Deserialize)]
pub struct QueryRequest {
    query: String,
    #[serde(default = "default_top_k")]
    top_k: i32,
}

impl QueryRequest {
    pub fn validate(&self) -> Result<(), AppError> {
        if self.query.trim().is_empty() {
            return Err(AppError::Validation("query must not be empty".into()));
        }
        if self.query.len() > 1000 {
            return Err(AppError::Validation("query must be 1000 chars or less".into()));
        }
        if !(1..=20).contains(&self.top_k) {
            return Err(AppError::Validation("top_k must be between 1 and 20".into()));
        }
        Ok(())
    }
}

fn default_top_k() -> i32 {
    5
}

#[derive(Serialize)]
pub struct QueryResponse {
    answer: String,
    sources: Vec<SourceResult>,
}

#[derive(Serialize)]
pub struct SourceResult {
    content: String,
    document_id: Uuid,
    similarity: f32,
}

pub async fn health_check_handler() -> &'static str {
    "OK"
}

pub async fn ingest_document_handler(
    State(state): State<AppState>,
    Json(payload): Json<CreateDocumentPayload>,
) -> Result<Json<serde_json::Value>, AppError> {
    payload.validate()?;

    let chunk_contents = chunking::chunk_text(&payload.content, &state.embedder);

    if chunk_contents.is_empty() {
        return Err(AppError::Validation(
            "Document content produced no chunks".into(),
        ));
    }

    let mut chunks_with_embeddings = Vec::with_capacity(chunk_contents.len());

    for content in chunk_contents {
        let embedding = state
            .embedder
            .embed(content.clone())
            .await
            .map_err(|e| AppError::Embedding(format!("{:?}", e)))?;
        chunks_with_embeddings.push((content, embedding));
    }

    let (document, chunks) = db::create_document_with_chunks(
        &state.pool,
        &payload.title,
        &payload.content,
        &chunks_with_embeddings,
    )
    .await?;

    Ok(Json(serde_json::json!({
        "id": document.id,
        "title": document.title,
        "chunks_created": chunks.len(),
    })))
}

pub async fn get_document_handler(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> Result<Json<DocumentResponse>, AppError> {
    let doc = db::get_document(&state.pool, id)
        .await?
        .ok_or(AppError::NotFound)?;

    Ok(Json(DocumentResponse {
        id: doc.id,
        title: doc.title,
        content: doc.content,
    }))
}

pub async fn query_handler(
    State(state): State<AppState>,
    Json(payload): Json<QueryRequest>,
) -> Result<Json<QueryResponse>, AppError> {
    payload.validate()?;

    let query_embedding = state
        .embedder
        .embed(payload.query.clone())
        .await
        .map_err(|e| AppError::Embedding(format!("{:?}", e)))?;

    let results = db::search_similar_chunks(
        &state.pool,
        Vector::from(query_embedding),
        payload.top_k as i64,
    )
    .await?;

    if results.is_empty() {
        return Err(AppError::NotFound);
    }

    let chunk_contents: Vec<String> = results.iter().map(|row| row.content.clone()).collect();

    let messages = LlmClient::build_rag_prompt(&chunk_contents, &payload.query);

    let answer = state
        .llm
        .chat(messages)
        .await
        .map_err(|e| AppError::Llm(format!("{:?}", e)))?;

    let sources: Vec<SourceResult> = answer
        .sources
        .iter()
        .filter_map(|marker| {
            let idx: usize = marker.trim_start_matches("Fonte ").parse().ok()?;
            let i = idx.checked_sub(1)?;
            let row = results.get(i)?;
            Some(SourceResult {
                content: row.content.clone(),
                document_id: row.document_id,
                similarity: (1.0 - row.distance) as f32,
            })
        })
        .collect();

    Ok(Json(QueryResponse {
        answer: answer.answer,
        sources,
    }))
}
