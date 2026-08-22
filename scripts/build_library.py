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

# Default tolerance (inches) for treating two tapers as the same profile.
DEFAULT_TOL = 0.0015

# When two records are the SAME taper (name + dimensions match), keep the one
# from the higher-priority source. Hexrod's rows carry cleaner geometry fields,
# so it wins. Records with the same name but a genuinely different taper are
# NOT collapsed — both are kept.
SOURCE_PRIORITY = {
    "David Ray's Taper Library (Hexrod)": 2,
    "RodDNA v2.0": 1,
    "RodDNA v1.4 update": 1,
    "2019 Bamboo Taper Sheets (Tom Morgan)": 1,
}

# Short display tags used to disambiguate same-name / different-taper rods.
SOURCE_TAG = {
    "David Ray's Taper Library (Hexrod)": "Hexrod",
    "RodDNA v2.0": "RodDNA",
    "RodDNA v1.4 update": "RodDNA",
    "2019 Bamboo Taper Sheets (Tom Morgan)": "Morgan",
}


def norm(m):
    return " ".join((m.get("name") or "").lower().split())


def src(m):
    return (m.get("provenance") or {}).get("source")


def priority(m):
    return SOURCE_PRIORITY.get(src(m), 0)


def same_taper(a, b, tol):
    """True if two records represent the same taper (equal-length dimension
    arrays whose points all agree within `tol`). Different point counts count
    as different tapers, so nothing distinct is ever silently merged."""
    da, db = a.get("dimensions") or [], b.get("dimensions") or []
    if len(da) != len(db) or not da:
        return False
    return all(abs(x - y) <= tol for x, y in zip(da, db))


def dedup(models, tol):
    """Collapse only true duplicates (name + taper match). Keeps same-name rods
    with different tapers, disambiguating their display names. Returns
    (kept_models, dropped_count, distinct_same_name_count)."""
    # Group by name, preserving first-seen order.
    groups = {}
    order = []
    for m in models:
        k = norm(m)
        if k not in groups:
            groups[k] = []
            order.append(k)
        bucket = groups[k]
        # Find an existing kept record with a matching taper.
        hit = next((i for i, e in enumerate(bucket) if same_taper(e, m, tol)), None)
        if hit is None:
            bucket.append(m)
        elif priority(m) > priority(bucket[hit]):
            bucket[hit] = m  # same taper, better source

    kept = []
    distinct_same_name = 0
    for k in order:
        bucket = groups[k]
        if len(bucket) > 1:
            distinct_same_name += 1
            disambiguate(bucket)
        kept.extend(bucket)
    dropped = len(models) - len(kept)
    return kept, dropped, distinct_same_name


def disambiguate(bucket):
    """Append a source tag (and a counter if still ambiguous) to same-name rods
    so GUI labels stay distinct. Original name is preserved in provenance."""
    used = {}
    for m in bucket:
        tag = SOURCE_TAG.get(src(m), "alt")
        n = used.get(tag, 0) + 1
        used[tag] = n
        suffix = tag if n == 1 else f"{tag} #{n}"
        prov = m.setdefault("provenance", {}) or {}
        prov.setdefault("orig_name", m.get("name"))
        m["provenance"] = prov
        m["name"] = f"{m.get('name')} ({suffix})"


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--keep-all", action="store_true",
                    help="keep every record; do not dedupe")
    ap.add_argument("--tol", type=float, default=DEFAULT_TOL,
                    help=f"taper match tolerance in inches (default {DEFAULT_TOL})")
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
        models, dropped, distinct = dedup(models, args.tol)
        print(f"dedup: {total_in} -> {len(models)} ({dropped} true duplicates "
              f"dropped @ tol {args.tol}\"; {distinct} names kept with >1 distinct "
              f"taper)", file=sys.stderr)

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
