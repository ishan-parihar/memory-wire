#!/usr/bin/env bash
# Blocks secret-shaped strings from entering a commit.
#   secret-scan.sh --staged    added lines in the index (the pre-commit gate)
#   secret-scan.sh --all       every tracked file (an audit, not a gate)
#   secret-scan.sh --selftest  the pattern against a synthetic token
# Reports file:line only. Never the match: a scanner that echoes the secret
# re-leaks it into the terminal and any log that captures it.
# ponytail: regex list, not gitleaks — no new tool on the box. Add gitleaks
# when this list misses a real leak.
set -euo pipefail

PAT="[0-9]{8,10}:AA[A-Za-z0-9_-]{30,}|(^|[^A-Za-z0-9])sk-[A-Za-z0-9_-]{20,}|AIza[0-9A-Za-z_-]{35}|gh[pousr]_[A-Za-z0-9]{36}|github_pat_[A-Za-z0-9_]{60,}|AKIA[0-9A-Z]{16}|xox[baprs]-[A-Za-z0-9-]{10,}|BEGIN [A-Z ]*PRIVATE KEY|(api[_-]?key|secret|passwd|password|token)[\"']?[[:space:]]*[:=][[:space:]]*[\"'][A-Za-z0-9_/+.=-]{24,}[\"']" # secret-scan:allow

# Test fixtures that contain deliberate fake secrets, and this file (its
# pattern list is itself secret-shaped text). Keep this list short.
EXCLUDE=(':(exclude)scripts/secret-scan.sh' ':(exclude)src/capture.rs' ':(exclude)src/vector.rs')

selftest() {
  local fake="1234567890:AA$(printf 'Z%.0s' $(seq 30))"
  if ! printf 'x %s\n' "$fake" | grep -qE "$PAT"; then
    echo "secret-scan: selftest failed — pattern missed a Telegram-shaped token" >&2
    exit 1
  fi
  if printf 'plain prose, no key here\n' | grep -qE "$PAT"; then
    echo "secret-scan: selftest failed — pattern matches plain prose" >&2
    exit 1
  fi
  echo "secret-scan: selftest ok"
}

report() {
  local n
  n=$(wc -l < "$1")
  if [ "$n" -gt 0 ]; then
    echo "secret-scan: $n secret-shaped line(s) (values not printed):" >&2
    sed 's/^/  /' "$1" >&2
    echo "Remove the value and rotate it. If the line is a deliberate fixture, add it to EXCLUDE in scripts/secret-scan.sh." >&2
    exit 1
  fi
}

case "${1:-}" in
  --selftest)
    selftest
    ;;
  --staged)
    out=$(mktemp)
    trap 'rm -f "$out"' EXIT
    git diff --cached -U0 --no-color --diff-filter=AM -- . "${EXCLUDE[@]}" |
      awk -v pat="$PAT" '
        /^\+\+\+ /   { file = substr($0, 7); next }
        /^@@ /       { split($3, r, ","); n = substr(r[1], 2) - 1; next }
        /^\+/        { n++; if (index($0, "secret-scan:allow") == 0 && substr($0, 2) ~ pat) print file ":" n; next }
      ' > "$out"
    report "$out"
    ;;
  --all)
    out=$(mktemp)
    trap 'rm -f "$out"' EXIT
    git grep -nIE -- "$PAT" -- . "${EXCLUDE[@]}" 2>/dev/null |
      grep -v 'secret-scan:allow' | cut -d: -f1,2 > "$out" || true
    report "$out"
    echo "secret-scan: tracked tree clean"
    ;;
  *)
    echo "usage: secret-scan.sh --staged | --all | --selftest" >&2
    exit 2
    ;;
esac
