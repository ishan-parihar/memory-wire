# Recall quality and latency vs bank size (memory-wire)

Run 2026-09-28 from `--profile=release` on AMD Ryzen 9 5900X 12-Core Processor, 24 logical CPUs.

One fixed query set of 32 queries — identical text, identical gold ids at every size — recalled at a 2000-token budget through `MemoryService::recall` (shipped) or `MemoryService::recall_with_weights` (every other row), which is the same code path with the weights as an argument. Sizes 1000 / 10000 / 50000 / 100000; 3 latency passes per size, **interleaved across configurations**; seed 42; pool 200 ∪ BM25 50.

One bank is built per size and every configuration is scored on it, so two rows cannot disagree because their index happened to build differently. The passes are interleaved rather than run configuration-after-configuration so that a load spike lands on all of them in the same pass — that makes the latency columns comparable *to each other* and to nothing else. **The absolute microseconds still need an idle box; the R@1/R@5 columns do not.** Load at start `24.64 33.47 31.95 39/7147 1242984`, at end `24.64 33.47 31.95 40/7147 1242984`.

Corpus: 32 gold rows plus distractors, mixed by a seeded LCG permutation so gold rows land throughout insertion order rather than at the front. Each gold row's unique nonce token is what its query repeats; the rest of every row — gold and distractor alike — comes from the four-topic generator `examples/scale_sweep.rs` uses, so the FTS stream has real topical competition at each size and the corpus vocabulary stays comparable with `eval/SCALE_SWEEP.md`. Every query is **six tokens**: five from its topic sentence, repeated across a quarter of the bank, plus the unique nonce. So a distractor of the matching topic matches **five** of a query's six tokens and the gold row matches **all six** — the count stream separates them by one, and only term rarity separates them by five.

**The candidate pool is measured, not assumed.** `in window` counts gold rows inside the `ORDER BY rowid DESC LIMIT 200` window (exact: rowids are assigned in write order). `in BM25` counts gold rows the FTS stream returns at `LIMIT 50`, read through the store's own public `keyword_search_fts`. `in pool` is the union — the candidate set fusion could choose from, and therefore the ceiling on R@1.

