#!/usr/bin/env python3
"""Convert RodDNA XML taper libraries to a single typed JSON file.

Source: RodDNA v2.0 (Larry Tusoni, highsierrarods.com), Models module XML.
Output: data/tapers.json — a lossless-ish, typed representation of every rod
model, with the taper `dimensions` (flat-to-flat cross-section in inches at
each station) parsed into a float array plus derived station positions.
"""
import json
import re
import sys
from pathlib import Path
from xml.etree import ElementTree as ET

ROOT = Path(__file__).resolve().parent.parent
RAW = ROOT / "data" / "raw"
OUT = ROOT / "data" / "tapers.json"

SOURCES = [
    ("RodDNAModels.xml", "models-v2.0"),
    ("RodDNAModelsv1.4Update.xml", "v1.4-update"),
]

# Fields parsed as floats; everything else stays a string (or None if empty).
FLOAT_FIELDS = {
    "length", "action_length", "line_weight", "line_length", "line_cast",
    "pieces", "ferrule1_loc", "ferrule2_loc", "ferrule3_loc", "tiptop_size",
    "lwv", "rav", "tip_impact_factor", "bamboo_density", "tip_weight",
    "station_multiplier", "station_bias", "station_increment", "db_number",
}
# Comma-separated numeric lists.
LIST_FIELDS = {"dimensions", "stresses", "guide_spacings", "guide_sizes"}


def num(text):
    if text is None or text.strip() == "":
        return None
    t = text.strip()
    try:
        f = float(t)
        return int(f) if f.is_integer() else f
    except ValueError:
        return t


def num_list(text):
    if text is None or text.strip() == "":
        return []
    out = []
    for part in text.split(","):
        part = part.strip()
        if part:
            try:
                out.append(float(part))
            except ValueError:
                pass
    return out


def parse_model(el):
    m = {}
    for child in el:
        tag, text = child.tag, child.text
        if tag in LIST_FIELDS:
            m[tag] = num_list(text)
        elif tag in FLOAT_FIELDS:
            m[tag] = num(text)
        else:
            m[tag] = (text or "").strip() or None
    # Derived: station positions (inches from tip) for each dimension point.
    inc = m.get("station_increment") or 5
    dims = m.get("dimensions") or []
    m["stations"] = [round(i * inc, 3) for i in range(len(dims))]
    return m


def main():
    models = []
    for fname, source in SOURCES:
        path = RAW / fname
        if not path.exists():
            print(f"skip (missing): {fname}", file=sys.stderr)
            continue
        # Strip comments defensively; ElementTree handles them, but be safe.
        text = path.read_text(encoding="utf-8")
        root = ET.fromstring(text)
        count = 0
        for el in root.findall("model"):
            m = parse_model(el)
            m["_source"] = source
            models.append(m)
            count += 1
        print(f"{fname}: {count} models", file=sys.stderr)

    payload = {
        "meta": {
            "source": "RodDNA v2.0 (highsierrarods.com), Models module",
            "author": "Larry Tusoni",
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
    print(f"wrote {OUT} ({len(models)} models)", file=sys.stderr)


if __name__ == "__main__":
    main()
