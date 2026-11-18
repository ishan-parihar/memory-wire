# Verification pass — round 2 (2026-11-18)

Mandatory pre-adoption pass. A reviewer's finding is a **hypothesis about the
artefacts**, never a verdict. Each row below is verified against the tree at
`555d1aa` by reading the cited code, not by re-reading the review.

Verdicts are kept strictly separate: **their finding, verified** /
**their finding, refuted** / **my finding, prompted by theirs** / **unknown**.

A web-sourced finding is verified **at the source**, with the URL, and filed with
the date read. Two were cited; both were opened.

---

## 0. What the round changed, in one line

The reviewer's headline is **adopted**: the per-turn hook path injects
best-of-bad-set recall with no relevance and no trust gate, and that is the
highest-value thing left. Two of its supporting claims are **refuted** by the
tree. One of its claims is **true but imprecise** in a way that matters. And it
found **three bookkeeping defects I introduced** that are entirely real.

---

## 1. Bottlenecks

### 1a. "Every prompt turn spawns a process and injects up to the 8,000-token budget un-gated" — **their finding, verified**

`src/hooks.rs:234` `if prompt.chars().count() <= SHORT_PROMPT_CHARS` then
`recall_at(...)`, and `:243` `Some(hits) if !hits.is_empty() => recall_section(bank, &hits)`.
`recall_section` (`:680-706`) reads `hit.content` and `hit.created_at` and
**never reads `hit.score`** — `rg -c score` over that function body is 0. So
every non-empty recall injects, regardless of relevance. Verified.

### 1b. "Storage/retrieval compute is already sub-30 ms, so compute paths are not the bottleneck" — **their finding, verified as a reading of the pack, with a caveat**

The pack carries `7.2 ms p50 at 10k synthetic` and `26.5 ms end-to-end LoCoMo`
as `[PRIOR]`/`[UNCONFIRMED]` figures at load 11–28. The reviewer labelled them
correctly. **My finding, prompted by theirs:** neither number is a 555d1aa
measurement, and AGENTS §7 says latency is only meaningful on a quiet box. The
honest statement is "unmeasured at HEAD", not "sub-30 ms".

### 1c. "The binary breaches Gate 8's ≤ 9 MiB cap" — **their finding, verified**

Plan §4 line 412: `binary ≤ 9 MiB (from 8.8)`. Measured:
`target/release/memory-wire` = **10,937,128 B (10.43 MiB)**, `DT_NEEDED` 3.
The gate is breached and nothing in the tree says so. **Adopted.**

---

## 2. Redundancies

### 2a. "The TS plugins and `src/hooks.rs` have diverged" — **their finding, REFUTED on the one concrete claim they named**

The review says the `<160`-char mitigation "shipped in the two TS plugins, not
in `src/hooks.rs`".

**Refuted.** `src/hooks.rs:49` `const SHORT_PROMPT_CHARS: usize = 160;`, used
at `:234`, with a test at `:1425`. The TS plugins carry the same constant
(`extension/memory-wire.ts:81`, `opencode-plugin/memory-wire.ts:70`). All three
implement it.

**What is true** is the weaker claim: the TS plugins and the Rust hook are two
implementations of the same policy, so a future change to one can miss the
others. That is a duplication risk, not a present divergence, and I record it as
such rather than adopting the stronger version.

### 2b. "1,142 LOC of disabled `vector.rs` + `embed.rs` is dead weight" — **their finding, true on LOC, imprecise on weight**

`src/vector.rs` (965) + `src/embed.rs` (177) = 1,142 source lines. But
`src/lib.rs:50-51` gates the module `#[cfg(feature = "embed")]`, and
`Cargo.toml:88` `default = []`. So the shipped binary contains **zero bytes** of
it. It is dead *source*, not dead *binary*.

**My finding, prompted by theirs:** the LOC figure is a real number about the
authored surface and belongs in any honest accounting. Calling it "dead weight"
in a round about a *binary's* footprint invites a reader to think it costs
bytes. It does not.

### 2c. "`--guidelines` and `--with-skills` are two parallel static-guidance mechanisms" — **their finding, verified**

`src/guidelines.rs` (251 LOC) and `src/skills.rs` (278 LOC) are separate
installers for the same purpose, selected by two different flags on the same
command (`src/main.rs:131,137`). Verified. Whether they should merge is a
judgement; the duplication is a fact.

### 2d. Explicit rejections the reviewer got right — **their finding, verified**

