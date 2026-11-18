# Surface audit — memory-wire against the Claude-memory / Dreaming upgrade proposal

Dated-frozen: audited **2026-11-18**; every finding below is marked
**[verified]** (read in this tree at the pinned commit) or **[asserted]**
(inference from a verified fact, or a claim about another repo).

This document exists because three upgrade proposals arrived describing a
`memory-wire` that **does not exist**. All three were written from the README's
top half over a web-browse allowlist, at `b478d15`, and each states its own
blindness. They were not wrong to ask for a hierarchy; they were wrong about
what is already here, and one of them was wrong about the *benchmark's shape*
in a way that would have redirected the whole effort.

**This audit is the gate on the plan.** `docs/plans/2026-11-18-hierarchical-grounded-memory-upgrade.md`
is derived from it, and every work item there names the finding it rests on.

---

## Scope

| | |
|---|---|
| Repo | `agentic-utility/memory/memory-wire`, branch `main` |
| Commit | **`36bb9474f7733227676b1e79a307b9cf195014a3`** (v0.10.0) — *not* the `b478d15` the proposals pin |
| Tree | clean except `M scripts/genome` (a vendored CLI refresh, pre-existing) |
| Gate state | `cargo test --locked`: **345 passed, 1 failed** in the bin suite (`connect::tests::golden_install_shapes`, environmental — see F9); all 4 integration suites pass (18 tests). **This failure predates the audit and is not a regression** — verified by stashing the working tree and re-running on the pristine tree. |
| Machine | AMD Ryzen 9 5900X, 24 logical CPUs, 47 GiB RAM, load 4.33 at test time |

Nothing was tuned, no benchmark was run, no ranking weight was touched, and
**no LongMemEval consultation was consumed** by this audit.

---

## Part I — The surface, as it actually is

### 1. It is much bigger than the proposals describe, and much smaller

| axis | proposals claimed | verified |
|---|---|---|
| MCP tools | **54** | **4** — `memory_retain`, `memory_recall`, `memory_reflect`, `memory_bank_config_get` (`src/mcp.rs`, `RETAIN`/`RECALL`/`REFLECT`/`CONFIG_GET` consts) |
| Rust LOC | ~3k core | **31,480** across 28 modules |
| tests | unmeasured | **602** `#[test]` functions |
| version | `b478d15` | **v0.10.0**, commit `36bb947` |

The "54 MCP tools" figure appears in all three proposals and in *none* of the
repo. `README.md:656` says "the same four tools are served two ways";
`README.md:1024` says "the MCP server (`rmcp`, 4 tools)". The proposals
apparently counted the portfolio's tool surface, not this project's.

**Why this matters and is not pedantry:** two proposals built a whole pillar
("audit & prune the 54 tools", "do not inflate past 54") on a number that is
off by 13×. The real constraint is the opposite one: **4 tools is already
lean, and the risk is not bloat but that the four cannot express the new
model.** A plan that starts by "auditing 54 tools" spends its first week on a
non-problem.

### 2. Most of what Pillar 1–2 of the proposals asks for is already built

| proposal asks for | verified state |
|---|---|
| `WAL` + `busy_timeout` + `BEGIN IMMEDIATE` | **done** — `src/store.rs:1091-1094`: `PRAGMA journal_mode=WAL; foreign_keys=ON; busy_timeout=5000; synchronous=NORMAL`. `synchronous=NORMAL` is a *documented, measured* durability trade (`src/store.rs:1043-1055`). |
| serialize multi-process writes | **done, structurally** — one `Mutex<Connection>` writer + a 4-connection WAL **read pool** (`src/store.rs:784-860`), not one mutex for everything. 16-client soak: 0 `SQLITE_BUSY`, 0 panics (`eval/SOAK.md`). |
| keep blocking SQLite off the tokio workers | **done** — `src/api.rs:1357` `spawn_blocking`; every route and every MCP `call_tool` goes through `api::blocking`. |
| bind loopback by default | **done** — `paths::DEFAULT_ADDR = "127.0.0.1:8888"`, the single literal; both clap defaults read it; a test asserts it. A non-loopback bind emits a **documented, once-per-process warning** naming that there is no auth (`src/main.rs:645-672`). |
| zero daemon / low RSS | **done and measured** — 8.8 MiB binary, 10.7 MiB idle RSS, `DT_NEEDED`=3, 0 background processes until `daemon start`. |
| per-bank `summary` capsules | **done** — `format: "summary"` (`api.rs:205-216`, `summary_content`), per-bank adoption knob plus `MEMORY_WIRE_FORMAT` (`src/hooks.rs:533-574`), frozen-battery measured at −41–43% bytes at byte-identical ranking. |
| occurrence dates in-band | **done** — `[YYYY-MM-DD]` prefixing, HaluMem Dynamic Update 0.2056→0.4056 (`CONSISTENCY.md` §46). |
| `redact` retroactively across history | **absent** — redaction is write-time only (`capture::redact_pii`); there is **no** version table to scrub. |
| `scope`/`domain`/`aspect`/`lifecycle`/`trust_level` | **absent** — no such column exists anywhere in `src/`. |
| `HotIndex` / startup capsule ≤200 lines | **absent**. |
| read-only vs read-write mounts | **absent** — bank isolation is by *name* only. |

