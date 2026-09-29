//! Vendored, offline, dense-vector retrieval — Phase D of `docs/EXCEED_PLAN.md`.
//!
//! **The whole point of this module is that it needs nothing at runtime.** The
//! model, the tokenizer and the ONNX runtime that executes them are all either
//! compiled into the executable or statically linked into it. There is no
//! download, no cache directory, no `HF_HOME`, no `~/.fastembed_cache`, and no
//! network call on any code path. A binary built with `--features embed` embeds
//! a text on a machine that has never been online.
//!
//! The weight that turns the arm on, [`crate::recall::FusionWeights::vector`],
//! ships at `0.0`.
//! Enabling this cargo feature builds the machinery; it does not fire it.
//!
//! # Provenance and licence — read this before vendoring anything else
//!
//! | | |
//! |---|---|
//! | Model | `Xenova/all-MiniLM-L6-v2` (ONNX export of `sentence-transformers/all-MiniLM-L6-v2`) |
//! | Licence | **Apache-2.0** (`license: apache-2.0` on the Hugging Face model card, and the upstream `sentence-transformers` licence) |
//! | Weights | `onnx/model_int8.onnx` — **int8 dynamically-quantised**, 22,972,370 B |
//! | Weights SHA-256 | `afdb6f1a0e45b715d0bb9b11772f032c399babd23bfc31fed1c170afc848bdb1` |
//! | Tokenizer | `tokenizer.json` (fast-tokenizers), 711,661 B |
//! | Tokenizer SHA-256 | `da0e79933b9ed51798a3ae27893d3c5fa4a201126cef75586296df9b4d2c62a0` |
//! | Config SHA-256 | `7135149f7cffa1a573466c6e4d8423ed73b62fd2332c575bf738a0d033f70df7` (`config.json`) |
//! | Special tokens SHA-256 | `b6d346be366a7d1d48332dbc9fdf3bf8960b5d879522b7799ddba59e76237ee3` (`special_tokens_map.json`) |
//! | Tokenizer config SHA-256 | `9261e7d79b44c8195c1cada2b453e55b00aeb81e907a6664974b4d7776172ab3` (`tokenizer_config.json`) |
//!
//! Apache-2.0 is compatible with this crate's own `MIT OR Apache-2.0`, and it
//! carries an attribution requirement, which is why the table is here and not in
//! a commit message: a vendored Apache-2.0 artefact has to carry its provenance
//! in the source, or the licence obligation is satisfied only by whoever happens
//! to remember to read `git log`. Apache-2.0 §4 also requires retaining notices,
//! and the upstream `LICENSE` travels in the Hugging Face repository of record
//! linked in the table above.
//!
//! `the_vendored_model_is_the_file_the_provenance_records` re-derives the
//! SHA-256 of the bytes this module compiled in, so the table cannot go stale
//! without a test failing.
//!
//! # The int8 build, and the fp32 one that must not be used
//!
//! `onnx/model.onnx` is 90.4 MB and `onnx/model_int8.onnx` is 23.0 MB. Only the
//! int8 file is vendored. The user accepted a larger binary for a self-contained
//! one; 90 MB of fp32 weights to gain precision nobody has measured on this
//! retrieval task is not that trade, it is a different one.
//!
//! # `tokenizer.json`, not `vocab.txt`
//!
//! The brief asked for `vocab.txt` (231,508 B). What the pinned runtime actually
//! loads is `tokenizers::Tokenizer::from_bytes`, which is the fast-tokenizers
//! serialisation — `fastembed`'s own error string for this field is literally
//! `"Could not read tokenizer.json"` (`src/common.rs:127`). `vocab.txt` alone
//! would not build.
//!
//! So `tokenizer.json` is what is vendored. It is a strict superset, not a
//! substitute: its `model.vocab` holds **byte-identical token→id pairs for all
//! 30,522 entries** of `vocab.txt` (verified: 30,522 vs 30,522, zero differing
//! entries), plus the normalizer and pre-tokenizer configuration that
//! `vocab.txt` has no room for. Keeping both would add 231 KB of
//! never-read bytes to a repository whose README quotes its binary size in every
//! headline, so `vocab.txt` is not vendored. The vocabulary the brief asked for
//! is present, in the form the runtime reads.
//!
//! # Pooling and normalisation, and who does them
//!
//! MiniLM is a BERT model: it emits one 384-d row per *token*, and the sentence
//! vector is the attention-masked **mean** of those rows, then **L2-normalised**.
//! Both steps are done by `fastembed` (`pooling::Pooling::Mean`, then
//! `common::normalize` in `text_embedding/output.rs:46`) rather than here,
//! because getting the attention mask wrong is the classic way to produce a
//! vector that is subtly wrong and looks fine. The two settings are set
//! explicitly below rather than inherited from a default, because
//! `Pooling::default()` is `Cls` — the *wrong* one for this model — for backward
//! compatibility with models that want it, and the failure mode is a silently
//! worse ranking rather than an error.
//!
//! Because the stored vector is already unit length, cosine between two of them
//! is arithmetically a dot product. The scan still calls
//! [`crate::embed::cosine`] rather than a dot product: it is three times the
//! arithmetic over a 200-row pool, and it is the one kernel in this crate with
//! unit tests covering the length-mismatch and degenerate cases, so the code that
//! scores a recall is the code that was tested rather than a second
//! implementation of it.
//!
//! # Storage
//!
//! Vectors live in the `memory_vectors` side table, written once on the retain
//! path by [`crate::vector::retain`]. A recall therefore embeds **one** text —
//! the query — and never the bank. See [`crate::store::Store::put_vector`] for
//! why a side table and not a column on `memories`.
//!
//! # No ANN index
//!
//! The scan is exact: [`crate::embed::rank_by_cosine`] over the recall's own
//! candidate pool. agentmemory ships exactly this — a plain `Map`, no ANN — and
//! publishes 95.2% R@5 with it (`docs/EXCEED_PLAN.md` §0.1). At
//! [`store::RECALL_POOL_LIMIT`] = 200 rows of 384 `f32`, the arithmetic is ~77k
//! multiply-adds on data already in the page cache. HNSW would be a large
//! dependency, a second on-disk structure, and a recall-quality parameter that
//! would have to be selected like any other, for no gain measurable at this
//! cardinality. Revisit at 10<sup>5</sup> rows per bank.

