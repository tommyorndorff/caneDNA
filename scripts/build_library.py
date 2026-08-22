#!/usr/bin/env python3
"""Merge all per-source taper files into the single library the app loads.

Reads every data/sources/*.json (each a list of rod models produced by an
importer such as convert_tapers.py or import_hexrod.py) and writes the merged
data/tapers.json with a top-level meta block. Attribution lives per-taper in
each record's `provenance`, so merging is a simple concatenation.
"""
import argparse
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SRC_DIR = ROOT / "data" / "sources"
OUT = ROOT / "data" / "tapers.json"

# When two sources record the same rod name, keep the record from the
# higher-priority source. Hexrod's rows carry cleaner geometry/feet fields, so
# it wins over RodDNA on a name collision.
SOURCE_PRIORITY = {
    "David Ray's Taper Library (Hexrod)": 2,
    "RodDNA v2.0": 1,
    "RodDNA v1.4 update": 1,
}


def norm(m):
    return " ".join((m.get("name") or "").lower().split())


def priority(m):
    return SOURCE_PRIORITY.get((m.get("provenance") or {}).get("source"), 0)


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--keep-all", action="store_true",
                    help="keep duplicate names instead of deduping")
    args = ap.parse_args()

    models = []
    files = sorted(SRC_DIR.glob("*.json"))
    if not files:
        sys.exit(f"no source files in {SRC_DIR} — run the importers first")
    for f in files:
        data = json.loads(f.read_text(encoding="utf-8"))
        print(f"{f.name}: {len(data)} models", file=sys.stderr)
        models.extend(data)

    total_in = len(models)

    if not args.keep_all:
        # Dedup by normalized name, keeping the highest-priority source.
        # dict preserves first-seen order; replacing a value keeps its slot.
        kept = {}
        dropped = 0
        for m in models:
            k = norm(m)
            if k not in kept:
                kept[k] = m
            elif priority(m) > priority(kept[k]):
                kept[k] = m
                dropped += 1  # replaced one we'd previously kept
            else:
                dropped += 1
        models = list(kept.values())
        print(f"dedup: {total_in} -> {len(models)} ({dropped} dropped, "
              f"prefer Hexrod on name collision)", file=sys.stderr)

    # Distinct source libraries represented, for the summary line.
    sources = sorted({
        (m.get("provenance") or {}).get("source", "unknown") for m in models
    })

    payload = {
        "meta": {
            "source": "Multiple libraries — see each taper's provenance",
            "sources": sources,
            "units": {
                "length": "inches",
                "dimensions": "inches (flat-to-flat cross-section)",
                "stations": "inches from tip",
            },
            "count": len(models),
        },
        "models": models,
    }
    OUT.write_text(json.dumps(payload, indent=2), encoding="utf-8")
    print(f"wrote {OUT} ({len(models)} models from {len(files)} sources)", file=sys.stderr)


if __name__ == "__main__":
    main()
