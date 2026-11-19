# Outline plan — the remaining implementation gaps

Written 2026-11-19, after round-3 context-pack review closed at `98b1a36`.
Every row below was checked against the tree or a command before it was written;
where a register row and the code disagreed, the code won and the disagreement is
named.

**This is an outline, not a run.** Nothing here has been implemented. Rows are
ordered by (1) whether anything blocks them, (2) whether they are product work or
research spend, (3) whether they have a proof that can fail.

---

## 0. What the registers say, and where they are wrong

Three sources claim to be the list: `TODO.md` (32 rows, 26 done), its domain docs
(`INTEGRATION_GAPS.md`, `INTEGRATION_PLAN.md`), and `docs/handoff/RND_PLAN.md`
§6. They disagree. Resolving that is step zero, because a plan built on a stale
row sends work to a place that is already done.

| claim | source | check | verdict |
|---|---|---|---|
| P2.1 open (golden-fixture tier) | `TODO.md` row 4 | `connect::tests::golden_install_shapes` exists, `src/connect.rs:1351` | **stale.** The plan's own header (2026-10-15) already says done; the TODO row was never updated |
| P4.1 open (hermes slot research) | `TODO.md` row 4 | plan header: "P4.1 done 2026-10-15 — the slot binds by directory name per hermes' own `find_provider_dir`" | **stale**, same cause |
| P3.1 recorder missing | `INTEGRATION_PLAN.md` P3.1 step 3 | `examples/bench_footprint.rs:69` reads `DT_NEEDED` via `readelf` | **stale.** Only the v0.5.0 byte-compare is unrun |
| G6.8 open (no `system_prompt_block`) | `INTEGRATION_GAPS.md` "What is left" | `plugin/integrations/hermes/memory-wire/__init__.py:458` implements it | **stale.** The method exists and is documented as static-by-contract |
| G4 open (bank opt-in) | `INTEGRATION_GAPS.md` | no `opt_in` code in `src/` | **genuinely open, and a policy decision, not a bug** |
| TODO #31 open (door parity) | `TODO.md` row 31 | verified table in `INTEGRATION_GAPS.md` addendum | **genuinely open, and explicitly a surface decision** |

**The pattern:** every stale row is one where the work landed and one of two
registers was updated. That is the same class of defect round 3 found in
`measure_surface.sh` — a count that stopped being maintained — one level up, in
the plan of record itself. The fix is cheap and worth doing first.

---

## 1. The one finding round 3 left half-closed — **DONE 2026-11-19**

> **Done.** `plugin/integrations/trust_gate_test.sh` now runs all three doors
> from one entry point, and `plugin/integrations/hermes/trust_gate_test.py`
> drives the real `MemoryWireProvider.prefetch` against a local HTTP stub.
> Mutation-checked per door: removing the hermes filter fails the hermes check,
> removing the extension filter fails only its own. Record: CONSISTENCY §55.

Round 3 added the trust gate to both TypeScript plugins and left proof for them
(`plugin/integrations/trust_gate_test.sh`). Checking the other doors:

| door | gate present | executable check |
|---|---|---|
| `src/hooks.rs` | yes | yes — Rust tests, incl. a full-chain test |
| `plugin/integrations/extension/memory-wire.ts` | yes | yes — `trust_gate_test.sh` |
| `plugin/integrations/opencode-plugin/memory-wire.ts` | yes | yes — `trust_gate_test.sh` |
| `plugin/integrations/hermes/memory-wire/__init__.py` | **yes** (`UNTRUSTED_TIER`, line 110; filter at 516) | **no** |

The hermes gate is real and correctly written — it even filters before the take,
with the same reasoning. It is the one door with the policy and no executable
check, and `trust_gate_test.sh` only covers the two `.ts` files because that is
what it was written for.

**Item 1 — extend the proof to the fourth door.** Small, mechanical, and it
closes the shape round 3 identified rather than one instance of it. The
alternative — leaving it — means the next person to touch `prefetch` finds out
the hard way.

---

## 2. A second real gap the review did not name: three doors serve dates, one does not

Found while checking whether DECISIONS #1 ("serve occurrence dates in the
surfaces") was still open. It is, and unevenly:

| door | requests `format` | prefixes `[<date>]` |
|---|---|---|
| `src/hooks.rs` | `full` / `summary` | **yes** — `recall_section`, line ~766, with a documented H12 rationale |
| hermes plugin | `full` | **no** — `_recall` drops `created_at` entirely, `prefetch` emits `- {content}` |
| extension `.ts` | `full` | **no** |
| opencode `.ts` | `full` | **no** |

`src/hooks.rs` carries the reason in a comment: H12 measured +13.0pp overall on
HaluMem from the date prefix alone, which is the largest single lever the cycle
found. The wire has carried `created_at` on every `format: "full"` hit since
TODO #15 shipped. So three doors are serving an instrument the Rust door already
proved is worth serving — and each drops the field on the floor.

This is **not** a ranking change and not a threshold: it is a rendering change
downstream of ranking, byte-identical when the wire carries no date. It is the
same class as the trust gate — one policy, four doors, unevenly enforced — and it
is the higher-value half.