use std::collections::HashMap;

use fastembed::{
    InitOptionsUserDefined, Pooling, QuantizationMode, TextEmbedding, TokenizerFiles,
    UserDefinedEmbeddingModel,
};
use crate::api::{normalize_tags, ApiError, ScoredMemory, FTS_LIMIT, MAX_RESULTS, OVERLAP_LIMIT};
use crate::capture::redact_pii;
use crate::embed::rank_by_cosine;
use crate::memory::Memory;
use crate::recall::{
    rank_candidates_scoped, recency_rank, rrf_fuse_with_magnitudes, trim_to_budget, FusionWeights,
    RankedHit,
};
use crate::store::{Store, StoreError};

/// Dimensionality of the vendored model, and therefore of every vector this
/// module produces or stores.
///
/// A constant rather than a read of the ONNX graph's `hidden_size` because it
/// has to be usable in a `const` assertion, and because a mismatch between the
/// graph and this number is a *build* error worth having rather than a runtime
/// shape surprise. `the_embedder_returns_384_dimensions` reads the real graph
/// output and pins the two together.
pub const EMBED_DIM: usize = 384;

/// The int8 ONNX weights, compiled into the executable.
const MODEL_BYTES: &[u8] = include_bytes!("../models/model_int8.onnx");
/// The WordPiece vocabulary and its normalizer, compiled in.
const TOKENIZER_BYTES: &[u8] = include_bytes!("../models/tokenizer.json");
/// The BERT architecture config (hidden size, layer count, vocab size).
const CONFIG_BYTES: &[u8] = include_bytes!("../models/config.json");
/// `[CLS]` / `[SEP]` / `[PAD]` / `[UNK]` / `[MASK]`.
const SPECIAL_TOKENS_BYTES: &[u8] = include_bytes!("../models/special_tokens_map.json");
/// Lowercasing, `model_max_length`, and the pad token the loader reads.
const TOKENIZER_CONFIG_BYTES: &[u8] = include_bytes!("../models/tokenizer_config.json");

/// The SHA-256 of [`MODEL_BYTES`], as recorded in the module's provenance table.
///
/// A `const` so the test that verifies the vendored file is a compile-time
/// comparison of a literal against the bytes in the binary — the digest is
/// checked in, and a blob swapped in without updating it fails the test run
/// rather than passing quietly. `cfg(test)` because nothing outside the test
/// reads it: this crate has no startup self-check, and adding one to a CLI that
/// runs on every hook invocation would be a cost the default build does not need
/// (and, for the `embed` build, would have to hash 23 MB to do).
#[cfg(test)]
const MODEL_SHA256: &str = "afdb6f1a0e45b715d0bb9b11772f032c399babd23bfc31fed1c170afc848bdb1";

/// Sequence length the tokenizer truncates to.
///
/// 512 is the model's `max_position_embeddings` and the value in
/// `tokenizer_config.json`; `fastembed` takes the *minimum* of this and
/// `model_max_length`, so passing a larger value cannot overflow the position
/// embeddings. A memory longer than 512 tokens is truncated by the tokenizer,
/// which is MiniLM's documented behaviour and not a limit this crate imposes.
const MAX_SEQUENCE_LENGTH: usize = 512;

/// Something went wrong building the session or running inference.
///
/// A separate type from [`StoreError`] and [`ApiError`] because the cause is
/// neither: the model is in the binary, so this can only be a corrupt build
/// artifact or an ONNX Runtime resource failure. Both are operator problems, and
/// neither should be reported to a client as a storage fault.
#[derive(Debug, thiserror::Error)]
pub enum EmbedError {
    /// ONNX Runtime or the tokenizer rejected the vendored files.
    #[error("embedding model unavailable: {0}")]
    Model(String),
    /// The session produced a row that is not [`EMBED_DIM`] wide.
    #[error("embedding dimension {got}, expected {EMBED_DIM}")]
    Dimension {
        /// The width the session actually returned.
        got: usize,
    },
    /// The session produced a row the ranking could not use: an empty vector, a
    /// non-finite component, or a zero norm.
    ///
    /// Its own arm rather than a generic "bad model" because the fix is
    /// different: this is a model that loaded and misbehaved, not one that would
    /// not load. A non-finite row is refused rather than passed on, because a
    /// `NaN` cosine silently reorders the whole fused list onto its id
    /// tiebreak.
    #[error("embedding is not usable: {0}")]
    Unusable(String),
}

/// The vendored embedder: 384-d MiniLM, mean-pooled and L2-normalised.
///
/// Construction loads the 23 MB of weights and builds an ONNX session, so it is
/// worth holding for the life of a process rather than per call. Inference
/// needs `&mut self` (ONNX Runtime sessions are not concurrently callable), so a
/// server that embeds on the request path will want a lock around one of these;
/// nothing in this crate does, because the arm ships at weight `0.0` and no
/// request path reaches it.
pub struct Embedder {
    model: TextEmbedding,
}

