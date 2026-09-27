# Eval facility

Official-dataset harness for LongMemEval-S (retrieval-only, no LLM judge).

## Quick start

```bash
./eval/download.sh   # 264 MB, once
cargo run --release --example longmemeval -- --data eval/data/longmemeval_s_cleaned.json --n 500
```

Flags: `--data PATH` (required unless default), `--n 0` (= all), `--seed 42`,
`--out-json eval/results.json`, `--out-md eval/RESULTS.md`.

## Method

Same as agentmemory `benchmark/longmemeval-bench.ts` (see
`../_audit/agentmemory/benchmark/LONGMEMEVAL.md`): per question, fresh
in-memory index, one memory per haystack session (`id` = session id, content =
turns joined), query = question text, session-level recall_any@K / NDCG@10 /
MRR vs `answer_session_ids`. Refresh the numbers with `cargo run` and paste
the `RESULTS.md` table into `README.md` + `docs/BENCHMARK_SCALE.md`.

**Use `--release`.** The committed artifacts were generated from the release
profile, which is the one that ships; a default-profile run reports the same
retrieval metrics but roughly 2–5× the latency, so a debug run would silently
contradict the provenance note at the top of each artifact. `coding_life` and
`scale_sweep` take no dataset argument:
`cargo run --release --example coding_life` and
`cargo run --release --example scale_sweep`.