So: **the security/concurrency half of the proposals (their Batch 1) is
already shipped**, and the dimensional-schema half (Batch 2) is genuinely
missing. A plan that "starts with WAL and auth" would be re-doing v0.7.0.

### 3. The ranking half: the proposals' central idea was already tried and killed

This is the most important finding in the audit.

Two proposals independently argued that `single-session-preference` sits at
**43.3% R@1 vs 100.0% R@20** because a 2-sentence preference is diluted inside
a 2,626-token session, and that `memory-wire`'s **four "built but inert"
mechanisms** (sentence-level overlap, BM25-magnitude tiebreak, recency stream,
temporal-query classifier) are the exact tools for it.

**[verified]** Every one of those four was pre-registered as a hypothesis,
measured on the LoCoMo dev set (n=1,531), and **closed**:

| mechanism | measurement | verdict |
|---|---|---|
| `keyword_scope: ExactAndPrefix` (H1a) | R@1 60.4→**63.4%**, MRR +2.2pp, NDCG +1.7pp | **gate failed** — `recall_all@10` +0.5pp < the +1pp gate; single-hop R@5 −0.4 |
| `overlap_scope: BestSentence` (H1b) | R@1 60.4→**57.0%**; multi-hop R@1 **−11.2pp**; single-hop **−10.3pp** | **gate failed** |
| `bm25_magnitude: 1.0` (H1c) | R@1 60.4→**60.4%**; `recall_all@10` **−0.2pp** (4 up / 7 down); multi-hop R@1 −3.4pp | **gate failed**, re-closed 2026-10-14 after a re-open |
| `recency 1.0 TemporalQueriesOnly` (H3) | temporal `recall_all@10` **−7.8pp**, temporal R@1 **−33.4pp**; detector-negative lists bit-identical 1025/1025 | **falsified** |

Source: `eval/LOCOMO_LEVERS.md` (generator `examples/locomo_levers.rs`,
artifact `eval/results_locomo_levers.json`), registered in
`docs/handoff/CLOSED.md` as closed.

**And the proposals' proposed fix has a second, worse problem.** Both sketch
`RRF_lexical + w_tier · TierPrior + …`. RRF terms are `1/(60+rank)` ≈ 0.005–0.016.
An additive structural constant of even 1.0 therefore swamps the entire lexical
signal on non-preference queries — which is exactly what `docs/handoff/RND_PLAN.md`
and the third proposal's own red-team caught. A structural prior must enter
**as a rank voter inside the RRF**, or not at all.

**Also [verified]:** the third proposal's core claim about the benchmark is
right, and it is the one thing that would have wasted the most time.
LongMemEval-S runs *per-question fresh index, session-as-document,
question-text query*. There is no pre-existing `HotIndex` and no cross-session
`global` tier during scoring. Therefore:

> A `tier` column and a `HotIndex` table move LongMemEval-S
> `single-session-preference` R@1 by **0.0pp**. They cannot move it: the
> benchmark has no such rows.

Only changes to **what `retain` writes** and **what `recall` ranks on the
session-as-document corpus** can move that number. Everything else in the
hierarchy is real product value with **zero** effect on the one metric the
proposals promise to lift.

### 4. What the proposals get right, and it is a lot

- **The 43.3% / +16.2pp ordering gap is the largest single number in the
  system** and is correctly identified as the target (`README.md:62-76`).
- **Uniform-default trap** is real: adding `kind`/`aspect` columns that
  `retain` never populates is dormant config, which this repo already has a
  policy against (`RND_PLAN.md` H11 was closed *because* the counter would
  read 1 everywhere).
- **`operant` ↔ `memory-wire` boundary** (third proposal) is correctly drawn:
  Dreaming orchestration belongs to the caller; the substrate exposes
  deterministic primitives. This repo has no LLM in the default build and
  must keep it that way.
