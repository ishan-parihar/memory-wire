#!/usr/bin/env python3
"""Build the H4 LoCoMo subset input file for AMB from the committed manifest.

The manifest (``eval/h4_subset.json``, written by ``examples/locomo_h4_subset.rs``)
is the committed record: eligibility (every gold session inside the shipped
configuration's top 3) and the seeded stratified draw live there, reproducibly.
This script only materialises the AMB-shaped input file: it drops every
``qa`` entry the manifest did not choose, keeping each conversation's chosen
questions in their original order.

Positional note: AMB derives a query's id from the *positional* index inside the
conversation's ``qa`` list, so ids in the subset file renumber relative to the
full corpus whenever an earlier question is dropped. That is fine for the H4
experiment — the subset is self-contained and AMB never crosses back into
`eval/` metrics — and the manifest keeps the original ids for provenance.

Usage:
    python3 eval/amb/h4_build_subset.py \
        [--manifest eval/h4_subset.json] \
        [--data eval/data/locomo/locomo10.json] \
        [--out "$TMPDIR/h4_locomo10_subset.json"]
"""

import argparse
import json


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--manifest", default="eval/h4_subset.json")
    ap.add_argument("--data", default="eval/data/locomo/locomo10.json")
    ap.add_argument("--out", required=True)
    args = ap.parse_args()

    manifest = json.load(open(args.manifest))
    chosen: set[str] = {q["id"] for q in manifest["queries"]}

    corpus = json.load(open(args.data))
    kept = 0
    for conv in corpus:
        qa = conv.get("qa", [])
        sample_id = conv["sample_id"]
        picked = []
        for qi, entry in enumerate(qa):
            if f"{sample_id}_q{qi}" in chosen:
                picked.append(entry)
        kept += len(picked)
        conv["qa"] = picked

    assert kept == len(chosen), f"manifest ids missing from the corpus: {kept} != {len(chosen)}"
    with open(args.out, "w") as fh:
        json.dump(corpus, fh)
    print(f"h4_build_subset: wrote {args.out} — {kept} questions across {len(corpus)} conversations")


if __name__ == "__main__":
    main()
