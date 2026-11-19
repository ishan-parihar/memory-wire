//! retain/recall/reflect API surface (Hindsight ops on agentmemory capture).
//!
//! - retain: redact content *and* context, persist via [`crate::store::Store`].
//! - recall: overlap-ranked retrieval + token-budget trim.
//! - reflect: top-hit synthesis with cited ids (LLM synthesis lands later).

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::{header::CONTENT_TYPE, HeaderName, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use uuid::Uuid;

use crate::capture::redact_pii;
use crate::memory::{Aspect, Bank, Lifecycle, Memory, MemoryMeta, Scope};
use crate::recall::{
    rank_candidates_scoped, recency_rank, rrf_fuse_with_magnitudes, trim_to_budget, FusionWeights,
    RankedHit,
};
pub use crate::store::BankStats;
use crate::store::{LexicalQuery, Store, StoreError, UpdateMode, CREATED_AT_FORMAT};

/// FTS5 candidates pulled per recall — the LIMIT is pushed into the SQL.
///
/// Public only so [`crate::vector`]'s parallel recall arm builds its streams
/// from the *same* numbers. Duplicating them there would be one more place for
/// the two arms to disagree, and a vector arm that silently fused a 200-row
/// overlap stream against a 50-row lexical one would be a bug with no test able
/// to see it.
pub const FTS_LIMIT: usize = 50;
/// Token-overlap candidates fused per recall.
///
/// Public for the same reason as [`FTS_LIMIT`].
pub const OVERLAP_LIMIT: usize = 200;
/// Hard ceiling on memories returned by one recall.
///
/// Public for the same reason as [`FTS_LIMIT`]. The vector arm honours it, so a
/// vector-fused recall returns the same shape of answer as a lexical one.
pub const MAX_RESULTS: usize = 100;
/// Recall budget used when neither the request nor the bank config sets one.
///
/// **8,000 tokens = 3 × the 2,626-token median session** measured in
/// `docs/CONSISTENCY.md` §14.9, at the store's own 4-chars-per-token rule.
///
/// Chosen on size arithmetic, not on a score. The trim is a hard ceiling, so a
/// budget below the size of a stored memory cannot return it: the top hit is cut
/// to the cap and everything below it is skipped. The previous 2,000 therefore
/// admitted *zero* median-size sessions whole and truncated 64.4% of them. Three
/// medians is the smallest multiple that admits several whole sessions rather than
/// one, so a conversation-shaped bank gets its second and third best hits back
/// instead of a truncated first. Every budget at or above 8,000 is identical on
/// §14.9's size table; the step that matters is crossing the median.
///
/// No value was compared on LongMemEval before choosing this one and none was
/// tried after. `eval/RESULTS.md` cannot move for this change at all: its harness
/// passes its own explicit 100,000-token budget (`examples/longmemeval.rs:235`).
///
/// Atomic-memory banks are unaffected in content — "auth uses jose" was always far
/// under 2,000 tokens, so the same hits come back. Only the ceiling on context per
/// recall moves; [`MAX_RESULTS`] (100) bounds the other end.
pub const DEFAULT_RECALL_BUDGET: usize = 8000;
/// Ceiling on tags stored per memory (request tags ∪ bank `retainTags`).
pub const MAX_TAGS: usize = 20;

/// The store's candidate window has to cover the whole overlap stream's budget —
/// a window shallower than the stream would silently starve it, which is the one
/// way the two constants can drift apart unnoticed.
const _: () = assert!(crate::store::RECALL_POOL_LIMIT >= OVERLAP_LIMIT);

/// Synthesis framing for [`MemoryService::reflect`].
///
/// A bank is a historian's record, not a change queue. Reflect reports what was
/// decided and why, in declarative past-tense prose — never the current
/// implementation state, because a bank still records a decision after the code
/// has moved past it, and an imperative would silently rewrite that history.
/// Literal tables, identifiers, and numbers are reproduced verbatim: a
/// paraphrased or rounded figure is a fabricated one.
pub const REFLECT_SYSTEM_PROMPT: &str = "\
You are a historian reading a memory bank's record of past decisions.

Report decisions and rationale, never the current implementation.

- Write declarative past-tense prose: what was decided, and on what reasoning.
- Never issue instructions or recommendations. Do not write \"you should\",
  \"remove\", \"update\", \"switch to\", \"add\", or any other imperative.
- Reproduce literal tables, identifiers, and numbers verbatim. Do not
  summarize, round, or paraphrase a recorded figure.
- Treat every entry as history. If a decision was later superseded, say it was
  superseded — do not restate it as what the code does now.
";

/// Errors from the service layer.
#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// Storage failure.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// Retained content was empty or whitespace only.
    #[error("invalid content")]
    InvalidContent,
    /// The bank is mounted read-only (W2): every write at all three surfaces
    /// is refused. Carries no detail beyond the refusal — the bank id is
    /// already in the URL the client called, and echoing it back adds nothing.
    #[error("bank is read-only")]
    ReadOnlyBank,
}

/// Classify a service error into a client-safe HTTP status and body.
///
/// Raw `rusqlite` text can echo SQL fragments and on-disk paths, so every
/// storage failure collapses to a generic 500. Bank-id, config, and content
/// validation are the exceptions: they are client input errors (400), and naming
/// them is what makes the 400 actionable. So is a repeated document id (409):
/// a well-formed request conflicting with the current state of a resource, which
/// 500 would misclassify as a server fault. A well-formed request against a bank
/// that was never created is 404, not 500 — nothing failed, the target is simply
/// absent.
///
/// Every arm above the `_` is a client mistake the caller can act on. The `_` is
/// therefore *exactly* the storage-failure path, and it is the one place the
/// server records that a request failed: both surfaces classify through here —
/// the HTTP handler via `to_http`, MCP via `From<ApiError>` — so one
/// `tracing::error!` covers both without either of them logging separately. What
/// it records is the store error, which is driver text and on-disk paths; what it
/// does *not* record is the request, so a retained memory never reaches the log.
pub fn http_error(e: &ApiError) -> (StatusCode, &'static str) {
    match e {
        ApiError::Store(StoreError::InvalidBank) => {
            (StatusCode::BAD_REQUEST, "invalid bank id")
        }
        ApiError::Store(StoreError::InvalidConfig) => {
            (StatusCode::BAD_REQUEST, "invalid bank config")
        }
        ApiError::Store(StoreError::UnknownBank) => (StatusCode::NOT_FOUND, "unknown bank"),
        ApiError::Store(StoreError::DocumentConflict) => (
            StatusCode::CONFLICT,
            "document already exists; use update_mode=replace",
        ),
        ApiError::InvalidContent => (StatusCode::BAD_REQUEST, "invalid content"),
        // W2: a read-only bank refuses writes at all three surfaces. The
        // frozen HTTP error contract maps it to the same 400 shape
        // `InvalidContent` already uses, so a client that already handles a
        // refused retain needs no new code.
        ApiError::ReadOnlyBank => (StatusCode::BAD_REQUEST, "bank is read-only"),
        _ => {
            // Opaque to the client, loud on the server. Without this, disk-full,
            // a corrupt page, a locked database, a poisoned lock and a panicking
            // blocking task are one indistinguishable `500 storage error` with
            // nothing anywhere saying which.
            tracing::error!(error = %e, "memory-wire: storage failure");
            (StatusCode::INTERNAL_SERVER_ERROR, "storage error")
        }
    }
}

/// Map a service error to a client-safe `(status, message)` response body.
pub fn to_http(e: ApiError) -> (StatusCode, String) {
    let (status, msg) = http_error(&e);
    (status, msg.to_string())
}

/// A client's `update_mode`, parsed once at the boundary.
///
/// Absent is [`UpdateMode::Replace`], the two documented values are taken as
/// themselves, and anything else is the frozen table's existing bad-input 400.
/// Refusing rather than defaulting is the whole point: `replace` deletes the
/// document's prior revision, so a value that fell through to it would destroy a
/// revision over a typo the caller never knew they made. The refusal reuses the
/// existing 400 rather than adding a status or a message, because the error
/// table callers are told is fixed must stay fixed.
pub fn parse_update_mode(raw: Option<&str>) -> Result<UpdateMode, ApiError> {
    UpdateMode::parse(raw).ok_or(ApiError::InvalidContent)
}

/// A client's `created_at` — when the memory *happened* — parsed once at the
/// boundary into the store's own [`CREATED_AT_FORMAT`].
///
/// Absent or blank is `None` (the store stamps "now"). An RFC 3339 time with any
/// offset is converted to UTC, and a bare `YYYY-MM-DD` is that day's midnight
/// UTC. Anything else is the frozen table's existing bad-input 400, never a
/// stored string: the retention sweep compares stamps as bytes, and a
/// malformed one would sort as if it were newer than every real memory.
pub fn parse_created_at(raw: Option<&str>) -> Result<Option<String>, ApiError> {
    let Some(raw) = raw.map(str::trim).filter(|r| !r.is_empty()) else {
        return Ok(None);
    };
    let utc = match chrono::DateTime::parse_from_rfc3339(raw) {
        Ok(t) => t.with_timezone(&chrono::Utc),
        Err(_) => chrono::NaiveDate::parse_from_str(raw, "%Y-%m-%d")
            .map_err(|_| ApiError::InvalidContent)?
            .and_time(chrono::NaiveTime::MIN)
            .and_utc(),
    };
    Ok(Some(utc.format(CREATED_AT_FORMAT).to_string()))
}

/// One recall hit in the `format: "full"` citing shape, shared by the HTTP and
/// MCP adapters so the two cannot drift: `{id, score, created_at, content}`. The
/// `trust` tier rides along because the auto-injection path is the one caller
/// that must be able to refuse a row: a memory ingested from a web page lands as
/// `untrusted_external`, and injecting it verbatim into a system prompt is the
/// prompt-injection hole W2's write-time ceiling cannot close — the ceiling only
/// bounds what a write may *claim*, so the row still lands and still ranks.
/// `trust` is therefore on the wire, not just in the row: a hook that cannot see
/// it cannot honour it.
pub fn full_hit_json(hit: &ScoredMemory) -> serde_json::Value {
    serde_json::json!({
        "id": hit.memory.id,
        "score": hit.score,
        "created_at": hit.memory.created_at,
        "content": hit.memory.content,
        "trust": hit.memory.meta.trust.0
    })
}

/// One hit under `format: "summary"` (H8): the citing fields of [`full_hit_json`]
/// with `content` replaced by its [`summary_content`] capsule — a reader
/// scanning k hits reads k capsules, and every hidden remainder is one GET
/// away by the id the same object serves.
pub fn summary_hit_json(hit: &ScoredMemory, bank: &str) -> serde_json::Value {
    serde_json::json!({
        "id": hit.memory.id,
        "score": hit.score,
        "created_at": hit.memory.created_at,
        "content": summary_content(&hit.memory, bank),
        "trust": hit.memory.meta.trust.0
    })
}

/// Normalize a client tag list: trim, lowercase, drop empties, dedupe, cap at
/// [`MAX_TAGS`].
///
/// Order is preserved (and the cap takes the head), so a caller's tag ordering
/// survives — the list is [`Store::put_tagged`]'s whole truth, not a set to
/// reshuffle.
pub fn normalize_tags(tags: &[String]) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for tag in tags {
        let tag = tag.trim().to_lowercase();
        if tag.is_empty() || out.contains(&tag) {
            continue;
        }
        out.push(tag);
        if out.len() == MAX_TAGS {
            break;
        }
    }
    out
}

/// The part of a bank's JSON config the service acts on.
///
/// Other keys (`retain_mission`, `recallPromptPreamble`, anything a future
/// version adds) are stored and served verbatim but carry no behavior here.
#[derive(Debug, Default)]
struct BankConfig {
    /// `recallMaxTokens` — the bank's default recall budget.
    recall_max_tokens: Option<usize>,
    /// `retainTags` — added to every retain in this bank.
    retain_tags: Vec<String>,
    /// `recallSynonyms` — user-supplied paraphrase terms, keyed by the term the
    /// user's query must actually contain. Empty when the key is absent, which is
    /// the shipped default and means no expansion at all.
    ///
    /// Keyed, not a flat list, because the only safe moment to expand a query is
    /// after reading it: a list could be ORed in unconditionally and would then
    /// apply to every query in the bank regardless of what was asked. Keying on a
    /// trigger term means a bank that says `"migrate"` is a *rule*, not a tax on
    /// recall, and a query that says nothing about migrating is unaffected.
    ///
    /// Empty by default and not merely by convention: [`BankConfig::parse`]
    /// produces an empty table for a config that omits the key, a config that
    /// spells it wrong, and a config that is not an object at all.
    recall_synonyms: SynonymTable,
    /// `readOnly` (W2) — refuse every write to this bank at all three surfaces
    /// (HTTP, MCP, hook). Defaults to `false` so a bank that does not opt in
    /// keeps the shipped write path. A bank that wants to swallow a write
    /// entirely (an untrusted mount ingesting web content it must never read
    /// back as trusted rules) sets this rather than relying on a trust ceiling:
    /// the ceiling downgrades, which still lands a row.
    read_only: bool,
    /// `maxTrust` (W2) — the highest trust tier a write may carry verbatim.
    /// `None` (the default) means no ceiling: every trust accepted. A write
    /// above the ceiling is downgraded one tier and lands; see [`Trust`].
    max_trust: Option<Trust>,
}

/// `recallSynonyms` as parsed: lowercased trigger term to its entry.
type SynonymTable = HashMap<String, RecallSynonyms>;

/// The empty synonym table, shared by every call site that reads no config.
///
/// A `const`-adjacent shared value rather than a fresh `HashMap::new()` per call so
/// the shipped `recall` path — which is the sweep entry point and must not change
/// cost — provably allocates nothing for a feature it has not enabled. `Sync` is
/// what makes this sound: it is never mutated, and [`expand_synonyms`] takes
/// `&`.
static EMPTY_SYNONYMS: std::sync::LazyLock<SynonymTable> =
    std::sync::LazyLock::new(HashMap::new);

/// One `recallSynonyms` entry: the terms to add, and how much each is worth.
///
/// Flat `{"terms": [...], "weight": 0.7}` rather than the per-term-object shape.
/// Per-term weights were the alternative and they are the wrong shape here for
/// two reasons: FTS5 has no per-term weight, so expressing one would mean a
/// separate index scan per distinct weight for a table that is a hand-written
/// convenience, and a table where most entries are a bare word list would spend
/// most of its bytes on `{"terms":` and `}`.
#[derive(Debug, Clone, PartialEq)]
struct RecallSynonyms {
    /// Terms to add to the query when the key matches.
    terms: Vec<String>,
    /// How much each added term counts against a full-weight one. `1.0` is a
    /// full term. Clamped to `(0, 1]` on read: a synonym that counts *more* than
    /// the term the user typed inverts the ranking in favour of the paraphrase,
    /// which is never what "a synonym" means.
    weight: f64,
}

/// A ceiling on the trust a write may carry.
///
/// Ordered from least to most trusted. A bank's `maxTrust` names the highest
/// tier the bank is willing to store verbatim; a write that exceeds it is
/// **downgraded** (never refused) to the next tier down. Refusing would break
/// the never-fail hook path — a hook that cannot write its capture must exit 0
/// silently, not put an error in front of a model — so a bank that wants to
/// swallow a write entirely sets `readOnly` instead.
///
/// The tiers are free text in the config, matched case-insensitively; an
/// unrecognised value reads as "no ceiling" (every trust accepted), which is
/// the same fail-open default `readOnly` and every other `BankConfig` key has:
/// a typo in a hand-written config must not take writes down.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Trust {
    /// Read from an external source this build has no reason to trust: a web
    /// page, a PR comment, an email body. Below `AgentInferred` so a ceiling
    /// can be set *above* it and still exclude it, which is what makes
    /// `maxTrust: "agent_inferred"` mean "no untrusted content lands here".
    ///
    /// The auto-injection gate in `hooks.rs` refuses this tier outright: an
    /// explicit `memory_recall` still returns the row, because a reader who
    /// asked is not the same as a turn that would have been injected.
    UntrustedExternal,
    /// Inferred by an agent with no further verification.
    AgentInferred,
    /// Stated by a named person in an authorised channel.
    HumanStated,
    /// Confirmed by a tool, file, or commit this build can name.
    Verified,
}

impl Trust {
    /// Parse a free-text tier. Unknown values read as "no ceiling", which the
    /// caller expresses as `None`.
    pub fn tier(raw: &str) -> Option<Trust> {
        match raw.to_ascii_lowercase().as_str() {
            "untrusted_external" | "untrusted-external" | "untrusted" => {
                Some(Trust::UntrustedExternal)
            }
            "agent_inferred" | "agent-inferred" | "agent_inferred_only" => Some(Trust::AgentInferred),
            "human_stated" | "human-stated" | "human" => Some(Trust::HumanStated),
            "verified" => Some(Trust::Verified),
            _ => None,
        }
    }

    /// One tier down from `self`, or `self` when already at the floor. A
    /// downgrade at the floor is a no-op: there is nothing below
    /// `UntrustedExternal` to fall to, and the write must still land.
    fn downgrade(self) -> Trust {
        match self {
            Trust::Verified => Trust::HumanStated,
            Trust::HumanStated => Trust::AgentInferred,
            Trust::AgentInferred => Trust::UntrustedExternal,
            Trust::UntrustedExternal => Trust::UntrustedExternal,
        }
    }

    /// The canonical string form stored in `memories.trust` (W1 column).
    ///
    /// The strings match what [`Trust::tier`] accepts, so a round-trip is
    /// byte-stable: parse -> store -> read -> parse returns the same variant.
    pub fn as_str(self) -> &'static str {
        match self {
            Trust::UntrustedExternal => "untrusted_external",
            Trust::AgentInferred => "agent_inferred",
            Trust::HumanStated => "human_stated",
            Trust::Verified => "verified",
        }
    }
}

impl BankConfig {
    /// Read the keys this build understands, treating everything else as absent.
    ///
    /// A malformed value falls back to the default instead of failing: config is
    /// written by hand over HTTP, and a typo in one key must not take recall
    /// down with it. The raw value is still served by `GET .../config`, so the
    /// typo stays visible.
    fn parse(raw: &str) -> Self {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(raw) else {
            return Self::default();
        };
        let retain_tags = value
            .get("retainTags")
            .and_then(|v| v.as_array())
            .map(|arr| {
                normalize_tags(
                    &arr.iter()
                        .filter_map(|t| t.as_str().map(String::from))
                        .collect::<Vec<_>>(),
                )
            })
            .unwrap_or_default();
        Self {
            recall_max_tokens: value
                .get("recallMaxTokens")
                .and_then(|n| n.as_u64())
                .and_then(|n| usize::try_from(n).ok()),
            retain_tags,
            recall_synonyms: parse_recall_synonyms(&value),
            // W2. A malformed value falls back to the default (false / None) for
            // the same reason every other key does: a typo in a hand-written
            // config must not take writes down with it. The raw value is still
            // served by `GET .../config`, so the typo stays visible.
            read_only: value
                .get("readOnly")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            max_trust: value
                .get("maxTrust")
                .and_then(|v| v.as_str())
                .and_then(Trust::tier),
        }
    }
}

/// Read `recallSynonyms`, discarding every entry that is not well formed.
///
/// Drops rather than errors for the same reason [`BankConfig::parse`] does on
/// every other key: a typo in a hand-written config must not take recall down.
/// The raw JSON is still served by `GET .../config`, so the typo stays visible.
///
/// The clamps are the interesting part. `weight` outside `(0, 1]` is coerced
/// rather than dropped, because a negative weight would make a synonym
/// *subtract* from a document's score and a weight above 1.0 would let a
/// paraphrase outrank the term the user actually typed — both are a config that
/// says something no reader meant, and both are safer to bound than to honour.
fn parse_recall_synonyms(value: &serde_json::Value) -> HashMap<String, RecallSynonyms> {
    let Some(obj) = value.get("recallSynonyms").and_then(|v| v.as_object()) else {
        return HashMap::new();
    };
    let mut out = HashMap::new();
    for (key, entry) in obj {
        // Keys are matched case-insensitively against the lowercased query token,
        // so the stored key is lowercased to make that a hash lookup. Without
        // this, a config written `"Migrate"` would silently never fire.
        let key = key.to_lowercase();
        let terms: Vec<String> = entry
            .get("terms")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|t| t.as_str()).map(String::from).collect())
            .unwrap_or_default();
        if terms.is_empty() {
            continue;
        }
        let weight = entry
            .get("weight")
            .and_then(|w| w.as_f64())
            .unwrap_or(DEFAULT_SYNONYM_WEIGHT)
            .clamp(f64::MIN_POSITIVE, 1.0);
        out.insert(key, RecallSynonyms { terms, weight });
    }
    out
}

/// Weight a synonym gets when its entry omits one.
///
/// 0.7, matching agentmemory's `search-index.ts:105-110` single constant. Copied
/// as a *shape* — a synonym below full weight — and not as a selected value:
/// this build has no table to apply it to, so there is nothing here for 0.7 to
/// have been fitted to, and a bank that wants a different ratio sets it per entry.
const DEFAULT_SYNONYM_WEIGHT: f64 = 0.7;

