# Secrets: what stops a leak, and what to do when one leaks

This repository is **public**. A committed credential is public the moment it is
pushed, and deleting it later does not revoke it. So the order is fixed:
**rotate first, then clean up.** A rewritten history with a live credential is
still a leak: every clone, fork and cached view made before the rewrite keeps it.

## 1. What stops a leak, and what each layer cannot stop

| layer | where it runs | catches | does not catch |
|---|---|---|---|
| Generator redaction | `eval/relevance/run_battery.py` (`SECRET`) | secret-shaped text in a committed battery report | other generators; patterns not yet mirrored in `scripts/secret-scan.sh` |
| Product capture redaction | `src/capture.rs` `redact_pii`, run on retain (`src/api.rs`) | the patterns it lists: `sk-`, `ghp_`, `AKIA`, `AIza`, `xox`, Bearer, JWT, PEM, email | Telegram bot tokens — no pattern (open gap, §3) |
| pre-commit | `.githooks/pre-commit` → `scripts/secret-scan.sh --staged` | secret-shaped lines being added | a clone without `core.hooksPath`; `git commit --no-verify` |
| commit-msg | `.githooks/commit-msg` → `--message FILE` | the same, in the commit message | same as pre-commit |
| release gate | `scripts/build-release.sh` → `--all` | any tracked secret-shaped line, at tag time, including `--check` | anything not tracked (gitignored files) |

Enable the local gates once per clone:

```bash
git config core.hooksPath .githooks
```

`scripts/secret-scan.sh` reports `file:line` and never the value. A line carrying
`secret-scan:allow` is skipped; a file of deliberate fake secrets goes in its
`EXCLUDE` list (today: `src/capture.rs`, `src/vector.rs`, the scanner itself).
`--selftest` checks the pattern still matches a synthetic token.

## 2. Responding to a leak

Work in this order.

1. **Rotate or revoke the credential at its issuer.** For a Telegram bot token:
   BotFather `/revoke`, then update every consumer. Only the owner can do this.
2. **Find every copy, counting without printing.**
   - Working tree: `scripts/secret-scan.sh --all`.
   - History: `git log --all -G'<pattern>' --format=%h` lists commits whose diff
     touches the pattern; `git grep -cE '<pattern>' <sha>` per commit.
   - Memory bank: hooks capture prompts, so a pasted credential lands in the bank.
     Find it with the pattern and delete through the API. Owner's call.
   - Gitignored files (`eval/data/**`, `eval/omnimevaleval/**`): grep them. Do not
     commit them to make the grep pass.
3. **Rewrite history in a mirror clone, never in a working tree with uncommitted
   work.** `git filter-repo` resets the tree it runs in.

   ```bash
   git clone --mirror <repo> <scratch>/mw.git
   cd <scratch>/mw.git
   # rules file, one line:  regex:<pattern>==>REDACTED-<KIND>
   git filter-repo --force --replace-text <rules>
   ```

   Verify before pushing: zero commits whose tree matches the pattern, zero commit
   messages that match, and `git diff --name-only <old> <new>` lists only the files
   that held the secret.
4. **Push the rewritten refs with leases pinned to the SHAs you read first.**

   ```bash
   git push --force-with-lease=refs/heads/main:<old-main> \
            --force-with-lease=refs/tags/<tag>:<old-tag-object> \
            origin refs/heads/main refs/tags/<tag>
   ```

   Every tag whose ancestry changed must move too: a tag left on an old commit
   keeps the secret reachable. Tags whose ancestry did not change keep their SHA.
   GitHub Releases follow the tag name. A force-push changes what everyone who
   cloned sees, so it needs the owner's go-ahead.
5. **Sync local clones.** `git fetch origin`, then `git reset --mixed origin/main`
   (keeps the working tree; `--hard` would discard uncommitted edits), then
   `git reflog expire --expire=now --all && git gc --prune=now`.
6. **Map the SHAs.** Any doc that cites a rewritten commit now cites a SHA that no
   longer names the same content. Record the mapping in §4 and add a note where the
   SHA is cited. `docs/CONSISTENCY.md` is append-only: add the note there, do not edit.
7. **Record the incident in §4.**