| Memories | configuration | signal % | in window | in BM25 | in pool | R@1 | R@5 | p50 us | p95 us | Build |
|---|---|---|---|---|---|---|---|---|---|---|
| 1000 | shipped | 3.200% | 3 | 32 | 32 | 71.9% | 96.9% | 2164 | 3263 | 0.6s |
| 1000 | raw overlap=0.50 | 3.200% | 3 | 32 | 32 | 71.9% | 96.9% | 2230 | 4179 | 0.6s |
| 1000 | raw overlap=0.75 | 3.200% | 3 | 32 | 32 | 71.9% | 100.0% | 2194 | 4042 | 0.6s |
| 1000 | raw overlap=1.00 | 3.200% | 3 | 32 | 32 | 78.1% | 100.0% | 2192 | 4106 | 0.6s |
| 1000 | raw overlap=2.00 | 3.200% | 3 | 32 | 32 | 84.4% | 100.0% | 2183 | 3549 | 0.6s |
| 1000 | idf overlap=0.25 | 3.200% | 3 | 32 | 32 | 71.9% | 96.9% | 2118 | 3732 | 0.6s |
| 1000 | idf overlap=0.50 | 3.200% | 3 | 32 | 32 | 71.9% | 96.9% | 2156 | 4055 | 0.6s |
| 1000 | idf overlap=0.75 | 3.200% | 3 | 32 | 32 | 71.9% | 100.0% | 2166 | 2912 | 0.6s |
| 1000 | idf overlap=1.00 | 3.200% | 3 | 32 | 32 | 78.1% | 100.0% | 2138 | 4826 | 0.6s |
| 1000 | idf overlap=1.25 | 3.200% | 3 | 32 | 32 | 78.1% | 100.0% | 2166 | 3861 | 0.6s |
| 1000 | idf overlap=1.50 | 3.200% | 3 | 32 | 32 | 81.2% | 100.0% | 2201 | 3823 | 0.6s |
| 1000 | shipped + coverage=0.10 | 3.200% | 3 | 32 | 32 | 71.9% | 96.9% | 2176 | 4722 | 0.6s |
| 1000 | shipped + coverage=0.25 | 3.200% | 3 | 32 | 32 | 71.9% | 96.9% | 2175 | 3187 | 0.6s |
| 1000 | shipped + coverage=1.00 | 3.200% | 3 | 32 | 32 | 78.1% | 100.0% | 2176 | 3156 | 0.6s |
| 1000 | idf overlap=1.00 + coverage=0.25 | 3.200% | 3 | 32 | 32 | 78.1% | 100.0% | 2174 | 4036 | 0.6s |
| 1000 | idf overlap=1.00 + coverage=1.00 | 3.200% | 3 | 32 | 32 | 84.4% | 100.0% | 2164 | 4099 | 0.6s |
| 10000 | shipped | 0.320% | 0 | 32 | 32 | 75.0% | 84.4% | 15172 | 25553 | 6.2s |
| 10000 | raw overlap=0.50 | 0.320% | 0 | 32 | 32 | 75.0% | 90.6% | 15693 | 33171 | 6.2s |
| 10000 | raw overlap=0.75 | 0.320% | 0 | 32 | 32 | 75.0% | 100.0% | 15062 | 26778 | 6.2s |
| 10000 | raw overlap=1.00 | 0.320% | 0 | 32 | 32 | 75.0% | 100.0% | 16021 | 28084 | 6.2s |
| 10000 | raw overlap=2.00 | 0.320% | 0 | 32 | 32 | 84.4% | 100.0% | 15791 | 27474 | 6.2s |
| 10000 | idf overlap=0.25 | 0.320% | 0 | 32 | 32 | 75.0% | 84.4% | 16045 | 26784 | 6.2s |
| 10000 | idf overlap=0.50 | 0.320% | 0 | 32 | 32 | 75.0% | 90.6% | 15545 | 27307 | 6.2s |
| 10000 | idf overlap=0.75 | 0.320% | 0 | 32 | 32 | 75.0% | 100.0% | 15475 | 28210 | 6.2s |
| 10000 | idf overlap=1.00 | 0.320% | 0 | 32 | 32 | 75.0% | 100.0% | 15204 | 32790 | 6.2s |
| 10000 | idf overlap=1.25 | 0.320% | 0 | 32 | 32 | 81.2% | 100.0% | 16316 | 38440 | 6.2s |
| 10000 | idf overlap=1.50 | 0.320% | 0 | 32 | 32 | 84.4% | 100.0% | 15628 | 24882 | 6.2s |
| 10000 | shipped + coverage=0.10 | 0.320% | 0 | 32 | 32 | 75.0% | 84.4% | 15285 | 25923 | 6.2s |
| 10000 | shipped + coverage=0.25 | 0.320% | 0 | 32 | 32 | 75.0% | 90.6% | 14856 | 25055 | 6.2s |
| 10000 | shipped + coverage=1.00 | 0.320% | 0 | 32 | 32 | 81.2% | 100.0% | 14790 | 26264 | 6.2s |
| 10000 | idf overlap=1.00 + coverage=0.25 | 0.320% | 0 | 32 | 32 | 81.2% | 100.0% | 14902 | 28646 | 6.2s |
| 10000 | idf overlap=1.00 + coverage=1.00 | 0.320% | 0 | 32 | 32 | 84.4% | 100.0% | 15030 | 31293 | 6.2s |
| 50000 | shipped | 0.064% | 0 | 32 | 32 | 75.0% | 90.6% | 54500 | 98835 | 32.0s |
| 50000 | raw overlap=0.50 | 0.064% | 0 | 32 | 32 | 75.0% | 96.9% | 55343 | 89935 | 32.0s |
| 50000 | raw overlap=0.75 | 0.064% | 0 | 32 | 32 | 78.1% | 100.0% | 55537 | 89604 | 32.0s |
| 50000 | raw overlap=1.00 | 0.064% | 0 | 32 | 32 | 81.2% | 100.0% | 55808 | 89025 | 32.0s |
| 50000 | raw overlap=2.00 | 0.064% | 0 | 32 | 32 | 87.5% | 100.0% | 53929 | 79326 | 32.0s |
| 50000 | idf overlap=0.25 | 0.064% | 0 | 32 | 32 | 75.0% | 90.6% | 55290 | 85848 | 32.0s |
| 50000 | idf overlap=0.50 | 0.064% | 0 | 32 | 32 | 75.0% | 96.9% | 57388 | 84994 | 32.0s |
| 50000 | idf overlap=0.75 | 0.064% | 0 | 32 | 32 | 78.1% | 100.0% | 55387 | 85469 | 32.0s |
| 50000 | idf overlap=1.00 | 0.064% | 0 | 32 | 32 | 81.2% | 100.0% | 55414 | 84625 | 32.0s |
| 50000 | idf overlap=1.25 | 0.064% | 0 | 32 | 32 | 84.4% | 100.0% | 57093 | 86197 | 32.0s |
| 50000 | idf overlap=1.50 | 0.064% | 0 | 32 | 32 | 84.4% | 100.0% | 54880 | 83027 | 32.0s |
| 50000 | shipped + coverage=0.10 | 0.064% | 0 | 32 | 32 | 75.0% | 90.6% | 55786 | 89194 | 32.0s |
| 50000 | shipped + coverage=0.25 | 0.064% | 0 | 32 | 32 | 75.0% | 96.9% | 56670 | 76971 | 32.0s |
| 50000 | shipped + coverage=1.00 | 0.064% | 0 | 32 | 32 | 84.4% | 100.0% | 54782 | 78761 | 32.0s |
| 50000 | idf overlap=1.00 + coverage=0.25 | 0.064% | 0 | 32 | 32 | 84.4% | 100.0% | 53998 | 85000 | 32.0s |
| 50000 | idf overlap=1.00 + coverage=1.00 | 0.064% | 0 | 32 | 32 | 87.5% | 100.0% | 52594 | 85156 | 32.0s |
| 100000 | shipped | 0.032% | 0 | 32 | 32 | 71.9% | 90.6% | 102450 | 131254 | 59.6s |
| 100000 | raw overlap=0.50 | 0.032% | 0 | 32 | 32 | 71.9% | 96.9% | 105191 | 132646 | 59.6s |
| 100000 | raw overlap=0.75 | 0.032% | 0 | 32 | 32 | 71.9% | 100.0% | 105390 | 133867 | 59.6s |
| 100000 | raw overlap=1.00 | 0.032% | 0 | 32 | 32 | 75.0% | 100.0% | 106186 | 133005 | 59.6s |
| 100000 | raw overlap=2.00 | 0.032% | 0 | 32 | 32 | 81.2% | 100.0% | 102625 | 135078 | 59.6s |
| 100000 | idf overlap=0.25 | 0.032% | 0 | 32 | 32 | 71.9% | 90.6% | 104485 | 137243 | 59.6s |
| 100000 | idf overlap=0.50 | 0.032% | 0 | 32 | 32 | 71.9% | 96.9% | 104169 | 127146 | 59.6s |
| 100000 | idf overlap=0.75 | 0.032% | 0 | 32 | 32 | 71.9% | 100.0% | 103385 | 131222 | 59.6s |
| 100000 | idf overlap=1.00 | 0.032% | 0 | 32 | 32 | 75.0% | 100.0% | 104820 | 141549 | 59.6s |
| 100000 | idf overlap=1.25 | 0.032% | 0 | 32 | 32 | 81.2% | 100.0% | 103294 | 138868 | 59.6s |
| 100000 | idf overlap=1.50 | 0.032% | 0 | 32 | 32 | 81.2% | 100.0% | 101192 | 137395 | 59.6s |
| 100000 | shipped + coverage=0.10 | 0.032% | 0 | 32 | 32 | 71.9% | 90.6% | 103343 | 135938 | 59.6s |
| 100000 | shipped + coverage=0.25 | 0.032% | 0 | 32 | 32 | 71.9% | 96.9% | 100508 | 127896 | 59.6s |
| 100000 | shipped + coverage=1.00 | 0.032% | 0 | 32 | 32 | 81.2% | 100.0% | 100303 | 129115 | 59.6s |
| 100000 | idf overlap=1.00 + coverage=0.25 | 0.032% | 0 | 32 | 32 | 81.2% | 100.0% | 103098 | 127511 | 59.6s |
| 100000 | idf overlap=1.00 + coverage=1.00 | 0.032% | 0 | 32 | 32 | 81.2% | 100.0% | 106564 | 123753 | 59.6s |

