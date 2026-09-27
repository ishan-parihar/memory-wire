//! Vector stream for hybrid recall (cosine over dense embeddings).
//!
//! Nothing produces a vector today: there is no embedder, no stored embedding
//! column, and no vector branch in the recall path. What is here is the cheap
//! half of a future vector stream — the ranking kernel that consumes
//! already-embedded candidates — kept because it is dependency-free and costs
//! zero bytes in a build that never calls it.

/// Cosine similarity in [-1, 1] (0.0 when either vector is degenerate).
///
/// Pure kernel — no embedding dependency required.
pub fn cosine(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }
    let (mut dot, mut na, mut nb) = (0.0f32, 0.0f32, 0.0f32);
    for (x, y) in a.iter().zip(b.iter()) {
        dot += x * y;
        na += x * x;
        nb += y * y;
    }
    if na <= 0.0 || nb <= 0.0 {
        return 0.0;
    }
    dot / (na.sqrt() * nb.sqrt())
}

/// Rank candidate ids by cosine against the query vector (best-first).
///
/// Returns `(id, score)` sorted by descending score, stable by id.
pub fn rank_by_cosine(ids: &[String], query: &[f32], vectors: &[Vec<f32>]) -> Vec<(String, f32)> {
    let mut out: Vec<(String, f32)> = ids
        .iter()
        .zip(vectors.iter())
        .map(|(id, v)| (id.clone(), cosine(query, v)))
        .filter(|(_, s)| *s > 0.0)
        .collect();
    out.sort_by(|a, b| {
        b.1.partial_cmp(&a.1)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then_with(|| a.0.cmp(&b.0))
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cosine_should_be_one_for_identical() {
        assert!((cosine(&[1.0, 0.0], &[1.0, 0.0]) - 1.0).abs() < 1e-6);
    }

    #[test]
    fn cosine_should_be_zero_for_orthogonal() {
        assert!(cosine(&[1.0, 0.0], &[0.0, 1.0]).abs() < 1e-6);
    }

    #[test]
    fn cosine_should_be_zero_on_a_length_mismatch() {
        // Different dimensions are not comparable, and a truncated embedding is
        // the shape a half-written vector store produces, so it must not be
        // silently zipped into a partial (and wrong) score.
        assert_eq!(cosine(&[1.0, 0.0], &[1.0, 0.0, 0.0]), 0.0);
        assert_eq!(cosine(&[1.0, 0.0, 0.0], &[1.0, 0.0]), 0.0);
    }

    #[test]
    fn cosine_should_be_zero_on_a_degenerate_vector() {
        // A zero vector has no direction, so its angle to anything is
        // undefined; 0.0 is the "no evidence" answer, not NaN.
        assert_eq!(cosine(&[0.0, 0.0], &[1.0, 0.0]), 0.0);
        assert_eq!(cosine(&[1.0, 0.0], &[0.0, 0.0]), 0.0);
        assert_eq!(cosine(&[0.0, 0.0], &[0.0, 0.0]), 0.0);
    }

    #[test]
    fn rank_should_order_best_first() {
        let ids = vec!["a".to_string(), "b".to_string()];
        let out = rank_by_cosine(&ids, &[1.0, 0.0], &[vec![0.0, 1.0], vec![1.0, 0.0]]);
        assert_eq!(out[0].0, "b");
    }
}
