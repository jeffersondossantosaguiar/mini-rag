use reqwest::Client;
use serde::{Deserialize, Serialize};

#[derive(Clone)]
pub struct LlmClient {
    client: Client,
    base_url: String,
    model: String,
}

#[derive(Serialize)]
struct ChatMessage {
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
struct ResponseMessage {
    content: String,
}

#[derive(Deserialize)]
struct ChatResponse {
    message: ResponseMessage,
}

impl LlmClient {
    const SYSTEM_PROMPT: &'static str = "Responda apenas com o contexto que sera passado, nao invente respostas. Se nao souber a resposta, diga que nao sabe. Cite as fontes que usar na resposta, ex: Fonte N; Responda em português;responda em 1 a 3 sentenças";
    const MAX_PROMPT_TOKENS: usize = 2048;

    pub fn new(base_url: &str, model: &str) -> Self {
        Self {
            client: Client::new(),
            base_url: base_url.to_string(),
            model: model.to_string(),
        }
    }

    pub async fn chat(&self, messages: Vec<ChatMessage>) -> Result<String, reqwest::Error> {
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
        Ok(body.message.content)
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
