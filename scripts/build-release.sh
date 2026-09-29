#!/usr/bin/env bash
# Build memory-wire release assets locally, then optionally upload them.
#
# Why this exists instead of relying on .github/workflows/release.yml: GitHub
# Actions minutes are a metered, exhaustible resource, and a release that cannot
# be cut because the quota is spent is a release blocker. Everything here runs on
# one machine and publishes through the GitHub REST API via `gh`, which consumes
# no Actions minutes at all. The workflow remains useful for people who have
# quota; this script is the floor under it, not a replacement.
#
# Usage:
#   scripts/build-release.sh                 # build every target into dist/
#   scripts/build-release.sh --upload v0.4.0 # build, then create/update the release
#   scripts/build-release.sh --only linux-x86_64
#   scripts/build-release.sh --check         # build and verify, never upload
#
# Cross-compilation uses cargo-zigbuild, which supplies its own libc headers per
# target. That is what lets a Linux box produce macOS binaries: a bare
# `--target` would fail at link time for want of a macOS SDK.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT"

BIN="memory-wire"
DIST="$ROOT/dist"
UPLOAD_TAG=""
ONLY=""
DO_UPLOAD=0

die() { printf 'build-release: %s\n' "$*" >&2; exit 1; }
log() { printf '\033[1mbuild-release\033[0m %s\n' "$*"; }

while [ $# -gt 0 ]; do
  case "$1" in
    --upload) [ $# -ge 2 ] || die "--upload needs a tag"; UPLOAD_TAG="$2"; DO_UPLOAD=1; shift ;;
    --upload=*) UPLOAD_TAG="${1#--upload=}"; DO_UPLOAD=1 ;;
    --only) [ $# -ge 2 ] || die "--only needs a target"; ONLY="$2"; shift ;;
    --only=*) ONLY="${1#--only=}" ;;
    --check) DO_UPLOAD=0 ;;
    -h|--help) sed -n '2,20p' "$0" | sed 's/^# \{0,1\}//'; exit 0 ;;
    *) die "unknown flag: $1 (try --help)" ;;
  esac
  shift
done

# target -> rust triple. Asset names must stay byte-identical to what
# install/get-memory-wire.sh asks for, or the installer 404s on that platform.
ALL_TARGETS="linux-x86_64 linux-aarch64 macos-x86_64 macos-aarch64"
triple_for() {
  case "$1" in
    linux-x86_64)  echo x86_64-unknown-linux-gnu  ;;
    linux-aarch64) echo aarch64-unknown-linux-gnu ;;
    macos-x86_64)  echo x86_64-apple-darwin       ;;
    macos-aarch64) echo aarch64-apple-darwin      ;;
    *) die "unknown target: $1" ;;
  esac
}
[ -n "$ONLY" ] && ALL_TARGETS="$ONLY"

HOST_TRIPLE="$(rustc -vV | sed -n 's/^host: //p')"
command -v cargo >/dev/null || die "cargo not on PATH"

# Preflight the cross-compiler. Checked per target rather than against the FIRST
# entry: an earlier version compared the host against linux-x86_64 only, so on a
# Linux box the check never fired and a missing cargo-zigbuild surfaced mid-loop
# as a raw cargo error instead of the install hint.
needs_cross=0
for t in $ALL_TARGETS; do
  [ "$(triple_for "$t")" = "$HOST_TRIPLE" ] || needs_cross=1
done
if [ "$needs_cross" -eq 1 ] && ! command -v cargo-zigbuild >/dev/null; then
  die "cargo-zigbuild is required to cross-compile ($(echo $ALL_TARGETS | tr ' ' ',') includes a non-host target).
  Install: cargo install cargo-zigbuild"
fi

# A dirty tree means the binary does not match the tag it would be published
# under. Refuse by default: a v0.4.0 asset built from uncommitted work is worse
# than no asset, because it is unreproducible.
if [ -n "$UPLOAD_TAG" ] && [ -n "$(git status --porcelain 2>/dev/null)" ]; then
  die "working tree is dirty. Commit or stash first — a release asset must be
  reproducible from the tag it is published under.
  (override deliberately with: git stash -u, rebuild, then re-apply)"
fi

# The tag must exist and must name the commit about to be built. Checked HERE,
# beside the dirty-tree guard, not after the build loop: a release takes ten
# minutes per target, and discovering a missing tag at the end of that is a
# waste the preflight exists to prevent.
if [ -n "$UPLOAD_TAG" ]; then
  if ! git rev-parse -q --verify "refs/tags/$UPLOAD_TAG" >/dev/null; then
    die "tag $UPLOAD_TAG does not exist locally, so the assets could not be
  traced to the commit they were built from. Create it first:
    git tag -a $UPLOAD_TAG -m 'memory-wire $UPLOAD_TAG'"
  fi
  tag_commit="$(git rev-list -n1 "$UPLOAD_TAG")"
  head_commit="$(git rev-parse HEAD)"
  [ "$tag_commit" = "$head_commit" ] || die "tag $UPLOAD_TAG points at $tag_commit
  but HEAD is $head_commit. A release publishes its assets under the tag, so
  the two must name the same commit. Check out the tag, or move the tag, then
  rebuild."
fi

built=""
failed=""
skipped=""
HOST_OS="$(uname -s)"

