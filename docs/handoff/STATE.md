# State at handoff — 2026-10-16

Repo `agentic-utility/memory/memory-wire`, branch `main`, HEAD `c919f13` — **v0.8.2,
released** (the tag names `95e54c1`; `3ac422c` and `c919f13` are the records and
navigation passes on top). The day cut two patches: **v0.8.1** (`fbe0a91`+`52e27a7`: the H8 capsule
mode, the `MEMORY_WIRE_FORMAT` hook knob, the OmniMemEval HaluMem battery entry,
acceptance #5 byte-identical) and **v0.8.2** (`62fb072`+`95e54c1`: the `update`
`parse_tag` fix found dead on the first real hop — §45 tells it plainly, including the
correction of the v0.8.0-era "live-verified update" claim). racknerd sits on 0.8.2
(installer hop; wiring preserved; `update` now reads 12 known releases). The H9 BEAM
shape-check landed its verdict: not mechanical, labelling stays the unblock.

## 1. What is in the working tree (post-release residue)

  The §45 records pass and the navigation pass are **committed** (`3ac422c`,
  `c919f13`); the tree was clean at `c919f13`. It now holds the **§6 cycle
  pass, uncommitted by instruction**: the outline (`RND_PLAN.md` §6, H12
  pre-registered) plus its execution — W1 (BEAM pair-unit labels + harness:
  `eval/beam_label.py`, `eval/beam_labels_100k.json`, `examples/beam.rs`,
  artifacts `eval/BEAM.md` + `eval/results_beam.json`), W2 (frozen battery:
  `eval/relevance/freeze_battery.py`, grown `cases.jsonl`, artifact
  `BATTERY-2026-10-16-frozen.txt`), `docs/CONSISTENCY.md` §47–§48,
  `docs/handoff/README.md`, `docs/handoff/STATE.md`, `TODO.md` (rows 6, 9, 20).
  Untracked data under `eval/data/` is gitignored by design. W3's adapter fix
  lives in the pinned clone, not here.

  Cycle state (§6, **closed 2026-10-17 — closing record `docs/handoff/CLOSING-2026-10-17.md`**): **W1 done** — the BEAM pool-scale
  overlap re-test answered (`eval/BEAM.md`, §47): the stream is **not
  deleteable** (0.25 > 0.00 on every ordering metric; 0.00 additionally loses
  R@pool 100→95.5% — the overlap stream carries coverage below BM25's 50-row
  cut at 94–196-doc banks), **but the unfitted 1.00 leads 0.25 on the same
  corpus** (R@5 67.1 vs 63.6, opposite sign to LoCoMo's +1.4pp) — the weight
  stays user-gated. **W2 done** — the battery is frozen (sha256-pinned
  snapshot → deterministic 1,413-row bank) and grown 10→21 (gold 16/17,
  junk 0/9, D-case passes, H8 re-confirmed); precision levers registrable
  from here — none registered. **W3 done — H12** (`eval/omnimevaleval/
  memory-wire-mw_h12_dated_medium/`, §46): the dated instrument reads
  Dynamic Update 0.2056→**0.4056** (+20.0pp), overall 0.6129→**0.7433**,
  Memory Boundary control holding at −2.5pp and context only +12% — the
  H10 headline measured a reader blind to time; both numbers stand as two
  instrument versions. **The §6 cycle is complete**: every workstream
  terminal, records current, 0 test consultations added all cycle. Blocked:
  Track 2/agent-memory-bench (agent-model budget). No ranking lever is open;
  the fitted `overlap 0.25` is now *replicated on LoCoMo, inverted on BEAM* —
  an honest split, the user's call.

That release batch (feature `04123f7`, bump `a272196`) carried everything the old
list below named — the full R&D record set, the product code, and every new
harness and artifact — into the v0.8.0 tag. The pre-release residue list is
retired here rather than kept current:

  Modified (all now **in `v0.8.0`**): `AGENTS.md INSTALL_FOR_AGENTS.md README.md TODO.md
  docs/CONSISTENCY.md docs/INTEGRATION_GAPS.md docs/INTEGRATION_PLAN.md eval/LOCOMO.md
  eval/README.md eval/RESULTS.md eval/download.sh eval/locomo_dev.rs eval/results_locomo.json
  examples/answer_quality.rs examples/bench_footprint.rs examples/locomo.rs
  examples/longmemeval.rs plugin/skills/memory-wire/SKILL.md src/api.rs src/connect.rs
  src/connect_ext.rs src/connect_opencode.rs src/doctor.rs src/hooks.rs src/main.rs
  src/mcp.rs src/paths.rs src/store.rs src/sweep.rs`.
 Untracked (all now **in `v0.8.0`**): `docs/AROUTE_BUGS.md docs/BENCHMARK_LANDSCAPE.md
  docs/handoff/ eval/amb/ eval/baseline/ eval/llm_shim.py eval/LOCOMO_TURNS.md
  eval/results_locomo_turns.json examples/locomo_ranks.rs eval/LOCOMO_RANKS.md
  eval/results_locomo_ranks.json examples/locomo_levers.rs eval/LOCOMO_LEVERS.md
  eval/results_locomo_levers.json eval/LOCOMO_WINDOWS.md eval/results_locomo_windows.json
  eval/h4_subset.json eval/amb/h4_build_subset.py src/connect_claude.rs src/connect_copilot.rs
  eval/SELECTION_VECTOR_AXIS_ALL10.md eval/results_selection_vector_all10.json src/update.rs`.

`eval/amb/` now also carries the run artifacts `PRECISIONMEMBENCH-2026-10-14-h6-filters.json.gz`
and `h4_locomo_rag_topk{3,5,10}.json.gz`; `eval/baseline/` carries
`2026-10-14-v0.7.0-dated-answer-quality/` (H5) and `2026-10-14-bench-concurrency/` (H7).

Check with `git status --short`; `git diff --stat` shows size. The gate passed on this tree
on **2026-10-15, after the H1d measurement** (release build + all examples, **589** tests
across 9 suites, clippy `-D warnings --all-features` clean — the embed code path included,
0 doc warnings). The embed build ran in an isolated `CARGO_TARGET_DIR`; the default
binary is untouched (`DT_NEEDED` 3). **Re-run the whole gate (AGENTS §2) before
reporting anything as verified** — the tree keeps moving.

### Product changes in this cycle (all tested, none tuned)

| change | where | test |
|---|---|---|
| `retain` accepts `created_at` (RFC 3339 or `YYYY-MM-DD`, stored UTC fixed-width; else frozen `400 invalid content`, nothing written) over HTTP and MCP | `src/api.rs` `parse_created_at`, `retain_doc_at`; `src/main.rs`; `src/mcp.rs`; const `store::CREATED_AT_FORMAT` | `retain_created_at_should_round_trip_through_full_recall` in `src/main.rs` and `src/mcp.rs` |
| `format: "full"` recall returns `{id, score, created_at, content}` (one shared `api::full_hit_json`) | `src/api.rs` | same tests; default (bare-string) recall unchanged |
| `sweep.rs` uses the shared stamp constant | `src/sweep.rs` | existing sweep tests |

TODO row 15 is closed by this. It is a *capability*, not a ranking change: the recency
weight ships at 0.0, so retrieval order is unchanged.

### Harness changes in this cycle

| change | where | note |
|---|---|---|
| `recall_all@10` (every gold session in the top 10) added to the LoCoMo dev scorer and its artifact | `eval/locomo_dev.rs` (`recall_all`, `Row.all10`, `Agg.all10()`), `examples/locomo.rs` | session arm is metric-identical to the pre-change run |
| `--turns` arm: one memory per dialogue turn, session ranked at its best turn | `eval/locomo_dev.rs` (`Corpus::load_at`, `session_of`), `examples/locomo.rs` | artifact `eval/LOCOMO_TURNS.md` — a *rejected* experiment, see `CLOSED.md` |
| corpus occurrence times: `Document.timestamp` ingested via `api::parse_created_at` (H0) | `eval/locomo_dev.rs` (`Document::created_at`, `Corpus::load_at`); note + regeneration in `eval/LOCOMO.md` | makes LoCoMo date-aware for H3; control bit-identical at shipped `recency 0.0` — `locomo`, `locomo --turns`, `locomo_ranks` re-run under dated ingestion, every diff line provenance-only |
| H1.0 rank distribution of missed gold sessions | `examples/locomo_ranks.rs`, artifact `eval/LOCOMO_RANKS.md`, `CONSISTENCY.md` §36 | descriptive, selects nothing; gate fires for reordering levers (H1a–c) |
| H1a/H1b/H1c/H3 lever A/B (one pre-registered value each, shipped control in-run) | `examples/locomo_levers.rs`, artifact `eval/LOCOMO_LEVERS.md`, `CONSISTENCY.md` §37 | **all four gates failed** (H1a near-miss: R@1 +3.0pp, all10 +0.5pp < +1pp gate, single-hop R@5 −0.4pp); CLOSED.md rows added; 0 test consultations |
| H2 windows arm: `--windows W`, `Granularity` enum in `eval/locomo_dev.rs` | `examples/locomo.rs --windows 2`, artifact `eval/LOCOMO_WINDOWS.md` | W=2 pre-registered from the production turn-pair; gate failed (R@1 −1.1pp, all10 −0.4pp, R@pool →98.7%); granularity closed |
| H4 fixed 200-q reader-drowning subset | `examples/locomo_h4_subset.rs` → `eval/h4_subset.json`; `eval/amb/h4_build_subset.py` materialises the AMB input | eligibility all-gold-in-top-3 (1,035 eligible), proportional category split, seeded draw 42 |
| AMB adapter: `MEMORY_WIRE_TOPK` cap (H4) + scope-tag filters (H6) | `eval/amb/memory_wire.py` (copied into `../_audit/amb`) | PMB: 14→19/77, precision 0.24 at recall 0.95; `narrow_any` unhonoured, named |
| `answer_quality` stores sessions at `haystack_dates`, prefixes each served memory `[<date>]`, puts `question_date` in the prompt | `examples/answer_quality.rs` | **changes the instrument**: runs before 2026-10-14 are undated and not comparable |
| AMB provider sends `Document.timestamp` as `created_at`, reads it back from `full` recall | `eval/amb/memory_wire.py` (copied into `../_audit/amb/src/memory_bench/memory/`) | the committed baseline run predates this; see §3 |
| `eval/llm_shim.py`: unwrap/retry/total-deadline proxy between harnesses and aroute | `eval/llm_shim.py` | exists only to paper over `docs/AROUTE_BUGS.md` B1–B2; delete when aroute is fixed |
| H7 quiet-box `bench_concurrency` | `eval/baseline/2026-10-14-bench-concurrency/BENCH_CONCURRENCY.md` | **PASS**: 1.85× at 64 clients vs 1.50 floor, load 0.98→4.33, RSS flat 12.9 MB; the 2026-10-13 failure was load noise (`CONSISTENCY.md` §40) |
| `eval/baseline/run_baseline.sh`: whole battery, resumable `amb_rag` | `eval/baseline/` | see `RUNBOOK.md` |

## 2. Current numbers (every one traces to a committed artifact)

Baseline directory: **`eval/baseline/2026-10-13-v0.7.0/`** (see its `README.md` and
`PROVENANCE.md`). Load was 11–28 on 24 cores throughout: retrieval metrics are
load-independent, **every latency/throughput/RSS figure is range-quality.**

### Retrieval (free, deterministic)

| | R@1 | R@5 | R@10 | NDCG@10 | n | artifact |
|---|---|---|---|---|---|---|
| LongMemEval-S, all (**test**) | 83.8% | 97.2% | 98.6% | 88.2% | 500 | `LONGMEMEVAL.md` |
| LongMemEval-S, 470 (abstention excluded) | 84.0% | 97.2% | 98.5% | 88.5% | 470 | same |
| LoCoMo (**dev**), `overlap 0.25` shipped | 60.4% | 86.0% | 94.2% | 73.6% | 1,531 | `LOCOMO.md`, repo `eval/LOCOMO.md` |
| LoCoMo `recall_all@10` (new), shipped | — | — | **83.0%** | — | 1,531 | `eval/LOCOMO.md` |
| LoCoMo `recall_any@10` for contrast | | | 94.2% | | 1,531 | same |
| BEAM-100k (**dev**, pool-scale re-test §47): 1.00 / 0.25 / 0.00 | 42.5 / 39.9 / 36.4 | 67.1 / 63.6 / 62.3 | 77.6 / 76.4 / 74.8 | 42.3 / 41.2 / 39.3 | 313 | `eval/BEAM.md` |
| Frozen relevance battery (§48): auto gold 16/17, junk-injected 0/9, search-reachable 17/17; H8 gate green (43% less text) | | | | | 21 | `eval/relevance/BATTERY-2026-10-16-frozen.txt` |
| PrecisionMemBench (measure-only) | total 14/77, active 3/43, precision 0.17, recall 0.95 | | | | 77 | `precisionmembench.json.gz` |

Honesty note: `overlap 0.25` was chosen on LongMemEval (46 configurations) and is
**fitted** (`docs/EVALUATION_HYGIENE.md` §2.1). The honest unfitted LongMemEval R@5 is
93.0% (equal weight); 97.2% is the fitted number. Do not quote 97.2% as evidence that
anything is "better".

New 2026-10-14 (**H1.0**, dev set, no LLM — `eval/LOCOMO_RANKS.md`, `CONSISTENCY.md` §36):
of the 261 `recall_all@10` misses, the worst-ranked gold session sits at 11–15 / 16–20 /
21+ / absent = 89 / 73 / 99 / **0**. Every miss is an ordering position inside a complete
list (fused list = whole bank on 261/261); 62.1% within 11–20 ⇒ reordering levers (H1a–c)
are worth building, 99 misses at 21+ bound what they can recover.

### LLM-judged (reader and judge both aroute `small-stack`, temperature 0)

| | accuracy | n | artifact |
|---|---|---|---|
| `answer_quality`, LongMemEval-S (test), conditioned | 17.0% (85) | 500 | `ANSWER_QUALITY_LME.md`, `answer_quality_lme.json` |
| same, closed-book control | 6.0% (30) | 500 | same |
| AMB LoCoMo `rag` (measure-only) | **84.1%** (1,295) | 1,540 | `amb_locomo_rag.json.gz` |
| — open-domain / temporal / multi-hop / single-hop | 92.7% / 81.9% / 68.8% / 66.0% | 841 / 321 / 96 / 282 | same |
| — by gold coverage in the served top 10: all / some / none | 91.2% / 53.0% / 36.5% | 1,289 / 166 / 85 | derived from the same file |

How to read them (the full reasoning is `docs/CONSISTENCY.md` §34):

- The LongMemEval 17.0% is a **small-reader floor**, not memory quality: a gold session
  was served in 454/500 and ranked first in 419 of those, yet 368 of the 374 wrong
  answers were `I don't know`. Gold alone was answered 6/10, gold + 2 other sessions
  1/10 (10 items, diagnostic).
- On LoCoMo the reader does *not* refuse (12 of 245 wrong answers). Accuracy tracks
  whether **all** gold sessions were served — this is why `recall_all@10` exists.
- **Never read 84.1% against 17.0%.** Different corpus and bank size (LoCoMo banks hold
  19–32 sessions). Never place a retrieval number beside a judged one, or beside a
  number from a different reader/judge (`docs/BENCHMARK_LANDSCAPE.md` §6).

### Footprint and speed (range-quality — load 11–28)

Binary 10,745,080 B (10.25 MiB), `DT_NEEDED` 3; idle RSS 9.88 MB; cold start 13–16 ms;
retain 2,018 ops/s; recall p50 7.2 ms at 10k memories. **`bench_concurrency` failed its
2-client gate (1.30× vs 1.50×) under load but absolute throughput rose** — unreadable;
re-measure on a quiet box (`TODO.md` #14).

## 3. Instrument caveats a fresh agent will otherwise rediscover the hard way

- **The reader is a fallback chain.** aroute routes only combo names; `small-stack` is
  served by free-tier models, in practice `poolside/laguna-s-2.1:free` (3,542 of 3,572
  AMB-era calls). The served model cannot be pinned; `llm-calls.jsonl` records it per
  call. Two baselines compare cleanly only where the served mix matches. Free tier is
  slow at load (median 12 s/call on ~15k-token prompts).
- **The committed AMB LoCoMo row used the *old* adapter** (dates held client-side in a
  header prefix). The current adapter round-trips `created_at` through the product. The
  retrieval order is identical, so the expectation is a similar score, but a re-run is a
  new instrument version — put it in a new `TAG`, never over the old directory.
- **AMB `rag` trap:** the dataset prompt builders replace the formatted documents with
  `json.dumps(raw_response)` whenever a provider returns one. The adapter must return
  `None` there (it does; the first run returned metadata and scored 0/29).
- **AMB does not retry client timeouts** (SDK default 600 s). The shim therefore caps a
  call at 480 s and returns a 502 (which AMB retries). Do not remove `--deadline`.
- **The host reset three times** during the baseline (network outage 21:00–22:10, then
  hard resets 02:09 and 03:34 on 2026-10-14; no OOM/panic record precedes either reset).
  `amb_rag` resumes from AMB's per-conversation saves in `target/amb-out/$TAG`
  (gitignored). Relaunch the same command with the same `TAG`; it skips finished units.
- **aroute defects** (HTTP-200-with-error-body, cline `{"success","data"}` envelope,
  cline token headers 0, 403 without cooldown): `docs/AROUTE_BUGS.md`. When aroute is
  fixed the verification recipe is in that file; then delete `eval/llm_shim.py`.

## 4. Environment

| thing | value |
|---|---|
| build env | `export PATH=/usr/bin:$PATH` (a different `ar` in `~/.local/bin` breaks `libsqlite3-sys`) |
| live memory-wire serve | `127.0.0.1:8888`, bank `omp`, the user's **real** store — never write to it from a harness. Harnesses use scratch servers on 8897–8899 |
| aroute | systemd user unit `aroute.service`, `127.0.0.1:20128`, no client auth; do not restart/modify |
| AMB checkout | `../_audit/amb` @ `03c1d0f`, venv `../_audit/amb/.venv/bin/amb`; provider installed from `eval/amb/memory_wire.py` |
| datasets | `eval/data/` is gitignored; fetch with `./eval/download.sh` (LoCoMo, LongMemEval-S are already present on this machine) |
| disk | `/home` ~126 GB free at handoff |
| background jobs | the PMV2 triangulation chain is running (resume #2 from `--from-step 3`, `/var/tmp/pmv2_resume_chain.sh`); marker `PMV2 chain done` in `/var/tmp/pmv2/resume-launcher.log`; lands §49 + `eval/omnimevaleval/memory-wire-mw_pmv2_dated/`; on a daemon-restart wake, FIRST check :8899 health + the responses checkpoint, then relaunch (idempotent) |

## 5. What the LoCoMo judged run says about where the headroom is

From `amb_locomo_rag.json.gz` joined with `eval/data/locomo/queries.json`
(`gold_ids`) — derive again with the snippet in `RUNBOOK.md` §7:

- 1,289 of 1,540 questions had every gold session in the reader's top 10; those were
  answered 91.2%. The remaining 251 were answered 47.4% (119/251). So most recoverable
  headroom is *coverage of multi-session questions*, not single-gold ordering.
- `recall_all@10` misses concentrate where gold is plural: single-hop with ≥4 gold
  sessions 43/49 miss, multi-hop with ≥4 gold 10/10 miss, single-hop with 2 gold
  75/163 miss (from `eval/results_locomo.json`, shipped config).
- **H1.0 (2026-10-14) settled where those missed sessions sit**: worst gold at 11–15 /
  16–20 / 21+ / absent = 89 / 73 / 99 / 0 of 261 misses; per missed session
  146 / 113 / 115 / 0 of 374 (`eval/LOCOMO_RANKS.md`). Zero absent and the fused list
  covers the whole bank on every miss, so **no miss is a matching failure** — the
  pre-registered gate fires for the reordering levers (H1a–c); the 99 queries at 21+
  are out of a reordering lever's reach and are the semantic residue (H1d, user's call).
- **H1d (dense arm) measured 2026-10-15 — gate FAILED** (`CONSISTENCY.md` §41,
  `eval/SELECTION_VECTOR_AXIS_ALL10.md`): `recall_all@10` +1.8pp best at vec=0.75,
  bought at −9.9pp R@1; deep-miss rescue **4/99** — the arm reorders the shallow end,
  it does not reach the semantic residue. No test consultation. The `--features embed`
  capability stays opt-in/off-by-default; the promotion decision is now informed and
  the evidence says no. **This closes every open H1 lever; the R&D queue is empty
  except condition-gated Tier-3 items.**
- **The reordering levers were then measured once each and all failed their gates**
  (`CONSISTENCY.md` §37, `eval/LOCOMO_LEVERS.md`): H1a `ExactAndPrefix` near-miss (R@1
  +3.0pp but all10 +0.5pp, single-hop R@5 −0.4pp), H1b `BestSentence` (all10 −0.3pp,
  multi-hop R@1 −11.3pp), H1c `bm25_magnitude` (all10 −0.2pp). H3 recency was
  falsified on the dated corpus (temporal R@1 −33.4pp, control exact), and H2 windows
   W=2 lost too (R@pool →98.7%). **The shipped configuration survived every challenger**
   — the 99 deep misses remain H1d's (user's call), and precision levers await H4's
   verdict (landed: AMB rag k=3/5/10 on `eval/h4_subset.json`; **flat: 92.0/94.0/94.0%**
   at median context 4.3k/7.2k/14.6k tokens, zero refusals, n=200 per k ⇒ the drowning
   failure does not reproduce on the dev set; TODO #17 closed "not reproduced", and no
   precision lever is built — `CONSISTENCY.md` §38). H5 dated `answer_quality` also landed:
   **18.6%** conditioned (93/499, 1 unscored), closed-book 6.0%, delta +12.6pp — beside the
   undated 17.0% as a different instrument, consultation #3 (`CONSISTENCY.md` §39,
   `eval/baseline/2026-10-14-v0.7.0-dated-answer-quality/`). The disagreement shape is
   unchanged: gold served 90.8%, 73.0% served-and-wrong — the answerer, not the ranking.
- AMB `rag` serves k = 10 of a 19–32-session bank. That cut is a *harness* parameter,
  not a memory-wire one. The adapter now carries `MEMORY_WIRE_TOPK` to move that cut
  for measurement; H4 used it to fix the served set at k = 3/5/10.
