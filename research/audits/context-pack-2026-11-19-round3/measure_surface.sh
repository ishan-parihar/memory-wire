#!/usr/bin/env bash
# The authored-surface count this round's pack cites.
#
# Why this script exists: the protocol's rule 7 says count the AUTHORED surface
# only, and rule 8 says ship the measurement with the round. A line count that
# includes generated residue (target/, eval artifacts, vendored corpora) measures
# what the system produced by running, not what it is. This script excludes those
# and names the exclusion beside every number.
#
# Run from the repo root:  bash measure_surface.sh
set -euo pipefail

echo "# Authored surface — memory-wire at $(git rev-parse --short HEAD)"
echo
echo "## Excluded, by name"
echo "  target/            build output"
echo "  research/          audit rounds (this pack's own directory tree)"
echo "  _audit/, _audit_scratch/   read-only competitor checkouts, not our code"
echo "  eval/              datasets and generated result artifacts"
echo "  genome/, scripts/genome/    vendored third-party CLI"
echo "  tests/fixtures/    deliberate secret fixtures"
echo

echo "## src/ — lines of Rust per module (the binary)"
wc -l src/*.rs | sort -rn

echo
echo "## src/ — total"
find src -name '*.rs' | xargs cat | wc -l

echo
echo "## tests/ + examples/ — the harness surface"
wc -l tests/*.rs 2>/dev/null | tail -1
wc -l examples/*.rs 2>/dev/null | tail -1

# `plugin/` is code that runs in a model's context, and `install/` is code that
# runs on a user's machine. Both were outside the count, which made the surface
# look like `src/` alone — and hid exactly the two TypeScript plugins whose trust
# gate was missing. Count them, and say they are counted separately, because a
# TypeScript plugin is not a Rust module and the two numbers must not be added
# into one misleading total.
echo
echo "## plugin/ — the injection path (TypeScript; runs in a model's context)"
find plugin -name '*.ts' -not -path '*/node_modules/*' | xargs wc -l 2>/dev/null | tail -1
echo "  python (hermes):"
find plugin -name '*.py' -not -path '*/node_modules/*' | xargs wc -l 2>/dev/null | tail -1

echo
echo "## install/ — code that runs on the user's machine"
find install -type f 2>/dev/null | xargs wc -l 2>/dev/null | tail -1

echo
echo "## Dependency count (the default build must not gain one)"
grep -c '^\[dependencies\]' Cargo.toml >/dev/null
sed -n '/^\[dependencies\]/,/^\[/p' Cargo.toml | grep -cE '^[a-z0-9_-]+ *=' || true
echo "--- feature-gated (embed, OFF by default) ---"
sed -n '/^\[features\]/,/^\[/p' Cargo.toml | head -20

echo
echo "## Shared libraries the release binary links"
if [ -f target/release/memory-wire ]; then
  readelf -d target/release/memory-wire | grep NEEDED || echo "(static)"
else
  echo "no release build; run cargo build --release first"
fi

echo
echo "## Binary size"
ls -l target/release/memory-wire 2>/dev/null | awk '{print $5" bytes"}' || echo "no release build"

echo
echo "## Public MCP tool surface (the count the docs must not misstate)"
grep -oE 'pub const [A-Z_]+: &str = "memory_[a-z_]+"' src/mcp.rs | sed 's/.*= //'

echo
echo "## Public HTTP routes (the count the docs must not misstate)"
echo "# Only ACTUAL .route() registrations. A grep for \"/banks/...\" also"
echo "# matches test fixtures and doc strings, which measure residue, not surface."
grep -hoE '\.route\("[^"]*"' src/*.rs | sed 's/.*route("//;s/"//' | sort -u
echo "# MCP mounts (a fourth door, not a separate service)"
grep -hoE 'nest_service\("[^"]*"' src/*.rs | sed 's/.*("//;s/"//' | sort -u

echo
echo "## Authored doc surface"
wc -l AGENTS.md README.md docs/*.md 2>/dev/null | tail -1
