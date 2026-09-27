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
set -eu

MW_REPO="${MW_REPO:-ishan-parihar/memory-wire}"
MW_VERSION="${MW_VERSION:-latest}"
MW_INSTALL_DIR="${MW_INSTALL_DIR:-$HOME/.local/bin}"
BIN="memory-wire"
UNINSTALL=0
DB_PATH=""

die() { printf 'memory-wire: %s\n' "$*" >&2; exit 1; }

while [ $# -gt 0 ]; do
  case "$1" in
    --uninstall) UNINSTALL=1 ;;
    --db) [ $# -ge 2 ] || die "--db needs a path"; DB_PATH="$2"; shift ;;
    --db=*) DB_PATH="${1#--db=}" ;;
    -h|--help) sed -n '2,10p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
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
  Static fallback: pin a tag, e.g.  MW_VERSION=v0.2.0  sh get-memory-wire.sh"
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
