//! retain/recall/reflect API surface (Hindsight ops on agentmemory capture).
//!
//! - retain: redact content *and* context, persist via [`crate::store::Store`].
//! - recall: overlap-ranked retrieval + token-budget trim.
//! - reflect: top-hit synthesis with cited ids (LLM synthesis lands later).

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use uuid::Uuid;

use crate::capture::redact_pii;
use crate::memory::{Bank, Memory};
use crate::recall::{
    rank_candidates_scoped, recency_rank, rrf_fuse_with_magnitudes, trim_to_budget, FusionWeights,
    RankedHit,
};
pub use crate::store::BankStats;
use crate::store::{Store, StoreError, UpdateMode};

/// FTS5 candidates pulled per recall — the LIMIT is pushed into the SQL.
const FTS_LIMIT: usize = 50;
/// Token-overlap candidates fused per recall.
const OVERLAP_LIMIT: usize = 200;
/// Hard ceiling on memories returned by one recall.
const MAX_RESULTS: usize = 100;
/// Recall budget used when neither the request nor the bank config sets one.
pub const DEFAULT_RECALL_BUDGET: usize = 2000;
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
        }
    }
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
        Self::require_bank(bank_id)?;
        if content.trim().is_empty() {
            // A memory of nothing matches no query and cites nothing, so it is
            // dead weight forever. Rejecting it names the client's actual
            // mistake instead of storing a row that can never be retrieved.
            return Err(ApiError::InvalidContent);
        }
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
        all.extend(self.config_of(bank_id)?.retain_tags);
        let memory = Memory {
            id: Uuid::new_v4().to_string(),
            bank_id: bank_id.to_string(),
            content: redact_pii(content),
            context: context.as_deref().map(redact_pii),
            // The store stamps this on insert; `None` means "not yet known", and
            // is now the only way to ask for that.
            created_at: None,
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
    /// [`DEFAULT_RECALL_BUDGET`]. An explicit budget skips the config read
    /// entirely, so the override path costs no extra query.
    ///
    /// The config read is propagated, not swallowed. It used to be
    /// `.ok().and_then(…)`, which turned a storage fault into a *successful*
    /// recall trimmed to a budget the bank never asked for — a 200 carrying an
    /// answer the caller had no way to know was wrong. `retain_doc` already
    /// propagates the same call; this was an oversight, not a policy.
    pub fn recall_filtered(
        &self,
        bank_id: &str,
        query: &str,
        budget: Option<usize>,
        tags: &[String],
    ) -> Result<Vec<ScoredMemory>, ApiError> {
        Self::require_bank(bank_id)?;
        let budget = match budget {
            Some(b) => b,
            None => self
                .config_of(bank_id)?
                .recall_max_tokens
                .unwrap_or(DEFAULT_RECALL_BUDGET),
        };
        self.recall_with(bank_id, query, budget, tags, &FusionWeights::SHIPPED)
    }

    /// Recall top memories for `query` within `budget_tokens`.
    pub fn recall(
        &self,
        bank_id: &str,
        query: &str,
        budget_tokens: usize,
    ) -> Result<Vec<ScoredMemory>, ApiError> {
        Self::require_bank(bank_id)?;
        self.recall_with(bank_id, query, budget_tokens, &[], &FusionWeights::SHIPPED)
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
        self.recall_with(bank_id, query, budget_tokens, &[], weights)
    }

    /// Recall top memories for `query`, restricted to memories carrying any of
    /// `tags` (empty = the whole bank).
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
        tags: &[String],
        weights: &FusionWeights,
    ) -> Result<Vec<ScoredMemory>, ApiError> {
        let normalized = normalize_tags(tags);
        if !tags.is_empty() && normalized.is_empty() {
            // Fail closed. A filter the caller sent that normalizes to nothing
            // (["", "   "]) names no memory at all, and answering it with the
            // whole bank would reply to a scoped question with unrelated rows
            // that look exactly like a real result.
            return Ok(Vec::new());
        }
        let (all, keyword_hits) = self
            .store
            .recall_inputs(bank_id, query, &normalized, FTS_LIMIT)?;
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

    /// Delete one memory; `Ok(false)` when it was not in `bank_id`.
    pub fn delete_memory(&self, bank_id: &str, id: &str) -> Result<bool, ApiError> {
        Self::require_bank(bank_id)?;
        Ok(self.store.delete(bank_id, id)?)
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
        let hits = self.recall_filtered(bank_id, query, Some(2000), tags)?;
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
        is_temporal_query, rank_candidates, recency_rank, OverlapScope, RecencyPolicy,
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
        assert_eq!(svc.recall_filtered("b", "jose", None, &[]).expect("all").len(), 3);
        // Any-match: one hit per matching tag, and an untagged memory never does.
        for filter in ["auth", "ops", "secrets"] {
            assert_eq!(
                svc.recall_filtered("b", "jose", None, &tags(&[filter])).expect("filter").len(),
                1,
                "filter {filter} should match exactly one memory"
            );
        }
        assert!(svc
            .recall_filtered("b", "jose", None, &tags(&["nothing-has-this"]))
            .expect("miss")
            .is_empty());
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
                svc.recall_filtered("b", "jose", None, &tags(&[filter])).expect("filter").len(),
                1,
                "{filter} should reach the memory"
            );
        }
        // A bank-wide default applies to every later retain, not just this one.
        svc.retain("b", "auth uses jose second", None).expect("plain retain");
        assert_eq!(svc.recall_filtered("b", "jose", None, &tags(&["bank"])).expect("bank").len(), 2);
        assert!(svc.recall_filtered("b", "jose", None, &tags(&["req"])).expect("req").len() == 1);
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
        let bank_default = len(svc.recall_filtered("cfg", "jose", None, &[]).expect("bank"));
        let explicit = len(svc.recall_filtered("cfg", "jose", Some(10), &[]).expect("explicit"));
        let server_default = len(svc.recall_filtered("plain", "jose", None, &[]).expect("default"));

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
        let hits = svc.recall_filtered("b", "jose", None, &[]).expect("recall");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].memory.content, "auth uses jose middleware");
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
            .recall_filtered("b", "jose", None, &old_tags)
            .expect("old tag")
            .is_empty());
        assert_eq!(
            svc.recall_filtered("b", "jose", None, &tags(&["auth"])).expect("new tag").len(),
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
            .recall_filtered("b", "jose", None, &tags(&["", "   "]))
            .expect("blank filter")
            .is_empty());
        // An absent filter is the whole bank, and stays that way.
        assert_eq!(svc.recall_filtered("b", "jose", None, &[]).expect("empty filter").len(), 1);
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
            })
            .expect("put");

        // The bank default cannot be read, so the recall cannot know its budget.
        let err = svc
            .recall_filtered("b", "jose", None, &[])
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
            svc.recall_filtered("b", "jose", Some(2000), &[]).expect("explicit budget").len(),
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
}
