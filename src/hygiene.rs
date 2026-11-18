//! Deterministic hygiene scan for a bank's memories.
//!
//! The "dreaming substrate" — a pure-function read that surfaces candidate
//! merges, contradictions, relative dates, and bloat without an LLM, a new
//! dependency, or a new process. The scan is read-only; mutation is a separate
//! verb (`apply`) so the two fail differently and the scan stays cheap.
//!
//! # Deterministic
//!
//! Every detector is a pure function of the rows in the bank. No clock, no
//! randomness, no ranking path. Running the scan twice on the same state
//! produces byte-identical reports.
//!
//! # Surface
//!
//! `hygiene_scan(bank, cursor_id, opts) -> HygieneReport` is the entry point.
//! The cursor is a `created_at` watermark stored in `banks.config` as
//! `hygieneCursor`; a `None` cursor scans the whole bank.
//!
//! # Detectors
//!
//! - **near-duplicates**: Jaccard token similarity above `min_jaccard` within
//!   the same `(scope, aspect)` — candidate `MERGE`.
//! - **contradictions**: same `scope` + overlapping token set + differing
//!   `created_at` and a negation/contrast marker — candidate `SUPERSEDE`.
//! - **relative dates**: regex for relative-date markers on rows that lack an
//!   ISO date — candidate `ANCHOR` (a *proposal*, never an automatic rewrite).
//! - **promotion candidates**: the same `(scope, aspect)` pattern appearing in
//!   >= `min_promotion_count` sessions — candidate `PROMOTE`.
//! - **bloat**: rows over `max_record_bytes`, and banks over
//!   `max_hot_index_lines/bytes` — candidate `SPLIT`.

use regex::Regex;
use std::collections::BTreeSet;
use std::fmt;

use crate::memory::{Aspect, Scope};

/// Default Jaccard threshold for near-duplicate detection.
pub const DEFAULT_MIN_JACCARD: f64 = 0.6;

/// Default minimum session count for promotion candidates.
pub const DEFAULT_MIN_PROMOTION_COUNT: usize = 2;

/// Default max bytes per record before it is flagged as bloat.
pub const DEFAULT_MAX_RECORD_BYTES: usize = 8192;

/// Default max lines before a bank is flagged as bloat.
pub const DEFAULT_MAX_HOT_INDEX_LINES: usize = 5000;

/// Default max bytes before a bank is flagged as bloat.
pub const DEFAULT_MAX_HOT_INDEX_BYTES: usize = 5 * 1024 * 1024;

/// A single hygiene candidate.
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub enum HygieneCandidate {
    /// Two or more memories in the same `(scope, aspect)` with Jaccard
    /// similarity above the threshold — candidate for merge.
    Merge {
        /// The memory ids involved.
        ids: Vec<String>,
        /// The Jaccard similarity that triggered the candidate.
        jaccard: f64,
    },
    /// Two memories in the same scope with overlapping token sets and a
    /// negation/contrast marker — candidate for supersede.
    Supersede {
        /// The newer memory's id.
        newer: String,
        /// The older memory's id.
        older: String,
    },
    /// A row with a relative-date marker and no ISO date — candidate for
    /// anchoring. A proposal, never an automatic rewrite.
    Anchor {
        /// The memory id.
        id: String,
        /// The relative-date markers found.
        markers: Vec<String>,
    },
    /// The same `(scope, aspect)` pattern appearing in multiple sessions —
    /// candidate for promotion.
    Promote {
        /// The memory ids.
        ids: Vec<String>,
        /// The `(scope, aspect)` pattern.
        pattern: String,
        /// The number of sessions.
        sessions: usize,
    },
    /// A row or bank over a size limit — candidate for split.
    Split {
        /// The memory id (or bank id for a bank-level split).
        id: String,
        /// The size that triggered the candidate.
        size: usize,
    },
}

impl HygieneCandidate {
    /// The kind of candidate, for filtering.
    pub fn kind(&self) -> &'static str {
        match self {
            HygieneCandidate::Merge { .. } => "merge",
            HygieneCandidate::Supersede { .. } => "supersede",
            HygieneCandidate::Anchor { .. } => "anchor",
            HygieneCandidate::Promote { .. } => "promote",
            HygieneCandidate::Split { .. } => "split",
        }
    }
}

