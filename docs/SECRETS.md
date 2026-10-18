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
| remote check | `.githooks/pre-commit` → `--remotes` | a credential embedded in a remote URL (`git remote -v`, `.git/config`) | a clone without `core.hooksPath`; a remote added after the last commit |
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

- **Product redaction had no Telegram bot-token pattern.** That is why the token in
  §4 reached the `omp` bank unredacted. `src/capture.rs` now has `TELEGRAM_RE`
  (`[0-9]{8,10}:AA[A-Za-z0-9_-]{30,}` → `[REDACTED:telegram_token]`) with two tests.
  **Status: written, not yet gated.** The sandbox that made this change could not
  reach `crates.io`, so `cargo test/clippy/doc --locked` have not run on it. Run the
  full gate before pushing that commit.
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

### 2026-10-18 follow-up — the other leaks found while auditing the projects

Found by sweeping every repo under `my-projects/` after the bot-token incident.
Counts and names only in this record; no value is written here.

- **GitLab personal access token in remote URLs.** Eight repos had it embedded in
  an `origin` or named remote URL, which is stored in plaintext in `.git/config`:
  `kali-mahabali`, `metatrader-bridge`, `Aegis`, `AlphaForge`, `AlphaTrade`,
  `timegpt-testing`, `c-suite-agents`, `c-suite-agents-mcp`. The URLs are now clean.
  The same token is in `~/.git-credentials` (`credential.helper = store`).
  *Owner:* rotate it, then move GitLab auth to a keyring helper. GitLab was not
  reachable from the machine that made this change, so auth after the change is
  unverified. `--remotes` (above) now blocks this class at commit time.
- **Supabase, Google and Razorpay secrets in `saas/lifeos-website/frontend/.dev.vars`.**
  The file was tracked, with a service-role key, OAuth and payment secrets. It is
  untracked, and its history was rewritten with `git filter-repo --path
  frontend/.dev.vars --invert-paths` in a mirror. That repo has no remote, so no
  other copy was pushed. Checked by value: none of the secret values appears in the
  rewritten history. The remaining matches are public or config values that a
  tracked `wrangler.toml` also carries. *Owner:* rotate the service-role key, the
  Google client secret and the Razorpay key and webhook secrets. Those values are
  still in the untracked `.dev.vars` on disk.
- **Personal Notion/lifeos config in `agentic-harness/operant`.**
  `.superpowers/lifeos.config.json` is untracked. That repo is **public** on GitHub,
  and the file is already in its history. *Owner:* decide whether to rewrite that
  history and force-push. Nothing was pushed for it here.
- **Bot-token rows in the live `omp` bank.** Four rows held token-shaped strings.
  They were deleted through `DELETE /banks/omp/memories/{id}`, and a read-only
  re-scan found none left. Three distinct strings were involved: one token appeared
  in three rows, and two other strings appeared once each. **Those two were not
  identified.** If either is a live credential, revoke it too.
- **Battery snapshot.** `eval/data/battery/omp-snapshot.jsonl` (gitignored) still
  holds the token. It is the frozen input whose sha256 the battery record cites, so
  it was not edited. Redacting it means a new freeze with a new sha, and that is a
  measurement decision.
- **Checked and cleared.** `law-of-one-india/.env.example` contains a PEM *header
  in a comment*, not key material: the body is prose and placeholder text, with no
  base64 run of 60 or more characters. The earlier worry was wrong.
- **Hardening.** Secret scanning and push protection are enabled on the public
  `memory-wire` repo. The redaction pattern for Telegram tokens is in
  `src/capture.rs` (see the open-gaps entry above).