- **`connect` does not deploy `SKILL.md`** — **[verified]**, `grep -c SKILL`
  returns 0 for `src/connect.rs`, `src/main.rs`, `src/paths.rs`, and no
  `skills/` path appears anywhere in `src/`. This is a real, small, honest gap.
- **Relative-date and contradiction hygiene has no home**: **[verified]**,
  `src/sweep.rs` is TTL-deletion only; no relative-time detection exists.
- **No retroactive redaction**: **[verified]** above.

### 5. Where the proposals contradict this repo's own rules

| proposal says | repo rule |
|---|---|
| "measure `single-session-preference` R@1 target ≥80%" | `AGENTS.md` §6: LongMemEval is **test**, never selection. "Sweep a parameter against the LongMemEval numbers and keep the winner" is forbidden. |
| "hot index ≤200 lines / 25 KiB" | No source. This repo's own measured cost is `DEFAULT_RECALL_BUDGET = 8000` tokens = 3× the 2,626-token median session (`tests/recall_budget.rs`); a 25 KiB index is ~2–3× that budget *before* any recall. |
| "activate the 4 dormant mechanisms" | `CLOSED.md` forbids re-proposing a closed lever without new evidence. |
| "`content_sha256` + `epoch` CAS" | Not present, not measured, and the writes it would protect are **already serialized** by one writer mutex + WAL. |
| "retire redundant tools among the 54" | There are 4. |

---

## Part II — Findings, ranked `ponytail` order

Ranked **Correctness → Risk/Security → Concurrency → Tested → Fast → Lean**.

### F1 [CORRECTNESS] Three of four "unbuilt" Pillars are already shipped

**Evidence:** §2 above. WAL, busy_timeout, loopback bind + exposure warning,
spawn_blocking, WAL read pool, summary capsules, in-band dates, per-bank
capsules.