/// The `(term, weight)` pairs one query expands to, deduplicated and lowercased.
///
/// Returns empty for an empty table, which is the case for every bank that has
/// not configured `recallSynonyms` — so the common path allocates nothing and
/// the store's synonym branch is skipped entirely.
///
/// The query is matched with the *same* tokenization the store uses for the
/// `MATCH` string, rather than a substring test. A substring test would fire on
/// `"migrate"` inside `"unmigrated"`, and it would not fire on a trigger term
/// split across a hyphen the tokenizer treats as a separator. Matching tokens is
/// the only version that agrees with the query that will actually be issued.
fn expand_synonyms(table: &SynonymTable, query: &str) -> Vec<(String, f64)> {
    if table.is_empty() {
        return Vec::new();
    }
    let tokens: Vec<String> = query
        .split(|c: char| !c.is_alphanumeric())
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .map(str::to_lowercase)
        .collect();
    let mut out: Vec<(String, f64)> = Vec::new();
    for token in &tokens {
        let Some(entry) = table.get(token) else {
            continue;
        };
        for term in &entry.terms {
            let term = term.to_lowercase();
            if term == *token || out.iter().any(|(t, _)| *t == term) {
                // A synonym equal to its own trigger adds nothing, and emitting it
                // would double-count that term's BM25 — the same effect
                // `KeywordScope` documents for an exact term ORed with its prefix.
                continue;
            }
            out.push((term, entry.weight));
        }
    }
    out
}

/// A recalled memory with its fused rank score.
#[derive(Debug, Clone)]
pub struct ScoredMemory {
    /// The recalled memory.
    pub memory: Memory,
    /// Fused RRF score, `Σ 1/(k + rank)` over the streams the memory ranked in
    /// (higher is better). Not a raw relevance count: the two streams are on
    /// incomparable scales, so the only honest way to say "how highly was this
    /// ranked" is the value the ranking actually used.
    pub score: f64,
}

/// Core service: retains and recalls bank-isolated memories.
#[derive(Clone)]
pub struct MemoryService<S: Store> {
    /// Backing store (SQLite now, Postgres via same trait later).
    pub store: Arc<S>,
}

/// The two halves of tag filtering as one parameter, so the recall funnel
/// keeps a fixed arity: `include` narrows to rows carrying any of them,
/// `exclude` drops rows carrying any of them before ranking.
/// The dimensional filter one recall narrows the candidate pool by.
///
/// `include_history` is the only flag that widens rather than narrows: the
/// default read path hides `superseded`/`quarantined` rows entirely (see
/// [`DimensionalScope::active`]), and passing it opts back in so the lineage of
/// a superseded memory stays readable. `lifecycle` narrows further, on top of
/// that default. Every filter is applied in SQL before ranking — a filter that
/// narrowed after fusion would still pay for the ranking work.
#[derive(Debug, Clone, Default)]
pub struct DimensionalScope<'a> {
    /// Allowed `scope` values; empty keeps them all.
    pub scopes: &'a [String],
    /// Allowed `aspect` values; empty keeps them all.
    pub aspects: &'a [String],
    /// Allowed `lifecycle` values; empty keeps them all (minus the default
    /// hide of `superseded`/`quarantined`).
    pub lifecycles: &'a [String],
    /// Serve superseded/quarantined rows too.
    pub include_history: bool,
}

impl DimensionalScope<'_> {
    /// The default scope: no filter, hide superseded/quarantined.
    pub fn active() -> DimensionalScope<'static> {
        DimensionalScope::default()
    }
}

#[derive(Default)]
struct TagScope<'a> {
    include: &'a [String],
    exclude: &'a [String],
    /// Optional dimensional filter; `None` keeps the shape byte-identical to
    /// the pre-W1 recall (no SQL, no filters, no row removal).
    dimensions: Option<DimensionalScope<'a>>,
}

impl<'a> TagScope<'a> {
    fn new(include: &'a [String], exclude: &'a [String]) -> Self {
        Self {
            include,
            exclude,
            dimensions: None,
        }
    }
}


impl<S: Store> MemoryService<S> {
    /// Create a service over the given store.
    pub fn new(store: S) -> Self {
        Self {
            store: Arc::new(store),
        }
    }

    /// Reject a blank bank id, before any store call.
    ///
    /// One guard for every entry point, so a blank bank fails identically
    /// wherever it arrives instead of each method re-deciding what it means.
    fn require_bank(bank_id: &str) -> Result<(), ApiError> {
        if bank_id.trim().is_empty() {
            return Err(ApiError::Store(StoreError::InvalidBank));
        }
        Ok(())
    }

    /// Retain redacted content under `bank_id`; returns the new memory id.
    ///
    /// Both `content` and `context` are redacted before they touch storage:
    /// capture context carries hook payloads and is just as leaky as content.
    ///
    /// The write is stored at `Trust::AgentInferred`, the floor of the trust
    /// ladder. A caller that has actually verified a fact (a tool read it, a
    /// commit names it) uses [`Self::retain_doc_at_trusted`] to declare a
    /// higher tier; a bank with a `maxTrust` ceiling below that tier will
    /// downgrade the write one step. See [`Trust`].
    pub fn retain(
        &self,
        bank_id: &str,
        content: &str,
        context: Option<String>,
    ) -> Result<String, ApiError> {
        self.retain_tagged(bank_id, content, context, &[])
    }

    /// Retain with tags: the caller's tags plus the bank's `retainTags`, both
    /// normalized into one capped set stored with the memory.
    ///
    /// Request tags come first, so a bank-wide default can never displace a tag
    /// the caller named explicitly.
    pub fn retain_tagged(
        &self,
        bank_id: &str,
        content: &str,
        context: Option<String>,
        tags: &[String],
    ) -> Result<String, ApiError> {
        self.retain_doc(bank_id, content, context, tags, None, UpdateMode::Replace)
    }

    /// Retain a document-scoped memory under an optional `document_id`.
    ///
    /// `document_id` is the caller's stable key for "this is the same document".
    /// `update_mode` is [`UpdateMode::Append`] to keep every earlier row under
    /// that key (each retain is then a separate memory), or
    /// [`UpdateMode::Replace`] — the default — which drops the prior row first,
    /// so the document has exactly one current revision and the tags written here
    /// are the whole truth. A blank `document_id` is no document at all, so
    /// nothing is superseded.
    ///
    /// Appending under a `document_id` that already holds a row in this bank is
    /// [`StoreError::DocumentConflict`] (409): a document has one current
    /// revision, and asking to *add* to it is neither replacing it nor naming a
    /// different document.
    ///
    /// The bank is created if it does not exist, which is what makes a library
    /// retain mean the same thing the HTTP and MCP handlers already make it mean
    /// (both declare the bank before writing). The returned id is the row that
    /// ends up holding the content: a write deduplicated against a byte-identical
    /// memory already in this bank returns that memory's id instead of a fresh one.
    ///
    /// The row and its tags are written in one transaction: a reader must never
    /// see a new revision carrying the previous one's tag set.
    pub fn retain_doc(
        &self,
        bank_id: &str,
        content: &str,
        context: Option<String>,
        tags: &[String],
        document_id: Option<&str>,
        update_mode: UpdateMode,
    ) -> Result<String, ApiError> {
        self.retain_doc_at(bank_id, content, context, tags, document_id, update_mode, None)
    }

    /// [`Self::retain_doc`] with a caller-stated occurrence time.
    ///
    /// `created_at` is a [`parse_created_at`] result: `None` lets the store stamp
    /// "now", `Some` dates the memory to when it happened — an imported
    /// conversation's own session date, which is what a reader needs to answer
    /// "when". It is the same column a bank's `ttl_days` sweep reads, so a
    /// backdated memory is as old as it says it is. A write deduplicated against
    /// an existing row keeps that row's time.
    #[allow(clippy::too_many_arguments)] // one more optional field on the same write
    pub fn retain_doc_at(
        &self,
        bank_id: &str,
        content: &str,
        context: Option<String>,
        tags: &[String],
        document_id: Option<&str>,
        update_mode: UpdateMode,
        created_at: Option<String>,
    ) -> Result<String, ApiError> {
        self.retain_doc_at_trusted(
            bank_id,
            content,
            context,
            tags,
            document_id,
            update_mode,
            created_at,
            Trust::AgentInferred,
        )
    }

    /// [`Self::retain_doc_at_trusted`] with explicit dimensional metadata (W1).
    ///
    /// The six W1 fields — `scope`, `aspect`, `lifecycle`, `trust`,
    /// `parent_id`, `superseded_by` and `scope_globs` — were writable only from
    /// Rust: no HTTP, MCP or CLI caller could set any of them, so every row in
    /// every real store carried the defaults (`local`/`episodic`). The filters
    /// that read those columns therefore had nothing to filter, and `grounding`
    /// correctly returned nothing because nothing could be eligible.
    ///
    /// `meta.trust` is still the caller's *claim*: the bank's `maxTrust` ceiling
    /// decides what is stored, exactly as in `retain_doc_at_trusted`. Every other
    /// field is stored verbatim, because they are descriptive and a descriptive
    /// field the store second-guessed would make a write non-reproducible.
    ///
    /// `lifecycle` is the caller's too: a migration importing an archive needs to
    /// write `superseded` or `quarantined` directly rather than through a second
    /// call.
    #[allow(clippy::too_many_arguments)] // one optional metadata bundle, not seven params
    pub fn retain_doc_at_full(
        &self,
        bank_id: &str,
        content: &str,
        context: Option<String>,
        tags: &[String],
        document_id: Option<&str>,
        update_mode: UpdateMode,
        created_at: Option<String>,
        meta: crate::memory::MemoryMeta,
    ) -> Result<String, ApiError> {
        Self::require_bank(bank_id)?;
        if content.trim().is_empty() {
            return Err(ApiError::InvalidContent);
        }
        let cfg = self.config_of(bank_id)?;
        if cfg.read_only {
            return Err(ApiError::ReadOnlyBank);
        }
        // The caller's `trust` is free text on the wire; parse it into the typed
        // tier the ceiling is defined against. An unrecognised value reads as
        // `AgentInferred`, the same fail-open rule `Trust::tier` documents for a
        // typo'd `maxTrust` — a value this build does not understand must not
        // take a write down, and the floor tier is the safe reading.
        let claimed = Trust::tier(&meta.trust.0).unwrap_or(Trust::AgentInferred);
        let stored_trust = match cfg.max_trust {
            Some(ceiling) if claimed > ceiling => claimed.downgrade(),
            _ => claimed,
        };
        self.store.put_bank(&Bank {
            id: bank_id.to_string(),
            name: bank_id.to_string(),
        })?;
        let document_id = document_id.map(str::trim).filter(|d| !d.is_empty());
        let mut all = tags.to_vec();
        all.extend(cfg.retain_tags);
        // Everything the caller set, with only `trust` replaced by what the
        // ceiling accepted. A descriptive field the store rewrote would make two
        // identical requests store different rows.
        let meta = crate::memory::MemoryMeta {
            trust: crate::memory::Trust(stored_trust.as_str().to_string()),
            ..meta
        };
        let memory = Memory {
            id: Uuid::new_v4().to_string(),
            bank_id: bank_id.to_string(),
            content: redact_pii(content),
            context: context.as_deref().map(redact_pii),
            created_at,
            meta,
        };
        // Same call as `retain_doc_at_trusted`, so dedup-on-content-hash and the
        // version write behave identically here. The returned id is the row that
        // now holds the content, which is this memory's own id unless the write
        // deduplicated onto an existing row.
        self.store
            .put_doc(&memory, &normalize_tags(&all), document_id, update_mode)
            .map_err(ApiError::from)
    }

    /// [`Self::retain_doc_at`] with an explicit trust tier (W2).
    ///
    /// The tier is the caller's *claim* about how much the row has been
    /// checked. The bank's `maxTrust` ceiling decides what is actually stored:
    /// a tier above the ceiling is downgraded one step and lands, never
    /// refused — a refusal would break the never-fail hook path. A bank with
    /// no ceiling (the default) stores the tier verbatim.
    #[allow(clippy::too_many_arguments)] // one more optional field on the same write
    pub fn retain_doc_at_trusted(
        &self,
        bank_id: &str,
        content: &str,
        context: Option<String>,
        tags: &[String],
        document_id: Option<&str>,
        update_mode: UpdateMode,
        created_at: Option<String>,
        trust: Trust,
    ) -> Result<String, ApiError> {
        Self::require_bank(bank_id)?;
        if content.trim().is_empty() {
            // A memory of nothing matches no query and cites nothing, so it is
            // dead weight forever. Rejecting it names the client's actual
            // mistake instead of storing a row that can never be retrieved.
            return Err(ApiError::InvalidContent);
        }
        // W2 mount profiles: the bank's config is read before any write so
        // `readOnly` and `maxTrust` are honoured at every surface. A bank that
        // does not exist yet has no config, which reads as the defaults
        // (`read_only: false`, no ceiling) — the shipped behaviour.
        let cfg = self.config_of(bank_id)?;
        if cfg.read_only {
            // A read-only bank refuses every write at all three surfaces. The
            // frozen HTTP error contract carries this as 400 invalid content —
            // the same shape `InvalidContent` already uses — so a client that
            // already handles a refused retain needs no new code. The hook path
            // never sees the refusal (it swallows errors and exits 0); the HTTP
            // and MCP surfaces surface it as a clean 4xx the caller can act on.
            return Err(ApiError::ReadOnlyBank);
        }
        // W2 trust ceiling: a tier above the bank's `maxTrust` is downgraded
        // one step and lands. Refusing would break the never-fail hook path,
        // which must exit 0 silently — a bank that wants to swallow a write
        // entirely sets `readOnly` instead. With no ceiling (the default) the
        // tier is stored verbatim.
        let stored_trust = match cfg.max_trust {
            Some(ceiling) if trust > ceiling => trust.downgrade(),
            _ => trust,
        };
        // Insert-if-missing, so a re-declared bank keeps its stored name and
        // config and only a bank that was never written to is created. Done here
        // rather than left to the foreign key, which would turn a first retain
        // into a 500 for every library caller.
        self.store.put_bank(&Bank {
            id: bank_id.to_string(),
            name: bank_id.to_string(),
        })?;
        let document_id = document_id.map(str::trim).filter(|d| !d.is_empty());
        let mut all = tags.to_vec();
        all.extend(cfg.retain_tags);
        // W1 schema + W2 ceiling: the stored meta carries the tier the bank
        // actually accepted (possibly downgraded), so a reader can tell a
        // human-stated fact from an agent's inference even after the fact that
        // the caller asked for a higher tier was lost.
        let meta = crate::memory::MemoryMeta {
            trust: crate::memory::Trust(stored_trust.as_str().to_string()),
            ..crate::memory::MemoryMeta::default()
        };
        let memory = Memory {
            id: Uuid::new_v4().to_string(),
            bank_id: bank_id.to_string(),
            content: redact_pii(content),
            context: context.as_deref().map(redact_pii),
            // `None`: the store stamps it on insert. `Some`: already normalized
            // by `parse_created_at`, so the stored stamp is byte-comparable.
            created_at,
            meta,
        };
        // The store returns the id of the row that now holds the content, which is
        // this memory's own id unless the write was deduplicated.
        Ok(self
            .store
            .put_doc(&memory, &normalize_tags(&all), document_id, update_mode)?)
    }

    /// The bank's stored config JSON, or `{}` when it has none.
    ///
    /// 404s on an unknown bank, unlike the read path inside recall: a config
    /// fetch names its target, so a typo must not read back as an empty config.
    pub fn bank_config(&self, bank_id: &str) -> Result<String, ApiError> {
        Self::require_bank(bank_id)?;
        self.store
            .get_bank_config(bank_id)?
            .ok_or(ApiError::Store(StoreError::UnknownBank))
    }

    /// Replace a bank's whole config. Anything that is not a JSON object is
    /// rejected (400); an unknown bank is rejected (404), never created.
    pub fn set_bank_config(&self, bank_id: &str, config: &str) -> Result<(), ApiError> {
        Self::require_bank(bank_id)?;
        self.store.set_bank_config(bank_id, config)?;
        Ok(())
    }

    /// Config for the read paths, where an absent bank is simply "no settings".
    fn config_of(&self, bank_id: &str) -> Result<BankConfig, ApiError> {
        let raw = self
            .store
            .get_bank_config(bank_id)?
            .unwrap_or_else(|| "{}".to_string());
        Ok(BankConfig::parse(&raw))
    }

    /// Recall with an optional tag filter and an optional budget override.
    ///
    /// Budget precedence: explicit `budget` > the bank's `recallMaxTokens` >
    /// [`DEFAULT_RECALL_BUDGET`].
    ///
    /// The config read is propagated, not swallowed, on the branch where the
    /// budget comes from it. It used to be `.ok().and_then(…)`, which turned a
    /// storage fault into a *successful* recall trimmed to a budget the bank
    /// never asked for — a 200 carrying an answer the caller had no way to know
    /// was wrong. `retain_doc` already propagates the same call; this was an
    /// oversight, not a policy.
    ///
    /// The config is read on the explicit-budget branch too, for
    /// `recallSynonyms` only, and a fault there is non-fatal: the budget is the
    /// caller's there, so no wrong budget can be served.
    /// Recall top memories for `query` within `budget_tokens`, restricted to
    /// memories carrying any of `tags` (empty = the whole bank) and with every
    /// memory carrying any of `exclude` dropped from the candidate set before
    /// ranking. An `exclude` that normalizes to nothing excludes nothing —
    /// unlike `tags` it cannot fail closed, because the safe reading of a
    /// nonsense exclusion is "hide no rows", not "answer with an empty set".
    pub fn recall_filtered(
        &self,
        bank_id: &str,
        query: &str,
        budget: Option<usize>,
        tags: &[String],
        exclude: &[String],
    ) -> Result<Vec<ScoredMemory>, ApiError> {
        self.recall_filtered_dimensional(bank_id, query, budget, tags, exclude, None)
    }

    /// Recall narrowed by the W1 dimensions, in SQL, before ranking.
    ///
    /// Passing `None` for `dimensions` is byte-identical to [`Self::recall_filtered`]:
    /// no extra SQL clause is emitted and no row is removed, so the shipped
    /// recall path pays nothing for the capability it does not use.
    pub fn recall_filtered_dimensional(
        &self,
        bank_id: &str,
        query: &str,
        budget: Option<usize>,
        tags: &[String],
        exclude: &[String],
        dimensions: Option<DimensionalScope<'_>>,
    ) -> Result<Vec<ScoredMemory>, ApiError> {
        Self::require_bank(bank_id)?;
        // Both branches read the config, because `recallSynonyms` is independent
        // of the budget: a caller who passes an explicit `budget` named the token
        // cap, not the ranking, and HTTP and MCP *both* always pass a budget, so
        // reading the config only when the budget is absent meant a bank that
        // configured `recallSynonyms` never got expansion on either real surface.
        let owned;
        let (budget, synonyms) = match budget {
            Some(b) => {
                // Best-effort on this branch only. The fault the budget used to
                // swallow cannot arise here — the budget is the caller's, so a
                // config read that fails cannot make this recall answer at a
                // budget the caller did not ask for, and
                // `a_failed_config_read_should_fail_the_recall_that_needed_it`
                // keeps passing unchanged. What is lost on a fault is the
                // optional expansion, not the correctness of the cap.
                owned = self.config_of(bank_id).unwrap_or_default();
                (b, &owned.recall_synonyms)
            }
            None => {
                // Hoisted out of the arm so the borrow of `cfg.recall_synonyms`
                // lives as long as the tuple does.
                owned = self.config_of(bank_id)?;
                (
                    owned.recall_max_tokens.unwrap_or(DEFAULT_RECALL_BUDGET),
                    &owned.recall_synonyms,
                )
            }
        };
        {
            let mut scope = TagScope::new(tags, exclude);
            scope.dimensions = dimensions;
            self.recall_with(
                bank_id,
                query,
                budget,
                scope,
                &FusionWeights::SHIPPED,
                synonyms,
            )
        }
    }

    /// Recall top memories for `query` within `budget_tokens`.
    ///
    /// Reads no bank config at all — not `recallMaxTokens`, not `retainTags`, and
    /// so not `recallSynonyms` either. That is the existing contract of this
    /// entry point and it is not widened here: an explicit budget means the
    /// caller has already said what it wants, and a library caller with no config
    /// read should not start paying one.
    pub fn recall(
        &self,
        bank_id: &str,
        query: &str,
        budget_tokens: usize,
    ) -> Result<Vec<ScoredMemory>, ApiError> {
        Self::require_bank(bank_id)?;
        self.recall_with(
            bank_id,
            query,
            budget_tokens,
            TagScope::default(),
            &FusionWeights::SHIPPED,
            &EMPTY_SYNONYMS,
        )
    }

