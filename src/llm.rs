use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::from_str;

#[derive(Clone)]
pub struct LlmClient {
    client: Client,
    base_url: String,
    model: String,
}

#[derive(Serialize)]
pub struct ChatMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct ChatRequest {
    model: String,
    messages: Vec<ChatMessage>,
    stream: bool,
    format: &'static str,
}

#[derive(Deserialize)]
pub struct ResponseMessage {
    content: String,
}

#[derive(Deserialize)]
pub struct ChatResponse {
    message: ResponseMessage,
}

#[derive(Deserialize)]
pub struct Answer {
    pub answer: String,
    pub sources: Vec<String>,
}

impl LlmClient {
    const SYSTEM_PROMPT: &'static str = "Você é um assistente de RAG. Responda APENAS com base no contexto fornecido.\n\
        Não invente informações. Se o contexto não contiver a resposta, diga que não sabe.\n\
        Cite as fontes que usar no formato \"Fonte N\".\n\
        Responda em português, em 1 a 3 sentenças.\n\
        Responda SEMPRE em JSON válido com o formato: {\"answer\": \"...\", \"sources\": [\"Fonte 1\", ...]}";
    const MAX_PROMPT_TOKENS: usize = 2048;

    pub fn new(base_url: &str, model: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.to_string(),
            model: model.to_string(),
        }
    }

    pub async fn chat(&self, messages: Vec<ChatMessage>) -> Result<Answer, anyhow::Error> {
        let request = ChatRequest {
            model: self.model.clone(),
            messages,
            stream: false,
            format: "json",
        };

        let response = self
            .client
            .post(format!("{}/api/chat", self.base_url))
            .json(&request)
            .send()
            .await?;

        let body: ChatResponse = response.json().await?;

        let answer = from_str(&body.message.content).map_err(|e| anyhow::anyhow!(e))?;

        Ok(answer)
    }

    pub fn build_rag_prompt(chunks: &[String], question: &str) -> Vec<ChatMessage> {
        let mut messages = Vec::new();

        messages.push(ChatMessage {
            role: "system".to_string(),
            content: Self::SYSTEM_PROMPT.to_string(),
        });

        let mut context = String::new();
        let mut used_tokens = Self::SYSTEM_PROMPT.split_whitespace().count();
        used_tokens += question.split_whitespace().count();

        for (i, chunk) in chunks.iter().enumerate() {
            let chunk_tokens = chunk.split_whitespace().count();
            if used_tokens + chunk_tokens > Self::MAX_PROMPT_TOKENS {
                break;
            }
            context.push_str(&format!("[Fonte {}] {}\n\n", i + 1, chunk));
            used_tokens += chunk_tokens;
        }

        messages.push(ChatMessage {
            role: "user".to_string(),
            content: format!("Contexto:\n{}\n\nPergunta: {}", context, question),
        });

        messages
    }
}
