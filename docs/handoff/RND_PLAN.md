# R&D plan — hypotheses, tests, gates

Status 2026-10-14. Each hypothesis has: **Status**, **Mechanism** (the argument, written
*before* any number), **Test**, **Gate**, **Record**, **Cost**. Do them in the order of
§2. A hypothesis is not a licence to sweep: parameter values come from mechanism, and
where a sweep is unavoidable it runs on **LoCoMo only** and is recorded as selection on
the dev set with its fittedness stated (AGENTS §6).

## 0. The measurement setup in one paragraph

Three roles (AGENTS §6). **dev** = LoCoMo (1,531 scorable queries, 272 session-documents
in 10 banks of 19–32) and the relevance battery: all selection. **test** = LongMemEval-S
500 q: measure-only, release acceptance, every run counted in `docs/CONSISTENCY.md`.
**measure-only** = PrecisionMemBench, AMB `rag` runs, everything else external.
LoCoMo retrieval is free and takes ~25 s (`cargo run --release --example locomo`). The
judged LoCoMo run (AMB `rag`) costs ~3,500 free-tier LLM calls and ~5–8 hours; do it
sparingly and never as an inner loop. **The inner loop is the free retrieval harness.**

Metrics, in order of what they predict: `recall_all@10` (every gold session in the top
10 — tracks judged accuracy on LoCoMo: 91.2% / 53.0% / 36.5% for all / some / none gold
served), then R@1 / R@5 / NDCG@10 / MRR, with `recall_any@K` as the legacy scale shared
with LongMemEval. Always report per-category and the per-query up/down counts (the
harness prints them), and always the n.

## 1. What the evidence says about where headroom is

1. **Ordering of single-gold questions is mostly solved** on LongMemEval: first gold at
   rank 1 in 419/500, rank 2–5 in 67 (EXCEED_PLAN §1). The remaining LongMemEval answer
   gap is the *reader*, not retrieval (STATE §2).
2. **On LoCoMo the headroom is coverage of multi-session ("list everything") questions.**
   Shipped config: `recall_all@10` 83.0% vs `recall_any@10` 94.2%. Misses by gold-set
   size (shipped config, n 1,531): multi-hop 1 gold 13/58, 2 gold 10/14, 3 gold 6/7,
   ≥4 gold 10/10; single-hop 2 gold 75/163, 3 gold 40/56, ≥4 gold 43/49; temporal and
   open-domain mostly single-gold with 29/292 and 25/840 misses.
3. **The gold session is always somewhere in the fused list** (`gold_in_pool` 100% on
   the session arm). So a miss is an *ordering* problem inside a 19–32 session bank — but
   for a plural question the several gold sessions each share only part of the query, so
   lexical rank cannot put all of them in the top 10. Whether the missing sessions sit at
   rank 11–15 (reachable by reordering) or at rank 20+ (semantic) is **unmeasured** and is
   the first thing to find out (H1.0).
4. A **small reader drowns in extra context** on LongMemEval (gold alone 6/10, gold + 2
   others 1/10). LoCoMo does not exhibit it. A precision lever therefore has no dev set
   yet (H4).

## 2. Order of work

```
H1.0  rank distribution of the missing gold      (free, ~1 h, decides H1.x)
H0    date-aware dev corpus                      (free, ~1 h, unlocks H3)
H1.x  coverage levers, cheapest first            (free, dev set)
H2    intermediate granularity, one value        (free, dev set, low prior)
H3    recency on a dated dev set                 (free, dev set, low prior)
H4    a dev set that shows reader drowning       (LLM, costly; prerequisite for any precision lever)
H5    answer_quality re-measure with dates       (LLM; test consultation #3; once)
H6    PMB precision via scope tags               (free; measure-only)
H7    housekeeping: concurrency, provider granularity, aroute verification
```

Stop conditions: if H1.0 shows the missing gold is mostly far down the list (rank 20+),
reordering levers (H1a–c) are not worth building and the honest finding is the one in
`docs/EXCEED_PLAN.md` §4 — the deficit is semantic, and the only remaining move is the
dense arm (H1d), whose shipping is the **user's** decision.

