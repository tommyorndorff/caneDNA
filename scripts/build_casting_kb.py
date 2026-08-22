#!/usr/bin/env python3
"""Build a casting knowledge base from the RMA listserv archive.

Links sentences describing how a rod *casts* to the makers AND specific models in
our taper library, and tags each snippet with the casting "action" language it
uses (fast / slow / parabolic / …). The idea: decades of organic "how does this
rod feel/cast" discussion becomes structured, attributed evidence we can surface
next to a taper and query by action.

v2 approach (transparent + conservative):
  * Maker vocabulary = first token of each library rod name (kept when it looks
    like a recurring surname), unioned with a curated core.
  * Model vocabulary = per maker, the second token of each library name that
    looks like a model designator (e.g. "Payne 98" -> 98, "Garrison 212" -> 212).
  * A message body is split into sentences; a sentence is captured when it
    mentions a maker AND casting-descriptive language. If it also names one of
    that maker's model designators, it is additionally linked to that model.
  * Each snippet is tagged with the action language it contains.

Output: data/kb/casting_kb.json — aggregated by maker and by model, capped with
true totals recorded (no silent truncation).
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
MAX_SNIPPETS_PER_MODEL = 20
SNIPPET_MAXLEN = 400

CURATED_MAKERS = {
    "Payne", "Garrison", "Leonard", "Dickerson", "Cattanach", "Young",
    "Thomas", "Gillum", "Edwards", "Orvis", "Winston", "Powell", "Phillipson",
    "Granger", "Heddon", "Sharpe", "Hardy", "Halstead", "Howells", "Carmichael",
}
NOT_MAKERS = {
    "fly", "dry", "wet", "midge", "baitcast", "casting", "spinning", "custom",
    "unknown", "the", "small", "light", "heavy", "spey", "rod", "bamboo", "quad",
    "hex", "penta", "para", "parabolic", "trout", "salmon", "bass", "medium",
}
MODEL_STOP = {"the", "and", "for", "rod", "fly", "no", "mk"}

# Casting-descriptive language (for capturing a sentence at all).
CASTING_TERMS = [
    r"casts?", r"casting", r"action", r"fast[- ]action", r"slow[- ]action",
    r"medium[- ]action", r"tip[- ]?heavy", r"tip[- ]?action", r"butt[- ]?action",
    r"delicate", r"crisp", r"presentation", r"loads?\b", r"recovery",
    r"parabolic", r"progressive", r"dry[- ]fly", r"wet[- ]fly", r"powerful",
    r"forgiving", r"tracks?\b", r"dampen\w*", r"line speed", r"mends?\b",
    r"stiff", r"soft\b", r"smooth", r"wobble", r"lively", r"responsive",
]
CASTING_RE = re.compile(r"\b(?:" + "|".join(CASTING_TERMS) + r")\b", re.I)

# Action tags: label -> matching pattern. A snippet may carry several.
ACTION_TAGS = {
    "fast": r"fast(?:[- ]action)?|quick|crisp|stiff|tip[- ]?heavy",
    "slow": r"slow(?:[- ]action)?|soft\b|full[- ]?flex|noodl",
    "medium": r"medium(?:[- ]action)?|moderate",
    "parabolic": r"parabolic|para\b|semi[- ]?para",
    "progressive": r"progressive",
    "delicate": r"delicate|dry[- ]fly|presentation|light",
    "powerful": r"powerful|power\b|strong|hauls?|distance",
    "smooth": r"smooth|tracks?\b|dampen\w*|forgiving|lively|responsive",
    "wet-fly": r"wet[- ]fly|nymph",
}
ACTION_RES = {k: re.compile(v, re.I) for k, v in ACTION_TAGS.items()}

SENT_SPLIT = re.compile(r"(?<=[.!?])\s+")
WS = re.compile(r"\s+")


def library_vocab():
    """Return (makers set, maker_lc -> {model_token_lc -> display 'Maker Model'})."""
    data = json.loads(TAPERS.read_text(encoding="utf-8"))
    counts = defaultdict(int)
    models = defaultdict(dict)
    for m in data["models"]:
        name = (m.get("name") or "").strip()
        if not name:
            continue
        toks = re.split(r"[\s,]+", name)
        maker = toks[0].strip("’'\"().")
        low = maker.lower()
        if len(maker) < 4 or not maker[0].isalpha() or low in NOT_MAKERS:
            continue
        if not maker[:1].isupper():
            continue
        counts[maker] += 1
        # Second token as a model designator, if it looks like one.
        if len(toks) > 1:
            model = toks[1].strip("’'\"().")
            ml = model.lower()
            if len(model) >= 2 and ml not in MODEL_STOP and any(c.isalnum() for c in model):
                models[low][ml] = f"{maker} {model}"
    makers = {m for m, c in counts.items() if c >= 3} | CURATED_MAKERS
    # Keep only model vocab for makers we kept.
    kept_lc = {m.lower() for m in makers}
    models = {mk: mv for mk, mv in models.items() if mk in kept_lc}
    return makers, models


def clean(text):
    return WS.sub(" ", text).strip()


def is_quoted(line):
    return line.lstrip().startswith((">", "|"))


def tag_actions(sent):
    return sorted(k for k, rx in ACTION_RES.items() if rx.search(sent))


def new_bucket():
    return {"mentions": 0, "snippets": [], "action_counts": defaultdict(int)}


def add_snippet(bucket, cap, quote, actions, msg):
    bucket["mentions"] += 1
    for a in actions:
        bucket["action_counts"][a] += 1
    if len(bucket["snippets"]) < cap:
        bucket["snippets"].append({
            "quote": quote,
            "actions": actions,
            "year": msg["year"],
            "date": msg["date"],
            "subject": msg["subject"],
            "author": msg["from_name"] or msg["from_email"],
        })


def serialize(bucket, label=None):
    out = {
        "mentions_with_casting": bucket["mentions"],
        "snippets_shown": len(bucket["snippets"]),
        "action_counts": dict(sorted(bucket["action_counts"].items(),
                                     key=lambda kv: kv[1], reverse=True)),
        "snippets": bucket["snippets"],
    }
    if label:
        out["label"] = label
    return out


def main():
    if not TAPERS.exists():
        sys.exit(f"missing {TAPERS}")
    makers, model_vocab = library_vocab()
    maker_res = {m: re.compile(r"\b" + re.escape(m) + r"\b", re.I) for m in makers}
    # Per maker (lc): {model_token_lc: (display, compiled regex)}
    model_res = {
        mk: {tok: (disp, re.compile(r"\b" + re.escape(tok) + r"\b", re.I))
             for tok, disp in toks.items()}
        for mk, toks in model_vocab.items()
    }

    maker_buckets = defaultdict(new_bucket)   # maker display -> bucket
    model_buckets = {}                        # model_key_lc -> (display, bucket)
    scanned = 0

    for msg in iter_messages():
        scanned += 1
        body = msg.get("body") or ""
        unquoted = "\n".join(l for l in body.splitlines() if not is_quoted(l))
        if not CASTING_RE.search(unquoted):
            continue
        for sent in SENT_SPLIT.split(unquoted.replace("\n", " ")):
            if not CASTING_RE.search(sent):
                continue
            actions = tag_actions(sent)
            quote = clean(sent)
            if len(quote) > SNIPPET_MAXLEN:
                quote = quote[:SNIPPET_MAXLEN].rstrip() + "…"
            for maker, rx in maker_res.items():
                if not rx.search(sent):
                    continue
                add_snippet(maker_buckets[maker], MAX_SNIPPETS_PER_MAKER,
                            quote, actions, msg)
                # Model-level: does the sentence also name one of this maker's models?
                for tok, (disp, mrx) in model_res.get(maker.lower(), {}).items():
                    if mrx.search(sent):
                        key = disp.lower()
                        if key not in model_buckets:
                            model_buckets[key] = (disp, new_bucket())
                        add_snippet(model_buckets[key][1], MAX_SNIPPETS_PER_MODEL,
                                    quote, actions, msg)

    makers_out = {
        m: serialize(b)
        for m, b in sorted(maker_buckets.items(),
                           key=lambda kv: kv[1]["mentions"], reverse=True)
    }
    models_out = {
        key: serialize(b, label=disp)
        for key, (disp, b) in sorted(model_buckets.items(),
                                     key=lambda kv: kv[1][1]["mentions"], reverse=True)
    }

    payload = {
        "meta": {
            "version": 2,
            "source": "Rodmakers (RMA) listserv archive, 1995-2004",
            "source_url": "https://www.hexrod.net/RMA_allmsg/index.html",
            "license": "listserv archive; attribute the Rodmakers list / hexrod.net",
            "method": (
                "sentence-level co-occurrence of a library maker (and, when named, "
                "model designator) with casting-descriptive language; quoted reply "
                "lines excluded; snippets tagged with action language"
            ),
            "action_tags": list(ACTION_TAGS.keys()),
            "messages_scanned": scanned,
            "makers": len(makers_out),
            "models": len(models_out),
            "snippet_cap_per_maker": MAX_SNIPPETS_PER_MAKER,
            "snippet_cap_per_model": MAX_SNIPPETS_PER_MODEL,
        },
        "makers": makers_out,
        "models": models_out,
    }
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(payload, indent=2, ensure_ascii=False), encoding="utf-8")
    print(f"scanned {scanned} messages; {len(makers_out)} makers, "
          f"{len(models_out)} models with casting mentions", file=sys.stderr)
    print(f"wrote {OUT}", file=sys.stderr)


if __name__ == "__main__":
    main()