## Per size, every configuration

**1000 memories** (32 gold rows, 3.200% signal share, build 0.6s)

| configuration | BM25 | overlap | coverage | idf | R@1 | R@5 | p50 us | p95 us |
|---|---|---|---|---|---|---|---|---|
| shipped | 1.00 | 0.25 | 0.00 | no | 71.9% | 96.9% | 2164 | 3263 |
| raw overlap=0.50 | 1.00 | 0.50 | 0.00 | no | 71.9% | 96.9% | 2230 | 4179 |
| raw overlap=0.75 | 1.00 | 0.75 | 0.00 | no | 71.9% | 100.0% | 2194 | 4042 |
| raw overlap=1.00 | 1.00 | 1.00 | 0.00 | no | 78.1% | 100.0% | 2192 | 4106 |
| raw overlap=2.00 | 1.00 | 2.00 | 0.00 | no | 84.4% | 100.0% | 2183 | 3549 |
| idf overlap=0.25 | 1.00 | 0.25 | 0.00 | yes | 71.9% | 96.9% | 2118 | 3732 |
| idf overlap=0.50 | 1.00 | 0.50 | 0.00 | yes | 71.9% | 96.9% | 2156 | 4055 |
| idf overlap=0.75 | 1.00 | 0.75 | 0.00 | yes | 71.9% | 100.0% | 2166 | 2912 |
| idf overlap=1.00 | 1.00 | 1.00 | 0.00 | yes | 78.1% | 100.0% | 2138 | 4826 |
| idf overlap=1.25 | 1.00 | 1.25 | 0.00 | yes | 78.1% | 100.0% | 2166 | 3861 |
| idf overlap=1.50 | 1.00 | 1.50 | 0.00 | yes | 81.2% | 100.0% | 2201 | 3823 |
| shipped + coverage=0.10 | 1.00 | 0.25 | 0.10 | no | 71.9% | 96.9% | 2176 | 4722 |
| shipped + coverage=0.25 | 1.00 | 0.25 | 0.25 | no | 71.9% | 96.9% | 2175 | 3187 |
| shipped + coverage=1.00 | 1.00 | 0.25 | 1.00 | no | 78.1% | 100.0% | 2176 | 3156 |
| idf overlap=1.00 + coverage=0.25 | 1.00 | 1.00 | 0.25 | yes | 78.1% | 100.0% | 2174 | 4036 |
| idf overlap=1.00 + coverage=1.00 | 1.00 | 1.00 | 1.00 | yes | 84.4% | 100.0% | 2164 | 4099 |

