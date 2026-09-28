# Eval facility

Retrieval-only harnesses (no LLM judge) over two corpora, on the three-set
discipline in `AGENTS.md` §1 and `docs/EVALUATION_HYGIENE.md` §3.1:

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
