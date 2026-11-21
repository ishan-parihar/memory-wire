# Provenance — rulings round, 2026-11-20

- **Question.** The implementation plan
  (`docs/plans/2026-11-20-remaining-gaps-plan.md`) ends with three decisions
  waiting on the operator. What should be decided, what would change each
  answer, and which matters most?
- **Why a rulings round and not an audit round.** Rounds 1-5 audited the system.
  This round's subject is the *plan*, and the plan's open items are decisions,
  not defects. `references/rulings-round.md` is the instrument for that.
- **Model / effort.** `gemini-4-hydrogen` at **default effort**, per the
  operator's ruling of 2026-11-20 ("only use gemini hydrogen model specific
  with the default effort"). Earlier rounds used `xhigh`; this one did not.
- **Subject.** memory-wire. The pack was built at `05672ec`; the reply arrived
  after `333185b` had landed, which is why its audit item 6 found the blank-row
  premise stale — the fix (`088176f`) landed while the round was in flight.
- **Pack.** 130,457 B, ~43k tokens, 9 sections. Every figure in `RULINGS.md`
  is produced by `measure_rulings.sh`, kept beside the spec.
- **Gates run before send.** `pack_drift_gate.py` — no committed pack drifted;
  `pack_pipeline_mutation_battery.py` — 5 of 5 mutations caught.
- **Send.** Three attempts. Twice the composer fill came up short (2256 of
  3613, 3331 of 4777) and the tool refused rather than posting a truncated
  prompt — the documented flaky-fill path, caught safely. The third attempt
  posted, and the reply was delivered as a chat message rather than through
  `RESPONSE.md`; it is transcribed into `RESPONSE.md` verbatim.
- **Excluded, deliberately.** `target/`, every prior round under
  `research/audits/`, `eval/` outputs, and any file holding operator-private
  state or credentials (AGENTS 14). The PersonaMem run directory is gitignored
  because it wrote memory rows verbatim and one held a pasted token.