`hygiene.rs` (645), `grounding.rs` (349) and `sweep.rs` (276) do not duplicate
one another: `sweep` hard-deletes by TTL, `hygiene` detects candidates
non-destructively, `grounding` formats a capsule. And `mcp.rs` (stdio) vs
`mcp_http.rs` is host-required (Codex needs a URL). I verified each and agree.

---

## 3. Fragmentations

### 3a. "One service, three doors is contradicted: capabilities are siloed" — **their finding, verified, and the sharpest structural finding of the round**

Verified per capability:

| capability | HTTP | MCP | CLI | hook |
|---|---|---|---|---|
| redact (W3) | **no route** | **no tool** | **YES** (`Cmd::Redact`, main.rs:242) | write-time only |
| grounding (W5) | **no route** | **no tool** | **YES** (`Cmd::Grounding`, main.rs:263) | SessionStart, default off |
| hygiene (W4) | **YES** (`/banks/:id/hygiene`) | **no tool** | **no** | no |
| recall | yes | `memory_recall` | **no** | yes |
| supersede (W7a) | **no route** | **YES** (5th tool) | **no** | no |

The architecture doc's "one service, three doors" is a claim about
`MemoryService`, and it is **true** — there is one implementation. What is false
is the implied symmetry: **every door has a different subset**. Verified, and
adopted.

### 3b. "Plan W4 specified `memory_hygiene` as the 5th tool; shipped has `memory_supersede` and no hygiene tool" — **their finding, verified**

Plan line 243: `One MCP tool memory_hygiene (a 5th tool, justified: ...)`. The
shipped 5th tool is `memory_supersede`; hygiene went to HTTP. Verified. The
plan's surface and the shipped surface diverged, and nothing records the change.

### 3c. "6 MCP tool constants vs README's 5 vs its table's 4" — **their finding, true but imprecise; the real defect is mine**

`src/mcp.rs` has 6 `memory_*` constants (`RETAIN`, `RECALL`, `REFLECT`,
`CONFIG_GET`, `SUPERSEDE`, `HISTORIAN`). But `tools()` registers **5** —
`HISTORIAN` is a *prompt*, not a tool (`:105`). So:

- "6 constants" — **true**
- README's "five tools" — **correct**
- README's tool table listing 4 (missing `memory_supersede`) — **the actual defect**

The reviewer wrote "6 tool constants while README claims 5", which reads as
though the README overcounts. It undercounts its table. **Adopted with the
correction:** fix the table, and say in `mcp.rs` that `HISTORIAN` is a prompt
constant so the next counter does not re-add it.

---

## 4. Integration gaps toward an optimal agentic experience

### 4a. "No read-time trust gate: W2 only downgrades on write" — **their finding, verified**

`recall_section` reads content and date only. No `trust` predicate exists on
the read path. A row written as `untrusted_external` (which W2 permits — it
only prevents `scope=policy`) is injected into context verbatim. Verified, and
it is the strongest part of the round.

### 4b. "Hooks print empty framing indistinguishable from an empty bank" — **their finding, verified as a behaviour, refuted as an AGENTS §10 violation**

`prompt_at` returns `String::new()` when the endpoint is unreachable
(`src/hooks.rs:243-245`, `_ => String::new()`). So a down server and an empty
bank both produce empty output.

But AGENTS §10's rule is about *output that claims to be an answer*:
"a list endpoint states the total; an empty result says so definitively rather
than returning a bare `[]`". An empty string is not a bare `[]` claiming
success — it is the never-fail hook contract (AGENTS §9), which *requires*
silence when the server is down. The two rules are in tension, and the hook
correctly follows §9.

**My finding, prompted by theirs:** the real defect is not a §10 violation, it
is **undiagnosability** — the agent cannot tell "no memories matched" from "the
server is down", and neither can the user. That is worth fixing (e.g. a
one-line stderr note on the failure path, which the never-fail contract
permits because it never reaches the model), and it is not what the reviewer
said.

### 4c. "Short-prompt blindness on CLI hooks" — **their finding, REFUTED**

Covered in 2a. `SHORT_PROMPT_CHARS` is implemented in `src/hooks.rs:234`.

### 4d. "TODO rows 21–29 are stale-open despite the plan marking W0–W7 shipped" — **their finding, verified, and it is mine**

Plan §6: `**All items complete.**`. `TODO.md` rows 21–29: all still `open`.
Verified. Every one of those nine rows is a completed item. **Adopted** — this
is a documentation defect I introduced by not closing the register when the
batches landed.

