//! Import a Hindsight document export into a memory-wire bank.
//!
//! Built for the 2026-09-30 migration off Hindsight, and kept because the shape it
//! handles — a competitor's corpus arriving as JSON, with real creation times that
//! the serving path cannot express — is not a one-off.
//!
//! # Why this is an `examples/` binary and not a flag on `memory-wire retain`
//!
//! Two reasons, both about honesty rather than effort.
//!
//! **Timestamps.** `memories.created_at` is stamped by the store on insert, and the
//! REST retain path has no way to say otherwise: a body field named `created_at` is
//! silently ignored, because the request struct has no such arm. So an import over
//! HTTP would collapse a seven-day corpus onto a single import instant, and nothing
//! in the resulting data would reveal it. `Store::put_doc` does take a `Memory` with
//! a real `created_at`, and that is what this uses.
//!
//! **Argument count.** Threading a timestamp through `retain_doc` would take it to
//! eight parameters against a clippy limit of seven — and `retain_doc` is *already*
//! at seven. Getting under the limit means either a struct refactor across 22 call
//! sites or an `#[allow]`, and this crate has zero `#[allow]` in the whole tree. A
//! migration tool is the right place for the capability; the serving API keeps the
//! shape it shipped with.
//!
//! # Chunking
//!
//! Hindsight stored whole sessions as single documents. The largest is 724 KB
//! carrying 2,042 derived memory units, and a memory that size is not retrievable —
//! `DEFAULT_RECALL_BUDGET` is 8,000 tokens, so recall would hand back one arbitrary
//! fragment of it. So a document over `--chunk-chars` is packed into consecutive
//! groups of whole turns, never cut mid-turn.
//!
//! Turn-level splitting would be too fine rather than too coarse: those 18 documents
//! hold 13,863 turn bodies between them, ~370 characters each, mostly assistant
//! mid-thought. Chunking by size at turn boundaries gives ~700 memories, where
//! per-turn splitting gives 14,315.
//!
//! Slicing is by byte offset, so every chunk is a byte-identical slice of the
//! source. Nothing is re-serialised, re-joined or normalised. The reported totals
//! are bytes, because that is what `str::len()` counts and what the size bound
//! applies to.
//!
//! # What is not carried
//!
//! - **Redaction is skipped.** The serving path runs `redact_pii` on retain; a store
//!   write does not. That is deliberate here — the source data was already accepted
//!   and stored by the other system, so re-running redaction at import time would
//!   silently rewrite the user's own history. It does mean imported rows are not
//!   redacted, which matters if the corpus ever held something it should not have.
//! - **Hindsight's derived units are not carried.** Each document reports
//!   `memory_unit_count` (18,175 across this corpus) — LLM-distilled facts extracted
//!   from the text. Only the original text is imported. The units are recoverable by
//!   re-deriving them; the text they were derived from is not in question.
//!
//! # Usage
//!
//! ```text
//! cargo run --release --example import_hindsight -- \
//!     --db ~/.local/share/memory-wire/memory.db --bank omp \
//!     ~/hindsight-export-20260930/omp.json
//! ```
//!
//! `--dry-run` reports the shape without writing. The import is idempotent: each
//! memory is written under `hindsight:<source id>` (or `hindsight:<id>#<n>` for a
//! chunk) in `UpdateMode::Replace`, so re-running replaces rather than duplicates.

use std::collections::BTreeSet;
use std::path::PathBuf;

use clap::Parser;
use memory_wire::memory::{Bank, Memory};
use memory_wire::store::{SqliteStore, Store, UpdateMode};
use regex::Regex;
use serde_json::Value;

/// A turn header, which is also a safe split point: Hindsight writes exactly
/// `[role: user]` or `[role: assistant]` at the start of a line.
fn turn_delim() -> Regex {
    Regex::new(r"(?m)^\[role: (?:user|assistant)\]\n").expect("turn delimiter is a valid regex")
}

/// First inline `[timestamp: ...]` in a slice, if any.
fn timestamp_of(block: &str) -> Option<String> {
    let re = Regex::new(r"\[timestamp: ([^\]]+)\]").expect("timestamp regex is valid");
    re.captures(block).and_then(|c| c.get(1)).map(|m| m.as_str().to_string())
}

