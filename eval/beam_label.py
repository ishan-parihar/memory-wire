#!/usr/bin/env python3
"""BEAM-100k labelling generator — H9's pool-scale dev set (RND_PLAN §6.1).

Upstream BEAM (ICLR 2026, `mohammadtavakoli78/BEAM`, HF `Mohammadta/BEAM`,
CC BY-SA 4.0) carries 20 conversations at the 100k scale, each split into
3-5 sessions of ~100K characters — session-as-document would give 3-5-document
banks, *smaller* than LoCoMo's 19-32 and useless for the §14.2 pool-scale
question. The corpus's native QA structure gives the right unit instead: every
user turn has exactly one assistant reply, so one document per **QA turn pair**
(~143 per bank) is both the pool scale the re-test needs and the granularity
the product's own hooks write at (the live `omp` bank is turn rows).

Emits, under `eval/data/beam/` (gitignored — upstream content, never committed):

- `documents.json` — one document per turn pair, in the schema
  `eval/locomo_dev.rs`'s `Document` expects: `content` is a JSON-encoded
  `[{"speaker", "text"}]` list, `timestamp` the pair's `time_anchor` as
  YYYY-MM-DD.
- `queries.json` — the 400 probing questions with agent-authored gold pair
  ids, in the `Query` schema: `gold_answers` holds the upstream
  `ideal_response` (never scored — the harness is retrieval-only),
  `meta.category` the BEAM dimension.

Also writes `eval/beam_labels_100k.json` — the **committed** record of the
labels: ids, match scores and resolve stats only, **no upstream text**.

Labelling method (content-match, then review): the gold answer prose's
distinctive terms are matched against each turn pair of the question's own
conversation; a pair containing >= THRESH of those terms is gold, capped at
MAX_GOLD by score. `abstention` questions have no gold by design (being
unanswerable is the point); `summarization` answers synthesize the whole
conversation and have no localizable gold. Both are emitted with empty
`gold_ids` and are dropped **and counted** by the harness, never scored.

The matcher is tuned ONLY against label-review agreement, never against any
retrieval metric (`eval/relevance/README.md` discipline). Self-authored gold
carries the authoring-weakness caveat in every artifact that uses it.

Usage:
    python3 eval/beam_label.py                       # label
    python3 eval/beam_label.py --sample 25 --seed 7  # print a review sample
"""

from __future__ import annotations

import argparse
import ast
import datetime as dt
import hashlib
import json
import pathlib
import random
import re

HERE = pathlib.Path(__file__).parent
DATA = HERE / "data" / "beam"

# Matching knobs. Chosen against label-review agreement only; a change here is a
# re-labelling and must be recorded in the digest's config block.
THRESH = 0.28
# Dimensions whose gold answer spans several statements (contradictions name both
# sides, event orderings list 4-6 items, updates name old and new) get a wider
# gold cap than single-fact dimensions.
MAX_GOLD_MULTI = {"contradiction_resolution", "event_ordering", "knowledge_update", "multi_session_reasoning"}
MAX_GOLD = 5
MAX_GOLD_SINGLE = 3
# An answer whose distinctive set is this small cannot clear a fraction bar —
# a single shared generic term would label half the bank. It must instead be
# mostly *contained* in a pair (e.g. "1,200 calls per day" → the pair that says
# exactly that).
SMALL_SET = 3
SMALL_SET_THRESH = 0.67

STOP = {
    "about", "after", "again", "their", "there", "these", "those", "this",
    "that", "with", "from", "have", "been", "were", "will", "would", "could",
    "should", "into", "than", "then", "them", "they", "when", "what", "your",
    "some", "such", "only", "also", "just", "like", "make", "made", "does",
    "doing", "because", "before", "being", "between", "both", "each", "more",
    "most", "other", "over", "same", "under", "until", "very", "while", "where",
    "which", "yourself", "here", "user", "chat",
}
# Prose boilerplate of BEAM's ideal_response fields, stripped before matching.
ANSWER_BOILER = {
    "based", "provided", "according", "conversation", "information",
    "mentioned", "related", "however", "details", "detail", "specified",
    "explicitly", "directly", "response", "question", "answer", "probing",
    "provided", "given", "stated", "statement", "user", "users",
}

_DIMS_WITH_NO_LOCALIZABLE_GOLD = {"abstention", "summarization"}


def terms(text: str) -> set[str]:
    # Commas inside numbers are not separators ("1,200" is one token), and a
    # numeric token of 2+ digits is a term at any length: a quota figure is more
    # distinctive than any word.
    lowered = re.sub(r"(?<=\d),(?=\d)", "", text.lower())
    return {
        t
        for t in re.findall(r"[a-z0-9']+", lowered)
        if len(t) >= 4 or (len(t) >= 2 and t.isdigit())
    } - STOP


