use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::RwLock;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticMatch {
    pub symbol_path: String,
    pub signature: String,
    pub score: f32,
}

pub struct LocalSemanticIndex {
    doc_vectors: RwLock<HashMap<String, (String, HashMap<u32, f32>)>>,
    total_docs: RwLock<usize>,
}

impl LocalSemanticIndex {
    pub fn new() -> Self {
        Self {
            doc_vectors: RwLock::new(HashMap::new()),
            total_docs: RwLock::new(0),
        }
    }

    /// Hashes subwords and character 3-grams into feature IDs with camelCase and snake_case awareness
    fn vectorize(text: &str) -> HashMap<u32, f32> {
        let mut tf = HashMap::new();

        // 1. Split on camelCase boundaries and non-alphanumeric chars
        let mut tokens = Vec::new();
        let mut current_token = String::new();
        let chars: Vec<char> = text.chars().collect();

        for i in 0..chars.len() {
            let c = chars[i];
            if c.is_alphanumeric() {
                if c.is_uppercase() && !current_token.is_empty() && (i + 1 < chars.len() && chars[i + 1].is_lowercase()) {
                    tokens.push(current_token.to_lowercase());
                    current_token.clear();
                }
                current_token.push(c);
            } else {
                if !current_token.is_empty() {
                    tokens.push(current_token.to_lowercase());
                    current_token.clear();
                }
            }
        }
        if !current_token.is_empty() {
            tokens.push(current_token.to_lowercase());
        }

        // 2. Also add full identifier words from lowercased text
        let lower = text.to_lowercase();
        let raw_words: Vec<&str> = lower
            .split(|c: char| !c.is_alphanumeric() && c != '_')
            .filter(|w| !w.is_empty())
            .collect();

        for w in &raw_words {
            tokens.push(w.to_string());
        }

        for w in &tokens {
            if w.is_empty() {
                continue;
            }
            // Word hash weight
            let h = Self::hash_term(w);
            *tf.entry(h).or_insert(0.0) += 2.0;

            // Character trigrams for subword semantic robustness
            if w.len() >= 3 {
                for window in w.as_bytes().windows(3) {
                    let tri_hash = Self::hash_ngram(window);
                    *tf.entry(tri_hash).or_insert(0.0) += 1.0;
                }
            }
        }

        // L2 normalize
        let norm: f32 = tf.values().map(|v| v * v).sum::<f32>().sqrt();
        if norm > 0.0 {
            for v in tf.values_mut() {
                *v /= norm;
            }
        }

        tf
    }

    fn hash_term(term: &str) -> u32 {
        let mut h = 0x811c9dc5u32;
        for &b in term.as_bytes() {
            h ^= b as u32;
            h = h.wrapping_mul(0x01000193);
        }
        h
    }

    fn hash_ngram(bytes: &[u8]) -> u32 {
        let mut h = 0x811c9dc5u32;
        for &b in bytes {
            h ^= b as u32;
            h = h.wrapping_mul(0x01000193);
        }
        h
    }

    /// Adds a symbol to the local in-process index
    pub fn index_symbol(&self, symbol_path: &str, signature: &str, doc_text: &str) {
        let combined = format!("{} {} {}", symbol_path, signature, doc_text);
        let vec = Self::vectorize(&combined);

        let mut docs = self.doc_vectors.write().unwrap();
        docs.insert(symbol_path.to_string(), (signature.to_string(), vec));
        *self.total_docs.write().unwrap() = docs.len();
    }

    /// In-process cosine similarity search (<0.5 ms)
    pub fn search(&self, query: &str, top_k: usize) -> Vec<SemanticMatch> {
        let query_vec = Self::vectorize(query);
        if query_vec.is_empty() {
            return Vec::new();
        }

        let docs = self.doc_vectors.read().unwrap();
        let mut scored = Vec::with_capacity(docs.len());

        for (path, (sig, doc_vec)) in docs.iter() {
            let mut dot_product = 0.0f32;
            for (dim, q_val) in &query_vec {
                if let Some(d_val) = doc_vec.get(dim) {
                    dot_product += q_val * d_val;
                }
            }

            if dot_product > 0.03 {
                scored.push(SemanticMatch {
                    symbol_path: path.clone(),
                    signature: sig.clone(),
                    score: dot_product,
                });
            }
        }

        scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(top_k);
        scored
    }

    pub fn total_indexed(&self) -> usize {
        *self.total_docs.read().unwrap()
    }
}