/// Normalise a Hindsight timestamp to the RFC 3339 form `created_at` expects.
///
/// Hindsight emits `2026-09-22T20:26:23.270960+00:00` — a `+00:00` offset, and
/// fractional seconds of arbitrary precision. The store compares `created_at`
/// as bytes, so a mixed set would sort wrongly; trimming to milliseconds with a
/// `Z` suffix keeps every row in one comparable format.
fn normalise_timestamp(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    let (body, offset) = match trimmed.strip_suffix("+00:00") {
        Some(b) => (b, true),
        None => (trimmed.strip_suffix('Z').unwrap_or(trimmed), false),
    };
    let body = if offset {
        body
    } else {
        trimmed
    };
    // Cut the fraction to at most 3 digits, which is what RFC 3339 calls for.
    let (secs, frac) = match body.split_once('.') {
        Some((s, f)) => {
            let digits: String = f.chars().filter(char::is_ascii_digit).take(3).collect();
            (s, digits)
        }
        None => (body, String::new()),
    };
    if frac.is_empty() {
        Some(format!("{secs}Z"))
    } else {
        Some(format!("{secs}.{frac}Z"))
    }
}

/// One memory to write, before it becomes a `Memory`.
struct Chunk<'a> {
    text: &'a str,
    /// `[timestamp: ...]` of the first turn in this chunk.
    stamp: Option<String>,
}

/// Pack a document into size-bounded groups of whole turns.
///
/// Returns one chunk when the document is small enough, or when it has no turn
/// headers at all — a document with no delimiter is indivisible, and cutting it on
/// an arbitrary byte count would be worse than storing it whole.
fn chunk_document<'a>(text: &'a str, limit: usize) -> Vec<Chunk<'a>> {
    if text.len() <= limit {
        return vec![Chunk {
            text,
            stamp: timestamp_of(text),
        }];
    }
    let delim = turn_delim();
    let starts: Vec<usize> = delim.find_iter(text).map(|m| m.start()).collect();
    if starts.len() < 2 {
        return vec![Chunk {
            text,
            stamp: timestamp_of(text),
        }];
    }

    let mut blocks: Vec<&str> = Vec::with_capacity(starts.len());
    for w in starts.windows(2) {
        blocks.push(&text[w[0]..w[1]]);
    }
    blocks.push(&text[starts[starts.len() - 1]..]);

    let mut out: Vec<Chunk<'a>> = Vec::new();
    let mut cursor = 0usize;
    let mut acc = 0usize;
    let mut acc_stamp: Option<String> = None;
    for block in blocks {
        // A block that alone exceeds the limit is emitted on its own rather than
        // cut: a turn is the smallest unit here, and cutting inside one is the
        // mid-sentence split this whole design exists to avoid.
        if acc != 0 && acc + block.len() > limit {
            out.push(Chunk {
                text: &text[cursor..cursor + acc],
                stamp: acc_stamp.take(),
            });
            cursor += acc;
            acc = 0;
        }
        if acc == 0 {
            acc_stamp = timestamp_of(block);
        }
        acc += block.len();
    }
    if acc != 0 {
        out.push(Chunk {
            text: &text[cursor..cursor + acc],
            stamp: acc_stamp,
        });
    }
    out
}

#[derive(Parser)]
#[command(about = "Import a Hindsight document export into a memory-wire bank")]
struct Args {
    /// Path to the memory-wire SQLite database.
    #[arg(long)]
    db: PathBuf,
    /// Target bank id. Created if absent.
    #[arg(long)]
    bank: String,
    /// Maximum bytes per memory before a document is packed into turn groups.
    ///
    /// Bytes, not characters: the size bound is compared against `str::len()`,
    /// which is UTF-8 bytes. This corpus carries 44,342 bytes of multi-byte
    /// characters across 6,355,408 code points, so a byte bound is the stricter
    /// of the two and a chunk is never larger than it claims to be.
    #[arg(long, default_value_t = 24_000)]
    chunk_bytes: usize,
    /// Report the shape of the import without writing anything.
    #[arg(long, default_value_t = false)]
    dry_run: bool,
    /// Hindsight export JSON files: an array of document objects.
    files: Vec<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    if args.files.is_empty() {
        anyhow::bail!("no export files given; pass one or more Hindsight export .json paths");
    }