def answer_terms(ideal_response: str) -> set[str]:
    return terms(ideal_response) - ANSWER_BOILER


def parse_time_anchor(raw: str | None) -> str | None:
    if not raw:
        return None
    try:
        return dt.datetime.strptime(raw, "%B-%d-%Y").strftime("%Y-%m-%d")
    except ValueError:
        return None


def sha256(path: pathlib.Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 16), b""):
            h.update(chunk)
    return h.hexdigest()


def build_pairs(conv: dict) -> list[dict]:
    """One {id, turns} per QA pair: a user turn plus the assistant turns that
    follow it, in conversation order across the chat's sessions."""
    pairs = []
    current = None
    for session in conv["chat"]:
        for turn in session if isinstance(session, list) else []:
            role = turn.get("role", "")
            if role == "user":
                if current is not None:
                    pairs.append(current)
                current = {
                    "id": f"{conv['conversation_id']}-p{len(pairs):03d}",
                    "turns": [turn],
                }
            elif current is not None:
                current["turns"].append(turn)
    if current is not None:
        pairs.append(current)
    return pairs


def pair_document(pair: dict, conv_id: str) -> dict:
    return {
        "id": pair["id"],
        "user_id": conv_id,
        "content": json.dumps(
            [
                {"speaker": t.get("role", "?"), "text": t.get("content", "")}
                for t in pair["turns"]
            ],
            ensure_ascii=False,
        ),
        "timestamp": parse_time_anchor(pair["turns"][0].get("time_anchor")),
    }


def pair_text(pair: dict) -> str:
    return "\n".join(t.get("content", "") for t in pair["turns"])


CLAUSE_PIVOT = re.compile(
    r",? but you also (?:mentioned |said )?"
    r"|\. You said "
    r"| but also said "
)


