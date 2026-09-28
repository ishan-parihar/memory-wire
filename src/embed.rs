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
///
/// # What disqualifies a candidate
///
/// **Only a missing vector.** `ids` and `vectors` are read as parallel lists
/// (`zip`), so an id with no vector has no row here at all and is *unranked*;
/// that is the whole of the rule, and it is why a vectorless memory cannot
/// become "unranked filler" in a fused list. `crate::vector::vector_stream` is
/// the caller that enforces it, by pairing an id with a vector only when the
/// store holds one.
///
/// **A stored vector is always ranked, however low its cosine.** A negative
/// cosine is a real measurement — the query points away from the document — so
/// the row is kept and sorted to the bottom. Deleting it instead of ranking it
/// would make the kernel's reach smaller than the store's, which is a
/// *coverage* claim and not a ranking one: RRF's `w/(k+rank)` is monotone in
/// rank, so a document at the bottom of a shorter list and a document deleted
/// from a longer one are not the same statement about what the arm can find.
/// (This used to be a `filter(|(_, s)| *s > 0.0)` and was a defect: it
/// silently deleted one gold document's only path into the ranking on the
/// LoCoMo dev set, whose measured cosine was `-0.013347`.)
pub fn rank_by_cosine(ids: &[String], query: &[f32], vectors: &[Vec<f32>]) -> Vec<(String, f32)> {
    let mut out: Vec<(String, f32)> = ids
        .iter()
        .zip(vectors.iter())
        .map(|(id, v)| (id.clone(), cosine(query, v)))
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

    /// The regression this kernel's rule exists to pin: a **negative cosine is a
    /// score, not a verdict**. A vector that points away from the query is
    /// evidence, and the arm's job is to order every candidate it was handed.
    /// Filtering non-positive scores out instead deleted the candidate, which
    /// cost the dense arm one gold document on the LoCoMo dev set (`conv-50_q50`,
    /// cosine `-0.013347`) and made its measured R@pool 99.9347% rather than
    /// 100% — a reach shortfall reported as a ranking result.
    ///
    /// Both halves of the rule are in one test because they are one rule: a
    /// stored vector is ranked, and only a *missing* vector is unranked. Here the
    /// vectorless id is expressed the way `vector_stream` expresses it — the id
    /// has no row in the parallel `vectors` list, so the `zip` never pairs it.
    #[test]
    fn a_negative_cosine_is_ranked_last_and_only_a_missing_vector_is_unranked() {
        // query points along +x.
        let ids = vec![
            "aligned".to_string(),   // cos  1.0
            "oblique".to_string(),   // cos  0.6
            "orthogonal".to_string(), // cos  0.0
            "opposed".to_string(),   // cos -1.0
        ];
        let out = rank_by_cosine(
            &ids,
            &[1.0, 0.0],
            &[
                vec![1.0, 0.0],
                vec![0.6, 0.8],
                vec![0.0, 1.0],
                vec![-1.0, 0.0],
            ],
        );
        // Every candidate that was handed a vector is ranked — four in, four out.
        assert_eq!(out.len(), 4, "a stored vector is always ranked: {}", out.len());
        assert_eq!(
            out.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(),
            ["aligned", "oblique", "orthogonal", "opposed"],
            "non-negative scores keep their order, and the negative one sorts below all of them"
        );
        assert!(out[3].1 < 0.0, "the last row is the opposed one, and it is negative");

        // And the counterpart: an id with no vector is *unranked*, not ranked at
        // `0.0`. Same shape `vector_stream` builds — an id is paired with a
        // vector only when the store holds one.
        let with_gap = vec!["aligned".to_string(), "opposed".to_string()];
        let out = rank_by_cosine(
            &with_gap,
            &[1.0, 0.0],
            &[
                vec![1.0, 0.0],
                vec![-1.0, 0.0], // "opposed"
                vec![0.0, 1.0],  // an extra vector with no id: never paired
            ],
        );
        assert_eq!(
            out.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(),
            ["aligned", "opposed"],
            "a vector without an id and an id without a vector are both unranked, in their own ways"
        );
    }

    /// Two candidates with equal cosine are ordered by id, ascending, and the
    /// tiebreak reaches a negative score too — the sort is over the whole
    /// ranking, not over a filtered prefix of it.
    #[test]
    fn equal_scores_tiebreak_by_id_including_at_negative_values() {
        let ids = vec!["b".to_string(), "a".to_string()];
        let out = rank_by_cosine(&ids, &[1.0, 0.0], &[vec![-1.0, 0.0], vec![-1.0, 0.0]]);
        assert_eq!(
            out.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(),
            ["a", "b"],
            "the id tiebreak must still apply once negatives are in the ranking"
        );
    }
}
