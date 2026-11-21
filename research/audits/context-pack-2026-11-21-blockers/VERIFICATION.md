# Verification — blockers round, 2026-11-21

Venue: `gemini-4-hydrogen/high` on aistudio, inline mode (48,046 chars posted,
20,793-char reply captured). Spec sha256 `1e3a04d2303de735…` (46,672 B), verified
against a fresh rebuild before send. Drift gate: no drift. Mutation battery: 5/5.

The send reported `TIMEOUT after 420.0s (progress 0)` — that is the tool's
stability detector never seeing a settle signal, not a truncated answer. The
captured `RESPONSE.md` ends with its own five rulings, the lens analysis and its
source list, so the reply is complete. Recorded because a `TIMEOUT` line in a log
reads like a failed round and it was not one.

## The five rulings, and what I checked each against

| ruling | Hydrogen's recommendation | checked |
|---|---|---|
| **R1 release** | (a-gated) R4(b) → bump → three-Linux → local upgrade | Correct on the facts I gave it: 89 commits untagged, all five gates green. Its added caution — running the *old* 0.9.0 binary against local state during the interim is the untested-migration risk — is right and was not in my ASK.md |
| **R2 bank** | (a) `connect --bank omp` after authorization | Correct target. I verified the pool facts independently below |
| **R3 agent-memory-bench** | (b) defer until R1+R2 land | Its reason is the one I under-weighted: paying cash to benchmark an *unreleased* 0.9.0 yields a number no tag can reproduce |
| **R4 doctor** | (b) fix the silence, do not touch pid 1295 | Matches plan v4's §R4 exactly. It also named the asymmetry as intentional, which is right — `daemon start` only manages processes it owns |
| **R5 LongMemEval** | (b) once, at the release boundary | Correct: one consultation, test partition, not an inner-loop instrument |

## One correction to my own pack

My ASK.md said the host banks resolve to `omp` and "4,246 memories". Hydrogen's R2
reasoning repeated that. While its reply was in flight I diagnosed R4 properly and
found **three** pools, not two:

| store | memories | what holds it |
|---|---:|---|
| `~/.local/share/memory-wire/memory.db` (default) | **6,262** | the running server, pid 1295, `~/.local/bin/memory-wire` 0.9.0 |
| `.../banks/omp` | 4,246 | not served — no process reads it |
| `.../banks/ishan-parihar-memory-wire` | 5 | not served — what the hooks resolve to |

So the recommendation stands but the *premise* in my pack was incomplete: the
biggest pool is the one the **running** server holds, not the one the hosts point
at. That matters for R2, because `connect --bank omp` and `daemon stop && daemon
start` are not two independent choices — the restart changes which pool is live,
and the 6,262-memory default DB would stop being served by that process. This is
the operator's call and the pack should have said so.

## Where its reasoning was weak

- It did not question whether `omp` is the right bank, because I told it to treat
  that as decided. That is the pack's design (operator-only facts are labelled as
  such) but it means R2 carries no independent scrutiny of the *choice*, only of
  the *mechanism*.
- Its "could not verify" list is thin: it flagged the kernel claim and the aroute
  service status, but not the one thing that would most change its R1 answer —
  whether the v0.9.0-migration note is still an open operator question or is
  closed by this release.
- On R5 it asserted the relevance battery's "four days old" numbers still hold.
  They were measured 2026-11-17; the round ran 2026-11-21. Nothing was committed
  that would change them, so I accept it, but it did not say how it knew.

## Verdict

Rulings are actionable and internally consistent. R1's gating of the release on
R4(b) matches plan v4's own dependency. R2's mechanism is right; its premise needs
the three-pool correction above before the user acts on it.