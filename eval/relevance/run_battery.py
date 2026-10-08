#!/usr/bin/env python3
"""Run the labelled relevance battery against a live bank.

Each case in cases.jsonl is recalled through the same REST route every
surface uses, in two shapes:

- ``auto``   — the auto-injection shape (``exclude_tags`` as the surfaces
               send them, budget 1200): what a turn in flight receives.
- ``search`` — the explicit-search shape (no exclusions, budget 2000):
               what a model calling ``memory_recall`` sees.

Labels are content prefixes: ``gold`` must rank, ``junk`` must not
auto-inject. The report is the artifact a relevance-gating decision cites;
numbers belong in docs/CONSISTENCY.md, never in a LongMemEval claim.
"""

from __future__ import annotations

import argparse
import json
import pathlib
import urllib.request

HERE = pathlib.Path(__file__).parent
DEFAULT_ENDPOINT = "http://127.0.0.1:8888"
AUTO_EXCLUDE = ["transcript", "marker"]
AUTO_BUDGET = 1200
SEARCH_BUDGET = 2000


def recall(endpoint: str, bank: str, query: str, budget: int, exclude: list[str]):
    body = {"query": query, "budget": budget, "format": "full"}
    if exclude:
        body["exclude_tags"] = exclude
    req = urllib.request.Request(
        f"{endpoint}/banks/{bank}/recall",
        data=json.dumps(body).encode(),
        headers={"content-type": "application/json"},
        method="POST",
    )
    with urllib.request.urlopen(req, timeout=10) as resp:
        return json.loads(resp.read())


def hits(hits: list) -> str:
    def flat(c: str) -> str:
        return " ".join(c.split())

    return " | ".join(flat(h["content"])[:70] for h in hits[:5])


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--bank", default="omp")
    ap.add_argument("--endpoint", default=DEFAULT_ENDPOINT)
    ap.add_argument("--case", help="run only this case id")
    args = ap.parse_args()

    cases = [json.loads(l) for l in (HERE / "cases.jsonl").read_text().splitlines() if l.strip()]
    if args.case:
        cases = [c for c in cases if c["id"] == args.case]
        if not cases:
            print(f"no such case: {args.case}")
            return 1

    auto_gold = auto_junk = 0
    auto_cost = 0
    print(f"{'case':<14}{'gold@5':>7}{'junk@5':>8}{'chars':>7}   top-5 (auto shape)")
    for c in cases:
        top = recall(args.endpoint, args.bank, c["query"], AUTO_BUDGET, AUTO_EXCLUDE)
        texts = [h["content"] for h in top[:5]]
        gold = any(any(t.startswith(g) or g in t for g in c["gold"]) for t in texts) if c["gold"] else None
        junk = any(any(j in t for j in c["junk"]) for t in texts) if c["junk"] else False
        cost = sum(min(len(t), 400) for t in texts)
        auto_cost += cost
        mark_gold = "-" if gold is None else ("Y" if gold else "!")
        mark_junk = "Y" if junk else "-"
        if gold:
            auto_gold += 1
        if junk:
            auto_junk += 1
        print(f"{c['id']:<14}{mark_gold:>7}{mark_junk:>8}{cost:>7}   {hits(top)}")

    print(f"\nauto shape: gold {auto_gold}/{sum(1 for c in cases if c['gold'])} "
          f"junk-injected {auto_junk}/{sum(1 for c in cases if c['junk'])} "
          f"~{auto_cost} chars total across {len(cases)} cases")
    print("\nsearch shape (what memory_recall sees, no exclusions):")
    for c in cases:
        top = recall(args.endpoint, args.bank, c["query"], SEARCH_BUDGET, [])
        reachable = any(any(g in h["content"] for h in top[:5]) for g in c["gold"]) if c["gold"] else None
        tag = "-" if reachable is None else ("Y" if reachable else "!")
        print(f"  {c['id']:<14} gold-reachable@5: {tag}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