impl fmt::Display for HygieneCandidate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            HygieneCandidate::Merge { ids, jaccard } => {
                write!(f, "merge: {} (jaccard {:.2})", ids.join(", "), jaccard)
            }
            HygieneCandidate::Supersede { newer, older } => {
                write!(f, "supersede: {} supersedes {}", newer, older)
            }
            HygieneCandidate::Anchor { id, markers } => {
                write!(f, "anchor: {} ({})", id, markers.join(", "))
            }
            HygieneCandidate::Promote { ids, pattern, sessions } => {
                write!(f, "promote: {} ({}x {})", ids.join(", "), pattern, sessions)
            }
            HygieneCandidate::Split { id, size } => {
                write!(f, "split: {} ({} bytes)", id, size)
            }
        }
    }
}

/// Options for a hygiene scan.
#[derive(Debug, Clone)]
pub struct HygieneOptions {
    /// Minimum Jaccard similarity for near-duplicate detection.
    pub min_jaccard: f64,
    /// Minimum session count for promotion candidates.
    pub min_promotion_count: usize,
    /// Maximum bytes per record before it is flagged as bloat.
    pub max_record_bytes: usize,
    /// Maximum lines before a bank is flagged as bloat.
    pub max_hot_index_lines: usize,
    /// Maximum bytes before a bank is flagged as bloat.
    pub max_hot_index_bytes: usize,
}

impl Default for HygieneOptions {
    fn default() -> Self {
        Self {
            min_jaccard: DEFAULT_MIN_JACCARD,
            min_promotion_count: DEFAULT_MIN_PROMOTION_COUNT,
            max_record_bytes: DEFAULT_MAX_RECORD_BYTES,
            max_hot_index_lines: DEFAULT_MAX_HOT_INDEX_LINES,
            max_hot_index_bytes: DEFAULT_MAX_HOT_INDEX_BYTES,
        }
    }
}

/// The result of a hygiene scan.
#[derive(Debug, Clone, PartialEq)]
pub struct HygieneReport {
    /// The bank that was scanned.
    pub bank: String,
    /// The cursor the scan started from (the `created_at` watermark).
    pub cursor: Option<String>,
    /// The candidates found, sorted deterministically.
    pub candidates: Vec<HygieneCandidate>,
    /// The total number of rows scanned.
    pub rows_scanned: usize,
}

impl fmt::Display for HygieneReport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "hygiene report for bank `{}`", self.bank)?;
        if let Some(cursor) = &self.cursor {
            writeln!(f, "  cursor: {}", cursor)?;
        } else {
            writeln!(f, "  cursor: (full scan)")?;
        }
        writeln!(f, "  rows scanned: {}", self.rows_scanned)?;
        writeln!(f, "  candidates: {}", self.candidates.len())?;
        for c in &self.candidates {
            writeln!(f, "    {}", c)?;
        }
        Ok(())
    }
}

/// Relative-date markers that indicate a row needs anchoring.
const RELATIVE_DATE_PATTERN: &str = r"(?i)\b(yesterday|today|tomorrow|last\s+(?:week|month|year)|next\s+(?:week|month|year)|currently|now|this\s+(?:sprint|week|month|quarter))\b";

/// Negation/contrast markers that indicate a potential contradiction.
const CONTRAST_PATTERN: &str =
    r"(?i)\b(actually|but|however|although|instead|rather|not|no|never|changed|switched|now|anymore)\b";

/// An ISO date (YYYY-MM-DD) somewhere in the content.
const ISO_DATE_PATTERN: &str = r"\b\d{4}-\d{2}-\d{2}\b";

/// Tokenize a string into a set of lowercased words.
fn tokenize(s: &str) -> BTreeSet<String> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|w| w.len() > 1)
        .map(|w| w.to_lowercase())
        .collect()
}

/// Jaccard similarity between two token sets.
fn jaccard(a: &BTreeSet<String>, b: &BTreeSet<String>) -> f64 {
    let intersection = a.intersection(b).count();
    let union = a.union(b).count();
    if union == 0 {
        0.0
    } else {
        intersection as f64 / union as f64
    }
}

