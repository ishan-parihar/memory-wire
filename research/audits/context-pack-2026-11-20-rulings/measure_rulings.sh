#!/bin/sh
# Measure the surface the rulings round cites. Every figure in RULINGS.md is
# produced here, so a reviewer (or a later agent) re-derives a number instead of
# trusting a claim. Run from the subject repo root:
#
#   sh research/audits/context-pack-2026-11-20-rulings/measure_rulings.sh
#
# Nothing here mutates the tree. It reads, counts, and prints.

cd "$(dirname "$0")/../../.." || exit 1

echo "## Subject"
echo "HEAD:        $(git log -1 --format='%h %s')"
echo "branch:      $(git rev-parse --abbrev-ref HEAD)"
echo "tree:        $(git status --porcelain | wc -l | tr -d ' ') modified paths"
echo "version:     $(grep -m1 '^version' Cargo.toml | sed 's/.*"\(.*\)"/\1/')"
echo

echo "## Where the work stands"
printf 'TODO open:        %s\n' "$(awk -F'|' '/^\| *[0-9]+ *\|/ {gsub(/ /,"",$3); if($3=="open"||$3=="in-progress") c++} END{print c+0}' TODO.md)"
printf 'TODO done:        %s\n' "$(awk -F'|' '/^\| *[0-9]+ *\|/ {gsub(/ /,"",$3); if($3=="done") c++} END{print c+0}' TODO.md)"
printf 'modules:          %s\n' "$(ls src/*.rs | wc -l | tr -d ' ')"
printf 'src lines:        %s\n' "$(cat src/*.rs | wc -l | tr -d ' ')"
printf 'release vs tag:   %s commits past v0.10.0\n' "$(git rev-list --count v0.10.0..HEAD)"
echo

echo "## The TS door duplication, measured not estimated"
for f in plugin/integrations/extension/memory-wire.ts plugin/integrations/opencode-plugin/memory-wire.ts; do
  printf '%-52s %s lines\n' "$f" "$(wc -l < "$f" | tr -d ' ')"
done
echo
echo "-- module-level constants (top-level 'var NAME =', uppercase) --"
for f in plugin/integrations/extension/memory-wire.ts plugin/integrations/opencode-plugin/memory-wire.ts; do
  printf '%s:\n' "$f"
  grep -o '^var [A-Z][A-Z0-9_]*' "$f" | sed 's/^var /  /'
done
echo
echo "-- function names per door --"
for f in plugin/integrations/extension/memory-wire.ts plugin/integrations/opencode-plugin/memory-wire.ts; do
  printf '%s:\n' "$f"
  grep -o '^\(export \)\?function [A-Za-z_][A-Za-z0-9_]*' "$f" | sed 's/^export //;s/^function /  /'
done
echo

echo "## The installed binary vs the tree"
printf 'installed:  %s\n' "$("$HOME/.local/bin/memory-wire" --version 2>&1 | head -1)"
printf 'in tree:    %s\n' "$(./target/release/memory-wire --version 2>&1 | head -1)"
printf 'release bin built: %s\n' "$(date -r target/release/memory-wire '+%Y-%m-%d %H:%M' 2>/dev/null || echo unknown)"
echo

echo "## The blank-content guard and the schema it does not enforce"
grep -n "content.trim().is_empty()" src/api.rs | head -3
grep -n "content TEXT NOT NULL" src/store.rs | head -3
printf 'CHECK constraints naming content: %s\n' "$(grep -c "CHECK.*content" src/store.rs)"
echo

echo "## Gate cost, so the reviewer can price a new check"
printf 'trust gate checks:  %s\n' "$(grep -c '^check \|^  check \|^ok \|^  ok ' plugin/integrations/trust_gate_test.sh 2>/dev/null)"
printf 'tests:              %s\n' "$(PATH=/usr/bin:$PATH cargo test --locked 2>/dev/null | grep -o 'test result: ok. [0-9]*' | awk '{s+=$4} END{print s+0}')"
