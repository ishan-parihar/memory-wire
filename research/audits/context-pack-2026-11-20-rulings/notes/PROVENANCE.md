# Provenance — rulings round, 2026-11-20

- **Question.** The implementation plan
  (`docs/plans/2026-11-20-remaining-gaps-plan.md`) ends with three decisions
  waiting on the operator. What should be decided, what would change each
  answer, and which matters most?
- **Why a rulings round and not an audit round.** Rounds 1-5 audited the system.
  This round's subject is the *plan*, and the plan's open items are decisions,
  not defects. `references/rulings-round.md` is the instrument for that.
- **Model / effort.** `claude-sonnet-5-5` at high, per the preflight receipt.
- **Subject.** memory-wire at `9b1ea80` (the pack was built at `05672ec`; the
  only later commit is the STATE correction, which the pack does not cite).
- **Pack.** 129,585 B, ~43k tokens, 9 sections. Every figure in `RULINGS.md`
  is produced by `measure_rulings.sh`, kept beside the spec.
- **Gates run before send.** `pack_drift_gate.py` — no committed pack has
  drifted; `pack_pipeline_mutation_battery.py` — 5 of 5 mutations caught.
- **Excluded, deliberately.** `target/`, every prior round under
  `research/audits/`, `eval/` outputs, and any file holding operator-private
  state or credentials (AGENTS 14). The PersonaMem run directory is gitignored
  because it wrote memory rows verbatim and one held a pasted token.
