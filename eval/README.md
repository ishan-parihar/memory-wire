# Eval facility

Harnesses over two corpora, on the three-set discipline in `AGENTS.md` §1 and
`docs/EVALUATION_HYGIENE.md` §3.1. Most are **retrieval-only, no LLM judge**; the
one exception is `answer_quality`, called out below.

| set | corpus | role |
|---|---|---|
| **test** | LongMemEval-S, 500 questions | measurement and release acceptance only — **never selection** |
| **dev** | LoCoMo, 1,540 queries / 272 documents | **all selection happens here** |
| synthetic | `bench_recall_curve`, `tests/scale.rs` | scaling and regression assertions; we author the distractors, so weak evidence about ranking quality — do not tune against it either |

## Quick start

```bash
./eval/download.sh   # 264 MB LongMemEval-S + 740 KB LoCoMo, once each
cargo run --release --example longmemeval -- --data eval/data/longmemeval_s_cleaned.json --n 500
cargo run --release --example locomo -- --out-md eval/LOCOMO.md --out-json eval/results_locomo.json
```

`download.sh` fetches both corpora, sha256-pins the LoCoMo files at a named
upstream commit, and materialises them as **plain un-gzipped JSON** under
`eval/data/locomo/` so the Rust harness needs no gzip crate (`serde_json` is
already a dependency; `flate2` is not). It is idempotent, re-fetches a truncated
file, and exits nonzero on a checksum or record-count mismatch. `eval/data/` is
gitignored, so neither corpus is committed.

Flags: `--data PATH` (required unless default), `--n 0` (= all), `--seed 42`,
`--out-json eval/results.json`, `--out-md PATH`. `locomo` takes `--docs PATH`
alongside `--data`, and defaults `--out-json` to a scratch file rather than a
committed path (its per-question dump is a grid's, not the suite's record).

**`--out-md` has no `eval/` default, and that is deliberate.** A bare
`cargo run --release --example …` writes its markdown artifact to a scratch file
under `$TMPDIR` and prints where. Updating a committed `eval/*.md` requires
naming it: `--out-md eval/RESULTS.md`. Those files are the reviewed record of
what was measured, and a single run silently replacing one is a
documentation-destroying edit — it already cost this project the curated
methodology note in `RESULTS.md` once. Every harness shares one rule
(`bench_common::out_md`).

`--out-json` still defaults to `eval/results.json`, which is gitignored, so
overwriting it is harmless.

## Artifacts that must NOT be regenerated

Two committed artifacts are **stale relative to their own generator**, because
the configurations they record were removed from `src/` after they were measured.
Running their generator today silently deletes the evidence for those removals.
This is the same hazard `README.md` documents for `SWEEP_FUSION.md`; it applies
here too, and the list below is the complete set **as of 2026-09-29, checked
against the generators in the tree on that date**. The other grid artifacts in
this directory are the opposite case and are covered in the next section.

| artifact | what a re-run would destroy |
|---|---|
| `eval/SWEEP_FUSION.md` | sections F and G — the E3 `idf` and E4 `coverage` grid rows, removed from `examples/sweep_fusion.rs` |
| `eval/CODING_LIFE.md` | **8 of its 12 swept rows and 2 of its 10 columns.** The committed table has `coverage` and `idf` columns; `examples/coding_life.rs` emits neither, and its grid is 4 entries (`shipped`, `raw overlap=0.50/0.75/1.00`) against the artifact's 12 — the 5 `idf overlap=*` rows and 3 `shipped + coverage=*` rows would vanish |

Verified 2026-09-28 by reading both sides: `examples/coding_life.rs:218` writes
`| configuration | BM25 | overlap | P@k | R@k | Hit rate | p50 latency | n |`
and iterates the 4-entry grid at lines 154–157, while `eval/CODING_LIFE.md:18`
declares `| configuration | BM25 | overlap | coverage | idf | P@k | R@k | Hit rate | p50 latency | n |`
and carries 12 rows.

So: read these two, do not run their generators. If a future change genuinely
needs a refreshed coding-life sweep, the removed arms have to be restored to the
generator first, and that is a `docs/NEXT_ITERATION.md` question, not a refresh.

## The two `SELECTION*` grids are the opposite case — safe, and stale