    /// Recall with explicit fusion weights instead of [`FusionWeights::SHIPPED`].
    ///
    /// Exists for `examples/sweep_fusion.rs` and nothing else: the shipped HTTP
    /// and MCP surfaces both go through [`Self::recall`], so no request can pick
    /// its own weights and the measured default cannot drift under a caller.
    /// Everything upstream of the fusion — the candidate pool, both streams, the
    /// budget trim, the result cap — is the same code either way, so a swept row
    /// and the shipped row differ *only* in the arithmetic this selects.
    pub fn recall_with_weights(
        &self,
        bank_id: &str,
        query: &str,
        budget_tokens: usize,
        weights: &FusionWeights,
    ) -> Result<Vec<ScoredMemory>, ApiError> {
        Self::require_bank(bank_id)?;
        self.recall_with(
            bank_id,
            query,
            budget_tokens,
            TagScope::default(),
            weights,
            &EMPTY_SYNONYMS,
        )
    }

    /// Recall top memories for `query`, scoped by [`TagScope`].
    ///
    /// Bounded by construction: the store hands over a bounded candidate pool, the
    /// FTS stream is limited in SQL, the overlap stream is capped, and at most
    /// [`MAX_RESULTS`] memories come back. The token budget is applied to the
    /// fused order first, then the result cap. Content is borrowed from the single
    /// store read — never copied into a second full-bank map.
    fn recall_with(
        &self,
        bank_id: &str,
        query: &str,
        budget_tokens: usize,
        tags: TagScope<'_>,
        weights: &FusionWeights,
        synonyms: &SynonymTable,
    ) -> Result<Vec<ScoredMemory>, ApiError> {
        let normalized = normalize_tags(tags.include);
        if !tags.include.is_empty() && normalized.is_empty() {
            // Fail closed. A filter the caller sent that normalizes to nothing
            // (["", "   "]) names no memory at all, and answering it with the
            // whole bank would reply to a scoped question with unrelated rows
            // that look exactly like a real result.
            return Ok(Vec::new());
        }
        // The lexical query the BM25 stream is built from, assembled from what
        // the caller already holds. No store call and no config read happens
        // here: `synonyms` arrived from the caller's own config read (or is a
        // shared empty map), and the two paths that read no config at all pass an
        // empty one, so the shipped `recall` pays exactly what it paid before
        // this existed.
        let lexical = LexicalQuery {
            text: query,
            scope: weights.keyword_scope,
            synonyms: expand_synonyms(synonyms, query),
        };
        // The dimensional filter is built from what the caller passed; `None`
        // means the shipped recall path runs byte-identically — no SQL, no rows
        // removed, same ranking, same score. `Some` narrows both streams in SQL
        // before fusion, so an excluded row never votes.
        let dims = tags.dimensions.as_ref().map(|d| crate::store::DimensionalFilter {
            scopes: d.scopes.to_vec(),
            aspects: d.aspects.to_vec(),
            lifecycles: d.lifecycles.to_vec(),
            include_history: d.include_history,
        });
        let (mut all, mut keyword_hits) = if let Some(dims) = dims.as_ref() {
            self.store
                .recall_inputs_dimensional(bank_id, &lexical, &normalized, FTS_LIMIT, dims)?
        } else {
            self.store
                .recall_inputs_lexical(bank_id, &lexical, &normalized, FTS_LIMIT)?
        };
        // Exclusion, one set lookup for the whole recall: the ids carrying any
        // excluded tag are dropped from both streams before fusion, so an
        // excluded row neither votes nor can be injected (audit GAP-3).
        // Normalized but not failed closed — see `recall_filtered`'s doc.
        let excluded: HashSet<String> = normalize_tags(tags.exclude)
            .into_iter()
            .collect::<HashSet<_>>();
        let excluded = if excluded.is_empty() {
            excluded
        } else {
            self.store
                .ids_tagged(bank_id, &excluded.into_iter().collect::<Vec<_>>())?
                .into_iter()
                .collect()
        };
        if !excluded.is_empty() {
            all.retain(|m| !excluded.contains(&m.id));
            keyword_hits.retain(|(id, _)| !excluded.contains(id));
        }
        // Stream A: real BM25 from SQLite FTS5 (bank- and tag-scoped in SQL).
        let fts_stream: Vec<RankedHit> = keyword_hits
            .iter()
            .enumerate()
            .map(|(i, (id, _))| RankedHit { id: id.clone(), rank: i + 1 })
            .collect();
        // The BM25 magnitudes, keyed by id. `store::KeywordHits` already carries
        // them as the `f64` of each pair — FTS5's `bm25()`, more negative better —
        // and they were dropped here at the `_` above. Built into a lookup only
        // when `bm25_magnitude` is non-zero, so the shipped path allocates
        // nothing and the fusion kernel skips the term entirely (an exact `+ 0.0`
        // is bit-identity, but not adding it at all is cheaper and provable).
        let magnitudes: HashMap<String, f64> = if weights.bm25_magnitude == 0.0 {
            HashMap::new()
        } else {
            keyword_hits.iter().map(|(id, rank)| (id.clone(), *rank)).collect()
        };
        // Stream B: token-overlap rank, capped so a large bank cannot flood fusion.
        // `overlap_scope` defaults to the whole-document scorer, which is the same
        // function this line has always called.
        let ranked =
            rank_candidates_scoped(query, all.iter().map(|m| m.content.as_str()), weights.overlap_scope);
        let overlap_stream: Vec<RankedHit> = ranked
            .iter()
            .take(OVERLAP_LIMIT)
            .enumerate()
            .map(|(i, (idx, _))| RankedHit { id: all[*idx].id.clone(), rank: i + 1 })
            .collect();
        // Stream C: recency, off unless the resolved weight is non-zero. Resolved
        // through `recency_weight_for` rather than read off the struct so the
        // decision to *build* the stream and the weight it is fused under are the
        // same value — and so a zero here never reads the clock, which is what
        // keeps the shipped recall deterministic.
        let mut streams = vec![fts_stream, overlap_stream];
        let recency_weight = weights.recency_weight_for(query);
        if recency_weight != 0.0 {
            let recency_stream = recency_rank(
                &all,
                chrono::Utc::now(),
                weights.recency_half_life_days,
            );
            if !recency_stream.is_empty() {
                streams.push(recency_stream);
            }
        }
        // `of()` names stream 2 as `recency`, so the third slot is only ever the
        // recency stream — and a stream that was not built leaves a 2-element
        // `of()` lookup untouched.
        let fused = rrf_fuse_with_magnitudes(&streams, &magnitudes, weights);

        // One index over the single store read, plus the score the ranking used.
        // Fused ids are bounded by the two stream caps, so the content lookup
        // below is bounded too.
        let by_id: HashMap<&str, &Memory> =
            all.iter().map(|m| (m.id.as_str(), m)).collect();
        let mut ids: Vec<String> = Vec::with_capacity(fused.len());
        let mut scores: HashMap<&str, f64> = HashMap::with_capacity(fused.len());
        // The content the budget trim reads is content `by_id` already holds, so
        // it is projected out of that one index in the same pass rather than
        // re-resolving every id a second time.
        let mut contents: HashMap<&str, &str> = HashMap::with_capacity(fused.len());
        for (id, score) in &fused {
            scores.insert(id.as_str(), *score);
            if let Some(m) = by_id.get(id.as_str()) {
                contents.insert(id.as_str(), m.content.as_str());
            }
            ids.push(id.clone());
        }
        let kept = trim_to_budget(&ids, &contents, budget_tokens);

        let mut out = Vec::with_capacity(kept.len().min(MAX_RESULTS));
        for (id, content) in kept.into_iter().take(MAX_RESULTS) {
            let Some(m) = by_id.get(id.as_str()) else {
                continue;
            };
            out.push(ScoredMemory {
                memory: Memory {
                    id: m.id.clone(),
                    bank_id: m.bank_id.clone(),
                    content,
                    context: m.context.clone(),
                    created_at: m.created_at.clone(),
                    // Borrowed as-is: recall never rewrites metadata, so the
                    // served shape is the stored shape.
                    meta: m.meta.clone(),
                },
                score: scores.get(id.as_str()).copied().unwrap_or(0.0),
            });
        }
        Ok(out)
    }

    /// One page of a bank's memories, in insertion order.
    ///
    /// `offset` counts rows rather than pages, so a caller walks the bank with a
    /// fixed limit. Bank-validated like every other entry point.
    pub fn list_memories(
        &self,
        bank_id: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Memory>, ApiError> {
        Self::require_bank(bank_id)?;
        Ok(self.store.list_page(bank_id, limit, offset)?)
    }

    /// One memory by id, or `None` when no such memory lives in `bank_id`.
    ///
    /// Bank-scoped: another bank's id reads as absent rather than handing back
    /// data from a namespace the caller was not granted.
    pub fn get_memory(&self, bank_id: &str, id: &str) -> Result<Option<Memory>, ApiError> {
        Self::require_bank(bank_id)?;
        Ok(self.store.get(bank_id, id)?)
    }

    /// The read side of the W2 `readOnly` guard, for the write paths that are
    /// not `retain`.
    ///
    /// `retain_doc_at_trusted` reads the config inline because it also needs the
    /// `maxTrust` ceiling; the three custody writes — delete, supersede, redact —
    /// need only this. They were gated on `require_bank` alone until the audit,
    /// which meant `readOnly` refused the write a caller usually means and
    /// silently allowed the three a caller uses to *remove* data. A read-only
    /// bank that can still be emptied is not a boundary, so the guard is one
    /// function every non-retain write calls rather than three copies that can
    /// drift.
    fn require_writable(&self, bank_id: &str) -> Result<(), ApiError> {
        Self::require_bank(bank_id)?;
        if self.config_of(bank_id)?.read_only {
            return Err(ApiError::ReadOnlyBank);
        }
        Ok(())
    }

    /// Delete one memory; `Ok(false)` when it was not in `bank_id`.
    ///
    /// A `readOnly` bank refuses this like every other write (W2): the flag is
    /// "this bank's contents are settled", and a delete is the most settled-
    /// content-destroying operation there is.
    pub fn delete_memory(&self, bank_id: &str, id: &str) -> Result<bool, ApiError> {
        self.require_writable(bank_id)?;
        Ok(self.store.delete(bank_id, id)?)
    }

    /// Mark a memory as superseded by another (or by nothing, for a retract).
    /// The row stays in the store but is excluded from default recall; its
    /// lineage survives in `memory_versions` and the `superseded_by` column.
    ///
    /// `superseded_by` is the id of the superseding memory, or `None` for a
    /// plain retract. Returns `Ok(true)` when a row was marked, `Ok(false)`
    /// when the id was not in `bank_id`.
    ///
    /// A `readOnly` bank refuses this: a supersede hides a row from default
    /// recall and appends to the audit trail, so it changes what the bank serves
    /// even though the row survives. Hiding is a write.
    pub fn supersede_memory(&self, bank_id: &str, id: &str, superseded_by: Option<&str>) -> Result<bool, ApiError> {
        self.require_writable(bank_id)?;
        Ok(self.store.supersede(bank_id, id, superseded_by)?)
    }

    /// Redact a pattern from a memory's current content and every version row.
    /// Returns the number of rows rewritten (current + versions); `Ok(0)` when
    /// the id is not in `bank_id`, or when the pattern no longer matches (a
    /// second redact is a no-op).
    ///
    /// The audit skeleton (op, created_at, actor, row id) survives; the value
    /// is gone from `memories` and `memory_versions`. The replacement is
    /// `[REDACTED:<name>]` so the row is still recognizable.
    ///
    /// A `readOnly` bank refuses this. Redaction rewrites stored bytes in place,
    /// which is the clearest case of a write the flag exists to stop.
    pub fn redact_memory(&self, bank_id: &str, id: &str, pattern: &str, name: &str) -> Result<usize, ApiError> {
        self.require_writable(bank_id)?;
        Ok(self.store.redact(bank_id, id, pattern, name)?)
    }

    /// Row counts and age bounds for one bank; zeroes and `None`s when empty.
    pub fn bank_stats(&self, bank_id: &str) -> Result<BankStats, ApiError> {
        Self::require_bank(bank_id)?;
        Ok(self.store.bank_stats(bank_id)?)
    }

    /// Reflect: cite the top recall hit as the synthesized answer (stub).
    ///
    /// `tags` scopes the answer to memories carrying any of them (empty = the
    /// whole bank), with the same fail-closed normalization recall applies.
    /// The framing an LLM step must follow is [`REFLECT_SYSTEM_PROMPT`].
    pub fn reflect(
        &self,
        bank_id: &str,
        query: &str,
        tags: &[String],
    ) -> Result<String, ApiError> {
        Self::require_bank(bank_id)?;
        let hits = self.recall_filtered(bank_id, query, Some(2000), tags, &[])?;
        match hits.first() {
            None => Ok("no relevant memories".to_string()),
            Some(h) => Ok(format!("based on [{}]: {}", h.memory.id, h.memory.content)),
        }
    }
}

/// Planned operations. Signatures land in Phase 2 (see PLAN.md).
pub fn operations() -> [&'static str; 3] {
    ["retain", "recall", "reflect"]
}

/// `GET`/`PUT /banks/:id/config`.
///
/// Shipped next to the service so the config surface and its validation cannot
/// drift; the binary wires it in with
/// `Router::merge(memory_wire::api::bank_config_routes::<S>())`.
pub fn bank_config_routes<S: Store + 'static>() -> Router<Arc<MemoryService<S>>> {
    Router::new().route("/banks/:id/config", get(get_bank_config).put(put_bank_config))
}

/// The TOON encoder, built in-tree rather than pulled in as a crate.
///
/// Declared here with an explicit `#[path]` so this module is the one file that
/// has to exist for it to compile — the crate root's module list belongs to
/// whoever owns `src/lib.rs`, and a `mod render;` here would look for
/// `src/api/render.rs` rather than `src/render.rs`. Promoting it to a top-level
/// `pub mod render;` in `lib.rs` is a one-line change whenever that is free; the
/// two are identical modules either way.
#[path = "render.rs"]
mod render;

/// The memory read routes, shipped next to the service for the same reason
/// [`bank_config_routes`] is.
///
/// `GET /banks/:id/memories` and `GET /banks/:id/memories/:mid` together, because
/// the truncated list value points at the second one as its escape hatch: split
/// across two files, the pointer and the route it names could disagree.
pub fn memories_routes<S: Store + 'static>() -> Router<Arc<MemoryService<S>>> {
    Router::new()
        .route("/banks/:id/memories", get(list_memories))
        .route("/banks/:id/memories/:mid", get(get_memory))
}

/// `GET /banks/:id/hygiene` — the W4 deterministic hygiene scan.
///
/// Read-only and deterministic: the same bank scanned twice yields the same
/// report. `?cursor=` narrows the delta to rows created after that
/// `created_at` watermark; omit it for a full scan. The response is the
/// [`crate::hygiene::HygieneReport`] rendered as JSON, so a caller can act on
/// the candidates rather than reading prose.
pub fn hygiene_routes<S: Store + 'static>() -> Router<Arc<MemoryService<S>>> {
    Router::new().route("/banks/:id/hygiene", get(bank_hygiene))
}

/// Ceiling on one page of memories, so one request cannot ask for the whole bank.
///
/// Unchanged from the value the binary shipped with, because the default response
/// may not move. A caller who genuinely wants the whole bank pages with
/// `?limit=500&offset=…`.
pub const MAX_PAGE: usize = 500;

/// Page size when `?limit` is absent.
const DEFAULT_PAGE: usize = 50;

/// `Content-Type` for `?format=toon`.
///
/// `text/toon; charset=utf-8`, which is the TOON specification's own provisional
/// media type (SPEC v4.1 §17) plus the charset it says is always assumed. Chosen
/// over the two candidates because it is a `text/*` type — so every generic HTTP
/// client, log viewer and text tool handles it without knowing the format — while
/// still naming the format, which `text/plain; charset=utf-8` would discard and
/// `application/toon` would get wrong (that subtype is neither specified nor
/// IANA-registered, and a client meeting an unknown `application/*` type may try
/// to parse it as a structured document).
pub const TOON_CONTENT_TYPE: &str = "text/toon; charset=utf-8";

/// True total in the bank, whatever `?limit` and `?offset` cut the page down to.
///
/// A header, not a body field, because the body of the list route is a bare JSON
/// array and that is frozen. It is the only way a caller learns whether the page
/// it is holding is the whole bank, which is otherwise guesswork.
const TOTAL_COUNT: HeaderName = HeaderName::from_static("x-total-count");

/// How many rows this response cut short, present only when at least one was cut.
///
/// The per-row marker inside each `content` names the exact original length and
/// the full form; this says how many rows are affected, so a caller can tell one
/// long memory from a whole page of them.
const TRUNCATED: HeaderName = HeaderName::from_static("x-truncated");

/// Which encoding a read route answers in.
///
/// A closed enum rather than a free string, so `?format=xml` is refused by the
/// query extractor as axum's own 400 — the same answer a non-numeric `?limit`
/// already gives this route — instead of being silently served as JSON. That
/// adds no status and no message to the frozen error table; it reuses the 400 a
/// malformed query parameter has always produced here.
#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum OutputFormat {
    /// `application/json`. The default, and byte-identical to the pre-TOON body.
    Json,
    /// `text/toon; charset=utf-8`.
    Toon,
}

/// Query parameters for `GET /banks/:id/memories`.
///
/// Every one is opt-in and every one defaults to what the route did before it
/// existed: `limit` 50 capped at [`MAX_PAGE`], `offset` 0, no truncation, JSON.
#[derive(Debug, Default, Deserialize)]
struct MemoriesQuery {
    /// Maximum rows in the page.
    limit: Option<usize>,
    /// Rows to skip before the page.
    offset: Option<usize>,
    /// Cap each `content` at this many characters. Absent means no cap.
    truncate: Option<usize>,
    /// `json` (default) or `toon`.
    format: Option<OutputFormat>,
}

/// Query parameters for `GET /banks/:id/memories/:mid`.
///
/// Only `format`. This route is the escape hatch a truncated list value points
/// at, so it is never truncated — and a separate struct rather than a shared one
/// so a malformed `?limit` on it stays the ignored query parameter it has always
/// been instead of becoming a new 400.
#[derive(Debug, Default, Deserialize)]
struct FormatQuery {
    /// `json` (default) or `toon`.
    format: Option<OutputFormat>,
}

/// Page size and offset: `DEFAULT_PAGE`/`0` by default, limit capped.
fn page_bounds(limit: Option<usize>, offset: Option<usize>) -> (usize, usize) {
    (
        limit.unwrap_or(DEFAULT_PAGE).min(MAX_PAGE),
        offset.unwrap_or(0),
    )
}

/// The content one row serves, and whether it was cut.
///
/// `cap` is `?truncate`. The cap applies to the memory's **own** characters; the
/// marker that follows is not counted against it, because a caller has to be able
/// to find where the memory stops — a cap that swallowed the marker would make
/// the notice indistinguishable from content.
///
/// Borrowed whenever nothing was cut, so a request that does not ask for
/// truncation copies no content: this is the default path.
fn served_content<'a>(memory: &'a Memory, cap: Option<usize>, bank: &str) -> (Cow<'a, str>, bool) {
    let Some(n) = cap else {
        return (Cow::Borrowed(memory.content.as_str()), false);
    };
    let total = memory.content.chars().count();
    if total <= n {
        return (Cow::Borrowed(memory.content.as_str()), false);
    }
    let mut out: String = memory.content.chars().take(n).collect();
    out.push_str(&format!(
        "…[truncated from {total} chars; full: GET /banks/{bank}/memories/{id}]",
        id = memory.id
    ));
    (Cow::Owned(out), true)
}

/// The longest a `format: "summary"` capsule may run before a sentence
/// boundary, then a hard cut, applies.
///
/// 200 is a structural fact, not a tuned one: it is roughly two lines of the
/// prose a hook captures and comfortably larger than a session-label `context`
/// line, so a capsule is scannable at k hits while never exceeding the length
/// of the smallest real memory. Derived from the form of the data; if a corpus
/// of one-line memories longer than this ever matters, the rule to change is
/// the sentence boundary, not this cap.
const SUMMARY_CAP_CHARS: usize = 200;

/// The capsule one memory serves under `format: "summary"`, with a pointer to
/// the whole when anything was hidden (H8).
///
/// The rule is structural, never an LLM: the **first line** of the content; if
/// that line runs past the 200-char cap, the first sentence boundary
/// (`. ` / `! ` / `? `) inside the cap, else a hard cut at the cap. When the
/// capsule hides anything — more lines, or the rest of the line — the marker
/// carries the original length and the fetch command, in the same shape
/// `?truncate` already uses so every consumer of one recognizes the other:
///
/// ```text
/// first line…[summary of 1234 chars; full: GET /banks/b/memories/<id>]
/// ```
///
/// Borrowed when the content is its own capsule, so a corpus of one-line
/// memories serves through this path with zero copies.
pub fn summary_content<'a>(memory: &'a Memory, bank: &str) -> Cow<'a, str> {
    let content = memory.content.as_str();
    let total = content.chars().count();
    if total == 0 {
        return Cow::Borrowed(content);
    }
    // The capsule proper: first line, sentence-cut when the line is long.
    let first_line_end = content.find('\n').unwrap_or(content.len());
    let mut end = first_line_end;
    if end > SUMMARY_CAP_CHARS {
        // A sentence boundary inside the cap, punctuation included. `rfind`
        // because the nearest boundary reads best, and the cap holds the scan.
        let head = &content[..content
            .char_indices()
            .nth(SUMMARY_CAP_CHARS)
            .map_or(content.len(), |(i, _)| i)];
        end = head
            .char_indices()
            .rev()
            .find_map(|(i, c)| match c {
                '.' | '!' | '?' => Some(i + c.len_utf8()),
                _ => None,
            })
            .unwrap_or(head.len());
    }
    // A capsule that reproduced the whole content hides nothing and carries no
    // marker — indistinguishable from the default shape for that row.
    if end >= content.len() {
        return Cow::Borrowed(content);
    }
    let mut out = content[..end].to_string();
    out.push_str(&format!(
        "…[summary of {total} chars; full: GET /banks/{bank}/memories/{id}]",
        id = memory.id
    ));
    Cow::Owned(out)
}

