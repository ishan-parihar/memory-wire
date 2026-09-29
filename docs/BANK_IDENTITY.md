# Bank identity

Which bank a hook, a `seed`, or an MCP session lands in, and what happens to
the memories of anyone who upgrades.

## The order

Bank resolution is a five-step ladder. The first step that produces a usable id
wins, and nothing after it is consulted.

| # | Source | Example |
|---|---|---|
| 1 | `--bank <id>` on the `hook` subcommand | `memory-wire hook session-start --bank my-project` |
| 2 | `MEMORY_WIRE_BANK` | `MEMORY_WIRE_BANK=my-project` |
| 3 | `owner/repo` of the repository's `origin` remote | `https://github.com/acme/api.git` → `acme-api` |
| 4 | The git work tree's top-level directory name, sanitised | `/home/x/api` → `api` |
| 5 | The literal `memory-wire` | outside a work tree |

Steps 1 and 2 are the escape hatches. Step 3 is the identity: it is what makes
two repositories that happen to share a directory name stop sharing a memory
namespace. Step 4 is the old behaviour, kept for the repositories step 3 cannot
describe. Step 5 is the last resort.

An explicit value that sanitises to nothing — `""`, `"   "`, `"///"` — is treated
as absent, and the ladder continues, rather than becoming a bank id that cannot
survive a URL path segment.

## Why step 3 exists

Before this, the bank id was the work tree's basename and nothing else. That is
a name, not an identity:

```text
/home/x/api   ->  bank "api"
/home/y/api   ->  bank "api"
```

Two unrelated projects, one namespace, silently. Nothing warns, nothing fails.
A repository you merely cloned — someone else's code, a tutorial you were
following — reads and writes the first project's memories, because the only
thing separating them is a directory name the clone happens to share.

The `origin` remote is the one thing about a checkout that distinguishes *this*
`api` from that `api`: it names who owns it. `acme/api` and `other/api` are two
banks, and neither can reach the other.

## How the remote is read

`.git/config` is parsed as text, and the `url` key of the `[remote "origin"]`
section is read. **No `git` process is spawned.**

That is a deliberate reversal, not an oversight. Hindsight's
`coding-agents/src/core/git-layout.ts:1-13` documents removing a
`git rev-parse` spawn precisely because a spawned `git` collapses every failure
into one indistinguishable answer: binary missing, timeout, `EAGAIN` from a fork,
or a directory that genuinely is not a repository all return the same "no". A
hook cannot tell a transient failure from a real answer, and a hook runs inside
a session it did not start. The filesystem distinguishes them; a subprocess
does not.

Recognised URL forms, all from real remotes:

```text
https://github.com/owner/repo.git     -> owner-repo
git@github.com:owner/repo.git         -> owner-repo
ssh://git@host/owner/repo.git         -> owner-repo
https://github.com/owner/repo         -> owner-repo
```

A remote with no `owner/repo` to read — a bare host, a single path component, a
local filesystem path, a malformed line — falls through to step 4. A repository
with no `origin` at all falls through too.

### `.git` as a file

A worktree or submodule has `.git` as a *file* containing `gitdir: <path>`
rather than a directory. That path is followed one level, and a relative one is
resolved against the work tree root. Both spellings work, because both occur in
practice: relative in a `git worktree add`, absolute in a submodule.

One level only. A linked worktree's git directory has no `config` of its own —
it reaches the parent repository's through `commondir` — and chasing that chain
is how a resolver ends up confidently reading a different repository's remotes.
When the one level does not resolve, resolution falls through to the basename.
A less specific answer is recoverable; a wrong one is not.

## The upgrade

**A bank id derived from a remote is a different bank from the basename that
produced the same memories before this change.** An existing user upgrading finds
their memories where they always were, under the old basename-derived name, and
their new sessions writing to a new name. That is deliberate, and it is not
being papered over.

There is no automatic migration and no bank aliasing. A migration that guesses
wrong moves memories between namespaces on the strength of a directory name,
and a wrong automatic migration destroys more than it fixes — the failure is
silent, it is not reversible, and the user has no way to know it happened. An
explicit name is a fact; a guessed one is a guess. So the guessing does not
happen.

`memory-wire doctor` says so when it can see it: when the bank this directory
resolves to differs from the basename it used to resolve to **and** a bank of
that old name exists in the store, it prints a warning naming both and the two
ways to reach the old one. Both conditions are required — a new name with no
bank behind it is a fresh project, not an orphaned memory, and an old name that
is still the resolved name is not a change at all. `doctor` remains read-only;
it reports, it does not repair.

### Recovering

Point the hook at the old bank, by flag or by environment:

```bash
memory-wire hook session-start --bank api
MEMORY_WIRE_BANK=api memory-wire hook session-start
```

Either reaches the existing memories exactly as before. To consolidate onto the
new name instead, read the old bank out and retain it into the new one — the
HTTP surface is the interface for that:

```bash
curl -sS 'localhost:8888/banks/api/memories?limit=500'   # read the old bank
# then retain each into the new bank id, or leave the old one where it is
```

Nothing is ever deleted by any of this.

## Is a bank id storable?

Yes, with one caveat worth stating rather than discovering.

`paths::sanitize_bank` maps every character outside `[a-z0-9_-.]` to `-`,
lowercases, and trims leading and trailing `-` and `.`. It returns `None` when
nothing usable is left, and the ladder treats that as "this source names no
bank" and continues.

`owner/repo` sanitises to `owner-repo` — the `/` becomes a `-`. That is a legal
bank id: lowercase alphanumerics and `-`, storable, and a valid URL path
segment.

The caveat: sanitisation is lossy. Two distinct remotes can sanitise to the same
id — `acme/api` and `acme.api` both become `acme-api`. The remote-derived
branch is a large improvement over basename derivation, not a proof of global
uniqueness. A collision now needs two repositories to agree on both owner *and*
name modulo punctuation, rather than merely on a directory name. For a single
user's machine that is a remote possibility; across a fleet sharing one server
it is worth knowing about.

If a specific id matters more than the derivation — a team sharing one server,
a project that must never share a bank with anything — that is what steps 1 and
2 are for. Name it.

## Measured: what can and cannot collide

The caveat above is about lossy sanitisation. It is worth separating that from
the case that actually drives the competitors' design, because they are not the
same risk and only one of them applies here.

Measured on 2026-09-29 by resolving the bank with `memory-wire doctor` in each
directory:

| directory | resolved bank | collides? |
|---|---|---|
| two unrelated local repos both named `api`, no git remote | `api`, `api` | **yes** |
| a local repo with a unique name, no git remote | `unique-thing` | no |
| a clone of `github.com/ishan-parihar/memory-wire` | `ishan-parihar-memory-wire` | no |
| a clone of `github.com/rust-lang/rust` | `rust-lang-rust` | no |

**A fork does not collide.** A GitHub fork has a different owner, so its
`origin` differs, so the remote-derived branch produces a different bank. That
is the threat Hindsight names when it fails closed — "a cloned repository must
not be able to turn memory on" — and remote-derived identity already covers it.
Two clones of the *same* repository do share a bank, which is the same user's
own two checkouts and is almost certainly the intent.

The residual is therefore narrow: **two different local directories that share
a basename and have no git remote.** Set `MEMORY_WIRE_BANK`, or pass `--bank`,
and it is named rather than derived.

Whether a repository must *opt in* before a hook will write to it is a product
decision rather than a correctness fix, and it is deliberately not made here.
The measurement above is the input to it.
