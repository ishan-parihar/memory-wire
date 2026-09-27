//! Native agent-rules writer: an idempotent, marker-delimited markdown block.
//!
//! The block is the only thing this writer owns. Everything outside the
//! markers is the user's, so an existing file is edited in place rather than
//! rewritten, and a file whose markers are unbalanced is refused outright: a
//! half-written block means something else edited it, and guessing where the
//! user's content resumes risks truncating it.

use std::path::{Path, PathBuf};

use crate::paths;

/// Opening marker of the managed block.
pub const START: &str = "<!-- memory-wire:start -->";
/// Closing marker of the managed block.
pub const END: &str = "<!-- memory-wire:end -->";

/// A rules file and the agents that read it.
pub struct Target {
    /// Hosts served by this file.
    pub labels: &'static [&'static str],
    /// Path relative to the base directory.
    pub rel: &'static str,
}

/// Every rules file `connect --guidelines` maintains.
///
/// `AGENTS.md` is the cross-agent standard *and* opencode's native rules file,
/// so it is one physical file serving two labels — writing it twice would be
/// the same bytes twice.
pub const TARGETS: &[Target] = &[
    Target { labels: &["universal", "opencode"], rel: "AGENTS.md" },
    Target { labels: &["claude-code"], rel: "CLAUDE.md" },
    Target { labels: &["gemini-cli"], rel: "GEMINI.md" },
    Target { labels: &["cursor"], rel: ".cursorrules" },
    Target { labels: &["copilot-cli"], rel: ".github/copilot-instructions.md" },
];

/// What a target write did.
#[derive(Debug, PartialEq, Eq)]
pub enum Applied {
    /// The file did not exist and now holds only the block.
    Created,
    /// The block was inserted or refreshed inside an existing file.
    Updated,
    /// The file already had exactly this block.
    Unchanged,
    /// Refused: the file was left untouched.
    Refused(String),
}

impl Applied {
    /// One report line.
    pub fn line(&self, rel: &str, labels: &[&str]) -> String {
        let who = labels.join(", ");
        match self {
            Applied::Created => format!("{rel:<32} created   ({who})"),
            Applied::Updated => format!("{rel:<32} updated   ({who})"),
            Applied::Unchanged => format!("{rel:<32} unchanged ({who})"),
            Applied::Refused(why) => format!("{rel:<32} REFUSED   ({who}): {why}"),
        }
    }
}

/// The three operations as `curl` one-liners.
///
/// Plain `curl` because that is the one thing every host can run without a
/// client library: the block is read by a model, not by a program.
pub fn curl_examples(endpoint: &str, bank: &str) -> String {
    let post = |op: &str, key: &str, arg: &str| {
        format!(
            "curl -sS -X POST {endpoint}/banks/{bank}/{op} \\\n    -H 'Content-Type: application/json' \\\n    -d '{{\"{key}\":\"{arg}\"}}'"
        )
    };
    format!(
        "- **retain** — record what was decided and on what reasoning:\n  {}\n- **recall** — search memory before assuming:\n  {}\n- **reflect** — ask what was decided and why:\n  {}",
        post("retain", "content", "the durable fact, in one sentence"),
        post("recall", "query", "your question"),
        post("reflect", "query", "the decision you need explained"),
    )
}

/// The full managed block for one endpoint + bank.
pub fn block(endpoint: &str, bank: &str) -> String {
    format!(
        "{START}\n\
         ## memory-wire — persistent memory over HTTP\n\
         \n\
         Bank `{bank}`, endpoint `{endpoint}`. Three operations:\n\
         \n\
         {}\n\
         \n\
         How to use it:\n\
         \n\
         - **Recall before you answer** anything about a decision, a convention, or\n  \
           prior work in this repository. A recall costs one loopback call; guessing\n  \
           costs a wrong edit.\n\
         - **Retain outcomes, not file state.** Store what was decided and on what\n  \
           reasoning. A memory that says \"the parser is in `src/x.rs`\" goes stale and\n  \
           then misleads; \"auth uses `jose`, chosen over `jsonwebtoken` to avoid a\n  \
           second JWT implementation\" stays true.\n\
         - **Never store secrets.** Redaction is a safety net, not permission.\n\
         - **Reflect for history.** When asked why something is the way it is, read\n  \
           the record rather than guessing from the current code, which may have moved\n  \
           past the decision.\n\
         \n\
         Managed by `memory-wire connect --guidelines`. Edit outside the markers.\n\
         {END}",
        curl_examples(endpoint, bank)
    )
}