`eval/SELECTION.md` and `eval/SELECTION_VECTOR_AXIS_FIXED.md` come from
`examples/select_fusion.rs`, which the list above never named. **They are not on
the hazard list, and adding them would be the wrong warning**: a re-run of the
current generator would *add* rows to `SELECTION.md`, not delete any.

The check is the axis table. `SELECTION.md`'s pass-2 table declares **six** axes,
and the generator's current axis constants are **seven** — the six are identical
value-for-value, and `VECTOR` is the new one:

| axis | `SELECTION.md` pass-2 table | generator constant today | same? |
|---|---|---|---|
| `overlap` | 0.00, 0.10, 0.25, 0.50, 0.75, 1.00 | `OVERLAP` = same six | yes |
| `k` | 5.00, 10.00, 20.00, 40.00, 60.00, 120.00 | `K` = same six | yes |
| `bm25_magnitude` | 0.00, 0.25, 0.50, 1.00, 2.00, 4.00 | `BM25_MAGNITUDE` = same six | yes |
| `agreement` | 0.00, 0.05, 0.10, 0.25, 0.50 | `AGREEMENT` = same five | yes |
| `recency` | 0.00, 0.10, 0.25, 0.50, 1.00 | `RECENCY` = same five | yes |
| `recency_half_life_days` | 1.00, 7.00, 30.00, 90.00 | `RECENCY_HALF_LIFE` = same four | yes |
| `vector` | **absent** | `VECTOR` = 0.00, 0.10, 0.25, 0.50, 0.75, 1.00, 1.50 | **new** |

So `SELECTION.md` is a strict subset: regenerating it in a default (non-`embed`)
build reproduces its six axes and loses nothing, and in an `--features embed` build
it gains a seventh. `SELECTION_VECTOR_AXIS_FIXED.md` (02:08, newer) already carries
all seven and matches the generator exactly, so it is the current one.

**`SELECTION_VECTOR_AXIS_FIXED.md` also carries a known template defect, and it is
worth knowing about before quoting §5 of that file.** Where the walk's measured set
is empty, the "spanning `k` from X to Y" line renders its bounds as
`179769313486231570814527423731704356798070567525844996598917476803157260780028538760589558632766878171540458953514382464234321326889464182768467546703537516986049910576551282076245490090389328944075868508455133942304583236903222948165808559332123348274797826204144723168738177180919299881250404026184124858368`
— `f64::MAX`, the fold identity over an empty iterator. It is a formatting bug in
`examples/select_fusion.rs`, not a measurement, and per `AGENTS.md` §2 the fix is in
the generator, which this directory does not own. Both files are untracked, so
neither is yet the committed record.

## Method

Same as agentmemory `benchmark/longmemeval-bench.ts` (see
`../_audit/agentmemory/benchmark/LONGMEMEVAL.md`): per question, fresh
in-memory index, one memory per haystack session (`id` = session id, content =
turns joined), query = question text, session-level recall_any@K / NDCG@10 /
MRR vs `answer_session_ids`. Refresh the numbers with `cargo run` and paste
the `RESULTS.md` table into `README.md` + `docs/BENCHMARK_SCALE.md`. The binary
owns the whole of `RESULTS.md`, methodology paragraph included, so a run
reproduces the committed file apart from the run date and the latency column.

## LoCoMo (the dev set) — `examples/locomo.rs` → `eval/LOCOMO.md`

One bank per `user_id` (the conversation, and the isolation unit), one memory per
LoCoMo document, question-text query. Measures `recall_any@{1,5,10,20}` plus MRR
and NDCG@10, overall and per `meta.category`, with the n on every row, across
**five `overlap` weights in one invocation** — `1.00` (the unfitted equal-weight
baseline), `0.75`, `0.50`, `0.25` (the shipped value) and `0.00` (a diagnostic
bound, not a candidate) — so the comparison is one table. Retrieval only: no
model, no judge, no network call.

It exists to answer one question: **`overlap: 0.25` was selected by sweeping 46
configurations against LongMemEval-S's 500 questions, i.e. fitted to the test set.
Does it beat the unfitted `1.00` on data that never chose it?** See
`docs/PERFORMANCE_PLAN.md` P−1 and `docs/EVALUATION_HYGIENE.md` §3.2.

