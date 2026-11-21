# DELTA — what landed since the 2026-11-21 rulings round

Sixteen commits after `303a096` (the last rulings round's base). Verbatim
subjects, newest first; the round should read these as **done**, not proposed.

```
55994e3 docs: move a measurement date out of prose and into the register
7069325 Outline plan, third pass: the register is empty because I keep closing it
2155adf CONSISTENCY §75 and STATE: the v3 execution record
ed96bbd Redact gets its HTTP route, and that exposed a frozen-contract gap
c425f1e Teach the count gate to read src/, where the last stale count hid
1fcde7d Fix the live stale counts, and mark both superseded plans
6ab79d6 Outline plan for the remaining gaps, with the work log the question asked for
790987d STATE: full-scale execution recorded, including what was deliberately not run
b7bee48 CONSISTENCY §74: what shipped, what was measured, and the LongMemEval run I chose not to spend
59a2919 Close TODO 36 as a decision, and record TODO 38's block with its evidence
34237bf supersede gets an HTTP route and hygiene gets a CLI, closing the asymmetry
6df26dd Doctor names the input that produced an empty bank, not just the flag
d86d200 Outline plan for the remaining gaps, after checking the ground truth
303a096 Record the execution: round-trip test, count gate, parity ruling
3a4b1a6 The round-trip test: does a retained memory reach the injected block?
3050ab2 TODO 38: pin the row-inclusion invariant, which is what actually broke
```

**Three of these change what the ASK section can claim.**

1. **`ed96bbd` — redact got its HTTP route, and testing it found a
   frozen-contract gap.** `StoreError::BadInput` (a redact `pattern` that is not
   a regex) fell through `http_error`'s catch-all to an opaque `500 storage
   error`. It now maps to `400 invalid content`. Reachable before through the CLI
   and MCP, so it is a contract correction, not a new-route detail.

2. **`c425f1e` — the count gate now scans `src/`.** The defect that motivated it
   (`mcp_http.rs` saying "four tools" in three places while serving six) survived
   review because its test asserted against `Server::tools()` rather than a
   literal: the assertion was right, only the name and the docs were wrong.

3. **`59a2919` and `34237bf` — two TODO rows closed as decisions, not as work.**
   TODO 36 (extract the TypeScript door core) was closed as *deliberately not
   extracted* with the measured reason; TODO 38's remaining half is recorded as
   **blocked** with its evidence.

**Measured after all of it (CONSISTENCY §74 and §75):**

| run | result | load |
|---|---|---|
| LoCoMo dev (free) | R@5 86.0%, R@10 94.2%, NDCG@10 73.6%, MRR 71.8%, R@pool 100.0% | 23.71 on 24 cores |
| Relevance battery (free) | H8 gate held; ranking IDENTICAL; 42% fewer auto-injected chars (16,034 vs 27,766) | — |

Both are **unchanged within rounding** from the batch before, which is the
expected result for a batch that touched doors, a doctor line, docs and a gate,
and no ranking code.

**Footprint after all of it:** release build **10.73 MiB** (was 10.65 at the
start of today), `DT_NEEDED` still **3** (`libgcc_s`, `libm`, `libc`). No
dependency added. The competitive position in AGENTS §1 holds.

**Not run, both days:** LongMemEval-S. The dataset is present and the command
works; it is a counted consultation (count = **5**) and both days' changes could
not have moved a ranking number.

**The one thing that got worse:** the installed binary is still 0.9.0 and is now
**89 commits** behind `v0.10.0` (it was 68 when this session began and 87 at the last plan).
