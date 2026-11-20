# B-DELTA — what landed after round 4 was sent

Round 4 was sent **2026-11-19 23:52** (`SEND.log`, page 263). Everything below
is after that. Round 4's two live claims were checked against the tree and both
are already closed; the delta records that, because a reviewer told "still
broken" when it is fixed will spend the round on a stale defect.

## D0. Round 4's own claims, re-checked — both closed

| round 4 claimed | tree now | evidence |
|---|---|---|
| "`extension/memory-wire.ts` lines 224–240 still filter `untrusted_external` **after** slicing the first `RECALL_ENTRIES` — an active N−1 bug today" | **closed** — `ed8eea4` (2026-11-20 00:20), 28 minutes after the send. The loop is now `for (var i = 0; i < items.length; i++)` with `if (isUntrusted(item)) continue;` and `if (lines.length >= RECALL_ENTRIES) break;` — filter first, take on *output* | `plugin/integrations/extension/memory-wire.ts` `formatRecall`, read 2026-11-20 |
| "TODO.md row 18 still reads `named-not-scheduled` despite completing 2026-10-17" | **closed** — `e0f1bb4` | `TODO.md` row 18 is `done` |

So round 4's *conclusion* (the N−1 shape recurs) is not contradicted, but its
**instances** are gone. Do not re-litigate either.

## D1. The opencode door had never loaded on its host

`3376b5c`. Found by probing the question round 4 could not answer (does either
TS host resolve imports?), not by reading.

- The shipped plugin exported a **v1 bare function**. opencode 2.x requires
  `{id, setup}` → `PluginModule.LoadError` on 2.3.2.
- It was installed as a **flat file**. opencode 2.x discovers plugins as
  *directories* and reads `<dir>/index.ts`; a flat file is skipped with **no
  error and no log line at all**.
- Fixed both; version marker 2→3; legacy flat file removed on install and
  uninstall. Verified live on 2.3.2, plus a stub-ctx drive proving injection and
  the retain round-trip.
- `session.idle` does **not** fire under `opencode run`; the capture hook is
  `session.execution.succeeded`.

**Why this is in the delta and not a footnote:** the round-4 review looked at
this door and called it healthy. It was dead on the host it named. That is the
same "checked in N−1 places" shape one layer out — the check covered what
reached a model and could not see that *nothing* did.

## D2. `connect_common` — six byte-identical helpers extracted

`2bfe837`. Measured first: grepping every `fn` signature in `src/connect*.rs`
and grouping by name found 11 repeats, of which **six were byte-identical
bodies** (diffed per pair, not inferred). Five are extracted; the sixth,
`version`, is left in place and named below:

| helper | copies | what it encodes |
|---|---|---|
| `mcp_url` | claude, copilot, **codex** (3) | the bank is a `/mcp/<bank>` path segment, because the stateless mount pins the subtree |
| `baked` | ext, opencode | render the template with the bank baked into `DEFAULT_BANK` |
| `carries_marker` | ext, opencode | ownership = both markers, not the name |
| `is_ours` | ext, opencode | an unreadable file is not ours |
| `preserve_comment` | codex, hermes | keep the user's trailing `# comment` on a rewritten line |

The sixth byte-identical pair is `version` (ext, opencode). Its two copies differ
by a **comment** only — the body is identical — and it stayed in place because
the shared form would have to carry both comments or drop one. Recorded here so
the count is checkable rather than asserted.

Net **−138 lines** in the six modules, +177 for `connect_common.rs` (incl. its
tests). Callers import directly; no same-named wrappers.

**Two name collisions were deliberately NOT merged**, and the reason matters
for anyone tempted to "finish the job":

- `connect::is_ours(&Value)` tests a JSON entry; `connect_ext::is_ours(&Path)`
  reads a file. Merging means serializing a value to a string it already holds.
- `version` and `prune_empty_dirs` differ between ext and opencode by the
  directory they stop at (`extensions` vs `plugins`) and by how they treat a
  non-empty directory.

## D3. `memory_redact` on MCP — the sixth tool

`b3ecc3b`. TODO #34, the door-parity gap that was a **lockout**: an agent that
retained a secret through MCP could not retract it through MCP.

`MemoryService::redact_memory` and the `readOnly` guard already existed, so this
was a registration, a dispatch arm, and tests.

