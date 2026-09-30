#!/bin/sh
# memory-wire installer.
#
#   curl -fsSL https://raw.githubusercontent.com/$MW_REPO/main/install/get-memory-wire.sh | sh
#
# Env: MW_REPO=<owner>/<repo> (defaults to this repo), MW_VERSION=<tag>
# (default "latest" via the GitHub API), MW_INSTALL_DIR=<dir> (default
# ~/.local/bin), MW_LOCAL_ASSET=<file|dir> (install a local .tar.gz, or a
# directory of them named as release assets, and skip the network).
# Release assets are memory-wire-<target>.tar.gz, <target> in linux-x86_64
# linux-x86_64-musl linux-aarch64 macos-x86_64 macos-aarch64. The musl build is
# static and is what gets installed when the glibc build will not start.
#
# Flags:
#   --db <path>   echo the serve/daemon lines with this --db
#   --connect     wire the agent hosts (default; --no-connect to skip)
#   --hosts <a,b> wire only these hosts, e.g. claude-code,codex. Never prompts:
#                 this is the path for a script or an AI agent. Run the installer
#                 with no flag on a terminal to pick from a numbered list.
#   --daemon      also start the background server and run doctor
#   --uninstall   remove the binary only; the database is left in place
set -eu

MW_REPO="${MW_REPO:-ishan-parihar/memory-wire}"
MW_VERSION="${MW_VERSION:-latest}"
MW_INSTALL_DIR="${MW_INSTALL_DIR:-$HOME/.local/bin}"
BIN="memory-wire"
UNINSTALL=0
DB_PATH=""
CONNECT=auto        # auto | yes | no
HOSTS=""            # --hosts <a,b>; empty means every detected host
START_DAEMON=no

die() { printf 'memory-wire: %s\n' "$*" >&2; exit 1; }

while [ $# -gt 0 ]; do
  case "$1" in
    --uninstall) UNINSTALL=1 ;;
    --db) [ $# -ge 2 ] || die "--db needs a path"; DB_PATH="$2"; shift ;;
    --db=*) DB_PATH="${1#--db=}" ;;
    --connect) CONNECT=yes ;;
    --no-connect) CONNECT=no ;;
    --hosts) [ $# -ge 2 ] || die "--hosts needs a host list, e.g. --hosts claude-code,codex"
              HOSTS="$2"; shift ;;
    --hosts=*) HOSTS="${1#--hosts=}" ;;
    --daemon) START_DAEMON=yes ;;
    # Print the header comment block. Done by scanning for the first non-comment
    # line rather than a hardcoded line range, which silently truncated the flag
    # list every time a line was added above.
    -h|--help) awk 'NR>1 && /^#/ { sub(/^# ?/, ""); print; next } NR>1 { exit }' "$0"; exit 0 ;;
    *) die "unknown flag: $1 (try --help)" ;;
  esac
  shift
done

# --- target detection -------------------------------------------------------
os=$(uname -s | tr 'A-Z' 'a-z')
arch=$(uname -m)
case "$arch" in
  x86_64|amd64) arch=x86_64 ;;
  aarch64|arm64) arch=aarch64 ;;
esac
case "$os" in
  linux|darwin) os=$([ "$os" = darwin ] && echo macos || echo linux) ;;
  *) die "unsupported OS: $(uname -s) (supported: linux, macos)" ;;
esac
case "$arch" in
  x86_64|aarch64) : ;;
  *) die "unsupported arch: $(uname -m) (supported: x86_64, aarch64)" ;;
esac
TARGET="$os-$arch"

BIN_PATH="$MW_INSTALL_DIR/$BIN"

# --- uninstall --------------------------------------------------------------
if [ "$UNINSTALL" -eq 1 ]; then
  [ -e "$BIN_PATH" ] || die "not installed: $BIN_PATH"
  rm -f "$BIN_PATH"
  printf 'removed %s (database left in place at %s — delete it by hand)\n' \
    "$BIN_PATH" "${XDG_DATA_HOME:-$HOME/.local/share}/memory-wire"
  exit 0
fi

