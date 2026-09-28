#!/usr/bin/env bash
# Fetch the official evaluation corpora. Neither is vendored; `data/` is
# gitignored.
#
#   LongMemEval-S — 264 MB, the *test* set. Measurement and release acceptance
#     only, never selection (`AGENTS.md` §1).
#     Source: https://huggingface.co/datasets/xiaowu0162/longmemeval-cleaned
#
#   LoCoMo — 740 KB, the *dev* set. All selection happens here.
#     Source: vectorize-io/agent-memory-benchmark `data/locomo/locomo10/`, the
#     canonical LoCoMo distribution used by Snap Research's benchmark, pinned to
#     the upstream commit below rather than to `main` so a rerun fetches the same
#     bytes. See `docs/EVALUATION_HYGIENE.md` §3.2.
#
# Idempotent: a file already present with the expected record count is left
# alone, and a truncated one is re-fetched. Fails loudly, nonzero exit, on a
# checksum or shape mismatch — a silently wrong corpus is a wrong measurement.
set -euo pipefail
cd "$(dirname "$0")"
mkdir -p data

# ---------------------------------------------------------------- LongMemEval-S
URL="https://huggingface.co/datasets/xiaowu0162/longmemeval-cleaned/resolve/main/longmemeval_s_cleaned.json"
if [ -f data/longmemeval_s_cleaned.json ]; then
  echo "already present: data/longmemeval_s_cleaned.json"
else
  curl -sL -o data/longmemeval_s_cleaned.json "$URL"
fi

# ------------------------------------------------------------------------ LoCoMo
# Materialised as *plain, un-gzipped* JSON, so the Rust harness needs no gzip
# crate: `serde_json` is already a dependency and `flate2` is not. Decompression
# belongs here rather than in the harness for that reason alone.
LOCOMO_REF="decbb07f4f9899deac28a76293564cf263872652"
LOCOMO_BASE="https://raw.githubusercontent.com/vectorize-io/agent-memory-benchmark/$LOCOMO_REF/data/locomo/locomo10"
mkdir -p data/locomo

# Linux ships `sha256sum`, macOS ships `shasum`; nothing else is assumed.
sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$1" | cut -d' ' -f1
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$1" | cut -d' ' -f1
  else
    echo "download.sh: no sha256sum or shasum on PATH; cannot verify LoCoMo" >&2
    exit 1
  fi
}

# Occurrences of one JSON key in a file. A shape check that needs no JSON parser:
# every record in both corpora carries its identifying key exactly once, so the
# count is the record count and a truncated, HTML-error or wrong-version file
# cannot reach the harness looking like a corpus. Full parse validation is the
# harness's job — `examples/locomo.rs` deserialises with serde and errors on a
# missing or mistyped field.
count_key() { grep -o "\"$2\"" "$1" 2>/dev/null | wc -l | tr -d ' '; }

# $1 file stem  $2 expected sha256 of the .gz  $3 identifying key  $4 record count
locomo_fetch() {
  local name="$1" want_sha="$2" want_key="$3" want_count="$4"
  local out="data/locomo/$name.json" tmp="data/locomo/.$name.gz.tmp"
  if [ -f "$out" ] && [ "$(count_key "$out" "$want_key")" = "$want_count" ]; then
    echo "already present: $out ($want_count records)"
    return
  fi
  echo "fetching $name.json.gz ..."
  curl -fsSL -o "$tmp" "$LOCOMO_BASE/$name.json.gz"
  local got_sha
  got_sha="$(sha256_of "$tmp")"
  if [ "$got_sha" != "$want_sha" ]; then
    rm -f "$tmp"
    echo "download.sh: CHECKSUM MISMATCH for $name.json.gz" >&2
    echo "  expected $want_sha" >&2
    echo "  got      $got_sha" >&2
    echo "  upstream $LOCOMO_BASE/$name.json.gz" >&2
    echo "  If upstream changed on purpose, update the pin here and in the" >&2
    echo "  provenance header of eval/LOCOMO.md. Do not relax the check." >&2
    exit 1
  fi
  # `gzip -dc` verifies the stream's own CRC, so corruption fails here.
  gzip -dc "$tmp" > "$out.part"
  rm -f "$tmp"
  local got_count
  got_count="$(count_key "$out.part" "$want_key")"
  if [ "$got_count" != "$want_count" ]; then
    rm -f "$out.part"
    echo "download.sh: SHAPE MISMATCH for $out — \"$want_key\" appears $got_count times, expected $want_count" >&2
    exit 1
  fi
  mv "$out.part" "$out"
  echo "wrote $out ($want_count records)"
}

locomo_fetch documents \
  ac1cbc4f58f41888c7f2bd77993a2ed3ada5b5eb9c143a8d535c1cf531da9912 \
  user_id 272
locomo_fetch queries \
  d74b7a7c4c2de0369091952d29f83b395a91c23694da7740260cc523aefa1ed1 \
  gold_ids 1540

ls -la data/ data/locomo/