**Consequence:** a plan that begins with Batch 1 ("Concurrency, Panic
Telemetry & Auth") would re-derive v0.7.0/v0.8.0. The real frontier is the
**dimensional schema** (nothing exists) and the **serving shape** (dates +
capsules already proven; the surface-level adoption is a small patch).

### F2 [CORRECTNESS] The "activate the dormant 4" lever is closed with measurements

**Evidence:** `eval/LOCOMO_LEVERS.md` §Gates; `CLOSED.md`. All four failed
pre-registered gates on n=1,531.

**Consequence:** any plan item that says "activate sentence-level overlap" or
"activate the recency stream" is **re-proposing a rejected lever**. It needs
new evidence *stated before spending anything* (AGENTS §8), and the only new
evidence that would qualify is a **different mechanism** (e.g. a sub-document
index that changes what is ranked), not a re-run of the same weights.

### F3 [CORRECTNESS] The proposals' benchmark diagnosis is right, so the metric promise is wrong

**Evidence:** README methodology line + `examples/longmemeval.rs` shape; §3 above.

**Consequence:** **no hierarchy feature can lift `single-session-preference`
R@1.** The only levers that can touch that number on this instrument are:
(a) what `retain` writes, (b) how `recall` ranks a session-as-document, (c)
the serving budget/shape. Any plan whose Phase 2 gate is "R@1 43.3% → 80%" is
promising something its own design cannot deliver.

### F4 [CORRECTNESS] Additive structural priors over RRF are algebraically wrong

**Evidence:** RRF score magnitudes (own derivation, consistent with
`src/recall.rs` docs and `eval/SELECTION.md`).

**Consequence:** a tier/kind/trust prior must be a **rank voter inside the
fusion**, bounded like `bm25_magnitude`'s `u ∈ [0,1]` formulation, or an
intent-gated margin. Never a raw constant added to `1/(k+rank)`.

### F5 [CORRECTNESS] "54 MCP tools" is wrong by 13×, and the plan it seeded is a non-problem

**Evidence:** §1.

**Consequence:** the tool-surface work item collapses from "audit and prune 54
tools" to "**extend 4 schemas**; decide whether the new model needs a 5th".
This is a one-day item, not a pillar.

### F6 [RISK] Retroactive redaction is genuinely absent

**Evidence:** `capture::redact_pii` is write-time only; no `memory_versions`
table; `delete_memory` is the only removal path.

**Consequence:** a leaked secret discovered post-ingest cannot be scrubbed
from history without destroying the row. Real gap. Also: no test pins the
redactor against code/UUID false positives (`capture.rs` tests cover
OpenAI/GitHub/AWS/Telegram/JWT/PEM/bearer only).

### F7 [RISK] No mount-level read-only boundary

**Evidence:** bank isolation is by name; there is no `read_only` concept in
`src/`.

**Consequence:** an agent ingesting untrusted web content writes into the same
bank it later reads as trusted rules. The proposals' "prompt-injection
firewall via access segregation" is a **real** gap. Note it is cheap:
`BankConfig` already exists as a per-bank JSON object (`recallMaxTokens`,
`retainTags`) — a `read_only` flag there is a small, testable addition.

### F8 [RISK] No relative-date / contradiction hygiene

**Evidence:** §4. `src/sweep.rs` is TTL-only.

**Consequence:** "what did we decide last week" degrades silently; no
detector exists. This is the one hygiene capability that is *pure Rust, no LLM,
no new dependency*, and therefore fits the substrate boundary exactly.

### F9 [TESTED] One pre-existing test failure, environmental

**Evidence:** `connect::tests::golden_install_shapes` fails on the pristine
tree with `CopilotCli: unexpected outcome Skipped("not detected on this
machine")`. The golden test calls `run()`, which uses the **real** `PATH`
probe (`binary_on_path`), and `copilot` is not installed on this machine.

**Consequence:** the test is **not hermetic** — it contradicts its own
`no_path` helper's stated intent. This is a **real bug in the test**, worth
fixing (inject `no_path`), and it means "the gate is green" is currently
**false on this host**. Any upgrade plan that says "gated by `cargo test`" must
say "gated by `cargo test`, which is currently red for an environmental
reason".

### F10 [LEAN] The real cost axis is the 8,000-token budget, not the tool count

**Evidence:** `tests/recall_budget.rs`, `README.md:142-170`, H12 +12% context
growth. The measured wins this project has ever had on *quality* were serving
shape (dates, +20.0pp) and budget (40.0→60.0%).

**Consequence:** a startup `HotIndex` must be **subtracted from** the serving
budget, not added to it. A 4 KiB index beside an 8,000-token recall is a
~50% overhead on the dominant cost; that is why the plan caps it at ~1,000
tokens and makes it opt-in.

---

## Part III — What is genuinely unbuilt (the real frontier)

Six things survive the audit. Each is small, deterministic, and fits the
"substrate, not agent" boundary.

| # | capability | why it is not already here | size |
|---|---|---|---|
| 1 | **Dimensional metadata**: `scope` / `aspect` / `lifecycle` / `trust` on a memory | never existed | schema + migration + `retain`/`recall` params |
| 2 | **Mount profiles**: per-bank `read_only`, per-call trust ceiling | never existed; `BankConfig` is the natural home | one config field + two guards |
| 3 | **Retroactive redaction**: `redact(id, pattern)` across versions | no version table | new table + one op |
| 4 | **Hygiene scan**: duplicate/contradiction/relative-date/bloat candidates, pure Rust, cursor-based | `sweep.rs` is TTL-only | one new module + one MCP verb |
| 5 | **Startup grounding capsule**: bounded index of active rules, opt-in, TOON/compact | never existed; capsules exist per-*memory* not per-*bank* | new read path |
| 6 | **`connect` deploys `SKILL.md`** (`--with-skills`) | `connect` writes hooks/MCP only | one flag + one template copy |

Plus one **test bug** (F9) and one **redactor test gap** (F6).

Everything else in the three proposals is either **already shipped** (F1), or
**closed by measurement** (F2), or **cannot do what it promises** (F3).

---

## Part IV — Method note for the next agent

If you re-run this audit, the commands that produced it are:

```bash
git rev-parse HEAD; git status --short
wc -l src/*.rs | sort -rn
rg -n 'RETAIN|RECALL|REFLECT|CONFIG_GET' src/mcp.rs | head
rg -n 'journal_mode|busy_timeout|synchronous' src/store.rs
rg -n 'DEFAULT_ADDR|is_loopback' src/main.rs src/paths.rs
rg -n 'pub const SHIPPED' -A 25 src/recall.rs
sed -n '1,30p' eval/LOCOMO_LEVERS.md
grep -c SKILL src/connect.rs
PATH=/usr/bin:$PATH cargo test --locked          # note F9
```

And the three priors to distrust: **the README's top half is marketing
(8.8 MiB, 54 tools-adjacent claims)**; **the proposals are written from it,
not from the code**; and **"inert" in `docs/PERFORMANCE_PLAN.md` means
"measured and closed"**, not "waiting to be switched on".