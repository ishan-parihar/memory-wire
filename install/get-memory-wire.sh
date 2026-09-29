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
START_DAEMON=no

die() { printf 'memory-wire: %s\n' "$*" >&2; exit 1; }

while [ $# -gt 0 ]; do
  case "$1" in
    --uninstall) UNINSTALL=1 ;;
    --db) [ $# -ge 2 ] || die "--db needs a path"; DB_PATH="$2"; shift ;;
    --db=*) DB_PATH="${1#--db=}" ;;
    --connect) CONNECT=yes ;;
    --no-connect) CONNECT=no ;;
    --daemon) START_DAEMON=yes ;;
    -h|--help) sed -n '2,14p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
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
# Installing the binary is not installing the product: without this the agent
# host has no hooks and no MCP entry, so nothing retains or recalls anything.
# `connect` is idempotent and only ever writes entries it owns.
if [ "$CONNECT" != no ] && [ -x "$BIN_PATH" ]; then
  printf '\nwiring agent hosts (memory-wire connect):\n'
  if "$BIN_PATH" connect; then
    printf '  hosts wired — restart your agent to pick up the change\n'
  else
    printf '  connect exited nonzero; wire them by hand with: %s connect <host>\n' \
      "$BIN_PATH" >&2
  fi
  # The agent-facing skill is a deliberate copy, not something connect does.
  SKILL_SRC="$TMP/../plugin/skills/memory-wire/SKILL.md"
  printf '  agent skill: copy plugin/skills/memory-wire/SKILL.md into your host'"'"'s\n'
  printf '  skills dir (~/.claude/skills/, ~/.config/opencode/skills/, ...)\n'
fi

if [ "$START_DAEMON" = yes ] && [ -x "$BIN_PATH" ]; then
  printf '\nstarting the background server:\n'
  "$BIN_PATH" daemon start --addr 127.0.0.1:8888 ${DB_PATH:+--db $DB_PATH} || true
  "$BIN_PATH" doctor ${DB_PATH:+--db $DB_PATH} || true
fi
