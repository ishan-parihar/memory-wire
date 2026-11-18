# Verification pass — round 1 (2026-11-18)

Mandatory pre-adoption pass. A reviewer's finding is a **hypothesis about the
artefacts**, never a verdict. Each row below is verified against the tree at
`36bb947` (v0.10.0) by reading the cited code, not by re-reading the review.

Verdicts are kept strictly separate: **their finding, verified** /
**their finding, refuted** / **my finding, prompted by theirs** / **unknown**.

---

## 1. The omission (D-ASK) — **their finding, verified on substance; refuted on one premise**

### 1a. "hooks.rs still discards the score into `Vec<String>`" — **REFUTED**

The review asserts `recall_at` deserialises `Vec<String>` and drops the fused
RRF score "one line later", citing `src/hooks.rs:333-362`. That code has moved
and the premise no longer holds at HEAD.

**Verified at `36bb947`:** `src/hooks.rs:660` `recall_at` returns
`Option<Vec<ScoredHit>>`; `ScoredHit` (`:496-507`) carries
`content`, `created_at` and `score: Option<f64>`. `recall_at_fmt` (`:583`)
requests `format` explicitly so the score is on the wire, and
`the_score_should_survive_the_client_boundary` (`:1497`) pins it.

The review's own text half-saw this: it wrote "whether `hooks.rs` carries the
2026-10-08 unverified header at 36bb947 is not in evidence" and "whether
`recall_at` still deserializes `Vec<String>` … is not in evidence", then
proceeded on the assumption that it does. **That assumption is false**, and the
pack caused it — see §5.

### 1b. "The score is not gated on" — **their finding, verified**

`recall_section` (`:680-706`) reads `hit.created_at` and `hit.content` and
**never reads `hit.score`**: `grep -c score` over that function body returns 0.
The only consumer of `score` is a `tracing::debug!` (`:640`) whose own comment
says "Nothing branches on this and nothing is gated by it".

So the **substance** of D-ASK survives intact: the scale that would separate a
confident hit from a best-of-a-bad-set is now *available* at the seam and still
**not used**. What changed since 2026-09-30 is that the evidence-gathering half
landed; the policy half did not.

### 1c. "The frozen battery lifted the blocker" — **their finding, verified**

D-ASK's stated blocker is "labelled (query, relevant?) pairs over the real
bank". `TODO.md` row 6 (done 2026-10-16) records the battery frozen at 21
cases and states "Precision levers **now registrable**". So the reviewer is
right that the blocker named in the doc has been lifted, and right that no
lever was registered.

### 1d. "The 2026-10-08 provenance label is missing" — **REFUTED**

The review says the `(recalled by relevance from bank <id>, unverified)`
header's presence at v0.10.0 is "not in evidence". It **is present**:
`src/hooks.rs:682` emits `"\n### memory-wire recall — `{bank}`\n(recalled by
relevance, unverified)\n\n"`, and four tests assert it (`:916`, `:1207`,
`:1444`, `:1544`, `:1623`).

**Net:** D-ASK is a real, open, unfixed defect on the per-turn injection path,
and W0–W6 does not list it. The reviewer's conclusion stands; one of its three
supporting claims does not.

---

## 2. The ranking — **their finding, partly verified; the reordering is a judgement, not a fact**

The review reorders to **W4 > W5 > W1 > W6 > W2 > W3 > W0**. That is a
reasoned argument from named anchors, not a measurement, and I verify it as
such rather than adopting it.

### 2a. "W4 is mis-ranked downward" — **their finding, verified as a mechanism claim**

The argument is: H3 (recency) is falsified, so contradictions and relative
dates stay equally rankable; `memory_retain` dedups only exact digests
(**verified**: `idx_memories_bank_hash` on a SHA-256 `content_hash`,
`src/store.rs:1184-1212` — "deliberately NOT unique … the store refuses
duplicates on its own"), so near-duplicate prose consumes the finite 8,000-token
budget. This is a **correct** reading of the code.

### 2b. "W6 is mis-ranked downward" — **their finding, verified**

`grep -c SKILL src/connect.rs` = 0 (verified earlier this session, recorded in
the surface audit F-series). `connect` wires 5 hooks and 4 MCP tools and
deploys no skill file. The "uniform-default trap" consequence is a real
mechanism.

### 2c. "W2 is mis-ranked upward for routine capability" — **their finding, verified as stated**

W2's value is conditional on untrusted ingest. In a single-repo loop with no
web scraping it adds refusals, not context quality. Fair.

### 2d. What I do **not** adopt

The exact ordering `W4 > W5 > W1 > …` is a **product judgement**, and adopting
it would contradict the plan's own dependency logic: W5 reads W1's columns, and
W4's detectors group by `(scope, aspect)` — W1's types. The honest reading is
narrower and I take only that:

> **W1 is a foundation, not a peer.** Ranking it 3rd among 7 treats a schema
> migration as if it were a feature with standalone value. Its value is
> entirely realised through W4 and W5. That is an argument about *how to
> present* the plan, not about reordering the work.

---

## 3. What W0–W6 structurally cannot move — **their finding, verified, and the sharpest thing in the review**

Verified claim: because every gate in the plan preserves
`FusionWeights::SHIPPED`, keeps unfiltered recall byte-identical, and leaves
the ranking path untouched, **no LongMemEval-S and no LoCoMo metric can move**.

- LongMemEval-S: W1 adds no unfiltered effect (Gate 2); W2 is a write guard;
  W5 has no capsule rows on a per-question fresh index; W4 never runs during a
  benchmark recall (Gate 5).
- LoCoMo: unfiltered queries + unchanged weights + no SessionStart capsule.

The review names the exceptions correctly: **PrecisionMemBench** (W1 filters,
if the adapter populates and queries them) and the **relevance battery** (W1 or
W4 apply). And it names the cost correctly: all four items can move footprint
and latency **negatively**.

**This is the single most useful finding of the round**, and it is a direct
consequence of a plan I wrote to be safe. It is also **consistent with the
plan's own stated thesis** (ranking is near its ceiling; the loss is in serving
shape) — which means the plan is honest about its own reach, and the review has
restated that honesty as a criticism. I record it as verified and **adopted as
a documentation defect**: the plan never says plainly that it cannot move the
headline metric.

