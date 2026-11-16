use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::sync::RwLock;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SemanticMatch {
    pub symbol_path: String,
    pub signature: String,
    pub score: f32,
}

#[derive(Debug, Clone)]
struct IndexedSymbol {
    symbol_path: String,
    signature: String,
}

#[derive(Debug, Clone, Copy)]
struct Posting {
    doc_id: u32,
    weight: f32,
}

/// Lightweight HTTP client using std::net::TcpStream with 500ms timeout
fn post_sidecar_json(path: &str, body: &str) -> Option<String> {
    let mut stream = TcpStream::connect_timeout(
        &"127.0.0.1:9819".parse().ok()?,
        Duration::from_millis(300),
    ).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(3))).ok()?;
    let req = format!(
        "POST {} HTTP/1.1\r\nHost: 127.0.0.1:9819\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
        path, body.len(), body
    );
    stream.write_all(req.as_bytes()).ok()?;
    let mut resp = String::new();
    stream.read_to_string(&mut resp).ok()?;
    if let Some(pos) = resp.find("\r\n\r\n") {
        Some(resp[pos + 4..].to_string())
    } else {
        None
    }
}

pub struct LocalSemanticIndex {
    symbols: RwLock<Vec<IndexedSymbol>>,
    symbol_to_id: RwLock<HashMap<String, u32>>,
    inverted_index: RwLock<HashMap<u32, Vec<Posting>>>,
    total_docs: RwLock<usize>,
}

impl Default for LocalSemanticIndex {
    fn default() -> Self {
        Self::new()
    }
}

impl LocalSemanticIndex {
    pub fn new() -> Self {
        Self {
            symbols: RwLock::new(Vec::new()),
            symbol_to_id: RwLock::new(HashMap::new()),
            inverted_index: RwLock::new(HashMap::new()),
            total_docs: RwLock::new(0),
        }
    }

    /// Hashes subwords and character 3-grams into feature IDs with camelCase and snake_case awareness
    pub fn vectorize(text: &str) -> HashMap<u32, f32> {
        let mut tf = HashMap::new();

        // 1. Split on camelCase boundaries and non-alphanumeric chars
        let mut tokens = Vec::new();
        let mut current_token = String::new();
        let chars: Vec<char> = text.chars().collect();

        for i in 0..chars.len() {
            let c = chars[i];
            if c.is_alphanumeric() {
                if c.is_uppercase()
                    && !current_token.is_empty()
                    && (i + 1 < chars.len() && chars[i + 1].is_lowercase())
                {
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
    #[allow(dead_code)]
    pub fn index_symbol(&self, symbol_path: &str, signature: &str, doc_text: &str) {
        self.index_symbols_batch(vec![(
            symbol_path.to_string(),
            signature.to_string(),
            doc_text.to_string(),
        )]);
    }

    /// Parallel batch indexing of symbols using all available CPU threads
    pub fn index_symbols_batch(&self, items: Vec<(String, String, String)>) {
        if items.is_empty() {
            return;
        }

        let vectorized: Vec<(String, String, HashMap<u32, f32>)> = items
            .into_par_iter()
            .map(|(symbol_path, signature, doc_text)| {
                let combined = format!("{} {} {}", symbol_path, signature, doc_text);
                let vec = Self::vectorize(&combined);
                (symbol_path, signature, vec)
            })
            .collect();

        let mut symbols = self.symbols.write().unwrap();
        let mut symbol_to_id = self.symbol_to_id.write().unwrap();
        let mut inv = self.inverted_index.write().unwrap();

        for (path, sig, vec) in vectorized {
            let doc_id = match symbol_to_id.get(&path) {
                Some(&existing_id) => {
                    symbols[existing_id as usize] = IndexedSymbol {
                        symbol_path: path.clone(),
                        signature: sig,
                    };
                    existing_id
                }
                None => {
                    let new_id = symbols.len() as u32;
                    symbols.push(IndexedSymbol {
                        symbol_path: path.clone(),
                        signature: sig,
                    });
                    symbol_to_id.insert(path, new_id);
                    new_id
                }
            };

            for (feature_id, weight) in vec {
                inv.entry(feature_id).or_default().push(Posting {
                    doc_id,
                    weight,
                });
            }
        }

        *self.total_docs.write().unwrap() = symbols.len();
    }

    /// Search using resident GPU neural embedding sidecar (if running), falling back to in-process inverted index
    pub fn search(&self, query: &str, top_k: usize) -> Vec<SemanticMatch> {
        // 1. Try GPU sidecar first
        let sidecar_req = json!({
            "query": query,
            "top_k": top_k
        }).to_string();

        if let Some(resp_body) = post_sidecar_json("/search", &sidecar_req) {
            #[derive(Deserialize)]
            struct SidecarResultItem {
                id: String,
                score: f32,
            }
            #[derive(Deserialize)]
            struct SidecarResponse {
                results: Vec<SidecarResultItem>,
            }
            if let Ok(parsed) = serde_json::from_str::<SidecarResponse>(&resp_body) {
                if !parsed.results.is_empty() {
                    let symbols = self.symbols.read().unwrap();
                    let id_map = self.symbol_to_id.read().unwrap();
                    let mut matches = Vec::new();
                    for item in parsed.results {
                        let sig = if let Some(&doc_id) = id_map.get(&item.id) {
                            symbols.get(doc_id as usize).map(|s| s.signature.clone()).unwrap_or_default()
                        } else {
                            String::new()
                        };
                        matches.push(SemanticMatch {
                            symbol_path: item.id,
                            signature: sig,
                            score: item.score,
                        });
                    }
                    if !matches.is_empty() {
                        return matches;
                    }
                }
            }
        }

        // 2. Fall back to in-process inverted index
        let query_vec = Self::vectorize(query);
        if query_vec.is_empty() {
            return Vec::new();
        }

        let symbols = self.symbols.read().unwrap();
        let inv = self.inverted_index.read().unwrap();
        if symbols.is_empty() {
            return Vec::new();
        }

        let mut scores: HashMap<u32, f32> = HashMap::with_capacity(1024);
        for (&feature_id, &q_weight) in &query_vec {
            if let Some(postings) = inv.get(&feature_id) {
                for posting in postings {
                    *scores.entry(posting.doc_id).or_insert(0.0) += q_weight * posting.weight;
                }
            }
        }

        if scores.is_empty() {
            return Vec::new();
        }

        let mut scored: Vec<SemanticMatch> = scores
            .into_iter()
            .filter(|&(_, score)| score > 0.08)
            .filter_map(|(doc_id, score)| {
                let sym = symbols.get(doc_id as usize)?;
                Some(SemanticMatch {
                    symbol_path: sym.symbol_path.clone(),
                    signature: sym.signature.clone(),
                    score,
                })
            })
            .collect();

        scored.sort_by(|a, b| b.score.partial_cmp(&a.score).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(top_k);
        scored
    }

    pub fn total_indexed(&self) -> usize {
        *self.total_docs.read().unwrap()
    }

    #[allow(dead_code)]
    pub fn clear(&self) {
        self.symbols.write().unwrap().clear();
        self.symbol_to_id.write().unwrap().clear();
        self.inverted_index.write().unwrap().clear();
        *self.total_docs.write().unwrap() = 0;
    }
}