# --- resolve version --------------------------------------------------------
if [ "$MW_VERSION" = latest ] && [ -z "${MW_LOCAL_ASSET:-}" ]; then
  MW_VERSION=$(curl -fsSL "https://api.github.com/repos/$MW_REPO/releases/latest" 2>/dev/null \
    | sed -n 's/.*"tag_name" *: *"\([^"]*\)".*/\1/p' | head -1) || MW_VERSION=''
  [ -n "$MW_VERSION" ] || die "no published release for $MW_REPO.
  Static fallback: pin a tag, e.g.  MW_VERSION=v0.3.0  sh get-memory-wire.sh"
fi

# --- candidates -------------------------------------------------------------
# The glibc build is made on glibc 2.44 and refuses to start below 2.39, so on
# an older host it downloads, installs, and dies on first run with "GLIBC_2.39
# not found" — while the installer has already said it succeeded. The musl build
# is static and has no floor, so it is the fallback.
#
# $TARGET is first and is the only candidate wherever no musl build is published,
# so a healthy host takes the path it always took and prints the same lines.
# scripts/build-release.sh builds linux-x86_64-musl and no aarch64-musl, so the
# extra candidate is added where one exists — never by appending "-musl" to
# whatever $TARGET happens to be.
CANDIDATES="$TARGET"
case "$TARGET" in
  linux-x86_64) CANDIDATES="$TARGET $TARGET-musl" ;;
esac

# --- fetch ------------------------------------------------------------------
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT INT TERM

# Stage one candidate into $TMP and verify it, or return non-zero so the next
# candidate is tried. The download and the checksum happen here for every
# candidate alike, so no asset can reach an install unverified.
fetch() {
  ASSET="$BIN-$1.tar.gz"
  if [ -n "${MW_LOCAL_ASSET:-}" ]; then
    # A directory is a mirror of the release directory, which is what makes the
    # fallback testable without a network; a plain file is the documented
    # single-asset hook and is offered to each candidate in turn.
    if [ -d "$MW_LOCAL_ASSET" ]; then
      [ -f "$MW_LOCAL_ASSET/$ASSET" ] || return 1
      cp "$MW_LOCAL_ASSET/$ASSET" "$TMP/$ASSET"
    else
      [ -f "$MW_LOCAL_ASSET" ] || die "MW_LOCAL_ASSET not a file: $MW_LOCAL_ASSET"
      cp "$MW_LOCAL_ASSET" "$TMP/$ASSET"
    fi
    return 0
  fi

  URL="https://github.com/$MW_REPO/releases/download/$MW_VERSION/$ASSET"
  printf 'downloading %s\n' "$URL"
  curl -fsSL "$URL" -o "$TMP/$ASSET" || return 1

  # Verify the download against the checksum the release publishes alongside it.
  # A missing .sha256 is not fatal (older releases predate it); a MISMATCH is, and
  # stays fatal for a fallback candidate too — a corrupt musl tarball is still a
  # corrupt tarball, not a reason to try something else.
  if curl -fsSL "$URL.sha256" -o "$TMP/$ASSET.sha256" 2>/dev/null; then
    if command -v sha256sum >/dev/null 2>&1; then
      (cd "$TMP" && sha256sum -c "$ASSET.sha256" >/dev/null 2>&1) \
        || die "checksum mismatch for $ASSET — refusing to install.
  The download does not match the published sha256. Re-run, or fetch manually:
    $URL"
      printf 'checksum ok\n'
    elif command -v shasum >/dev/null 2>&1; then
      (cd "$TMP" && shasum -a 256 -c "$ASSET.sha256" >/dev/null 2>&1) \
        || die "checksum mismatch for $ASSET — refusing to install."
      printf 'checksum ok\n'
    fi
  fi
}

