#![deny(missing_docs)]
//! memory-wire: best-of-both memory infrastructure in Rust.
//!
//! Integrates:
//! - Hindsight (vectorize-io/hindsight): biomimetic learning memory —
//!   retain/recall/reflect, banks.
//! - agentmemory (rohitg00/agentmemory): coding-agent capture —
//!   hooks auto-capture, hybrid RRF recall, MCP tools.
//!
//! See PLAN.md for the integration plan and docs/AUDIT.md for source audits.
//!
//! # Getting started
//!
//! A service is `MemoryService` over a store; the in-memory SQLite store is the
//! zero-setup one. `retain` creates the bank it names, so a first write is
//! also the bank declaration:
//!
//! ```
//! use memory_wire::api::MemoryService;
//! use memory_wire::store::SqliteStore;
//!
//! let store = SqliteStore::open_in_memory().expect("open in-memory store");
//! let svc = MemoryService::new(store);
//!
//! svc.retain("demo", "auth uses jose middleware", None)
//!     .expect("retain");
//!
//! // Recall is bank-isolated and budgeted; the query finds the memory retained above.
//! let hits = svc.recall("demo", "jose", 2000).expect("recall");
//! assert_eq!(hits.len(), 1);
//! assert_eq!(hits[0].memory.content, "auth uses jose middleware");
//! ```
//!
//! Nothing above survives the process: [`store::SqliteStore::open_in_memory`]
//! builds a throwaway database. For a store that persists, pass
//! [`store::SqliteStore::open`] a path instead.

pub mod api;
pub mod capture;
pub mod embed;
pub mod memory;
pub mod recall;
pub mod store;
/// The vendored, offline dense-vector arm: 23 MB of int8 MiniLM weights plus the
/// tokenizer, compiled into the binary behind the `embed` cargo feature. Absent
/// from a default build entirely, which is what keeps the shipped artifact
/// small and the vector arm honestly attributable.
#[cfg(feature = "embed")]
pub mod vector;