**10000 memories** (32 gold rows, 0.320% signal share, build 6.2s)

| configuration | BM25 | overlap | coverage | idf | R@1 | R@5 | p50 us | p95 us |
|---|---|---|---|---|---|---|---|---|
| shipped | 1.00 | 0.25 | 0.00 | no | 75.0% | 84.4% | 15172 | 25553 |
| raw overlap=0.50 | 1.00 | 0.50 | 0.00 | no | 75.0% | 90.6% | 15693 | 33171 |
| raw overlap=0.75 | 1.00 | 0.75 | 0.00 | no | 75.0% | 100.0% | 15062 | 26778 |
| raw overlap=1.00 | 1.00 | 1.00 | 0.00 | no | 75.0% | 100.0% | 16021 | 28084 |
| raw overlap=2.00 | 1.00 | 2.00 | 0.00 | no | 84.4% | 100.0% | 15791 | 27474 |
| idf overlap=0.25 | 1.00 | 0.25 | 0.00 | yes | 75.0% | 84.4% | 16045 | 26784 |
| idf overlap=0.50 | 1.00 | 0.50 | 0.00 | yes | 75.0% | 90.6% | 15545 | 27307 |
| idf overlap=0.75 | 1.00 | 0.75 | 0.00 | yes | 75.0% | 100.0% | 15475 | 28210 |
| idf overlap=1.00 | 1.00 | 1.00 | 0.00 | yes | 75.0% | 100.0% | 15204 | 32790 |
| idf overlap=1.25 | 1.00 | 1.25 | 0.00 | yes | 81.2% | 100.0% | 16316 | 38440 |
| idf overlap=1.50 | 1.00 | 1.50 | 0.00 | yes | 84.4% | 100.0% | 15628 | 24882 |
| shipped + coverage=0.10 | 1.00 | 0.25 | 0.10 | no | 75.0% | 84.4% | 15285 | 25923 |
| shipped + coverage=0.25 | 1.00 | 0.25 | 0.25 | no | 75.0% | 90.6% | 14856 | 25055 |
| shipped + coverage=1.00 | 1.00 | 0.25 | 1.00 | no | 81.2% | 100.0% | 14790 | 26264 |
| idf overlap=1.00 + coverage=0.25 | 1.00 | 1.00 | 0.25 | yes | 81.2% | 100.0% | 14902 | 28646 |
| idf overlap=1.00 + coverage=1.00 | 1.00 | 1.00 | 1.00 | yes | 84.4% | 100.0% | 15030 | 31293 |