# Try the candidates in order and keep the first that actually starts. The only
# honest test of an ELF is running it, so this probes `--version` against the
# staged copy in $TMP — never `ldd --version`, which is a version comparison in
# a locale-dependent format answering a question the executable answers exactly.
# The probe must happen before the mv: once the file is at $BIN_PATH an
# unusable binary is indistinguishable from an installed one.
CHOSEN=""
PROBE_ERR=""
for cand in $CANDIDATES; do
  ASSET="$BIN-$cand.tar.gz"
  # Clear the previous candidate's files. Without this a fallback that fails to
  # extract would leave the earlier candidate's $TMP/$BIN to be probed.
  rm -f "$TMP/$BIN" "$TMP/$ASSET" "$TMP/$ASSET.sha256"

  fetch "$cand" || continue
  tar -xzf "$TMP/$ASSET" -C "$TMP" "$BIN" 2>/dev/null \
    || tar -xzf "$TMP/$ASSET" -C "$TMP" || continue
  if [ ! -f "$TMP/$BIN" ]; then
    printf '  %s did not contain a %s executable\n' "$ASSET" "$BIN" >&2
    continue
  fi
  chmod +x "$TMP/$BIN"

  if PROBE_OUT=$("$TMP/$BIN" --version 2>&1); then
    CHOSEN="$cand"
    break
  fi
  PROBE_ERR=$(printf '%s\n' "$PROBE_OUT" | head -1)
  printf '  %s does not run on this host: %s\n' "$ASSET" "$PROBE_ERR" >&2
done

if [ -z "$CHOSEN" ]; then
  die "no memory-wire asset runs on this host (tried: $CANDIDATES).
  Last failure: ${PROBE_ERR:-no asset could be fetched}
  The linux-x86_64 build needs glibc >= 2.39 and refuses to start on an older
  libc with GLIBC_2.39 not found; the musl build is static and has no floor, so
  both failing means neither fits this box. Check what you have:
    ldd --version | head -1
  Or install a build you already have, skipping the network:
    MW_LOCAL_ASSET=<file.tar.gz> MW_INSTALL_DIR=<dir> sh get-memory-wire.sh
  Or build from source:
    cargo build --release && cp target/release/$BIN $BIN_PATH"
fi

# --- install ----------------------------------------------------------------
mkdir -p "$MW_INSTALL_DIR"

# The version a binary reports, or empty when it reports nothing usable. The last
# field is the version, but only if it looks like one — a banner carrying no
# version must read as unknown, not as the binary's own name. Read from the
# executable rather than from the release tag, so the two cannot disagree.
version_of() {
  "$1" --version 2>/dev/null | awk '{print $NF}' | sed 's/^[vV]//' \
    | grep -E '^[0-9][0-9.]*$' || true
}

# What is on disk right now, read from that binary before the mv overwrites it,
# and tolerant of its absence: a first install has nothing to report.
OLD_VER=""
if [ -x "$BIN_PATH" ]; then
  OLD_VER=$(version_of "$BIN_PATH")
fi

mv "$TMP/$BIN" "$BIN_PATH"
chmod +x "$BIN_PATH"
NEW_VER=$(version_of "$BIN_PATH")
printf 'installed %s -> %s\n' "$BIN-$CHOSEN" "$BIN_PATH"

# --- what changed ------------------------------------------------------------
# The gap this closes is the silence, not the version number: an upgrade, a
# downgrade and a no-op used to print one identical line, so a user who pinned an
# old tag had no way to see they had gone backwards.
#
# awk for the comparison because it is already assumed present, and this has to
# work on a box holding nothing but the tools the install itself needs. Prints 1
# when $1 is newer than $2, 0 when equal, -1 when older.
ver_cmp() {
  awk -v a="$1" -v b="$2" 'BEGIN {
    na = split(a, A, "."); nb = split(b, B, ".")
    n = (na > nb ? na : nb)
    for (i = 1; i <= n; i++) {
      x = (i <= na ? A[i] : 0) + 0
      y = (i <= nb ? B[i] : 0) + 0
      if (x != y) { print(x > y ? 1 : -1); exit }
    }
    print 0
  }'
}

if [ -z "$NEW_VER" ]; then
  printf 'installed, but %s --version printed nothing\n' "$BIN_PATH" >&2
elif [ -z "$OLD_VER" ]; then
  printf 'version %s (first install — nothing was here to replace)\n' "$NEW_VER"
elif [ "$OLD_VER" = "$NEW_VER" ]; then
  printf 'version %s -> %s (no change)\n' "$OLD_VER" "$NEW_VER"
else
  case "$(ver_cmp "$NEW_VER" "$OLD_VER")" in
    1)  printf 'version %s -> %s (upgrade)\n' "$OLD_VER" "$NEW_VER" ;;
    -1) printf 'version %s -> %s (DOWNGRADE)\n' "$OLD_VER" "$NEW_VER" ;;
    *)  printf 'version %s -> %s (no change)\n' "$OLD_VER" "$NEW_VER" ;;
  esac
