const MAX_CHUNK_SIZE: usize = 1000;

/// Quebra um texto em chunks, tentando preservar parágrafos inteiros.
/// Se um parágrafo sozinho passar do limite, quebra por sentença.
pub fn chunk_text(text: &str) -> Vec<String> {
    let paragraphs: Vec<&str> = text
        .split("\n\n")
        .map(|p| p.trim())
        .filter(|p| !p.is_empty())
        .collect();

    let mut chunks = Vec::new();

    for paragraph in paragraphs {
        if paragraph.len() <= MAX_CHUNK_SIZE {
            chunks.push(paragraph.to_string());
        } else {
            let sentence_chunks = split_by_sentence(paragraph);
            chunks.extend(sentence_chunks);
        }
    }

    chunks
}

/// Quebra um parágrafo grande por sentenças, agrupando sentenças
/// até chegar perto do limite de MAX_CHUNK_SIZE.
fn split_by_sentence(paragraph: &str) -> Vec<String> {
    let sentences: Vec<&str> = paragraph
        .split_inclusive([',', '!', '?'])
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    let mut chunks = Vec::new();
    let mut current = String::new();

    for sentence in sentences {
        if current.len() + sentence.len() > MAX_CHUNK_SIZE && !current.is_empty() {
            chunks.push(current.trim().to_string());
            current = String::new();
        }
        current.push_str(sentence);
        current.push(' '); // Adiciona um espaço entre sentenças
    }

    if !current.trim().is_empty() {
        chunks.push(current.trim().to_string());
    }

    chunks
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_chunk_simple_paragraphs() {
        let text = "This is a short paragraph.\n\nThis is another short paragraph.";
        let chunks = chunk_text(text);
        assert_eq!(chunks.len(), 2);
    }
}
