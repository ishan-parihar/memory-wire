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

/// Which sphere of influence a memory was written for.
///
/// The value is whatever the store holds: the column is free text on purpose,
/// not a SQLite `CHECK`, because a bank that writes a scope this build does not
/// know must degrade to "everything is unfiltered" rather than fail a write.
/// The six this build understands are a closed list on the *read* side; a typo
/// in a filter therefore serves zero rows rather than the whole bank, and a
/// typo on the write side is stored verbatim and matches nothing.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Scope(pub String);

impl Default for Scope {
    fn default() -> Self {
        Scope("local".to_string())
    }
}

/// What the memory is *for*, as opposed to how it was captured.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Aspect(pub String);

impl Default for Aspect {
    fn default() -> Self {
        Aspect("episodic".to_string())
    }
}

/// Where the memory is in its lifecycle.
///
/// `active` is the default and the only value the default recall path does not
/// special-case. `superseded` and `quarantined` are excluded from recall unless
/// the caller asks for history, which is the one read-side guarantee this
/// dimension carries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lifecycle(pub String);

impl Default for Lifecycle {
    fn default() -> Self {
        Lifecycle("active".to_string())
    }
}

/// How much confidence the memory deserves.
///
/// A write ceiling, not a ranking input: the value decides what a mount may
/// read and is never added to any score.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trust(pub String);

impl Default for Trust {
    fn default() -> Self {
        Trust("agent_inferred".to_string())
    }
}

/// The six orthogonal things a memory carries beyond its content.
///
/// One bundle rather than six parameters because the three callers that thread
/// "just the metadata" — retain, the recall filter, the startup capsule — all
/// need the same handful of fields, and the bundle is the type they already
/// know. `parent_id` and `superseded_by` are plain strings on purpose: a
/// dangling pointer must never fail a write, and a row must never disappear
/// because the row it points at was deleted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryMeta {
    /// Which sphere of influence, e.g. `policy` | `global` | `domain` |
    /// `local` | `agent` | `session`. Free text; stored verbatim.
    pub scope: Scope,
    /// What the memory is for, e.g. `preference` | `invariant` | `decision` |
    /// `procedure` | `trap` | `fact` | `map` | `episodic`.
    pub aspect: Aspect,
    /// Lifecycle stage, e.g. `candidate` | `active` | `quarantined` |
    /// `superseded` | `archived`.
    pub lifecycle: Lifecycle,
    /// How much confidence the memory deserves. Stored as free text; the tiers
    /// this build's ceiling understands are `untrusted_external` |
    /// `agent_inferred` | `human_stated` | `verified`, lowest to highest — see
    /// [`crate::api::Trust`].
    ///
    /// The design plan (W2) sketched a second set of names (`manual_policy`,
    /// `human_approved`, `agent_verified`); the shipped surface implemented the
    /// four above. The plan's names are not accepted, which is why this doc says
    /// the four that are.
    pub trust: Trust,
    /// The row this memory was derived from, if any. Not a foreign key: a
    /// dangling pointer must not fail a write.
    pub parent_id: Option<String>,
    /// The row that supersedes this one, if any. Not a foreign key.
    pub superseded_by: Option<String>,
    /// Comma-separated path prefixes this memory applies to, e.g.
    /// `src/,tests/`. Empty means every path.
    pub scope_globs: Vec<String>,
}

/// The six "no opinion" values. Written out in one place so the defaults are
/// auditable at a glance; `Scope`, `Aspect`, `Lifecycle` and `Trust` each
/// carry their own default above, and the two pointers and the glob list are
/// empty by construction.
#[allow(clippy::derivable_impls)]
impl Default for MemoryMeta {
    fn default() -> Self {
        Self {
            scope: Scope::default(),
            aspect: Aspect::default(),
            lifecycle: Lifecycle::default(),
            trust: Trust::default(),
            parent_id: None,
            superseded_by: None,
            scope_globs: Vec::new(),
        }
    }
}

impl MemoryMeta {
    /// Whether this memory's scope matches any of the requested paths.
    ///
    /// A memory with no glob list applies everywhere, so a path-conditioned
    /// read without paths still sees it. A memory with globs applies only when
    /// one of them is a prefix of an active path — the "conditional rule"
    /// behaviour of a map/instruction memory without a new store.
    pub fn matches_paths(&self, active_paths: &[String]) -> bool {
        if self.scope_globs.is_empty() {
            return true;
        }
        self.scope_globs.iter().any(|glob| {
            active_paths
                .iter()
                .any(|p| p.starts_with(glob.as_str()))
        })
    }
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
    /// The dimensional metadata bundle. Defaulted so a legacy row reads unchanged.
    #[serde(default)]
    pub meta: MemoryMeta,
}