**A `recall_any@K` from this harness is not a LoCoMo score.** LoCoMo's standard
metric is LLM-judged *answer* accuracy; this harness never generates or judges an
answer, so its number is a different measurement on a different scale and must
not be placed beside a published LoCoMo figure (including the 92.0% in
`README.md`). The artifact's "Comparability" section says this at length.

**This harness measures. It does not select.** A weight that wins on LoCoMo is a
replication, not a licence: picking a new value off that table is a selection
that must be recorded in `docs/CONSISTENCY.md` under the counting rule, and must
never be checked against LongMemEval.

**Use `--release`.** The committed artifacts were generated from the release
profile, which is the one that ships; a default-profile run reports the same
retrieval metrics but roughly 2–5× the latency, so a debug run would silently
contradict the provenance note at the top of each artifact. `coding_life` and
`scale_sweep` take no dataset argument:
`cargo run --release --example coding_life` and
`cargo run --release --example scale_sweep`.

## Answer quality — `examples/answer_quality.rs` → `eval/ANSWER_QUALITY.md`

**The only harness here that calls an LLM**, and the only one that measures what
LongMemEval actually scores. Everything else asks *did the gold session get
retrieved*; this asks *did an LLM, handed what `recall` actually returned, produce
an answer a grader accepted*. It runs both arms — retrieval-conditioned and a
closed-book control over the same questions with an empty context — so the number
cannot be read as a memory-system score on its own.

```bash
export MEMORY_WIRE_LLM_URL='<openai-compatible base url>'
export MEMORY_WIRE_LLM_MODEL='<answering model id>'
export MEMORY_WIRE_LLM_KEY='<bearer token>'
cargo run --release --example answer_quality -- \
  --data eval/data/longmemeval_s_cleaned.json --n 25 --seed 42 \
  --out-md eval/ANSWER_QUALITY.md --out-json eval/results_answer_quality.json
```

Both judge and answering prompts are printed verbatim in the artifact, so the
number can be audited rather than trusted. Two traps, both of which look like a
dead endpoint: a gateway that **times out** (the harness caps each request at 180s,
so 4 calls per question turns a hang into a very long run), and a model that
returns an **empty completion at low `max_tokens`** with `finish_reason: 'length'`.

**Its 40.0% is not a quality score and must not be quoted as one.** Two independent
reasons, both in `docs/CONSISTENCY.md` §14.5: the judge is a local model rather than
the official grader, and the shipped default recall budget of 2,000 tokens does not
hold one median LongMemEval session (2,626 tokens), so the answerer was starved of
context that retrieval had already found. The budget finding is the product bug
here; the accuracy figure is the symptom.

## The benchmark harnesses

`bench_footprint`, `bench_write`, `bench_recall_curve`, `bench_concurrency`,
`bench_coldstart` and `soak`. Each writes a markdown artifact and each carries
its own provenance line (date, build profile, machine). To update a committed
one, pass `--out-md eval/<FILE>.md` explicitly:

```bash
cargo run --release --example bench_footprint    --out-md eval/BENCH_FOOTPRINT.md
cargo run --release --example bench_write        --out-md eval/BENCH_WRITE.md
cargo run --release --example bench_recall_curve --out-md eval/BENCH_RECALL_CURVE.md
cargo run --release --example bench_concurrency  --out-md eval/BENCH_CONCURRENCY.md
cargo run --release --example bench_coldstart    --out-md eval/BENCH_COLDSTART.md
cargo run --release --example soak               --out-md eval/SOAK.md
```

Wall-clock latency in these artifacts is measured against whatever else the
machine is doing. Read the provenance line before quoting a number.

The eighth harness is `select_fusion` (`examples/select_fusion.rs`), the dev-set
axis grid behind `eval/SELECTION*.md` described above. Two things separate it from
the six: it is the only one that needs `--features embed` for its dense axis, and
it is the only one that must never be read as a recommendation — it emits tables
in evaluation order, not sorted by score, precisely so that nothing in the file
can be mistaken for a ranking. `cargo run --release --example select_fusion
-- --axes all` writes to `$TMPDIR` unless `--out-md` names a path.