/// Insert or replace the block inside `existing`.
///
/// `Ok` carries the new whole-file content. `Err` means the caller must leave
/// the file alone: the markers are unbalanced, duplicated, or out of order.
pub fn upsert(existing: &str, block: &str) -> Result<String, String> {
    let starts = existing.matches(START).count();
    let ends = existing.matches(END).count();
    if starts == 0 && ends == 0 {
        let mut out = String::with_capacity(existing.len() + block.len() + 2);
        out.push_str(existing.trim_end());
        if !out.is_empty() {
            out.push_str("\n\n");
        }
        out.push_str(block);
        out.push('\n');
        return Ok(out);
    }
    if starts != 1 || ends != 1 {
        return Err(format!(
            "found {starts} '{START}' and {ends} '{END}' markers, need exactly one of each"
        ));
    }
    let s = existing.find(START).expect("counted above");
    let e = existing.find(END).expect("counted above");
    if e < s {
        return Err("end marker appears before start marker".to_string());
    }
    let head = &existing[..s];
    let tail = &existing[e + END.len()..];
    let mut out = String::with_capacity(head.len() + block.len() + tail.len());
    out.push_str(head);
    out.push_str(block);
    out.push_str(tail);
    Ok(out)
}

/// Write the block into one rules file under `base`.
pub fn apply_to(base: &Path, target: &Target, block: &str) -> Applied {
    let path: PathBuf = base.join(target.rel);
    let existing = match paths::read_or_empty(&path) {
        Ok(s) => s,
        Err(e) => return Applied::Refused(e.to_string()),
    };
    let created = !path.exists();
    let next = match upsert(&existing, block) {
        Ok(s) => s,
        Err(why) => return Applied::Refused(why),
    };
    if next == existing {
        return Applied::Unchanged;
    }
    match paths::write_atomic(&path, &next) {
        Ok(()) if created => Applied::Created,
        Ok(()) => Applied::Updated,
        Err(e) => Applied::Refused(e),
    }
}

/// Write the block into every target, returning one report line per target.
pub fn apply_all(base: &Path, block: &str) -> Vec<(String, Applied)> {
    TARGETS
        .iter()
        .map(|t| {
            let applied = apply_to(base, t, block);
            let line = applied.line(t.rel, t.labels);
            (line, applied)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn demo_block() -> String {
        block("http://127.0.0.1:8888", "demo")
    }

    #[test]
    fn upsert_should_append_when_absent_and_then_be_a_no_op() {
        let first = upsert("# My rules\n\nBe nice.\n", &demo_block()).expect("append");
        assert!(first.starts_with("# My rules\n\nBe nice.\n\n"));
        assert!(first.contains(START) && first.contains(END));
        // Idempotency: same input, byte-identical output.
        let second = upsert(&first, &demo_block()).expect("replace");
        assert_eq!(first, second);
    }

    #[test]
    fn upsert_should_replace_the_block_and_keep_both_sides() {
        let head = "# Head\nkeep me\n";
        let tail = "\n# Tail\nkeep me too\n";
        let original = format!("{head}{}{tail}", demo_block());
        let refreshed = format!("{head}{}{tail}", demo_block().replace("demo", "other"));
        let out = upsert(&original, &refreshed).expect("replace");
        assert!(out.contains("other"));
        assert!(!out.contains("demo"));
        assert!(out.starts_with(head) && out.ends_with(tail));
        assert_eq!(out.matches(START).count(), 1);
    }

    #[test]
    fn upsert_should_refuse_unbalanced_or_duplicate_markers() {
        let orphan_start = format!("# mine\n{START}\nhalf a block\n");
        let orphan_end = format!("# mine\nhalf a block\n{END}\n");
        let doubled = format!("{}\n{}\n", demo_block(), demo_block());
        let reversed = format!("{END}\n{}\n{START}\n", demo_block());
        for bad in [orphan_start, orphan_end, doubled, reversed] {
            assert!(upsert(&bad, &demo_block()).is_err(), "should have refused: {bad}");
        }
    }

    #[test]
    fn apply_to_should_create_then_unchanged() {
        let dir = std::env::temp_dir().join(format!("mw-guide-{}", std::process::id()));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("tmp");
        let t = &TARGETS[0];
        assert_eq!(apply_to(&dir, t, &demo_block()), Applied::Created);
        assert_eq!(apply_to(&dir, t, &demo_block()), Applied::Unchanged);
        // A .cursorrules with an orphan marker is refused and left byte-identical.
        let cursor = dir.join(".cursorrules");
        let broken = format!("user rules\n{START}\n");
        std::fs::write(&cursor, &broken).expect("write");
        let ct = TARGETS.iter().find(|t| t.rel == ".cursorrules").expect("target");
        assert!(matches!(apply_to(&dir, ct, &demo_block()), Applied::Refused(_)));
        assert_eq!(std::fs::read_to_string(&cursor).expect("read"), broken);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn curl_examples_should_name_all_three_ops() {
        let ex = curl_examples("http://127.0.0.1:8888", "demo");
        for op in ["retain", "recall", "reflect"] {
            assert!(ex.contains(&format!("/banks/demo/{op}")), "missing {op}");
        }
        assert!(ex.contains("\"content\"") && ex.contains("\"query\""));
    }
}