---

## 4. The agentic-experience gap — **their finding, verified**

Concrete and correct on every point I checked:

- **SessionStart, flag off:** nothing from W5 arrives; the agent gets the
  existing preamble and a generic startup recall. Correct — W5 is opt-in and
  defaults off.
- **SessionStart, flag on:** W5's capsule is carved out of the 8,000-token
  budget, so the startup recall is correspondingly smaller. Correct per Gate 6.
- **Every turn:** nothing in W0–W6 repairs the per-turn path (§1b).
- **Mid-task MCP:** `memory_retain` can only append a competing row; there is no
  supersede/retract tool. **Verified**: the MCP surface is 4 tools
  (`RETAIN`/`RECALL`/`REFLECT`/`CONFIG_GET`), no delete/redact verb is exposed,
  and `delete_memory` is HTTP-only.
- **Stop / PreCompact / SessionEnd:** `FLUSH_CHARS = 2000` (`src/hooks.rs:92`)
  retains up to 2,000 chars of prose per flush, and `stop_at` retains a marker
  row. **Verified.**

The one item I'd add: the review says the hook block carries no W1
scope/trust/lifecycle badge. Correct today, and W1 as scoped would not change
that — `recall_section` renders content and date only.

---

## 5. The pack omission — **their finding, verified, and it is MY defect**

The review's strongest process finding: the pack omitted
`docs/plans/…upgrade.md` **lines 82–331** (the entire W0–W6 body), carrying
only lines 11–57, 58–81 and 332–358.

**Verified:** `grep '^# FILE: docs/plans' PACK-AS-SENT.md` shows exactly three
slices — (11,57), (58,81), (332,358). The plan is 386 lines. The W0–W6 item
specifications were never in front of the reviewer.

**This is my error, and it is the protocol's own failure mode.** I sliced the
plan to "invariants + shape + gates" on a byte budget, and thereby asked the
reviewer to judge a plan without showing it the plan. The review says so
plainly and correctly: "every question turns on *not in evidence* details".

It also explains §1a. The reviewer could not see the plan's items, so it fell
back on D-ASK's own stale description of `hooks.rs` — a doc written
2026-09-30, before the score was plumbed. **A pack that omits the artefact
under review converts the reviewer's gaps into confident claims about a tree it
cannot see.** That is the exact failure the protocol warns about, arriving
through a slice I chose.

**Adopted:** any future round on this plan packs the plan **in full**.

---

## 6. The web-sourced findings — **verified as citations, not adopted as evidence**

Four citations, two distinct sources:

- `https://arxiv.org/abs/2605.06527` — "STALE: Can LLM Agents Know When Their
  Memories Are No Longer Valid?" — cited for: invalidated memories left
  retrievable degrade accuracy (55.2% overall, n=1,200 queries / 400 conflict
  scenarios).
- `https://aaif.io/blog/agent-memory-patterns-tradeoffs-open-problems` —
  "Agent memory: patterns, tradeoffs, open problems" — cited for: separate
  always-loaded scoped instructions from episodic corpora; sleeper memory
  poisoning; explicit capture policy.

Per the venue rules these are admissible **only with URL + read date**, which
they carry (`read 2026-11-18`). I have **not** independently verified the 55.2%
figure and do not adopt it; it is a pointer worth reading, not a measurement of
mine. Recorded as *their finding from a cited source*.

---

## Verdict summary

| # | finding | verdict |
|---|---|---|
| 1a | `hooks.rs` drops the score into `Vec<String>` | **refuted** — `ScoredHit` carries `score` since before v0.10.0 |
| 1b | the score is never gated on | **verified** — `recall_section` reads content + date only |
| 1c | the frozen battery lifted D-ASK's blocker | **verified** — TODO #6, 21 cases, "registrable" |
| 1d | the provenance label is missing at HEAD | **refuted** — present at `src/hooks.rs:682` |
| 2 | W4/W6 mis-ranked downward; W2 mis-ranked upward | **verified as mechanism**; exact order **not adopted** (a judgement) |
| 3 | W0–W6 cannot move any LME/LoCoMo metric | **verified** — and **adopted** as a documentation defect |
| 4 | the per-turn path is unrepaired; no supersede tool | **verified** |
| 5 | the pack omitted the plan's W0–W6 body | **verified — my defect, adopted** |
| 6 | two web citations | **recorded as cited, not adopted** |

**Net effect on the plan:** one new item (a supersede/retract verb, or an
explicit statement that the 4-tool surface is final), one amended gate (say
plainly that the plan moves no headline retrieval metric), one process fix (pack
the plan whole), and one correction to record — D-ASK's description of
`hooks.rs` is stale and should be updated in place rather than left to mislead
the next reader.