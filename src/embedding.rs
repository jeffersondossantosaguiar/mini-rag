use sqlx::Row;
use std::sync::{Arc, Mutex};

use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
use tokenizers::Tokenizer;
use tokio::task::spawn_blocking;

pub const MAX_CHUNK_TOKENS: usize = 256; // Real sequence limit of all-MiniLM-L6V2

#[derive(Clone)]
pub struct Embedder {
    model: Arc<Mutex<TextEmbedding>>,
    tokenizer: Arc<Tokenizer>,
}

impl Embedder {
    pub fn new() -> Result<Self, anyhow::Error> {
        let model = TextEmbedding::try_new(InitOptions::new(EmbeddingModel::AllMiniLML6V2))?;

        let tokenizer = Tokenizer::from_file("assets/tokenizer.json")
            .map_err(|e| anyhow::anyhow!("failed to load tokenizer: {}", e))?;

        Ok(Self {
            model: Arc::new(Mutex::new(model)),
            tokenizer: Arc::new(tokenizer),
        })
    }

    pub async fn embed(&self, text: String) -> Result<Vec<f32>, anyhow::Error> {
        let model = self.model.clone();

        let embedding = spawn_blocking(move || {
            let mut model = model.lock().unwrap();
            model.embed(vec![text], None)
        })
        .await??;

        Ok(embedding.into_iter().next().unwrap())
    }

    pub fn count_tokens(&self, text: &str) -> usize {
        self.tokenizer
            .encode(text, false)
            .map(|encoding| encoding.len())
            .unwrap_or(0)
    }
}