**50000 memories** (32 gold rows, 0.064% signal share, build 32.0s)

| configuration | BM25 | overlap | coverage | idf | R@1 | R@5 | p50 us | p95 us |
|---|---|---|---|---|---|---|---|---|
| shipped | 1.00 | 0.25 | 0.00 | no | 75.0% | 90.6% | 54500 | 98835 |
| raw overlap=0.50 | 1.00 | 0.50 | 0.00 | no | 75.0% | 96.9% | 55343 | 89935 |
| raw overlap=0.75 | 1.00 | 0.75 | 0.00 | no | 78.1% | 100.0% | 55537 | 89604 |
| raw overlap=1.00 | 1.00 | 1.00 | 0.00 | no | 81.2% | 100.0% | 55808 | 89025 |
| raw overlap=2.00 | 1.00 | 2.00 | 0.00 | no | 87.5% | 100.0% | 53929 | 79326 |
| idf overlap=0.25 | 1.00 | 0.25 | 0.00 | yes | 75.0% | 90.6% | 55290 | 85848 |
| idf overlap=0.50 | 1.00 | 0.50 | 0.00 | yes | 75.0% | 96.9% | 57388 | 84994 |
| idf overlap=0.75 | 1.00 | 0.75 | 0.00 | yes | 78.1% | 100.0% | 55387 | 85469 |
| idf overlap=1.00 | 1.00 | 1.00 | 0.00 | yes | 81.2% | 100.0% | 55414 | 84625 |
| idf overlap=1.25 | 1.00 | 1.25 | 0.00 | yes | 84.4% | 100.0% | 57093 | 86197 |
| idf overlap=1.50 | 1.00 | 1.50 | 0.00 | yes | 84.4% | 100.0% | 54880 | 83027 |
| shipped + coverage=0.10 | 1.00 | 0.25 | 0.10 | no | 75.0% | 90.6% | 55786 | 89194 |
| shipped + coverage=0.25 | 1.00 | 0.25 | 0.25 | no | 75.0% | 96.9% | 56670 | 76971 |
| shipped + coverage=1.00 | 1.00 | 0.25 | 1.00 | no | 84.4% | 100.0% | 54782 | 78761 |
| idf overlap=1.00 + coverage=0.25 | 1.00 | 1.00 | 0.25 | yes | 84.4% | 100.0% | 53998 | 85000 |
| idf overlap=1.00 + coverage=1.00 | 1.00 | 1.00 | 1.00 | yes | 87.5% | 100.0% | 52594 | 85156 |

**100000 memories** (32 gold rows, 0.032% signal share, build 59.6s)

