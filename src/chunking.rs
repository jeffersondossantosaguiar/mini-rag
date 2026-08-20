use crate::embedding::{Embedder, MAX_CHUNK_TOKENS};

/// Quebra um texto em chunks, tentando preservar parágrafos inteiros.
/// Se um parágrafo sozinho passar do limite, quebra por sentença.
pub fn chunk_text(text: &str, embedder: &Embedder) -> Vec<String> {
    let paragraphs: Vec<&str> = text
        .split("\n\n")
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect();

    let mut chunks = Vec::new();

    for paragraph in paragraphs {
        if embedder.count_tokens(paragraph) <= MAX_CHUNK_TOKENS {
            chunks.push(paragraph.to_string());
        } else {
            let sentence_chunks = split_by_sentence(paragraph, embedder);
            chunks.extend(sentence_chunks);
        }
    }

    chunks
}

/// Quebra um parágrafo grande por sentenças, agrupando sentenças
/// até chegar perto do limite de MAX_CHUNK_TOKENS.
fn split_by_sentence(paragraph: &str, embedder: &Embedder) -> Vec<String> {
    let sentences: Vec<&str> = paragraph
        .split_inclusive([',', '!', '?'])
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    let mut chunks = Vec::new();
    let mut current = String::new();

    for sentence in sentences {
        let candidate = if current.is_empty() {
            sentence.to_string()
        } else {
            format!("{}{}", current, sentence) // current already ends with space
        };
        if embedder.count_tokens(&candidate) > MAX_CHUNK_TOKENS && !current.is_empty() {
            chunks.push(normalize_whitespace(&current));
            current = String::new();
        }
        current.push_str(sentence);
        current.push(' ');
    }

    if !current.trim().is_empty() {
        chunks.push(normalize_whitespace(&current));
    }

    chunks
}

fn normalize_whitespace(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_simple_paragraphs() {
        let text = "This is a short paragraph.\n\nThis is another short paragraph.";
        let embedder = Embedder::new().unwrap();
        let chunks = chunk_text(text, &embedder);
        assert_eq!(chunks.len(), 2);
    }
}
