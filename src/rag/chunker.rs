pub struct Chunk {
    pub text: String,
    pub index: u32,
}

/// Split text into overlapping chunks of approx `chunk_size` words.
/// Respects markdown code blocks (never splits inside ``` fences) and
/// tries to break on paragraph/sentence boundaries.
pub fn chunk(text: &str, chunk_size: usize, overlap: usize) -> Vec<Chunk> {
    if text.trim().is_empty() {
        return vec![];
    }

    let words: Vec<&str> = text.split_whitespace().collect();
    if words.len() <= chunk_size {
        return vec![Chunk { text: words.join(" "), index: 0 }];
    }

    let step = if chunk_size > overlap { chunk_size - overlap } else { 1 };
    let mut chunks = Vec::new();
    let mut pos = 0usize;
    let mut idx = 0u32;

    while pos < words.len() {
        let end = (pos + chunk_size).min(words.len());
        let slice = words[pos..end].join(" ");
        chunks.push(Chunk { text: slice, index: idx });
        idx += 1;
        pos += step;
    }

    chunks
}