**Item 2 — serve dates on the remaining three doors.** One small change per
door, gated on `created_at` being present so an old server degrades to today's
exact bytes. Proof: an end-to-end run per door against a live server, asserting
the canary-free block carries `[YYYY-MM-DD]` when the wire does and does not when
it does not.

---

## 3. The capsule knob stops at the Rust door

DECISIONS #1 also recommended "capsules per-bank". The mechanism shipped:
`bank_injection_format_at` reads a bank's `injectionFormat` config key, with
`MEMORY_WIRE_FORMAT` as fallback, and `format: "summary"` serves H8 capsules.

It exists in `src/hooks.rs` only. Neither hermes nor the two TS plugins read the
key or the env, so a bank configured for capsules gets them from the Rust hook
and full text everywhere else.

**Item 3 — either honour the knob on all doors, or document it as Rust-only.**
The first is a small patch per door. The second is a doc line and is defensible:
the knob is young, and a surface that ignores it silently is worse than one that
is documented not to have it. Decide before implementing — this is the one row
here that is a product call rather than a mechanical port.

---

## 4. What is genuinely blocked, and should stay blocked

These are open in the registers and I am **not** proposing work on them. Each is
blocked on the user, on money, or on another repo, and each says so in its own
row.

| item | why it stays |
|---|---|
| TODO #10 — agent-memory-bench Claude Code arm | needs the user's agent-model budget. The only external benchmark grading task success with hooks+MCP wired |
| TODO #11 — LLM-judged runs (AMB `rag`, OAMB LME-60, memorybench, MemBase) | same budget gate; OAMB/LME runs are test consultations |
| TODO #12 — AMB provider writes at turn granularity | **measured and rejected on merit.** LoCoMo dev: turn-level loses to whole sessions on every aggregate (R@1 60.4% → 54.6%, n 1531); windows lose too. The row buys measurability of long corpora at a ranking cost and is explicitly not a ranking lever |
| TODO #13 — aroute bugs B1–B4 | different repo (`proxy/artificial-route`), user's. memory-wire agents do not edit aroute |
| TODO #31 — door parity | a surface decision. Redact on MCP is the widest gap and the honest answer may be a documented ruling that the doors stay asymmetric |
| G4 — bank opt-in policy | a policy decision. `BANK_IDENTITY.md` already shows a fork does not collide, so the case the parents design against is covered; the residual is two remote-less dirs sharing a basename |
| §6.4 in full — head-to-head, Track 2, paid reader, dense arm | user-gated. `DECISIONS.md` has a ranked recommendation per row |
| R&D — any new ranking lever | **closed by law.** `CLOSED.md` is the complete list; AGENTS §8 forbids re-proposing without new evidence. The six-repo audit found no incoming mechanism |

The in-house retrieval queue is empty and that is a finding, not a gap to fill.
Round 3 found no ranking lever either; it found enforcement asymmetry between
doors, which is a different and cheaper kind of problem.

---

## 5. Order of work

Each item is one batch, one commit, gated the same way (`cargo test --locked`,
clippy `-D warnings`, `cargo doc` 0 warnings, plus live proof for anything that
changes what a model receives).

```
1  Fix the stale register rows (§0)              docs only, ~30 min
   -> P2.1/P4.1/P3.1/G6.8 corrected in TODO.md and INTEGRATION_GAPS.md,
      each with the command or file:line that closes it

2  Extend trust_gate_test.sh to hermes (§1)      small, closes the shape
   -> the fourth door gets an executable check; mutation-proved

3  Serve dates on hermes + both TS (§2)          the real prize
   -> one patch per door, each degraded-safe; live proof per door

4  Capsule knob: decide then act (§3)            blocked on a product call
   -> either honour injectionFormat on all doors, or document Rust-only

5  Nothing else. Blocked rows (§4) stay blocked.
```

Items 2 and 3 are independent and could run in either order; 3 is larger and
worth more. Item 1 is first because every later item cites the register.

**Not in this plan, deliberately:** any retrieval change, any new benchmark run,
any LongMemEval consultation, any threshold. AGENTS §6 and §8 both bite, and the
round-3 evidence is that the remaining product risk is in enforcement parity
across doors, not in ordering.

---

## 6. How each item proves itself

The standard round 3 set, because it caught two things a green suite did not:

1. **Positive:** the behaviour happens, observed on a live server built from the
   tree — not a canned response.
2. **Negative:** remove the change, the check fails, restore, it passes. A test
   that passes with the code deleted is not a test.
3. **Degradation:** an old server (bare strings, no `trust`, no `created_at`)
   produces exactly today's bytes. Every gate here is evidence-on-presence, never
   evidence-on-absence.
4. **Doc agreement:** the claim and the code say the same thing. Round 3 found
   four docs that did not.

Two traps from this session worth carrying into any of it:

- **A hooks test that spawns the binary is only as current as the last explicit
  `cargo build --bin memory-wire`.** `cargo test --bin` does not rebuild it.
- **A running server may predate the code you are testing.** My first live check
  of the trust wire showed no `trust` field at all, because the server on :8888
  was a Nov 17 build. Start a fresh one from `target/` before believing a
  negative result.