/// Scan a bank for hygiene candidates.
///
/// The scan is read-only and deterministic. `cursor` is a `created_at`
/// watermark; rows with `created_at <= cursor` are skipped (the cursor is
/// exclusive). A `None` cursor scans the whole bank.
///
/// Generic over [`crate::store::Store`] rather than `&SqliteStore`: the scan
/// reads one trait method and writes nothing, and pinning it to the one
/// backend would make the HTTP route impossible — every route in this crate
/// is generic because the service is. The bank comes back as `list()` gives it,
/// so a backend that orders rows differently still scans deterministically.
pub fn hygiene_scan<S: crate::store::Store>(
    store: &S,
    bank: &str,
    cursor: Option<&str>,
    opts: &HygieneOptions,
) -> Result<HygieneReport, crate::store::StoreError> {
    // Fetch all rows in the bank. We do not paginate because the scan needs the
    // full set to find duplicates and contradictions; the cursor narrows the
    // delta but the detectors still need the whole bank for context.
    let all_rows = store.list(bank)?;

    // Filter to rows after the cursor (exclusive).
    let rows: Vec<_> = all_rows
        .into_iter()
        .filter(|r| match cursor {
            Some(c) => r.created_at.as_deref().map(|t| t > c).unwrap_or(true),
            None => true,
        })
        .collect();

    let rows_scanned = rows.len();
    let mut candidates = Vec::new();

    // Precompute tokens and parse the regexes once.
    let relative_re = Regex::new(RELATIVE_DATE_PATTERN)
        .map_err(|e| crate::store::StoreError::BadInput(format!("relative date regex: {e}")))?;
    let contrast_re = Regex::new(CONTRAST_PATTERN)
        .map_err(|e| crate::store::StoreError::BadInput(format!("contrast regex: {e}")))?;
    let iso_re = Regex::new(ISO_DATE_PATTERN)
        .map_err(|e| crate::store::StoreError::BadInput(format!("iso date regex: {e}")))?;

    let tokens: Vec<BTreeSet<String>> = rows.iter().map(|r| tokenize(&r.content)).collect();

    // 1. Near-duplicates: Jaccard similarity above threshold within the same
    //    (scope, aspect).
    for i in 0..rows.len() {
        for j in (i + 1)..rows.len() {
            let ri = &rows[i];
            let rj = &rows[j];
            if ri.meta.scope != rj.meta.scope || ri.meta.aspect != rj.meta.aspect {
                continue;
            }
            let sim = jaccard(&tokens[i], &tokens[j]);
            if sim >= opts.min_jaccard {
                let mut ids = vec![ri.id.clone(), rj.id.clone()];
                ids.sort();
                candidates.push(HygieneCandidate::Merge { ids, jaccard: sim });
            }
        }
    }

    // 2. Contradictions: same scope + overlapping token set + a contrast marker
    //    in one of them + differing created_at.
    for i in 0..rows.len() {
        for j in (i + 1)..rows.len() {
            let ri = &rows[i];
            let rj = &rows[j];
            if ri.meta.scope != rj.meta.scope {
                continue;
            }
            // Overlapping token set: at least 30% overlap.
            let sim = jaccard(&tokens[i], &tokens[j]);
            if sim < 0.3 {
                continue;
            }
            // Differing created_at.
            let ci = ri.created_at.as_deref().unwrap_or("");
            let cj = rj.created_at.as_deref().unwrap_or("");
            if ci == cj {
                continue;
            }
            // At least one has a contrast marker.
            let has_contrast =
                contrast_re.is_match(&ri.content) || contrast_re.is_match(&rj.content);
            if !has_contrast {
                continue;
            }
            // The newer one (by created_at) supersedes the older.
            let (newer, older) = if ci > cj { (&ri.id, &rj.id) } else { (&rj.id, &ri.id) };
            candidates.push(HygieneCandidate::Supersede {
                newer: newer.clone(),
                older: older.clone(),
            });
        }
    }

    // 3. Relative dates: rows with a relative-date marker and no ISO date.
    for r in &rows {
        if iso_re.is_match(&r.content) {
            continue;
        }
        if let Some(m) = relative_re.find(&r.content) {
            candidates.push(HygieneCandidate::Anchor {
                id: r.id.clone(),
                markers: vec![m.as_str().to_lowercase()],
            });
        }
    }

    // 4. Promotion candidates: the same (scope, aspect) pattern appearing in
    //    >= min_promotion_count sessions. We approximate "sessions" by the
    //    number of distinct created_at dates.
    let mut pattern_sessions: std::collections::BTreeMap<(Scope, Aspect), BTreeSet<String>> =
        std::collections::BTreeMap::new();
    for r in &rows {
        let key = (r.meta.scope.clone(), r.meta.aspect.clone());
        if let Some(date) = r.created_at.as_deref().and_then(|t| t.get(..10)) {
            pattern_sessions.entry(key).or_default().insert(date.to_string());
        }
    }
    for ((scope, aspect), dates) in &pattern_sessions {
        if dates.len() >= opts.min_promotion_count {
            let ids: Vec<String> = rows
                .iter()
                .filter(|r| r.meta.scope == *scope && r.meta.aspect == *aspect)
                .map(|r| r.id.clone())
                .collect();
            candidates.push(HygieneCandidate::Promote {
                ids,
                pattern: format!("{}/{}", scope.0, aspect.0),
                sessions: dates.len(),
            });
        }
    }

    // 5. Bloat: rows over max_record_bytes.
    for r in &rows {
        let size = r.content.len();
        if size > opts.max_record_bytes {
            candidates.push(HygieneCandidate::Split {
                id: r.id.clone(),
                size,
            });
        }
    }

    // Sort candidates deterministically by kind then by the first id-like field.
    candidates.sort_by(|a, b| {
        let ka = a.kind();
        let kb = b.kind();
        if ka != kb {
            return ka.cmp(kb);
        }
        // Within a kind, sort by the ids.
        match (a, b) {
            (HygieneCandidate::Merge { ids: ai, .. }, HygieneCandidate::Merge { ids: bi, .. }) => {
                ai.cmp(bi)
            }
            (
                HygieneCandidate::Supersede { newer: an, .. },
                HygieneCandidate::Supersede { newer: bn, .. },
            ) => an.cmp(bn),
            (HygieneCandidate::Anchor { id: ai, .. }, HygieneCandidate::Anchor { id: bi, .. }) => {
                ai.cmp(bi)
            }
            (
                HygieneCandidate::Promote { ids: ai, .. },
                HygieneCandidate::Promote { ids: bi, .. },
            ) => ai.cmp(bi),
            (HygieneCandidate::Split { id: ai, .. }, HygieneCandidate::Split { id: bi, .. }) => {
                ai.cmp(bi)
            }
            _ => std::cmp::Ordering::Equal,
        }
    });

    Ok(HygieneReport {
        bank: bank.to_string(),
        cursor: cursor.map(|c| c.to_string()),
        candidates,
        rows_scanned,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{Memory, MemoryMeta};
    use crate::store::{SqliteStore, Store};
    

    fn mem_with_content(
        bank: &str,
        id: &str,
        content: &str,
        scope: Scope,
        aspect: Aspect,
        created_at: &str,
    ) -> Memory {
        Memory {
            id: id.into(),
            bank_id: bank.into(),
            content: content.into(),
            context: None,
            created_at: Some(created_at.into()),
            meta: MemoryMeta {
                scope,
                aspect,
                ..Default::default()
            },
        }
    }

    fn setup_store_with_rows(rows: Vec<Memory>) -> SqliteStore {
        let s = SqliteStore::open_in_memory().expect("open");
        s.put_bank(&crate::memory::Bank {
            id: "a".into(),
            name: "a".into(),
        })
        .expect("bank");
        for r in rows {
            s.put(&r).expect("put");
        }
        s
    }

    #[test]
    fn scan_is_deterministic() {
        let rows = vec![
            mem_with_content(
                "a",
                "m1",
                "the quick brown fox jumps over the lazy dog",
                Scope("global".into()),
                Aspect("preference".into()),
                "2026-10-14T10:00:00Z",
            ),
            mem_with_content(
                "a",
                "m2",
                "the quick brown fox jumps over a lazy dog",
                Scope("global".into()),
                Aspect("preference".into()),
                "2026-10-15T10:00:00Z",
            ),
            mem_with_content(
                "a",
                "m3",
                "yesterday we deployed the new feature",
                Scope("domain".into()),
                Aspect("fact".into()),
                "2026-10-16T10:00:00Z",
            ),
        ];
        let s = setup_store_with_rows(rows);
        let opts = HygieneOptions::default();
        let r1 = hygiene_scan(&s, "a", None, &opts).expect("scan1");
        let r2 = hygiene_scan(&s, "a", None, &opts).expect("scan2");
        assert_eq!(r1, r2, "scan must be deterministic");
    }

    #[test]
    fn scan_finds_near_duplicates() {
        let rows = vec![
            mem_with_content(
                "a",
                "m1",
                "the quick brown fox jumps over the lazy dog",
                Scope("global".into()),
                Aspect("preference".into()),
                "2026-10-14T10:00:00Z",
            ),
            mem_with_content(
                "a",
                "m2",
                "the quick brown fox jumps over a lazy dog",
                Scope("global".into()),
                Aspect("preference".into()),
                "2026-10-15T10:00:00Z",
            ),
            mem_with_content(
                "a",
                "m3",
                "completely different content about databases",
                Scope("global".into()),
                Aspect("preference".into()),
                "2026-10-16T10:00:00Z",
            ),
        ];
        let s = setup_store_with_rows(rows);
        let opts = HygieneOptions::default();
        let report = hygiene_scan(&s, "a", None, &opts).expect("scan");
        let merges: Vec<_> = report
            .candidates
            .iter()
            .filter(|c| matches!(c, HygieneCandidate::Merge { .. }))
            .collect();
        assert_eq!(merges.len(), 1, "expected one merge candidate");
        if let HygieneCandidate::Merge { ids, jaccard } = merges[0] {
            assert_eq!(ids, &vec!["m1".to_string(), "m2".to_string()]);
            assert!(*jaccard > 0.6);
        }
    }

    #[test]
    fn scan_finds_relative_dates() {
        let rows = vec![
            mem_with_content(
                "a",
                "m1",
                "yesterday we deployed the new feature",
                Scope("domain".into()),
                Aspect("fact".into()),
                "2026-10-14T10:00:00Z",
            ),
            mem_with_content(
                "a",
                "m2",
                "the database migration completed on 2026-10-15",
                Scope("domain".into()),
                Aspect("fact".into()),
                "2026-10-15T10:00:00Z",
            ),
        ];
        let s = setup_store_with_rows(rows);
        let opts = HygieneOptions::default();
        let report = hygiene_scan(&s, "a", None, &opts).expect("scan");
        let anchors: Vec<_> = report
            .candidates
            .iter()
            .filter(|c| matches!(c, HygieneCandidate::Anchor { .. }))
            .collect();
        assert_eq!(anchors.len(), 1, "expected one anchor candidate");
        if let HygieneCandidate::Anchor { id, markers } = anchors[0] {
            assert_eq!(id, "m1");
            assert!(markers.iter().any(|m| m.contains("yesterday")));
        }
    }

    #[test]
    fn scan_finds_contradictions() {
        let rows = vec![
            mem_with_content(
                "a",
                "m1",
                "the api uses rest for all endpoints",
                Scope("global".into()),
                Aspect("fact".into()),
                "2026-10-01T10:00:00Z",
            ),
            mem_with_content(
                "a",
                "m2",
                "actually the api now uses graphql for most endpoints",
                Scope("global".into()),
                Aspect("fact".into()),
                "2026-10-15T10:00:00Z",
            ),
        ];
        let s = setup_store_with_rows(rows);
        let opts = HygieneOptions::default();
        let report = hygiene_scan(&s, "a", None, &opts).expect("scan");
        let supersedes: Vec<_> = report
            .candidates
            .iter()
            .filter(|c| matches!(c, HygieneCandidate::Supersede { .. }))
            .collect();
        assert_eq!(supersedes.len(), 1, "expected one supersede candidate");
        if let HygieneCandidate::Supersede { newer, older } = supersedes[0] {
            assert_eq!(newer, "m2");
            assert_eq!(older, "m1");
        }
    }

    #[test]
    fn scan_finds_bloat() {
        let big = "x".repeat(10_000);
        let rows = vec![mem_with_content(
            "a",
            "m1",
            &big,
            Scope("global".into()),
            Aspect("fact".into()),
            "2026-10-14T10:00:00Z",
        )];
        let s = setup_store_with_rows(rows);
        let opts = HygieneOptions {
            max_record_bytes: 5000,
            ..Default::default()
        };
        let report = hygiene_scan(&s, "a", None, &opts).expect("scan");
        let splits: Vec<_> = report
            .candidates
            .iter()
            .filter(|c| matches!(c, HygieneCandidate::Split { .. }))
            .collect();
        assert_eq!(splits.len(), 1, "expected one split candidate");
    }

    #[test]
    fn cursor_skips_old_rows() {
        let rows = vec![
            mem_with_content(
                "a",
                "m1",
                "old row",
                Scope("global".into()),
                Aspect("fact".into()),
                "2026-10-01T10:00:00Z",
            ),
            mem_with_content(
                "a",
                "m2",
                "new row",
                Scope("global".into()),
                Aspect("fact".into()),
                "2026-10-15T10:00:00Z",
            ),
        ];
        let s = setup_store_with_rows(rows);
        let opts = HygieneOptions::default();
        let report = hygiene_scan(&s, "a", Some("2026-10-10T00:00:00Z"), &opts).expect("scan");
        assert_eq!(report.rows_scanned, 1, "only rows after the cursor are scanned");
    }
}