/// One memory as the lifecycle routes serve it.
///
/// `created_at` is always present; `context` only when the memory has one, so a
/// memory stored without capture context does not claim an empty one. `content` is
/// the served string rather than the stored one, which is the same string unless
/// `?truncate` cut it.
///
/// The W1 dimensions ride in a `meta` object and are omitted when the memory is
/// default on every one of them, so a row written through the shipped path reads
/// back exactly as it did before. A caller who *did* set `scope`/`aspect` can now
/// see what was stored: without this the write side was exposed on the wire while
/// the read side silently dropped it, and every filter's result was unauditable.
fn memory_json(memory: &Memory, content: &str) -> serde_json::Value {
    let mut v = serde_json::json!({
        "id": memory.id,
        "content": content,
        "created_at": memory.created_at
    });
    if let Some(ctx) = &memory.context {
        v["context"] = serde_json::json!(ctx);
    }
    let meta = memory_meta_json(memory);
    if !meta.is_null() {
        v["meta"] = meta;
    }
    v
}

/// The W1 dimension block for one memory, or `null` when every axis is at its
/// default.
///
/// `Default` is the test: an empty list and an empty `Trust` mean "not set", and a
/// memory that set nothing should not carry five empty fields a client has to
/// special-case. `trust` is the one axis that is *always* set (the shipped path
/// writes `AgentInferred` like every insert), so it is emitted as a bare string
/// rather than folded into the omission rule.
fn memory_meta_json(memory: &Memory) -> serde_json::Value {
    let MemoryMeta {
        scope,
        aspect,
        lifecycle,
        trust,
        parent_id,
        superseded_by,
        scope_globs,
    } = &memory.meta;
    // The defaults every write carries: a request that names no tier lands on
    // these. A row that set nothing should not carry five default fields a client
    // has to special-case, so each axis is checked against its default and only
    // non-default values are emitted.
    let def_scope = Scope::default().0;
    let def_aspect = Aspect::default().0;
    let def_lifecycle = Lifecycle::default().0;
    let def_trust = crate::memory::Trust::default().0;
    if scope.0 == def_scope
        && aspect.0 == def_aspect
        && lifecycle.0 == def_lifecycle
        && trust.0 == def_trust
        && parent_id.is_none()
        && superseded_by.is_none()
        && scope_globs.is_empty()
    {
        return serde_json::Value::Null;
    }
    let mut v = serde_json::json!({
        "scope": scope.0,
        "trust": trust.0,
    });
    if aspect.0 != def_aspect {
        v["aspects"] = serde_json::json!(aspect.0);
    }
    if lifecycle.0 != def_lifecycle {
        v["lifecycle"] = serde_json::json!(lifecycle.0);
    }
    if let Some(p) = parent_id {
        v["parent_id"] = serde_json::json!(p);
    }
    if let Some(s) = superseded_by {
        v["superseded_by"] = serde_json::json!(s);
    }
    if !scope_globs.is_empty() {
        v["scope_globs"] = serde_json::json!(scope_globs);
    }
    v
}

/// `GET /banks/:id/memories` — one page of the bank's memories.
///
/// The default answer is byte-for-byte what this route answered before `?limit`'s
/// companions, `?format` and the two metadata headers existed: a bare JSON array
/// built by [`Json`] from the same values in the same order. Everything added here
/// is opt-in through the query or carried in a header, because the array shape is
/// frozen and a client parsing it as an array is entitled to keep working.
async fn list_memories<S: Store + 'static>(
    State(svc): State<Arc<MemoryService<S>>>,
    Path(bank): Path<String>,
    Query(q): Query<MemoriesQuery>,
) -> Result<Response, (StatusCode, String)> {
    let (limit, offset) = page_bounds(q.limit, q.offset);
    let format = q.format.unwrap_or(OutputFormat::Json);
    let rows = blocking({
        let (page_svc, page_bank) = (Arc::clone(&svc), bank.clone());
        move || page_svc.list_memories(&page_bank, limit, offset)
    })
    .await
    .map_err(to_http)?;
    // A second read, because the page cannot answer "is that everything" on its
    // own: a full page and a whole bank look identical from inside the array.
    // `bank_stats` is the count already on the surface — a narrower count would
    // mean a new `Store` method, and `src/store.rs` is not this change's file.
    // It costs one `COUNT(*)` alongside the page read, not a second pass over the
    // rows the page already holds.
    let total = blocking({
        let (count_svc, count_bank) = (Arc::clone(&svc), bank.clone());
        move || count_svc.bank_stats(&count_bank).map(|s| s.memories)
    })
    .await
    .map_err(to_http)?;

    let cap = q.truncate;
    let mut truncated = 0usize;
    let mut res = match format {
        OutputFormat::Json => {
            let body: Vec<serde_json::Value> = rows
                .iter()
                .map(|m| {
                    let (content, cut) = served_content(m, cap, &bank);
                    truncated += usize::from(cut);
                    memory_json(m, content.as_ref())
                })
                .collect();
            Json(body).into_response()
        }
        OutputFormat::Toon => {
            let (cells, cut) = toon_rows(&rows, cap, &bank);
            truncated = cut;
            let body = render::document(&toon_fields(total, cut), "memories", &cells);
            (StatusCode::OK, [(CONTENT_TYPE, TOON_CONTENT_TYPE)], body).into_response()
        }
    };
    set_metadata_headers(&mut res, total, truncated);
    Ok(res)
}

/// `GET /banks/:id/memories/:mid` — one memory, or 404 `unknown memory`.
///
/// Also the destination a `?truncate`d list value names, so it accepts `?format`
/// and answers in the format the caller is already reading.
async fn get_memory<S: Store + 'static>(
    State(svc): State<Arc<MemoryService<S>>>,
    Path((bank, mid)): Path<(String, String)>,
    Query(q): Query<FormatQuery>,
) -> Result<Response, (StatusCode, String)> {
    let found = blocking(move || svc.get_memory(&bank, &mid))
        .await
        .map_err(to_http)?;
    let Some(m) = found else {
        return Err((StatusCode::NOT_FOUND, "unknown memory".to_string()));
    };
    Ok(match q.format.unwrap_or(OutputFormat::Json) {
        OutputFormat::Json => Json(memory_json(&m, m.content.as_str())).into_response(),
        OutputFormat::Toon => {
            let mut fields = vec![
                ("id".to_string(), render::Cell::Text(m.id.clone())),
                (
                    "created_at".to_string(),
                    m.created_at
                        .clone()
                        .map_or(render::Cell::Null, render::Cell::Text),
                ),
                ("content".to_string(), render::Cell::Text(m.content.clone())),
            ];
            if let Some(ctx) = &m.context {
                fields.push(("context".to_string(), render::Cell::Text(ctx.clone())));
            }
            let mut res = (
                StatusCode::OK,
                [(CONTENT_TYPE, TOON_CONTENT_TYPE)],
                render::object(&fields),
            )
                .into_response();
            set_metadata_headers(&mut res, 1, 0);
            res
        }
    })
}

/// Attach the two metadata headers a list response carries.
fn set_metadata_headers(res: &mut Response, total: usize, truncated: usize) {
    let headers = res.headers_mut();
    headers.insert(TOTAL_COUNT, count_header(total));
    if truncated > 0 {
        headers.insert(TRUNCATED, count_header(truncated));
    }
}

/// An integer as a header value.
///
/// `HeaderValue::from_str` rejects anything but visible ASCII, and a `usize`
/// renders as decimal digits, so this cannot fail for the only input it takes.
fn count_header(n: usize) -> HeaderValue {
    HeaderValue::from_str(&n.to_string()).expect("a usize renders as ASCII digits")
}

/// The scalar fields a TOON document carries ahead of its table.
///
/// `total` is unconditional: TOON has no response headers a caller can read
/// without parsing the body, so the count the caller needs has to be in the body
/// it is already parsing, and making it conditional on having been truncated is
/// arithmetic the caller then has to do to learn whether it is looking at
/// everything. `content_truncated` appears only when rows were cut.
fn toon_fields(total: usize, truncated: usize) -> Vec<(String, render::Cell)> {
    let mut fields = vec![("total".to_string(), render::Cell::Number(total.to_string()))];
    if truncated > 0 {
        fields.push((
            "content_truncated".to_string(),
            render::Cell::Number(truncated.to_string()),
        ));
    }
    fields
}

/// The list rows as TOON cells, `id` first, plus how many were truncated.
///
/// Field order is the header's and nothing depends on it being alphabetical the
/// way the JSON object's is — `id` first because a caller reading a table wants
/// the key before the prose, and `created_at` before `content` for the same
/// reason. `context` is last and present only when the memory has one, which is
/// what makes a page that mixes context-bearing rows non-uniform and therefore
/// list-shaped (see [`render`]).
fn toon_rows(rows: &[Memory], cap: Option<usize>, bank: &str) -> (Vec<render::Row>, usize) {
    let mut cut = 0;
    let cells = rows
        .iter()
        .map(|m| {
            let (content, was_cut) = served_content(m, cap, bank);
            cut += usize::from(was_cut);
            let mut row = vec![
                ("id".to_string(), render::Cell::Text(m.id.clone())),
                (
                    "created_at".to_string(),
                    m.created_at
                        .clone()
                        .map_or(render::Cell::Null, render::Cell::Text),
                ),
                ("content".to_string(), render::Cell::Text(content.into_owned())),
            ];
            if let Some(ctx) = &m.context {
                row.push(("context".to_string(), render::Cell::Text(ctx.clone())));
            }
            row
        })
        .collect();
    (cells, cut)
}

/// Run one blocking store call off the async worker threads.
///
/// SQLite is synchronous, so a handler that called the store inline parked a
/// tokio worker in `lock()`/SQLite for the length of the request. Past the
/// worker count every one of them is parked, and then the graceful-shutdown
/// future — which also needs a worker to be polled — can never run, so the
/// process stops answering to a signal it never sees. The blocking pool runs
/// the same work with the workers left free.
///
/// This is still true with the store's read pool: a reader is a connection's
/// worth of blocking work, not the mutex, and the pool bounds how many of those
/// can run at once rather than making any of them non-blocking.
///
/// A panic inside the task arrives here as a join failure, and is mapped into the
/// same error type as a storage fault so it still answers the documented 500
/// instead of leaving the request with no response at all.
pub async fn blocking<T, F>(f: F) -> Result<T, ApiError>
where
    F: FnOnce() -> Result<T, ApiError> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|_| ApiError::Store(StoreError::TaskFailed))?
}

/// `GET /banks/:id/config` — the raw config object, or `{}` when unset.
async fn get_bank_config<S: Store + 'static>(
    State(svc): State<Arc<MemoryService<S>>>,
    Path(bank): Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let raw = blocking(move || svc.bank_config(&bank))
        .await
        .map_err(to_http)?;
    Ok(Json(serde_json::from_str(&raw).unwrap_or_else(|_| serde_json::json!({}))))
}

/// `?cursor=` for the hygiene scan: a `created_at` watermark, exclusive.
#[derive(Deserialize)]
pub struct HygieneQuery {
    /// Only scan rows created after this timestamp. Omit for a full scan.
    pub cursor: Option<String>,
}

/// `GET /banks/:id/hygiene` — run the W4 deterministic scan and return the
/// report as JSON.
///
/// Read-only: the handler calls `hygiene::hygiene_scan`, which opens no write
/// transaction. `blocking` because the scan walks the whole bank and must not
/// run on a tokio worker (the crate rule).
async fn bank_hygiene<S: Store + 'static>(
    State(svc): State<Arc<MemoryService<S>>>,
    Path(bank): Path<String>,
    Query(q): Query<HygieneQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // The bank must exist before it can be scanned: a typo'd id should be a
    // 404 naming it, not an empty report that reads as "this bank is clean".
    // `bank_stats` is the existing existence check on this surface.
    let stats = blocking({
        let (count_svc, count_bank) = (Arc::clone(&svc), bank.clone());
        move || count_svc.bank_stats(&count_bank)
    })
    .await
    .map_err(to_http)?;
    if stats.memories == 0 && stats.newest.is_none() {
        // No rows and no age: either an unknown bank or an empty one. Both are
        // answered the same way the other collection routes answer an absence —
        // `200` with an empty report is right for a *known* empty bank, but a
        // name no client has written to is a 404 the caller can act on.
        let known = blocking({
            let (cfg_svc, cfg_bank) = (Arc::clone(&svc), bank.clone());
            move || cfg_svc.bank_config(&cfg_bank)
        })
        .await
        .map_err(to_http)?;
        if known == "{}" {
            // Config absent *and* no rows: nothing was ever written here.
            return Err((
                StatusCode::NOT_FOUND,
                format!("bank `{bank}` is empty; nothing to scan"),
            ));
        }
    }
    let opts = crate::hygiene::HygieneOptions::default();
    let report = blocking({
        let (scan_svc, scan_bank, cursor) = (Arc::clone(&svc), bank.clone(), q.cursor.clone());
        move || {
            crate::hygiene::hygiene_scan(scan_svc.store.as_ref(), &scan_bank, cursor.as_deref(), &opts)
                .map_err(ApiError::from)
        }
    })
    .await
    .map_err(to_http)?;
    Ok(Json(hygiene_json(&report)))
}

/// The report as a JSON object a caller can act on.
///
/// Hand-built rather than `#[derive(Serialize)]` on the report type: the
/// candidates are an enum with per-variant payloads, and the shape that reads
/// best over the wire is one object per candidate with a `kind` tag, not a
/// serde-untagged guess. The Display form stays the human one.
fn hygiene_json(report: &crate::hygiene::HygieneReport) -> serde_json::Value {
    use crate::hygiene::HygieneCandidate;
    let candidates: Vec<serde_json::Value> = report
        .candidates
        .iter()
        .map(|c| {
            let kind = match c {
                HygieneCandidate::Merge { .. } => "merge",
                HygieneCandidate::Supersede { .. } => "supersede",
                HygieneCandidate::Anchor { .. } => "anchor",
                HygieneCandidate::Promote { .. } => "promote",
                HygieneCandidate::Split { .. } => "split",
            };
            serde_json::json!({ "kind": kind, "detail": c.to_string() })
        })
        .collect();
    serde_json::json!({
        "bank": report.bank,
        "cursor": report.cursor,
        "rows_scanned": report.rows_scanned,
        "candidates": candidates,
    })
}

