use sqlx::{Error, PgPool, prelude::FromRow, query_as, types::Uuid};

#[derive(Debug, FromRow)]
pub struct Document {
    pub id: Uuid,
    pub title: String,
    pub content: String,
}

pub async fn create_document(pool: &PgPool, title: &str, content: &str) -> Result<Document, Error> {
    query_as!(
        Document,
        r#"
    INSERT INTO documents (title, content)
      VALUES ($1, $2)
      RETURNING id, title, content
    "#,
        title,
        content
    )
    .fetch_one(pool)
    .await
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
