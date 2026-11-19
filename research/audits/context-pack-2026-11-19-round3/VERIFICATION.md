# Verification — round 3, hydrogen review, 2026-11-19

Every finding in `RESPONSE.md`, checked against the artefact it cites. Three
verdicts, kept separate: **their finding, verified**; **my finding, prompted by
theirs**; **unknown / not in evidence**. A reviewer's finding is a hypothesis
about the artefacts, never a verdict.

## Their findings

### 1. "Ordering is largely solved" is overstated — **VERIFIED, and it is the most important finding in the round**

They wrote that it "holds on whole-session, tiny-bank QA benchmarks and breaks
down the moment the corpus resembles production granularity or bank size."

Checked at source, and the figures they cite are real:

| claim | artefact | verdict |
|---|---|---|
| LongMemEval pool 500/500 | `AGENTS.md:52`, `CLOSED.md` | **verified**, but see the caveat below |
| LoCoMo 19–32 session banks | `STATE.md:230`, `RND_PLAN.md:51` | **verified** — 1,531 queries, and `H1.0` records the fused list equals the whole bank on **261/261** misses. On a 19–32 document bank, "100% pool coverage" means the pool is the entire bank |
| BEAM-100k 39.9% R@1 | `eval/BEAM.md`, `CONSISTENCY.md` §47 | **verified** (27–50 doc banks, 313/400 resolved) |
| LoCoMo Turns R@1 54.6% (−5.8pp) | `eval/LOCOMO_TURNS.md` | **verified** |
| PMB 19/77 precision 0.24 | `TODO.md` row 7, `CONSISTENCY.md` §37 | **verified** |

**My reading, prompted by theirs.** The pack asked "is the 500/500 claim an
artefact of the instrument?" and the answer is **yes, in a specific way I had
not stated**: `500/500` counts LongMemEval's 500 questions *including* its 30
`_abs` abstention cases, whose gold is by construction absent (the benchmark
scores 470 for that reason; `TODO.md` row 8). Quoting "500 of 500" as pool
coverage therefore counts 30 unanswerable questions as having gold in-pool. The
underlying coverage claim is still strong — the 470 that *are* answerable all
have gold in the pool — but the number as written overstates its own
population. **This is my defect, in `AGENTS.md` §1 and in the pack.**

### 2. The plural-miss decomposition is an ordering problem — **VERIFIED, with the arithmetic checked**

They decomposed the 1,531 queries and concluded `recall_all@10`'s gap is
architectural, not a ranking failure. I recomputed every step against
`RND_PLAN.md` §1's own table:

- multi-hop misses: 13/58 + 10/14 + 6/7 + 10/10 = **39/89** (43.8%)
- single-hop ≥2 gold: 75/163 + 40/56 + 43/49 = **158/268** (59.0%)
- combined plural: (39+158) / (89+268) = 197/357 — they wrote **184/299**

**Discrepancy, and it does not rescue the claim.** They used a narrower
"plural" population (299) than the table's own ≥2-gold rows sum to (357). The
direction of their conclusion is unaffected — plural queries dominate the
misses either way — but the two denominators are not the same set, and they
present 299 as if it were the table's sum. Under their own arithmetic
184/261 = 70.5%; under the table's, 197/261 = 75.5%. **Either way the finding
holds; the figure is imprecise.**

They also wrote `>=4 gold: 53/59 = 89.8%`. From the table that is
(10+43)/(10+49) = 53/59. **Verified.**

And the load-bearing one: "99/261 = 37.9% of worst-ranked gold sessions sit at
rank 21+". That is `H1.0`'s own finding (`RND_PLAN.md` §86ff). **Verified** —
and it is the strongest sentence in the review, because it says no reordering
inside k=10 can reach those.

### 3. The trust gate is closed in Rust and hermes but OPEN in the two TypeScript plugins — **VERIFIED, and this is a hole I left**

This is the finding I would have missed, and it is the one that matters most.

`B-DELTA` D1 records the gate as shipped in `src/hooks.rs` and
`plugin/integrations/hermes/memory-wire/__init__.py`. The reviewer noted two
other in-repo auto-injecting plugins and asked whether they carry it. Checked:

```
plugin/integrations/extension/memory-wire.ts        — before_agent_start, no `trust`
plugin/integrations/opencode-plugin/memory-wire.ts  — chat.system.transform, no `trust`
```

Neither file contains the string `trust` at all. The opencode plugin even
documents that it accepts `{id, score, content}` and drops anything else — so a
row marked `untrusted_external` is injected verbatim into the system prompt.
**The round-2 prompt-injection hole is still live on those two surfaces.**

### 4. `memory_supersede` is a secret-retention trap on MCP — **VERIFIED**

`src/mcp.rs:410` wires `SUPERSEDE` with `annotations(false)`, and
`annotations()` sets `destructive_hint = Some(false)` unconditionally
(`src/mcp.rs:256`). Its own description says "The row stays in the store."
So an MCP agent that retains a credential and then calls `memory_supersede` to
undo it: the row leaves default recall and **the plaintext stays on disk**.

They are right that `redact` is CLI-only (`src/main.rs:462`) and that the CLI
has no `recall` either (verified against `--help`: `info serve daemon connect
hook doctor update mcp sweep seed redact grounding help`).