| configuration | BM25 | overlap | coverage | idf | R@1 | R@5 | p50 us | p95 us |
|---|---|---|---|---|---|---|---|---|
| shipped | 1.00 | 0.25 | 0.00 | no | 71.9% | 90.6% | 102450 | 131254 |
| raw overlap=0.50 | 1.00 | 0.50 | 0.00 | no | 71.9% | 96.9% | 105191 | 132646 |
| raw overlap=0.75 | 1.00 | 0.75 | 0.00 | no | 71.9% | 100.0% | 105390 | 133867 |
| raw overlap=1.00 | 1.00 | 1.00 | 0.00 | no | 75.0% | 100.0% | 106186 | 133005 |
| raw overlap=2.00 | 1.00 | 2.00 | 0.00 | no | 81.2% | 100.0% | 102625 | 135078 |
| idf overlap=0.25 | 1.00 | 0.25 | 0.00 | yes | 71.9% | 90.6% | 104485 | 137243 |
| idf overlap=0.50 | 1.00 | 0.50 | 0.00 | yes | 71.9% | 96.9% | 104169 | 127146 |
| idf overlap=0.75 | 1.00 | 0.75 | 0.00 | yes | 71.9% | 100.0% | 103385 | 131222 |
| idf overlap=1.00 | 1.00 | 1.00 | 0.00 | yes | 75.0% | 100.0% | 104820 | 141549 |
| idf overlap=1.25 | 1.00 | 1.25 | 0.00 | yes | 81.2% | 100.0% | 103294 | 138868 |
| idf overlap=1.50 | 1.00 | 1.50 | 0.00 | yes | 81.2% | 100.0% | 101192 | 137395 |
| shipped + coverage=0.10 | 1.00 | 0.25 | 0.10 | no | 71.9% | 90.6% | 103343 | 135938 |
| shipped + coverage=0.25 | 1.00 | 0.25 | 0.25 | no | 71.9% | 96.9% | 100508 | 127896 |
| shipped + coverage=1.00 | 1.00 | 0.25 | 1.00 | no | 81.2% | 100.0% | 100303 | 129115 |
| idf overlap=1.00 + coverage=0.25 | 1.00 | 1.00 | 0.25 | yes | 81.2% | 100.0% | 103098 | 127511 |
| idf overlap=1.00 + coverage=1.00 | 1.00 | 1.00 | 1.00 | yes | 81.2% | 100.0% | 106564 | 123753 |


**The pool held every gold row at every size and every configuration**, so R@1 and R@5 below are ranking results and nothing was lost to the candidate window. R@k is deterministic for a given seed, so a movement in them is a changed corpus or a changed ranking, never measurement noise.

**3.1 percentage points per query.** A move of one or two points in R@1 or R@5 is one question, not a trend.

Every requested size was built and measured.

## What this does not measure

- Recall quality on human text. Generated strings and a nonce token are not LongMemEval sessions: R@1 here says a retrieval pipeline finds an identifiable needle in a haystack of a given size, not that it answers natural multi-session questions. `eval/RESULTS.md` stays the human-text number; this does not replace it.
- The tag-filtered recall path, `reflect`, the lifecycle routes, or non-default budgets.
- Concurrent readers — recall latency here is single-threaded; `examples/bench_concurrency.rs` is where contention is measured.
- Write cost. The corpus is built with `Store::put` outside every timed region; `eval/BENCH_WRITE.md` prices the write path.
- First-touch cost. Latency is the median of warm passes on an already-built store; `examples/bench_coldstart.rs` reports the first query separately.

## Limitations

- R@k is binary per query, so at 32 queries one query is 3.1 percentage points.
- Distractors repeat one of four topic sentences, so a 100k bank is a repetition of a small vocabulary. Real text has a longer tail, and BM25's IDF weighting over four topic words is not the same as over a large one. The consequence is stated where it matters: the *pool* behaviour measured here transfers to real text, the latency magnitude does not, and the R@k scores may not either.
- A gold row's nonce is unique by construction, so a defect in the BM25 stream surfaces as R@1 loss here. A defect that only affects rows sharing every term with a competitor will not.
- Build time grows with size and a large size can take minutes. Build seconds are in the table so a slow row is visible instead of skipped.