impl Embedder {
    /// Build the session from the compiled-in bytes.
    ///
    /// Takes a few seconds on first use on a cold box and is the only place in
    /// the crate that touches ONNX Runtime. It cannot fail on a machine that has
    /// never been online: there is no URL, no cache path and no environment
    /// variable consulted on the way.
    pub fn new() -> Result<Self, EmbedError> {
        let model = UserDefinedEmbeddingModel::new(
            MODEL_BYTES.to_vec(),
            TokenizerFiles {
                tokenizer_file: TOKENIZER_BYTES.to_vec(),
                config_file: CONFIG_BYTES.to_vec(),
                special_tokens_map_file: SPECIAL_TOKENS_BYTES.to_vec(),
                tokenizer_config_file: TOKENIZER_CONFIG_BYTES.to_vec(),
            },
        )
        // `Static` rather than `Dynamic`: the weights are already int8 on disk, so
        // there is no fp32 copy in the graph to quantise at load time and no
        // reason to pay for it. Spelled out because the default is `None` today
        // and a change there would be a silent behaviour change, not a build error.
        .with_quantization(QuantizationMode::Static)
        // Explicit, and not the crate default: `Pooling::default()` is `Cls`,
        // which is the wrong pooling for a sentence-transformer and fails
        // silently as a worse ranking.
        .with_pooling(Pooling::Mean);
        let model = TextEmbedding::try_new_from_user_defined(
            model,
            InitOptionsUserDefined::new().with_max_length(MAX_SEQUENCE_LENGTH),
        )
        .map_err(|e| EmbedError::Model(e.to_string()))?;
        Ok(Self { model })
    }

    /// Embed one text into an [`EMBED_DIM`]-wide, L2-normalised vector.
    ///
    /// The returned row has `‖v‖ == 1` to within `f32` rounding, so a caller
    /// comparing two of them may use cosine or a dot product interchangeably.
    /// An empty or whitespace-only text is a caller error rather than a silent
    /// zero row: it has no tokens to mean-pool, and a zero vector would rank
    /// against everything at cosine `0.0` while looking like a real result.
    pub fn embed(&mut self, text: &str) -> Result<Vec<f32>, EmbedError> {
        if text.trim().is_empty() {
            return Err(EmbedError::Unusable("empty text".into()));
        }
        let mut rows = self
            .model
            .embed([text], Some(1))
            .map_err(|e| EmbedError::Model(e.to_string()))?;
        let row = rows
            .pop()
            .ok_or_else(|| EmbedError::Unusable("model returned no row".into()))?;
        if row.len() != EMBED_DIM {
            return Err(EmbedError::Dimension { got: row.len() });
        }
        if !row.iter().all(|x| x.is_finite()) || row.iter().map(|x| x * x).sum::<f32>() <= 0.0 {
            return Err(EmbedError::Unusable("non-finite or zero vector".into()));
        }
        Ok(row)
    }
}

/// The dense stream for one query: candidate ids ranked by cosine, best first.
///
/// `vectors` is keyed by memory id and is *expected* to cover `ids`; an id with
/// no vector is not ranked rather than ranked at `0.0`, which is
/// [`rank_by_cosine`]'s rule — `ids` and `vectors` are read as parallel lists,
/// so a vectorless id has no row to rank at all — and the same one the overlap
/// stream uses for a document that matched no query token. Ranking a vectorless
/// memory at zero would put it in the fused list as unranked filler, which is
/// exactly the failure `a_zero_weight_should_remove_its_stream` exists to
/// prevent.
///
/// **The complement of that rule, which the kernel also states: a stored vector
/// is ranked however low its cosine.** A negative cosine is a measurement, not
/// a verdict, so it sorts to the bottom of the stream rather than being deleted
/// from it. The distinction is reach, not ordering: deleting a candidate makes
/// the dense arm's pool smaller than the store's, which is a coverage claim
/// about what the arm can find at all.
pub fn vector_stream(
    ids: &[String],
    query: &[f32],
    vectors: &HashMap<String, Vec<f32>>,
) -> Vec<RankedHit> {
    // `filter_map`, not a collect into `Option`: an id the store holds no vector
    // for is *unranked*, not a reason to discard the rows that do have one. A
    // half-embedded bank is the normal state of a bank written before the arm
    // existed, and dropping the whole stream for it would make the arm blind to
    // exactly the rows it can already see.
    let mut kept_ids: Vec<String> = Vec::with_capacity(ids.len());
    let mut kept_vectors: Vec<Vec<f32>> = Vec::with_capacity(ids.len());
    for id in ids {
        if let Some(v) = vectors.get(id) {
            kept_ids.push(id.clone());
            kept_vectors.push(v.clone());
        }
    }
    if kept_ids.is_empty() {
        return Vec::new();
    }
    rank_by_cosine(&kept_ids, query, &kept_vectors)
        .into_iter()
        .enumerate()
        .map(|(i, (id, _))| RankedHit { id, rank: i + 1 })
        .collect()
}

