# B-DELTA — what landed since round 3, and what it changed about the shape

Round 3 closed at `98b1a36`. Six commits later, at `3ca8857`. Every claim here
names the command or the file that settles it.

---

## 1. The four-door enforcement problem was real, and it is now closed in all four doors

Round 3's strongest finding was that one policy — the `untrusted_external`
injection gate — existed in `src/hooks.rs` and the hermes plugin but in **neither
TypeScript plugin**. Both injected every row a recall returned, so an untrusted
mount's web content reached a model's system prompt as rules.

What round 3 did *not* notice: **hermes had the gate and no executable check.**
It filtered correctly (`__init__.py:516`), before the take, with the right
reasoning. Nothing could run it. The gate was enforced in three of four doors and
checked in two.

Closed across four batches:

| commit | what |
|---|---|
| `1f5ee17` (round 3) | gate added to both TS plugins, `trust` on the recall wire, `UntrustedExternal` added as a real tier |
| `db57850` | hermetic test for hermes (`trust_gate_test.py`, local HTTP stub, no hermes runtime) |
| `0b518de` | occurrence dates on all four doors |
| `1f93bf0` | capsule knob on the three plugin doors |

**The shape that matters for this round:** the failure was never a bug in one
file. It was **a policy with N implementations and N−1 checks**, plus a register
that stopped being maintained. The same shape produced:

- `measure_surface.sh` counting `src/` and stopping, so 1,622 lines of
  injection-path code were never in any surface number
- `TODO.md` row 4 listing P2.1/P4.1 open for a month after they shipped
- `INTEGRATION_GAPS.md` listing G6.8 open after hermes implemented it
- `README.md` describing a capsule knob that only the Rust door read

That is five instances of one defect class in one repo, found by one round of
external review. **It is the strongest evidence in this pack that the remaining
risk is not in any single module.**

---

## 2. The measured surface (re-derivable: `measure_surface.sh`)

| | claim | measured |
|---|---|---|
| `src/` size | AGENTS.md: **24.8k lines, 22 modules** | **35,298 lines, 32 modules** |
| AGENTS.md table | — | **18 rows** |
| modules absent from that table | — | **14**: `capture`, `connect_claude`, `connect_copilot`, `connect_ext`, `connect_opencode`, `grounding`, `guidelines`, `http`, `hygiene`, `lib`, `memory`, `skills`, `sweep`, `update` |
| `connect_*` family | 1 module in the table | **6,609 lines across 7 files** |
| TS plugins | not counted at all in round 3 | **741 lines, 98 identical** (59% of the smaller file) |
| inline `#[test]` blocks | — | **631** |

Line counts include inline test blocks, so a per-module count is a mix of code
and its own tests. That is deliberate and stated, not hidden: the repo keeps
tests in the same file (`AGENTS.md` §5), so "lines per module" is the unit a
reader of that module actually reads.

---

## 3. The open surface is five rows, and four of them are blocked on you

Verified from `TODO.md` on 2026-11-19:

| row | status | blocked on |
|---|---|---|
| 10 — agent-memory-bench Claude Code arm | open | agent-model budget |
| 11 — LLM-judged runs (AMB rag, OAMB, memorybench, MemBase) | open | budget **and** a consultation-budget conflict |
| 12 — AMB provider at turn granularity | open | **nothing — already decided by data** |
| 13 — aroute B1–B4 | open | another repo, another owner |
| 31 — door parity (redact/hygiene/supersede) | open | a surface decision |

Row 12 is worth naming precisely because it is the inverse of the round-3
finding: it is marked open, and its own row text says the decision was made.
Turn-level writes lose to shipped whole sessions on every aggregate
(`LOCOMO_TURNS.md`: R@1 54.6% vs 60.4%, `recall_all@10` 92.2% vs 94.2%, n
1,531), and windows breach the pool ceiling. **The register is stale in both
directions — it carries done work as open and open work as undecided.**

---

## 4. What the last pass exposed that nobody had recorded

Three findings from writing the *previous* plan, all verified, all new:

- **PersonaMem v2 ran to completion on 2026-10-17 and was never scored.**
  4,954 answers, 4,954 judged, 4,989 successful searches, 0 failures. The
  answers carry `is_correct`, so no judge re-run is needed. Scored now:
  **37.8% (1,872 / 4,954)**. `TODO.md` row 18 has said "named-not-scheduled"
  for two months. It is neither — it is run and unread. The directory is
  gitignored on purpose (the run wrote memory rows verbatim, one carrying a
  pasted bot token), so the raw rows cannot be committed under AGENTS §14.
- **`../_audit/` and `../_audit_scratch/` are both gone.** The pinned
  OmniMemEval clone (`a5c8b4b`) no longer exists on this box. H10 and H12 are
  reproducible in principle; re-running either means **re-cloning and
  re-pinning**, which is a different cost than DECISIONS §2 assumes.
- **`RND_PLAN.md` §6 still says "the upcoming cycle, outlined"** while W1–W3
  are all done (§47, §48, §46).

---

## 5. The provenance of every number here

| figure | class | where |
|---|---|---|
| 35,298 lines / 32 modules / 631 tests | `[LIVE]` | `measure_surface.sh`, run 2026-11-19 |
| 98 / 741 TS lines identical | `[LIVE]` | same script |
| 24.8k / 22 modules / 18 rows | `[CERT]` | `AGENTS.md`, quoted as the claim under audit |
| 37.8% PersonaMem v2 | `[LIVE]`-computed, **unrecorded** | `eval/omnimevaleval/memory-wire-mw_pmv2_dated/`, computed 2026-11-19 from committed run outputs; the directory is gitignored |
| 60.4% / 54.6% / 94.2% / 92.2% | `[CERT]` | `eval/LOCOMO.md`, `eval/LOCOMO_TURNS.md` |
| 659 tests, clippy clean, 0 doc warnings | `[LIVE]` | gate run 2026-11-19 at `3ca8857` |
| 500/500 gold in pool | `[CERT]` | `AGENTS.md` §1, from `eval/RESULTS.md` |
| 42 commits since v0.10.0 | `[LIVE]` | `git log v0.10.0..HEAD` |

**Not in this pack, deliberately:** any LongMemEval retrieval number as a
selection signal. Five consultations are spent; AGENTS §6 reserves the sixth for
release acceptance.