# Preflight: refuse to start a macOS cross-build that cannot finish.
# Apple's SDK is not redistributable and is not on a Linux box, and this crate
# needs it: libsqlite3-sys links -framework CoreFoundation on macOS, which zig
# cannot synthesise the way it supplies libc headers. The build dies at link
# time after ten minutes of compiling every dependency first. Say so up front.
#
# There is no workaround short of building on a Mac. `cargo install
# memory-wire` works there, as does CI when Actions quota is available.
#
# This runs BEFORE dist/ is cleared. An earlier version wiped dist first, so a
# preflight abort destroyed assets that had already been built and verified.
if [ "$HOST_OS" != "Darwin" ]; then
  for t in $ALL_TARGETS; do
    case "$t" in
      macos-*)
        log "SKIP $t — macOS assets cannot be cross-built from $HOST_OS."
        log "      Apple's SDK is required (libsqlite3-sys links CoreFoundation)."
        log "      Build on a Mac, use CI, or install from source: cargo install memory-wire"
        skipped="$skipped $t"
        ;;
    esac
  done
  ALL_TARGETS=$(printf '%s\n' $ALL_TARGETS | grep -v '^macos-' || true)
  [ -n "$ALL_TARGETS" ] || die "no buildable targets left (host is $HOST_OS)"
fi

# Only now is it safe to clear the output directory.
rm -rf "$DIST"
mkdir -p "$DIST"

for t in $ALL_TARGETS; do
  triple="$(triple_for "$t")"
  log "building $t  ($triple)"
  if ! rustup target list --installed | grep -qx "$triple"; then
    log "  SKIP $t — rust target $triple not installed (rustup target add $triple)"
    failed="$failed $t"
    continue
  fi
  # cargo-zigbuild for cross targets; plain cargo when the triple IS this host,
  # because zigbuild adds a wrapper for no benefit and occasionally trips on
  # build scripts that inspect the compiler.
  if [ "$triple" = "$HOST_TRIPLE" ]; then
    cargo build --release --locked --target "$triple" || { failed="$failed $t"; continue; }
  else
    cargo zigbuild --release --locked --target "$triple" || { failed="$failed $t"; continue; }
  fi

  src="target/$triple/release/$BIN"
  [ -f "$src" ] || { log "  FAIL $t — $src not produced"; failed="$failed $t"; continue; }

  # The installer does: tar -xzf asset -C "$TMP" "$BIN"  — so the archive must
  # hold `memory-wire` at its root, with no target/release/ prefix.
  strip "$src" 2>/dev/null || true
  cp "$src" "$DIST/$BIN"
  tar -czf "$DIST/$BIN-$t.tar.gz" -C "$DIST" "$BIN"
  rm -f "$DIST/$BIN"

  # Checksum ships beside the asset; the installer verifies it when present.
  if command -v sha256sum >/dev/null; then
    ( cd "$DIST" && sha256sum "$BIN-$t.tar.gz" > "$BIN-$t.tar.gz.sha256" )
  else
    ( cd "$DIST" && shasum -a 256 "$BIN-$t.tar.gz" > "$BIN-$t.tar.gz.sha256" )
  fi

  size=$(du -h "$DIST/$BIN-$t.tar.gz" | cut -f1)
  log "  OK   $t  ($size)  $(cut -d' ' -f1 < "$DIST/$BIN-$t.tar.gz.sha256" | cut -c1-16)…"
  built="$built $t"
done

echo
log "built: ${built:- none}"
[ -n "$skipped" ] && log "skipped:$skipped (not buildable on this host)"
[ -n "$failed" ] && log "FAILED:$failed"

if [ -z "$built" ]; then
  die "no assets were produced"
fi

# A partial release is worse than none: the installer's failure message points at
# MW_REPO/MW_VERSION, which is the wrong thing to debug when the real cause is a
# missing platform asset. Refuse to publish an incomplete set — but count a
# deliberate, explained skip as incomplete too, because the installer will 404 on
# that platform exactly as it did before.
if [ "$DO_UPLOAD" -eq 1 ] && { [ -n "$failed" ] || [ -n "$skipped" ]; }; then
  die "refusing to publish an incomplete release.
  failed:${failed:- none}
  skipped:${skipped:- none}
  install/get-memory-wire.sh will 404 on every platform not in this upload, which
  is the bug this script exists to fix. Pass --only to publish a deliberate
  subset, or build the remainder where its SDK is available."
fi

if [ "$DO_UPLOAD" -eq 0 ]; then
  log "not uploading (pass --upload <tag> to publish). Artifacts in $DIST"
  exit 0
fi

# --- publish ------------------------------------------------------------------

# `gh release` is a REST API call. It does not start a workflow and does not
# bill Actions minutes — which is the entire reason this script exists.
log "uploading to release $UPLOAD_TAG via the GitHub API (no Actions minutes)"
if gh release view "$UPLOAD_TAG" >/dev/null 2>&1; then
  # Never silently replace assets on a published release. A release someone has
  # already installed from is a promise; re-running with a stale build must fail
  # loudly rather than swap the bytes under them. Cut a new version instead.
  existing="$(gh release view "$UPLOAD_TAG" --json assets --jq '.assets | length' 2>/dev/null || echo 0)"
  if [ "${existing:-0}" -gt 0 ]; then
    die "release $UPLOAD_TAG already has $existing published asset(s).
  Refusing to clobber them: an installer that fetched v$UPLOAD_TAG would get different
  bytes than the checksum it recorded. Cut a new version, or delete the release
  yourself if you truly mean to replace it."
  fi
  gh release upload "$UPLOAD_TAG" "$DIST/$BIN"-*.tar.gz "$DIST/$BIN"-*.tar.gz.sha256 --clobber
  log "uploaded assets to the existing $UPLOAD_TAG release"
else
  gh release create "$UPLOAD_TAG" "$DIST/$BIN"-*.tar.gz "$DIST/$BIN"-*.tar.gz.sha256 \
    --title "memory-wire $UPLOAD_TAG" --generate-notes
  log "created release $UPLOAD_TAG"
fi

log "verify with: gh release view $UPLOAD_TAG --json assets --jq '.assets[].name'"