A rewrite cannot reach copies that already exist: other clones, forks, anyone who
fetched, and GitHub's cached views of unreachable objects. That is why rotation
comes first, and why the rewrite is cleanup rather than the fix. Purging GitHub's
cache needs GitHub Support.

## 3. Open gaps

- **Product redaction has no Telegram bot-token pattern.** `src/capture.rs` lists
  no `<digits>:AA…` shape, so the token in §4 was retained into the `omp` bank
  unredacted. Fix: add `[0-9]{8,10}:AA[A-Za-z0-9_-]{30,}` to the patterns in
  `src/capture.rs`, with a test. That is a product change and needs the full gate
  (`cargo build/test/clippy/doc --locked`) and a version bump, so it is not in the
  change that wrote this document.
- **GitHub secret-scanning push protection** is a repository setting. Nothing in
  this repo enables it. It is the only layer that stops a push from a clone with no
  hooks. Owner's decision.
- **Fresh clones have no gates until `core.hooksPath` is set.** No script sets it
  for you. §1 says so, and `AGENTS.md` §2 repeats it for agents.

## 4. Incident log

### 2026-10-18 — Telegram bot token in a committed battery artifact

- **What leaked.** A bot token pasted into a prompt was captured into the `omp`
  bank. The frozen relevance battery `eval/relevance/BATTERY-2026-10-16-frozen.txt`,
  added in `06b5bd7`, rendered that row verbatim. The repository is public, and
  `06b5bd7` was on `origin/main`. Tag `v0.9.0` (`cde07e1`) carried the same file.
- **Fixed in the tree.** The renderer now redacts (`0efea30`). The artifact was
  regenerated from its frozen snapshot rather than edited (CONSISTENCY §49).
- **Removed from history.** The token-shaped text was replaced with
  `REDACTED-TELEGRAM-BOT-TOKEN` in a mirror clone with `git filter-repo
  --replace-text`. Verified: zero commits and zero commit messages match the
  pattern. Only `eval/relevance/BATTERY-2026-10-16-frozen.txt` changed content in
  the rewritten commits.
- **Pushed (force, lease-checked).** `main` `20e71a3` → `f170bf7`. Tag `v0.9.0`
  object `109e47e` → `3557aca` (peeled `cde07e1` → `17d148a`). The other tags and
  `origin/ci-musl-build` were unchanged and not pushed. The `v0.9.0` release binary
  was built before the rewrite; the rewrite changed only the battery text, which is
  not compiled into the binary.
- **Commit map, old → new** (7 characters; full SHAs are in the mirror's
  `filter-repo/commit-map`):

  | old | new | commit |
  |---|---|---|
  | `06b5bd7` | `d00d1f6` | records: the 2026-10-16/17 R&D cycle (adds the battery) |
  | `37661ce` | `2ebc091` | feat: the H12 serving shape |
  | `20e71a3` | `2474bc4` | docs: closing record for the R&D cycle |
  | `5456ea7` | `8818906` | docs: serving shape in the README/agent-skill |
  | `cde07e1` | `17d148a` | bump: 0.9.0 (tag `v0.9.0`) |
  | `0efea30` | `4189b30` | fix(eval): stop the battery artifact echoing the token |
  | `0f1309f` | `f170bf7` | chore: block credentials at commit time |

  Cited in old docs: `docs/CONSISTENCY.md` (`06b5bd7`) and
  `docs/handoff/CLOSING-2026-10-17.md` (`06b5bd7`, `37661ce`).
- **Still the owner's.**
  1. Revoke the token. Every copy made before the rewrite still holds it, so the
     revocation is what closes the leak.
  2. Delete the `omp` bank row that holds it. The gitignored snapshot
     `eval/data/battery/omp-snapshot.jsonl` also holds the value. Changing it changes
     the frozen input, so any re-freeze must cite a new sha256.
  3. Other clones of this repository still hold the old history until they fetch
     and reset.
  4. Enable push protection (§3), and ask GitHub Support to purge cached views if
     the token was not rotated.
- **Checked.** The 244 MB run tree `eval/omnimevaleval/memory-wire-mw_pmv2_dated/`
  contains no Telegram-shaped token. Its generic matches (`sk-…` in 49 files,
  `api_key`-style in 7) were counted, not read one by one. The tree is gitignored,
  so none of them can reach a commit, but a credential found there would still need
  rotating.
