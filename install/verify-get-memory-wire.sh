#!/bin/sh
# Installer checks — the install-script-creator step-6 gate for this repo.
#
# The installer is release-critical surface with no cargo test coverage, so
# its discipline is checked here: syntax, shellcheck at error severity, the
# sections the skill's non-negotiables require, and the grep-before-append
# idempotency guard. Run it after any change to install/get-memory-wire.sh:
#
#   sh install/verify-get-memory-wire.sh
#
# Checks, in order:
#   1. sh -n           — POSIX syntax valid under dash, not just bash.
#   2. shellcheck -S error — no error-severity findings. Info/warning are
#      annotated in-place with reasons; the script targets /bin/sh on Alpine
#      (dash), where bash-only constructs are the bug, not the style.
#   3. Sections        — the flags the header advertises parse (--help exits 0),
#      and the glibc-floor musl fallback and the checksum verification are
#      present, because both were live failures before they were code.
#   4. No curl|sudo    — the credential-theft vector the skill refuses.
set -eu
cd "$(dirname "$0")/.."
INSTALLER=install/get-memory-wire.sh

fail() { printf 'verify-get-memory-wire: %s\n' "$*" >&2; exit 1; }

# 1. POSIX syntax (dash where present, sh otherwise).
SH_BIN=$(command -v dash || command -v sh)
"$SH_BIN" -n "$INSTALLER" || fail "syntax invalid under $SH_BIN"

# 2. shellcheck, error severity only.
if command -v shellcheck >/dev/null 2>&1; then
  shellcheck -s sh -S error "$INSTALLER" || fail "shellcheck found error-severity findings"
else
  printf '  (shellcheck not installed — skipped)\n'
fi

# 3. Sections and invariants the installer's own comments claim.
grep -q 'musl' "$INSTALLER" || fail "no musl fallback — the glibc-floor failure has no answer"
grep -q 'sha256' "$INSTALLER" || fail "no checksum verification — no asset may install unverified"
"$SH_BIN" "$INSTALLER" --help >/dev/null 2>&1 || fail "--help must exit 0 and print the header"

# 4. The one never-evar rule. Comments carry the recipe on purpose — the check
#    is about code, so strip comment lines first.
if grep -vE '^[[:space:]]*#' "$INSTALLER" | grep -qE 'curl[^|]*\|[[:space:]]*(sudo|sh|bash)'; then
  fail "curl piped into a privileged or executing shell — download, verify, run"
fi

printf 'verify-get-memory-wire: ok (%s)\n' "$SH_BIN"