**The test caught a defect in itself, twice:**

1. It first used `sk-live-deadbeef` as the secret and got `redacted_rows: 0`.
   `capture::redact_pii` runs at **retain** time and its OpenAI rule
   (`sk-[A-Za-z0-9_-]{8,}`) eats that string before it reaches the store. A test
   built on it would have passed a tool that never had to work. It now seeds
   prose that survives the retain-time redactor, proves it is there by reading
   it back, then redacts. `memory_redact` exists for the *second* pass.
2. The tool's `pattern` is a **regex**, not a literal (the store compiles it and
   refuses an invalid one). The schema said "literal text" — wrong about the
   interface a client builds against.

Nine places stated the tool count and all said five or four; all corrected.

## D4. `doctor` names every bank

`6a2a125`. C2's deliverable. `collect` already computed per-bank counts and
threw them away, keeping only the single largest when the resolved bank is
empty. The screen now lists every bank with its count, largest first, and lists
the **resolved** bank even when it holds nothing (`<- resolved here, holds
nothing`) — a list built from `memories` rows cannot contain a zero-row bank.

Nothing else: no migration, no auto-move, no recommendation. Suppressed below
two banks.

## D5. Three gates that were wired to nothing

- `131cffb` — `plugin/integrations/trust_gate_test.sh` wired into
  `.githooks/pre-commit` (behind `command -v node`) and `ci.yml`. It had been
  running nowhere; both bugs it catches were found by hand.
- `4646e7a` — an **export-shape** check per door in the same script (opencode
  must be an object with `id`/`setup`; omp/pi must be a function). This is the
  check whose absence let D1 ship.
- `9e95d9b` — `scripts/module_table_check.sh`, which compares AGENTS.md §3's
  module set and per-module counts to the tree. **It caught its first real
  drift on the very next commit** (`2bfe837` added a module and shifted seven
  counts) and again on `b3ecc3b`.

## D6. Round 4's falsifiers, now measurable

Round 4 said it could not tell whether `connect_*` is mostly inline test
fixtures or duplicated wiring, and named the falsifier: ">50% of the 6,609 lines
are per-host inline `#[test]` fixtures." That split is now measured
(`measure_surface.sh`, §code-vs-test):

```
connect.rs          total=2844 code=1328 test=1516
connect_plugin.rs   total=1092 code=555  test=537
connect_codex.rs    total=821  code=387  test=434
connect_ext.rs      total=795  code=256  test=539
connect_opencode.rs total=384  code=258  test=126
connect_claude.rs   total=312  code=160  test=152
connect_copilot.rs  total=293  code=177  test=116
connect_common.rs   total=177  code=108  test=69
```

**8 modules, 6,718 lines: 3,229 code / 3,489 test — 52% test.** Round 4's
falsifier is therefore *satisfied on its own terms*, which is the evidence it
asked for and one of the two readings it could not choose between. The round
should treat that as an input, not a verdict: 52% is a test-to-code ratio, and
the question it bears on is whether a shared abstraction would save real
production lines or only fixture lines.

## D7. What did NOT change (so the reviewer does not re-derive it)

- No new dependency. Still 17 in `[dependencies]`, `DT_NEEDED`=3.
- `src/vector.rs` (965) + `src/embed.rs` (177) still compile to **zero bytes**
  in a default build (`#[cfg(feature = "embed")]`, `default = []`).
- The recall pipeline is untouched: FTS5 BM25 + overlap + RRF, budget trim.
  No ranking change, no weight change, no new signal.
- The trust gate's read-time filter is unchanged since `1f5ee17` except that it
  now exists in all four auto-injection doors.
- `docs/OPEN_HOOK_RECALL_RELEVANCE.md` is still deliberately unfixed: no score
  threshold, ever (CONSISTENCY §32/§48).
- Footprint: **11,166,048 B = 10.65 MiB**, `DT_NEEDED`=3, at `6a2a125`.
  Round 4 recorded 11,141,240 B (10.63 MiB), so the delta across D2–D4 is
  **+24,808 B (+24.2 KiB)** — corrected 2026-11-20 after the round-5 review
  recomputed it. `AGENTS.md` §1 still carried the older figure and is corrected
  in the same commit.