/// Recall for `query` with the dense stream fused in, at `weights.vector`.
///
/// The whole of [`crate::api::MemoryService::recall`] — candidate pool, BM25
/// stream, overlap stream, optional recency, RRF, budget trim, result cap — plus
/// the dense stream, and it reads the *same* limits ([`FTS_LIMIT`],
/// [`OVERLAP_LIMIT`], [`MAX_RESULTS`]) rather than its own copies, so a
/// vector-fused recall is the lexical recall with one more voter and not a
/// second implementation that can drift from it.
///
/// It is a free function over a [`Store`] rather than a method on the service
/// for that reason as much as any other: the service's own `recall_with` is
/// private and its behaviour is not this crate's to change, and the arm is not
/// wired to any request path (its weight is `0.0`).
///
/// # What is different from a lexical recall, precisely
///
/// One extra element in `streams`, at index 3, and one extra pass over the pool
/// to build it. At `weights.vector == 0.0` that is not merely zero-weighted —
/// the query is **not embedded at all**, so a default-weight call costs one
/// `HashMap` lookup and no inference. The decision to build the stream and the
/// weight it is fused under are the same value, which is the discipline the
/// recency stream's `recency_weight_for` established and the reason a caller
/// cannot accidentally pay for a model it has not turned on.
///
/// # The empty placeholder at slot 2
///
/// `FusionWeights::of` is positional. Slot 2 is the recency stream, so a
/// caller that appended the vector stream to `[bm25, overlap]` would get it
/// fused under *recency's* weight — a live-looking knob silently reading a
/// different field. Slot 2 is therefore always occupied, by the recency stream
/// when [`FusionWeights::recency`] resolves non-zero and by an **empty** stream
/// otherwise. An empty `Vec<RankedHit>` is not a vote: the fusion's loop body
/// over it cannot touch `scores` or `found_in`, so it is a position and nothing
/// else, and at `0.0` for both the placeholder and the vector the result is
/// bit-identical to the two-stream kernel
/// (`a_zero_vector_weight_should_leave_the_fusion_bit_identical`).
///
/// # Bank isolation
///
/// `store.recall_inputs` and `store.bank_vectors` are both bank-scoped in SQL,
/// and `bank_vectors` has no all-banks form, so no row from another bank can
/// enter the pool, the vector map, or the fused list. That is the same
/// guarantee every other query in this crate makes, and it is structural rather
/// than a filter applied afterwards.
pub fn recall(
    store: &impl Store,
    bank_id: &str,
    query: &str,
    budget_tokens: usize,
    tags: &[String],
    weights: &FusionWeights,
    embedder: &mut Embedder,
) -> Result<Vec<ScoredMemory>, StoreError> {
    if bank_id.trim().is_empty() {
        return Err(StoreError::InvalidBank);
    }
    let normalized = normalize_tags(tags);
    if !tags.is_empty() && normalized.is_empty() {
        // Fail closed, for the reason `api.rs` gives: a filter that normalises to
        // nothing names no memory, and answering it with the whole bank would
        // return unrelated rows indistinguishable from real results.
        return Ok(Vec::new());
    }
    let (all, keyword_hits) = store.recall_inputs(bank_id, query, &normalized, FTS_LIMIT)?;
    let fts_stream: Vec<RankedHit> = keyword_hits
        .iter()
        .enumerate()
        .map(|(i, (id, _))| RankedHit { id: id.clone(), rank: i + 1 })
        .collect();
    // Built only when the term can be added, so a default-weight call allocates
    // nothing for it. An exact `+ 0.0` is bit-identity anyway, but not building
    // the lookup is both cheaper and provable.
    let magnitudes: HashMap<String, f64> = if weights.bm25_magnitude == 0.0 {
        HashMap::new()
    } else {
        keyword_hits.iter().map(|(id, rank)| (id.clone(), *rank)).collect()
    };
    let ranked =
        rank_candidates_scoped(query, all.iter().map(|m| m.content.as_str()), weights.overlap_scope);
    let overlap_stream: Vec<RankedHit> = ranked
        .iter()
        .take(OVERLAP_LIMIT)
        .enumerate()
        .map(|(i, (idx, _))| RankedHit { id: all[*idx].id.clone(), rank: i + 1 })
        .collect();

    // Slot 2 is always occupied; see the doc comment. The recency branch is the
    // same `weight_for` gate the service uses, so "is this stream live" is one
    // value rather than two.
    let recency_stream = if weights.recency_weight_for(query) != 0.0 {
        recency_rank(&all, chrono::Utc::now(), weights.recency_half_life_days)
    } else {
        Vec::new()
    };
    // The dense stream. Skipped entirely at the shipped `0.0`, so the default
    // build never embeds a query and never reads the vector table.
    let ids: Vec<String> = all.iter().map(|m| m.id.clone()).collect();
    let dense = if weights.vector == 0.0 {
        Vec::new()
    } else {
        let stored: HashMap<String, Vec<f32>> = store.bank_vectors(bank_id)?.into_iter().collect();
        let qv = embedder.embed(query).map_err(|e| StoreError::Unsupported(
            match e {
                EmbedError::Model(_) => "embed session",
                EmbedError::Dimension { .. } => "embed dimension",
                EmbedError::Unusable(_) => "embed output",
            },
        ))?;
        vector_stream(&ids, &qv, &stored)
    };
    let fused = rrf_fuse_with_magnitudes(
        &[fts_stream, overlap_stream, recency_stream, dense],
        &magnitudes,
        weights,
    );

    let by_id: HashMap<&str, &Memory> = all.iter().map(|m| (m.id.as_str(), m)).collect();
    let mut ordered: Vec<String> = Vec::with_capacity(fused.len());
    let mut scores: HashMap<&str, f64> = HashMap::with_capacity(fused.len());
    let mut contents: HashMap<&str, &str> = HashMap::with_capacity(fused.len());
    for (id, score) in &fused {
        scores.insert(id.as_str(), *score);
        if let Some(m) = by_id.get(id.as_str()) {
            contents.insert(id.as_str(), m.content.as_str());
        }
        ordered.push(id.clone());
    }
    let kept = trim_to_budget(&ordered, &contents, budget_tokens);
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

/// Retain `content` into `bank_id` and store its embedding alongside it.
///
/// Two store calls, deliberately **not** one transaction: the memory is the
/// record and the vector is a derived index over it. Wrapping them together
/// would mean a failed inference rolls back a write the caller was told
/// succeeded, and a succeeded memory whose vector is missing is merely a row the
/// vector stream cannot rank — which is the same state the store is in for every
/// memory retained before the arm existed, and which it handles by scoring `0.0`
/// in a stream that is off.
///
/// # The vector is built from the *redacted* text, and that is not a detail
///
/// `retain_doc` runs [`redact_pii`] before it persists, so the text the store
/// will hold is not the text it was handed. Embedding the caller's original
/// would put a secret — an email, a JWT, a bearer token — into a column that no
/// redaction pass ever sees, and an embedding of a secret is still the secret:
/// it is a deterministic function of it, retrievable by anyone who can run the
/// model, and not greppable, so nothing would ever notice it was there.
///
/// So the same redaction runs first, and the *redacted* string is what is both
/// embedded and persisted. `retain_doc` redacts a second time, which is a no-op
/// because every pattern it matches has already been replaced by a placeholder
/// that matches none of them —
/// `redaction_is_idempotent_so_the_vector_matches_the_stored_row` pins that
/// rather than leaving it as an assumption, and
/// `the_stored_vector_is_the_embedding_of_the_redacted_text` pins the pairing.
pub fn retain(
    store: &impl Store,
    bank_id: &str,
    content: &str,
    tags: &[String],
    document_id: Option<&str>,
    embedder: &mut Embedder,
) -> Result<String, EmbedError> {
    let redacted = redact_pii(content);
    // `MemoryService::new` takes an `S` and wraps it in an `Arc`, so the borrow
    // itself is the `S`: `store.rs` has one blanket `impl Store for &T`, so this
    // forwards every method rather than a hand-picked six of them.
    let svc = crate::api::MemoryService::new(store);
    let id = svc
        .retain_doc(
            bank_id,
            &redacted,
            None,
            tags,
            document_id,
            crate::store::UpdateMode::Replace,
        )
        .map_err(|e: ApiError| EmbedError::Model(e.to_string()))?;
    let vector = embedder.embed(&redacted)?;
    store
        .put_vector(bank_id, &id, &vector)
        .map_err(|e| EmbedError::Model(e.to_string()))?;
    Ok(id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::{decode_vector, SqliteStore};
    use sha2::{Digest, Sha256};
    use std::sync::Mutex;

    fn bank(id: &str) -> crate::memory::Bank {
        crate::memory::Bank { id: id.to_string(), name: id.to_string() }
    }

    fn mem(id: &str, bank_id: &str, content: &str) -> Memory {
        Memory {
            id: id.to_string(),
            bank_id: bank_id.to_string(),
            content: content.to_string(),
            context: None,
            created_at: None,
        }
    }

    /// One embedder for the whole module's tests: building the session reads and
    /// initialises 23 MB of weights, and it is the same immutable thing every
    /// time. `Mutex` because tests run in parallel threads and `embed` needs
    /// `&mut`.
    fn embedder() -> &'static Mutex<Embedder> {
        static ONCE: std::sync::OnceLock<Mutex<Embedder>> = std::sync::OnceLock::new();
        ONCE.get_or_init(|| {
            Mutex::new(Embedder::new().expect("the vendored model must load offline"))
        })
    }

    /// The provenance table is a claim; this is the check. Hashing the bytes the
    /// compiler put in the binary and comparing to the literal recorded in the
    /// module docs is what makes that table a fact rather than a comment — a blob
    /// replaced without updating the digest fails here.
    #[test]
    fn the_vendored_model_is_the_file_the_provenance_records() {
        let digest = Sha256::digest(MODEL_BYTES);
        let got = digest.iter().map(|b| format!("{b:02x}")).collect::<String>();
        assert_eq!(got, MODEL_SHA256, "the vendored ONNX file is not the recorded one");
        // And it is the int8 build, not the 90 MB fp32 one this must never use.
        assert_eq!(
            MODEL_BYTES.len(),
            22_972_370,
            "fp32 weights would be 90.4 MB; only the int8 build is vendored"
        );
    }

    /// The same claim for the tokenizer, and it pins the *form*: a `vocab.txt`
    /// swapped in here would fail to build the tokenizer at all, so the byte
    /// count is the assertion that cannot be accidentally satisfied.
    #[test]
    fn the_vendored_tokenizer_is_the_fast_tokenizers_serialisation() {
        let got = Sha256::digest(TOKENIZER_BYTES)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>();
        assert_eq!(
            got,
            "da0e79933b9ed51798a3ae27893d3c5fa4a201126cef75586296df9b4d2c62a0"
        );
        // Contains the vocabulary the module docs claim: 30,522 WordPiece entries.
        let text = String::from_utf8_lossy(TOKENIZER_BYTES);
        assert!(text.contains("\"[UNK]\"") && text.contains("\"[CLS]\""));
    }

    /// The real end-to-end contract: a 384-d, finite, unit-length row, from a
    /// text, with no network and no file on disk. This is the test another agent
    /// should read to learn the minimum: build an [`Embedder`], call `embed`.
    #[test]
    fn the_embedder_returns_384_dimensions() {
        let mut e = embedder().lock().expect("embedder mutex");
        let v = e.embed("auth uses jose").expect("embed");
        assert_eq!(v.len(), EMBED_DIM, "the constant must match the graph");
        assert!(v.iter().all(|x| x.is_finite()), "a NaN would reorder the fused list");
        let norm = v.iter().map(|x| x * x).sum::<f32>().sqrt();
        assert!((norm - 1.0).abs() < 1e-3, "mean-pooled rows are L2-normalised, got ‖v‖={norm}");
    }

    /// The property the whole arm rests on, asserted rather than hoped for: two
    /// texts that share no vocabulary are far apart in embedding space, so the
    /// arm can reach a document the lexical streams score `0.0`. If this ever
    /// fails, the 23 MB is not buying the thing the 23 MB was for.
    #[test]
    fn the_embedder_separates_a_paraphrase_from_an_unrelated_pair() {
        let mut e = embedder().lock().expect("embedder mutex");
        let paraphrase = e
            .embed("SSO redirect loop fixed by clearing the stale session cookie")
            .expect("embed");
        let about_it = e.embed("how did we handle a user who could not log in").expect("embed");
        let unrelated = e.embed("quarterly revenue by region and segment").expect("embed");
        let near = crate::embed::cosine(&paraphrase, &about_it);
        let far = crate::embed::cosine(&paraphrase, &unrelated);
        assert!(
            near > far + 0.20,
            "a paraphrase must be much closer than an unrelated text ({near} vs {far})"
        );
        // And the shared-vocabulary tokens the lexical streams rank on are absent
        // from the paraphrase pair, which is the reach the arm is for.
        assert!(!about_it.is_empty());
    }

    /// A cosine stream is an ordering, and an ordering is all RRF consumes.
    /// Checked against the kernel it delegates to rather than reimplemented.
    #[test]
    fn the_vector_stream_ranks_by_cosine_and_omits_vectorless_ids() {
        let ids = vec!["a".to_string(), "b".to_string(), "c".to_string()];
        let mut vectors = HashMap::new();
        vectors.insert("a".to_string(), vec![1.0, 0.0]);
        vectors.insert("b".to_string(), vec![0.6, 0.8]);
        let q = vec![1.0, 0.0];
        let stream = vector_stream(&ids, &q, &vectors);
        // "c" has no vector: not ranked, rather than ranked at 0.0 as unranked
        // filler. "a" (cos 1.0) outranks "b" (cos 0.6).
        assert_eq!(stream.len(), 2);
        assert_eq!(stream[0].id, "a");
        assert_eq!(stream[0].rank, 1);
        assert_eq!(stream[1].id, "b");
        assert_eq!(stream[1].rank, 2);

        // A store with no vectors for any pooled id yields no stream at all,
        // rather than a fabricated one.
        assert!(vector_stream(&ids, &q, &HashMap::new()).is_empty());
    }

    /// Vectors survive a write/read round trip through SQLite bit-for-bit.
    /// Little-endian f32 in a BLOB, so this is a real encoding test and not an
    /// approximate one.
    #[test]
    fn a_stored_vector_round_trips_bit_for_bit() {
        let store = SqliteStore::open_in_memory().expect("open");
        store.put_bank(&bank("b")).expect("bank");
        store.put(&mem("m1", "b", "c")).expect("put");
        let v = vec![0.5f32, -0.25, 0.0, 1.0 / 3.0];
        store.put_vector("b", "m1", &v).expect("put vector");
        let back = store.bank_vectors("b").expect("read");
        assert_eq!(back.len(), 1);
        assert_eq!(back[0].0, "m1");
        for (a, b) in v.iter().zip(&back[0].1) {
            assert_eq!(a.to_bits(), b.to_bits(), "f32 must round trip exactly");
        }
    }

    /// Bank isolation is the property every query in this crate promises, and for
    /// the vector table it is structural: `bank_vectors` names a bank and has no
    /// all-banks form. So a vector written into one bank is simply not in the
    /// map another bank's recall reads.
    #[test]
    fn vectors_are_isolated_by_bank() {
        let store = SqliteStore::open_in_memory().expect("open");
        for b in ["a", "b"] {
            store.put_bank(&bank(b)).expect("bank");
        }
        store.put(&mem("ma", "a", "c")).expect("put a");
        store.put(&mem("mb", "b", "c")).expect("put b");
        store.put_vector("a", "ma", &[1.0, 0.0]).expect("vector a");
        store.put_vector("b", "mb", &[0.0, 1.0]).expect("vector b");

        let a = store.bank_vectors("a").expect("read a");
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].0, "ma");
        assert!(a.iter().all(|(id, _)| id == "ma"), "bank a must not see bank b");
        let b = store.bank_vectors("b").expect("read b");
        assert_eq!(b.len(), 1);
        assert_eq!(b[0].0, "mb");

        // A blank bank id is refused rather than read as "no filter".
        assert!(matches!(
            store.bank_vectors("  "),
            Err(StoreError::InvalidBank)
        ));
    }

    /// Deleting a memory retires its vector, through the same `ON DELETE CASCADE`
    /// the tag rows use — so the vector stream can never cite a memory that is
    /// gone. This is the reason the storage is a side table with a foreign key
    /// rather than a column.
    #[test]
    fn deleting_a_memory_retires_its_vector() {
        let store = SqliteStore::open_in_memory().expect("open");
        store.put_bank(&bank("b")).expect("bank");
        store.put(&mem("m1", "b", "c")).expect("put");
        store.put_vector("b", "m1", &[1.0, 2.0]).expect("vector");
        assert_eq!(store.bank_vectors("b").expect("read").len(), 1);
        store.delete("b", "m1").expect("delete");
        assert!(
            store.bank_vectors("b").expect("read").is_empty(),
            "a deleted memory must not leave a rankable vector behind"
        );
    }

    /// A blob that cannot be decoded is dropped, not errored and not guessed at.
    /// A `NaN` reaching `cosine` would poison `partial_cmp` for every id sharing
    /// it and silently drop the fused list onto its id tiebreak.
    #[test]
    fn an_unreadable_vector_row_is_dropped_rather_than_scored() {
        assert!(decode_vector(&[0u8; 15], 3).is_none(), "length not a whole f32 count");
        assert!(decode_vector(&[0u8; 16], 5).is_none(), "dim disagrees with the blob");
        assert!(decode_vector(&[0u8; 16], 0).is_none(), "no width");
        let mut nan = f32::NAN.to_le_bytes().to_vec();
        nan[3] = 0x7f; // pin the payload so this is a NaN and not a denormal
        assert!(decode_vector(&nan, 1).is_none(), "a non-finite component is refused");
        assert_eq!(decode_vector(&1.5f32.to_le_bytes(), 1), Some(vec![1.5]));
    }

    /// A caller error is refused before SQL, and refused *loudly*: a zero-width
    /// or non-finite vector silently stored would make every cosine against it
    /// `0.0` with nothing to explain it.
    #[test]
    fn an_unusable_vector_is_refused_before_it_reaches_the_database() {
        let store = SqliteStore::open_in_memory().expect("open");
        for bad in [vec![], vec![f32::NAN], vec![f32::INFINITY]] {
            assert!(
                matches!(
                    store.put_vector("b", "m1", &bad),
                    Err(StoreError::InvalidVector)
                ),
                "{bad:?} must be refused"
            );
        }
    }

    /// The shipped default never reaches the model. At `vector: 0.0` the arm must
    /// not embed the query, must not read the vector table, and must produce the
    /// same `f64` values the two-stream kernel produces.
    #[test]
    fn the_default_weight_never_embeds_anything() {
        let store = SqliteStore::open_in_memory().expect("open");
        store.put_bank(&bank("b")).expect("bank");
        store.put(&mem("m1", "b", "auth uses jose")).expect("put 1");
        store.put(&mem("m2", "b", "rate limiting notes")).expect("put 2");
        let mut e = embedder().lock().expect("embedder mutex");
        // Real 384-d vectors, stored through the same path a retain uses. A toy
        // 2-d vector would score `0.0` against a 384-d query by
        // `embed::cosine`'s length-mismatch rule, and the arm would appear inert
        // for a reason that has nothing to do with its weight.
        for (id, content) in [("m1", "auth uses jose"), ("m2", "rate limiting notes")] {
            let v = e.embed(content).expect("embed");
            store.put_vector("b", id, &v).expect("vector");
        }

        let off = recall(&store, "b", "jose auth", 2000, &[], &FusionWeights::SHIPPED, &mut e)
            .expect("recall at the shipped weight");
        // With the vector weight live and the two lexical streams the only voters,
        // the ordering is the lexical one — which is the point: the dense stream
        // is fused as a voter, it does not take over.
        let on = recall(
            &store,
            "b",
            "jose auth",
            2000,
            &[],
            &FusionWeights {
                vector: 0.5,
                ..FusionWeights::SHIPPED
            },
            &mut e,
        )
        .expect("recall with the vector stream live");
        assert!(!on.is_empty());
        fn ids(rows: &[ScoredMemory]) -> Vec<&str> {
            rows.iter().map(|r| r.memory.id.as_str()).collect()
        }
        // **The reach, asserted rather than asserted-about.** `m2` shares no token
        // with "jose auth", so `fts_match_query` never names it and
        // `rank_candidates` drops it at `score > 0.0`: at the shipped weight the
        // fused list is `m1` alone. Turning the dense stream on puts `m2` in it,
        // because cosine does not care about tokens. That is the whole argument
        // for the arm in one assertion, and no weight of the other two could have
        // produced it.
        assert_eq!(ids(&off), ["m1"], "at the shipped weight the lexical pair answers alone");
        assert_eq!(ids(&on), ["m1", "m2"], "the dense stream reaches a tokenless match");
        // And `m1` is still first, so the arm is a voter rather than a takeover.
        // The two differ in score, because the dense stream really was read.
        let score = |rows: &[ScoredMemory], id: &str| {
            rows.iter()
                .find(|r| r.memory.id == id)
                .map(|r| r.score)
                .expect("id present")
        };
        assert!(
            (score(&on, "m1") - score(&off, "m1")).abs() > 1e-9,
            "a live vector weight must change the fused score"
        );
    }

    /// The default-weight call is *bit*-identical to the same recall computed
    /// without the vector arm at all — the strongest form of the inertness claim,
    /// and the one that would catch a stray `1e-18` that a tolerance would let
    /// through into a downstream tiebreak.
    #[test]
    fn the_shipped_weight_produces_the_lexical_scores_exactly() {
        let store = SqliteStore::open_in_memory().expect("open");
        store.put_bank(&bank("b")).expect("bank");
        for (id, c) in [
            ("m1", "auth uses jose"),
            ("m2", "rate limiting notes"),
            ("m3", "deploy runbook"),
        ] {
            store.put(&mem(id, "b", c)).expect("put");
        }
        let mut e = embedder().lock().expect("embedder mutex");
        let armed = recall(&store, "b", "jose auth", 2000, &[], &FusionWeights::SHIPPED, &mut e)
            .expect("recall");
        // The same fusion with no dense stream at all, which is what the arm
        // reduces to at 0.0.
        let bare = {
            let (all, hits) = store.recall_inputs("b", "jose auth", &[], FTS_LIMIT).expect("inputs");
            let fts: Vec<RankedHit> = hits
                .iter()
                .enumerate()
                .map(|(i, (id, _))| RankedHit { id: id.clone(), rank: i + 1 })
                .collect();
            let ranked = rank_candidates_scoped(
                "jose auth",
                all.iter().map(|m| m.content.as_str()),
                FusionWeights::SHIPPED.overlap_scope,
            );
            let ov: Vec<RankedHit> = ranked
                .iter()
                .take(OVERLAP_LIMIT)
                .enumerate()
                .map(|(i, (idx, _))| RankedHit { id: all[*idx].id.clone(), rank: i + 1 })
                .collect();
            let fused = rrf_fuse_with_magnitudes(
                &[fts, ov, Vec::new(), Vec::new()],
                &HashMap::new(),
                &FusionWeights::SHIPPED,
            );
            fused
                .iter()
                .map(|(id, s)| (id.clone(), *s))
                .collect::<Vec<_>>()
        };
        for (id, score) in &bare {
            let got = armed
                .iter()
                .find(|r| &r.memory.id == id)
                .map(|r| r.score)
                .expect("bare id present in the armed recall");
            assert_eq!(
                got.to_bits(),
                score.to_bits(),
                "score at {id} drifted: {got:?} vs {score:?}"
            );
        }
    }

    /// A blank bank id is refused before any store call, so a typo cannot read a
    /// vector table the way a blank lexical query would.
    #[test]
    fn a_blank_bank_is_refused_before_any_store_call() {
        let store = SqliteStore::open_in_memory().expect("open");
        let mut e = embedder().lock().expect("embedder mutex");
        assert!(matches!(
            recall(&store, "  ", "q", 2000, &[], &FusionWeights::SHIPPED, &mut e),
            Err(StoreError::InvalidBank)
        ));
    }

    /// The assumption `retain`'s redaction pairing rests on, checked rather than
    /// asserted in prose: a second redaction pass changes nothing, so the string
    /// handed to `retain_doc` survives to the row unchanged and the string that
    /// was embedded is byte-for-byte the string that was stored.
    ///
    /// If this ever fails the consequence is concrete — either the stored row
    /// carries a placeholder the vector never saw, or the vector is of a different
    /// text than the row describes.
    #[test]
    fn redaction_is_idempotent_so_the_vector_matches_the_stored_row() {
        for secret in [
            "reach me at ishan@example.com about the deploy",
            "Authorization: Bearer abcdef0123456789abcdef",
            "key sk-abcdefghijklmnopqrstuvwxyz012345",
            "-----BEGIN PRIVATE KEY-----\nMIIEvQIBADANBg\n-----END PRIVATE KEY-----",
            "nothing to redact here at all",
            "",
        ] {
            let once = redact_pii(secret);
            let twice = redact_pii(&once);
            assert_eq!(once, twice, "redaction must be idempotent for {secret:?}");
        }
        // And it really does remove something, or the test above proves nothing.
        assert_ne!(redact_pii("mail ishan@example.com"), "mail ishan@example.com");
    }

    /// The pairing itself: the vector in the table is the embedding of the text
    /// the row holds, bit for bit. A secret in a vector is still a secret — it is
    /// a deterministic function of one, retrievable by anyone who can run the
    /// model, and not greppable, so nothing downstream would ever notice.
    #[test]
    fn the_stored_vector_is_the_embedding_of_the_redacted_text() {
        let store = SqliteStore::open_in_memory().expect("open");
        let mut e = embedder().lock().expect("embedder mutex");
        let content = "the oncall pager is ishan@example.com and the token is sk-abcdefghijklmnopqrstuvwxyz012345";
        let id = retain(&store, "b", content, &[], None, &mut e).expect("retain");

        let stored = store.get("b", &id).expect("get").expect("row exists");
        assert!(!stored.content.contains("ishan@example.com"), "the row must be redacted");
        assert!(!stored.content.contains("sk-abcdefghijklmnopqrstuvwxyz012345"));

        let expected = e.embed(&stored.content).expect("embed the stored text");
        let got = store.bank_vectors("b").expect("read");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].0, id);
        for (a, b) in expected.iter().zip(&got[0].1) {
            assert_eq!(
                a.to_bits(),
                b.to_bits(),
                "the stored vector must be the embedding of the stored (redacted) text"
            );
        }
    }

    /// The bank's own `retainTags` reach the row this module writes.
    ///
    /// There are two public retain paths and they must answer the same way. This
    /// one takes a `&impl Store` because it also has to write a vector, and
    /// `MemoryService::new` wants an owned `S` — so the borrow has to be a
    /// [`Store`] itself. Any method that borrow does not forward falls through to
    /// the trait's *default*, and a default is not a compile error: it is a
    /// plausible wrong answer. `get_bank_config`'s default is `Ok(None)`, and the
    /// retain path reads it for exactly this config key, so the failure mode was
    /// a bank that quietly wrote every memory *without* its configured tags while
    /// the service path wrote them with — two public entry points, divergent
    /// rows, and nothing to notice it. The readback below is a tag-filtered
    /// recall rather than a config read, because a dropped tag is invisible from
    /// the config and only visible from the rows it should have marked.
    #[test]
    fn the_vector_retain_path_writes_the_banks_configured_retain_tags() {
        let store = SqliteStore::open_in_memory().expect("open");
        store.put_bank(&bank("ops")).expect("bank");
        store
            .set_bank_config("ops", r#"{"retainTags":["from-config"]}"#)
            .expect("config");

        let mut e = embedder().lock().expect("embedder mutex");
        retain(&store, "ops", "the release is mine", &[], None, &mut e).expect("retain");

        let svc = crate::api::MemoryService::new(&store);
        let tagged = svc
            .recall_filtered("ops", "release", None, &["from-config".to_string()])
            .expect("recall");
        assert_eq!(
            tagged.len(),
            1,
            "the row written by vector::retain does not carry the bank's retainTags"
        );
        assert_eq!(tagged[0].memory.content, "the release is mine");
    }
}
