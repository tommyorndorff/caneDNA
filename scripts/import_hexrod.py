#!/usr/bin/env python3
"""Import tapers from David Ray's Taper Library (Hexrod) spreadsheet.

Source: https://www.hexrod.net/Tapers/drtapers/index.html  (drtapers.xlsx)
Output: data/sources/hexrod.json — a typed list of rod models matching the
caneDNA schema, each with a `provenance` block.

By default this imports only the Cattanach models (--maker Cattanach). Pass
--all to import every taper in the sheet, or --maker NAME to filter by another
maker (case-insensitive substring match on RodName).

Requires openpyxl (pip install openpyxl).

This is one importer in a multi-source pipeline; run `build_library.py` after to
merge all data/sources/*.json into data/tapers.json.
"""
import argparse
import json
import sys
from pathlib import Path

import openpyxl

ROOT = Path(__file__).resolve().parent.parent
XLSX = ROOT / "data" / "raw" / "drtapers.xlsx"
OUT = ROOT / "data" / "sources" / "hexrod.json"

IMPORT_DATE = "2026-08-22"

PROVENANCE = {
    "source": "David Ray's Taper Library (Hexrod)",
    "author": "compiled by David Ray",
    "source_url": "https://www.hexrod.net/Tapers/drtapers/index.html",
    "collection": "drtapers.xlsx",
    "license": "unspecified — hobbyist compilation, see source page",
}

# Columns before the station grid, in sheet order.
META_COLS = ["RodName", "Inches", "Feet", "LineWeight", "Pieces", "Geometry", "Notes"]


def as_float(v):
    try:
        return float(v)
    except (TypeError, ValueError):
        return None


def main():
    ap = argparse.ArgumentParser()
    g = ap.add_mutually_exclusive_group()
    g.add_argument("--maker", default="Cattanach", help="filter by maker substring")
    g.add_argument("--all", action="store_true", help="import every taper")
    args = ap.parse_args()

    if not XLSX.exists():
        sys.exit(f"missing {XLSX} — download drtapers.xlsx into data/raw/ first")

    wb = openpyxl.load_workbook(XLSX, read_only=True, data_only=True)
    ws = wb["Sheet1"]
    rows = list(ws.iter_rows(values_only=True))
    header = rows[0]
    # Station positions are the numeric headers after the metadata columns.
    station_hdr = [as_float(h) for h in header[len(META_COLS):]]

    models = []
    for r in rows[1:]:
        name = (r[0] or "").strip()
        if not name:
            continue
        if not args.all and args.maker.lower() not in name.lower():
            continue

        # Contiguous leading run of dimension values (stop at first blank).
        dims, stations = [], []
        for pos, cell in zip(station_hdr, r[len(META_COLS):]):
            f = as_float(cell)
            if f is None:
                break
            dims.append(f)
            stations.append(pos)

        notes = (r[6] or "")
        notes = notes.strip() if isinstance(notes, str) else str(notes)

        provenance = dict(PROVENANCE)
        provenance["imported"] = IMPORT_DATE

        models.append({
            "name": name,
            "type": "Fly-Rod",  # sheet has no type column; Cattanach are fly rods
            "const_type": (r[5] or "").strip() or None,
            "length": as_float(r[1]),
            "line_weight": as_float(r[3]),
            "pieces": as_float(r[4]),
            "station_increment": 5,
            "notes": notes or None,
            "dimensions": dims,
            "stations": stations,
            "stresses": [],
            "guide_spacings": [],
            "guide_sizes": [],
            "provenance": provenance,
        })

    label = "all makers" if args.all else args.maker
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_text(json.dumps(models, indent=2), encoding="utf-8")
    print(f"wrote {OUT} ({len(models)} models, filter: {label})", file=sys.stderr)


if __name__ == "__main__":
    main()