### 4e. "README lists 9 CLI subcommands; the tree has more" — **their finding, verified, and it is mine**

README's CLI section documents 9 (`info`, `daemon`, `serve`, `connect`, `hook`,
`doctor`, `mcp`, `sweep`, `seed`). `src/main.rs` has 13 variants: those 9 plus
`update`, `redact`, `grounding`, and `embed` (feature-gated). The README even
says "Nine subcommands in a default build, exactly as `--help` reports them" —
which is **false at HEAD**: `--help` reports 12 in a default build.

**Adopted.** I added `redact` and `grounding` in Batch 2 and never touched the
README. The claim "exactly as `--help` reports them" makes it worse than a
missing row: it asserts a verification that does not hold.

---

## 5. The recommendation

### 5a. "Ship a read-time relevance + trust gate on the per-turn hook path" — **their finding, adopted as the highest-value next move**

The argument is sound and the anchors are real:
- `docs/OPEN_HOOK_RECALL_RELEVANCE.md` records ~10,000 chars of live irrelevant
  injection, and the defect was diagnosed 2026-09-30 and never fixed.
- The 8,000-token budget is the dominant cost axis (surface-audit F10).
- Every closed lever in `CLOSED.md` was closed on *ranking*; none of them
  addressed *whether to inject at all*.

### 5b. "The cheapest mistake would be a 3-line score cut on `ScoredHit.score`" — **their finding, verified**

`docs/OPEN_HOOK_RECALL_RELEVANCE.md` and the review both note BM25 is min-max
normalised per query, so the top hit of an entirely irrelevant set scores 1.0.
`rg -n "min.max\|normal" src/recall.rs` confirms the normalisation. A raw
threshold on that score is mathematically broken. Verified, and this matches
AGENTS §8's standing veto on a raw score cut.

### 5c. What would make the opposite choice right — **their finding, verified as reasoning**

They name two falsifiable conditions (off-topic turns rarely over-match; agents
never retain untrusted content) and cite evidence against both. That is the
right shape for a decision record.

---

## 6. The web-sourced findings — **both verified at the source**

| citation | reviewer's claim | verified |
|---|---|---|
| `https://doi.org/10.48550/arXiv.2609.13889` (Huang et al., PMPA) | Claude Code 66.9% ISR / 81.7% C-ASR; prompt-level defence gives limited protection once poisoned | **exact match** on the arXiv abstract, read 2026-11-18 |
| `https://api.semanticscholar.org/CorpusID:267312595` (CRAG) | a retrieval-confidence evaluator between retrieval and generation, rejecting poor retrievals instead of trusting top-k | **verified** — arXiv 2401.15884, "lightweight retrieval evaluator … confidence degree" |

Both are admissible per the venue rules (URL + read date). The PMPA figures are
load-bearing for 4a and I adopt them as a *pointer*, not as my measurement.

---

## Verdict summary

| # | finding | verdict |
|---|---|---|
| 1a | per-turn injection is un-gated on relevance | **verified** — `recall_section` never reads `score` |
| 1c | binary breaches Gate 8's ≤ 9 MiB | **verified** — 10.43 MiB vs ≤ 9 |
| 2a | TS plugins and hooks.rs have diverged on `<160` | **refuted** — implemented in all three |
| 2b | 1,142 LOC dead vector arm | **true on LOC, imprecise on weight** — compiles to zero bytes |
| 2c | two parallel guidance installers | **verified** |
| 3a | door capability asymmetry | **verified** — the structural finding |
| 3b | plan promised `memory_hygiene`, shipped `memory_supersede` | **verified** |
| 3c | 6 constants vs README 5 vs table 4 | **true but imprecise** — table is the defect |
| 4a | no read-time trust gate | **verified** |
| 4b | empty framing violates AGENTS §10 | **refuted as stated** — real defect is undiagnosability |
| 4c | short-prompt blindness on CLI hooks | **refuted** — `SHORT_PROMPT_CHARS` at `:234` |
| 4d | TODO rows 21–29 stale-open | **verified — my defect** |
| 4e | README CLI count wrong | **verified — my defect** |
| 5a | next move = read-time relevance + trust gate | **adopted** |
| 5b | a raw score cut would be the cheap mistake | **verified** — min-max normalisation |
| 6 | two web citations | **both verified at the source** |

**Net effect:** one adopted direction (5a), four documentation/bookkeeping
defects to fix (1c, 3b, 3c, 4d, 4e), two refutations that must not be acted on
(2a, 4c), and one reframing (4b).