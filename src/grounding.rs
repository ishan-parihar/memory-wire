//! Startup grounding capsule (W5).
//!
//! A per-bank, bounded materialization of the `active` rows whose `aspect` is
//! `preference|invariant|trap|map` and whose `scope` is `policy|global|domain`,
//! one line each. The capsule is **subtracted from** the recall budget when
//! both are served in one injection, so the total never exceeds
//! `DEFAULT_RECALL_BUDGET`.
//!
//! # Bound
//!
//! `max_lines` default **40**, `max_bytes` default **2048** (~500 tokens).
//!
//! # Path conditioning
//!
//! Rows may carry a `scope_globs` list (comma-separated). A row with a
//! non-empty glob set is **excluded from the capsule** unless an
//! `active_paths` argument matches one.
//!
//! # Opt-in
//!
//! Per-bank config flag, default **off**. Nothing injects it until a bank asks.

use crate::memory::Lifecycle;
use crate::store::{SqliteStore, Store};

/// Default max lines in a capsule.
pub const DEFAULT_MAX_LINES: usize = 40;

/// Default max bytes in a capsule.
pub const DEFAULT_MAX_BYTES: usize = 2048;

/// The aspects that are eligible for the capsule.
const CAPSULE_ASPECTS: &[&str] = &["preference", "invariant", "trap", "map"];

/// The scopes that are eligible for the capsule.
const CAPSULE_SCOPES: &[&str] = &["policy", "global", "domain"];

/// A single line in the grounding capsule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapsuleLine {
    /// The memory id.
    pub id: String,
    /// The scope/aspect label.
    pub label: String,
    /// The ISO date prefix, if the wire carried one.
    pub date: Option<String>,
    /// The content, clamped.
    pub content: String,
}

impl std::fmt::Display for CapsuleLine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let date = self
            .date
            .as_deref()
            .filter(|t| t.len() >= 10)
            .map(|t| format!("[{}] ", &t[..10]))
            .unwrap_or_default();
        write!(f, "[{}] {}{}", self.label, date, self.content)
    }
}

/// Options for building a grounding capsule.
#[derive(Debug, Clone)]
pub struct CapsuleOptions {
    /// Maximum lines in the capsule.
    pub max_lines: usize,
    /// Maximum bytes in the capsule.
    pub max_bytes: usize,
    /// Active paths for glob conditioning. A row with a non-empty glob set is
    /// excluded unless one of these matches a glob.
    pub active_paths: Vec<String>,
}

impl Default for CapsuleOptions {
    fn default() -> Self {
        Self {
            max_lines: DEFAULT_MAX_LINES,
            max_bytes: DEFAULT_MAX_BYTES,
            active_paths: Vec::new(),
        }
    }
}

/// Build a grounding capsule for a bank.
///
/// Returns the capsule lines and the total byte count. The caller is
/// responsible for subtracting the byte count from the recall budget.
pub fn build_capsule(
    store: &SqliteStore,
    bank: &str,
    opts: &CapsuleOptions,
) -> Result<Vec<CapsuleLine>, crate::store::StoreError> {
    let rows = store.list(bank)?;

    let mut lines = Vec::new();
    let mut total_bytes = 0;

    for r in &rows {
        // Filter by lifecycle: only active.
        if r.meta.lifecycle != Lifecycle("active".into()) {
            continue;
        }
        // Filter by aspect.
        if !CAPSULE_ASPECTS.contains(&r.meta.aspect.0.as_str()) {
            continue;
        }
        // Filter by scope.
        if !CAPSULE_SCOPES.contains(&r.meta.scope.0.as_str()) {
            continue;
        }
        // Path conditioning: a row with a non-empty glob set is excluded unless
        // an active path matches one.
        if !r.meta.scope_globs.is_empty() {
            let matched = opts
                .active_paths
                .iter()
                .any(|p| r.meta.scope_globs.iter().any(|g| glob_match(g, p)));
            if !matched {
                continue;
            }
        }

        let label = format!("{}/{}", r.meta.scope.0, r.meta.aspect.0);
        let line = CapsuleLine {
            id: r.id.clone(),
            label,
            date: r.created_at.clone(),
            content: r.content.clone(),
        };

        let line_str = line.to_string();
        let line_bytes = line_str.len();
        if total_bytes + line_bytes > opts.max_bytes {
            break;
        }
        if lines.len() >= opts.max_lines {
            break;
        }

        total_bytes += line_bytes;
        lines.push(line);
    }

    Ok(lines)
}

