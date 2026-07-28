use pgvector::Vector;
use sqlx::{Error, PgPool, prelude::FromRow, query_as, types::Uuid};

#[derive(Debug, FromRow)]
pub struct Document {
    pub id: Uuid,
    pub title: String,
    pub content: String,
}

#[derive(Debug, FromRow)]
pub struct Chunk {
    pub id: Uuid,
    pub document_id: Uuid,
    pub content: String,
    pub chunk_index: i32,
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

pub async fn create_chunks(
    pool: &PgPool,
    document_id: Uuid,
    contents: &[String],
) -> Result<Vec<Chunk>, Error> {
    let mut chunks = Vec::with_capacity(contents.len());

    //TODO Loop com INSERT individual, ao invés de batch insert — isso é uma escolha consciente de simplicidade pra Fase 2. Um INSERT em batch (múltiplas linhas numa query só) seria mais performático, mas o SQLx não tem uma forma tão direta de fazer isso com query_as! compile-time-checked pra arrays dinâmicos — normalmente isso pede UNNEST no SQL ou uma lib auxiliar. Fica registrado como possível melhoria de performance pra Fase 6 (Robustez), quando formos pensar em otimizações.
    for (index, content) in contents.iter().enumerate() {
        let chunk = query_as!(
            Chunk,
            r#"
            INSERT INTO chunks (document_id, content, chunk_index)
            VALUES ($1, $2, $3)
            RETURNING id, document_id, content, chunk_index
            "#,
            document_id,
            content,
            index as i32
        )
        .fetch_one(pool)
        .await?;

        chunks.push(chunk);
    }

    Ok(chunks)
}
