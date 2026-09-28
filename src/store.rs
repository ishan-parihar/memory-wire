//! Storage backends: embedded SQLite (dev) + Postgres+pgvector (prod).
//!
//! Hindsight: Postgres + pgvector (pg0 embedded default, external PG / Oracle for prod).
//! agentmemory: SQLite/KV + in-memory vector index, zero external DBs.
//! memory-wire: single `Store` trait, SQLite first, Postgres when `DATABASE_URL` is set.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, MutexGuard, TryLockError};

use rusqlite::{params, params_from_iter, Connection, OptionalExtension, Row, ToSql};
use serde::Serialize;

use crate::capture::hash_content;
use crate::memory::{Bank, Memory};

/// Default database file: `$XDG_DATA_HOME/memory-wire/memory.db`, falling back
/// to `~/.local/share/memory-wire/memory.db` when `XDG_DATA_HOME` is unset or
/// relative (the XDG spec says a relative value must be ignored).
pub fn default_db_path() -> PathBuf {
    let base = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| {
            PathBuf::from(std::env::var_os("HOME").unwrap_or_default())
                .join(".local/share")
        });
    base.join("memory-wire").join("memory.db")
}

/// Errors from the storage layer.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// Underlying SQLite failure.
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    /// Bank id missing or empty.
    #[error("invalid bank id")]
    InvalidBank,
    /// Bank row does not exist.
    #[error("unknown bank")]
    UnknownBank,
    /// Bank config is not a JSON object.
    #[error("invalid bank config")]
    InvalidConfig,
    /// An `append` reused a `document_id` that already holds a row in this bank.
    ///
    /// The caller sent one document's content and asked for it to be *added* to a
    /// document that exists, so nothing about the request is a server fault: the
    /// two states are "replace the current revision" and "a different document
    /// id", and the caller chose neither.
    #[error("document already exists; use update_mode=replace")]
    DocumentConflict,
    /// The backend does not implement this operation.
    #[error("unsupported: {0}")]
    Unsupported(&'static str),
    /// The store's connection lock was poisoned by a panic that unwound while
    /// the guard was held.
    ///
    /// A poisoned lock means some earlier call panicked mid-transaction, so the
    /// connection may hold a half-finished state. Every later call is refused
    /// rather than served from it: handing back a possibly-inconsistent
    /// connection would answer reads with data no write ever committed, and a
    /// loud 500 an operator can find in the log is worth more than a silent
    /// wrong answer. Recovery is the operator's call — reopen the store.
    ///
    /// Carries no path, no SQL and no content: it is a statement about this
    /// process, and it reaches a client as the same opaque 500 as every other
    /// storage fault.
    #[error("store lock poisoned")]
    LockPoisoned,
    /// A blocking store task did not run to completion.
    ///
    /// The store calls that can block (SQLite, and the mutex in front of it) run
    /// on tokio's blocking pool, and a task that panics there is reported as a
    /// join failure rather than as the value it was going to return. It is a
    /// storage fault like any other, so it answers the same opaque 500 instead
    /// of leaving the request unanswered.
    #[error("storage task failed")]
    TaskFailed,
}

/// `banks.config`, added after the table shipped; absent on older databases.
const CONFIG_COLUMN: &str = "config";
/// `memories.created_at`, added after the table shipped.
const CREATED_AT_COLUMN: &str = "created_at";
/// `memories.document_id`, added after the table shipped.
const DOCUMENT_ID_COLUMN: &str = "document_id";
/// `memories.content_hash`, added after the table shipped; absent on older
/// databases.
const CONTENT_HASH_COLUMN: &str = "content_hash";

/// `banks.ttl_days`, added after the table shipped; `NULL` means the bank never
/// expires anything.
const TTL_DAYS_COLUMN: &str = "ttl_days";

/// `banks.background`, removed after the table shipped. The bank preamble reads
/// `background` from the config JSON, so this column was a second home for a
/// concept nothing read: wired up it would have been two sources of truth, and
/// unwired it was a public field that silently discarded whatever a caller set.
const BACKGROUND_COLUMN: &str = "background";

/// Timestamp written to a row that predates timestamping.
///
/// A fixed sentinel rather than the migration's own clock: the migration runs
/// once, so dating every legacy row to the moment it was migrated would state a
/// fact about the migration, not about the memory. Epoch reads as "unknown
/// age", which is the truth, and a visibly wrong age beats a plausible one.
const UNKNOWN_CREATED_AT: &str = "1970-01-01T00:00:00Z";

/// The wire spelling of [`UpdateMode::Append`].
///
/// Kept for callers that build the request; the decision itself is
/// [`UpdateMode`], parsed once at the boundary, so a typo can no longer arrive
/// at the store as a raw `&str` and land on the destructive side.
pub const APPEND_MODE: &str = "append";

/// The wire spelling of [`UpdateMode::Replace`], and the default when a request
/// names no mode at all.
pub const REPLACE_MODE: &str = "replace";

/// How many read connections a file-backed store keeps open **besides** the
/// write connection.
///
/// Not a knob: a compile-time constant with the reasoning attached, because a
/// runtime dial would be a thing to tune against a number nobody has measured on
/// a second machine. It was chosen by measurement, not taste — `bench_concurrency`
/// at three sizes under [`SqliteStore::read_conn`]'s writer-first policy, this
/// box, 2,000 memories, `--memories 2000`, one round each (the highest-resolution
/// block this box produced, load average 28-50):
///
/// | size | 1-cl p50 | 4-cl | 8-cl | 16-cl | 32-cl | 64-cl ops/s | sweep RSS |
/// |---|---|---|---|---|---|---|---|
/// | 2 | 1,213 us | 1,592 | 1,533 | 1,518 | 1,439 | 1,152 | 11.9 MB |
/// | 4 | 1,283 us | 1,426 | 1,445 | 1,366 | 1,377 | 1,239 | 12.9 MB |
/// | 8 | 1,279 us | 1,661 | 1,548 | 1,447 | 1,475 | 1,291 | 15.0 MB |
///
/// and a second, rotated block — arm order reshuffled per round so a slow minute
/// cannot land on one size — of the mixed read/write `soak` (16 clients, load
/// 72-103):
///
/// | size | aggregate ops/s | RSS start → total | ceiling |
/// |---|---|---|---|
/// | 2 | 1,719 / 1,860 | 6.2 → 15.0 MB · 6.2 → 18.7 MB | 23.8 MB |
/// | 4 | 1,478 / 1,215 | 6.4 → 21.6 MB · 6.6 → 21.2 MB | 23.8 MB |
/// | 8 | 1,400 / 1,385 | 6.7 → 23.7 MB · 6.7 → 24.2 MB | 23.8 MB |
///
/// Two is not enough. Its throughput *falls* from the 4-client step on — 1,592
/// → 1,533 → 1,518 → 1,439 → 1,152 ops/s — because two spilled readers cannot
/// cover four or more clients, so the sweep plateaus and then degrades into a
/// serialization signature (1.23x from 1 to 64 clients, under the harness's
/// 1.50x floor). `soak.rs` shows the same shape from the other side: the
/// smallest 16-client resident growth of the three (+8.8 / +12.4 MB against
/// +14.6 / +15.2 MB for four), because half its reads are queueing rather than
/// working.
///
/// Four holds the plateau. Eight buys nothing measurable over it at any client
/// count — 1,291 against 1,239 at 64 clients, inside the spread, and it does not
/// win at 4 or 16 either — while every cost here is per connection:
/// `cache_size=-2000` is a *per-connection* 2 MB ceiling, so each reader is
/// another 2 MB of page cache, and each is a separate FTS5 churn stream leaving
/// its own freed blocks in whatever thread's arena they land in. That is
/// +2.1 MB of RSS in the concurrency sweep and +2.5 MB in the mixed soak over
/// four, and it puts the soak total at 23.7-24.2 MB against `soak.rs`'s own
/// 23.8 MB ceiling — at the line, or through it, for a gain that does not exist.
///
/// Wider cannot help writes. WAL still permits exactly one writer, and that
/// writer is the separate connection in [`SqliteStore::conn`], so there is no
/// write workload to size this against.
const READ_POOL_SIZE: usize = 4;

/// What a document-scoped write does with the document's prior revision.
///
/// An enum rather than a `&str` because the two sides are not symmetric: `append`
/// only ever *adds*, while `replace` deletes the document's earlier row. A
/// mistyped string that fell through to `replace` would delete a revision the
/// caller never asked to lose, and a type cannot be mistyped. The wire value is
/// parsed once, at the boundary, by [`UpdateMode::parse`].
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum UpdateMode {
    /// The document ends with exactly one current revision: its prior row is
    /// deleted in the same transaction as the new one. The default.
    #[default]
    Replace,
    /// Every earlier row under this document id is kept, and the write adds one.
    Append,
}

impl UpdateMode {
    /// This mode's wire spelling, as a client's JSON field.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Replace => REPLACE_MODE,
            Self::Append => APPEND_MODE,
        }
    }

    /// The mode a client's `update_mode` names, or `None` when it names none
    /// this build accepts.
    ///
    /// Absent is the default, and exactly two values are recognized: anything
    /// else — `"append "`, `"APPEND"`, a typo — is refused rather than defaulted,
    /// because defaulting is what turned a misspelling into a destructive
    /// replace. Turning the refusal into a response is the caller's job;
    /// [`crate::api::parse_update_mode`] is that one place.
    pub fn parse(raw: Option<&str>) -> Option<Self> {
        match raw {
            None => Some(Self::Replace),
            Some(REPLACE_MODE) => Some(Self::Replace),
            Some(APPEND_MODE) => Some(Self::Append),
            Some(_) => None,
        }
    }
}

/// Column list every `memories` read selects, in [`memory_from_row`] order.
const MEMORY_COLUMNS: &str = "id, bank_id, content, context, created_at";

/// One-shot markers recording work a database only ever needs once.
const MARKER_TABLE: &str = "schema_markers";
/// Set once every `memories` row carries a `content_hash`, so the backfill is
/// not re-run (and not re-scanned) on every open.
const HASH_BACKFILL_MARKER: &str = "content_hash_backfilled";
/// Set once `memories_fts` has been recreated with `detail=none`, so a database
/// written before that change is rebuilt exactly once rather than on every open.
///
/// A marker rather than a read of the FTS options because **FTS5 does not record
/// `detail` where a probe could find it**: the `{table}_config` shadow table of a
/// `memories_fts` holds exactly one row and one column of interest — `version=4`
/// — for a `detail=full` and a `detail=none` table alike (both verified against
/// SQLite 3.53.4, and `detail` is absent from the table in both cases). So there
/// is nothing to read back, and the only honest way to answer "was this database
/// already converted?" is to record the answer. Same one-shot mechanism, and the
/// same shape, as [`HASH_BACKFILL_MARKER`].
const FTS_DETAIL_MARKER: &str = "fts_detail_none";

/// Rows the recall candidate pool is drawn from, on top of the BM25 hits.
///
/// The overlap stream spends at most `crate::api::OVERLAP_LIMIT` candidates on
/// its budget (it truncates with `.take`), so a window deeper than that would
/// only read rows the stream then discards — this is the whole budget it has,
/// which is why the bound is derived from that constant rather than invented.
/// Newest-first is the one order SQLite can serve from `idx_memories_bank`
/// without ranking the bank first, and on a capture store the recent tail is
/// also the context a query is most often about. The BM25 hits are unioned in
/// whatever their age, so a relevant old row the window missed still reaches
/// the fusion instead of being dropped for being old.
pub const RECALL_POOL_LIMIT: usize = 200;

/// BM25 hits as `(memory_id, rank)`, best-first.
pub type KeywordHits = Vec<(String, f64)>;

/// The two retrieval streams of one recall: the bank's memories and its BM25 hits.
pub type RecallInputs = (Vec<Memory>, KeywordHits);

/// Row counts and age bounds for one bank.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct BankStats {
    /// Memories in the bank.
    pub memories: usize,
    /// Distinct tags carried by those memories.
    pub tags: usize,
    /// Oldest `created_at`, or `None` for an empty bank.
    pub oldest: Option<String>,
    /// Newest `created_at`, or `None` for an empty bank.
    pub newest: Option<String>,
}

/// Map a `memories` row selected as [`MEMORY_COLUMNS`].
fn memory_from_row(row: &Row<'_>) -> rusqlite::Result<Memory> {
    Ok(Memory {
        id: row.get(0)?,
        bank_id: row.get(1)?,
        content: row.get(2)?,
        context: row.get(3)?,
        // The read path always fills it: the column is `NOT NULL`, so a row
        // that exists has a stamp and the only way to express one is `Some`.
        created_at: Some(row.get(4)?),
    })
}

/// `AND EXISTS (... t.tag IN (?,...))` for a non-empty tag list, else empty.
///
/// Any-match: a memory survives the filter when it carries at least one of the
/// requested tags. The predicate rides on the same statement as the bank scope,
/// so a filter narrows candidates in SQL instead of after retrieval.
fn tag_predicate(alias: &str, tags: &[String]) -> String {
    if tags.is_empty() {
        return String::new();
    }
    let marks = vec!["?"; tags.len()].join(",");
    format!(
        " AND EXISTS (SELECT 1 FROM memory_tags t \
         WHERE t.memory_id = {alias}.id AND t.tag IN ({marks}))"
    )
}

/// True when a driver error is the one constraint a caller can act on: the
/// unique index on `(bank_id, document_id)` refusing a repeated document id.
///
/// `SQLITE_CONSTRAINT_UNIQUE` is distinct from the primary-key and foreign-key
/// codes SQLite reports through the same `SqliteFailure` variant, so an orphan
/// bank or a clashing row id can never be read as a document conflict.
fn is_repeated_document(e: &rusqlite::Error) -> bool {
    matches!(
        e,
        rusqlite::Error::SqliteFailure(inner, _)
            if inner.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_UNIQUE
    )
}

/// Minimal storage trait (Phase 1: banks + memories, bank-isolated).
pub trait Store: Send + Sync {
    /// Persist a bank (idempotent).
    fn put_bank(&self, bank: &Bank) -> Result<(), StoreError>;
    /// Persist a memory.
    fn put(&self, m: &Memory) -> Result<(), StoreError>;
    /// Fetch one memory by id (bank-scoped: must match).
    fn get(&self, bank_id: &str, id: &str) -> Result<Option<Memory>, StoreError>;
    /// List memories for one bank only (strict isolation).
    fn list(&self, bank_id: &str) -> Result<Vec<Memory>, StoreError>;
    /// BM25 keyword search scoped to one bank: `(memory_id, rank)` best-first.
    ///
    /// Default is empty (backends without full-text override this).
    fn keyword_search(
        &self,
        _bank_id: &str,
        _query: &str,
        _limit: usize,
    ) -> Result<KeywordHits, StoreError> {
        Ok(Vec::new())
    }
    /// Persist a memory together with its tags in one transaction.
    ///
    /// `tags` is the complete tag set for the row. Default is
    /// [`StoreError::Unsupported`], like every other operation a backend cannot
    /// honour: a default that quietly dropped the tags would hand a tag-filtered
    /// recall rows the caller asked to exclude, and a wrong answer is worse
    /// than a refusal a future backend author cannot miss.
    fn put_tagged(&self, _m: &Memory, _tags: &[String]) -> Result<(), StoreError> {
        Err(StoreError::Unsupported("put tagged"))
    }
    /// Persist a document-scoped memory with its tags in one transaction.
    ///
    /// `document_id` is the caller's stable key for "this is the same
    /// document"; `None` means the memory belongs to no document. `update_mode`
    /// is [`UpdateMode::Append`] to keep every earlier row under that key, or
    /// [`UpdateMode::Replace`] to delete the prior row first so the document has
    /// exactly one current revision.
    ///
    /// Returns the id of the row that now holds the content: normally `m.id`, and
    /// the id of the pre-existing row when the same content was already in this
    /// bank and the write was skipped as a duplicate. A backend that dedups must
    /// return the id it actually kept, not the one it was handed.
    ///
    /// Note the pair's tension: the unique index on `(bank_id, document_id)` is
    /// what keeps a document from holding two rows under one key, so an append
    /// that *reuses* a document id is refused rather than deduped — with
    /// [`StoreError::DocumentConflict`], which the API surfaces as `409` naming
    /// `update_mode=replace`. Distinct ids (or `None`) are the append path's
    /// normal input.
    ///
    /// Default is [`StoreError::Unsupported`]: a default that dropped
    /// `document_id` would turn a replace into a second row under a fresh id,
    /// which is the opposite of what the caller asked for and looks exactly like
    /// an append that worked.
    fn put_doc(
        &self,
        _m: &Memory,
        _tags: &[String],
        _document_id: Option<&str>,
        _update_mode: UpdateMode,
    ) -> Result<String, StoreError> {
        Err(StoreError::Unsupported("put document"))
    }
    /// Delete one memory, bank-scoped: another bank's id must not be reachable.
    ///
    /// `Ok(true)` when a row went away. Default is
    /// [`StoreError::Unsupported`], because a backend that cannot delete must
    /// say so rather than report a silent no-op.
    fn delete(&self, _bank_id: &str, _id: &str) -> Result<bool, StoreError> {
        Err(StoreError::Unsupported("delete"))
    }
    /// One page of a bank's memories, in the backend's own insertion order.
    ///
    /// Default pages [`Store::list`] in memory, which is the same order; a
    /// backend that can window in SQL should override it rather than read a
    /// whole bank to return twenty rows.
    fn list_page(
        &self,
        bank_id: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Memory>, StoreError> {
        Ok(self.list(bank_id)?.into_iter().skip(offset).take(limit).collect())
    }
    /// Row counts and age bounds for one bank.
    ///
    /// Default is [`StoreError::Unsupported`]: a partial summary (rows but no
    /// tag count) would read exactly like a real one.
    fn bank_stats(&self, _bank_id: &str) -> Result<BankStats, StoreError> {
        Err(StoreError::Unsupported("bank stats"))
    }
    /// The two retrieval streams of one recall: the bank's memories and its BM25
    /// hits, both restricted to rows carrying ANY of `tags` (empty = whole bank).
    ///
    /// The memory stream is a *bounded candidate pool*, not the whole bank: a
    /// recall that read every row would make its cost grow with the bank while
    /// fusion could only ever use [`RECALL_POOL_LIMIT`] of them. A backend
    /// should bound the read in SQL rather than materialize the bank to drop
    /// most of it.
    ///
    /// Tag filtering happens in the store because that is the only layer that
    /// can push it into SQL. Default is [`StoreError::Unsupported`], because a
    /// default that ignored the filter would answer a scoped recall with the
    /// whole bank — the request was scoped, and the unfiltered rows would be
    /// indistinguishable from real results.
    fn recall_inputs(
        &self,
        _bank_id: &str,
        _query: &str,
        _tags: &[String],
        _fts_limit: usize,
    ) -> Result<RecallInputs, StoreError> {
        Err(StoreError::Unsupported("recall inputs"))
    }
    /// Raw bank config JSON, or `None` when the bank does not exist.
    fn get_bank_config(&self, _bank_id: &str) -> Result<Option<String>, StoreError> {
        Ok(None)
    }
    /// Replace a bank's whole config JSON. Rejects anything that is not a JSON
    /// object; unknown banks are [`StoreError::UnknownBank`], not created.
    fn set_bank_config(&self, _bank_id: &str, _config: &str) -> Result<(), StoreError> {
        Err(StoreError::Unsupported("bank config"))
    }
    /// Every bank with its retention policy: `(id, ttl_days)`, `None` for a bank
    /// that never expires anything. Ordered by id so a sweep's report is stable.
    ///
    /// Reads one pass over a handful of rows rather than parsing each bank's
    /// config JSON, which is what the `ttl_days` column exists for.
    fn bank_ttls(&self) -> Result<Vec<(String, Option<u32>)>, StoreError> {
        Err(StoreError::Unsupported("bank retention"))
    }
    /// Delete a bank's memories created strictly before `cutoff`; returns how
    /// many went. `dry_run` counts them instead of deleting.
    ///
    /// `cutoff` is an RFC 3339 UTC stamp in the same fixed-width form the store
    /// writes into `created_at`, so the comparison is a byte compare and
    /// "created exactly at the cutoff" survives.
    ///
    /// Two stamp shapes that byte compare decides, both pinned by tests: the
    /// epoch sentinel (`1970-01-01T00:00:00Z`) sorts before every real stamp, so
    /// a pre-timestamp row is swept the moment its bank asks to forget anything —
    /// the honest reading of "unknown age". A stamp that is not in the RFC 3339
    /// form at all sorts *after* every real one, so a malformed value is kept and
    /// never mistaken for an old one: the sweep forgets, so the wrong direction
    /// here is the expensive one.
    ///
    /// Default is [`StoreError::Unsupported`]: a backend that cannot expire must
    /// say so rather than report a silent no-op.
    fn expire_before(&self, _bank_id: &str, _cutoff: &str, _dry_run: bool) -> Result<usize, StoreError> {
        Err(StoreError::Unsupported("expire"))
    }
}

/// The `ttl_days` a stored config asks for, as the column holds it.
///
/// Absent, `null`, out of `u32` range, and non-numeric all read the same way:
/// no policy, so the bank never expires. A hand-written config must not be able
/// to make forgetting happen by accident, and it must not be able to fail a
/// write either — this is read on the write path, where an unparseable value
/// would otherwise turn a typo into a refused config.
fn ttl_days_of(config: &serde_json::Value) -> Option<u32> {
    config
        .get("ttl_days")
        .and_then(|v| v.as_u64())
        .and_then(|n| u32::try_from(n).ok())
}

/// SQLite-backed store (bundled, zero daemon). Postgres lands in Phase 1b via the same trait.
///
/// Reads and writes take *different* connections, because the database runs in
/// WAL mode and WAL is the one journal mode that lets a reader work while a
/// writer commits. One `Mutex<Connection>` cannot express that: it funnels both
/// through the same connection, so `N` concurrent recalls queue behind each
/// other on a mutex rather than overlapping on the database.
pub struct SqliteStore {
    /// The write connection, the *preferred* read connection, and — for an
    /// in-memory store — the only connection. See [`SqliteStore::read_conn`].
    conn: Mutex<Connection>,
    /// Read connections, one per WAL reader, for a file-backed store. **Empty
    /// for an in-memory store**, and that is load-bearing rather than an
    /// oversight: a `:memory:` database is per-connection, so a second
    /// connection to one is a *second, empty* database, and a pool of them would
    /// hand a read an empty bank while the write connection held every row. A
    /// shared-cache URI (`file:...?mode=memory&cache=shared`) can join them, and
    /// was not chosen: it brings its own table-level locking, and the tests and
    /// the zero-setup path both want the plain, obvious database.
    reads: Vec<Mutex<Connection>>,
    /// Where [`SqliteStore::read_conn`] resumes looking for a free reader after
    /// the spill path found the current one busy. Advisory: a stale value costs
    /// a retry. Advanced on the spill path only, so an uncontended reader —
    /// which always wins the writer — never rotates off it and never pays for
    /// a second cold page cache it has no contention to amortize.
    read_cursor: AtomicUsize,
    /// Whether this open had to rebuild the FTS index. Facts about the open, not
    /// about the data: nothing branches on it at runtime, it is here so a
    /// regression test can observe the gate instead of inferring it from timing.
    rebuilt_fts: bool,
}

impl SqliteStore {
    /// Open an in-memory database (tests only — nothing survives the process).
    pub fn open_in_memory() -> Result<Self, StoreError> {
        let conn = Connection::open_in_memory()?;
        let mut store = Self {
            conn: Mutex::new(conn),
            reads: Vec::new(),
            read_cursor: AtomicUsize::new(0),
            rebuilt_fts: false,
        };
        store.configure()?;
        store.rebuilt_fts = store.migrate()?;
        Ok(store)
    }

