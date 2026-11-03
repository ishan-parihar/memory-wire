# Architecture

**What this document is.** The shape of the whole system on one page, written
for someone who will not read the code. If you want the full API surface,
`docs/` holds it. If you want to know what this thing *is*, this is the page.

---

## The one sentence

memory-wire gives an AI agent a memory that survives the conversation, keeps
each project's memories separate from every other project's, and does it in a
binary small enough to ship as a download rather than an install.

---

## The parts

A Rust library with five modules behind one service, plus a binary that
exposes the same service three ways. They are separate because they fail
differently and are measured separately.

| Module | What it is | Why it is its own module |
|---|---|---|
| `api` | `MemoryService` — retain, recall, reflect. The only thing a caller touches. | The interface is the contract; changing it is a breaking change, so it sits apart from every implementation. |
| `store` | SQLite persistence, and an in-memory variant for tests and zero-setup use. | Storage is swappable by design, and the in-memory store exists so a test never touches a file on disk. |
| `recall` | Bank-isolated retrieval: FTS5 BM25 plus reciprocal-rank fusion. | Ranking is the part that is benchmarked, so it is the part that must be measured in isolation. |
| `capture` | What gets kept automatically while a coding agent works. | Capture and recall meet at `api` and nowhere else; coupling them would make retention policy a side effect of search. |
| `embed` | Optional dense-vector arm — 23 MB of int8 weights, behind a cargo feature and absent from a default build. | A feature that doubles the binary has to be optional at compile time, not just at runtime. |
| `memory` | Bank model, isolation rules, and PII redaction on write. | The privacy guarantee is a boundary, and a boundary belongs where it can be audited in one place. |

---

## How a memory gets kept

```
 an agent working in one terminal writes something worth keeping
        |
 capture: what is worth keeping is decided here, not by the caller
        |
 memory: the entry is redacted on write and assigned to a bank
        |
 store: one row, in this bank, in one SQLite file
```

---

## How a memory comes back

```
 a query arrives, with the bank it may look in
        |
 recall: FTS5 BM25 and RRF fusion rank within that bank only
        |
 the budget is applied before the bytes are returned
        |
 the caller gets the ranked set, each entry stamped with its own date
```

The budget is applied at the end and not the beginning, which is what keeps a
recall from returning something the caller did not ask for.

---

## The boundaries

- **One service, three doors.** HTTP, an MCP server over stdio, and the CLI all
  reach the same `MemoryService`. There is no second implementation of recall.
- **Banks never mix.** A recall names its bank. There is no cross-bank fallback,
  because a memory from one project appearing in another is the failure this
  exists to prevent.
- **Redaction happens on write, not on read.** A secret is not stored, so it
  cannot be recalled.
- **The daemon is a deployment, not a dependency.** No background process is
  required; `memory-wire daemon start` runs the same binary in the background
  when you want it to outlive a terminal.

---

## What this respects

The design integrates two upstream approaches — biomimetic retain/recall/reflect
and coding-agent auto-capture — and the credit is recorded: `docs/AUDIT.md`
holds the teardowns of both, and the benchmark claims live in `eval/` with the
datasets they were run on.