def label_question(question: dict, dimension: str, pairs: list[dict]) -> tuple[list[str], float]:
    """Gold pair ids for one question: the pairs of its own conversation that
    contain a threshold share of the answer's distinctive terms, top-capped by
    score, ties by conversation order. Empty when nothing clears THRESH.

    `contradiction_resolution` ideals are formulaic — "You said X, but you also
    said Y" — and a single whole-ideal match floods the cap with whichever side
    is near-verbatim in the corpus while the paraphrased other side ("I just
    attended a workshop on international resume standards") sits just under
    the bar. So that dimension matches each *statement* of the contradiction
    separately and unions the results: both sides become gold by construction.
    """
    ideal = (
        question.get("answer")
        or question.get("ideal_response")
        or question.get("ideal_answer")
        or question.get("ideal_summary")
        or question.get("expected_compliance")
        or ""
    )
    if dimension == "contradiction_resolution":
        clauses = [c for c in CLAUSE_PIVOT.split(ideal) if c and len(terms(c)) > 1]
        if len(clauses) >= 2:
            picked: list[str] = []
            top = 0.0
            for clause in clauses:
                a = answer_terms(clause)
                if not a:
                    continue
                thresh = SMALL_SET_THRESH if len(a) <= SMALL_SET else THRESH
                scored = []
                for pair in pairs:
                    score = len(a & terms(pair_text(pair))) / len(a)
                    if score >= thresh:
                        scored.append((score, pair["id"]))
                scored.sort(key=lambda s: (-s[0], s[1]))
                for _, pid in scored[:MAX_GOLD_SINGLE]:
                    if pid not in picked:
                        picked.append(pid)
                if scored:
                    top = max(top, scored[0][0])
            if picked:
                return picked[:MAX_GOLD], top
        # A contradiction ideal that did not split still falls through to the
        # whole-ideal match below.
    a = answer_terms(ideal)
    if not a:
        return [], 0.0
    thresh = SMALL_SET_THRESH if len(a) <= SMALL_SET else THRESH
    scored = []
    for pair in pairs:
        u = terms(pair_text(pair))
        score = len(a & u) / len(a)
        if score >= thresh:
            scored.append((score, pair["id"]))
    scored.sort(key=lambda s: (-s[0], s[1]))
    cap = MAX_GOLD if dimension in MAX_GOLD_MULTI else MAX_GOLD_SINGLE
    return [pid for _, pid in scored[:cap]], (scored[0][0] if scored else 0.0)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--input", default=str(DATA / "beam_100k.json"))
    ap.add_argument("--sample", type=int, default=0)
    ap.add_argument("--seed", type=int, default=7)
    args = ap.parse_args()
    src = pathlib.Path(args.input)

    documents: list[dict] = []
    queries: list[dict] = []
    digest: list[dict] = []
    stats = {"conversations": 0, "pairs": 0, "questions": 0, "by_dimension": {}}

    with open(src) as f:
        for line in f:
            conv = json.loads(line)
            conv_id = conv["conversation_id"]
            stats["conversations"] += 1
            pairs = build_pairs(conv)
            stats["pairs"] += len(pairs)
            documents.extend(pair_document(p, conv_id) for p in pairs)
            pair_term_cache = {p["id"]: terms(pair_text(p)) for p in pairs}

            pq = conv.get("probing_questions", {})
            if isinstance(pq, str):
                pq = ast.literal_eval(pq)
            for dimension, q_list in pq.items():
                dstat = stats["by_dimension"].setdefault(
                    dimension, {"total": 0, "resolved": 0, "no_gold_by_design": 0, "unresolved": 0}
                )
                for question in q_list:
                    stats["questions"] += 1
                    dstat["total"] += 1
                    if dimension in _DIMS_WITH_NO_LOCALIZABLE_GOLD:
                        gold: list[str] = []
                        dstat["no_gold_by_design"] += 1
                        top = 0.0
                    else:
                        gold, top = label_question(question, dimension, pairs)
                        if gold:
                            dstat["resolved"] += 1
                        else:
                            dstat["unresolved"] += 1
                    qid = f"{conv_id}-q{len(queries):02d}"
                    queries.append(
                        {
                            "id": qid,
                            "query": question.get("question", ""),
                            "gold_answers": [
                                question.get("answer")
                                or question.get("ideal_response")
                                or question.get("ideal_answer")
                                or question.get("ideal_summary")
                                or question.get("expected_compliance")
                                or ""
                            ],
                            "gold_ids": gold,
                            "user_id": conv_id,
                            "meta": {"category": dimension},
                        }
                    )
                    digest.append(
                        {
                            "query_id": qid,
                            "conv_id": conv_id,
                            "dimension": dimension,
                            "gold_pair_ids": gold,
                            "top_score": round(top, 4),
                        }
                    )

    DATA.mkdir(parents=True, exist_ok=True)
    (DATA / "documents.json").write_text(json.dumps(documents, ensure_ascii=False))
    (DATA / "queries.json").write_text(json.dumps(queries, ensure_ascii=False))
    (HERE / "beam_labels_100k.json").write_text(
        json.dumps(
            {
                "generator": "eval/beam_label.py",
                "input_sha256": sha256(src),
                "config": {
                    "thresh": THRESH,
                    "max_gold_multi": MAX_GOLD,
                    "max_gold_single": MAX_GOLD_SINGLE,
                    "small_set": SMALL_SET,
                    "small_set_thresh": SMALL_SET_THRESH,
                },
                "unit": "one document per QA turn pair (user turn + its assistant reply)",
                "stats": stats,
                "labels": digest,
            },
            indent=1,
        )
    )

    resolved = sum(v["resolved"] for v in stats["by_dimension"].values())
    no_gold = sum(v["no_gold_by_design"] for v in stats["by_dimension"].values())
    unresolved = sum(v["unresolved"] for v in stats["by_dimension"].values())
    print(f"conversations {stats['conversations']}, pairs {stats['pairs']}, "
          f"questions {stats['questions']}")
    print(f"resolved {resolved}, no-gold-by-design {no_gold}, unresolved {unresolved}")
    per_bank = [d for d in documents]
    banks: dict[str, int] = {}
    for d in per_bank:
        banks[d["user_id"]] = banks.get(d["user_id"], 0) + 1
    sizes = sorted(banks.values())
    print(f"banks {len(banks)}, docs/bank min/med/max "
          f"{sizes[0]}/{sizes[len(sizes) // 2]}/{sizes[-1]}")
    for dim in sorted(stats["by_dimension"]):
        v = stats["by_dimension"][dim]
        print(f"  {dim:26} total {v['total']:3} resolved {v['resolved']:3} "
              f"by-design {v['no_gold_by_design']:3} unresolved {v['unresolved']:3}")

    if args.sample:
        # Review sample: the question, its gold pairs' text, for hand-verification
        # of the labels. Agreement issues fix the matcher, never the other way.
        rng = random.Random(args.seed)
        resolved_qs = [q for q in queries if q["gold_ids"]]
        docs_by_id = {d["id"]: d for d in documents}
        for q in rng.sample(resolved_qs, min(args.sample, len(resolved_qs))):
            print("=" * 100)
            print(f"[{q['meta']['category']}] {q['id']}: {q['query']}")
            print(f"  ideal: {q['gold_answers'][0][:240]}")
            for gid in q["gold_ids"]:
                turns = json.loads(docs_by_id[gid]["content"])
                text = " / ".join(f"{t['speaker']}: {t['text'][:150]}" for t in turns)
                print(f"  GOLD {gid}: {text[:360]}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
