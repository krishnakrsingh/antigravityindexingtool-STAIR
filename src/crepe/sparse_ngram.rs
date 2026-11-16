use super::frequency::get_pair_weight;
use std::collections::HashSet;

pub const MIN_NGRAM_LEN: usize = 3;
pub const MAX_NGRAM_LEN: usize = 32;

/// 64-bit FNV-1a hash for n-gram byte slices
#[inline]
pub fn hash_ngram(bytes: &[u8]) -> u64 {
    let mut h = 0xcbf29ce484222325u64;
    for &b in bytes {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

/// Sparse N-gram extractor implementing Cursor's crepe algorithm:
/// Extracts substrings where min(endpoint_weights) > max(interior_weights)
pub struct SparseNgramExtractor;

impl SparseNgramExtractor {
    /// Extracts all sparse n-grams from a document during index construction
    pub fn extract_all(text: &[u8]) -> Vec<u64> {
        let n = text.len();
        if n < MIN_NGRAM_LEN {
            return Vec::new();
        }

        // Precompute bigram weights
        let num_bigrams = n - 1;
        let mut weights = Vec::with_capacity(num_bigrams);
        for i in 0..num_bigrams {
            weights.push(get_pair_weight(text[i], text[i + 1]));
        }

        let mut ngrams = HashSet::new();

        // For small trigrams with high-weight endpoints
        for i in 0..num_bigrams - 1 {
            let w1 = weights[i];
            let w2 = weights[i + 1];
            if w1 >= 45000 || w2 >= 45000 {
                let slice = &text[i..i + 3];
                ngrams.insert(hash_ngram(slice));
            }
        }

        // Variable-length sparse n-grams with endpoint dominance:
        // len is character length: 4..=MAX_NGRAM_LEN
        // bigram count is len - 1
        for len in 4..=MAX_NGRAM_LEN.min(n) {
            let bigrams_in_window = len - 1;
            for i in 0..=num_bigrams - bigrams_in_window {
                let left_w = weights[i];
                let right_w = weights[i + bigrams_in_window - 1];
                let min_endpoints = left_w.min(right_w);

                // Interior bigrams: i + 1 .. i + bigrams_in_window - 1
                let mut interior_max = 0u16;
                for j in (i + 1)..(i + bigrams_in_window - 1) {
                    if weights[j] > interior_max {
                        interior_max = weights[j];
                    }
                }

                if min_endpoints > interior_max {
                    let slice = &text[i..i + len];
                    ngrams.insert(hash_ngram(slice));
                }
            }
        }

        ngrams.into_iter().collect()
    }

    /// Extracts sparse n-grams from a query for candidate document filtering
    pub fn extract_query_ngrams(query: &str) -> Vec<u64> {
        let bytes = query.as_bytes();
        let mut ngrams = HashSet::new();

        // If query itself is small (e.g. 3..=32 chars), add full query hash
        if bytes.len() >= MIN_NGRAM_LEN && bytes.len() <= MAX_NGRAM_LEN {
            ngrams.insert(hash_ngram(bytes));
        }

        // Also extract sub-ngrams
        let extracted = Self::extract_all(bytes);
        for h in extracted {
            ngrams.insert(h);
        }

        // If query is longer than MAX_NGRAM_LEN, sliding window of size MAX_NGRAM_LEN
        if bytes.len() > MAX_NGRAM_LEN {
            for window in bytes.windows(MAX_NGRAM_LEN) {
                ngrams.insert(hash_ngram(window));
            }
        }

        ngrams.into_iter().collect()
    }
}