fi

# Is anything still executing this path? rename(2) over a running executable does
# not return ETXTBSY on Linux — the old inode survives as the running process's
# image and only the directory entry moves — so the mv above replaced the file
# without interrupting anything, and a server that was already up is still
# serving the old code. Saying so is the installer's job. Restarting it is not:
# the daemon may be under a supervisor this script cannot introspect, and a
# restart is the user's decision. `ps` fails open — a box without it gets no
# warning, which is not a failed install.
if [ -n "$OLD_VER" ]; then
  SERVING=$(ps -eo pid=,args= 2>/dev/null | awk -v p="$BIN_PATH" -v me=$$ '
    $1 != me && index($2, p) == 1 { print $1 }')
  if [ -n "$SERVING" ]; then
    printf '\n  a server is still running the old %s (pid %s): it kept the old\n' \
      "$OLD_VER" "$SERVING" >&2
    printf '  inode when the file was replaced, so it must be restarted to serve %s.\n' \
      "$NEW_VER" >&2
    printf '    %s daemon stop && %s daemon start\n' "$BIN_PATH" "$BIN_PATH" >&2
    printf '  (if a supervisor started it: restart that unit)\n' >&2
  fi
fi

# --- handoff ----------------------------------------------------------------
case ":$PATH:" in
  *":$MW_INSTALL_DIR:"*) : ;;
  *) printf '\n%s is not on your PATH. Add it:\n  export PATH="%s:$PATH"\n' \
       "$MW_INSTALL_DIR" "$MW_INSTALL_DIR" >&2 ;;
esac

printf '\nnext:\n'
printf '  %s serve --addr 127.0.0.1:8888%s\n' \
  "$BIN_PATH" "${DB_PATH:+ --db $DB_PATH}"
printf '  curl -s http://127.0.0.1:8888/health      # -> ok\n'

# --- wire the agent harnesses ------------------------------------------------
# Installing the binary is not installing the product. Without this step the
# agent host has no hooks and no MCP entry, so nothing retains or recalls
# anything and memory-wire is just a binary on disk. This runs by DEFAULT;
# --no-connect is the opt-out.
#
# `connect` is idempotent, backs up every file it edits, refuses a malformed
# config untouched, and never overwrites a hook it did not write.
#
# Resolve the host list to run first, so the scan is on screen before anything
# is written: `--list` only reads, and says which hosts are here and which are
# already wired.
connect() {
  if [ -n "$HOSTS" ]; then
    # $HOSTS unquoted on purpose: a host list is several words, and the names are
    # [a-z-], so there is nothing here for the shell to glob or split wrongly.
    "$BIN_PATH" connect $HOSTS "$@"
  else
    "$BIN_PATH" connect "$@"
  fi
}

# Turn an answer at the menu into a host list. $1 is what the person typed, $2 the
# detected hosts in menu order. A number and a host name are both accepted; an
# answer that is neither is passed through to `connect`, which names the valid
# hosts itself rather than this script carrying a second list of them. An empty
# answer prints nothing, which leaves the caller's default alone.
choose_hosts() {
  # $2 unquoted for the same reason $HOSTS is above.
  # shellcheck disable=SC2086
  printf '%s\n' $2 | awk -v want="$1" '
    BEGIN { n = split(want, a, /[,[:space:]]+/)
            for (i = 1; i <= n; i++) if (a[i] != "") pick[a[i]] = 1 }
    { if (pick[NR] || pick[$1]) { printf "%s%s", seen ? " " : "", $1; seen = 1 } }'
}