/// `PUT /banks/:id/config` — replace the whole config, echoing what was stored.
async fn put_bank_config<S: Store + 'static>(
    State(svc): State<Arc<MemoryService<S>>>,
    Path(bank): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    // The config text is built here so the closure owns it: the body is echoed
    // back at the end, and a move would leave nothing to echo.
    let raw = body.to_string();
    blocking(move || svc.set_bank_config(&bank, &raw))
        .await
        .map_err(to_http)?;
    Ok(Json(body))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::recall::{
        is_temporal_query, rank_candidates, recency_rank, KeywordScope, OverlapScope, RecencyPolicy,
    };
    use crate::memory::Bank;
    use crate::store::{SqliteStore, UpdateMode};

    #[test]
    fn retain_should_redact_before_persist() {
        let s = SqliteStore::open_in_memory().expect("open");
        s.put_bank(&Bank { id: "b".to_string(), name: "b".to_string() })
            .expect("bank");
        let svc = MemoryService::new(s);
        let id = svc.retain("b", "key sk-abcDEF123456", None).expect("retain");
        let got = svc.store.get("b", &id).expect("get").expect("some");
        assert!(got.content.contains("[REDACTED:api_key]"));
    }

    #[test]
    fn recall_should_isolate_banks() {        let s = SqliteStore::open_in_memory().expect("open");
        for b in ["a", "b"] {
            s.put_bank(&Bank { id: b.to_string(), name: b.to_string() })
                .expect("bank");
        }
        let svc = MemoryService::new(s);
        svc.retain("a", "auth uses jose middleware", None).expect("retain");
        assert_eq!(svc.recall("b", "jose", 2000).expect("recall").len(), 0);
        assert_eq!(svc.recall("a", "jose", 2000).expect("recall").len(), 1);
    }

    #[test]
    fn retain_should_redact_context_too() {
        let s = SqliteStore::open_in_memory().expect("open");
        s.put_bank(&Bank { id: "b".to_string(), name: "b".to_string() })
            .expect("bank");
        let svc = MemoryService::new(s);
        let id = svc
            .retain("b", "harmless body", Some("hook saw ghp_abcdefgh12345678".into()))
            .expect("retain");
        let got = svc.store.get("b", &id).expect("get").expect("some");
        let ctx = got.context.expect("context");
        assert!(ctx.contains("[REDACTED:github_token]"), "got {ctx}");
    }

    fn invalid_bank<T>(r: Result<T, ApiError>) -> bool {
        matches!(r, Err(ApiError::Store(StoreError::InvalidBank)))
    }

    #[test]
    fn blank_bank_id_should_fail_on_every_entry_point() {
        let svc = MemoryService::new(SqliteStore::open_in_memory().expect("open"));
        for bank in ["", "   ", "\t\n"] {
            assert!(invalid_bank(svc.retain(bank, "x", None)), "retain {bank:?}");
            assert!(invalid_bank(svc.recall(bank, "x", 100)), "recall {bank:?}");
            assert!(invalid_bank(svc.reflect(bank, "x", &[])), "reflect {bank:?}");
            assert!(invalid_bank(svc.list_memories(bank, 1, 0)), "list {bank:?}");
            assert!(invalid_bank(svc.get_memory(bank, "m")), "get {bank:?}");
            assert!(invalid_bank(svc.delete_memory(bank, "m")), "delete {bank:?}");
            assert!(invalid_bank(svc.bank_stats(bank)), "stats {bank:?}");
            assert!(
                invalid_bank(svc.retain_doc(bank, "x", None, &[], Some("d"), UpdateMode::Replace)),
                "retain_doc {bank:?}"
            );
        }
    }

    #[test]
    fn http_error_should_map_bad_input_to_400_and_storage_to_opaque_500() {
        let svc = MemoryService::new(SqliteStore::open_in_memory().expect("open"));

        let (status, msg) = http_error(&svc.retain("  ", "x", None).expect_err("blank bank"));
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(msg, "invalid bank id");

        // A retain names the bank it creates, so the orphan-bank write that used
        // to land here is gone; the opaque-500 half of the contract is asserted
        // against the error itself, which is what a storage failure still maps to.
        let failed = ApiError::Store(StoreError::Sqlite(rusqlite::Error::QueryReturnedNoRows));
        let (status, msg) = http_error(&failed);
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(msg, "storage error");
        assert!(!msg.contains("FOREIGN KEY"), "raw driver text must not leak");
    }

    // The HTTP and MCP handlers both declared the bank before writing, so a
    // library retain that refused to would have been the one surface where the
    // documented "a bank is created by its first retain" rule did not hold.
    #[test]
    fn retain_should_create_the_bank_it_names() {
        let s = SqliteStore::open_in_memory().expect("open");
        let svc = MemoryService::new(s);
        assert!(svc.store.get_bank_config("fresh").expect("config").is_none());

        let id = svc.retain("fresh", "auth uses jose", None).expect("retain");
        assert!(svc.store.get_bank_config("fresh").expect("config").is_some());
        assert_eq!(
            svc.get_memory("fresh", &id).expect("get").expect("some").content,
            "auth uses jose"
        );
        // Every entry point into the write path, not just the plain one.
        svc.retain_tagged("tagged", "jose note", None, &tags(&["t"]))
            .expect("tagged");
        svc.retain_doc("documented", "jose note", None, &[], Some("d"), UpdateMode::Replace)
            .expect("documented");
        for bank in ["fresh", "tagged", "documented"] {
            assert_eq!(svc.bank_stats(bank).expect("stats").memories, 1, "{bank}");
        }
    }

    // Re-declaring the bank must not rename it or drop its config, so a second
    // retain cannot quietly reset a bank the caller configured.
    #[test]
    fn retain_should_not_clobber_a_configured_bank() {
        let svc = svc_with_banks(&["b"]);
        svc.set_bank_config("b", r#"{"retain_mission":"own the release"}"#)
            .expect("config");
        svc.retain("b", "auth uses jose", None).expect("retain");
        assert_eq!(
            svc.bank_config("b").expect("config"),
            r#"{"retain_mission":"own the release"}"#,
            "a retain re-declares the bank and must keep what it already held"
        );
    }

    // Byte-identical content is one memory, not two: the second retain hands
    // back the row that already holds it and writes nothing.
    #[test]
    fn retain_should_dedup_byte_identical_content_within_a_bank() {
        let svc = svc_with_banks(&["a", "b"]);
        let first = svc.retain("a", "auth uses jose middleware", None).expect("first");
        let again = svc.retain("a", "auth uses jose middleware", None).expect("again");
        assert_eq!(again, first, "the row that already holds the content is returned");
        assert_eq!(svc.bank_stats("a").expect("stats").memories, 1, "no second copy");

        // Scoped to the bank: the same content in another bank is a different
        // memory, not a duplicate of this one.
        svc.retain("b", "auth uses jose middleware", None).expect("other bank");
        assert_eq!(svc.bank_stats("b").expect("stats").memories, 1);

        // Different content is a different memory.
        svc.retain("a", "auth uses jose secrets", None).expect("changed");
        assert_eq!(svc.bank_stats("a").expect("stats").memories, 2);
    }

    // Dedup compares what would be *written*, so two spellings of one secret
    // collapse and nothing else does.
    #[test]
    fn retain_should_dedup_on_the_redacted_content() {
        let svc = svc_with_banks(&["a"]);
        let first = svc.retain("a", "key sk-abcDEF123456", None).expect("first");
        let again = svc.retain("a", "key sk-zzzYYY987654", None).expect("again");
        assert_eq!(again, first, "both redact to the same stored text");
        assert_eq!(svc.bank_stats("a").expect("stats").memories, 1);
    }

    // A caller holding a `document_id` is choosing upsert semantics, so dedup
    // must not answer with some other row's id and quietly leave the document
    // without a revision.
    #[test]
    fn a_document_scoped_retain_should_not_be_deduped() {
        let svc = svc_with_banks(&["a"]);
        let first = svc
            .retain_doc("a", "auth uses jose v1", None, &[], Some("doc"), UpdateMode::Replace)
            .expect("v1");
        let second = svc
            .retain_doc("a", "auth uses jose v1", None, &[], Some("other"), UpdateMode::Replace)
            .expect("v2");
        assert_ne!(second, first, "each document id gets its own row");
        assert_eq!(svc.bank_stats("a").expect("stats").memories, 2);

        // An append is the one mode that must keep adding: a second row under a
        // fresh document id is the whole point, so identical content is still two
        // memories.
        let a = svc
            .retain_doc("a", "identical", None, &[], Some("d1"), UpdateMode::Append)
            .expect("append 1");
        let b = svc
            .retain_doc("a", "identical", None, &[], Some("d2"), UpdateMode::Append)
            .expect("append 2");
        assert_ne!(a, b);
    }

    #[test]
    fn recall_should_truncate_rather_than_exceed_the_budget() {
        let s = SqliteStore::open_in_memory().expect("open");
        s.put_bank(&Bank { id: "b".to_string(), name: "b".to_string() })
            .expect("bank");
        let svc = MemoryService::new(s);
        let long = "jose ".repeat(400);
        svc.retain("b", &long, None).expect("retain");

        let hits = svc.recall("b", "jose", 5).expect("recall");
        assert_eq!(hits.len(), 1, "an over-budget top hit is truncated, not dropped");
        let got = &hits[0].memory.content;
        assert!(got.ends_with(crate::recall::TRUNCATION_MARKER), "got len {}", got.len());
        assert!(got.chars().count() <= 5 * 4, "budget cap breached: {}", got.chars().count());

        // A one-token budget cannot fit the marker, so the top hit comes back cut
        // to exactly the cap rather than dropped: R@1 is never bought with a
        // budget the caller set too small to hold one.
        let tiny = svc.recall("b", "jose", 1).expect("tiny");
        assert_eq!(tiny.len(), 1, "a tiny budget still returns the top hit");
        assert_eq!(tiny[0].memory.content.chars().count(), 4, "cut to exactly the cap");
        assert!(!tiny[0].memory.content.contains(crate::recall::TRUNCATION_MARKER));

        assert!(svc.recall("b", "jose", 0).expect("recall").is_empty());
    }

    #[test]
    fn recall_should_cap_results_at_one_hundred() {
        let s = SqliteStore::open_in_memory().expect("open");
        s.put_bank(&Bank { id: "b".to_string(), name: "b".to_string() })
            .expect("bank");
        let svc = MemoryService::new(s);
        for i in 0..150 {
            svc.retain("b", &format!("shared token note number {i}"), None).expect("retain");
        }
        let hits = svc.recall("b", "shared token", 1_000_000).expect("recall");
        assert_eq!(hits.len(), MAX_RESULTS, "recall must cap at {MAX_RESULTS}");
        assert!(hits.iter().all(|h| !h.memory.content.is_empty()));
    }

    #[test]
    fn reflect_prompt_should_stay_declarative_and_historical() {
        let p = REFLECT_SYSTEM_PROMPT.to_lowercase();
        // Reports decisions and rationale, never the current implementation.
        assert!(REFLECT_SYSTEM_PROMPT.contains("Report decisions and rationale, never the current implementation"));
        // Past tense framing, not a change queue.
        assert!(p.contains("past decisions"));
        // Imperatives are banned, so each is only ever allowed to appear inside
        // the prohibition itself.
        for banned in ["you should", "you must", "you need to"] {
            let hits = p.matches(banned).count();
            assert!(hits <= 1, "\"{banned}\" appears {hits}x: {p}");
        }
        // Literal tables and numbers are reproduced, not summarized.
        assert!(p.contains("verbatim"));
    }

    fn svc_with_banks(banks: &[&str]) -> MemoryService<SqliteStore> {
        let s = SqliteStore::open_in_memory().expect("open");
        for b in banks {
            s.put_bank(&Bank { id: b.to_string(), name: b.to_string() })
                .expect("bank");
        }
        MemoryService::new(s)
    }

    // ── the memory read routes ──────────────────────────────────────────────
    //
    // These drive the handlers directly rather than through a live server: the
    // router is wired in `main.rs`, which this change does not own, and a handler
    // called with the same `State`/`Path`/`Query` values a real request produces
    // exercises the same code. The one thing this cannot prove is that the route
    // is registered at all, which is a one-line merge in the binary.

    /// Split a response into its headers and raw body bytes.
    async fn split(res: Response) -> (axum::http::HeaderMap, Vec<u8>) {
        let (parts, body) = res.into_parts();
        let bytes = axum::body::to_bytes(body, usize::MAX).await.expect("body");
        (parts.headers, bytes.to_vec())
    }

    /// A response body as text.
    async fn text(res: Response) -> String {
        let (_, body) = split(res).await;
        String::from_utf8(body).expect("a utf-8 body")
    }

    /// `GET /banks/:bank/memories` with an explicit query.
    async fn call_list(
        svc: &Arc<MemoryService<SqliteStore>>,
        bank: &str,
        q: MemoriesQuery,
    ) -> Response {
        list_memories(
            State(Arc::clone(svc)),
            Path(bank.to_string()),
            Query(q),
        )
        .await
        .expect("a valid bank lists")
    }

    /// `GET /banks/:bank/memories/:mid` with an explicit query.
    async fn call_get(
        svc: &Arc<MemoryService<SqliteStore>>,
        bank: &str,
        mid: &str,
        q: FormatQuery,
    ) -> Result<Response, (StatusCode, String)> {
        get_memory(
            State(Arc::clone(svc)),
            Path((bank.to_string(), mid.to_string())),
            Query(q),
        )
        .await
    }

    /// The list route's pre-change row builder, kept exactly as `main.rs` had it.
    ///
    /// Deliberately a second implementation: a byte-identity test that called the
    /// helper under test would compare a value with itself and pass whatever the
    /// helper did.
    fn legacy_memory_json(m: &Memory) -> serde_json::Value {
        let mut v = serde_json::json!({
            "id": m.id,
            "content": m.content,
            "created_at": m.created_at
        });
        if let Some(ctx) = &m.context {
            v["context"] = serde_json::json!(ctx);
        }
        v
    }

    /// A bank with one short row, one 9,000-character row, and one context row —
    /// the shapes that make truncation, the total count and the TOON form each do
    /// something other than pass through.
    fn wide_bank() -> Arc<MemoryService<SqliteStore>> {
        let svc = Arc::new(svc_with_banks(&["t"]));
        svc.retain("t", "auth uses jose", None).expect("retain");
        svc.retain("t", &"deploy key rotates. ".repeat(450), None)
            .expect("retain long");
        svc.retain_doc(
            "t",
            "hooks retain the transcript",
            Some("saw the prompt".into()),
            &[],
            None,
            UpdateMode::Replace,
        )
        .expect("retain with context");
        svc
    }

    /// The long row's id, which the truncation marker has to name.
    fn long_id(svc: &MemoryService<SqliteStore>) -> String {
        svc.list_memories("t", 50, 0)
            .expect("list")
            .into_iter()
            .find(|m| m.content.chars().count() == 9_000)
            .expect("the 9000-character row")
            .id
    }

    #[tokio::test]
    async fn the_default_list_body_should_be_byte_identical_to_the_pre_change_shape() {
        let svc = wide_bank();
        let rows = svc.list_memories("t", DEFAULT_PAGE, 0).expect("rows");

        // The response the route served before this change, rebuilt here.
        let legacy: Vec<serde_json::Value> = rows.iter().map(legacy_memory_json).collect();
        let (want_headers, want_body) = split(Json(legacy).into_response()).await;

        let (got_headers, got_body) = split(call_list(&svc, "t", MemoriesQuery::default()).await).await;

        assert_eq!(got_body, want_body, "the default body bytes moved");
        // Every header the old response carried is still there, unchanged, and
        // the only addition is the count.
        for (name, value) in &want_headers {
            assert_eq!(got_headers.get(name), Some(value), "{name} changed");
        }
        let added: Vec<&str> = got_headers
            .keys()
            .filter(|n| !want_headers.contains_key(*n))
            .map(axum::http::HeaderName::as_str)
            .collect();
        assert_eq!(added, vec!["x-total-count"], "unexpected new headers");
    }

    #[tokio::test]
    async fn format_json_should_be_byte_identical_to_no_format_at_all() {
        let svc = wide_bank();
        let (default_headers, default_body) =
            split(call_list(&svc, "t", MemoriesQuery::default()).await).await;
        let (json_headers, json_body) = split(
            call_list(
                &svc,
                "t",
                MemoriesQuery { format: Some(OutputFormat::Json), ..Default::default() },
            )
            .await,
        )
        .await;
        assert_eq!(default_body, json_body);
        assert_eq!(default_headers, json_headers);
    }

    #[tokio::test]
    async fn the_total_count_header_should_name_the_whole_bank_not_the_page() {
        let svc = wide_bank();
        let (headers, _) = split(
            call_list(
                &svc,
                "t",
                MemoriesQuery { limit: Some(1), ..Default::default() },
            )
            .await,
        )
        .await;
        assert_eq!(headers.get("x-total-count").expect("count"), "3");
    }

    #[tokio::test]
    async fn an_untruncated_response_should_carry_no_truncation_header() {
        let svc = wide_bank();
        let (headers, body) = split(call_list(&svc, "t", MemoriesQuery::default()).await).await;
        assert!(headers.get("x-truncated").is_none(), "{headers:?}");
        let rows: Vec<serde_json::Value> = serde_json::from_slice(&body).expect("array");
        assert_eq!(rows.len(), 3);
        assert!(rows.iter().all(|r| !r["content"].as_str().unwrap_or("").contains("truncated")));
    }

    #[tokio::test]
    async fn truncate_should_cap_content_and_name_the_full_form() {
        let svc = wide_bank();
        let id = long_id(&svc);
        let (headers, body) = split(
            call_list(
                &svc,
                "t",
                MemoriesQuery { truncate: Some(40), ..Default::default() },
            )
            .await,
        )
        .await;
        let rows: Vec<serde_json::Value> = serde_json::from_slice(&body).expect("array");
        let long = rows
            .iter()
            .find(|r| r["content"].as_str().unwrap_or("").contains("truncated from"))
            .expect("the truncated row")
            .clone();
        let content = long["content"].as_str().expect("content");

        // The cap applies to the memory's own characters; the marker is added
        // after them, so a caller can still find where the memory stopped.
        assert!(content.starts_with(&"deploy key rotates. ".repeat(2)[..40]), "{content}");
        assert!(content.contains("truncated from 9000 chars"), "{content}");
        assert!(
            content.contains(&format!("full: GET /banks/t/memories/{id}")),
            "the escape hatch must name this row's own full form: {content}"
        );
        assert_eq!(headers.get("x-truncated").expect("count"), "1");
        assert_eq!(headers.get("x-total-count").expect("count"), "3");
    }

    #[tokio::test]
    async fn truncate_should_leave_a_row_that_fits_untouched() {
        let svc = wide_bank();
        let (_, body) = split(
            call_list(
                &svc,
                "t",
                MemoriesQuery { truncate: Some(9_000), ..Default::default() },
            )
            .await,
        )
        .await;
        let rows: Vec<serde_json::Value> = serde_json::from_slice(&body).expect("array");
        assert!(
            rows.iter().all(|r| !r["content"].as_str().unwrap_or("").contains("truncated")),
            "a cap the whole page fits under must cut nothing"
        );
    }

    /// A summary capsule is the first line, sentence-cut at the cap, with the
    /// original length and fetch command when anything was hidden (H8). Every
    /// rule below is the pre-registered structural one — no LLM, no config.
    #[test]
    fn summary_content_should_serve_the_first_line_and_pointer() {
        let m = Memory {
            id: "m1".into(),
            bank_id: "b".into(),
            content: "gold marker: the auth stack.\nThe second line hides.\nThird.".into(),
            context: None,
            created_at: None,
            meta: crate::memory::MemoryMeta::default(),
        };
        let served = summary_content(&m, "b");
        assert!(served.starts_with("gold marker: the auth stack."), "{served}");
        assert!(
            served.contains("[summary of ") && served.contains("chars; full: GET /banks/b/memories/m1]"),
            "the hidden remainder must be one GET away: {served}"
        );
    }

    #[test]
    fn summary_content_should_cut_a_long_line_at_its_first_sentence_boundary() {
        let long = format!("{} {} {}", "decision made here.".repeat(20), "later tail", "x".repeat(400));
        let m = Memory {
            id: "m2".into(),
            bank_id: "b".into(),
            content: long,
            context: None,
            created_at: None,
            meta: crate::memory::MemoryMeta::default(),
        };
        let served = summary_content(&m, "b");
        let capsule_end = served.find("…[summary").expect("marker");
        // The capsule proper (everything before the marker) ends on a sentence
        // boundary inside the cap.
        let capsule = &served[..capsule_end];
        assert!(capsule.chars().count() <= SUMMARY_CAP_CHARS, "{capsule}");
        assert!(capsule.ends_with('.') || capsule.ends_with('!') || capsule.ends_with('?'), "{capsule}");
    }

    #[test]
    fn summary_content_should_hard_cut_a_boundaryless_line() {
        let m = Memory {
            id: "m3".into(),
            bank_id: "b".into(),
            content: "n".repeat(500),
            context: None,
            created_at: None,
            meta: crate::memory::MemoryMeta::default(),
        };
        let served = summary_content(&m, "b");
        let capsule_end = served.find("…[summary").expect("marker");
        assert_eq!(capsule_end, SUMMARY_CAP_CHARS, "hard cut at the cap");
    }

    #[test]
    fn summary_content_should_not_marker_a_memory_that_is_its_own_capsule() {
        let m = Memory {
            id: "m4".into(),
            bank_id: "b".into(),
            content: "one line, short".into(),
            context: None,
            created_at: None,
            meta: crate::memory::MemoryMeta::default(),
        };
        assert_eq!(summary_content(&m, "b"), "one line, short");
        // Empty content serves empty, borrowed, no marker.
        let empty = Memory { content: String::new(), ..m };
        assert_eq!(summary_content(&empty, "b"), "");
    }

    #[test]
    fn summary_content_should_count_characters_and_not_bytes() {
        // Every character is two bytes: a byte-indexed boundary search would cut
        // mid-character.
        let m = Memory {
            id: "m5".into(),
            bank_id: "b".into(),
            content: format!("{}{}", "é".repeat(100), "\nsecond line"),
            context: None,
            created_at: None,
            meta: crate::memory::MemoryMeta::default(),
        };
        let served = summary_content(&m, "b");
        assert!(served.starts_with(&"é".repeat(100)), "{served}");
    }

    #[tokio::test]
    async fn truncate_should_count_characters_and_not_bytes() {
        let svc = Arc::new(svc_with_banks(&["t"]));
        // Every character here is two bytes, so a byte cap would land mid-character.
        let content = "é".repeat(100);
        svc.retain("t", &content, None).expect("retain");
        let (_, body) = split(
            call_list(
                &svc,
                "t",
                MemoriesQuery { truncate: Some(10), ..Default::default() },
            )
            .await,
        )
        .await;
        let rows: Vec<serde_json::Value> = serde_json::from_slice(&body).expect("array");
        let served = rows[0]["content"].as_str().expect("content");
        assert!(served.starts_with(&"é".repeat(10)), "{served}");
        assert!(served.contains("truncated from 100 chars"), "{served}");
    }

    #[tokio::test]
    async fn the_empty_list_should_stay_a_bare_array_by_default() {
        let svc = Arc::new(svc_with_banks(&["empty"]));
        let (headers, body) = split(call_list(&svc, "empty", MemoriesQuery::default()).await).await;
        assert_eq!(body, b"[]");
        assert_eq!(headers.get("x-total-count").expect("count"), "0");
    }

    #[tokio::test]
    async fn a_toon_list_should_be_tabular_and_carry_the_total() {
        let svc = Arc::new(svc_with_banks(&["t"]));
        svc.retain("t", "auth uses jose", None).expect("retain");
        svc.retain("t", "the deploy key rotates on the first", None)
            .expect("retain");
        let res = call_list(
            &svc,
            "t",
            MemoriesQuery { format: Some(OutputFormat::Toon), ..Default::default() },
        )
        .await;
        let headers = {
            let (h, _) = split(res).await;
            h
        };
        assert_eq!(
            headers.get(CONTENT_TYPE).expect("content-type"),
            TOON_CONTENT_TYPE
        );
        let out = text(
            call_list(
                &svc,
                "t",
                MemoriesQuery { format: Some(OutputFormat::Toon), ..Default::default() },
            )
            .await,
        )
        .await;
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines[0], "total: 2");
        assert_eq!(lines[1], "memories[2]{id,created_at,content}:");
        // The two rows, each declaring its own length in the header above.
        assert!(lines[2].starts_with("  "), "{out}");
        assert!(lines[2].contains(",auth uses jose"), "{out}");
        assert!(!out.contains("truncated"), "{out}");
        assert!(!out.ends_with('\n'), "§12: no trailing newline");
    }

    #[tokio::test]
    async fn an_empty_toon_list_should_state_the_zero_and_the_success() {
        let svc = Arc::new(svc_with_banks(&["empty"]));
        let out = text(
            call_list(
                &svc,
                "empty",
                MemoriesQuery { format: Some(OutputFormat::Toon), ..Default::default() },
            )
            .await,
        )
        .await;
        // §9.1's `name: []` on its own, under an explicit count: the array form
        // says the collection is empty and the count says there is nothing hidden
        // behind a page either.
        assert_eq!(out, "total: 0\nmemories: []");
    }

    #[tokio::test]
    async fn a_toon_page_mixing_context_rows_should_fall_back_to_list_form() {
        let svc = Arc::new(svc_with_banks(&["t"]));
        svc.retain("t", "plain row", None).expect("retain");
        svc.retain_doc("t", "row with context", Some("saw the prompt".into()), &[], None, UpdateMode::Replace)
            .expect("retain with context");
        let out = text(
            call_list(
                &svc,
                "t",
                MemoriesQuery { format: Some(OutputFormat::Toon), ..Default::default() },
            )
            .await,
        )
        .await;
        // §9.3 requires one key set across the whole array, so this page is not a
        // table and must not pretend to be one.
        assert!(out.contains("memories[2]:"), "{out}");
        assert!(!out.contains('{'), "a non-uniform page takes the list form: {out}");
        assert!(out.contains("  - id: "), "{out}");
        assert!(out.contains("    context: saw the prompt"), "{out}");
    }

    #[tokio::test]
    async fn a_toon_list_should_carry_its_truncation_notice_in_band() {
        let svc = wide_bank();
        let out = text(
            call_list(
                &svc,
                "t",
                MemoriesQuery {
                    truncate: Some(40),
                    format: Some(OutputFormat::Toon),
                    ..Default::default()
                },
            )
            .await,
        )
        .await;
        // No headers to fall back on, so both facts are in the body.
        assert!(out.contains("total: 3"), "{out}");
        assert!(out.contains("content_truncated: 1"), "{out}");
        assert!(out.contains("truncated from 9000 chars"), "{out}");
        assert!(out.contains("full: GET /banks/t/memories/"), "{out}");
    }

    #[test]
    fn an_unknown_format_should_be_refused_rather_than_served_as_json() {
        // The closed enum is what makes `?format=xml` a 400 from the query
        // extractor instead of a silent JSON body: no new status, no new message.
        assert!(serde_json::from_str::<OutputFormat>("\"toon\"").is_ok());
        assert!(serde_json::from_str::<OutputFormat>("\"json\"").is_ok());
        assert!(serde_json::from_str::<OutputFormat>("\"xml\"").is_err());
        assert!(serde_json::from_str::<OutputFormat>("\"TOON\"").is_err());
    }

    #[tokio::test]
    async fn the_single_memory_route_should_answer_both_formats() {
        let svc = Arc::new(svc_with_banks(&["t"]));
        let id = svc.retain("t", "auth uses jose", None).expect("retain");

        let (json_headers, json_body) =
            split(call_get(&svc, "t", &id, FormatQuery::default()).await.expect("found")).await;
        let json: serde_json::Value = serde_json::from_slice(&json_body).expect("object");
        assert_eq!(json["id"], serde_json::json!(id));
        assert_eq!(json["content"], serde_json::json!("auth uses jose"));
        assert_eq!(json_headers.get(CONTENT_TYPE).expect("content-type"), "application/json");

        let toon_res = call_get(
            &svc,
            "t",
            &id,
            FormatQuery { format: Some(OutputFormat::Toon) },
        )
        .await
        .expect("found");
        let (toon_headers, _) = split(toon_res).await;
        assert_eq!(toon_headers.get(CONTENT_TYPE).expect("content-type"), TOON_CONTENT_TYPE);
        let out = text(
            call_get(&svc, "t", &id, FormatQuery { format: Some(OutputFormat::Toon) })
                .await
                .expect("found"),
        )
        .await;
        assert!(out.starts_with(&format!("id: {id}\n")), "{out}");
        assert!(out.contains("\ncontent: auth uses jose"), "{out}");
    }

    #[tokio::test]
    async fn the_single_memory_route_should_keep_its_404_for_an_unknown_id() {
        let svc = Arc::new(svc_with_banks(&["t"]));
        let (status, msg) = call_get(&svc, "t", "nope", FormatQuery::default())
            .await
            .expect_err("absent");
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(msg, "unknown memory");
    }

    #[tokio::test]
    async fn the_single_memory_route_should_ignore_a_page_parameter_it_never_had() {
        // It takes only `format`, so `?limit=abc` stays the ignored query
        // parameter it has always been here rather than becoming a new 400.
        let svc = Arc::new(svc_with_banks(&["t"]));
        let id = svc.retain("t", "auth uses jose", None).expect("retain");
        let res = list_memories(
            State(Arc::clone(&svc)),
            Path("t".to_string()),
            Query(MemoriesQuery { limit: Some(1), ..Default::default() }),
        )
        .await
        .expect("valid bank lists");
        assert!(!text(res).await.is_empty());
        assert!(call_get(&svc, "t", &id, FormatQuery::default()).await.is_ok());
    }

    #[tokio::test]
    async fn limit_and_offset_should_keep_their_established_bounds() {
        let svc = Arc::new(svc_with_banks(&["t"]));
        for i in 0..3 {
            svc.retain("t", &format!("row {i}"), None).expect("retain");
        }
        let rows = |q: MemoriesQuery| async {
            let body = text(call_list(&svc, "t", q).await).await;
            body.matches("\"content\"").count()
        };
        // Default page size.
        assert_eq!(rows(MemoriesQuery::default()).await, 3);
        // The cap, and the offset that walks the bank with it.
        assert_eq!(rows(MemoriesQuery { limit: Some(1), ..Default::default() }).await, 1);
        assert_eq!(rows(MemoriesQuery { limit: Some(1), offset: Some(2), ..Default::default() }).await, 1);
        assert_eq!(rows(MemoriesQuery { offset: Some(99), ..Default::default() }).await, 0);
        assert_eq!(
            page_bounds(Some(usize::MAX), None).0,
            MAX_PAGE,
            "an absurd limit is capped, not honoured"
        );
    }

    // `format: "full"` is the citing shape, so the number in it has to be the
    // one the ranking used. An overlap count read as "relevance" is a
    // different quantity on a different scale, and a caller sorting on it would
    // be sorting on something the retriever never computed.
    #[test]
    fn recall_should_score_with_the_fused_rrf_value() {
        let svc = svc_with_banks(&["b"]);
        svc.retain("b", "auth uses jose middleware", None).expect("hit");
        svc.retain("b", "cafeteria menu noodles", None).expect("miss");

        let hits = svc.recall("b", "jose", 2000).expect("recall");
        assert_eq!(hits.len(), 1, "only the memory mentioning jose is a candidate");
        // Rank 1 in the BM25 stream and rank 1 in the overlap stream: the fused
        // score is the *weighted* sum of both contributions, not either count.
        // Built from the shipped weights rather than written out, so changing
        // the default (as Phase E1 did) cannot leave a stale constant here —
        // this assertion is about the shape of the sum, and it keeps testing that
        // shape rather than a number that used to be true.
        let w = FusionWeights::SHIPPED;
        let expected = w.bm25 / (w.k + 1.0) + w.overlap / (w.k + 1.0);
        assert_eq!(hits[0].score, expected);
        // An overlap count would have been 1 here, and the fused value is not
        // that — the assertion above is the point, this one pins the scale.
        assert!(hits[0].score < 1.0, "RRF scores are reciprocal-rank sums, not counts");
    }

    // Scores have to order results the way the ranking did, which is the only
    // reason a caller is given one.
    #[test]
    fn recall_scores_should_order_the_results_the_same_way_the_fusion_did() {
        let svc = svc_with_banks(&["b"]);
        for content in [
            "jose jwt middleware in the auth path",
            "jose is also mentioned by the deploy notes",
            "mentions jose exactly once",
        ] {
            svc.retain("b", content, None).expect("retain");
        }
        let hits = svc.recall("b", "jose", 10_000).expect("recall");
        assert_eq!(hits.len(), 3);
        for pair in hits.windows(2) {
            assert!(
                pair[0].score >= pair[1].score,
                "results must come back in descending fused score: {:?}",
                hits.iter().map(|h| (h.memory.content.as_str(), h.score)).collect::<Vec<_>>()
            );
        }
    }

    fn tags(list: &[&str]) -> Vec<String> {
        list.iter().map(|t| t.to_string()).collect()
    }

    #[test]
    fn normalize_tags_should_trim_lowercase_dedupe_and_cap() {
        assert_eq!(
            normalize_tags(&tags(&["  Auth ", "AUTH", "jose", "", "   "])),
            tags(&["auth", "jose"])
        );
        let many: Vec<String> = (0..MAX_TAGS + 5).map(|i| format!("t{i}")).collect();
        let capped = normalize_tags(&many);
        assert_eq!(capped.len(), MAX_TAGS);
        assert_eq!(capped[0], "t0", "the cap keeps the head, not an arbitrary subset");
    }

    #[test]
    fn recall_should_filter_by_any_matching_tag() {
        let svc = svc_with_banks(&["b"]);
        svc.retain_tagged("b", "auth uses jose middleware", None, &tags(&["auth"]))
            .expect("retain auth");
        svc.retain_tagged("b", "auth uses jose secrets", None, &tags(&["ops", "secrets"]))
            .expect("retain ops");
        svc.retain("b", "auth uses jose untagged", None).expect("retain none");

        // Unfiltered recall is unchanged: every memory still comes back.
        assert_eq!(svc.recall_filtered("b", "jose", None, &[], &[]).expect("all").len(), 3);
        // Any-match: one hit per matching tag, and an untagged memory never does.
        for filter in ["auth", "ops", "secrets"] {
            assert_eq!(
                svc.recall_filtered("b", "jose", None, &tags(&[filter]), &[])
                    .expect("filter")
                    .len(),
                1,
                "filter {filter} should match exactly one memory"
            );
        }
        assert!(svc
            .recall_filtered("b", "jose", None, &tags(&["nothing-has-this"]), &[])
            .expect("miss")
            .is_empty());
    }

    /// GAP-3: the exclusion half of tag filtering. Dropped rows neither vote
    /// nor can be injected; absent exclusion is byte-identical to before the
    /// field existed; an exclusion naming nothing excludes nothing — the safe
    /// reading, unlike `tags`' fail-closed one.
    #[test]
    fn exclude_tags_should_drop_tagged_rows_and_keep_the_rest() {
        let svc = svc_with_banks(&["b"]);
        svc.retain_tagged("b", "auth uses jose transcript", None, &tags(&["transcript"]))
            .expect("retain transcript");
        svc.retain_tagged("b", "auth uses jose live", None, &tags(&["auth"]))
            .expect("retain live");

        assert_eq!(svc.recall_filtered("b", "jose", None, &[], &[]).expect("all").len(), 2);
        let hits = svc
            .recall_filtered("b", "jose", None, &[], &tags(&["transcript"]))
            .expect("excluded");
        assert_eq!(hits.len(), 1, "{hits:?}");
        assert!(hits[0].memory.content.contains("live"), "{hits:?}");
        assert_eq!(
            svc.recall_filtered("b", "jose", None, &[], &tags(&["nothing-has-this"]))
                .expect("no-op")
                .len(),
            2
        );
    }

    #[test]
    fn retain_should_union_request_tags_with_bank_retain_tags() {
        let svc = svc_with_banks(&["b"]);
        svc.set_bank_config("b", r#"{"retainTags":["Bank","ops"]}"#).expect("config");
        svc.retain_tagged("b", "auth uses jose middleware", None, &tags(&["  Req  "]))
            .expect("retain");

        // The request tag is normalized into the same namespace as the bank's.
        for filter in ["req", "bank", "ops"] {
            assert_eq!(
                svc.recall_filtered("b", "jose", None, &tags(&[filter]), &[])
                    .expect("filter")
                    .len(),
                1,
                "{filter} should reach the memory"
            );
        }
        // A bank-wide default applies to every later retain, not just this one.
        svc.retain("b", "auth uses jose second", None).expect("plain retain");
        assert_eq!(
            svc.recall_filtered("b", "jose", None, &tags(&["bank"]), &[])
                .expect("bank")
                .len(),
            2
        );
        assert!(svc
            .recall_filtered("b", "jose", None, &tags(&["req"]), &[])
            .expect("req")
            .len()
            == 1);
    }

    /// The overlap stream is **not** a "no query terms match" fallback: it drops
    /// every zero-overlap document, so a query nothing matches leaves both
    /// streams empty and the recall empty. Pinned because the obvious reason to
    /// keep a non-zero overlap weight is exactly that fallback, and it does not
    /// exist — a weight change must not be justified by it.
    #[test]
    fn a_query_nothing_matches_should_return_nothing() {
        let s = SqliteStore::open_in_memory().expect("open");
        s.put_bank(&Bank { id: "b".to_string(), name: "b".to_string() })
            .expect("bank");
        let svc = MemoryService::new(s);
        svc.retain("b", "auth uses jose", None).expect("retain");
        svc.retain("b", "deploy the redis cache", None).expect("retain");
        for w in [FusionWeights::SHIPPED, FusionWeights { overlap: 0.25, ..FusionWeights::SHIPPED }] {
            let hits = svc
                .recall_with_weights("b", "kubernetes helm rollout", 2000, &w)
                .expect("recall");
            assert!(hits.is_empty(), "nothing matches {hits:?} at {w:?}");
        }
    }


    /// The one job BM25 cannot do: **more than 50 candidates.** BM25 is `LIMIT
    /// FTS_LIMIT` = 50 in SQL, so a query matching more than 50 rows truncates
    /// there. The overlap stream is capped at 200, so it is the only thing that
    /// can put rows 51–200 in front of the caller.
    ///
    /// This is the structural cost of an `overlap` weight of exactly 0.0, and it
    /// is one the LongMemEval suite **cannot** see: 38–62 sessions per bank with
    /// BM25 truncating at 50 means the suite has no query that matches more than
    /// 50 rows, which is why its highest-scoring row is the one that drops the
    /// stream. `eval/BENCH_RECALL_CURVE.md` (1k–100k banks) is the harness that
    /// does exercise it.
    #[test]
    fn a_query_matching_more_rows_than_bm25_returns_must_still_surface_the_overflow() {
        let s = SqliteStore::open_in_memory().expect("open");
        s.put_bank(&Bank { id: "b".to_string(), name: "b".to_string() })
            .expect("bank");
        let svc = MemoryService::new(s);
        // 120 rows, all matching `shard`; the needle is the last one written, so
        // it is outside BM25's 50 by construction.
        for i in 0..120 {
            svc.retain("b", &format!("shard replica {i} rollout note"), None)
                .expect("retain");
        }
        let needle = svc.retain("b", "shard replica 120 rollout note", None).expect("retain");
        let ids = |rows: Vec<ScoredMemory>| {
            rows.into_iter().map(|r| r.memory.id).collect::<Vec<_>>()
        };
        let shipped = ids(svc.recall("b", "shard", 100_000).expect("recall"));
        assert!(
            shipped.len() > 50,
            "BM25 truncates at 50, so a non-zero overlap weight is what carries the overflow; got {}",
            shipped.len()
        );
        assert!(!shipped.contains(&needle), "the needle is not a gold row here, only an overflow witness");
        // And the contrast that makes it a real test rather than a comment.
        let bm25_only = FusionWeights { overlap: 0.0, ..FusionWeights::SHIPPED };
        let dropped = ids(
            svc.recall_with_weights("b", "shard", 100_000, &bm25_only)
                .expect("recall"),
        );
        assert_eq!(
            dropped.len(),
            50,
            "a zero overlap weight must truncate the candidate set at BM25's LIMIT 50"
        );
    }

    /// The shipped default must be a non-zero overlap weight, and every other
    /// field must still be the plain value the sweep left alone.
    ///
    /// This is the one place the E1 decision is pinned in code, so a later
    /// "let's just round this to 1.0" edit fails here instead of quietly shipping
    /// the 4.0pp R@5 regression the sweep measured against. The *value* lives in
    /// [`FusionWeights::SHIPPED`]; this asserts the shape that value must keep.
    ///
    /// The four fields below the RRF ones are the inert defaults of the
    /// mechanisms in `recall.rs` — BM25 magnitude, the recency stream, the recency
    /// policy, and the overlap scope. They are spelled out here rather than
    /// defaulted so that *turning one on* has to delete a line in this test
    /// instead of happening silently, which is the whole point of shipping them
    /// off while `AGENTS.md` §1 forbids choosing their values.
    #[test]
    fn the_shipped_fusion_weights_should_keep_the_swept_overlap_weight() {
        assert_eq!(
            FusionWeights::SHIPPED,
            FusionWeights {
                bm25: 1.0,
                overlap: 0.25,
                agreement: 0.0,
                k: 60.0,
                bm25_magnitude: 0.0,
                recency: 0.0,
                recency_half_life_days: crate::recall::RECENCY_HALF_LIFE_PLACEHOLDER_DAYS,
                recency_policy: RecencyPolicy::Always,
                overlap_scope: OverlapScope::Document,
                // Phase D's fourth stream, shipped off. Listed here rather than
                // elided with `..SHIPPED` because the point of this test is that
                // the literal *is* the shipped value: a field that only ever
                // appears through a struct update could be set to anything without
                // this test noticing.
                vector: 0.0,
                keyword_scope: crate::recall::KeywordScope::Exact,
            },
            "the shipped default moved off the configuration E1 measured"
        );
        assert_ne!(
            FusionWeights::SHIPPED.overlap, 1.0,
            "equal weighting was measured 4.0pp worse on R@5; see docs/NEXT_ITERATION.md"
        );
        // The mechanisms exist, so their off switches are the only thing keeping
        // them inert, and those are asserted here at the default as well as by
        // equality above.
        assert_eq!(FusionWeights::SHIPPED.bm25_magnitude, 0.0);
        assert_eq!(FusionWeights::SHIPPED.recency, 0.0);
        assert_eq!(FusionWeights::default(), FusionWeights::SHIPPED);
    }

    /// The shipped default must be the equal-weight-shaped fusion through the
    /// public path, so a future edit that reweights recall *without* touching
    /// `rrf_fuse` is caught too.
    #[test]
    fn the_shipped_recall_should_be_the_default_fusion() {
        let s = SqliteStore::open_in_memory().expect("open");
        s.put_bank(&Bank { id: "b".to_string(), name: "b".to_string() })
            .expect("bank");
        let svc = MemoryService::new(s);
        for c in [
            "auth uses jose for the session cookie",
            "deploy the redis cache at the edge",
            "latency index shard replica",
        ] {
            svc.retain("b", c, None).expect("retain");
        }
        let shipped = svc.recall("b", "session cookie latency", 2000).expect("recall");
        let explicit = svc
            .recall_with_weights("b", "session cookie latency", 2000, &FusionWeights::SHIPPED)
            .expect("recall");
        assert_eq!(
            shipped.iter().map(|h| h.memory.id.as_str()).collect::<Vec<_>>(),
            explicit.iter().map(|h| h.memory.id.as_str()).collect::<Vec<_>>()
        );
    }

    #[test]
    fn recall_budget_should_prefer_explicit_then_bank_then_default() {
        let svc = svc_with_banks(&["cfg", "plain"]);
        svc.set_bank_config("cfg", r#"{"recallMaxTokens":5}"#).expect("config");
        // ~2000 chars: well past both a 5- and a 10-token budget, well inside the
        // 2000-token server default.
        let long = "jose ".repeat(400);
        for bank in ["cfg", "plain"] {
            svc.retain(bank, &long, None).expect("retain");
        }

        let len = |hits: Vec<ScoredMemory>| hits[0].memory.content.chars().count();
        let bank_default = len(svc.recall_filtered("cfg", "jose", None, &[], &[]).expect("bank"));
        let explicit = len(svc.recall_filtered("cfg", "jose", Some(10), &[], &[]).expect("explicit"));
        let server_default = len(svc.recall_filtered("plain", "jose", None, &[], &[]).expect("default"));

        assert_eq!(bank_default, 5 * 4, "bank recallMaxTokens must be the default");
        assert_eq!(explicit, 10 * 4, "an explicit budget must win");
        assert_eq!(server_default, long.chars().count(), "no config falls back to the server default");
        assert!(bank_default < explicit && explicit < server_default);
    }

    #[test]
    fn bank_config_should_roundtrip_and_reject_bad_input_with_a_4xx() {
        let svc = svc_with_banks(&["b"]);
        let raw = r#"{"retain_mission":"own the release","recallMaxTokens":128,
                      "recallPromptPreamble":"From the project bank:","retainTags":["ops"]}"#;
        svc.set_bank_config("b", raw).expect("set");
        // Unknown keys and whitespace are served back exactly as stored.
        assert_eq!(svc.bank_config("b").expect("get").as_str(), raw);

        // A whole-object replace, not a merge: the previous value is gone.
        svc.set_bank_config("b", r#"{"recallMaxTokens":8}"#).expect("replace");
        assert_eq!(svc.bank_config("b").expect("get").as_str(), r#"{"recallMaxTokens":8}"#);

        let (status, msg) = http_error(&svc.set_bank_config("b", "{\"a\":").expect_err("bad json"));
        assert_eq!((status, msg), (StatusCode::BAD_REQUEST, "invalid bank config"));
        for bad in ["[1,2]", "\"str\"", "null"] {
            assert_eq!(
                http_error(&svc.set_bank_config("b", bad).expect_err(bad)).0,
                StatusCode::BAD_REQUEST
            );
        }
        assert_eq!(
            http_error(&svc.set_bank_config("ghost", "{}").expect_err("ghost")),
            (StatusCode::NOT_FOUND, "unknown bank")
        );
        assert_eq!(
            http_error(&svc.bank_config("ghost").expect_err("ghost")).0,
            StatusCode::NOT_FOUND
        );
    }

    #[test]
    fn a_malformed_config_key_should_fall_back_instead_of_breaking_recall() {
        let svc = svc_with_banks(&["b"]);
        svc.set_bank_config("b", r#"{"recallMaxTokens":"lots","retainTags":"nope"}"#)
            .expect("config");
        svc.retain("b", "auth uses jose middleware", None).expect("retain");
        // The bad keys are ignored, so the server default applies and recall works.
        let hits = svc.recall_filtered("b", "jose", None, &[], &[]).expect("recall");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].memory.content, "auth uses jose middleware");
    }

    // ── W2 mount profiles ──────────────────────────────────────────────

    #[test]
    fn a_read_only_bank_should_refuse_every_write_at_every_surface() {
        let svc = svc_with_banks(&["ro"]);

        // Seed one row *before* the flag goes on, so the custody writes have
        // something real to refuse. This is the case the earlier version of
        // this test missed: it only exercised `retain`, so it went green while
        // DELETE and supersede happily emptied the bank.
        svc.retain("ro", "jose is great", None).expect("seed before read-only");
        let id = svc.list_memories("ro", 10, 0).expect("list").remove(0).id;
        svc.set_bank_config("ro", r#"{"readOnly":true}"#).expect("set read-only");

        let denied = |e: ApiError| {
            assert_eq!(http_error(&e), (StatusCode::BAD_REQUEST, "bank is read-only"))
        };

        // 1. retain (HTTP and MCP both route through here).
        denied(svc.retain("ro", "jose is great", None).expect_err("retain must refuse"));

        // 2. delete. The destructive half the flag exists for.
        denied(svc.delete_memory("ro", &id).expect_err("delete must refuse"));

        // 3. supersede. Hiding a row from default recall is a write.
        denied(svc.supersede_memory("ro", &id, None).expect_err("supersede must refuse"));

        // 4. redact. Rewriting stored bytes in place is a write.
        denied(svc.redact_memory("ro", &id, "jose", "probe").expect_err("redact must refuse"));

        // Nothing changed: the row is still there and still readable. A guard
        // that refused the call but let the write through would pass every
        // assertion above, so the state is the real test.
        let after = svc.list_memories("ro", 10, 0).expect("list after");
        assert_eq!(after.len(), 1, "the seeded row survived every refused write");
        assert_eq!(after[0].content, "jose is great");
        assert!(svc.get_memory("ro", &id).expect("get").is_some());
        // And it is still in default recall, i.e. the supersede did not land.
        assert!(!svc.recall("ro", "jose", 100).expect("recall").is_empty());
    }

    #[test]
    fn a_writable_bank_keeps_every_custody_write() {
        // The mirror of the test above: with no `readOnly` the same four calls
        // must all still work. A guard that refused everything would pass the
        // read-only test and break every real bank.
        let svc = svc_with_banks(&["rw"]);
        let a = svc.retain("rw", "alpha", None).expect("retain a");
        let b = svc.retain("rw", "beta", None).expect("retain b");

        svc.supersede_memory("rw", &a, Some(&b)).expect("supersede");
        assert!(svc.get_memory("rw", &a).expect("get").expect("some").meta.superseded_by.is_some());

        svc.redact_memory("rw", &b, "beta", "probe").expect("redact");
        assert!(svc.get_memory("rw", &b).expect("get").expect("some").content.contains("[REDACTED:probe]"));

        assert!(svc.delete_memory("rw", &a).expect("delete"));
        assert!(!svc.delete_memory("rw", &a).expect("second delete finds nothing"));
    }

    #[test]
    fn a_bank_without_read_only_should_keep_the_shipped_write_path() {
        let svc = svc_with_banks(&["b"]);
        // The default (no config) writes fine.
        svc.retain("b", "jose is great", None).expect("retain");
        // So does a bank whose config spells readOnly wrong — the typo must not
        // take writes down, the same fail-open rule every BankConfig key has.
        svc.set_bank_config("b", r#"{"readOnly":"yes"}"#).expect("set");
        svc.retain("b", "jose again", None).expect("retain after typo");
    }

    #[test]
    fn a_write_above_the_trust_ceiling_should_downgrade_and_land() {
        let svc = svc_with_banks(&["b"]);
        svc.set_bank_config("b", r#"{"maxTrust":"human_stated"}"#).expect("set");
        // A verified write against a human_stated ceiling is downgraded one
        // step (verified -> human_stated) and lands, never refused.
        let id = svc
            .retain_doc_at_trusted(
                "b",
                "the auth stack uses jose",
                None,
                &[],
                None,
                UpdateMode::Replace,
                None,
                Trust::Verified,
            )
            .expect("downgrade and land");
        let got = svc.get_memory("b", &id).expect("get").expect("some");
        assert_eq!(got.meta.trust.0, "human_stated", "verified downgraded to human_stated");
    }

    #[test]
    fn a_write_at_or_below_the_trust_ceiling_should_land_verbatim() {
        let svc = svc_with_banks(&["b"]);
        svc.set_bank_config("b", r#"{"maxTrust":"verified"}"#).expect("set");
        let id = svc
            .retain_doc_at_trusted(
                "b",
                "the auth stack uses jose",
                None,
                &[],
                None,
                UpdateMode::Replace,
                None,
                Trust::Verified,
            )
            .expect("verified under a verified ceiling");
        let got = svc.get_memory("b", &id).expect("get").expect("some");
        assert_eq!(got.meta.trust.0, "verified");

        // And an agent_inferred write under a verified ceiling is also verbatim
        // — the ceiling only bites when the write exceeds it.
        let id2 = svc
            .retain_doc_at_trusted(
                "b",
                "a hunch",
                None,
                &[],
                None,
                UpdateMode::Replace,
                None,
                Trust::AgentInferred,
            )
            .expect("agent_inferred under a verified ceiling");
        let got2 = svc.get_memory("b", &id2).expect("get").expect("some");
        assert_eq!(got2.meta.trust.0, "agent_inferred");
    }

    #[test]
    fn a_bank_without_a_trust_ceiling_should_store_every_tier_verbatim() {
        let svc = svc_with_banks(&["b"]);
        // No config at all: the default ceiling is "no ceiling".
        let id = svc
            .retain_doc_at_trusted(
                "b",
                "the auth stack uses jose",
                None,
                &[],
                None,
                UpdateMode::Replace,
                None,
                Trust::Verified,
            )
            .expect("verified with no ceiling");
        let got = svc.get_memory("b", &id).expect("get").expect("some");
        assert_eq!(got.meta.trust.0, "verified", "no ceiling stores verbatim");
    }

    #[test]
    fn an_unknown_trust_tier_in_config_should_read_as_no_ceiling() {
        let svc = svc_with_banks(&["b"]);
        svc.set_bank_config("b", r#"{"maxTrust":"totally_bogus"}"#).expect("set");
        // An unrecognised tier is the same fail-open default every other key
        // has: a typo must not take writes down.
        let id = svc
            .retain_doc_at_trusted(
                "b",
                "the auth stack uses jose",
                None,
                &[],
                None,
                UpdateMode::Replace,
                None,
                Trust::Verified,
            )
            .expect("verified under a bogus ceiling");
        let got = svc.get_memory("b", &id).expect("get").expect("some");
        assert_eq!(got.meta.trust.0, "verified", "a bogus ceiling stores verbatim");
    }

    #[test]
    fn trust_round_trips_through_the_string_form() {
        for t in [Trust::AgentInferred, Trust::HumanStated, Trust::Verified] {
            assert_eq!(Trust::tier(t.as_str()), Some(t), "round-trip {}", t.as_str());
        }
    }

    #[test]
    fn retain_doc_should_replace_the_prior_revision_by_default() {
        let svc = svc_with_banks(&["b"]);
        let old_tags = tags(&["handbook"]);
        let first = svc
            .retain_doc("b", "auth uses jose v1", None, &old_tags, Some("handbook"), UpdateMode::Replace)
            .expect("v1");
        let second = svc
            .retain_doc("b", "auth uses jose v2", None, &tags(&["auth"]), Some("handbook"), UpdateMode::Replace)
            .expect("v2");

        let rows = svc.list_memories("b", 10, 0).expect("list");
        assert_eq!(rows.len(), 1, "a replace leaves exactly one current revision");
        assert_eq!(rows[0].id, second);
        assert!(rows[0].content.ends_with("v2"));
        assert!(svc.get_memory("b", &first).expect("get").is_none(), "the superseded row is gone");
        // Tags are authoritative, not merged: the old revision's tag went with it.
        assert!(svc
            .recall_filtered("b", "jose", None, &old_tags, &[])
            .expect("old tag")
            .is_empty());
        assert_eq!(
            svc.recall_filtered("b", "jose", None, &tags(&["auth"]), &[]).expect("new tag").len(),
            1
        );
    }

    #[test]
    fn retain_doc_append_should_keep_every_row() {
        let svc = svc_with_banks(&["b"]);
        let a = svc
            .retain_doc("b", "turn one jose", None, &[], Some("session-1#1"), UpdateMode::Append)
            .expect("one");
        let b = svc
            .retain_doc("b", "turn two jose", None, &[], None, UpdateMode::Append)
            .expect("two");
        let ids: Vec<String> = svc
            .list_memories("b", 10, 0)
            .expect("list")
            .into_iter()
            .map(|m| m.id)
            .collect();
        assert_eq!(ids.len(), 2, "an append never supersedes a row");
        assert!(ids.contains(&a) && ids.contains(&b));
        assert_eq!(svc.recall("b", "jose", 2000).expect("recall").len(), 2);
    }

    #[test]
    fn an_append_that_reuses_a_document_id_should_be_refused_not_deduped() {
        let svc = svc_with_banks(&["b"]);
        svc.retain_doc("b", "first jose", None, &[], Some("doc"), UpdateMode::Append).expect("first");
        // The unique (bank, document_id) index is what stops a document holding
        // two rows under one key, so the repeat is refused loudly: silently
        // replacing would be the other mode, and silently keeping both would
        // break the unique revision a reader expects.
        let err = svc
            .retain_doc("b", "second jose", None, &[], Some("doc"), UpdateMode::Append)
            .expect_err("duplicate document id");
        assert!(
            matches!(err, ApiError::Store(StoreError::DocumentConflict)),
            "got {err:?}"
        );
        // 409, not 500: the request was well formed and the conflict is
        // client-detectable, so it names the fix instead of calling itself a
        // storage fault.
        let (status, msg) = http_error(&err);
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(msg, "document already exists; use update_mode=replace");
        assert!(msg.contains("replace"), "the body must name the mode that works: {msg}");
        assert!(!msg.contains("SQLITE"), "driver text must not leak: {msg}");

        let left = svc.list_memories("b", 10, 0).expect("list");
        assert_eq!(left.len(), 1, "the refused write left the first row intact");
        assert_eq!(left[0].content, "first jose", "and left it byte-unchanged");
    }

    #[test]
    fn a_blank_document_id_should_be_no_document() {
        let svc = svc_with_banks(&["b"]);
        // Distinct content, so this is about the document id and nothing else:
        // byte-identical content is the same memory and is deduplicated, which
        // would leave this indistinguishable from supersession.
        for (blank, content) in [("  ", "jose note one"), ("", "jose note two")] {
            svc.retain_doc("b", content, None, &[], Some(blank), UpdateMode::Replace)
                .expect(blank);
        }
        assert_eq!(
            svc.list_memories("b", 10, 0).expect("list").len(),
            2,
            "a blank document id supersedes nothing"
        );
    }

    #[test]
    fn empty_content_should_be_rejected_with_a_400() {
        let svc = svc_with_banks(&["b"]);
        for blank in ["", " ", "\t\n"] {
            assert_eq!(
                http_error(&svc.retain("b", blank, None).expect_err(blank)),
                (StatusCode::BAD_REQUEST, "invalid content"),
                "{blank:?} must be named as the client's mistake"
            );
            assert!(matches!(
                svc.retain_doc("b", blank, None, &tags(&["t"]), Some("d"), UpdateMode::Replace),
                Err(ApiError::InvalidContent)
            ));
        }
        // A memory of nothing is dead weight forever, so nothing is stored.
        assert!(svc.list_memories("b", 10, 0).expect("list").is_empty());
    }

    #[test]
    fn reflect_should_answer_from_the_tagged_subset_only() {
        let svc = svc_with_banks(&["b"]);
        svc.retain_tagged("b", "auth uses jose middleware", None, &tags(&["auth"])).expect("auth");
        svc.retain_tagged("b", "auth uses jose deploy notes", None, &tags(&["ops"])).expect("ops");

        let auth = svc.reflect("b", "jose auth", &tags(&["auth"])).expect("auth scope");
        let ops = svc.reflect("b", "jose auth", &tags(&["ops"])).expect("ops scope");
        assert!(auth.contains("middleware"), "got {auth}");
        assert!(ops.contains("deploy notes"), "got {ops}");
        assert_ne!(auth, ops, "each tag scope must cite its own memory");
        // A scope nothing carries has nothing to cite, same as an empty bank.
        assert_eq!(
            svc.reflect("b", "jose", &tags(&["nope"])).expect("no scope"),
            "no relevant memories"
        );
    }

    #[test]
    fn a_filter_that_normalizes_to_nothing_should_return_nothing() {
        let svc = svc_with_banks(&["b"]);
        svc.retain("b", "auth uses jose", None).expect("retain");
        // Fail closed: a filter that names no memory must not widen to the whole
        // bank, which would answer a scoped question with unscoped rows that
        // look exactly like a real result.
        assert!(svc
            .recall_filtered("b", "jose", None, &tags(&["", "   "]), &[])
            .expect("blank filter")
            .is_empty());
        // An absent filter is the whole bank, and stays that way.
        assert_eq!(svc.recall_filtered("b", "jose", None, &[], &[]).expect("empty filter").len(), 1);
        assert_eq!(svc.recall("b", "jose", 2000).expect("unfiltered").len(), 1);
    }

    #[test]
    fn memories_should_be_listed_fetched_and_deleted_inside_one_bank_only() {
        let svc = svc_with_banks(&["a", "b"]);
        let id = svc.retain_tagged("a", "auth uses jose middleware", None, &tags(&["auth"]))
            .expect("retain");

        assert_eq!(svc.get_memory("a", &id).expect("get").expect("some").id, id);
        assert!(svc.get_memory("b", &id).expect("get").is_none(), "another bank's id must not resolve");
        assert!(!svc.delete_memory("b", &id).expect("cross-bank delete"), "another bank must not delete it");
        assert!(svc.get_memory("a", &id).expect("get").is_some(), "the refused delete changed nothing");

        assert!(svc.delete_memory("a", &id).expect("delete"), "a bank-scoped delete reports true");
        assert!(!svc.delete_memory("a", &id).expect("again"), "a second delete finds nothing");
        // The row's tags went with it, so the bank reports neither.
        assert_eq!(svc.bank_stats("a").expect("stats"), BankStats::default());
    }

    #[test]
    fn list_memories_should_page_in_insertion_order() {
        let svc = svc_with_banks(&["b"]);
        for i in 0..5 {
            svc.retain("b", &format!("shared note {i}"), None).expect("retain");
        }
        let page = |limit, offset| -> Vec<String> {
            svc.list_memories("b", limit, offset)
                .expect("page")
                .into_iter()
                .map(|m| m.id)
                .collect()
        };
        assert_eq!(page(2, 0).len(), 2);
        assert_ne!(page(2, 0), page(2, 2), "a second page must not repeat the first");
        assert!(page(2, 5).is_empty(), "an offset past the end is empty");
    }

    #[test]
    fn bank_stats_should_describe_a_service_bank() {
        let svc = svc_with_banks(&["a", "b"]);
        let empty = svc.bank_stats("a").expect("empty");
        assert_eq!(
            (empty.memories, empty.tags, empty.oldest, empty.newest),
            (0, 0, None, None)
        );

        svc.retain_tagged("a", "auth uses jose", None, &tags(&["auth", "jose"])).expect("1");
        svc.retain_tagged("a", "jose deploy notes", None, &tags(&["ops"])).expect("2");
        svc.retain("b", "other bank jose", None).expect("other bank");

        let st = svc.bank_stats("a").expect("stats");
        assert_eq!((st.memories, st.tags), (2, 3), "this bank's rows and distinct tags only");
        let (oldest, newest) = (st.oldest.clone().expect("oldest"), st.newest.clone().expect("newest"));
        assert!(oldest <= newest, "oldest {oldest} must not follow newest {newest}");
        let json = serde_json::to_string(&st).expect("serialize");
        for key in ["memories", "tags", "oldest", "newest"] {
            assert!(json.contains(key), "BankStats must expose {key}: {json}");
        }
    }

    /// A store whose bank-config read fails, and nothing else, so the one path
    /// that reads a config on a read-only request can be exercised end to end.
    struct NoConfigRead(SqliteStore);

    impl Store for NoConfigRead {
        fn put_bank(&self, b: &Bank) -> Result<(), StoreError> {
            self.0.put_bank(b)
        }
        fn put(&self, m: &Memory) -> Result<(), StoreError> {
            self.0.put(m)
        }
        fn get(&self, b: &str, i: &str) -> Result<Option<Memory>, StoreError> {
            self.0.get(b, i)
        }
        fn list(&self, b: &str) -> Result<Vec<Memory>, StoreError> {
            self.0.list(b)
        }
        fn get_bank_config(&self, _bank_id: &str) -> Result<Option<String>, StoreError> {
            Err(StoreError::Sqlite(rusqlite::Error::InvalidQuery))
        }
        fn recall_inputs(
            &self,
            bank_id: &str,
            query: &str,
            tags: &[String],
            fts_limit: usize,
        ) -> Result<crate::store::RecallInputs, StoreError> {
            self.0.recall_inputs(bank_id, query, tags, fts_limit)
        }
    }

    // A failed config read on the recall budget must not become a *successful*
    // recall.
    //
    // It used to: the budget was resolved through
    // `config_of(bank_id).ok().and_then(…)`, which threw away a real
    // `StoreError` and substituted the default. A disk or config fault
    // therefore answered `200` with results trimmed to a budget the bank never
    // asked for — a wrong answer wearing a success status, which the caller had
    // no way to detect. `retain_doc` already propagated the same call.
    #[test]
    fn a_failed_config_read_should_fail_the_recall_that_needed_it() {
        let svc = MemoryService::new(NoConfigRead(SqliteStore::open_in_memory().expect("open")));
        // Seeded through the inner store, not `retain`: this backend's config
        // read is broken from the start, and the point is the *recall* path, not
        // the retain that also reads a config for `retainTags`.
        svc.store.0.put_bank(&Bank { id: "b".to_string(), name: "b".to_string() }).expect("bank");
        svc.store
            .0
            .put(&Memory {
                id: "m1".to_string(),
                bank_id: "b".to_string(),
                content: "auth uses jose middleware".to_string(),
                context: None,
                created_at: None,
                meta: crate::memory::MemoryMeta::default(),
            })
            .expect("put");

        // The bank default cannot be read, so the recall cannot know its budget.
        let err = svc
            .recall_filtered("b", "jose", None, &[], &[])
            .expect_err("a swallowed config error must not answer 200 with the wrong budget");
        assert!(matches!(err, ApiError::Store(StoreError::Sqlite(_))), "got {err:?}");
        // …and it is the documented opaque 500, because that is what every
        // storage failure has always been to a client.
        assert_eq!(
            http_error(&err),
            (StatusCode::INTERNAL_SERVER_ERROR, "storage error")
        );

        // An explicit budget skips the config read entirely, so it still works —
        // the fix is not "recall now needs the config", it is "recall does not
        // lie about the config it could not read".
        assert_eq!(
            svc.recall_filtered("b", "jose", Some(2000), &[], &[]).expect("explicit budget").len(),
            1
        );
    }

    // A `update_mode` the caller misspelled is refused, and the refusal happens
    // before anything is written — so the document keeps the revision it had.
    #[test]
    fn an_unrecognized_update_mode_should_be_refused_without_touching_the_document() {
        let svc = svc_with_banks(&["b"]);
        svc.retain_doc("b", "auth uses jose v1", None, &[], Some("doc"), UpdateMode::Replace)
            .expect("v1");

        for typo in ["append ", "APPEND", "upsert", ""] {
            let err = parse_update_mode(Some(typo))
                .expect_err(&format!("{typo:?} must be refused"));
            assert_eq!(http_error(&err), (StatusCode::BAD_REQUEST, "invalid content"), "{typo:?}");
        }
        assert_eq!(
            parse_update_mode(None).expect("absent"),
            UpdateMode::Replace,
            "an absent field is the default, not a refusal"
        );

        // The refusal is at the parse, so the store was never asked — which is
        // what makes "must not delete the prior revision" true rather than
        // incidental.
        let rows = svc.list_memories("b", 10, 0).expect("list");
        assert_eq!(rows.len(), 1, "a refused mode is not a destructive replace");
        assert!(rows[0].content.ends_with("v1"));
    }

    // The two new storage faults are storage faults: opaque to the caller, and
    // carrying nothing a client could use.
    #[test]
    fn the_new_storage_faults_should_stay_opaque_on_the_wire() {
        for err in [
            ApiError::Store(StoreError::LockPoisoned),
            ApiError::Store(StoreError::TaskFailed),
        ] {
            let (status, msg) = http_error(&err);
            assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
            assert_eq!(msg, "storage error", "a client must learn nothing: {msg}");
            assert!(!msg.contains("lock") && !msg.contains("task"), "got {msg}");
        }
    }

    // ---- the four new mechanisms, end to end -------------------------------

    /// A bank whose rows deliberately disagree with the clock: several share one
    /// query token, one holds a rare one, and the timestamps are spread over
    /// months. It is the shape every one of the four mechanisms needs in order
    /// to be observable at all, and using one bank for all four keeps the
    /// fixtures from drifting apart.
    fn mechanism_bank() -> MemoryService<SqliteStore> {
        let s = SqliteStore::open_in_memory().expect("open");
        s.put_bank(&Bank { id: "b".to_string(), name: "b".to_string() })
            .expect("bank");
        let svc = MemoryService::new(s);
        for content in [
            "rate limiting is a token bucket",
            "rate limiting uses a token bucket with a burst allowance",
            "the deployment pipeline runs on token bucket credentials",
            "release notes mention the token bucket migration",
        ] {
            svc.retain("b", content, None).expect("retain");
        }
        svc
    }

    /// The inertness claim at the level the brief states it: with the shipped
    /// weights, `recall` returns exactly what it returned before these
    /// mechanisms existed — same ids, same order, and the same `score` bit
    /// pattern, not merely a close one.
    ///
    /// The reference is written out longhand rather than delegated to
    /// `rank_candidates`/`rrf_fuse` alone, because those two are the very
    /// functions under test; the point is that the *wiring* in `recall_with`
    /// (the magnitude map, the third stream, the scoped ranker) contributes
    /// nothing at the defaults. It re-derives the pre-change pipeline from
    /// `Store::recall_inputs` — BM25 ranks, whole-document overlap, plain
    /// `1/(k+1)/(k+rank)` over the two — and compares.
    #[test]
    fn the_shipped_recall_should_be_the_pre_change_fusion() {
        let svc = mechanism_bank();
        let query = "token bucket rate limiting";
        let (all, hits) = svc.store.recall_inputs("b", query, &[], FTS_LIMIT).expect("inputs");

        // The pre-change pipeline, written out.
        let mut reference: Vec<(String, f64)> = Vec::new();
        for (i, (id, _)) in hits.iter().enumerate() {
            let s = 1.0 / (60.0 + (i + 1) as f64);
            match reference.iter_mut().find(|(r, _)| r == id) {
                Some((_, acc)) => *acc += s,
                None => reference.push((id.clone(), s)),
            }
        }
        let ranked = rank_candidates(query, all.iter().map(|m| m.content.as_str()));
        for (i, (idx, _)) in ranked.iter().take(OVERLAP_LIMIT).enumerate() {
            let id = &all[*idx].id;
            let s = 0.25 / (60.0 + (i + 1) as f64);
            match reference.iter_mut().find(|(r, _)| r == id) {
                Some((_, acc)) => *acc += s,
                None => reference.push((id.clone(), s)),
            }
        }
        reference.sort_by(|a, b| {
            b.1.partial_cmp(&a.1)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.0.cmp(&b.0))
        });

        let got = svc.recall("b", query, 100_000).expect("recall");
        let got_ids: Vec<&str> = got.iter().map(|h| h.memory.id.as_str()).collect();
        let want_ids: Vec<&str> = reference.iter().map(|(id, _)| id.as_str()).collect();
        assert_eq!(got_ids, want_ids, "the shipped order moved");

        for ((id, want), hit) in reference.iter().zip(&got) {
            assert_eq!(hit.memory.id, *id);
            assert_eq!(
                hit.score.to_bits(),
                want.to_bits(),
                "score for {id} is not the same f64: {:?} vs {:?}",
                hit.score,
                want
            );
        }
    }

    /// A knob that reached `recall_with` and did nothing would pass every unit
    /// test above and still be dead code here. This is the end-to-end proof
    /// that the BM25 magnitude travels from the store's `KeywordHits` all the
    /// way to the fused score, on a real store with real `bm25()` values.
    #[test]
    fn a_non_zero_bm25_magnitude_should_reach_the_fused_score() {
        let svc = mechanism_bank();
        let query = "token bucket";
        let off = svc.recall_with_weights("b", query, 100_000, &FusionWeights::SHIPPED)
            .expect("recall");
        // A large enough knob that a row the rank-only RRF put lower has to
        // overtake the leader; the exact value is not a claim about quality, it
        // is large enough for the mechanism to be observable at all.
        let on = svc.recall_with_weights(
            "b",
            query,
            100_000,
            &FusionWeights { bm25_magnitude: 4.0, ..FusionWeights::SHIPPED },
        )
        .expect("recall");
        assert_eq!(off.len(), on.len(), "the candidate set must not change");
        assert_ne!(
            off.iter().map(|h| h.score).collect::<Vec<_>>(),
            on.iter().map(|h| h.score).collect::<Vec<_>>(),
            "a non-zero magnitude must change the scores, not just the order"
        );
        // And the lift is bounded: no row may gain more than `w` of a rank-1 hit.
        let cap = 4.0 / 61.0;
        for (before, after) in off.iter().zip(&on) {
            let lift = after.score - before.score;
            assert!(
                (lift..=cap + 1e-12).contains(&lift),
                "{} gained {lift}, over the {cap} bound",
                after.memory.id
            );
        }
    }

    /// The same end-to-end requirement for the third stream, and specifically
    /// that the shipped path never reads a clock: a bank whose rows are
    /// deliberately out of insertion order still returns the same ranking with
    /// the default, and a non-zero recency weight brings the newest row up.
    #[test]
    fn a_non_zero_recency_weight_should_add_a_third_stream() {
        let s = SqliteStore::open_in_memory().expect("open");
        s.put_bank(&Bank { id: "b".to_string(), name: "b".to_string() })
            .expect("bank");
        let svc = MemoryService::new(s);
        // Written oldest-first, so insertion order and recency disagree, and the
        // store is asked directly because `retain_doc` has no created_at arm —
        // the column is a store-level fact, stamped on insert unless a caller
        // supplies one.
        for (n, (content, created)) in [
            ("the token bucket capacity table", "2025-09-20T00:00:00Z"),
            ("the token bucket audit log", "2026-06-20T00:00:00Z"),
            ("the token bucket refill interval", "2026-09-20T00:00:00Z"),
        ]
        .iter()
        .enumerate()
        {
            svc.store
                .put(&Memory {
                    id: format!("m{n}"),
                    bank_id: "b".to_string(),
                    content: (*content).to_string(),
                    context: None,
                    created_at: Some((*created).to_string()),
                    meta: crate::memory::MemoryMeta::default(),
                })
                .expect("put");
        }
        let query = "token bucket";
        let ids = |w: f64| {
            svc.recall_with_weights(
                "b",
                query,
                100_000,
                &FusionWeights { recency: w, ..FusionWeights::SHIPPED },
            )
            .expect("recall")
            .iter()
            .map(|h| h.memory.id.clone())
            .collect::<Vec<_>>()
        };
        // Every row ties under both existing streams — same text shape, same
        // length, every query term present in all three — so the store's own
        // order is the whole of the baseline ranking.
        assert_eq!(ids(0.0), ["m0", "m1", "m2"], "the shipped order is the store's");

        // **A third stream at weight 1.0 cannot reverse the other two.** RRF's
        // discount is the same `1/(k+rank)` shape in all three streams, so if the
        // new stream is the exact reverse of the existing ranking its `+1/(k+1)`
        // to the tail is smaller than the `1/(k+1) - 1/(k+3)` the leader keeps,
        // and the sum comes out unchanged. This is a property of the algebra, not
        // of this implementation, and it is recorded on
        // `FusionWeights::recency` because whoever selects that value has to know
        // it: the interesting weights here are the ones far from 1.0.
        assert_eq!(
            ids(1.0),
            ["m0", "m1", "m2"],
            "at unit weight the reversed stream is exactly a wash"
        );
        assert_eq!(ids(4.0), ["m2", "m1", "m0"], "a weight past unit reverses it");
        let top = svc
            .recall_with_weights(
                "b",
                query,
                100_000,
                &FusionWeights { recency: 4.0, ..FusionWeights::SHIPPED },
            )
            .expect("recall");
        assert_eq!(top[0].memory.created_at.as_deref(), Some("2026-09-20T00:00:00Z"));
    }

    /// The scope knob reaches the ranker: a long memory that names each query
    /// token once in separate sentences loses to a short memory that is densely
    /// about the same tokens, but only under [`OverlapScope::BestSentence`].
    /// The scope knob reaches the ranker *and* can change the fused order — but
    /// only in a pool crowded enough for the overlap stream's rank gap to exceed
    /// BM25's. Both halves of that are asserted, because the second is the part
    /// that is easy to assume and hard to get: with one or two documents the
    /// scope moves the overlap rank by one and BM25's 1.0-weighted rank outranks
    /// a 0.25-weighted one every time, so the fused order is unchanged and the
    /// mechanism looks broken when it is merely outgunned.
    #[test]
    fn the_overlap_scope_should_change_which_document_outranks_which() {
        let s = SqliteStore::open_in_memory().expect("open");
        s.put_bank(&Bank { id: "b".to_string(), name: "b".to_string() })
            .expect("bank");
        let svc = MemoryService::new(s);
        // Eight query terms, one per sentence: the only document that has all
        // eight, and no single segment with more than one.
        let scattered = svc
            .retain("b", "rate. limiting. token. bucket. burst. refill. allowance. capacity.", None)
            .expect("retain");
        // Six documents that each hold two of the eight in one unbroken run, so
        // the sentence stream scores every one of them 2 and the scattered
        // document 1 — which pushes it to the bottom of that stream.
        for pair in [
            "the rate limiting window is configurable",
            "a token bucket refills at a fixed rate",
            "the burst allowance is per client",
            "refill capacity is bounded by the window",
            "the allowance decays with the bucket",
            "capacity planning sets the burst limit",
        ] {
            svc.retain("b", pair, None).expect("retain");
        }
        let query = "rate limiting token bucket burst refill allowance capacity";
        let top = |scope| {
            svc.recall_with_weights(
                "b",
                query,
                100_000,
                &FusionWeights { overlap_scope: scope, ..FusionWeights::SHIPPED },
            )
            .expect("recall")
            .into_iter()
            .next()
            .expect("a crowded pool always matches")
            .memory
            .id
        };
        assert_eq!(
            top(OverlapScope::Document),
            scattered,
            "whole-document overlap gives the scattered memory all eight terms"
        );
        assert_ne!(
            top(OverlapScope::BestSentence),
            scattered,
            "no single segment of it holds more than one, so six denser \
             documents must outrank it"
        );
    }

    /// The classifier is wired to the *weight*, not merely present: with
    /// [`RecencyPolicy::TemporalQueriesOnly`] a non-temporal query gets a 0.0
    /// weight, so `recall_with` never builds the third stream for it, and a
    /// temporal one gets the configured weight and does.
    #[test]
    fn the_recency_policy_should_gate_the_third_stream_on_the_classifier() {
        let svc = mechanism_bank();
        let gated = FusionWeights {
            recency: 1.0,
            recency_policy: RecencyPolicy::TemporalQueriesOnly,
            ..FusionWeights::SHIPPED
        };
        for q in ["token bucket rate limiting", "what did I do last time"] {
            let rows = svc
                .recall_with_weights("b", q, 100_000, &gated)
                .expect("recall");
            assert!(!rows.is_empty());
            assert_eq!(
                gated.recency_weight_for(q) != 0.0,
                is_temporal_query(q),
                "{q:?}: the policy and the classifier disagree"
            );
        }
        // `recency_rank` is exported and usable on its own, so the classifier is
        // not the only way to reach it.
        let (all, _) = svc
            .store
            .recall_inputs("b", "token bucket", &[], FTS_LIMIT)
            .expect("inputs");
        assert_eq!(recency_rank(&all, chrono::Utc::now(), 30.0).len(), all.len());
    }

    // ---- Phase B: the lexical query config ---------------------------------
    //
    // The bank-config half of the default-off claim. `KeywordScope` is proven
    // inert in `store.rs`; what is proven here is that a bank that has never
    // heard of `recallSynonyms` gets exactly the ranking it got before, and that
    // a bank that has can turn the feature on for itself without a build.

    /// A config with no `recallSynonyms` key must produce an empty table, so the
    /// store's synonym branch never runs for the overwhelming majority of banks.
    #[test]
    fn an_absent_recall_synonyms_key_should_expand_to_nothing() {
        for raw in [
            "{}",
            r#"{"recallMaxTokens": 500}"#,
            r#"{"retainTags": ["auth"]}"#,
            // A typo is absent, not fatal: the same rule every other key follows.
            r#"{"recallSynonym": {"a": {"terms": ["b"]}}}"#,
            r#"{"recallSynonyms": []}"#,
            r#"{"recallSynonyms": "not an object"}"#,
            "not json at all",
        ] {
            let table = BankConfig::parse(raw).recall_synonyms;
            assert!(table.is_empty(), "{raw} should have produced no synonyms: {table:?}");
            assert!(expand_synonyms(&table, "a b migrate").is_empty(), "{raw} expanded");
        }
    }

    /// An entry is keyed by a trigger term the query must actually contain, and
    /// fires only then. This is the property that makes a synonym table a *rule*
    /// rather than a tax on every query in the bank.
    #[test]
    fn a_synonym_should_fire_only_when_the_query_names_its_trigger() {
        let table = BankConfig::parse(
            r#"{"recallSynonyms": {"migrate": {"terms": ["refactor"], "weight": 0.7}}}"#,
        )
        .recall_synonyms;

        assert_eq!(
            expand_synonyms(&table, "we should migrate soon"),
            vec![("refactor".to_string(), 0.7)],
            "a matching trigger must expand"
        );
        assert!(
            expand_synonyms(&table, "what about the budget").is_empty(),
            "an unrelated query must not expand"
        );
        // Token matching, not substring: a trigger inside a longer word does not
        // count, and a trigger the tokenizer would split does not either.
        assert!(
            expand_synonyms(&table, "unmigrated").is_empty(),
            "a substring must not fire a synonym"
        );
    }

    /// Trigger matching is case-insensitive, in both directions: the config key
    /// is lowercased on read and the query token is lowercased on the way in, so
    /// `"Migrate"` in a config and `"MIGRATE"` in a query meet in the middle.
    #[test]
    fn synonym_trigger_matching_should_ignore_case_in_both_directions() {
        let table = BankConfig::parse(
            r#"{"recallSynonyms": {"Migrate": {"terms": ["REFACTOR"]}}}"#,
        )
        .recall_synonyms;
        assert_eq!(expand_synonyms(&table, "MIGRATE this").len(), 1);
        assert_eq!(expand_synonyms(&table, "migrate this").len(), 1);
        assert_eq!(expand_synonyms(&table, "MiGrAtE this").len(), 1);
    }

    /// A synonym that is its own trigger adds nothing, and emitting it would
    /// double-count that term's BM25 — the same defect `KeywordScope` documents
    /// for an exact term ORed with its own prefix.
    #[test]
    fn a_synonym_equal_to_its_trigger_should_not_be_emitted() {
        let table = BankConfig::parse(
            r#"{"recallSynonyms": {"migrate": {"terms": ["migrate", "Migrate"]}}}"#,
        )
        .recall_synonyms;
        assert!(
            expand_synonyms(&table, "migrate now").is_empty(),
            "a self-referential synonym must not double-count"
        );
    }

    /// A duplicated synonym across two firing triggers must be emitted once, so
    /// two triggers cannot silently double-weight one term.
    #[test]
    fn a_synonym_firing_from_two_triggers_should_be_emitted_once() {
        let table = BankConfig::parse(
            r#"{"recallSynonyms": {
                   "migrate": {"terms": ["refactor"]},
                   "postgres": {"terms": ["refactor"]}
               }}"#,
        )
        .recall_synonyms;
        let got = expand_synonyms(&table, "migrate the postgres database");
        assert_eq!(got.len(), 1, "one term, one weight: {got:?}");
    }

    /// Malformed entries are dropped and out-of-range weights are clamped. A
    /// hand-written config must not be able to make a synonym subtract from a
    /// document's score or outrank the term the user actually typed.
    #[test]
    fn a_malformed_or_out_of_range_synonym_should_be_dropped_or_clamped() {
        // No terms: dropped, because a synonym with no terms is not an entry.
        for raw in [
            r#"{"recallSynonyms": {"a": {}}}"#,
            r#"{"recallSynonyms": {"a": {"terms": []}}}"#,
            r#"{"recallSynonyms": {"a": {"terms": "not a list"}}}"#,
            r#"{"recallSynonyms": {"a": "not an object"}}"#,
        ] {
            assert!(
                BankConfig::parse(raw).recall_synonyms.is_empty(),
                "{raw} should have been dropped"
            );
        }

        // Weight above 1.0 is clamped to a full term.
        let over = BankConfig::parse(
            r#"{"recallSynonyms": {"a": {"terms": ["b"], "weight": 5.0}}}"#,
        )
        .recall_synonyms;
        assert_eq!(expand_synonyms(&over, "a query"), vec![("b".to_string(), 1.0)]);

        // A negative weight is clamped up to the smallest positive weight rather
        // than honoured: a synonym must not be able to *subtract*.
        let under = BankConfig::parse(
            r#"{"recallSynonyms": {"a": {"terms": ["b"], "weight": -3.0}}}"#,
        )
        .recall_synonyms;
        let got = expand_synonyms(&under, "a query");
        assert_eq!(got.len(), 1);
        assert!(got[0].1 > 0.0, "a negative weight must not survive: {:?}", got);

        // A missing weight takes the documented default, not zero and not one.
        let none = BankConfig::parse(r#"{"recallSynonyms": {"a": {"terms": ["b"]}}}"#)
            .recall_synonyms;
        assert_eq!(expand_synonyms(&none, "a query"), vec![("b".to_string(), DEFAULT_SYNONYM_WEIGHT)]);
    }

    /// The end-to-end default-off proof, in the form the brief asks for: with no
    /// `recallSynonyms` configured, the ranking is not merely close but the same
    /// ids in the same order carrying the same `f64` bit patterns.
    #[test]
    fn a_bank_without_recall_synonyms_should_recall_exactly_as_before() {
        let svc = mechanism_bank();
        let query = "token bucket rate limiting";

        // `recall` reads no config at all, so it is the reference.
        let reference = svc.recall("b", query, 100_000).expect("reference recall");
        assert!(!reference.is_empty(), "the fixture must actually match");

        // The config-aware entry point, with a config that has no synonym key.
        svc.set_bank_config("b", r#"{"recallMaxTokens": 100000}"#).expect("config");
        let got = svc.recall_filtered("b", query, None, &[], &[]).expect("configured recall");

        assert_eq!(
            got.iter().map(|h| h.memory.id.as_str()).collect::<Vec<_>>(),
            reference.iter().map(|h| h.memory.id.as_str()).collect::<Vec<_>>(),
            "a config without the key moved the order"
        );
        for (want, have) in reference.iter().zip(&got) {
            assert_eq!(
                have.score.to_bits(),
                want.score.to_bits(),
                "score for {} is not the same f64: {:?} vs {:?}",
                want.memory.id,
                have.score,
                want.score
            );
        }
    }

    /// A bank that *does* configure a synonym sees it applied, end to end
    /// through the public API. Without this the config key would be parsed,
    /// tested in isolation, and never observed to do anything.
    #[test]
    fn a_configured_synonym_should_reach_a_recall_the_query_alone_misses() {
        let svc = svc_with_banks(&["b"]);
        svc.retain("b", "postgres schema change scheduled friday", None)
            .expect("retain");

        // One token, sharing nothing with the memory. A longer query would make
        // this test pass or fail for the wrong reason: `fts_match_query` ORs
        // every token of length >= 2, so a query carrying "the" or "database"
        // matches almost any sentence and the "before" assertion below would
        // fail on a stopword rather than on the synonym.
        let query = "migrate";
        assert_eq!(
            svc.recall_filtered("b", query, None, &[], &[]).expect("before").len(),
            0,
            "the query alone must not match"
        );

        svc.set_bank_config(
            "b",
            r#"{"recallSynonyms": {"migrate": {"terms": ["schema"], "weight": 0.7}}}"#,
        )
        .expect("config");
        let after = svc.recall_filtered("b", query, None, &[], &[]).expect("after");
        assert_eq!(after.len(), 1, "the configured synonym must reach it: {after:?}");
        assert!(after[0].memory.content.contains("schema change"));

        // …and the same expansion survives an explicit `budget`, which is the
        // branch HTTP and MCP always take. It used to discard the whole synonym
        // table, so a bank that configured the key got no expansion on either
        // real surface while the key parsed, unit-tested and did nothing.
        let explicit = svc
            .recall_filtered("b", query, Some(DEFAULT_RECALL_BUDGET), &[], &[])
            .expect("explicit-budget recall");
        assert_eq!(
            explicit.iter().map(|h| h.memory.id.as_str()).collect::<Vec<_>>(),
            after.iter().map(|h| h.memory.id.as_str()).collect::<Vec<_>>(),
            "an explicit budget must not drop the configured synonyms"
        );
    }

    /// The prefix arm, end to end through the public API and gated on the
    /// shipped weights being off. `recall_with_weights` is the only entry point
    /// that can select it, which is what makes this a selection knob rather than
    /// a request parameter.
    #[test]
    fn the_prefix_arm_should_be_selectable_through_weights_and_off_by_default() {
        let svc = svc_with_banks(&["b"]);
        svc.retain("b", "the database migration needs a backup", None).expect("retain");
        let query = "migrat";

        // Off: nothing. The exact grammar cannot see "migration" from "migrat".
        let off = svc.recall_with_weights("b", query, 100_000, &FusionWeights::SHIPPED)
            .expect("off recall");
        assert!(
            off.iter().all(|h| !h.memory.content.contains("migration")),
            "the exact grammar must not match the variant: {off:?}"
        );

        // On: the same code path, one field different.
        let on = svc
            .recall_with_weights(
                "b",
                query,
                100_000,
                &FusionWeights { keyword_scope: KeywordScope::ExactAndPrefix, ..FusionWeights::SHIPPED },
            )
            .expect("on recall");
        assert!(
            on.iter().any(|h| h.memory.content.contains("migration")),
            "the prefix scope must reach the variant: {on:?}"
        );
    }
}