---

## H1.0 — Where do the missing gold sessions sit?  *(do this first — done)*

- **Status:** **done 2026-10-14** (`CONSISTENCY.md` §36, artifact `eval/LOCOMO_RANKS.md`, harness
  `examples/locomo_ranks.rs`, per-question `eval/results_locomo_ranks.json`). Answer: 261 misses
  (n 1,531), worst gold at 11–15/16–20/21+/absent = 89/73/99/**0**. No gold is ever absent and the
  fused list is the whole bank on 261/261 misses, so every miss is an ordering position, none a
  matching failure. 62.1% within 11–20 ⇒ the gate fires for **reordering levers (H1a–c)**; the 99
  (37.9%) at 21+ bound what reordering recovers. H1d not reopened. Test consultations: 0.
- **Mechanism:** a lever can only help if the session it must promote is within reach.
  This is the analogue of `docs/PERFORMANCE_PLAN.md` P2 ("can any lexical signal separate
  gold from the rest?"). It selects nothing; it sizes the problem.
- **Test:** in `examples/locomo.rs` (or a new `examples/locomo_ranks.rs` sharing
  `eval/locomo_dev.rs`), for every query with `recall_all@10 = 0` at the shipped config,
  record the rank of each gold session in the fused list (`session_of` over
  `recall_with_weights`). Report: histogram of the *worst-ranked* gold rank (11–15, 16–20,
  21+, absent), split by `meta.category` and by gold-set size. Also report the median
  bank size so "rank 21" is read against 19–32.
- **Gate:** none (descriptive). The output decides which of H1a–d is worth building:
  worst-gold mostly 11–20 ⇒ reordering levers; mostly 21+ or absent ⇒ semantic ⇒ H1d only.
- **Record:** `docs/CONSISTENCY.md` §36 (next free number) + a committed artifact
  `eval/LOCOMO_RANKS.md`. Test consultations: 0.
- **Cost:** free.

## H0 — Make the dev corpus date-aware

- **Status:** **done 2026-10-14** — `Document` gains `timestamp: Option<String>`, ingested via
  `api::parse_created_at` in `eval/locomo_dev.rs` (`Document::created_at`; unparseable corpus
  timestamps fail loudly). 272/272 documents carry RFC 3339 stamps. **Control passed:** at the
  shipped `recency 0.0` the full metric vector is bit-identical — `locomo`, `locomo --turns`,
  and `locomo_ranks` re-run with dated ingestion and diffed against their committed artifacts
  (`eval/LOCOMO.md`, `eval/LOCOMO_TURNS.md`, `eval/LOCOMO_RANKS.md`); every differing line is a
  provenance row (date/load/wall clock/output path), zero metric lines. The dated-corpus note
  is in `eval/LOCOMO.md` provenance (from the generator, then regenerated).
- **Mechanism:** `Corpus::load` ingests every LoCoMo document with `created_at: None`, so
  the store stamps the harness's own clock and the widest spread across a bank is 0.006 s
  (`eval/SELECTION.md` "recency caveat"). Any recency measurement on LoCoMo has therefore
  been a measurement of *ingest order*. `eval/data/locomo/documents.json` carries a real
  per-document `timestamp` (e.g. `2023-05-08T13:56:00+00:00`) that `Document` does not
  deserialize. Reading it is a fix to the instrument, not a retrieval change.
- **Test:** add `timestamp: Option<String>` to `Document` in `eval/locomo_dev.rs`; ingest
  it through the same conversion the product uses (`memory_wire::api::parse_created_at`)
  into `Memory.created_at`. **Control:** with `recency = 0.0` (shipped) the full metric
  vector must be bit-identical to the current `eval/LOCOMO.md`. If it is not, stop — you
  changed ranking.
- **Gate:** the control is identical.
- **Record:** note in `eval/LOCOMO.md` provenance that the corpus is now dated.
- **Cost:** free.

## H1 — Coverage levers for multi-session questions (target: `recall_all@10`) — **all levers measured, all gates failed**

- **Status (2026-10-15):** **H1d now measured too — gate FAILED, lever closed** (`CONSISTENCY.md`
  §41, artifacts `eval/SELECTION_VECTOR_AXIS_ALL10.md` +
  `eval/results_selection_vector_all10.json`). The vector axis re-scored with
  `recall_all@10` added to the metric vector (`Agg::all` → 7 metrics; the walk objective
  untouched): +0.4 / +0.9 / +1.3 / **+1.8** / +1.3 / −2.7pp at vec = 0.10/0.25/0.50/0.75/
  1.00/1.50, up/down 10/3 … 54/25 … 76/116. 0.10–0.25 sit below the +1pp gate; 0.50–1.00 cross
  it only by trading −7.0 to −12.3pp R@1 (deterministic harness — the regressions are real);
  1.50 collapses. **The deep-miss claim is falsified**: of the 99 worst-gold-at-21+ misses,
  dense fusion rescues 2/2/3/4 at 0.10/0.25/0.50/0.75 — the aggregate gain is shallow-end
  reordering, not semantic reach. No LongMemEval consultation (LoCoMo fail closes it).
  **The `--features embed` capability stays** as an opt-in, off-by-default arm; the evidence
  says do not promote it, and the user's shipping decision is now informed. H1a–c failed
  2026-10-14 (see below); H1e stays a user mechanism, not a benchmark lever.
- **Status (2026-10-14):** H1a/H1b/H1c measured once each on the dev set — **all three
  gates failed** (`CONSISTENCY.md` §37, artifacts `eval/LOCOMO_LEVERS.md`), CLOSED.md rows
  added. No LongMemEval consultation: a LoCoMo fail closes the lever, as pre-registered.
  H1a is the near-miss worth knowing about: R@1 +3.0pp, MRR +2.2pp, NDCG@10 +1.7pp, but
  `recall_all@10` +0.5pp < the +1pp gate and single-hop R@5 −0.4pp. H1d remains open and
  is the **user's** call (~3× artifact size); H1e stays a user mechanism, not a benchmark
  lever.
All are `FusionWeights` / query-side mechanisms that already exist and ship **inert**
(`src/recall.rs` module doc table). Turning one on is a one-line edit to
`FusionWeights::SHIPPED`; the work is the measurement. Each is judged on LoCoMo
`recall_all@10` **and** must not regress R@1/R@5/NDCG on any category by more than the
run-to-run spread (the harness is deterministic, so any regression is real — report it).

| id | lever | mechanism (argue it before measuring) | already measured? |
|---|---|---|---|
| H1a | `keyword_scope: ExactAndPrefix` (FTS5 `token*`) | BM25's exact-token match misses morphological variants (migrate/migration). Plural questions use a verb ("play", "visit") whose sessions use other forms | **No.** Not a walked axis in `select_fusion`. Read `KeywordScope`'s rustdoc first: it records measured `bm25()` values that decided what the prefix arm emits |
| H1b | `overlap_scope: Sentence` (per-segment overlap) | whole-document overlap is diluted by session length; a session where one sentence matches strongly is scored like one where many sentences match weakly. Plural questions need exactly the former | **No.** `eval/SELECTION.md` limitation 5 names it unswept |
| H1c | `bm25_magnitude > 0` | rank-only RRF discards the real BM25 score; a rank-7 row with a far better score is invisible to it | Partly: on LoCoMo R@5 86.0→86.3% at 4.00, no collapse (`eval/SELECTION.md` reading 2). **`recall_all@10` was not measured then** |
| H1d | dense stream (`--features embed`, `FusionWeights::vector`) | the one arm that reaches a document no lexical stream scores above zero | Axis exists (`select_fusion --axes vector`; see `eval/SELECTION_VECTOR_AXIS_FIXED.md`). Re-read it, then measure `recall_all@10` |
| H1e | per-bank `recallSynonyms` | user-supplied paraphrase; not a hardcoded table (EXCEED_PLAN Phase B) | a mechanism for *users*, not a benchmark lever: a table tuned on LoCoMo is fitted. Do not build one to score |

- **Test:** add `all10` to `select_fusion`'s metric vector (`METRIC_NAMES`) so existing
  axes report it, then run `cargo run --release --example select_fusion -- --axes <axes>`
  for the axes in question; for H1a/H1b add the field as an axis or run `locomo` with the
  field edited (`FusionWeights { keyword_scope: ..., ..SHIPPED }` is already how
  `locomo.rs` builds its grid — see `GRID`).
- **Pre-register values.** `ExactAndPrefix` and `Sentence` are binary, not a sweep. For
  `bm25_magnitude` use the one value the algebra suggests (1.0: one extra RRF-sized term,
  bounded by construction) rather than a grid.
- **Gate:** `recall_all@10` up by more than 1 pp **and** per-query up-count > down-count
  by a margin you can state, no category regression, R@20 not down. Then, and only then,
  LongMemEval retrieval **once** (test consultation, counted). A null on LoCoMo means do
  **not** run LongMemEval; record the null.
- **Record:** `docs/CONSISTENCY.md` §36+; if a default changes, mark it `provisional:` in
  code and docs if any LongMemEval number informed it.
- **Cost:** free; ~25 s per LoCoMo configuration.
- **Prior:** H1a/H1b low-to-moderate (cheap, mechanism is plausible); H1c low (already
  near-null); H1d the only one that can reach semantic misses, but it costs bytes — a
  user decision (README "Decisions that belong to the user").

## H2 — Intermediate granularity (one value, low prior)

- **Status:** **rejected 2026-10-14** — windows W=2 (stride 1, pre-registered from the
  production turn-pair unit) measured once: R@1 60.4→59.3%, `recall_all@10` 83.0→82.6%,
  and R@pool 100.0→98.7% — windowing pushes banks past the candidate-pool ceiling, so
  some gold becomes unreachable at any K. Gate failed; granularity is now closed
  (`Turns` §35 and `Windows(2)` both worse than sessions). No second W tried, as
  pre-registered (`CONSISTENCY.md` §37, `eval/LOCOMO_WINDOWS.md`).
- **Mechanism:** a window of consecutive turns keeps the local context a bare turn loses
  while still concentrating the match. Hooks write turn *pairs* in production, so a
  window is closer to the product's real unit than a 25k-token session (TODO #12).
- **Test:** `--turns`-style arm with windows of W consecutive turns (stride W/2), session
  scored at its best window (MaxP via `session_of`). **Pre-register a single W** from
  mechanism (e.g. the product's turn-pair ⇒ W = 2, or W ≈ 10 as "one topical exchange")
  and state why *before* running. Do not scan W.
- **Gate:** better than the session arm on `recall_all@10` and R@1, per-category. If not,
  record the null and stop; do not try a second W.
- **Record:** `eval/LOCOMO_WINDOWS.md` + CONSISTENCY. **Cost:** free. **Prior:** low.
- Note AGENTS/EXCEED_PLAN Phase E: granularity is a *product* capability and must not
  enter the LongMemEval comparison (it changes the corpus unit).

## H3 — Recency on a dated dev set (low prior)

- **Status:** **done 2026-10-14, falsified** (`CONSISTENCY.md` §37, `eval/LOCOMO_LEVERS.md`):
  temporal `recall_all@10` −7.8pp, temporal R@1 −33.4pp; control exact — 1,025/1,025
  detector-negative lists bit-identical. Protocol note: `TemporalQueriesOnly` gates on
  the *grammatical* `is_temporal_query` detector, not LoCoMo's category labels, so the
  "other categories unchanged" clause below was unsatisfiable as written (the detector
  fires on 506/1,531 queries, 137 outside the temporal category); the honored contract is
  the detector's negative set and the spillover is reported. Recency stays 0.0/`Always`
  (inert); the dated corpus (H0) remains for any date-*aware* mechanism that is not a
  recency prior.
- **Mechanism:** `RecencyPolicy::TemporalQueriesOnly` applies the third stream only to
  queries `is_temporal_query` reads as time-denoting. LoCoMo `temporal` is 321 q at 81.9%
  judged. **Caveat to state up front:** LoCoMo temporal questions mostly ask *when a
  specific event happened*, not for the *most recent* thing, so a recency prior may not
  help; and `docs/EXCEED_PLAN.md` Phase C records that the citation that motivated
  recency does not exist. This is a falsification test, not an expectation.
- **Test:** after H0, `recency` at one pre-registered weight (the RRF-sized 1.0) with
  `TemporalQueriesOnly` vs shipped. Judge the **temporal** category and the overall
  vector. (The existing `select_fusion` recency axis is invalid until H0 — its result
  was ingest order.)
- **Gate:** temporal-category `recall_all@10` and R@1 up, other categories unchanged
  (the policy must leave non-temporal queries bit-identical — assert it).
- **Record:** CONSISTENCY. **Cost:** free. **Prior:** low.

## H4 — A dev set that exhibits "reader drowns in context" (prerequisite for TODO #17)

## H4 — A dev set that exhibits "reader drowns in context" (prerequisite for TODO #17)

- **Status:** **done 2026-10-14 — flat verdict** (`CONSISTENCY.md` §38). 200-q subset (`eval/h4_subset.json`; eligible 1,035, per-category split 7/140/8/45, seed 42) at k = 3/5/10 through AMB `rag` on the small-stack reader/judge, accuracy **92.0% / 94.0% / 94.0%** at median context **4.3k / 7.2k / 14.6k** tokens, n=200 each, zero refusals. The drowning failure did not reproduce on independent data; TODO #17 closed as "not reproduced" (LongMemEval's 10→20 dip attaches to the instrument's 0.5B reader, not memory-wire). Collection handoff → H5 in flight in the same chain.
- **Mechanism:** the LongMemEval probe showed reader accuracy *given the gold session
  served* falling from 6/10 (gold alone) to 1/10 (gold + 2 distractor sessions). If that
  is a property of the small reader and not of LongMemEval, it should reproduce on LoCoMo
  as accuracy-given-all-gold-served falling when the served set grows.
- **Test (LLM):** add `MEMORY_WIRE_TOPK` (cap on `k`) to `eval/amb/memory_wire.py`; run
  AMB `rag` on a **fixed stratified 200-question LoCoMo subset** (seeded; only questions
  where all gold is within the top 3) at k = 3, 5, 10 through the shim. Compare accuracy
  per k at fixed gold coverage. ~1,200 calls.
- **Gate:** if accuracy at fixed coverage falls with k on LoCoMo, a precision lever has a
  dev set (candidates then: tighter recall budget composition, dropping near-duplicate
  sessions, tag-based exclusion — argue each by mechanism). If it does not fall, record
  that the LongMemEval observation is specific to LongMemEval's long, noisy haystacks and
  close TODO #17 as "not reproduced on independent data".
- **Record:** CONSISTENCY; commit the AMB output unedited as an `eval/amb/*.json.gz`.
- **Cost:** ~1,200 free-tier calls (hours). Same-reader/judge rules as the baseline.

## H5 — `answer_quality` with dates (one test consultation)

- **Status:** **done 2026-10-14, consultation #3** (`CONSISTENCY.md` §39, artifact
  `eval/baseline/2026-10-14-v0.7.0-dated-answer-quality/`). Dated instrument reads
  **18.6%** conditioned (93/499 scored, 1 unscored — provisional by its own rule), closed-book
  6.0%, delta +12.6pp; the undated 17.0% stays beside it as a different instrument. The
  disagreement shape is unchanged: gold served in 90.8% of contexts, 73.0% served-and-wrong —
  the answerer is the bottleneck, not the ranking. No retrieval parameter was read from this
  run; nothing was selected.
- **Mechanism:** LongMemEval temporal-reasoning (5.3%, n 133) and knowledge-update (14.1%,
  n 78) were close to unanswerable because the reader saw no dates. The change alters the
  prompt, not retrieval (ranking cannot move: the candidate pool is by rowid, ties break by
  id, recency weight is 0).
- **Test:** `TAG=<new> eval/baseline/run_baseline.sh answer_quality` (≈2,000 calls).
  Run it **once**, when the free tier is idle.
- **Gate:** report the whole table beside the undated 17.0%, flagged as a *different
  instrument*; expect the movement to concentrate in those two categories. This is a
  harness correctness check; **nothing is selected on it.**
- **Record:** CONSISTENCY, **test consultations +1** (this will be the third for the
  v0.7.0 line: retrieval, undated answer_quality, dated answer_quality).
- **Cost:** hours of free tier. Do not run concurrently with another LLM job (shared
  rate and, on this host, load 20+).

## H6 — PrecisionMemBench precision (TODO #7)

- **Status:** **done 2026-10-14** — the adapter now ingests dataset tags and honours the
  filter's `any` scope groups via the store's include-any recall (`supports_filters =
  True`); `narrow_any` identity clauses are not honoured (trigram tag-vocabulary
  resolution is Hindsight-native machinery; misspelled-name cases still over-return).
  **14→19 of 77 passes, precision 0.17→0.24, recall unchanged 0.95** — the number moved
  because of the provider capability change, not ranking (filters are pre-fusion SQL
  narrowing). Artifact: `eval/amb/PRECISIONMEMBENCH-2026-10-14-h6-filters.json.gz`
  (`CONSISTENCY.md` §37). Measure-only: no gate, no threshold chosen.
- **Mechanism:** PMB cases carry caller-supplied `scope:` tags; memory-wire can already
  filter on tags. The over-return is the same shape as `docs/OPEN_HOOK_RECALL_RELEVANCE.md`.
  A provider that honours scope tags is a **provider-capability change**, not a ranking
  change (`supports_filters` in `eval/amb/memory_wire.py`).
- **Test:** enable filters in the adapter by mechanism, re-run PMB once (10 s, free),
  commit the output as a new `eval/amb/PRECISIONMEMBENCH-*.json.gz`, and **state that the
  number moved because of the capability change.**
- **Gate:** none for selection — PMB is 77 cases and measure-only. **Do not** pick a score
  threshold off this table (rejected in CONSISTENCY §32).
- **Cost:** free.

## H7 — Housekeeping measurements

- **Concurrency (TODO #14):** **done 2026-10-14 — PASS** (`CONSISTENCY.md` §40,
  `eval/baseline/2026-10-14-bench-concurrency/BENCH_CONCURRENCY.md`): 1.85× at 64 clients
  vs 1.50× floor at load 0.98→4.33, RSS flat 12.9 MB, 0 panics. The 2026-10-13 failure was
  load noise, as suspected; the baseline artifact stays as the dated record of that regime.
  TODO #14 closed.
- **Provider granularity for the long AMB corpora (TODO #12):** only if you need
  BEAM/PersonaMem/LifeBench numbers; decided by mechanism (mirror the hooks' write path);
  it costs ranking quality on LoCoMo (H2 result), so it is a measurability change.
- **aroute verification (TODO #13):** when the user says aroute is fixed, run the
  recipe at the bottom of `docs/AROUTE_BUGS.md` and delete `eval/llm_shim.py` if clean.
- **Relevance battery growth (TODO #6):** 10 labelled cases is too few to gate on. The
  battery reads the live bank `omp`, which drifts (junk-injected moved 1/9→0/9 with the
  code unchanged). Prefer a frozen snapshot bank for any before/after.

## 3. Experiment protocol (copy this per hypothesis)

1. Write the mechanism paragraph in your notes; commit it to the plan *before* running.
2. Pre-register the values (single value, or the exact short list) and the gate.
3. Run the **control** first and confirm it reproduces the committed artifact.
4. Run once. Record the output file unedited under `eval/` (gzip if large).
5. Report per-category, n, and the per-query up/down counts. A pooled gain with a
   regression in one category is reported as both.
6. Append a `docs/CONSISTENCY.md` section: what, on which set, load, numbers, whether it
   was selected on the dev set, and the **test-consultation count** (0 unless you ran
   LongMemEval or a harness wrapping it).
7. Update `TODO.md`, the status line of the hypothesis here, and `docs/handoff/CLOSED.md`
   if it is a negative.
8. Run the gate (AGENTS §2). Do not commit.

## 4. Results log (append; never rewrite)

| date | hypothesis | outcome | artifact | CONSISTENCY § | LME consultations |
|---|---|---|---|---|---|
| 2026-10-14 | turn-level writes (`locomo --turns`) | rejected: `recall_all@10` 83.0→80.7%, R@1 60.4→54.6% | `eval/LOCOMO_TURNS.md` | 35 | 0 |
| 2026-10-14 | `recall_all@10` added to dev scorer | measurement only: 83.0% shipped | `eval/LOCOMO.md` | 35 | 0 |
| 2026-10-14 | `created_at` round-trip (TODO #15) | capability closed | tests in `src/main.rs`, `src/mcp.rs` | 35 | 0 |
| 2026-10-14 | **H1.0** rank distribution of missed gold | descriptive: 261 misses, worst gold 89/73/99/0 (11–15/16–20/21+/absent); gate fires for reordering levers | `eval/LOCOMO_RANKS.md` | 36 | 0 |
| 2026-10-14 | **H0** date-aware dev corpus | done; control bit-identical at recency 0 | `eval/LOCOMO.md` (provenance) | 36 | 0 |
| 2026-10-14 | **H1a** `keyword_scope: ExactAndPrefix` | gate FAILED: +0.5pp < +1pp, single-hop R@5 −0.4pp (R@1 +3.0pp recorded) | `eval/LOCOMO_LEVERS.md` | 37 | 0 |
| 2026-10-14 | **H1b** `overlap_scope: BestSentence` | gate FAILED: −0.3pp, multi-hop R@1 −11.3pp | `eval/LOCOMO_LEVERS.md` | 37 | 0 |
| 2026-10-14 | **H1c** `bm25_magnitude: 1.0` | gate FAILED: −0.2pp | `eval/LOCOMO_LEVERS.md` | 37 | 0 |
| 2026-10-14 | **H2** windows W=2 | rejected: −0.4pp, R@pool 100→98.7% (pool ceiling breached); granularity closed | `eval/LOCOMO_WINDOWS.md` | 37 | 0 |
| 2026-10-14 | **H3** recency 1.0 `TemporalQueriesOnly` | falsified: temporal R@1 −33.4pp, control exact | `eval/LOCOMO_LEVERS.md` | 37 | 0 |
| 2026-10-14 | **H6** PMB scope-tag filters | measured: 14→19/77, precision 0.17→0.24 (capability change, not ranking) | `eval/amb/PRECISIONMEMBENCH-2026-10-14-h6-filters.json.gz` | 37 | 0 |
| 2026-10-14 | **H4** reader drowning (k=3/5/10, 200-q subset) | flat: 92.0/94.0/94.0% at 4.3k/7.2k/14.6k tokens; not reproduced; TODO #17 closed | `eval/amb/h4_locomo_rag_topk{3,5,10}.json.gz` | 38 | 0 |
| 2026-10-14 | **H5** dated `answer_quality` re-measure | 18.6% conditioned (93/499), closed-book 6.0%; beside undated 17.0% as a different instrument | `eval/baseline/2026-10-14-v0.7.0-dated-answer-quality/` | 39 | **1** (consultation #3) |
| 2026-10-14 | **H7** `bench_concurrency` quiet-box re-run | PASS: 1.85× at 64 clients, load 0.98→4.33; 2026-10-13 failure was load noise; TODO #14 closed | `eval/baseline/2026-10-14-bench-concurrency/` | 40 | 0 |
| 2026-10-15 | **H1d** dense arm on `recall_all@10` | gate FAILED: +1.8pp best at vec=0.75 bought at −9.9pp R@1; deep-miss rescue 4/99; lever closed | `eval/SELECTION_VECTOR_AXIS_ALL10.md` | 41 | 0 |
| 2026-10-15 | **v0.8.0 release acceptance** | LongMemEval retrieval: byte-identical to the v0.7.0 record (83.8/97.2/98.6/99.6/88.2/89.2, n 500); release cut (feature `04123f7`, bump `a272196`, tag `v0.8.0`, 3-Linux assets via REST, CI disabled around the push) | `eval/RESULTS.md` | 42 | **1** (consultation #4) |