/// A simple glob matcher: `*` matches any sequence, `?` matches any single
/// character. The pattern is anchored at both ends.
fn glob_match(pattern: &str, path: &str) -> bool {
    // Convert to regex-equivalent matching.
    let mut p = String::new();
    for c in pattern.chars() {
        match c {
            '*' => p.push_str(".*"),
            '?' => p.push('.'),
            _ => {
                if ".+()|[]{}^$".contains(c) {
                    p.push('\\');
                }
                p.push(c);
            }
        }
    }
    let re = regex::Regex::new(&format!("^{}$", p)).ok();
    match re {
        Some(re) => re.is_match(path),
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{Aspect, Memory, MemoryMeta, Scope};
    use crate::memory::Bank;

    fn mem_with_meta(
        bank: &str,
        id: &str,
        content: &str,
        scope: Scope,
        aspect: Aspect,
        lifecycle: Lifecycle,
        scope_globs: Vec<String>,
    ) -> Memory {
        Memory {
            id: id.into(),
            bank_id: bank.into(),
            content: content.into(),
            context: None,
            created_at: Some("2026-10-14T10:00:00Z".into()),
            meta: MemoryMeta {
                scope,
                aspect,
                lifecycle,
                scope_globs,
                ..Default::default()
            },
        }
    }

    fn setup(rows: Vec<Memory>) -> SqliteStore {
        let s = SqliteStore::open_in_memory().expect("open");
        s.put_bank(&Bank {
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
    fn capsule_includes_only_preference_invariant_trap_map() {
        let rows = vec![
            mem_with_meta(
                "a",
                "m1",
                "never use em dashes",
                Scope("global".into()),
                Aspect("preference".into()),
                Lifecycle("active".into()),
                vec![],
            ),
            mem_with_meta(
                "a",
                "m2",
                "the api uses rest",
                Scope("global".into()),
                Aspect("fact".into()),
                Lifecycle("active".into()),
                vec![],
            ),
            mem_with_meta(
                "a",
                "m3",
                "longmem budgets are 100k",
                Scope("domain".into()),
                Aspect("trap".into()),
                Lifecycle("active".into()),
                vec![],
            ),
        ];
        let s = setup(rows);
        let lines = build_capsule(&s, "a", &CapsuleOptions::default()).expect("capsule");
        assert_eq!(lines.len(), 2, "only preference and trap, not fact");
        assert_eq!(lines[0].id, "m1");
        assert_eq!(lines[1].id, "m3");
    }

    #[test]
    fn capsule_respects_max_lines() {
        let rows: Vec<_> = (0..50)
            .map(|i| {
                mem_with_meta(
                    "a",
                    &format!("m{}", i),
                    &format!("rule number {}", i),
                    Scope("global".into()),
                    Aspect("preference".into()),
                    Lifecycle("active".into()),
                    vec![],
                )
            })
            .collect();
        let s = setup(rows);
        let lines = build_capsule(&s, "a", &CapsuleOptions::default()).expect("capsule");
        assert!(lines.len() <= DEFAULT_MAX_LINES);
    }

    #[test]
    fn capsule_respects_max_bytes() {
        let big = "x".repeat(500);
        let rows = vec![mem_with_meta(
            "a",
            "m1",
            &big,
            Scope("global".into()),
            Aspect("preference".into()),
            Lifecycle("active".into()),
            vec![],
        )];
        let s = setup(rows);
        let opts = CapsuleOptions {
            max_bytes: 100,
            ..Default::default()
        };
        let lines = build_capsule(&s, "a", &opts).expect("capsule");
        assert!(lines.is_empty(), "the one row exceeds the byte cap");
    }

    #[test]
    fn glob_scoped_row_excluded_without_matching_path() {
        let rows = vec![mem_with_meta(
            "a",
            "m1",
            "conditional rule",
            Scope("global".into()),
            Aspect("preference".into()),
            Lifecycle("active".into()),
            vec!["src/eng/**".into()],
        )];
        let s = setup(rows);
        let opts = CapsuleOptions {
            active_paths: vec!["src/other/file.rs".into()],
            ..Default::default()
        };
        let lines = build_capsule(&s, "a", &opts).expect("capsule");
        assert!(lines.is_empty(), "no matching path");
    }

    #[test]
    fn glob_scoped_row_included_with_matching_path() {
        let rows = vec![mem_with_meta(
            "a",
            "m1",
            "conditional rule",
            Scope("global".into()),
            Aspect("preference".into()),
            Lifecycle("active".into()),
            vec!["src/eng/**".into()],
        )];
        let s = setup(rows);
        let opts = CapsuleOptions {
            active_paths: vec!["src/eng/memory-wire/src/store.rs".into()],
            ..Default::default()
        };
        let lines = build_capsule(&s, "a", &opts).expect("capsule");
        assert_eq!(lines.len(), 1, "matching path includes the row");
    }

    #[test]
    fn superseded_rows_excluded() {
        let rows = vec![mem_with_meta(
            "a",
            "m1",
            "old rule",
            Scope("global".into()),
            Aspect("preference".into()),
            Lifecycle("superseded".into()),
            vec![],
        )];
        let s = setup(rows);
        let lines = build_capsule(&s, "a", &CapsuleOptions::default()).expect("capsule");
        assert!(lines.is_empty(), "superseded rows are excluded");
    }
}
