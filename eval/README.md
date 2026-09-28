# Eval facility

Official-dataset harness for LongMemEval-S (retrieval-only, no LLM judge).

## Quick start

```bash
./eval/download.sh   # 264 MB, once
cargo run --release --example longmemeval -- --data eval/data/longmemeval_s_cleaned.json --n 500
```

Flags: `--data PATH` (required unless default), `--n 0` (= all), `--seed 42`,
`--out-json eval/results.json`, `--out-md PATH`.

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
