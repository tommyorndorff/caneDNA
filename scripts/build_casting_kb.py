#!/usr/bin/env python3
"""Build a casting knowledge base from the RMA listserv archive.

Links sentences that describe how a rod *casts* to the rod makers in our taper
library. The idea: decades of organic "how does this rod feel/cast" discussion
becomes structured, attributed evidence we can surface next to a taper.

v1 approach (transparent + conservative):
  * Maker vocabulary is derived from the taper library (the first token of each
    rod name), kept when it looks like a surname and recurs, unioned with a small
    curated core of well-known makers.
  * A message body is split into sentences; a sentence is captured when it
    mentions a maker AND contains casting-descriptive language.
  * Each captured snippet keeps its citation (year, date, subject, author).

Output: data/kb/casting_kb.json — aggregated by maker, capped per maker with the
true totals recorded (no silent truncation).

Run parse first is not required; this imports the parser directly.
"""
import json
import re
import sys
from collections import defaultdict
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from parse_rma import iter_messages  # noqa: E402

ROOT = Path(__file__).resolve().parent.parent
TAPERS = ROOT / "data" / "tapers.json"
OUT = ROOT / "data" / "kb" / "casting_kb.json"

MAX_SNIPPETS_PER_MAKER = 40
SNIPPET_MAXLEN = 400

# Makers we always include even if sparse in the library.
CURATED_MAKERS = {
    "Payne", "Garrison", "Leonard", "Dickerson", "Cattanach", "Young",
    "Thomas", "Gillum", "Edwards", "Orvis", "Winston", "Powell", "Phillipson",
    "Granger", "Heddon", "Sharpe", "Hardy", "Halstead", "Howells", "Carmichael",
}

# First-token library names that are not makers.
NOT_MAKERS = {
    "fly", "dry", "wet", "midge", "baitcast", "casting", "spinning", "custom",
    "unknown", "the", "small", "light", "heavy", "spey", "rod", "bamboo", "quad",
    "hex", "penta", "para", "parabolic", "trout", "salmon", "bass", "medium",
}

# Casting-descriptive language. Word-boundary, case-insensitive.
CASTING_TERMS = [
    r"casts?", r"casting", r"action", r"fast[- ]action", r"slow[- ]action",
    r"medium[- ]action", r"tip[- ]?heavy", r"tip[- ]?action", r"butt[- ]?action",
    r"delicate", r"crisp", r"presentation", r"loads?\b", r"recovery",
    r"parabolic", r"progressive", r"dry[- ]fly", r"wet[- ]fly", r"powerful",
    r"forgiving", r"tracks?\b", r"dampen\w*", r"line speed", r"mends?\b",
    r"stiff", r"soft\b", r"smooth", r"wobble", r"lively", r"responsive",
]
CASTING_RE = re.compile(r"\b(?:" + "|".join(CASTING_TERMS) + r")\b", re.I)

SENT_SPLIT = re.compile(r"(?<=[.!?])\s+")
WS = re.compile(r"\s+")


def library_makers():
    data = json.loads(TAPERS.read_text(encoding="utf-8"))
    counts = defaultdict(int)
    for m in data["models"]:
        name = (m.get("name") or "").strip()
        if not name:
            continue
        tok = re.split(r"[\s,]", name, maxsplit=1)[0].strip("’'\"().")
        low = tok.lower()
        if len(tok) < 4 or not tok[0].isalpha() or low in NOT_MAKERS:
            continue
        if not tok[:1].isupper():
            continue
        counts[tok] += 1
    # Keep makers recurring in the library, plus the curated core.
    makers = {m for m, c in counts.items() if c >= 3} | CURATED_MAKERS
    return makers


def clean(text):
    return WS.sub(" ", text).strip()


def is_quoted(line):
    # Skip quoted reply lines to bias toward original opinions.
    return line.lstrip().startswith((">", "|"))


def main():
    if not TAPERS.exists():
        sys.exit(f"missing {TAPERS}")
    makers = library_makers()
    # One regex per maker for whole-word matching.
    maker_res = {m: re.compile(r"\b" + re.escape(m) + r"\b", re.I) for m in makers}

    hits = defaultdict(list)      # maker -> [snippet dict]
    totals = defaultdict(int)     # maker -> count of matching sentences
    scanned = 0

    for msg in iter_messages():
        scanned += 1
        body = msg.get("body") or ""
        # Drop quoted lines, then split into sentences.
        unquoted = "\n".join(l for l in body.splitlines() if not is_quoted(l))
        if not CASTING_RE.search(unquoted):
            continue
        for sent in SENT_SPLIT.split(unquoted.replace("\n", " ")):
            if not CASTING_RE.search(sent):
                continue
            for maker, rx in maker_res.items():
                if rx.search(sent):
                    totals[maker] += 1
                    if len(hits[maker]) < MAX_SNIPPETS_PER_MAKER:
                        quote = clean(sent)
                        if len(quote) > SNIPPET_MAXLEN:
                            quote = quote[:SNIPPET_MAXLEN].rstrip() + "…"
                        hits[maker].append({
                            "quote": quote,
                            "year": msg["year"],
                            "date": msg["date"],
                            "subject": msg["subject"],
                            "author": msg["from_name"] or msg["from_email"],
                        })

    makers_out = {}
    for maker in sorted(totals, key=lambda m: totals[m], reverse=True):
        makers_out[maker] = {
            "mentions_with_casting": totals[maker],
            "snippets_shown": len(hits[maker]),
            "snippets": hits[maker],
        }

    payload = {
        "meta": {
            "source": "Rodmakers (RMA) listserv archive, 1995-2004",
            "source_url": "https://www.hexrod.net/RMA_allmsg/index.html",
            "license": "listserv archive; attribute the Rodmakers list / hexrod.net",
            "method": (
                "sentence-level co-occurrence of a library maker name with "
                "casting-descriptive language; quoted reply lines excluded"
            ),
            "messages_scanned": scanned,
            "makers": len(makers_out),
            "snippet_cap_per_maker": MAX_SNIPPETS_PER_MAKER,
        },
        "makers": makers_out,
    }
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(payload, indent=2, ensure_ascii=False), encoding="utf-8")
    total_snips = sum(len(v) for v in hits.values())
    print(f"scanned {scanned} messages; {len(makers_out)} makers with casting "
          f"mentions; {total_snips} snippets kept (cap {MAX_SNIPPETS_PER_MAKER}/maker)",
          file=sys.stderr)
    print(f"wrote {OUT}", file=sys.stderr)


if __name__ == "__main__":
    main()