    let mut docs: Vec<Value> = Vec::new();
    for path in &args.files {
        let text = std::fs::read_to_string(path)
            .map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;
        let parsed: Value = serde_json::from_str(&text)
            .map_err(|e| anyhow::anyhow!("{}: {e}", path.display()))?;
        let items = parsed
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("{}: expected a JSON array of documents", path.display()))?;
        docs.extend(items.iter().cloned());
    }
    if docs.is_empty() {
        anyhow::bail!("no documents in the given files");
    }

    // Pass 1: shape, so a dry run reports the same numbers the write would produce.
    let mut planned = 0usize;
    let mut split_docs = 0usize;
    let mut largest = 0usize;
    let mut total_chars = 0usize;
    let mut tags: BTreeSet<String> = BTreeSet::new();
    let mut sessions: BTreeSet<String> = BTreeSet::new();
    let mut stamps: Vec<String> = Vec::new();
    for doc in &docs {
        let text = doc.get("original_text").and_then(Value::as_str).unwrap_or("");
        if text.trim().is_empty() {
            continue;
        }
        for t in doc.get("tags").and_then(Value::as_array).into_iter().flatten() {
            if let Some(s) = t.as_str() {
                tags.insert(s.to_string());
            }
        }
        if let Some(s) = doc
            .get("document_metadata")
            .and_then(|m| m.get("session_id"))
            .and_then(Value::as_str)
        {
            sessions.insert(s.to_string());
        }
        let chunks = chunk_document(text, args.chunk_bytes);
        if chunks.len() > 1 {
            split_docs += 1;
        }
        for c in &chunks {
            planned += 1;
            total_chars += c.text.len();
            largest = largest.max(c.text.len());
            let raw = c
                .stamp
                .clone()
                .or_else(|| doc.get("created_at").and_then(Value::as_str).map(str::to_string));
            if let Some(n) = raw.as_deref().and_then(normalise_timestamp) {
                stamps.push(n);
            }
        }
    }

    stamps.sort();
    println!("source documents : {}", docs.len());
    println!("memories planned : {planned}");
    println!(
        "  split at turns : {split_docs} documents (the rest import whole)"
    );
    println!("total bytes      : {total_chars}");
    println!("largest memory   : {largest} bytes");
    println!("distinct tags    : {}", tags.len());
    println!("distinct sessions: {}", sessions.len());
    match (stamps.first(), stamps.last()) {
        (Some(a), Some(b)) => println!("created_at range : {a} -> {b}"),
        _ => println!("created_at range : (none found)"),
    }
    if args.dry_run {
        println!("\ndry run: nothing written");
        return Ok(());
    }

    let store = SqliteStore::open(&args.db)?;
    store.put_bank(&Bank {
        id: args.bank.clone(),
        name: args.bank.clone(),
    })?;

    // Pass 2: write.
    let mut written = 0usize;
    let mut failed = 0usize;
    let mut failures: Vec<String> = Vec::new();
    for doc in &docs {
        let text = doc.get("original_text").and_then(Value::as_str).unwrap_or("");
        if text.trim().is_empty() {
            continue;
        }
        let source_id = doc.get("id").and_then(Value::as_str).unwrap_or("unknown");
        let doc_tags: Vec<String> = doc
            .get("tags")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        let session = doc
            .get("document_metadata")
            .and_then(|m| m.get("session_id"))
            .and_then(Value::as_str)
            .unwrap_or("-");

        let chunks = chunk_document(text, args.chunk_bytes);
        for (i, chunk) in chunks.iter().enumerate() {
            let document_id = if chunks.len() > 1 {
                format!("hindsight:{source_id}#{i}")
            } else {
                format!("hindsight:{source_id}")
            };
            let stamp = chunk
                .stamp
                .clone()
                .or_else(|| doc.get("created_at").and_then(Value::as_str).map(str::to_string))
                .as_deref()
                .and_then(normalise_timestamp);
            let context = format!(
                "imported from hindsight bank doc {source_id}, session {session}, part {}",
                i + 1
            );
            let memory = Memory {
                id: uuid::Uuid::new_v4().to_string(),
                bank_id: args.bank.clone(),
                content: chunk.text.to_string(),
                context: Some(context),
                created_at: stamp,
            };
            match store.put_doc(&memory, &doc_tags, Some(&document_id), UpdateMode::Replace) {
                Ok(_) => written += 1,
                Err(e) => {
                    failed += 1;
                    if failures.len() < 10 {
                        failures.push(format!("{source_id}#{i}: {e}"));
                    }
                }
            }
        }
    }

    println!("\nmemories written : {written}");
    println!("failures         : {failed}");
    for f in &failures {
        println!("  {f}");
    }
    let total = store.list(&args.bank)?.len();
    println!("bank now holds   : {total} memories");
    if written > 0 && failed > 0 {
        anyhow::bail!("{failed} of {} writes failed", written + failed);
    }
    Ok(())
}