    /// Open (or create) a file database. Parent directories must already exist.
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        let conn = Connection::open(path)?;
        let mut store = Self {
            conn: Mutex::new(conn),
            reads: Vec::new(),
            read_cursor: AtomicUsize::new(0),
            rebuilt_fts: false,
        };
        store.configure()?;
        store.rebuilt_fts = store.migrate()?;
        // Opened once, here, *after* `migrate` has committed: the readers must
        // attach to a database that already has its schema, and — the reason the
        // pool is a field rather than something built per call — there is no
        // path by which a read connection can be opened while the write
        // connection is mid-transaction. `put_doc`'s supersede-and-insert is
        // therefore as atomic as it was, and a reader that arrives during it
        // gets the pre-commit snapshot from WAL, never a half-applied one.
        store.reads = Self::open_read_pool(path)?;
        Ok(store)
    }

    /// Open the read connections for a file-backed store, each configured exactly
    /// like the write connection.
    fn open_read_pool(path: &Path) -> Result<Vec<Mutex<Connection>>, StoreError> {
        (0..READ_POOL_SIZE)
            .map(|_| {
                let conn = Connection::open(path)?;
                // Same pragmas as the write connection, and the omission of any
                // of them is silent rather than loud. `busy_timeout` in
                // particular: a reader without one does not wait for the writer,
                // it fails instantly with SQLITE_BUSY, which under a write storm
                // would turn every concurrent recall into a 500.
                Self::configure_conn(&conn)?;
                Ok(Mutex::new(conn))
            })
            .collect()
    }

    /// True when this open rebuilt the FTS index from `memories`.
    ///
    /// False is the normal case: the index is kept in step by triggers, so an
    /// in-step database pays two row counts at startup rather than a full
    /// reindex of the whole bank.
    pub fn rebuilt_fts_on_open(&self) -> bool {
        self.rebuilt_fts
    }

