//! Canonical memory types shared by all layers.
//!
//! Mirrors Hindsight banks/observations/mental-models and agentmemory
//! working/episodic/semantic/procedural tiers into one Rust model.

use serde::{Deserialize, Serialize};

/// Isolated memory store — one "brain" per user/agent/project (Hindsight bank).
///
/// The hook preamble reads a bank's `background` from its config JSON
/// (`GET .../config`), not from here: one concept, one home.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Bank {
    /// Stable bank identifier (used as isolation key).
    pub id: String,
    /// Human-readable bank name.
    pub name: String,
}

/// Raw captured fact or event (Hindsight retain input / agentmemory observation).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Memory {
    /// Stable memory identifier.
    pub id: String,
    /// Owning bank identifier (isolation key).
    pub bank_id: String,
    /// Redacted content payload.
    pub content: String,
    /// Optional capture context (e.g. hook source).
    pub context: Option<String>,
    /// When the row was created, RFC 3339 UTC.
    ///
    /// `None` means "not stamped yet" and is replaced by the store's own clock on
    /// insert — the only thing a caller that does not care about age should pass.
    /// `Some("1970-01-01T00:00:00Z")` is the sentinel a pre-timestamp database
    /// reads back: epoch here means "age unknown", not "born in 1970".
    ///
    /// An `Option` rather than an empty string because `""` was a third,
    /// unenforced state: a `String` can hold the "stamp it" sentinel, a real
    /// stamp, and anything else, and nothing in the type said which. A stamp the
    /// store cannot parse is still stored verbatim and is then compared *by byte*
    /// against the sweep's cutoff, so its shape decides its age — which is why
    /// the contract is spelled out here and pinned by tests rather than left to a
    /// doc comment.
    pub created_at: Option<String>,
}
