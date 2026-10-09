//! BM25 ranking for `canon search` (depenaudit U-1, 2026-10-09;
//! REAL-ALGORITHMS.md §5).
//!
//! Replaces the O(n·m) substring scan: no ranking, no term weighting, store
//! order as relevance. Robertson–Spärck Jones / Zaragoza BM25:
//!   score(d,q) = Σ IDF(t) · tf(t,d)·(k1+1) / (tf(t,d) + k1·(1−b+b·|d|/avgdl))
//!   IDF(t) = ln(1 + (N − df(t) + 0.5) / (df(t) + 0.5))
//! k1=1.2, b=0.75 (TREC-tuned defaults). Tokenizer: lowercase alphanumeric
//! runs. No stemming — dependency-free, deterministic, and exact-term IDF
//! already dominates the old scan on every non-typo query.

use std::collections::HashMap;

pub const K1: f64 = 1.2;
pub const B: f64 = 0.75;

/// Lowercase alphanumeric tokenization.
pub fn tokenize(s: &str) -> Vec<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| t.to_string())
        .collect()
}

/// Rank documents against a query. `docs` are pre-tokenized term lists.
/// Returns `(doc_index, score)` sorted by score descending, score > 0 only.
pub fn rank(query: &str, docs: &[Vec<String>]) -> Vec<(usize, f64)> {
    let qterms = tokenize(query);
    if qterms.is_empty() || docs.is_empty() {
        return Vec::new();
    }
    let n = docs.len() as f64;

    // Per-doc term frequencies and lengths (in tokens).
    let mut doc_tfs: Vec<HashMap<&str, usize>> = Vec::with_capacity(docs.len());
    let mut doc_lens: Vec<f64> = Vec::with_capacity(docs.len());
    for d in docs {
        let mut tf: HashMap<&str, usize> = HashMap::new();
        for t in d {
            *tf.entry(t.as_str()).or_insert(0) += 1;
        }
        doc_lens.push(d.len() as f64);
        doc_tfs.push(tf);
    }
    let avgdl = doc_lens.iter().sum::<f64>() / n;

    // Document frequency per DISTINCT query term.
    let mut qterms_dedup: Vec<&str> = Vec::new();
    for t in &qterms {
        if !qterms_dedup.contains(&t.as_str()) {
            qterms_dedup.push(t.as_str());
        }
    }
    let mut idf: HashMap<&str, f64> = HashMap::new();
    for t in &qterms_dedup {
        let df = doc_tfs.iter().filter(|tf| tf.contains_key(t)).count() as f64;
        idf.insert(t, ((n - df + 0.5) / (df + 0.5) + 1.0).ln());
    }

    let mut scored: Vec<(usize, f64)> = Vec::new();
    for (i, tf) in doc_tfs.iter().enumerate() {
        let mut s = 0.0;
        for t in &qterms_dedup {
            let f = *tf.get(t).unwrap_or(&0) as f64;
            if f == 0.0 {
                continue;
            }
            let denom = f + K1 * (1.0 - B + B * doc_lens[i] / avgdl);
            s += idf[t] * (f * (K1 + 1.0)) / denom;
        }
        if s > 0.0 {
            scored.push((i, s));
        }
    }
    scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
    scored
}

#[cfg(test)]
mod tests {
    use super::*;

    fn docs() -> Vec<Vec<String>> {
        vec![
            tokenize("the cat sat on the mat"),
            tokenize("a cat and a dog playing"),
            tokenize("quantum field theory lecture notes"),
        ]
    }

    #[test]
    fn ranks_relevant_first() {
        // No stemming: "cat" matches "cat", never "cats".
        let r = rank("cat", &docs());
        assert_eq!(r.len(), 2, "only the two cat docs match: {:?}", r);
        assert!(!r.iter().any(|(i, _)| *i == 2), "quantum doc must not match");
        // Higher tf wins at equal idf: doc with "cat cat" outranks "cat".
        let d2 = vec![
            tokenize("cat"),
            tokenize("cat cat cat"),
            tokenize("quantum field theory"),
        ];
        let r2 = rank("cat", &d2);
        assert_eq!(r2[0].0, 1, "tf=3 beats tf=1: {:?}", r2);
    }

    #[test]
    fn idf_downweights_ubiquitous_terms() {
        // "the" is in 2 of 3 docs, "quantum" in 1: rare term must dominate.
        let d = vec![
            tokenize("the cat sat"),
            tokenize("the dog ran"),
            tokenize("quantum theory"),
        ];
        let r = rank("the quantum", &d);
        assert_eq!(r[0].0, 2, "rare 'quantum' outranks ubiquitous 'the': {:?}", r);
    }

    #[test]
    fn empty_query_yields_nothing() {
        assert!(rank("", &docs()).is_empty());
        assert!(rank("   ", &docs()).is_empty());
    }

    #[test]
    fn scores_are_deterministic() {
        let a = rank("cat dogs", &docs());
        let b = rank("cat dogs", &docs());
        assert_eq!(a, b);
    }
}