    /// The write connection, or [`StoreError::LockPoisoned`] if a panic unwound
    /// while the lock was held.
    ///
    /// The single place a lock is turned into an error, because the single place
    /// that can report it: `expect("store lock")` in the middle of a request
    /// handler is not a crash the caller sees, it is a *missing response* — the
    /// connection drops with no status line — while `/health` keeps answering 200
    /// and `doctor` keeps calling the server up. And the poison is sticky, so one
    /// panic would take out every later request too. A loud error the error
    /// mapper turns into the documented 500 is the whole difference.
    ///
    /// Not recovered with `PoisonError::into_inner`: a connection caught
    /// mid-transaction may hold a state no write committed, and serving reads
    /// from it would answer with data that was never durable. Refusing is
    /// diagnosable; a plausible wrong answer is not.
    fn conn(&self) -> Result<MutexGuard<'_, Connection>, StoreError> {
        self.conn
            .lock()
            .map_err(|_| StoreError::LockPoisoned)
    }

    /// A connection a read may run on, or [`StoreError::LockPoisoned`].
    ///
    /// **Writer-first, spill to the pool only under contention.** The writer's
    /// own connection is tried first with [`Mutex::try_lock`], and a read that
    /// gets it stays there; only a read that finds the writer already locked
    /// falls through to the pool. The deployment target is localhost,
    /// single-user (see `README.md`), and under those conditions this *is* the
    /// single-connection store: one reader, one connection, one page cache.
    ///
    /// The predecessor selected pool-first (stay-put, spread-on-contention) and
    /// was measured a 4-5x single-client recall regression, p50 450-658 us ->
    /// 2,568-2,643 us. **That regression does not reproduce on this box**, and
    /// the reason is worth recording rather than quietly re-asserting: at
    /// `--memories 2000` the whole database is 729,088 B, which fits inside the
    /// `cache_size=-2000` page cache *of any connection*, so a lone reader on a
    /// pool slot warms that slot exactly as it would warm the writer's. Eight
    /// alternating rounds of a 1-client-only sweep at load average 90-102 (unrelated
    /// jobs, 24 cores) gave p50 2,542-3,465 us pool-first, 2,388-3,825 us here,
    /// 2,716-8,828 us with no pool at all — one spread, three arms. The
    /// 20,000-memory case, where the store is 6.4 MB and the cache genuinely
    /// cannot hold it, is the same story for latency and a clear one for memory:
    /// 1-client p50 24.2-31.2 / 27.0-29.5 / 27.1-29.7 ms across the three, while
    /// resident growth was +3.0 MB pool-first against +0.9 MB here.
    ///
    /// So the honest claim for this policy is **memory, not latency**: a lone
    /// client keeps the write connection's cache *and* the pool's four page
    /// caches stay cold, worth ~2.1 MB of resident at 20k memories, and 19.0-20.9
    /// MB total in the 16-client mixed read/write soak against pool-first's
    /// 25.3-27.4 MB in the two rounds where that arm was not itself collapsing
    /// (`soak.rs`'s RSS ceiling is 23.8 MB, and pool-first sat on it). Multi-client
    /// throughput is unaffected: at 16/32/64 clients the interleaved rounds put
    /// this policy at 1,068-1,402 / 919-1,070 / 900-1,175 ops/s against
    /// pool-first's 671-1,406 / 852-1,341 / 797-1,014, and a *no-pool* control
    /// collapses to 243-496 ops/s at 64 clients. The pool is what delivers
    /// scaling; this decides who pays for it and when.
    ///
    /// The pool is then paid for exactly where it earns: at 4+ clients a reader
    /// that misses the writer spills to a neighbour, so recalls still overlap
    /// in the database (WAL) instead of queueing on one mutex.
    ///
    /// The cursor is advanced on the spill path **only**. A read that wins the
    /// writer never rotates off it, so a lone client cannot be cycled through
    /// cold caches — which is exactly what store-wide round-robin measured
    /// (recall p50 203 us -> 281 us at one client, aggregate 2069 -> 1295 ops/s
    /// at two: *worse than no pool at all*, because it pays a cold-cache miss on
    /// every call and never lets any cache warm).
    ///
    /// **No torn read.** The writer's `Mutex` is held for the whole of
    /// [`Store::put_doc`]'s transaction — `put_doc` takes `self.conn()` into a
    /// `let mut conn` guard, opens `conn.transaction()`, and commits before the
    /// guard is dropped — so there are exactly two states a reader can find the
    /// writer lock in, and only one of them is safe to read on:
    ///
    /// - *A writer holds it.* Then the open transaction is on the *writer's*
    ///   connection. `try_lock` returns `WouldBlock`, the read spills, and it
    ///   lands on a **different** `Connection` object. A connection only ever
    ///   sees its own uncommitted transaction, so the spilled read reads the
    ///   last committed state from the WAL — never the half-applied
    ///   supersede-and-insert. This is why the spill is not a fallback to be
    ///   avoided but the *only* correct answer when the lock is busy.
    /// - *A reader holds it.* Then no writer is inside a transaction at all
    ///   (a writer cannot open one without the same lock), so reading on the
    ///   writer's connection is a plain read of committed data.
    ///
    /// There is no third state, because the lock is what separates them. A
    /// blocking `lock()` here would instead queue, and a read that *waited* for
    /// the writer would inherit its latency while holding the connection a
    /// second reader can no longer use — the serialization this store exists to
    /// avoid.
    ///
    /// **One guard per read call**, including the two-statement
    /// [`Store::recall_inputs`]: both statements run on the connection this
    /// returns, so the hit list and the candidate pool are read the same way the
    /// single-connection store read them, and neither can be handed a different
    /// snapshot by a second pool slot.
    ///
    /// An in-memory store has no pool and goes straight to
    /// [`SqliteStore::conn`], which is why it keeps behaving exactly as it did
    /// with one connection.
    fn read_conn(&self) -> Result<MutexGuard<'_, Connection>, StoreError> {
        let n = self.reads.len();
        if n == 0 {
            return self.conn();
        }
        // Checked across the whole pool rather than per slot: a poisoned reader
        // is refused for *every* read, not just the ones that happened to land on
        // it. One poison takes the read path down, exactly as it took the whole
        // store down before there was a pool — a read that still worked off the
        // other slots would answer from a store the operator has already
        // been told is poisoned. Checked *before* the writer is tried, so a read
        // is never served off the writer precisely because a pool slot is
        // poisoned: the connection a read would otherwise have used is the
        // writer, and poison is a refusal, never a redirection.
        if self.reads.iter().any(Mutex::is_poisoned) {
            return Err(StoreError::LockPoisoned);
        }
        // Fast path. Not `lock()`: a read that blocks here would be a read that
        // queued on the write path, and every argument above for the pool
        // disappears. `WouldBlock` is the contention signal that sends it to a
        // neighbour, not an error to retry.
        match self.conn.try_lock() {
            Ok(guard) => return Ok(guard),
            Err(TryLockError::Poisoned(_)) => return Err(StoreError::LockPoisoned),
            Err(TryLockError::WouldBlock) => {}
        }
        // Spill path: the writer is mid-transaction or held by another reader.
        // Start where the last spill left off so successive spilled reads walk
        // the pool instead of piling onto one slot.
        let start = self.read_cursor.load(Ordering::Relaxed) % n;
        for offset in 0..n {
            let slot = (start + offset) % n;
            match self.reads[slot].try_lock() {
                Ok(guard) => {
                    if offset > 0 {
                        // Relaxed: this is a hint about which slot to try first,
                        // not synchronisation. A stale value costs a retry
                        // through the loop and nothing else — the `Mutex` is what
                        // makes the connection itself exclusive.
                        self.read_cursor.store(slot, Ordering::Relaxed);
                    }
                    return Ok(guard);
                }
                Err(TryLockError::Poisoned(_)) => return Err(StoreError::LockPoisoned),
                Err(TryLockError::WouldBlock) => {}
            }
        }
        self.reads[start].lock().map_err(|_| StoreError::LockPoisoned)
    }

    /// Connection-scoped pragmas for one connection, applied before any DDL.
    ///
    /// Called for the write connection *and* for every reader, because which of
    /// these is per-connection is not uniform and getting it wrong is silent:
    ///
    /// - **Per-database** (recorded in the file header, so it is already in
    ///   force on a connection that merely opens the file): `journal_mode`.
    ///   Setting it again is a no-op that returns the current mode.
    /// - **Per-connection** (a fresh handle starts at SQLite's compiled
    ///   defaults, whatever the file says): `foreign_keys`, `busy_timeout`,
    ///   `synchronous`, `cache_size`, `mmap_size`. That is the list that has to
    ///   be reapplied to every reader, and `busy_timeout` is the one that decides
    ///   whether a read waits for a writer or fails.
    ///
    /// Kept out of the schema batch on purpose: SQLite ignores `foreign_keys`
    /// inside a transaction.
    ///
    /// `synchronous=NORMAL` is the one durability trade this store makes, and it
    /// is the only reason a retain is not two orders of magnitude slower than it
    /// could be. SQLite otherwise defaults to `FULL`, which in WAL mode fsyncs
    /// the WAL on every commit: measured on this machine, one single-row commit
    /// costs ~2 ms at `FULL` against ~0.1 ms at `NORMAL`.
    ///
    /// What it costs: at `NORMAL` in WAL mode a commit is not fsynced, so a
    /// **power loss or OS crash can lose transactions committed in the last
    /// few seconds**. A process crash cannot: the WAL is in the page cache and
    /// the next opener replays it, and committed reads never see the loss. The
    /// database is never *corrupt* either way — `NORMAL` in WAL mode is
    /// corruption-safe, only lossy, and SQLite's own documentation calls it "a
    /// good choice for most applications running in WAL mode".
    ///
    /// That is the right trade for an agent's memory store: the data is a cache
    /// of what the agent said, re-writable from the conversation, and losing the
    /// last few seconds of it costs a re-save rather than data that exists
    /// nowhere else. What it would *not* be right for is a ledger or a billing
    /// table, and nothing here is one.
    ///
    /// `cache_size=-2000` and `mmap_size=0` are the page-cache settings, and
    /// one of them is a measured refusal worth keeping. Both are SQLite's
    /// defaults; stating them pins the choice against a differently-configured
    /// build rather than inheriting it. Measured at a 10k corpus, same binary
    /// both sides, 9,600 samples per arm, 4 alternating rounds:
    ///
    /// - **`cache_size` stays at the default, and the reason is structural.**
    ///   Raising it to 32 MB moved `put` p50 by -0.6% (287.7 -> 286.1 us, i.e.
    ///   nothing) and `put_tagged` p50 by -3.1% (330.7 -> 320.6 us, better in
    ///   3 of 4 rounds), while costing a reproducible **+105 KB of idle RSS**
    ///   across a 7-point sweep. A bigger page cache would pay off if recall
    ///   read the whole bank; it does not. The candidate pool is the newest 200
    ///   rows plus the BM25 hits, so the read working set is bounded no matter
    ///   how large the bank gets, and a 2 MB cache already holds it. 8 MB was
    ///   measured too (274->269, 323->311 us) for +89 KB — the same RSS within
    ///   noise for less than half the (already unresolvable) gain. A knob that
    ///   costs RSS and buys nothing measurable is a knob to leave alone. It is
    ///   also why the pool is sized in [`READ_POOL_SIZE`]: the budget is per
    ///   connection, so every reader is another 2 MB of ceiling.
    /// - **`mmap_size=0` is a refusal with a large, unambiguous cost on the
    ///   other side.** Mapped database pages count toward RSS: 64 MB and 256 MB
    ///   of `mmap_size` measured +2.0 MB and +2.3 MB of RSS under load
    ///   (13,540 -> 15,472 / 15,876 KB, +15% / +17%) with no throughput gain —
    ///   `put` p50 271 -> 276 us, `put_tagged` 315 -> 320 us, and a 1000-row
    ///   batched transaction 19.2 -> 15.2/s. Unlike the cache ceiling, an mmap
    ///   region is paid in resident pages whether or not it earns its keep.
    fn configure_conn(conn: &Connection) -> Result<(), StoreError> {
        conn.execute_batch(
            "PRAGMA journal_mode=WAL;
             PRAGMA foreign_keys=ON;
             PRAGMA busy_timeout=5000;
             PRAGMA synchronous=NORMAL;
             PRAGMA cache_size=-2000;
             PRAGMA mmap_size=0;",
        )?;
        Ok(())
    }

    /// [`Self::configure_conn`] for the store's own write connection.
    fn configure(&self) -> Result<(), StoreError> {
        let conn = self.conn()?;
        Self::configure_conn(&conn)
    }

    fn migrate(&self) -> Result<bool, StoreError> {
        let conn = self.conn()?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS banks (
               id TEXT PRIMARY KEY,
               name TEXT NOT NULL,
               config TEXT NOT NULL DEFAULT '{}'
             );
             CREATE TABLE IF NOT EXISTS memories (
               id TEXT PRIMARY KEY,
               bank_id TEXT NOT NULL REFERENCES banks(id) ON DELETE CASCADE,
               content TEXT NOT NULL,
               context TEXT,
               created_at TEXT NOT NULL DEFAULT '1970-01-01T00:00:00Z',
               document_id TEXT
             );
             CREATE INDEX IF NOT EXISTS idx_memories_bank ON memories(bank_id);
             CREATE TABLE IF NOT EXISTS memory_tags (
               memory_id TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
               tag TEXT NOT NULL,
               PRIMARY KEY(memory_id, tag)
             );
             CREATE VIRTUAL TABLE IF NOT EXISTS memories_fts USING fts5(
               content, content='memories', content_rowid='rowid', detail=none
             );",
        )?;
        // `ALTER TABLE ... ADD COLUMN` has no IF NOT EXISTS in SQLite, and the
        // CREATEs above are no-ops on a database that already has these tables,
        // so every column added after a table shipped is patched here instead of
        // declared once. A NOT NULL column with a DEFAULT backfills the rows
        // that predate it, which is what makes a legacy database readable
        // without rewriting its contents.
        if !Self::has_column(&conn, "banks", CONFIG_COLUMN)? {
            conn.execute_batch(
                "ALTER TABLE banks ADD COLUMN config TEXT NOT NULL DEFAULT '{}';",
            )?;
        }
        if !Self::has_column(&conn, "memories", CREATED_AT_COLUMN)? {
            conn.execute_batch(&format!(
                "ALTER TABLE memories ADD COLUMN created_at TEXT NOT NULL DEFAULT '{UNKNOWN_CREATED_AT}';"
            ))?;
        }
        // Nullable with no default, which is the encoding for "this bank never
        // expires anything": a column that only ever holds NULL until somebody
        // writes a policy means forgetting is opt-in per bank, and a store opened
        // by this build sweeps nothing it was not told to.
        if !Self::has_column(&conn, "banks", TTL_DAYS_COLUMN)? {
            conn.execute_batch(&format!(
                "ALTER TABLE banks ADD COLUMN {TTL_DAYS_COLUMN} INTEGER;"
            ))?;
        }
        if !Self::has_column(&conn, "memories", DOCUMENT_ID_COLUMN)? {
            conn.execute_batch("ALTER TABLE memories ADD COLUMN document_id TEXT;")?;
        }
        if !Self::has_column(&conn, "memories", CONTENT_HASH_COLUMN)? {
            conn.execute_batch("ALTER TABLE memories ADD COLUMN content_hash BLOB;")?;
        }
        // The one column this build removes rather than adds. DROP COLUMN is
        // irreversible, so a database that actually recorded something in it
        // keeps the column and says so: the data outranks the tidy schema, and
        // nothing has ever read the column, so a stale one is inert rather than
        // wrong. Same defensive shape as the `has_column` checks above.
        if Self::has_column(&conn, "banks", BACKGROUND_COLUMN)? {
            let carried: bool = conn.query_row(
                "SELECT EXISTS(SELECT 1 FROM banks WHERE background IS NOT NULL LIMIT 1)",
                [],
                |r| r.get(0),
            )?;
            if carried {
                tracing::warn!(
                    "memory-wire: banks.background still holds a value; keeping the column"
                );
            } else {
                conn.execute_batch("ALTER TABLE banks DROP COLUMN background;")?;
            }
        }
        // Both indexes follow the ALTERs: a legacy table has neither column until
        // those run, and the unique index is the dedup rule the document upsert
        // leans on. SQLite lets any number of NULLs under a unique index, so
        // untagged memories never collide.
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_memories_bank_time ON memories(bank_id, created_at);
             CREATE UNIQUE INDEX IF NOT EXISTS idx_memories_bank_doc ON memories(bank_id, document_id);",
        )?;
        // The digest column is what makes the retain-side dedup a two-column index
        // seek instead of a scan of every row in the bank, so it is indexed like
        // the others — and unlike the document index it is deliberately NOT
        // unique: two rows may legitimately hold the same content (an append
        // writes a second copy on purpose), and the store refuses duplicates on
        // its own rather than letting the index do it as a constraint error.
        // It follows the ALTERs for the same reason the other two do: a legacy
        // table has no such column until that runs.
        conn.execute_batch(
            "CREATE INDEX IF NOT EXISTS idx_memories_bank_hash
               ON memories(bank_id, content_hash);",
        )?;
        conn.execute_batch(&format!(
            "CREATE TABLE IF NOT EXISTS {MARKER_TABLE} (name TEXT PRIMARY KEY, value TEXT NOT NULL);"
        ))?;
        // Backfill the digest for rows written before the column existed, and
        // only those. Gated on a marker rather than a per-open probe: finding an
        // undigested row means walking the table, and doing that at every startup
        // would buy nothing after the first pass. `hash_content` is the same
        // SHA-256 the capture dedup window used, so a row digested here and a row
        // digested on insert hash alike — which is what lets a pre-digest row be
        // recognized as the duplicate it now is.
        //
        // Runs *before* the triggers below exist, and that ordering is load
        // bearing rather than tidy: an UPDATE fires `memories_au`, whose FTS5
        // 'delete' half is rejected as a corrupt index while that index has never
        // been populated — the exact state a legacy database is in. With no
        // trigger attached, the backfill is a plain column write.
        if !Self::marker_set(&conn, HASH_BACKFILL_MARKER)? {
            let undigested: Vec<(String, String)> = {
                // `prepare`, not `prepare_cached`: the marker above means this
                // block runs at most once per database, ever, so there is no
                // second call to amortise a parse against.
                let mut stmt = conn
                    .prepare("SELECT id, content FROM memories WHERE content_hash IS NULL")?;
                let rows = stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?)))?;
                let mut out = Vec::new();
                for row in rows {
                    out.push(row?);
                }
                out
            };
            for (id, content) in &undigested {
                conn.execute(
                    "UPDATE memories SET content_hash = ?1 WHERE id = ?2",
                    params![hash_content(content).as_slice(), id],
                )?;
            }
            conn.execute(
                &format!("INSERT OR REPLACE INTO {MARKER_TABLE} (name, value) VALUES (?1, '1')"),
                params![HASH_BACKFILL_MARKER],
            )?;
        }
        // Recreate `memories_fts` with `detail=none`, once per database.
        //
        // **What `detail=none` changes, and what it does not.** It drops the
        // per-token *positions* the index stored; the tokens themselves, which
        // docs hold them (the `docsize` table BM25 needs), the `MATCH` grammar and
        // `bm25()`'s arithmetic are all untouched. Retrieval is therefore
        // invariant by construction rather than by argument: an identical corpus
        // gives bit-identical `(rowid, bm25(...))` lists under both settings.
        // What it does disable is positional *readback* — `snippet()`,
        // `highlight()` and `offsets()` all raise `SQLITE_ERROR`, as does a
        // multi-term phrase query (`MATCH '"a b"'`). None of the three readback
        // helpers is called anywhere in this crate, and a phrase query cannot be
        // built: [`SqliteStore::fts_match_query`] splits the query on every
        // non-alphanumeric character and emits each surviving token as its own
        // quoted string joined by `OR`, so the FTS5 grammar never sees a quoted
        // run of more than one term. Single-term quoted strings are ordinary
        // terms, not phrases, and are unaffected.
        //
        // The `integrity-check` gate below is *not* one of the disabled features
        // and was measured on both settings before this was written: it passes on
        // a `detail=none` external-content table and still reports a drifted
        // index as `SQLITE_CORRUPT_VTAB`, which is what the gate depends on.
        //
        // **Why a marker and not a probe.** FTS5 persists table options in the
        // `{table}_config` shadow table, so the obvious check is to read `detail`
        // back from it. It is not there: for a `detail=full` and a `detail=none`
        // `memories_fts`, `memories_fts_config` contains exactly the same single
        // row, `version=4`, and no `detail` key in either case (verified on
        // SQLite 3.53.4). So there is nothing to read back, and "has this
        // database been converted?" has to be recorded rather than derived. This
        // is the same one-shot marker, and the same reasoning, as the digest
        // backfill above.
        //
        // The whole step is one transaction: the table is dropped and recreated,
        // and a failure between those two would leave a database whose recall
        // cannot find anything at all. Rolled back, it stays the `detail=full`
        // store it was, and the next open tries again. `unchecked_transaction`
        // because this connection is behind the store's `Mutex` and is not
        // otherwise borrowed — the same single-writer assumption the whole
        // `migrate` runs under.
        if !Self::marker_set(&conn, FTS_DETAIL_MARKER)? {
            tracing::info!("memory-wire: converting the FTS index to detail=none; rebuilding it");
            let tx = conn.unchecked_transaction()?;
            tx.execute_batch(
                "DROP TABLE IF EXISTS memories_fts;
                 CREATE VIRTUAL TABLE memories_fts USING fts5(
                   content, content='memories', content_rowid='rowid', detail=none
                 );
                 INSERT INTO memories_fts(memories_fts) VALUES('rebuild');",
            )?;
            tx.execute(
                &format!("INSERT OR REPLACE INTO {MARKER_TABLE} (name, value) VALUES (?1, '1')"),
                params![FTS_DETAIL_MARKER],
            )?;
            tx.commit()?;
        }
        // Triggers are dropped and recreated rather than created-if-absent: a
        // trigger body that drifted from this version (or was never indexing at
        // all) is otherwise kept forever, and the FTS index then quietly stops
        // tracking writes with nothing to report the divergence.
        conn.execute_batch(
            "DROP TRIGGER IF EXISTS memories_ai;
             DROP TRIGGER IF EXISTS memories_ad;
             DROP TRIGGER IF EXISTS memories_au;
             CREATE TRIGGER memories_ai AFTER INSERT ON memories BEGIN
               INSERT INTO memories_fts(rowid, content) VALUES (new.rowid, new.content);
             END;
             CREATE TRIGGER memories_ad AFTER DELETE ON memories BEGIN
               INSERT INTO memories_fts(memories_fts, rowid, content) VALUES ('delete', old.rowid, old.content);
             END;
             CREATE TRIGGER memories_au AFTER UPDATE ON memories BEGIN
               INSERT INTO memories_fts(memories_fts, rowid, content) VALUES ('delete', old.rowid, old.content);
               INSERT INTO memories_fts(rowid, content) VALUES (new.rowid, new.content);
             END;",
        )?;
        // 'rebuild' reindexes every `memories` row into the FTS table, which is
        // what actually repairs an index that drifted from the content table. A
        // rowid diff is not enough — it misses stale rows whose content changed,
        // and it is wrong for an external-content table anyway.
        //
        // Doing that on *every* open charged the whole bank at every process start
        // to repair a fault the triggers above already prevent. So ask FTS5
        // instead: `integrity-check` is the command built to answer "is this
        // index in step with the content table it mirrors", and it reports a
        // mismatch as SQLITE_CORRUPT_VTAB rather than as a count — which is the
        // only honest signal here, since an external-content table read without
        // a MATCH answers from the content table and would report itself in step
        // however empty its index was. A clean check is the no-rebuild case, so
        // the check itself is the gate: one index pass at startup instead of a
        // full reindex, and a self-healing rebuild the moment it fails.
        if conn
            .execute_batch("INSERT INTO memories_fts(memories_fts, rank) VALUES('integrity-check', 1);")
            .is_err()
        {
            tracing::warn!(
                "memory-wire: FTS index out of step with memories; rebuilding it on open"
            );
            conn.execute_batch("INSERT INTO memories_fts(memories_fts) VALUES('rebuild');")?;
            return Ok(true);
        }
        Ok(false)
    }

    /// True when a one-shot migration marker has already been recorded.
    fn marker_set(conn: &Connection, name: &str) -> Result<bool, StoreError> {
        let set: bool = conn.query_row(
            &format!("SELECT EXISTS(SELECT 1 FROM {MARKER_TABLE} WHERE name = ?1)"),
            params![name],
            |r| r.get(0),
        )?;
        Ok(set)
    }

    /// True when `table` already has `column`.
    ///
    /// `table` is always a literal from this module, never client input.
    fn has_column(conn: &Connection, table: &str, column: &str) -> Result<bool, StoreError> {
        // `prepare`, not `prepare_cached`: this runs a handful of times per
        // process open from `migrate`, never on a request path, and its text
        // interpolates the table name so every table is its own cache key.
        let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
        let mut rows = stmt.query([])?;
        while let Some(row) = rows.next()? {
            if row.get::<_, String>(1)? == column {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Upsert body shared by [`Store::put`] and [`Store::put_doc`]; the caller
    /// owns the connection (and any open transaction).
    ///
    /// `created_at` is stamped by the database on insert and is deliberately
    /// absent from the conflict branch: it records when the row came into being,
    /// so rewriting a row must not restate its age. `None` means "stamp it",
    /// which is what every normal caller passes; the `NULLIF(…,'')` in the
    /// statement is belt-and-braces for a value that no longer has a type-level
    /// way to be written.
    ///
    /// `content_hash` is `digest` — the SHA-256 of the content as written, the
    /// caller's to compute so one retain digests its content once — which is
    /// what makes the retain-side dedup a two-column index seek rather than a
    /// scan of the bank. A rewritten row restates it; the content is what it
    /// digests.
    ///
    /// A unique-index refusal on the document is reported as
    /// [`StoreError::DocumentConflict`], not as raw driver text: it is the
    /// backstop behind [`Store::put_doc`]'s pre-check, so the two paths cannot
    /// hand a caller different errors for the same mistake.
    fn put_conn(
        conn: &Connection,
        m: &Memory,
        document_id: Option<&str>,
        digest: &[u8; 32],
    ) -> Result<(), StoreError> {
        // bank_id moves too: reusing an id in a second bank must relocate the
        // row, not rewrite its content under the other bank's ownership.
        conn.execute(
            "INSERT INTO memories (id, bank_id, content, context, created_at, document_id, content_hash)
             VALUES (?1, ?2, ?3, ?4,
                     COALESCE(NULLIF(?5, ''), strftime('%Y-%m-%dT%H:%M:%fZ','now')), ?6, ?7)
             ON CONFLICT(id) DO UPDATE SET
               bank_id=excluded.bank_id,
               content=excluded.content,
               context=excluded.context,
               content_hash=excluded.content_hash,
               document_id=COALESCE(excluded.document_id, memories.document_id)",
            params![
                m.id,
                m.bank_id,
                m.content,
                m.context,
                m.created_at.as_deref(),
                document_id,
                digest.as_slice()
            ],
        )
        .map_err(|e| match is_repeated_document(&e) {
            true => StoreError::DocumentConflict,
            false => StoreError::Sqlite(e),
        })?;
        Ok(())
    }

    /// The id of the row in `bank_id` whose content digests to `digest`, if any.
    ///
    /// The lookup is the indexed one the `content_hash` column exists for, so the
    /// duplicate check costs the same whether the bank holds ten rows or a
    /// hundred thousand. Scoped to the bank: another bank's memory with the same
    /// content is a different memory. The digest is passed in rather than
    /// computed here so one retain hashes its content once.
    fn find_duplicate(
        conn: &Connection,
        bank_id: &str,
        digest: &[u8; 32],
    ) -> Result<Option<String>, StoreError> {
        Ok(conn
            .query_row(
                "SELECT id FROM memories WHERE bank_id=?1 AND content_hash=?2 LIMIT 1",
                params![bank_id, digest.as_slice()],
                |r| r.get(0),
            )
            .optional()?)
    }

    /// Every memory in one bank, in insertion order.
    ///
    /// Untagged on purpose: recall used to be the tag-filtered caller, and it now
    /// goes through [`Self::recall_pool_conn`] instead — so a filter parameter
    /// here would have exactly one possible value, which is the whole bank.
    fn list_conn(conn: &Connection, bank_id: &str) -> Result<Vec<Memory>, StoreError> {
        let mut stmt = conn
            .prepare_cached(&format!(
                "SELECT {MEMORY_COLUMNS} FROM memories WHERE bank_id=? ORDER BY rowid"
            ))?;
        let rows = stmt.query_map(params![bank_id], memory_from_row)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// One page of a bank's memories in insertion order.
    fn list_page_conn(
        conn: &Connection,
        bank_id: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Memory>, StoreError> {
        let mut stmt = conn.prepare_cached(&format!(
            "SELECT {MEMORY_COLUMNS} FROM memories WHERE bank_id=? ORDER BY rowid LIMIT ? OFFSET ?"
        ))?;
        // saturating casts: limit/offset are untrusted, and a wrap would read the
        // wrong page rather than fail.
        let limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let offset = i64::try_from(offset).unwrap_or(i64::MAX);
        let rows = stmt.query_map(params![bank_id, limit, offset], memory_from_row)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// The bounded candidate pool one recall scores over: the newest `limit`
    /// rows of the bank, plus every BM25 hit whatever its age.
    ///
    /// One ordered statement, so the pool is a single read that cannot disagree
    /// with the hit list it is built from. The window is a `rowid IN (...)`
    /// subquery rather than `LIMIT` on the outer select because a plain `LIMIT`
    /// would silently drop the hits the window missed — the union has to be in
    /// the same `WHERE` as the window for that.
    ///
    /// The tag filter rides on *both* selects, not just the outer one, and that
    /// is the whole point of it here: a window filtered only by the outer clause
    /// would be the newest 200 rows of the bank whether or not they carry the
    /// requested tag, so a selective tag on a large bank could come back with an
    /// almost-empty pool and quietly under-rank exactly the memories the caller
    /// asked for. The filter has to be inside the window for the window to be a
    /// window over the *candidate set*.
    fn recall_pool_conn(
        conn: &Connection,
        bank_id: &str,
        tags: &[String],
        limit: usize,
        hit_ids: &[&str],
    ) -> Result<Vec<Memory>, StoreError> {
        // `IN ()` is a syntax error, so the union clause only exists when there
        // is a hit to union in.
        let union = if hit_ids.is_empty() {
            String::new()
        } else {
            format!(" OR id IN ({})", vec!["?"; hit_ids.len()].join(","))
        };
        let tags_sql = tag_predicate("memories", tags);
        let sql = format!(
            "SELECT {MEMORY_COLUMNS} FROM memories WHERE bank_id=?{tags_sql} \
             AND (rowid IN (SELECT rowid FROM memories WHERE bank_id=?{tags_sql} \
                            ORDER BY rowid DESC LIMIT ?){union}) \
             ORDER BY rowid"
        );
        let mut stmt = conn.prepare(&sql)?;
        // `prepare`, not `prepare_cached`, on purpose: the statement's *text* is
        // built from the hit count and the tag count, so this one call site is up
        // to ~1000 distinct SQL strings (0-50 hit ids x 0-20 tags) against a
        // 16-slot LRU. Caching it would evict the entry the next recall wants
        // and pin 16 prepared statements alive for nothing. Parsing is the price
        // of a variable query, and it is not what the recall budget goes on.
        // Bound in the order the statement binds them: the outer bank, the outer
        // tag filter, then the window's own bank and tag filter, its limit, and
        // finally the hit ids. saturating cast: a wrap would silently read a
        // different window.
        let win_limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let mut args: Vec<&dyn ToSql> = vec![&bank_id];
        args.extend(tags.iter().map(|t| t as &dyn ToSql));
        args.push(&bank_id);
        args.extend(tags.iter().map(|t| t as &dyn ToSql));
        args.push(&win_limit);
        args.extend(hit_ids.iter().map(|id| id as &dyn ToSql));
        let rows = stmt.query_map(params_from_iter(args), memory_from_row)?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// FTS5 BM25 hits for one bank, optionally narrowed by the same tag filter
    /// the [`Self::recall_pool_conn`] side uses — the two streams must agree, or
    /// the fusion would score memories the list stream never offered.
    fn keyword_search_conn(
        conn: &Connection,
        bank_id: &str,
        query: &str,
        limit: usize,
        tags: &[String],
    ) -> Result<KeywordHits, StoreError> {
        let m = Self::fts_match_query(query);
        if m.is_empty() {
            return Ok(Vec::new());
        }
        let sql = format!(
            "SELECT memories.id, bm25(memories_fts) AS rank
             FROM memories_fts JOIN memories ON memories.rowid = memories_fts.rowid
             WHERE memories_fts MATCH ? AND memories.bank_id = ?{}
             ORDER BY rank LIMIT ?",
            tag_predicate("memories", tags)
        );
        let mut stmt = conn.prepare(&sql)?;
        // `prepare`, not `prepare_cached`: as in `recall_pool_conn`, the text
        // carries one `?` per tag, so the key space is the tag count (0-20)
        // against the same 16-slot LRU. The tag-free shape - the common one - is
        // a single key, but caching it would let a tag-filtered recall evict it.
        // saturating cast, like every other untrusted limit in this file: a wrap
        // would silently bound the stream to some other window than the caller
        // asked for.
        let rank_limit = i64::try_from(limit).unwrap_or(i64::MAX);
        let mut args: Vec<&dyn ToSql> = vec![&m, &bank_id];
        args.extend(tags.iter().map(|t| t as &dyn ToSql));
        args.push(&rank_limit);
        let rows = stmt.query_map(params_from_iter(args), |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, f64>(1)?))
        })?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    /// Build a safe FTS5 MATCH query: quoted tokens joined with OR.
    ///
    /// Returns an empty string when no usable tokens exist (caller skips FTS).
    pub fn fts_match_query(query: &str) -> String {
        let mut toks = Vec::new();
        for tok in query.split(|c: char| !c.is_alphanumeric()) {
            let t = tok.trim().to_lowercase();
            if t.len() >= 2 {
                toks.push(format!("\"{}\"", t.replace('\"', "")));
            }
        }
        toks.join(" OR ")
    }

    /// BM25 keyword search scoped to one bank (FTS5 `bm25`, lower rank is better).
    ///
    /// Returns `(memory_id, rank)` ordered best-first, up to `limit` rows.
    pub fn keyword_search_fts(
        &self,
        bank_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<KeywordHits, StoreError> {
        let conn = self.read_conn()?;
        Self::keyword_search_conn(&conn, bank_id, query, limit, &[])
    }
}

impl Store for SqliteStore {
    fn put_bank(&self, bank: &Bank) -> Result<(), StoreError> {
        if bank.id.trim().is_empty() {
            return Err(StoreError::InvalidBank);
        }
        let conn = self.conn()?;
        // Insert-if-missing: a retain re-declares the bank it writes to and
        // carries no metadata, so an upsert here would rename a bank every time
        // something was written to it.
        conn.execute(
            "INSERT OR IGNORE INTO banks (id, name) VALUES (?1, ?2)",
            params![bank.id, bank.name],
        )?;
        Ok(())
    }

    fn put(&self, m: &Memory) -> Result<(), StoreError> {
        if m.bank_id.trim().is_empty() {
            return Err(StoreError::InvalidBank);
        }
        let conn = self.conn()?;
        Self::put_conn(&conn, m, None, &hash_content(&m.content))
    }

    fn put_tagged(&self, m: &Memory, tags: &[String]) -> Result<(), StoreError> {
        self.put_doc(m, tags, None, UpdateMode::Replace).map(|_| ())
    }

    fn put_doc(
        &self,
        m: &Memory,
        tags: &[String],
        document_id: Option<&str>,
        update_mode: UpdateMode,
    ) -> Result<String, StoreError> {
        if m.bank_id.trim().is_empty() {
            return Err(StoreError::InvalidBank);
        }
        let mut conn = self.conn()?;
        let tx = conn.transaction()?;
        // One digest for the whole write: `find_duplicate` asks what the bank
        // already holds and `put_conn` records what is being written, and both
        // questions are about the same bytes. SHA-256 over a memory's content
        // is not cheap enough to pay twice per retain.
        let digest = hash_content(&m.content);
        // Dedup: content this bank already holds is the same memory, so hand back
        // the row that holds it rather than store a second copy. Asked first, on
        // the digest index that decides the question and inside the same
        // transaction as the insert — and on the one connection that writes, which
        // is exclusive to writers, so nothing can slip in between the check and
        // the write.
        //
        // Scoped to the no-document path. A caller holding a `document_id` is
        // choosing upsert semantics explicitly, and handing it the id of some
        // other row would break the one-revision-per-document rule that the
        // append/replace split exists to keep. An append under a reused id is
        // still refused with [`StoreError::DocumentConflict`], not deduped.
        //
        // A deduped write touches nothing at all — not even the tags — so
        // repeating a retain is idempotent rather than a second, differently
        // tagged copy of a memory that already exists. The `!= m.id` guard is
        // what keeps a re-put of the *same row* (a rewrite that changes its tags
        // and nothing else) an update: a row already sitting under this id is not
        // a duplicate of itself, so the write has to go through.
        if document_id.is_none() {
            if let Some(id) = Self::find_duplicate(&tx, &m.bank_id, &digest)? {
                if id != m.id {
                    return Ok(id);
                }
            }
        }
        if let Some(doc) = document_id {
            if update_mode == UpdateMode::Append {
                // The unique index is the guarantee, but its error is a driver
                // string about a constraint, which names neither the caller's
                // mistake nor the fix. Ask first, on the index that decides the
                // question, inside the same transaction as the insert: writes are
                // serialized on the one write connection, so nothing can slip in
                // between.
                let taken: bool = tx.query_row(
                    "SELECT EXISTS(SELECT 1 FROM memories WHERE bank_id=?1 AND document_id=?2)",
                    params![m.bank_id, doc],
                    |r| r.get(0),
                )?;
                if taken {
                    return Err(StoreError::DocumentConflict);
                }
            } else {
                // Supersede the prior revision first, inside the same
                // transaction as the insert: a reader must never see the new
                // revision with the old tag set, and a failure must not leave
                // the document with no revision at all. The old row's tags leave
                // with it through the FK cascade.
                tx.execute(
                    "DELETE FROM memories WHERE bank_id=?1 AND document_id=?2",
                    params![m.bank_id, doc],
                )?;
            }
        }
        Self::put_conn(&tx, m, document_id, &digest)?;
        // The caller's list is the whole tag set: a row relocated to another bank
        // must not drag the previous owner's tags along with it.
        tx.execute("DELETE FROM memory_tags WHERE memory_id = ?1", params![m.id])?;
        // Prepared once, executed per tag: the statement is the same every time,
        // and re-preparing it inside the loop is per-tag parse work for a
        // statement the database has already seen. `prepare_cached` rather than
        // `prepare` so the parse is paid once per connection rather than once
        // per retain.
        let mut insert_tag =
            tx.prepare_cached("INSERT OR IGNORE INTO memory_tags (memory_id, tag) VALUES (?1, ?2)")?;
        for tag in tags {
            insert_tag.execute(params![m.id, tag])?;
        }
        drop(insert_tag);
        tx.commit()?;
        Ok(m.id.clone())
    }

    fn delete(&self, bank_id: &str, id: &str) -> Result<bool, StoreError> {
        let conn = self.conn()?;
        // Tags go with the row: memory_tags references memories ON DELETE CASCADE
        // and the connection sets foreign_keys=ON, so no second statement runs.
        let removed = conn.execute(
            "DELETE FROM memories WHERE bank_id=?1 AND id=?2",
            params![bank_id, id],
        )?;
        Ok(removed > 0)
    }

    fn get(&self, bank_id: &str, id: &str) -> Result<Option<Memory>, StoreError> {
        let conn = self.read_conn()?;
        let mut stmt = conn.prepare_cached(&format!(
            "SELECT {MEMORY_COLUMNS} FROM memories WHERE id=?1 AND bank_id=?2"
        ))?;
        let mut rows = stmt.query_map(params![id, bank_id], memory_from_row)?;
        match rows.next() {
            None => Ok(None),
            Some(row) => Ok(Some(row?)),
        }
    }

    fn list(&self, bank_id: &str) -> Result<Vec<Memory>, StoreError> {
        let conn = self.read_conn()?;
        Self::list_conn(&conn, bank_id)
    }

    fn list_page(
        &self,
        bank_id: &str,
        limit: usize,
        offset: usize,
    ) -> Result<Vec<Memory>, StoreError> {
        let conn = self.read_conn()?;
        Self::list_page_conn(&conn, bank_id, limit, offset)
    }

    fn bank_stats(&self, bank_id: &str) -> Result<BankStats, StoreError> {
        let conn = self.read_conn()?;
        let memories: usize = conn.query_row(
            "SELECT COUNT(*) FROM memories WHERE bank_id=?1",
            params![bank_id],
            |r| r.get(0),
        )?;
        let tags: usize = conn.query_row(
            "SELECT COUNT(DISTINCT t.tag) FROM memory_tags t
             JOIN memories m ON m.id = t.memory_id WHERE m.bank_id=?1",
            params![bank_id],
            |r| r.get(0),
        )?;
        // MIN/MAX over TEXT is a byte compare, which is the right order for
        // fixed-width RFC 3339 UTC, and puts the epoch sentinel first — so a bank
        // still holding pre-timestamp rows reports "unknown" as its floor rather
        // than a date the migration never chose.
        let (oldest, newest): (Option<String>, Option<String>) = conn.query_row(
            "SELECT MIN(created_at), MAX(created_at) FROM memories WHERE bank_id=?1",
            params![bank_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )?;
        Ok(BankStats { memories, tags, oldest, newest })
    }

    fn keyword_search(
        &self,
        bank_id: &str,
        query: &str,
        limit: usize,
    ) -> Result<KeywordHits, StoreError> {
        self.keyword_search_fts(bank_id, query, limit)
    }

    fn recall_inputs(
        &self,
        bank_id: &str,
        query: &str,
        tags: &[String],
        fts_limit: usize,
    ) -> Result<RecallInputs, StoreError> {
        let conn = self.read_conn()?;
        // BM25 first: the pool it feeds is bounded by the window *plus* these
        // hits, so the hit list has to be in hand before the pool can be asked
        // for. Doing it the other way round would need a second FTS scan.
        let hits = Self::keyword_search_conn(&conn, bank_id, query, fts_limit, tags)?;
        // Borrowed, not cloned: these are SQL bind parameters that are only ever
        // read, and `hits` outlives the statement.
        let hit_ids: Vec<&str> = hits.iter().map(|(id, _)| id.as_str()).collect();
        let pool = Self::recall_pool_conn(&conn, bank_id, tags, RECALL_POOL_LIMIT, &hit_ids)?;
        Ok((pool, hits))
    }

    fn get_bank_config(&self, bank_id: &str) -> Result<Option<String>, StoreError> {
        let conn = self.read_conn()?;
        let mut stmt = conn.prepare_cached("SELECT config FROM banks WHERE id = ?1")?;
        let mut rows = stmt.query_map(params![bank_id], |row| row.get::<_, String>(0))?;
        match rows.next() {
            None => Ok(None),
            Some(row) => Ok(Some(row?)),
        }
    }

    fn set_bank_config(&self, bank_id: &str, config: &str) -> Result<(), StoreError> {
        let parsed: serde_json::Value =
            serde_json::from_str(config).map_err(|_| StoreError::InvalidConfig)?;
        if !parsed.is_object() {
            return Err(StoreError::InvalidConfig);
        }
        let conn = self.conn()?;
        // No row updated means the bank was never created. The FK is not enough
        // here because an UPDATE never inserts, so the miss is detected and
        // reported instead of quietly configuring nothing.
        //
        // The config JSON stays the wire format and the whole truth about the
        // bank; `ttl_days` is projected onto its own column in the same
        // statement so a sweep reads every bank's policy off one pass. One write
        // path writes both, so they cannot drift — and because the value is
        // re-derived from the JSON on every replace, a config that drops the key
        // clears the column rather than leaving a policy behind that the config
        // no longer states.
        let changed = conn.execute(
            "UPDATE banks SET config = ?1, ttl_days = ?2 WHERE id = ?3",
            params![config, ttl_days_of(&parsed), bank_id],
        )?;
        if changed == 0 {
            return Err(StoreError::UnknownBank);
        }
        Ok(())
    }

    fn bank_ttls(&self) -> Result<Vec<(String, Option<u32>)>, StoreError> {
        let conn = self.read_conn()?;
        let mut stmt = conn.prepare_cached("SELECT id, ttl_days FROM banks ORDER BY id")?;
        let rows = stmt.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Option<u32>>(1)?)))?;
        let mut out = Vec::new();
        for row in rows {
            out.push(row?);
        }
        Ok(out)
    }

    fn expire_before(&self, bank_id: &str, cutoff: &str, dry_run: bool) -> Result<usize, StoreError> {
        let mut conn = self.conn()?;
        // One transaction per bank: a bank that fails to expire leaves every other
        // bank swept, rather than a half-expired bank that no single rerun can
        // describe.
        let tx = conn.transaction()?;
        let changed = if dry_run {
            tx.query_row(
                "SELECT COUNT(*) FROM memories WHERE bank_id = ?1 AND created_at < ?2",
                params![bank_id, cutoff],
                |r| r.get(0),
            )?
        } else {
            // No `content` predicate and no `id` list: the `memories_ad` trigger
            // retires the FTS row for every deleted row, and `memory_tags` cascades
            // off the foreign key, so one statement leaves the whole store
            // consistent. A row whose `created_at` is the epoch sentinel — a
            // memory that predates timestamping — reads as older than any cutoff
            // and is swept, which is the honest reading of "unknown age".
            tx.execute(
                "DELETE FROM memories WHERE bank_id = ?1 AND created_at < ?2",
                params![bank_id, cutoff],
            )?
        };
        tx.commit()?;
        Ok(changed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::{Bank, Memory};

    fn bank(id: &str) -> Bank {
        Bank {
            id: id.to_string(),
            name: id.to_string(),
        }
    }

    fn mem(bank: &str, id: &str) -> Memory {
        Memory {
            id: id.to_string(),
            bank_id: bank.to_string(),
            content: format!("content-{id}"),
            context: None,
            created_at: None,
        }
    }

    #[test]
    fn put_should_roundtrip_get_when_same_bank() {
        let s = SqliteStore::open_in_memory().expect("open");
        s.put_bank(&bank("a")).expect("bank");
        s.put(&mem("a", "m1")).expect("put");
        let got = s.get("a", "m1").expect("get").expect("some");
        assert_eq!(got.content, "content-m1");
    }

    #[test]
    fn get_should_return_none_when_bank_differs() {
        let s = SqliteStore::open_in_memory().expect("open");
        s.put_bank(&bank("a")).expect("bank a");
        s.put_bank(&bank("b")).expect("bank b");
        s.put(&mem("a", "m1")).expect("put");
        assert!(s.get("b", "m1").expect("get").is_none());
    }

    #[test]
    fn list_should_filter_by_bank_only() {
        let s = SqliteStore::open_in_memory().expect("open");
        s.put_bank(&bank("a")).expect("bank a");
        s.put_bank(&bank("b")).expect("bank b");
        s.put(&mem("a", "m1")).expect("put a");
        s.put(&mem("b", "m2")).expect("put b");
        assert_eq!(s.list("a").expect("list").len(), 1);
    }

    fn content_mem(bank: &str, id: &str, content: &str) -> Memory {
        Memory {
            id: id.to_string(),
            bank_id: bank.to_string(),
            content: content.to_string(),
            context: None,
            created_at: None,
        }
    }

    #[test]
    fn keyword_should_rank_bm25_best_first() {
        let s = SqliteStore::open_in_memory().expect("open");
        s.put_bank(&bank("a")).expect("bank");
        s.put(&content_mem("a", "m1", "auth uses jose middleware for jwt")).expect("put");
        s.put(&content_mem("a", "m2", "cafeteria menu noodles rice")).expect("put");
        let hits = s.keyword_search("a", "jose jwt", 10).expect("search");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].0, "m1");
    }

    #[test]
    fn keyword_should_isolate_banks() {
        let s = SqliteStore::open_in_memory().expect("open");
        s.put_bank(&bank("a")).expect("bank a");
        s.put_bank(&bank("b")).expect("bank b");
        s.put(&content_mem("a", "m1", "jose middleware secrets")).expect("put");
        assert!(s.keyword_search("b", "jose", 10).expect("search").is_empty());
    }

    #[test]
    fn keyword_should_ignore_short_tokens() {
        assert!(SqliteStore::fts_match_query("a I").is_empty());
    }

    fn tmp_db(tag: &str) -> PathBuf {
        let mut p = std::env::temp_dir();
        p.push(format!("memory-wire-{tag}-{}.db", uuid::Uuid::new_v4()));
        p
    }

    fn rm_db(path: &Path) {
        let _ = std::fs::remove_file(path);
        let _ = std::fs::remove_file(path.with_extension("db-wal"));
        let _ = std::fs::remove_file(path.with_extension("db-shm"));
    }

    #[test]
    fn default_db_path_should_follow_xdg_then_local_share() {
        // One test for both branches: env is process-global, so touching
        // XDG_DATA_HOME from two parallel tests would race.
        let prev_xdg = std::env::var_os("XDG_DATA_HOME");
        let prev_home = std::env::var_os("HOME");
        let (xdg, home) = (PathBuf::from("/tmp/xdg-probe"), PathBuf::from("/tmp/home-probe"));

        std::env::set_var("XDG_DATA_HOME", &xdg);
        std::env::set_var("HOME", &home);
        assert_eq!(default_db_path(), xdg.join("memory-wire").join("memory.db"));

        std::env::remove_var("XDG_DATA_HOME");
        assert_eq!(
            default_db_path(),
            home.join(".local/share").join("memory-wire").join("memory.db")
        );

        match prev_xdg {
            Some(v) => std::env::set_var("XDG_DATA_HOME", v),
            None => std::env::remove_var("XDG_DATA_HOME"),
        }
        match prev_home {
            Some(v) => std::env::set_var("HOME", v),
            None => std::env::remove_var("HOME"),
        }
    }

    #[test]
    fn file_db_should_persist_across_reopen() {
        let path = tmp_db("persist");
        {
            let s = SqliteStore::open(&path).expect("open");
            s.put_bank(&bank("a")).expect("bank");
            s.put(&content_mem("a", "m1", "durable jose content")).expect("put");
        }
        let s = SqliteStore::open(&path).expect("reopen");
        let got = s.get("a", "m1").expect("get").expect("some");
        assert_eq!(got.content, "durable jose content");
        assert_eq!(s.keyword_search("a", "jose", 10).expect("search").len(), 1);
        rm_db(&path);
    }

    // A read pool over the wrong database is the quietest possible break: every
    // read answers, the bank simply looks empty, and nothing anywhere raises.
    // So the pool is pinned to the *same* database the writes land in, and to
    // writes committed after it was built.
    #[test]
    fn a_file_store_should_serve_reads_from_the_same_database_its_writes_landed_in() {
        let path = tmp_db("pool-shared");
        let s = SqliteStore::open(&path).expect("open");
        s.put_bank(&bank("a")).expect("bank");
        s.put(&content_mem("a", "m1", "auth uses jose middleware")).expect("put");
        assert_eq!(s.list("a").expect("list").len(), 1, "a pool reader must see the first write");
        let (pool, hits) = s.recall_inputs("a", "jose", &[], 10).expect("recall inputs");
        assert_eq!((pool.len(), hits.len()), (1, 1));

        s.put(&content_mem("a", "m2", "rate limiting via token bucket")).expect("put");
        assert_eq!(
            s.list("a").expect("list").len(),
            2,
            "and the second, committed after the pool was built — a reader that missed it is a stale snapshot"
        );
        assert_eq!(s.keyword_search("a", "token bucket", 10).expect("search").len(), 1);
        assert_eq!(s.bank_stats("a").expect("stats").memories, 2);

        // The pool is a pool: a held reader is not handed out again, which is
        // the whole reason a second recall can run while the first is still
        // inside SQLite.
        let held = s.read_conn().expect("read connection");
        let other = s.read_conn().expect("second read connection");
        assert!(
            !std::ptr::eq(&*held, &*other),
            "two live reads were given the same connection, so the pool cannot overlap them"
        );
        drop(held);
        drop(other);
        rm_db(&path);
    }

    // A `:memory:` database is per-connection: a second connection to one is a
    // *second, empty* database. That is why an in-memory store keeps the single
    // connection and no pool, and it is the reason the choice is stated here
    // rather than left to a reader of the struct to infer.
    #[test]
    fn an_in_memory_store_should_keep_one_connection_and_a_file_store_a_pool() {
        assert!(
            SqliteStore::open_in_memory().expect("open").reads.is_empty(),
            "a pool over a `:memory:` database is N empty databases"
        );
        let path = tmp_db("pool-size");
        let s = SqliteStore::open(&path).expect("open");
        assert_eq!(s.reads.len(), READ_POOL_SIZE);
        rm_db(&path);
    }

    // `foreign_keys` and `busy_timeout` are per-connection, so a reader that
    // skipped `configure_conn` would fail differently from the writer, and
    // silently: without the busy timeout a read does not queue behind the
    // writer, it errors with SQLITE_BUSY, which is a 500 under a write storm.
    #[test]
    fn every_read_connection_should_carry_the_pragmas_the_write_connection_does() {
        let path = tmp_db("pool-pragmas");
        let s = SqliteStore::open(&path).expect("open");
        for (i, slot) in s.reads.iter().enumerate() {
            let conn = slot.lock().expect("read connection");
            let fk: i64 = conn.query_row("PRAGMA foreign_keys", [], |r| r.get(0)).expect("fk");
            let busy: i64 = conn.query_row("PRAGMA busy_timeout", [], |r| r.get(0)).expect("busy");
            let mode: String = conn.query_row("PRAGMA journal_mode", [], |r| r.get(0)).expect("mode");
            assert_eq!(fk, 1, "reader {i} must enforce foreign keys like the writer");
            assert_eq!(busy, 5000, "reader {i} must wait for the writer, not fail with SQLITE_BUSY");
            assert_eq!(mode, "wal", "reader {i} must be on the writer's journal");
        }
        rm_db(&path);
    }

    // A poison now has two homes, and the read one must behave like the write
    // one: an error, never a panic, and never a read quietly served off a
    // different slot. Same reasoning as
    // `a_poisoned_store_lock_should_refuse_loudly_instead_of_panicking`.
    #[test]
    fn a_poisoned_read_connection_should_refuse_every_read_rather_than_being_skipped() {
        let path = tmp_db("pool-poison");
        let s = SqliteStore::open(&path).expect("open");
        s.put_bank(&bank("a")).expect("bank");
        s.put(&content_mem("a", "m1", "jose middleware")).expect("put");

        let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _guard = s.reads[0].lock().expect("lock");
            panic!("deliberate: a panic while a read connection is held");
        }));
        assert!(unwound.is_err(), "the fixture must actually poison a reader");

        // Including the reads that would have landed on the seven healthy slots:
        // a store the operator has been told is poisoned must not go on answering
        // out of the rest of the pool.
        assert!(matches!(s.list("a"), Err(StoreError::LockPoisoned)));
        assert!(matches!(s.get("a", "m1"), Err(StoreError::LockPoisoned)));
        assert!(matches!(s.bank_stats("a"), Err(StoreError::LockPoisoned)));
        assert!(matches!(s.recall_inputs("a", "jose", &[], 10), Err(StoreError::LockPoisoned)));

        // The write path is a different lock: a read poison is not a write poison.
        s.put(&content_mem("a", "m2", "another jose note")).expect("writes still work");
        rm_db(&path);
    }

    // The regression this policy is meant to remove: a lone reader is served the
    // write connection, whose page cache is the one the write path has been
    // warming all along, and the pool's own caches stay cold. That is worth
    // ~2.1 MB of resident memory at 20k memories (measured) — the latency half
    // of the story does not reproduce at 2,000, where the store fits inside any
    // connection's page cache; see [`SqliteStore::read_conn`].
    #[test]
    fn a_lone_reader_should_be_served_the_write_connection_and_never_rotate_off_it() {
        let path = tmp_db("pool-writer-first");
        let s = SqliteStore::open(&path).expect("open");
        s.put_bank(&bank("a")).expect("bank");
        s.put(&content_mem("a", "m1", "jose middleware")).expect("put");

        // The address the writer's `Connection` lives at, captured by briefly
        // locking it. The `Mutex` owns the connection in place, so the address
        // is stable once the guard is dropped — which is what makes pointer
        // identity comparable across two separate acquisitions.
        let writer_at = {
            let g = s.conn.lock().expect("write connection");
            &*g as *const Connection as usize
        };
        for i in 0..8 {
            let guard = s.read_conn().expect("read connection");
            assert_eq!(
                &*guard as *const Connection as usize,
                writer_at,
                "read {i} was served off a pool connection while nothing was contending, \
                 so it leaves the writer's warm page cache — and four cold ones — \
                 for contention it never experiences"
            );
            drop(guard);
        }
        assert_eq!(
            s.read_cursor.load(Ordering::Relaxed),
            0,
            "the fast path must not advance the spill cursor: rotating an uncontended \
             reader is store-wide round-robin, which measured worse than no pool"
        );
        rm_db(&path);
    }

    // The other half of the contract: under contention the read *spills*, and
    // spilling is the only correct answer rather than a fallback. The writer's
    // lock is held here, which is either a write mid-transaction or another
    // reader; either way `try_lock` reports `WouldBlock` and the read must land
    // on a different connection instead of blocking.
    #[test]
    fn a_read_while_the_writer_is_locked_should_spill_instead_of_blocking() {
        let path = tmp_db("pool-spill");
        let s = SqliteStore::open(&path).expect("open");
        s.put_bank(&bank("a")).expect("bank");
        s.put(&content_mem("a", "m1", "jose middleware")).expect("put");

        let writer_at = {
            let g = s.conn.lock().expect("write connection");
            &*g as *const Connection as usize
        };
        // Every pool slot's address, captured while all of them are free — the
        // positive half of "it spilled", rather than only ruling out the writer.
        let pool_ats: Vec<usize> = s
            .reads
            .iter()
            .map(|slot| &*slot.lock().expect("pool slot") as *const Connection as usize)
            .collect();
        let held = s.conn.lock().expect("write connection");

        let started = std::time::Instant::now();
        let spilled = s
            .read_conn()
            .expect("a read under a held writer must not block or panic");
        let elapsed = started.elapsed();
        let spilled_at = &*spilled as *const Connection as usize;

        assert!(
            pool_ats.contains(&spilled_at) && spilled_at != writer_at,
            "the spilled read must take a pool connection, never the writer's own — \
             that is the state the spill exists to avoid, where an open transaction \
             could be read"
        );
        assert!(
            elapsed < std::time::Duration::from_millis(250),
            "the read waited {elapsed:?} on the writer; a blocked read is the \
             serialization this store exists to avoid"
        );

        // And the spill is a *usable* connection against the same database, not
        // a second empty one: it sees the committed write.
        let got: String = spilled
            .query_row("SELECT content FROM memories WHERE id='m1'", [], |r| r.get(0))
            .expect("spilled read must see committed state");
        assert_eq!(got, "jose middleware");

        drop(spilled);
        drop(held);
        rm_db(&path);
    }

    #[test]
    fn migrate_should_rebuild_a_drifted_fts_index() {
        let path = tmp_db("rebuild");
        {
            let s = SqliteStore::open(&path).expect("open");
            s.put_bank(&bank("a")).expect("bank");
            s.put(&content_mem("a", "m1", "reindex me jose")).expect("put");
        }
        // Simulate a legacy/desynced index: wipe FTS behind the store's back.
        {
            let conn = Connection::open(&path).expect("raw open");
            conn.execute("DELETE FROM memories_fts", []).expect("wipe fts");
        }
        let s = SqliteStore::open(&path).expect("reopen");
        let hits = s.keyword_search("a", "jose", 10).expect("search");
        assert_eq!(hits.len(), 1, "migrate must rebuild the FTS index");
        assert_eq!(hits[0].0, "m1");
        rm_db(&path);
    }

    /// The DDL FTS5 recorded for `memories_fts`, which is the only place the
    /// `detail=` option is readable after the fact.
    fn fts_ddl(path: &Path) -> String {
        let conn = Connection::open(path).expect("raw open");
        conn.query_row(
            "SELECT sql FROM sqlite_master WHERE type='table' AND name='memories_fts'",
            [],
            |r| r.get::<_, Option<String>>(0),
        )
        .expect("memories_fts must exist")
        .expect("sqlite_master.sql is never null for a table")
    }

    /// Build the database an older build would have left behind: the same schema
    /// with the FTS table at the *default* `detail=full`, populated, and a
    /// `schema_markers` table carrying the digest-backfill marker but not the
    /// `detail=none` one. Written by hand rather than produced by the code under
    /// test, so the test cannot pass by construction.
    fn legacy_detail_full_db(path: &Path, rows: &[(&str, &str)]) {
        let conn = Connection::open(path).expect("raw open");
        conn.execute_batch(
            "CREATE TABLE banks (
               id TEXT PRIMARY KEY, name TEXT NOT NULL,
               config TEXT NOT NULL DEFAULT '{}', ttl_days INTEGER);
             CREATE TABLE memories (
               id TEXT PRIMARY KEY,
               bank_id TEXT NOT NULL REFERENCES banks(id) ON DELETE CASCADE,
               content TEXT NOT NULL, context TEXT,
               created_at TEXT NOT NULL DEFAULT '1970-01-01T00:00:00Z',
               document_id TEXT, content_hash BLOB);
             CREATE TABLE memory_tags (
               memory_id TEXT NOT NULL REFERENCES memories(id) ON DELETE CASCADE,
               tag TEXT NOT NULL, PRIMARY KEY(memory_id, tag));
             CREATE VIRTUAL TABLE memories_fts USING fts5(
               content, content='memories', content_rowid='rowid');
             CREATE TABLE schema_markers (name TEXT PRIMARY KEY, value TEXT NOT NULL);
             INSERT INTO schema_markers VALUES ('content_hash_backfilled', '1');
             INSERT INTO banks (id, name) VALUES ('a', 'a');
             CREATE TRIGGER memories_ai AFTER INSERT ON memories BEGIN
               INSERT INTO memories_fts(rowid, content) VALUES (new.rowid, new.content);
             END;
             CREATE TRIGGER memories_ad AFTER DELETE ON memories BEGIN
               INSERT INTO memories_fts(memories_fts, rowid, content)
                 VALUES ('delete', old.rowid, old.content);
             END;
             CREATE TRIGGER memories_au AFTER UPDATE ON memories BEGIN
               INSERT INTO memories_fts(memories_fts, rowid, content)
                 VALUES ('delete', old.rowid, old.content);
               INSERT INTO memories_fts(rowid, content) VALUES (new.rowid, new.content);
             END;",
        )
        .expect("legacy schema");
        for (id, content) in rows {
            conn.execute(
                "INSERT INTO memories (id, bank_id, content) VALUES (?1, 'a', ?2)",
                params![id, content],
            )
            .expect("legacy insert");
        }
    }

    #[test]
    fn a_fresh_database_should_get_a_detail_none_fts_index() {
        let path = tmp_db("detail-fresh");
        let s = SqliteStore::open(&path).expect("open");
        drop(s);
        let ddl = fts_ddl(&path);
        assert!(
            ddl.contains("detail=none"),
            "a fresh database must not be built with the default detail: {ddl}"
        );
        rm_db(&path);
    }

    #[test]
    fn a_legacy_detail_full_database_should_be_migrated_to_detail_none() {
        let path = tmp_db("detail-migrate");
        legacy_detail_full_db(
            &path,
            &[
                ("m1", "auth uses jose for the session cookie"),
                ("m2", "deploy the redis cache at the edge"),
            ],
        );
        assert!(
            !fts_ddl(&path).contains("detail=none"),
            "the fixture must start at detail=full or this test proves nothing"
        );
        let s = SqliteStore::open(&path).expect("open legacy db");
        let ddl = fts_ddl(&path);
        assert!(ddl.contains("detail=none"), "not migrated: {ddl}");
        // The rebuild must repopulate from the content table, not leave the index
        // empty — an empty index on an external-content table still answers
        // queries, so only a real hit list proves the rows are back.
        let hits = s.keyword_search("a", "jose", 10).expect("search");
        assert_eq!(hits.len(), 1, "the rebuilt index lost its rows");
        assert_eq!(hits[0].0, "m1");
        drop(s);
        rm_db(&path);
    }

    #[test]
    fn reopening_a_migrated_database_should_not_touch_the_fts_index_again() {
        let path = tmp_db("detail-idempotent");
        legacy_detail_full_db(&path, &[("m1", "auth uses jose")]);
        let s = SqliteStore::open(&path).expect("first open migrates");
        assert!(!s.rebuilt_fts_on_open(), "a clean legacy index needs no repair");
        drop(s);
        // Every later open must be a no-op. `rebuilt_fts_on_open` is the flag the
        // rebuild sets, and the index *content* is the independent witness that
        // the `detail=none` table was not dropped and rebuilt a second time.
        //
        // Content, not page count. A page count is the tempting witness and the
        // wrong one: dropping and rebuilding the same corpus can land on the same
        // number of pages, and then "the size did not change" reads as "nothing
        // happened" while the index was in fact rebuilt. `memories_fts_data`
        // holds the actual doclist blocks, so hashing them hashes the index
        // itself, and a rebuild that changed anything at all cannot hide behind an
        // equal total. (This was learned the hard way in Phase E2, where a
        // tokenizer change was measured; the fingerprinting idea outlived the
        // change.)
        let after_migration = fts_content_hash(&path);
        for i in 0..3 {
            let s = SqliteStore::open(&path).expect("reopen");
            assert!(
                !s.rebuilt_fts_on_open(),
                "reopen {i}: a re-opened, already-converted database must not rebuild"
            );
            drop(s);
            assert_eq!(
                fts_content_hash(&path),
                after_migration,
                "reopen {i} changed the index content, so it was rebuilt"
            );
        }
        rm_db(&path);
    }

    /// A content fingerprint of the FTS index, for proving that a later open left
    /// it *alone*.
    ///
    /// `memories_fts_data` is the doclist table — one row per segment, keyed by
    /// `id`, with the encoded term postings in `block`. It is the index's own
    /// bytes, so this hashes what the index *says*, not how much room it takes.
    /// The `id` and the block length are folded in ahead of each block so a
    /// reordering of segments changes the hash and a block cannot be moved across
    /// an `id` boundary without changing it.
    ///
    /// Hex, not `from_utf8_lossy`. The doclist blocks are **binary** — varint
    /// deltas, not text — and lossy UTF-8 decoding maps every invalid byte to the
    /// same replacement character, so two different blocks would routinely hash
    /// the same. Hex is injective, so distinct index content cannot collide.
    fn fts_content_hash(path: &Path) -> [u8; 32] {
        let conn = Connection::open(path).expect("raw open");
        let mut stmt = conn
            .prepare("SELECT id, block FROM memories_fts_data ORDER BY id")
            .expect("prepare");
        let rows: Vec<(i64, Vec<u8>)> = stmt
            .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
            .expect("query")
            .collect::<Result<_, _>>()
            .expect("rows");
        let mut flat: Vec<u8> = Vec::new();
        for (id, block) in rows {
            flat.extend_from_slice(&id.to_be_bytes());
            flat.extend_from_slice(&(block.len() as u64).to_be_bytes());
            flat.extend_from_slice(&block);
        }
        let hex: String = flat.iter().map(|b| format!("{b:02x}")).collect();
        hash_content(&hex)
    }

    #[test]
    fn migrating_a_detail_full_database_should_not_change_a_single_bm25_ranking() {
        // The whole claim of this change is that `detail=none` is invisible to
        // retrieval. The strongest available statement of it: the *same* corpus,
        // searched through the *same* code path, returns bit-identical
        // `(id, rank)` pairs before the migration and after it.
        let rows: Vec<(String, String)> = (0..400)
            .map(|i| {
                (
                    format!("m{i}"),
                    format!(
                        "row {i} mentions auth jose session cookie deploy redis cache \
                         latency index shard replica retention budget {i}"
                    ),
                )
            })
            .collect();
        let queries = [
            "auth jose",
            "session cookie",
            "deploy redis cache",
            "latency index shard",
            "retention budget",
        ];

        let path = tmp_db("detail-parity");
        let borrowed: Vec<(&str, &str)> = rows
            .iter()
            .map(|(i, c)| (i.as_str(), c.as_str()))
            .collect();
        legacy_detail_full_db(&path, &borrowed);

        let mut before = Vec::new();
        {
            // Read the ranking the old index produced, through the real store.
            // Opening it here would migrate it, so the queries go through a bare
            // connection with the same SQL `keyword_search_conn` builds.
            let conn = Connection::open(&path).expect("raw open");
            for q in queries {
                let m = SqliteStore::fts_match_query(q);
                let mut stmt = conn
                    .prepare(
                        "SELECT memories.id, bm25(memories_fts) AS rank
                         FROM memories_fts JOIN memories ON memories.rowid = memories_fts.rowid
                         WHERE memories_fts MATCH ? AND memories.bank_id = ?
                         ORDER BY rank LIMIT ?",
                    )
                    .expect("prepare");
                let rows: Vec<(String, f64)> = stmt
                    .query_map(params![m, "a", 50i64], |r| {
                        Ok((r.get(0)?, r.get(1)?))
                    })
                    .expect("query")
                    .collect::<Result<_, _>>()
                    .expect("rows");
                before.push(rows);
            }
        }
        assert!(
            before.iter().all(|r| !r.is_empty()),
            "the fixture must actually match, or the comparison is vacuous"
        );

        let s = SqliteStore::open(&path).expect("open migrates");
        assert!(fts_ddl(&path).contains("detail=none"), "not migrated");
        for (q, want) in queries.iter().zip(&before) {
            let got = s.keyword_search("a", q, 50).expect("search");
            assert_eq!(
                &got, want,
                "recall moved for {q:?} across the detail=none migration"
            );
        }
        drop(s);
        rm_db(&path);
    }

    #[test]
    fn the_fts_integrity_gate_should_still_pass_under_detail_none() {
        // FTS5 documents restrictions around `detail=none` and external-content
        // tables, and this crate's whole drift-recovery story rests on
        // `integrity-check` reporting a desynced index. So the gate is asserted
        // here directly, on both the clean case and the drifted one: a pass
        // proves `migrate` took the no-rebuild branch, and a failure proves the
        // drift is still *detectable* rather than silently accepted.
        let path = tmp_db("detail-integrity");
        let s = SqliteStore::open(&path).expect("open");
        s.put_bank(&bank("a")).expect("bank");
        s.put(&content_mem("a", "m1", "integrity gate under detail none")).expect("put");
        drop(s);
        assert!(fts_ddl(&path).contains("detail=none"), "fixture is not detail=none");

        {
            let conn = Connection::open(&path).expect("raw open");
            conn.execute_batch(
                "INSERT INTO memories_fts(memories_fts, rank) VALUES('integrity-check', 1);",
            )
            .expect("integrity-check must succeed on a detail=none external-content table");
            // Now desync the index behind the store's back, exactly as a lost
            // trigger write would, and require the same command to object.
            conn.execute_batch("DROP TRIGGER memories_au;").expect("drop trigger");
            conn.execute(
                "UPDATE memories SET content = 'drifted away' WHERE id = 'm1'",
                [],
            )
            .expect("drift");
            let err = conn
                .execute_batch(
                    "INSERT INTO memories_fts(memories_fts, rank) VALUES('integrity-check', 1);",
                )
                .expect_err("a drifted detail=none index must fail integrity-check");
            assert!(
                err.to_string().contains("malformed") || err.to_string().contains("corrupt"),
                "unexpected drift report: {err}"
            );
        }
        // And the self-heal the gate exists to provide still fires on reopen.
        let s = SqliteStore::open(&path).expect("reopen heals");
        assert!(
            s.rebuilt_fts_on_open(),
            "a drifted index must still drive the rebuild branch"
        );
        let hits = s.keyword_search("a", "drifted", 10).expect("search");
        assert_eq!(hits.len(), 1, "the rebuild must repopulate the index");
        drop(s);
        rm_db(&path);
    }

    #[test]
    fn fts_match_query_should_never_be_able_to_build_a_phrase_query() {
        // `detail=none` rejects a *multi-term* phrase query with an error rather
        // than an answer, so the one thing standing between it and a recall that
        // 500s on ordinary user input is that the query builder cannot produce
        // one. It cannot: every surviving token is emitted as its own quoted
        // string joined by `OR`, and quotes in the input are separators. These
        // inputs are the shapes a caller could plausibly send.
        for q in [
            "how does auth work",
            "\"quoted phrase\"",
            "a AND (b OR c)",
            "col:value pref* near",
            "NEAR/2 x ^start {brace}",
            "   ",
            "a",
        ] {
            let m = SqliteStore::fts_match_query(q);
            if m.is_empty() {
                continue;
            }
            for part in m.split(" OR ") {
                assert!(
                    part.starts_with('"')
                        && part.ends_with('"')
                        && part[1..part.len() - 1].chars().all(|c| c.is_alphanumeric()),
                    "{q:?} produced a term that is not a single quoted token: {part:?}"
                );
            }
        }
        // The decisive case: a query whose quotes a caller *meant* as a phrase
        // comes out as two independent terms, which is what keeps `detail=none`
        // from turning a quoted search into a hard error. (Tokens under two
        // characters are dropped by the builder, hence `auth`/`jose` not `a`/`b`.)
        assert_eq!(
            SqliteStore::fts_match_query("\"auth jose\""),
            "\"auth\" OR \"jose\""
        );
        assert_eq!(SqliteStore::fts_match_query(""), "");
        assert_eq!(SqliteStore::fts_match_query("a"), "");
    }

    #[test]
    fn put_should_reject_orphan_bank_when_foreign_keys_are_on() {
        let s = SqliteStore::open_in_memory().expect("open");
        let err = s.put(&mem("ghost", "m1")).expect_err("orphan must be rejected");
        assert!(matches!(err, StoreError::Sqlite(_)), "got {err:?}");
        assert!(s.list("ghost").expect("list").is_empty());
    }

    // A retain re-declares the bank it writes to and carries no metadata, so
    // `put_bank` must insert-if-missing rather than upsert: overwriting would
    // rename the bank and discard the config recorded when it was created.
    #[test]
    fn put_bank_should_not_clobber_a_bank_that_already_exists() {
        let s = SqliteStore::open_in_memory().expect("open");
        s.put_bank(&Bank { id: "a".to_string(), name: "Original Name".to_string() })
            .expect("create");
        s.set_bank_config("a", r#"{"retain_mission":"own the release"}"#).expect("config");
        // A retain re-declares the bank with no metadata; that must not clobber.
        s.put_bank(&Bank { id: "a".to_string(), name: "a".to_string() }).expect("retain");
        let name: String = s
            .conn
            .lock()
            .expect("lock")
            .query_row("SELECT name FROM banks WHERE id = 'a'", [], |r| r.get(0))
            .expect("row");
        assert_eq!(name, "Original Name", "a re-declared bank keeps its name");
        assert_eq!(
            s.get_bank_config("a").expect("config").as_deref(),
            Some(r#"{"retain_mission":"own the release"}"#),
            "a re-declared bank keeps its config"
        );
    }

    #[test]
    fn put_should_relocate_a_row_when_its_id_is_reused_in_another_bank() {
        let s = SqliteStore::open_in_memory().expect("open");
        s.put_bank(&bank("a")).expect("bank a");
        s.put_bank(&bank("b")).expect("bank b");
        s.put(&content_mem("a", "shared", "lives in a")).expect("put a");
        s.put(&content_mem("b", "shared", "moved to b")).expect("put b");

        assert!(s.get("a", "shared").expect("get").is_none(), "row must leave bank a");
        assert!(s.list("a").expect("list a").is_empty());
        let got = s.get("b", "shared").expect("get b").expect("some");
        assert_eq!(got.bank_id, "b");
        assert_eq!(got.content, "moved to b");
    }

    fn tags_of(s: &SqliteStore, memory_id: &str) -> Vec<String> {
        let conn = s.conn.lock().expect("lock");
        let mut stmt = conn
            .prepare("SELECT tag FROM memory_tags WHERE memory_id = ?1 ORDER BY tag")
            .expect("prepare");
        let rows = stmt
            .query_map(params![memory_id], |row| row.get::<_, String>(0))
            .expect("query");
        rows.map(|r| r.expect("row")).collect()
    }

    fn tag_filter(tags: &[&str]) -> Vec<String> {
        tags.iter().map(|t| t.to_string()).collect()
    }

    #[test]
    fn put_tagged_should_roundtrip_and_replace_the_tag_set() {
        let s = SqliteStore::open_in_memory().expect("open");
        s.put_bank(&bank("a")).expect("bank");
        s.put_tagged(&mem("a", "m1"), &tag_filter(&["Auth", "jose"]))
            .expect("put");
        assert_eq!(tags_of(&s, "m1"), vec!["Auth", "jose"]);

        // A second write is the whole truth, so the earlier set is dropped rather
        // than merged — a relocated row must not keep the other bank's tags.
        s.put_tagged(&mem("a", "m1"), &tag_filter(&["only"])).expect("re-put");
        assert_eq!(tags_of(&s, "m1"), vec!["only"]);
    }

    #[test]
    fn tags_should_cascade_when_the_memory_is_deleted() {
        let s = SqliteStore::open_in_memory().expect("open");
        s.put_bank(&bank("a")).expect("bank");
        s.put_tagged(&mem("a", "m1"), &tag_filter(&["t1", "t2"])).expect("put");
        // Deleted through the store's own connection, so this also proves the
        // FK pragma the store sets is what arms the cascade.
        s.conn
            .lock()
            .expect("lock")
            .execute("DELETE FROM memories WHERE id = 'm1'", [])
            .expect("delete");
        assert!(tags_of(&s, "m1").is_empty(), "orphan tags must be cascaded away");
    }

    #[test]
    fn put_tagged_should_reject_an_orphan_bank() {
        let s = SqliteStore::open_in_memory().expect("open");
        assert!(matches!(
            s.put_tagged(&mem("ghost", "m1"), &tag_filter(&["t"])),
            Err(StoreError::Sqlite(_))
        ));
    }

    #[test]
    fn recall_inputs_should_filter_both_streams_by_tag() {
        let s = SqliteStore::open_in_memory().expect("open");
        s.put_bank(&bank("a")).expect("bank");
        s.put_tagged(&content_mem("a", "m1", "auth uses jose middleware"), &tag_filter(&["auth"]))
            .expect("put 1");
        s.put_tagged(&content_mem("a", "m2", "auth uses jose secrets"), &tag_filter(&["ops"]))
            .expect("put 2");
        s.put_tagged(&content_mem("a", "m3", "jose on the frontend"), &tag_filter(&["auth", "ui"]))
            .expect("put 3");

        // No filter: the whole bank.
        let (all, _) = s.recall_inputs("a", "jose", &[], 10).expect("all");
        assert_eq!(all.len(), 3);

        // Any-match, and both streams must agree or fusion would score rows the
        // list stream never offered.
        let (all, hits) = s
            .recall_inputs("a", "jose", &tag_filter(&["auth", "ui"]), 10)
            .expect("filtered");
        assert_eq!(
            all.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            ["m1", "m3"]
        );
        assert_eq!(hits.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(), ["m1", "m3"]);

        // A tag nothing carries empties both streams.
        let (all, hits) = s.recall_inputs("a", "jose", &tag_filter(&["nope"]), 10).expect("none");
        assert!(all.is_empty() && hits.is_empty());
    }

    #[test]
    fn set_bank_config_should_validate_json_and_require_an_existing_bank() {
        let s = SqliteStore::open_in_memory().expect("open");
        s.put_bank(&bank("a")).expect("bank");

        // Valid JSON that is not an object is not a config.
        for bad in ["not json", "[1,2]", "\"str\"", "null", "42"] {
            assert!(
                matches!(s.set_bank_config("a", bad), Err(StoreError::InvalidConfig)),
                "{bad:?} must be rejected"
            );
        }
        // An UPDATE never inserts, so an unknown bank cannot be conjured up.
        assert!(matches!(
            s.set_bank_config("ghost", "{}"),
            Err(StoreError::UnknownBank)
        ));
        assert!(s.get_bank_config("ghost").expect("get").is_none());

        // A fresh bank reads back as an empty object, and a stored config is
        // served byte-for-byte (unknown keys and all).
        assert_eq!(s.get_bank_config("a").expect("get").as_deref(), Some("{}"));
        let raw = r#"{"retain_mission":"ship it","recallMaxTokens":64,"extra":[1]}"#;
        s.set_bank_config("a", raw).expect("set");
        assert_eq!(s.get_bank_config("a").expect("get").as_deref(), Some(raw));
    }

    #[test]
    fn migrate_should_add_config_to_a_legacy_database() {
        let path = tmp_db("legacy");
        {
            // The pre-config schema: banks without a config column, and nothing else.
            let conn = Connection::open(&path).expect("raw open");
            conn.execute_batch(
                "CREATE TABLE banks (
                   id TEXT PRIMARY KEY,
                   name TEXT NOT NULL,
                   background TEXT
                 );
                 INSERT INTO banks (id, name) VALUES ('a', 'a');",
            )
            .expect("legacy schema");
        }
        let s = SqliteStore::open(&path).expect("reopen");
        assert_eq!(
            s.get_bank_config("a").expect("config").as_deref(),
            Some("{}"),
            "ALTER TABLE must backfill the pre-config row"
        );
        // The rest of the schema is created around the legacy table, so the
        // database is usable, not merely non-crashing.
        s.put(&content_mem("a", "m1", "legacy row still indexes jose")).expect("put");
        assert_eq!(s.keyword_search("a", "jose", 10).expect("search").len(), 1);
        s.set_bank_config("a", r#"{"recallMaxTokens":8}"#).expect("set");

        // Reopening runs migrate again and must not re-apply the ALTER.
        let s = SqliteStore::open(&path).expect("reopen again");
        assert_eq!(
            s.get_bank_config("a").expect("config").as_deref(),
            Some(r#"{"recallMaxTokens":8}"#)
        );
        assert_eq!(s.get("a", "m1").expect("get").expect("some").bank_id, "a");
        rm_db(&path);
    }

    /// Column names on `banks` in an open file-backed store.
    fn bank_columns(s: &SqliteStore) -> Vec<String> {
        let conn = s.conn.lock().expect("lock");
        let mut stmt = conn.prepare("PRAGMA table_info(banks)").expect("prepare");
        let rows = stmt
            .query_map([], |r| r.get::<_, String>(1))
            .expect("query");
        rows.map(|r| r.expect("row")).collect()
    }

    // A fresh database must not have the column at all, so nothing can keep
    // writing to it.
    #[test]
    fn a_fresh_database_should_not_have_a_background_column() {
        let s = SqliteStore::open_in_memory().expect("open");
        let cols = bank_columns(&s);
        assert!(
            !cols.contains(&BACKGROUND_COLUMN.to_string()),
            "a fresh schema must not carry {BACKGROUND_COLUMN}: {cols:?}"
        );
        assert_eq!(
            cols,
            ["id", "name", "config", TTL_DAYS_COLUMN],
            "and the columns it does carry are stable"
        );
    }

    // The other half of the guarded migration: a legacy database that recorded
    // nothing in the column loses it, and every row survives the drop.
    #[test]
    fn migrate_should_drop_an_unused_background_column_with_its_rows_intact() {
        let path = tmp_db("drop-background");
        write_legacy_schema(&path);
        {
            let conn = Connection::open(&path).expect("raw open");
            assert!(
                has_background_column(&conn),
                "the fixture must start pre-migration"
            );
        }
        let s = SqliteStore::open(&path).expect("reopen");
        assert!(
            !bank_columns(&s).contains(&BACKGROUND_COLUMN.to_string()),
            "an unused column must be dropped, not kept out of harm's way"
        );
        // Dropping a column must not drop the rows that referenced it.
        let old = s.get("a", "old-1").expect("get").expect("the legacy row survives");
        assert_eq!(old.content, "a legacy row about jose");
        assert_eq!(s.list("a").expect("list").len(), 1);
        assert!(s.get_bank_config("a").expect("config").is_some(), "the rest of the schema is intact");

        // Reopening runs migrate again, and the column is already gone.
        let s = SqliteStore::open(&path).expect("reopen again");
        assert!(!bank_columns(&s).contains(&BACKGROUND_COLUMN.to_string()));
        assert_eq!(s.list("a").expect("list").len(), 1);
        rm_db(&path);
    }

    // DROP COLUMN is irreversible, so a database that actually recorded a
    // background keeps the column and warns instead of losing the value. The
    // stale column is inert — nothing reads it — but the store must still serve
    // reads and writes over the rest of the schema.
    #[test]
    fn migrate_should_keep_a_background_column_that_still_carries_data() {
        let path = tmp_db("keep-background");
        Connection::open(&path)
            .expect("raw open")
            .execute_batch(
                "CREATE TABLE banks (
                   id TEXT PRIMARY KEY,
                   name TEXT NOT NULL,
                   background TEXT
                 );
                 CREATE TABLE memories (
                   id TEXT PRIMARY KEY,
                   bank_id TEXT NOT NULL REFERENCES banks(id) ON DELETE CASCADE,
                   content TEXT NOT NULL,
                   context TEXT
                 );
                 INSERT INTO banks (id, name, background) VALUES ('a', 'a', 'keep me');
                 INSERT INTO memories (id, bank_id, content)
                   VALUES ('old-1', 'a', 'a legacy row about jose');",
            )
            .expect("legacy schema");
        let s = SqliteStore::open(&path).expect("reopen");
        assert!(
            bank_columns(&s).contains(&BACKGROUND_COLUMN.to_string()),
            "a column that holds a value must survive the migration"
        );
        let kept: Option<String> = s
            .conn
            .lock()
            .expect("lock")
            .query_row("SELECT background FROM banks WHERE id = 'a'", [], |r| r.get(0))
            .expect("row");
        assert_eq!(kept.as_deref(), Some("keep me"), "the value itself is untouched");

        // Usable, not merely non-crashing: reads, writes, and config all work
        // over a table the migration chose not to clean.
        assert_eq!(s.list("a").expect("list").len(), 1);
        s.put(&content_mem("a", "m1", "written after the kept column, about jose")).expect("put");
        assert_eq!(
            s.keyword_search("a", "jose", 10).expect("search").len(),
            2,
            "both the legacy and the new row are indexed"
        );
        s.set_bank_config("a", r#"{"recallMaxTokens":8}"#).expect("config");
        // Re-declaring the bank must not disturb the row the migration kept.
        s.put_bank(&bank("a")).expect("re-declare");
        assert_eq!(s.list("a").expect("list").len(), 2);

        let s = SqliteStore::open(&path).expect("reopen again");
        assert!(bank_columns(&s).contains(&BACKGROUND_COLUMN.to_string()));
        assert_eq!(s.list("a").expect("list").len(), 2);
        rm_db(&path);
    }

    /// Read the column list through a raw connection, for the pre-migration
    /// side of the assertion.
    fn has_background_column(conn: &Connection) -> bool {
        let mut stmt = conn.prepare("PRAGMA table_info(banks)").expect("prepare");
        let names = stmt
            .query_map([], |r| r.get::<_, String>(1))
            .expect("query")
            .collect::<Result<Vec<String>, _>>()
            .expect("rows");
        names.iter().any(|name| name == BACKGROUND_COLUMN)
    }

    /// The pre-timestamp, pre-tags, pre-document schema: neither `memories`
    /// column and no tag table at all. The `background` column is here on
    /// purpose: these fixtures are pre-migration databases, and R3's migration
    /// is part of what a pre-migration database has to survive.
    fn write_legacy_schema(path: &Path) {
        Connection::open(path)
            .expect("raw open")
            .execute_batch(
                "CREATE TABLE banks (
                   id TEXT PRIMARY KEY,
                   name TEXT NOT NULL,
                   background TEXT
                 );
                 CREATE TABLE memories (
                   id TEXT PRIMARY KEY,
                   bank_id TEXT NOT NULL REFERENCES banks(id) ON DELETE CASCADE,
                   content TEXT NOT NULL,
                   context TEXT
                 );
                 INSERT INTO banks (id, name) VALUES ('a', 'a');
                 INSERT INTO memories (id, bank_id, content)
                   VALUES ('old-1', 'a', 'a legacy row about jose');",
            )
            .expect("legacy schema");
    }

    #[test]
    fn migrate_should_add_created_at_and_document_id_to_a_legacy_database() {
        let path = tmp_db("legacy-memories");
        write_legacy_schema(&path);
        let s = SqliteStore::open(&path).expect("reopen");

        // The added column's DEFAULT backfills the row that predates it, and it
        // is the epoch sentinel rather than the migration's own clock: the
        // migration knows when it ran, not when the memory was written.
        let old = s.get("a", "old-1").expect("get").expect("some");
        assert_eq!(
            old.created_at.as_deref(),
            Some(UNKNOWN_CREATED_AT),
            "a legacy row reads as unknown age"
        );
        assert_eq!(old.content, "a legacy row about jose", "the migration must not rewrite content");

        // The rest of the schema is created around the legacy table, so the
        // database is usable: tags, documents, and new timestamps all work.
        s.put_tagged(&content_mem("a", "new-1", "fresh row about jose"), &tag_filter(&["t"]))
            .expect("put tagged");
        let fresh = s.get("a", "new-1").expect("get").expect("some");
        assert_ne!(
            fresh.created_at.as_deref(),
            Some(UNKNOWN_CREATED_AT),
            "a row written after the migration must carry a real timestamp"
        );
        assert_eq!(tags_of(&s, "new-1"), vec!["t"]);
        assert_eq!(s.get_bank_config("a").expect("config").as_deref(), Some("{}"));

        // Reopening runs migrate again and must not re-apply the ALTERs.
        let s = SqliteStore::open(&path).expect("reopen again");
        assert_eq!(
            s.get("a", "old-1").expect("get").expect("some").created_at,
            Some(UNKNOWN_CREATED_AT.to_string())
        );
        assert_eq!(s.list("a").expect("list").len(), 2);
        rm_db(&path);
    }

    // The config JSON stays the whole truth; the column is its projection, and
    // because both are written from the same parse the projection cannot outlive
    // the config that states it.
    #[test]
    fn set_bank_config_should_project_ttl_days_onto_its_own_column() {
        let s = SqliteStore::open_in_memory().expect("open");
        s.put_bank(&bank("a")).expect("bank a");
        s.put_bank(&bank("b")).expect("bank b");
        assert_eq!(
            s.bank_ttls().expect("ttls"),
            [("a".to_string(), None), ("b".to_string(), None)],
            "a fresh store has no policy anywhere, so nothing can expire"
        );

        s.set_bank_config("a", r#"{"recallMaxTokens":64,"ttl_days":30}"#)
            .expect("set");
        let ttls = s.bank_ttls().expect("ttls");
        assert_eq!(ttls[0], ("a".to_string(), Some(30)));
        assert_eq!(ttls[1], ("b".to_string(), None), "an unconfigured bank stays unset");
        // The wire format is untouched by the projection.
        assert!(s
            .get_bank_config("a")
            .expect("config")
            .expect("some")
            .contains(r#""ttl_days":30"#));

        // A whole-object replace drops the key, so the policy goes with it.
        s.set_bank_config("a", r#"{"recallMaxTokens":64}"#).expect("replace");
        assert_eq!(s.bank_ttls().expect("ttls")[0], ("a".to_string(), None));

        // A value this build cannot use is a policy of "never", not a refused
        // write: forgetting must not be switchable by a typo.
        for bad in [r#"{"ttl_days":"30 days"}"#, r#"{"ttl_days":-1}"#, r#"{"ttl_days":null}"#] {
            s.set_bank_config("a", bad).expect("set");
            assert_eq!(s.bank_ttls().expect("ttls")[0], ("a".to_string(), None), "{bad}");
        }
        // Zero is a real policy, not an absent one.
        s.set_bank_config("a", r#"{"ttl_days":0}"#).expect("set");
        assert_eq!(s.bank_ttls().expect("ttls")[0], ("a".to_string(), Some(0)));
    }

    // The 0.2.0 store is a pre-`ttl_days` store: the column is added, every row
    // survives it, and the database is usable rather than merely non-crashing.
    #[test]
    fn migrate_should_add_ttl_days_to_a_legacy_database() {
        let path = tmp_db("legacy-ttl");
        write_legacy_schema(&path);
        let s = SqliteStore::open(&path).expect("reopen");
        assert!(
            bank_columns(&s).contains(&TTL_DAYS_COLUMN.to_string()),
            "the column must exist on a pre-ttl database"
        );
        // Nullable with no default, so the pre-policy row reads as "never" rather
        // than as zero — which would have meant forgetting everything on the first
        // sweep of a bank nobody had ever asked to expire.
        assert_eq!(s.bank_ttls().expect("ttls"), [("a".to_string(), None)]);
        assert_eq!(s.list("a").expect("list").len(), 1, "the legacy row survives");
        assert!(s.keyword_search("a", "jose", 10).expect("search").len() == 1);

        // Config writes still work over the added column, and the policy takes.
        s.set_bank_config("a", r#"{"ttl_days":7}"#).expect("config");
        assert_eq!(s.bank_ttls().expect("ttls"), [("a".to_string(), Some(7))]);

        // Reopening runs migrate again and must not re-apply the ALTER.
        let s = SqliteStore::open(&path).expect("reopen again");
        assert_eq!(s.bank_ttls().expect("ttls"), [("a".to_string(), Some(7))]);
        assert_eq!(s.list("a").expect("list").len(), 1);
        rm_db(&path);
    }

    /// A memory with an explicit `created_at`, which is what a retention boundary
    /// is expressed in. The store stamps `created_at` on insert only when the
    /// caller leaves it `None`, so a fixture can pin it.
    fn aged_mem(bank: &str, id: &str, content: &str, created_at: &str) -> Memory {
        Memory {
            created_at: Some(created_at.to_string()),
            ..content_mem(bank, id, content)
        }
    }

    // Expiring is a `DELETE` scoped by age, and it leaves the rest of the store
    // alone: younger rows, another bank, tags, and — the part a hand-rolled
    // implementation gets wrong — the FTS index.
    #[test]
    fn expire_before_should_delete_only_what_is_older_and_keep_the_index_in_step() {
        let s = SqliteStore::open_in_memory().expect("open");
        s.put_bank(&bank("a")).expect("bank a");
        s.put_bank(&bank("b")).expect("bank b");
        s.put_tagged(
            &aged_mem("a", "old", "jose middleware rotated last spring", "2020-01-01T00:00:00.000Z"),
            &tag_filter(&["auth"]),
        )
        .expect("old");
        s.put(&aged_mem("a", "young", "jose middleware rotated today", "2026-09-01T00:00:00.000Z"))
            .expect("young");
        s.put(&aged_mem("b", "other-bank", "jose middleware in another bank", "2020-01-01T00:00:00.000Z"))
            .expect("other bank");
        let cutoff = "2025-01-01T00:00:00.000Z";

        // A dry run counts and changes nothing — not the rows, not the index.
        assert_eq!(s.expire_before("a", cutoff, true).expect("dry run"), 1);
        assert_eq!(s.list("a").expect("list").len(), 2, "a dry run must delete nothing");
        assert_eq!(s.keyword_search("a", "jose", 10).expect("search").len(), 2);

        assert_eq!(s.expire_before("a", cutoff, false).expect("expire"), 1);
        let left = s.list("a").expect("list");
        assert_eq!(left.len(), 1, "only the row older than the cutoff went");
        assert_eq!(left[0].id, "young");
        // Scoped to the bank, so the same content in another bank is untouched.
        assert_eq!(s.list("b").expect("list").len(), 1);

        // The FTS trigger retires the row with it: a deleted memory must be
        // unfindable, or recall keeps citing content that no longer exists.
        let hits = s.keyword_search("a", "jose", 10).expect("search");
        assert_eq!(hits.len(), 1, "the expired row must leave the index: {hits:?}");
        assert_eq!(hits[0].0, "young");
        // And its tags went with it, through the cascade.
        assert!(tags_of(&s, "old").is_empty(), "an expired memory leaves no tags");

        // A second pass is a no-op, so a scheduled-looking caller cannot double-count.
        assert_eq!(s.expire_before("a", cutoff, false).expect("resweep"), 0);
    }

    #[test]
    fn migrate_should_repair_a_divergent_fts_trigger() {
        let path = tmp_db("trigger");
        {
            let s = SqliteStore::open(&path).expect("open");
            s.put_bank(&bank("a")).expect("bank");
        }
        // Plant a divergent trigger: the row still lands, but nothing indexes it.
        // `CREATE TRIGGER IF NOT EXISTS` would keep this forever.
        {
            let conn = Connection::open(&path).expect("raw open");
            conn.execute_batch(
                "DROP TRIGGER memories_ai;
                 CREATE TRIGGER memories_ai AFTER INSERT ON memories BEGIN SELECT 1; END;",
            )
            .expect("plant");
        }
        let s = SqliteStore::open(&path).expect("reopen repairs");
        s.put(&content_mem("a", "m1", "written after the repair jose")).expect("put");
        // The row is inserted after the repair, so only a working insert trigger
        // can put it in the index -- the 'rebuild' backfill cannot.
        let hits = s.keyword_search("a", "jose", 10).expect("search");
        assert_eq!(
            hits.len(),
            1,
            "migrate must replace a divergent insert trigger, not keep it"
        );
        assert_eq!(hits[0].0, "m1");
        rm_db(&path);
    }

    #[test]
    fn a_second_open_should_not_rebuild_the_fts_index() {
        let path = tmp_db("no-rebuild");
        {
            let s = SqliteStore::open(&path).expect("open");
            s.put_bank(&bank("a")).expect("bank");
            s.put(&content_mem("a", "m1", "indexed on the first write jose")).expect("put");
        }
        // Nothing drifted and the triggers are recreated below on every open, so
        // the index is already in step: this open must not pay to reindex the
        // whole bank, and must say so rather than leave it to be inferred from a
        // timing nobody can assert on.
        let s = SqliteStore::open(&path).expect("reopen");
        assert!(!s.rebuilt_fts_on_open(), "an in-step index must not be rebuilt");
        assert_eq!(s.keyword_search("a", "jose", 10).expect("search").len(), 1);

        // Self-healing is not the casualty: a wiped index is still repaired, and
        // only then.
        {
            let conn = Connection::open(&path).expect("raw open");
            conn.execute("DELETE FROM memories_fts", []).expect("wipe fts");
        }
        let s = SqliteStore::open(&path).expect("reopen after drift");
        assert!(s.rebuilt_fts_on_open(), "a drifted index must still be rebuilt");
        assert_eq!(s.keyword_search("a", "jose", 10).expect("search").len(), 1);
        rm_db(&path);
    }

    // A recall that read the whole bank made its cost grow with the bank while
    // fusion could only ever spend [`RECALL_POOL_LIMIT`] of the rows. The window
    // is the regression gate: the bound is a property of the store, so it is
    // asserted here rather than inferred from a latency number.
    #[test]
    fn recall_inputs_should_read_a_bounded_window_not_the_whole_bank() {
        let s = SqliteStore::open_in_memory().expect("open");
        s.put_bank(&bank("a")).expect("bank");
        // Every row matches, so none of them can drop out for being irrelevant —
        // the only thing that can shrink this pool is the bound itself.
        for i in 0..(RECALL_POOL_LIMIT * 3) {
            s.put(&content_mem("a", &format!("m{i}"), &format!("jose middleware note {i}")))
                .expect("put");
        }
        assert_eq!(s.list("a").expect("list").len(), RECALL_POOL_LIMIT * 3);

        let (pool, hits) = s.recall_inputs("a", "jose", &[], 50).expect("inputs");
        assert_eq!(hits.len(), 50, "the fixture must produce BM25 hits");
        // The bound is the window plus the hits unioned into it — a constant,
        // and a fraction of the bank no matter how far the bank has grown.
        assert_eq!(pool.len(), RECALL_POOL_LIMIT + hits.len());
        assert!(pool.len() <= RECALL_POOL_LIMIT + 50, "read {} rows", pool.len());
    }

    // The window is ordered by recency, so a relevant *old* row falls outside
    // it. BM25 is the signal that finds those, and dropping them would make
    // recall silently time-blind, so the hits are unioned in regardless of age.
    #[test]
    fn recall_inputs_should_keep_an_old_bm25_hit_the_window_missed() {
        let s = SqliteStore::open_in_memory().expect("open");
        s.put_bank(&bank("a")).expect("bank");
        let old = content_mem("a", "the-old-one", "an early note about jose middleware");
        s.put(&old).expect("put old");
        for i in 0..(RECALL_POOL_LIMIT * 2) {
            s.put(&content_mem("a", &format!("m{i}"), &format!("unrelated note {i}")))
                .expect("put");
        }
        assert_eq!(s.list("a").expect("list").len(), RECALL_POOL_LIMIT * 2 + 1);

        let (pool, hits) = s.recall_inputs("a", "jose", &[], 50).expect("inputs");
        assert_eq!(hits.len(), 1, "only the old row mentions the query");
        assert_eq!(hits[0].0, "the-old-one");
        let ids: Vec<&str> = pool.iter().map(|m| m.id.as_str()).collect();
        assert!(
            ids.contains(&"the-old-one"),
            "an old BM25 hit must reach the fusion, not be dropped for being old"
        );
        // ...and it is in the pool *only* because the hits were unioned in: the
        // window is the newest RECALL_POOL_LIMIT rows by insertion order, and the
        // old row is the one row that is not in it.
        let window: Vec<&str> = ids.iter().copied().filter(|id| *id != "the-old-one").collect();
        assert_eq!(window.len(), RECALL_POOL_LIMIT);
        assert_eq!(window.first(), Some(&"m200"), "the window is the newest rows");
        assert_eq!(window.last(), Some(&"m399"), "and it ends at the newest row");
        assert_eq!(pool.len(), RECALL_POOL_LIMIT + 1, "window ∪ hits, nothing more");
        assert_eq!(ids[0], "the-old-one", "and the pool stays in insertion order");
    }

    // A selective tag on a large bank is the case a bare recency window gets
    // wrong: the newest 200 rows may carry none of the tag, so the pool would
    // arrive nearly empty and the recall would under-rank exactly what the
    // caller scoped to. The window has to be over the candidate set.
    #[test]
    fn recall_inputs_should_window_over_the_tagged_rows_not_the_whole_bank() {
        let s = SqliteStore::open_in_memory().expect("open");
        s.put_bank(&bank("a")).expect("bank");
        // One old tagged row, then far more recent untagged ones: a window over
        // the bank would not reach the tagged row at all.
        s.put_tagged(
            &content_mem("a", "tagged", "jose middleware in the auth path"),
            &tag_filter(&["auth"]),
        )
        .expect("tagged");
        for i in 0..(RECALL_POOL_LIMIT * 2) {
            s.put(&content_mem("a", &format!("m{i}"), &format!("unrelated note {i}")))
                .expect("put");
        }
        let (pool, hits) = s
            .recall_inputs("a", "jose", &tag_filter(&["auth"]), 50)
            .expect("filtered");
        assert_eq!(
            pool.iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            ["tagged"],
            "the window is over the tagged rows, so the only one is still in it"
        );
        assert_eq!(hits.len(), 1, "and the FTS stream agrees with the pool");
    }

    #[test]
    fn a_null_document_id_should_never_collide() {
        let s = SqliteStore::open_in_memory().expect("open");
        for b in ["a", "b"] {
            s.put_bank(&bank(b)).expect("bank");
        }
        // SQLite treats every NULL as distinct under a unique index, which is
        // what lets untagged memories coexist while documents stay unique.
        s.put(&content_mem("a", "m1", "no document one")).expect("put 1");
        s.put(&content_mem("a", "m2", "no document two")).expect("put 2");
        assert_eq!(s.list("a").expect("list").len(), 2);

        // The index is per bank, so the same document id in another bank is a
        // different document, not a collision.
        s.put_doc(&content_mem("a", "m3", "doc in a"), &[], Some("shared"), UpdateMode::Replace)
            .expect("put a");
        s.put_doc(&content_mem("b", "m4", "doc in b"), &[], Some("shared"), UpdateMode::Replace)
            .expect("put b");
        assert_eq!(s.list("a").expect("list a").len(), 3);
        assert_eq!(s.list("b").expect("list b").len(), 1, "the other bank's row landed");
    }

    #[test]
    fn put_doc_should_replace_the_prior_row_for_a_document() {
        let s = SqliteStore::open_in_memory().expect("open");
        s.put_bank(&bank("a")).expect("bank");
        s.put_doc(&mem("a", "m1"), &tag_filter(&["v1"]), Some("doc"), UpdateMode::Replace)
            .expect("put v1");
        s.put_doc(&mem("a", "m2"), &tag_filter(&["v2"]), Some("doc"), UpdateMode::Replace)
            .expect("put v2");

        let left = s.list("a").expect("list");
        assert_eq!(left.len(), 1, "a replace leaves exactly one current revision");
        assert_eq!(left[0].id, "m2");
        // The superseded row's tags leave with it through the FK cascade, so the
        // new tag set is the whole truth with nothing inherited.
        assert!(tags_of(&s, "m1").is_empty(), "the superseded row takes its tags with it");
        assert_eq!(tags_of(&s, "m2"), vec!["v2"]);
        assert!(s.keyword_search("a", "content", 10).expect("search").len() == 1);
    }

    #[test]
    fn put_doc_append_should_keep_every_row() {
        let s = SqliteStore::open_in_memory().expect("open");
        s.put_bank(&bank("a")).expect("bank");
        s.put_doc(&mem("a", "m1"), &[], Some("doc#1"), UpdateMode::Append).expect("put 1");
        s.put_doc(&mem("a", "m2"), &[], Some("doc#2"), UpdateMode::Append).expect("put 2");
        s.put_doc(&mem("a", "m3"), &[], None, UpdateMode::Append).expect("put 3");

        assert_eq!(
            s.list("a").expect("list").iter().map(|m| m.id.as_str()).collect::<Vec<_>>(),
            ["m1", "m2", "m3"],
            "an append never supersedes a row"
        );
        // Reusing a document id in append mode is refused by the pre-check, not
        // silently deduped: a caller that wants one current revision asked for the
        // wrong mode, and a caller that wanted two rows needs two ids.
        assert!(matches!(
            s.put_doc(&mem("a", "m4"), &[], Some("doc#1"), UpdateMode::Append),
            Err(StoreError::DocumentConflict)
        ));
        let left = s.list("a").expect("list");
        assert_eq!(left.len(), 3, "the refused write changed nothing");
        assert_eq!(left[0].content, "content-m1", "and left the first row byte-unchanged");
    }

    // The index is per bank, so the same document id is a different document in
    // each — an append is never refused for another bank's row.
    #[test]
    fn put_doc_append_should_accept_a_document_id_another_bank_already_uses() {
        let s = SqliteStore::open_in_memory().expect("open");
        for b in ["a", "b"] {
            s.put_bank(&bank(b)).expect("bank");
        }
        s.put_doc(&content_mem("a", "m1", "doc in a"), &[], Some("shared"), UpdateMode::Append)
            .expect("append a");
        s.put_doc(&content_mem("b", "m2", "doc in b"), &[], Some("shared"), UpdateMode::Append)
            .expect("append b");
        assert_eq!(s.list("a").expect("list a").len(), 1);
        assert_eq!(s.list("b").expect("list b").len(), 1, "the other bank's row landed");

        // Replace then append in one bank is the same refusal as append twice:
        // the pre-check asks the index's question, not "did I write it".
        s.put_doc(&content_mem("a", "m3", "replacement"), &[], Some("shared"), UpdateMode::Replace)
            .expect("replace");
        assert!(matches!(
            s.put_doc(&content_mem("a", "m4", "too late"), &[], Some("shared"), UpdateMode::Append),
            Err(StoreError::DocumentConflict)
        ));
    }

    // The pre-check and the unique index are two answers to the same question,
    // so a caller must not be able to tell which one refused the write: a second
    // variant would mean the error contract depended on which code path lost the
    // race, and the pre-check is exactly the path a client always takes.
    #[test]
    fn a_repeated_append_document_id_should_read_the_same_whether_prechecked_or_indexed() {
        let s = SqliteStore::open_in_memory().expect("open");
        for b in ["checked", "forced"] {
            s.put_bank(&bank(b)).expect("bank");
        }
        s.put_doc(&content_mem("checked", "m1", "first"), &[], Some("doc"), UpdateMode::Append)
            .expect("checked: first");
        let prechecked = s
            .put_doc(
                &content_mem("checked", "m2", "second"),
                &[],
                Some("doc"),
                UpdateMode::Append,
            )
            .expect_err("the pre-check refuses the repeat");

        // Force the backstop: write both rows straight through the upsert body, so
        // no pre-check runs and only the unique index can refuse the second.
        // Distinct row ids per bank, because `id` is a global primary key and
        // reusing one would relocate the row out of the other bank.
        {
            let conn = s.conn.lock().expect("lock");
            SqliteStore::put_conn(
                &conn,
                &content_mem("forced", "f1", "first"),
                Some("doc"),
                &hash_content("first"),
            )
                .expect("forced: first");
            let indexed = SqliteStore::put_conn(
                &conn,
                &content_mem("forced", "f2", "second"),
                Some("doc"),
                &hash_content("second"),
            )
            .expect_err("the unique index refuses the repeat");
            assert!(
                matches!(indexed, StoreError::DocumentConflict),
                "the backstop must not leak driver text: got {indexed:?}"
            );
        }
        assert!(
            matches!(prechecked, StoreError::DocumentConflict),
            "both paths must name one variant: got {prechecked:?}"
        );

        // Both refusals are refusals: neither write left a second row behind.
        assert_eq!(s.list("checked").expect("list").len(), 1);
        assert_eq!(s.list("forced").expect("list").len(), 1);
    }

    // A constraint the caller *cannot* act on stays opaque: the foreign key on
    // an orphan bank is not a document conflict, and the backstop must not
    // relabel it as one.
    #[test]
    fn the_constraint_backstop_should_not_relabel_an_orphan_bank() {
        let s = SqliteStore::open_in_memory().expect("open");
        assert!(
            matches!(s.put_doc(&mem("ghost", "m1"), &[], Some("doc"), UpdateMode::Append), Err(StoreError::Sqlite(_))),
            "an orphan bank is a storage fault, not a document conflict"
        );
    }

    // The dedup rule is the store's, so it is asserted at the layer that owns
    // it: a second write of content the bank already holds is skipped and the
    // id of the row that holds it comes back, while a *rewrite of that same
    // row* is not a duplicate of itself and has to go through.
    #[test]
    fn put_doc_should_dedup_identical_content_but_still_rewrite_its_own_row() {
        let s = SqliteStore::open_in_memory().expect("open");
        s.put_bank(&bank("a")).expect("bank");
        s.put_bank(&bank("b")).expect("bank");

        let stored = s
            .put_doc(
                &content_mem("a", "m1", "auth uses jose"),
                &tag_filter(&["v1"]),
                None,
                UpdateMode::Replace,
            )
            .expect("first");
        assert_eq!(stored, "m1");
        let again = s
            .put_doc(
                &content_mem("a", "m2", "auth uses jose"),
                &tag_filter(&["v2"]),
                None,
                UpdateMode::Replace,
            )
            .expect("duplicate");
        assert_eq!(again, "m1", "the row already holding the content is returned");
        assert_eq!(s.list("a").expect("list").len(), 1, "no second copy");
        assert_eq!(tags_of(&s, "m1"), vec!["v1"], "and a deduped write changes nothing at all");

        // Scoped to the bank: the same content elsewhere is a different memory.
        s.put_doc(&content_mem("b", "m3", "auth uses jose"), &[], None, UpdateMode::Replace)
            .expect("other bank");
        assert_eq!(s.list("b").expect("list b").len(), 1);

        // A row already sitting under this id is not a duplicate of itself, so a
        // re-put that only changes the tag set stays an update.
        s.put_doc(&mem("a", "m1"), &tag_filter(&["v3"]), None, UpdateMode::Replace)
            .expect("rewrite");
        assert_eq!(tags_of(&s, "m1"), vec!["v3"]);

        // An append writes a second copy on purpose, so identical content under
        // two document ids is two memories and the digest index is not unique.
        let one = s
            .put_doc(
                &content_mem("a", "m4", "appended twice"),
                &[],
                Some("d1"),
                UpdateMode::Append,
            )
            .expect("append 1");
        let two = s
            .put_doc(
                &content_mem("a", "m5", "appended twice"),
                &[],
                Some("d2"),
                UpdateMode::Append,
            )
            .expect("append 2");
        assert_ne!(one, two, "an append under a fresh document id never dedups");
        assert_eq!(s.list("a").expect("list").len(), 3, "m1 plus the two appends");
    }

    // The digest column is additive, so a database that predates it arrives with
    // rows the dedup cannot see until they are digested. The backfill has to
    // close that or a pre-existing duplicate stays storeable forever.
    #[test]
    fn migrate_should_digest_rows_written_before_the_column_existed() {
        let path = tmp_db("hash-backfill");
        write_legacy_schema(&path);
        let s = SqliteStore::open(&path).expect("reopen");
        let digested: Option<Vec<u8>> = s
            .conn
            .lock()
            .expect("lock")
            .query_row("SELECT content_hash FROM memories WHERE id = 'old-1'", [], |r| r.get(0))
            .expect("row");
        assert_eq!(
            digested.as_deref(),
            Some(hash_content("a legacy row about jose").as_slice()),
            "a pre-digest row must be digested on open, or it can never be a duplicate"
        );
        // And the backfill is one-shot: reopening digests nothing and reopens clean.
        let s = SqliteStore::open(&path).expect("reopen again");
        assert!(s.keyword_search("a", "jose", 10).expect("search").len() == 1);
        rm_db(&path);
    }

    #[test]
    fn created_at_should_be_stamped_on_insert_and_survive_a_rewrite() {
        let s = SqliteStore::open_in_memory().expect("open");
        s.put_bank(&bank("a")).expect("bank");
        s.put(&content_mem("a", "m1", "first")).expect("put");
        let first = s.get("a", "m1").expect("get").expect("some").created_at;
        let first = first.as_deref().expect("the read path always fills created_at");
        assert_ne!(first, UNKNOWN_CREATED_AT, "a new row must not read as unknown age");
        assert!(first.ends_with('Z') && first.starts_with("20"), "expected RFC 3339 UTC, got {first}");

        // created_at records when the row came into being, so rewriting a row
        // must not restate its age — only the payload moves.
        s.put(&content_mem("a", "m1", "second")).expect("rewrite");
        let after = s.get("a", "m1").expect("get").expect("some");
        assert_eq!(after.created_at.as_deref(), Some(first), "a rewrite must not restate the creation time");
        assert_eq!(after.content, "second");
    }

    #[test]
    fn delete_should_be_bank_scoped_and_cascade_tags() {
        let s = SqliteStore::open_in_memory().expect("open");
        s.put_bank(&bank("a")).expect("bank a");
        s.put_bank(&bank("b")).expect("bank b");
        s.put_tagged(&mem("a", "m1"), &tag_filter(&["t1", "t2"])).expect("put");

        assert!(!s.delete("b", "m1").expect("cross-bank delete"), "another bank's id is unreachable");
        assert!(s.get("a", "m1").expect("get").is_some(), "the refused delete changed nothing");

        assert!(s.delete("a", "m1").expect("delete"), "a bank-scoped delete reports true");
        assert!(!s.delete("a", "m1").expect("again"), "a second delete finds nothing");
        assert!(!s.delete("a", "ghost").expect("ghost"), "an unknown id is not a delete");
        assert!(tags_of(&s, "m1").is_empty(), "tags must cascade with the row");
    }

    #[test]
    fn list_page_should_window_by_limit_and_offset() {
        let s = SqliteStore::open_in_memory().expect("open");
        s.put_bank(&bank("a")).expect("bank");
        for i in 0..5 {
            s.put(&mem("a", &format!("m{i}"))).expect("put");
        }
        let page = |limit: usize, offset: usize| -> Vec<String> {
            s.list_page("a", limit, offset)
                .expect("page")
                .iter()
                .map(|m| m.id.clone())
                .collect()
        };
        assert_eq!(page(2, 0), ["m0", "m1"]);
        assert_eq!(page(2, 2), ["m2", "m3"]);
        assert_eq!(page(2, 4), ["m4"]);
        assert!(page(2, 5).is_empty(), "an offset past the end is empty, not an error");
        assert!(page(0, 0).is_empty(), "a zero limit is empty");
    }

    #[test]
    fn bank_stats_should_report_counts_tags_and_age_bounds() {        let s = SqliteStore::open_in_memory().expect("open");
        s.put_bank(&bank("a")).expect("bank a");
        s.put_bank(&bank("b")).expect("bank b");
        assert_eq!(
            s.bank_stats("a").expect("empty"),
            BankStats::default(),
            "an empty bank is zeroes and no bounds"
        );

        s.put_tagged(&content_mem("a", "m1", "jose auth"), &tag_filter(&["auth", "jose"]))
            .expect("put 1");
        s.put_tagged(&content_mem("a", "m2", "jose ops"), &tag_filter(&["ops"])).expect("put 2");
        s.put(&content_mem("b", "m3", "other bank jose")).expect("put b");

        let st = s.bank_stats("a").expect("stats");
        assert_eq!(
            (st.memories, st.tags),
            (2, 3),
            "tags are counted distinct, inside this bank only"
        );
        let (oldest, newest) = (st.oldest.expect("oldest"), st.newest.expect("newest"));
        assert!(oldest <= newest, "oldest {oldest} must not follow newest {newest}");
        assert_eq!(
            newest,
            s.get("a", "m2").expect("get").expect("some").created_at.expect("stamped"),
            "the newest bound is the newest row's stamp"
        );
        assert_eq!(s.bank_stats("b").expect("b").memories, 1, "the other bank is counted on its own");
    }

/// A backend with nothing: only the four methods `Store` requires, so every
/// other method is a default. It exists to pin what those defaults do.
struct NoBackend;

impl Store for NoBackend {
    fn put_bank(&self, _bank: &Bank) -> Result<(), StoreError> {
        Err(StoreError::Unsupported("put_bank"))
    }
    fn put(&self, _m: &Memory) -> Result<(), StoreError> {
        Err(StoreError::Unsupported("put"))
    }
    fn get(&self, _bank_id: &str, _id: &str) -> Result<Option<Memory>, StoreError> {
        Err(StoreError::Unsupported("get"))
    }
    fn list(&self, _bank_id: &str) -> Result<Vec<Memory>, StoreError> {
        Err(StoreError::Unsupported("list"))
    }
}

// Every defaulted method must *refuse*, not answer.
//
// Five of them always did. The other three used to answer anyway, and each
// answer was quietly wrong rather than obviously absent: `put_tagged` dropped
// the tags, `put_doc` dropped the `document_id` (so a replace became a second
// row), and `recall_inputs` dropped the tag filter (so a scoped recall came
// back unscoped). A future backend that forgot to override any of them got
// plausible data instead of a compile-shaped nudge, which is the failure a
// default exists to prevent.
#[test]
fn every_defaulted_store_method_should_refuse_rather_than_answer() {
    let s = NoBackend;
    let m = mem("a", "m1");
    let refusals: Vec<(&str, Result<(), StoreError>)> = vec![
        ("delete", s.delete("a", "m1").map(|_| ())),
        ("bank_stats", s.bank_stats("a").map(|_| ())),
        ("set_bank_config", s.set_bank_config("a", "{}")),
        ("bank_ttls", s.bank_ttls().map(|_| ())),
        ("expire_before", s.expire_before("a", "2026-01-01T00:00:00.000Z", false).map(|_| ())),
        ("put_tagged", s.put_tagged(&m, &tag_filter(&["t"]))),
        ("put_doc", s.put_doc(&m, &[], Some("doc"), UpdateMode::Replace).map(|_| ())),
        ("recall_inputs", s.recall_inputs("a", "q", &[], 10).map(|_| ())),
    ];
    for (op, got) in refusals {
        assert!(
            matches!(got, Err(StoreError::Unsupported(_))),
            "{op} must refuse by default, got {got:?}"
        );
    }
}

// A poisoned lock must produce an error, not a second panic.
//
// This is the shape a real panic takes: unwind while the guard is held, and
// `Mutex` records it forever after. Every later call used to `expect` its way
// straight into a second panic — inside a request handler that is not a crash
// the client sees, it is a *missing response*, while `/health` kept answering
// 200 and `doctor` kept calling the server up. The fix is only real if the
// next call returns the variant.
#[test]
fn a_poisoned_store_lock_should_refuse_loudly_instead_of_panicking() {
    let s = SqliteStore::open_in_memory().expect("open");
    s.put_bank(&bank("a")).expect("bank");
    s.put(&content_mem("a", "m1", "jose middleware")).expect("put");

    // Poison it: the panic unwinds *out of* the guard, which is what a panic
    // inside any store call would do.
    let unwound = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _guard = s.conn.lock().expect("lock");
        panic!("deliberate: a panic while the store lock is held");
    }));
    assert!(unwound.is_err(), "the fixture must actually poison the lock");

    // Every operation that takes the lock now refuses with the one variant, and
    // none of them panics: an `expect` in a handler is a request with no reply.
    assert!(matches!(s.list("a"), Err(StoreError::LockPoisoned)));
    assert!(matches!(s.get("a", "m1"), Err(StoreError::LockPoisoned)));
    assert!(matches!(s.bank_stats("a"), Err(StoreError::LockPoisoned)));
    assert!(matches!(
        s.put(&content_mem("a", "m2", "another jose note")),
        Err(StoreError::LockPoisoned)
    ));
    assert!(matches!(
        s.recall_inputs("a", "jose", &[], 10),
        Err(StoreError::LockPoisoned)
    ));
    assert!(matches!(
        s.expire_before("a", "2026-01-01T00:00:00.000Z", true),
        Err(StoreError::LockPoisoned)
    ));
    assert!(matches!(s.get_bank_config("a"), Err(StoreError::LockPoisoned)));

    // Loud, but not leaky: the message says what happened and carries no path,
    // no SQL, and no content the caller wrote.
    let msg = StoreError::LockPoisoned.to_string();
    assert_eq!(msg, "store lock poisoned", "got {msg:?}");
}

// The wire mode is parsed once, and only the two documented values survive it.
//
// The reason it is an enum: `replace` deletes the document's prior revision, so
// a value that fell through to it destroyed a revision over a typo. A typo is
// now a refusal, and only an *absent* field is the default.
#[test]
fn update_mode_should_parse_only_the_two_documented_values() {
    assert_eq!(UpdateMode::parse(None), Some(UpdateMode::Replace), "absent is the default");
    assert_eq!(UpdateMode::parse(Some(REPLACE_MODE)), Some(UpdateMode::Replace));
    assert_eq!(UpdateMode::parse(Some(APPEND_MODE)), Some(UpdateMode::Append));
    assert_eq!(UpdateMode::default(), UpdateMode::Replace, "and the enum's own default agrees");

    for typo in ["append ", "APPEND", "Replace", " upsert", "", "append\n", "replace "] {
        assert_eq!(UpdateMode::parse(Some(typo)), None, "{typo:?} must be refused");
    }

    // The spellings round-trip, so the constants and the parser cannot drift.
    assert_eq!(UpdateMode::parse(Some(UpdateMode::Append.as_str())), Some(UpdateMode::Append));
    assert_eq!(UpdateMode::parse(Some(UpdateMode::Replace.as_str())), Some(UpdateMode::Replace));
}

// The epoch sentinel is "unknown age", so it goes the moment the bank asks to
// forget anything, and a stamp that is not a timestamp is never mistaken for an
// old one. Both are byte compares on `created_at`, and the sweep is the only
// thing that acts on it — so the behaviour is pinned here rather than left to a
// doc comment.
#[test]
fn expire_before_should_sweep_the_epoch_sentinel_and_keep_an_unparseable_stamp() {
    let s = SqliteStore::open_in_memory().expect("open");
    s.put_bank(&bank("a")).expect("bank");
    let cutoff = "2026-01-01T00:00:00.000Z";

    s.put(&aged_mem("a", "sentinel", "jose predates timestamping", UNKNOWN_CREATED_AT))
        .expect("put sentinel");
    s.put(&aged_mem("a", "garbage", "jose with a stamp that is not a date", "not-a-date"))
        .expect("put garbage");
    s.put(&aged_mem("a", "real", "jose from last year", "2020-01-01T00:00:00.000Z"))
        .expect("put real");

    assert_eq!(
        s.expire_before("a", cutoff, false).expect("expire"),
        2,
        "the sentinel and the genuinely old row, nothing else"
    );
    let left: Vec<String> = s.list("a").expect("list").into_iter().map(|m| m.id).collect();
    assert_eq!(
        left,
        ["garbage"],
        "an unparseable stamp sorts after every real one, so it is kept — a sweep that \
         forgot the wrong row would be the expensive direction"
    );
    // …and the sentinel is gone, because "unknown age" is older than any cutoff.
}
}