CONNECT_RC=0
CONNECT_OUT=""
if [ "$CONNECT" != no ] && [ -x "$BIN_PATH" ]; then
  printf '\nagent hosts on this machine (memory-wire connect --list):\n'
  HOSTS_LIST="$("$BIN_PATH" connect --list 2>&1)" || HOSTS_LIST=""
  printf '%s\n' "$HOSTS_LIST" | sed 's/^/  /'

  # When to ask and when not to. `--hosts` is the non-interactive path and never
  # prompts. Without it, only a terminal is asked: a pipe, a CI log or an agent's
  # captured stdin cannot answer, and a question nobody can answer is a hang, not
  # a question — so those wire every detected host exactly as before.
  if [ -z "$HOSTS" ] && [ -t 0 ]; then
    DETECTED=$(printf '%s\n' "$HOSTS_LIST" | awk '$2 == "detected=yes" { print $1 }')
    if [ -n "$DETECTED" ]; then
      n=0
      for h in $DETECTED; do
        n=$((n + 1))
        printf '  %d) %s\n' "$n" "$h"
      done
      printf 'wire which? [enter = all detected, or numbers/names such as 1,3]\n'
      printf '> '
      REPLY=""
      IFS= read -r REPLY || true
      HOSTS=$(choose_hosts "$REPLY" "$DETECTED")
    fi
  fi

  printf '\nwiring agent hosts (memory-wire connect%s):\n' "${HOSTS:+ $HOSTS}"
  CONNECT_OUT="$(connect 2>&1)" && CONNECT_RC=0 || CONNECT_RC=$?
  printf '%s\n' "$CONNECT_OUT" | sed 's/^/  /'

  if [ "$CONNECT_RC" -ne 0 ]; then
    # Loud, but not fatal: the binary is installed and usable, and a user who
    # only wants the HTTP surface should not get a failed installer. Exiting 0
    # while saying nothing useful, though, is exactly how "it installed but my
    # agent has no memory" happens.
    printf '\n  WARNING: memory-wire connect exited %s — the agent hosts are NOT wired.\n' "$CONNECT_RC" >&2
    printf '  The binary is installed and the HTTP surface works, but agents will not\n' >&2
    printf '  use it until you run this by hand:\n' >&2
    printf '    %s connect\n' "$BIN_PATH" >&2
  fi

  # Did anything actually get wired? A machine with no agent host installed is a
  # normal state, and silently reporting success there would be a lie. `wired` and
  # `already-wired` both count: `connect` is idempotent, so a re-run over a machine
  # that is already configured must not be told nothing was detected — the hosts
  # ARE wired, connect simply had no change to make.
  if printf '%s' "$CONNECT_OUT" | grep -qE '^[^ ]+ +(already-)?wired'; then
    printf '\n  hosts wired — restart your agent to load the hooks and MCP entry\n'
  elif [ "$CONNECT_RC" -eq 0 ]; then
    printf '\n  no supported agent host was detected, so nothing was wired.\n'
    printf '  Install Claude Code, Codex, Copilot CLI, Cursor or opencode and re-run:\n'
    printf '    %s connect\n' "$BIN_PATH"
  fi

  # --- agent skill ------------------------------------------------------------
  # The skill is the file actually addressed to a model: it teaches when to
  # retain, what budget means, and which errors are worth acting on. `connect`
  # deliberately does not install it (it owns only hooks and MCP entries), so
  # the installer does. Best-effort: a skill is an enhancement, not a
  # prerequisite, and a failed download must not fail the install.
  SKILL_DIR="$HOME/.agents/skills/$BIN"
  SKILL_URL="https://raw.githubusercontent.com/$MW_REPO/${MW_BRANCH:-main}/plugin/skills/$BIN/SKILL.md"
  mkdir -p "$SKILL_DIR"
  if [ -f "$SKILL_DIR/SKILL.md" ]; then
    printf '  agent skill: already present at %s\n' "$SKILL_DIR/SKILL.md"
  elif curl -fsSL "$SKILL_URL" -o "$SKILL_DIR/SKILL.md" 2>/dev/null; then
    printf '  agent skill: installed at %s\n' "$SKILL_DIR/SKILL.md"
    printf '  (agents reading skills from a per-host directory can copy it there as well:\n'
    printf '   ~/.claude/skills/, ~/.config/opencode/skills/, ~/.codex/skills/)\n'
  else
    printf '  agent skill: could not download from %s\n' "$SKILL_URL" >&2
    printf '  fetch it by hand:\n    curl -fsSL %s -o ~/.agents/skills/%s/SKILL.md\n' \
      "$SKILL_URL" "$BIN" >&2
  fi
fi

if [ "$START_DAEMON" = yes ] && [ -x "$BIN_PATH" ]; then
  printf '\nstarting the background server:\n'
  "$BIN_PATH" daemon start --addr 127.0.0.1:8888 ${DB_PATH:+--db $DB_PATH} || true
  "$BIN_PATH" doctor ${DB_PATH:+--db $DB_PATH} || true
fi
