use super::frequency::get_pair_weight;
use regex_syntax::hir::{Hir, HirKind};
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

#[derive(Debug, Clone)]
pub enum QueryAtoms {
    /// No selective literals available; must fallback to scanning all files
    AllFiles,
    /// Disjunction of conjunctions: (A AND B) OR (C AND D)
    Branches(Vec<Vec<String>>),
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

        // 1. For small trigrams with high-weight endpoints
        for i in 0..num_bigrams.saturating_sub(1) {
            let w1 = weights[i];
            let w2 = weights[i + 1];
            if w1 >= 45000 || w2 >= 45000 {
                let slice = &text[i..i + 3];
                ngrams.insert(hash_ngram(slice));
            }
        }

        // 2. Variable-length sparse n-grams with endpoint dominance:
        // min(W_L, W_R) > max(W_interior)
        // Maintained incrementally in O(1) per length extension
        for i in 0..num_bigrams {
            let left_w = weights[i];
            let max_len = (num_bigrams + 1 - i).min(MAX_NGRAM_LEN);
            if max_len < 4 {
                continue;
            }

            let mut interior_max = weights[i + 1];

            // Window of length 4: bigrams are weights[i], weights[i+1], weights[i+2]
            let right_w4 = weights[i + 2];
            if left_w.min(right_w4) > interior_max {
                ngrams.insert(hash_ngram(&text[i..i + 4]));
            }

            // Windows of length 5..=max_len:
            // The previous right endpoint (weights[i + len - 3]) is absorbed into the interior
            for len in 5..=max_len {
                interior_max = interior_max.max(weights[i + len - 3]);
                let right_w = weights[i + len - 2];
                if left_w.min(right_w) > interior_max {
                    ngrams.insert(hash_ngram(&text[i..i + len]));
                }
            }
        }

        ngrams.into_iter().collect()
    }

    /// Extracts sparse n-grams from a literal string for candidate document filtering
    pub fn extract_query_ngrams(query: &str) -> Vec<u64> {
        let bytes = query.as_bytes();
        let mut ngrams = HashSet::new();

        // Extract sub-ngrams using the deterministic sparse n-gram algorithm
        let extracted = Self::extract_all(bytes);
        for h in extracted {
            ngrams.insert(h);
        }

        // If query is longer than MAX_NGRAM_LEN, sliding window of size MAX_NGRAM_LEN
        if bytes.len() > MAX_NGRAM_LEN {
            for window in bytes.windows(MAX_NGRAM_LEN) {
                let sub = Self::extract_all(window);
                for h in sub {
                    ngrams.insert(h);
                }
            }
        }

        // Fallback to overlapping trigrams if sparse extractor produced no dominant endpoints
        if ngrams.is_empty() && bytes.len() >= MIN_NGRAM_LEN {
            for window in bytes.windows(3) {
                ngrams.insert(hash_ngram(window));
            }
        }

        ngrams.into_iter().collect()
    }

    /// Analyzes a regex query pattern and extracts literal search atoms
    pub fn extract_regex_atoms(pattern: &str) -> QueryAtoms {
        let mut parser = regex_syntax::ParserBuilder::new().build();
        let hir = match parser.parse(pattern) {
            Ok(h) => h,
            Err(_) => return Self::fallback_literal_atoms(pattern),
        };

        let atoms = Self::extract_from_hir(&hir);
        match atoms {
            Some(branches) if !branches.is_empty() => QueryAtoms::Branches(branches),
            _ => Self::fallback_literal_atoms(pattern),
        }
    }

    fn extract_from_hir(hir: &Hir) -> Option<Vec<Vec<String>>> {
        match hir.kind() {
            HirKind::Look(_) => Some(vec![vec![]]),
            HirKind::Literal(lit) => {
                let s = std::str::from_utf8(&lit.0).ok()?;
                if s.len() >= MIN_NGRAM_LEN {
                    Some(vec![vec![s.to_string()]])
                } else {
                    Some(vec![vec![]])
                }
            }
            HirKind::Concat(subs) => {
                let mut current_branches = vec![vec![]];
                for sub in subs {
                    let sub_atoms = Self::extract_from_hir(sub)?;
                    let mut next_branches = Vec::new();
                    for cur in &current_branches {
                        for sub_b in &sub_atoms {
                            let mut combined = cur.clone();
                            combined.extend(sub_b.clone());
                            next_branches.push(combined);
                        }
                    }
                    current_branches = next_branches;
                }
                Some(current_branches)
            }
            HirKind::Alternation(subs) => {
                let mut all_branches = Vec::new();
                for sub in subs {
                    match Self::extract_from_hir(sub) {
                        Some(sub_branches) => all_branches.extend(sub_branches),
                        None => return None,
                    }
                }
                if all_branches.is_empty() {
                    None
                } else {
                    Some(all_branches)
                }
            }
            HirKind::Capture(cap) => Self::extract_from_hir(&cap.sub),
            HirKind::Repetition(rep) => {
                if rep.min >= 1 {
                    Self::extract_from_hir(&rep.sub)
                } else {
                    Some(vec![vec![]])
                }
            }
            _ => None,
        }
    }

    fn fallback_literal_atoms(pattern: &str) -> QueryAtoms {
        let words: Vec<String> = pattern
            .split(|c: char| !c.is_alphanumeric() && c != '_')
            .filter(|w| w.len() >= MIN_NGRAM_LEN)
            .map(|w| w.to_string())
            .collect();

        if words.is_empty() {
            QueryAtoms::AllFiles
        } else {
            QueryAtoms::Branches(vec![words])
        }
    }
}
