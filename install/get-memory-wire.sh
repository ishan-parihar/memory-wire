#!/bin/sh
# memory-wire installer.
#
#   curl -fsSL https://raw.githubusercontent.com/$MW_REPO/main/install/get-memory-wire.sh | sh
#
# Env: MW_REPO=<owner>/<repo> (defaults to this repo), MW_VERSION=<tag>
# (default "latest" via the GitHub API), MW_INSTALL_DIR=<dir> (default
# ~/.local/bin), MW_LOCAL_ASSET=<file> (install a local .tar.gz, skip the network).
# Release assets are memory-wire-<target>.tar.gz, <target> in linux-x86_64
# linux-aarch64 macos-x86_64 macos-aarch64.
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

ASSET="$BIN-$TARGET.tar.gz"

# --- fetch ------------------------------------------------------------------
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT INT TERM
if [ -n "${MW_LOCAL_ASSET:-}" ]; then
  [ -f "$MW_LOCAL_ASSET" ] || die "MW_LOCAL_ASSET not a file: $MW_LOCAL_ASSET"
  cp "$MW_LOCAL_ASSET" "$TMP/$ASSET"
else
  URL="https://github.com/$MW_REPO/releases/download/$MW_VERSION/$ASSET"
  printf 'downloading %s\n' "$URL"
  curl -fsSL "$URL" -o "$TMP/$ASSET" || die "download failed: $URL
  Check MW_REPO=<owner>/<repo> and MW_VERSION=<tag>, or build locally:
    cargo build --release && cp target/release/$BIN $BIN_PATH"

  # Verify the download against the checksum the release publishes alongside it.
  # A missing .sha256 is not fatal (older releases predate it); a MISMATCH is.
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
fi

# --- install ----------------------------------------------------------------
mkdir -p "$MW_INSTALL_DIR"
tar -xzf "$TMP/$ASSET" -C "$TMP" "$BIN" 2>/dev/null \
  || tar -xzf "$TMP/$ASSET" -C "$TMP"
[ -f "$TMP/$BIN" ] || die "asset $ASSET did not contain a '$BIN' executable"
mv "$TMP/$BIN" "$BIN_PATH"
chmod +x "$BIN_PATH"
printf 'installed %s -> %s\n' "$BIN-$TARGET" "$BIN_PATH"

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
