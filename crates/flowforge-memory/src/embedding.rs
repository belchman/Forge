use xxhash_rust::xxh3::xxh3_64;

/// Hash-based deterministic embeddings using character n-gram feature hashing.
/// Not semantic, but effective for pattern similarity matching.
pub struct Embedding {
    dim: usize,
}

impl Embedding {
    pub fn new(dim: usize) -> Self {
        Self { dim }
    }

    /// Generate an embedding vector from text using character n-grams hashed with xxh3.
    pub fn embed(&self, text: &str) -> Vec<f32> {
        let mut vector = vec![0.0f32; self.dim];
        let text_lower = text.to_lowercase();
        let chars: Vec<char> = text_lower.chars().collect();

        if chars.is_empty() {
            return vector;
        }

        // Unigrams
        for &ch in &chars {
            let hash = xxh3_64(ch.to_string().as_bytes());
            let idx = (hash as usize) % self.dim;
            // Use hash bit to determine sign (+1 or -1)
            let sign = if (hash >> 32) & 1 == 0 { 1.0 } else { -1.0 };
            vector[idx] += sign;
        }

        // Bigrams
        for pair in chars.windows(2) {
            let bigram = format!("{}{}", pair[0], pair[1]);
            let hash = xxh3_64(bigram.as_bytes());
            let idx = (hash as usize) % self.dim;
            let sign = if (hash >> 32) & 1 == 0 { 1.0 } else { -1.0 };
            vector[idx] += sign * 1.5; // Bigrams weighted higher
        }

        // L2 normalize
        l2_normalize(&mut vector);

        vector
    }

    /// Compute cosine similarity between two vectors.
    pub fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
        if a.len() != b.len() || a.is_empty() {
            return 0.0;
        }

        let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
        let norm_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
        let norm_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

        if norm_a == 0.0 || norm_b == 0.0 {
            0.0
        } else {
            dot / (norm_a * norm_b)
        }
    }
}

impl Default for Embedding {
    fn default() -> Self {
        Self::new(128)
    }
}

fn l2_normalize(vector: &mut [f32]) {
    let norm: f32 = vector.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for v in vector.iter_mut() {
            *v /= norm;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embed_produces_correct_dim() {
        let emb = Embedding::new(128);
        let vec = emb.embed("hello world");
        assert_eq!(vec.len(), 128);
    }

    #[test]
    fn test_embed_is_normalized() {
        let emb = Embedding::new(128);
        let vec = emb.embed("test input text");
        let norm: f32 = vec.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_embed_is_deterministic() {
        let emb = Embedding::new(128);
        let v1 = emb.embed("same text");
        let v2 = emb.embed("same text");
        assert_eq!(v1, v2);
    }

    #[test]
    fn test_cosine_similarity_identical() {
        let emb = Embedding::new(128);
        let v = emb.embed("hello");
        let sim = Embedding::cosine_similarity(&v, &v);
        assert!((sim - 1.0).abs() < 0.01);
    }

    #[test]
    fn test_cosine_similarity_different() {
        let emb = Embedding::new(128);
        let v1 = emb.embed("rust programming language");
        let v2 = emb.embed("python programming language");
        let v3 = emb.embed("cooking recipes for dinner");
        let sim_related = Embedding::cosine_similarity(&v1, &v2);
        let sim_unrelated = Embedding::cosine_similarity(&v1, &v3);
        // Related texts should be more similar than unrelated
        assert!(sim_related > sim_unrelated);
    }

    #[test]
    fn test_embed_empty_string() {
        let emb = Embedding::new(128);
        let vec = emb.embed("");
        assert!(vec.iter().all(|&v| v == 0.0));
    }
}