### 5. `docs/ARCHITECTURE.md`'s "redaction happens on write" contradicts `redact` — **VERIFIED**

`ARCHITECTURE.md:87` says "A secret is not stored, so it cannot be recalled."
But `Cmd::Redact` (`main.rs:462`) exists precisely because secrets *are* stored
— it walks every version row and rewrites them. Write-time redaction catches
known patterns; `redact` is the remediation for what got through. The
architecture doc describes the first and denies the second.

### 6. Several figures are stale or mis-tagged — **VERIFIED, one correction**

| their claim | check | verdict |
|---|---|---|
| C-ASK says "every number in this pack is `[LIVE]`" | `C-ASK.md:58` | **verified — and it is false as written.** Only B-DELTA and SURFACE are `[LIVE]`; STATE/CONSISTENCY carry `[CERT]` snapshots, `overlap 0.25`'s 97.2% is `[IN-SAMPLE]`, and README's ≈94.4% is derived |
| PMB 14/77 superseded by 19/77 | `TODO.md` row 7 | **verified** — 14/77 survives only inside a struck-through "before" clause |
| `10.25 MiB` stale vs 10.63 | `AGENTS.md:44` | **verified** — 10.25 was the v0.7.0 build; HEAD is 10.63 |
| `memory_historian` counted as a tool | `mcp.rs` | **already corrected** in `1f5ee17` and again this round |
| `≈94.4%` out-of-sample has no artefact | `README.md:336` | **partly right.** There is no raw measurement artefact, but the derivation is documented (`EVALUATION_HYGIENE.md` §2.1: 93.0 unfitted + a measured +1.4pp). It is **derived**, not unconfirmed |

### 6a. LongMemEval 500/500 population correction — REFUTED. My own finding, not theirs.

**What I first asserted (in an earlier draft of this file).** AGENTS.md §1's "the
candidate pool contains the gold memory in 500 of 500 LongMemEval questions" counts 30
abstention (`_abs`) questions that have no gold, so the real population is 470 and the
claim is unsupported at 500.

**Checked against the data.** `eval/data/longmemeval_s_cleaned.json`: the 30 `_abs`
questions exist (`0862e8bf_abs` and 29 siblings). Their `question_id` carries the
marker; their `question_type` does not — they read `single-session-user` and similar,
which is exactly why agentmemory's filter misses them. And **0 of the 30 have an empty
`answer_session_ids`.** They are unanswerable-in-principle (the gold answer is that no
answer exists), not gold-less. `eval/RESULTS.md` then reports the split honestly: an
`overall` row at n=500 and an `abstention excluded` row at n=470, with a sentence
naming which one is comparable to upstream and why.

**Verdict.** The 500/500 claim is sound, the published `n` is not overstated, and there
is nothing to correct. Recorded because a plausible-sounding arithmetic objection I
had not checked is precisely what a verification pass exists to catch, and it did not
survive one.

### 7. `measure_surface.sh` omits `plugin/` and `install/` — **VERIFIED**

It counts `src/*.rs` (35,295 LOC) and excludes `plugin/`. The consequence is
the one they name: the two TS plugins are outside the surface count, which is
how a missing gate in them stays invisible. **My defect, in the script I wrote
for this round.**

## My findings, prompted by theirs

- **The pack's own `B-DELTA` D3 is stale** — they said so and they are right,
  but for a reason worth recording: I *fixed* `ARCHITECTURE.md` and rebuilt the
  pack **before** sending, so the pack carried the corrected nine-module /
  four-door text while D3 still described the old one. A delta section that
  describes a fix which has already landed is not a delta, it is history. The
  skill's rule 2 ("never pack history") caught this and I overrode it.
- **The reviewer's Rank 1 is the right rank, and their "not naively" caveat is
  the important half.** Adding `redact` to MCP on an unauthenticated loopback
  server with `destructiveHint: false` would hand a prompt-injection payload a
  remote secret-removal primitive. The order is: Host-header verification
  first, then a *scoped* `memory_redact`, then the hint.

## Unknown / not in evidence

- Whether `install/` and `plugin/` hold other auto-injecting surfaces. I
  verified the two `.ts` files the reviewer named and no others; a full sweep
  is outstanding.
- Whether the REST routes enforce rmcp's loopback Host-header guard. The MCP
  path does; the Axum path is a separate check I did not run.
- Whether real production turn-pair capture helps or hurts coding-agent task
  success. No artefact exists in either direction.

## What I am adopting

1. **The TS-plugin trust gate** (their #3) — a real, live hole I closed in two
   files and left open in two others.
2. **The `500/500` population correction** (their #1) — it counts 30
   abstention questions.
3. **The C-ASK overclaim** (their #6) — a false sentence in my own pack.
4. **`AGENTS.md`'s 10.25 MiB** → 10.63 (their #6).
5. **The architecture doc's redaction sentence** (their #5).
6. **`measure_surface.sh` omitting `plugin/`** (their #7).

Not adopting: the restated door-parity list as a Rank-1 *cycle* (it is the
right shape but needs the Host-header decision first), and the ≈94.4% "not in
evidence" verdict (the derivation exists).
