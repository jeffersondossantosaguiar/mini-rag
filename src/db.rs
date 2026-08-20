use pgvector::Vector;
use sqlx::{Error, PgPool, prelude::FromRow, query_as, types::Uuid};

#[derive(Debug, FromRow)]
pub struct Document {
    pub id: Uuid,
    pub title: String,
    pub content: String,
}

#[derive(Debug, FromRow)]
#[allow(dead_code)]
pub struct Chunk {
    pub id: Uuid,
    pub document_id: Uuid,
    pub content: String,
    pub chunk_index: i32,
}

pub struct ChunkSearchResult {
    pub document_id: Uuid,
    pub content: String,
    pub distance: f64,
}

pub async fn create_document_with_chunks(
    pool: &PgPool,
    title: &str,
    content: &str,
    chunks_with_embeddings: &[(String, Vec<f32>)],
) -> Result<(Document, Vec<Chunk>), Error> {
    let mut tx = pool.begin().await?;

    let document = query_as!(
        Document,
        r#"
    INSERT INTO documents (title, content)
      VALUES ($1, $2)
      RETURNING id, title, content
    "#,
        title,
        content
    )
    .fetch_one(&mut *tx)
    .await?;

    let mut chunks = Vec::with_capacity(chunks_with_embeddings.len());

    for (index, (chunk_content, embedding)) in chunks_with_embeddings.iter().enumerate() {
        let vector = Vector::from(embedding.clone());

        let chunk = query_as!(
            Chunk,
            r#"
            INSERT INTO chunks (document_id, content, chunk_index, embedding)
            VALUES ($1, $2, $3, $4)
            RETURNING id, document_id, content, chunk_index
            "#,
            document.id,
            chunk_content,
            index as i32,
            vector as Vector
        )
        .fetch_one(&mut *tx)
        .await?;

        chunks.push(chunk);
    }

    tx.commit().await?;

    Ok((document, chunks))
}

pub async fn get_document(pool: &PgPool, id: Uuid) -> Result<Option<Document>, Error> {
    query_as!(
        Document,
        r#" SELECT id, title, content FROM documents WHERE id = $1 "#,
        id
    )
    .fetch_optional(pool)
    .await
}

pub async fn search_similar_chunks(
    pool: &PgPool,
    query_embedding: Vector,
    top_k: i64,
) -> Result<Vec<ChunkSearchResult>, Error> {
    query_as!(
        ChunkSearchResult,
        r#"
        SELECT
            chunks.document_id,
            chunks.content,
            chunks.embedding <=> $1 as "distance!: f64"
        FROM chunks
        ORDER BY chunks.embedding <=> $1
        LIMIT $2
        "#,
        query_embedding as _,
        top_k
    )
    .fetch_all(pool)
    .await
}
