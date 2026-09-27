#!/usr/bin/env bash
# Fetch the official LongMemEval-S dataset (not vendored, 264 MB).
# Source: https://huggingface.co/datasets/xiaowu0162/longmemeval-cleaned
set -euo pipefail
cd "$(dirname "$0")"
mkdir -p data
URL="https://huggingface.co/datasets/xiaowu0162/longmemeval-cleaned/resolve/main/longmemeval_s_cleaned.json"
if [ -f data/longmemeval_s_cleaned.json ]; then
  echo "already present: data/longmemeval_s_cleaned.json"
else
  curl -sL -o data/longmemeval_s_cleaned.json "$URL"
fi
ls -la data/
