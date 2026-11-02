#!/usr/bin/env python3
"""Import hand-transcribed spinning-rod tapers from the Rodmakers (RMA) list.

Source: two posts to the public Rodmakers mailing list, as mirrored on
hexrod.net ("More rodmakers tapers"):

  * #399  Robert Holder, "The Ibantik Spinner", 6'0" ultralight (Oct 2002)
          https://www.hexrod.net/Tapers/More_rodmakers_tapers/399.html
  * #156  "Gary", Paul H. Young "Spinmaster" 7'2" (Mar 2001), measured from a
          vintage rod. The other Young rods in the same post (Spin-Caster 6'3",
          Special 6'4") are already in the library via RodDNA, and the
          Steelhead Baitcaster 8'9" is out of scope here.
          https://www.hexrod.net/Tapers/More_rodmakers_tapers/156.html

Use the hexrod.net pages, not data/raw/rma_archive.zip, for the numbers: the
zip truncates the last character of every line, so its dimensions lose a digit.

Small standalone script like import_bob_clay.py (hand-transcribed, not a bulk
converter) that still writes a per-source JSON so it goes through the normal
build_library.py merge instead of hand-editing data/tapers.json.

Only values stated in the posts are recorded. Neither post says hex vs. quad,
so `const_type` is left unset. Neither states a line weight or lure rating, so
the casting-load fields are unset too: the stress, deflection and action
engines return nothing for these rods rather than guessing. Note modal_analysis
still runs and assumes a hex section when const_type is unset.

Output: data/sources/rma_spinning.json
Run build_library.py afterwards to merge into tapers.json.
"""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "data" / "sources" / "rma_spinning.json"

IMPORT_DATE = "2026-11-01"
# Deliberately not mentioning "hexrod" in the source name: Taper::source_group()
# matches that substring and would file these under David Ray's Hexrod library.
# The hexrod.net mirror URLs live in each record's provenance.source_url.
SOURCE = "Rodmakers list (RMA) archive"
LICENSE = (
    "Posted to the public Rodmakers mailing list; no license stated. "
    "Attribution to the original poster preserved; remove on request."
)

# (inches from tip, flat-to-flat dimension in inches), exactly as posted.
HOLDER = [
    (0, 0.090), (5, 0.104), (10, 0.116), (15, 0.132), (20, 0.137),
    (25, 0.151), (30, 0.168), (35, 0.170), (40, 0.195), (45, 0.205),
    (50, 0.221), (55, 0.228), (60, 0.231), (65, 0.233), (70, 0.235),
]

# The post lists a "ferrule" line between 40" and 46"; stations are as posted.
SPINMASTER = [
    (0, 0.082), (5, 0.091), (10, 0.106), (15, 0.126), (20, 0.143),
    (25, 0.157), (30, 0.176), (35, 0.192), (40, 0.207),
    (46, 0.208), (50, 0.211), (55, 0.216), (60, 0.226), (65, 0.246),
    (70, 0.255), (74, 0.259),
]


def split(points):
    return [float(s) for s, _ in points], [d for _, d in points]


h_st, h_dim = split(HOLDER)
s_st, s_dim = split(SPINMASTER)

holder = {
    "name": "Holder Ibantik Spinner 6' UL",
    "type": "Spinning-Rod",
    "length": 72.0,
    "action_length": 70.0,
    "pieces": 2,
    "ferrule1_size": "11/64",
    "ferrule1_loc": 36.0,
    "station_increment": 5,
    "notes": (
        "Ultralight spinning rod built by Robert Holder for his father, "
        "posted in reply to a request for a 5.5-6' rod for 4 lb line and "
        "1/8 oz lures. The tip section came from a broken fly-rod tip; the "
        "butt was built to match. Taper stops at 70\"; the post gives no "
        "handle length. Geometry (hex vs. quad) and measuring convention "
        "(over varnish or not) are not stated. Note the +0.025\" step from "
        "35\" to 40\" is as posted. Guides from the tip: 4, 9, 16.5, 27, 40 in. "
        "(sizes 10, 12, 16, 20, 25 as listed, units not stated)."
    ),
    "dimensions": h_dim,
    "stations": h_st,
    "stresses": [],
    "guide_spacings": [],
    "guide_sizes": [],
    "provenance": {
        "source": SOURCE,
        "author": "Robert Holder",
        "source_url": "https://www.hexrod.net/Tapers/More_rodmakers_tapers/399.html",
        "collection": "More rodmakers tapers #399 (Re: Which Spinning Taper?, 2002-10-01)",
        "license": LICENSE,
        "imported": IMPORT_DATE,
        "guides_from_tip_in": [4, 9, 16.5, 27, 40],
        "guide_sizes_as_posted": [10, 12, 16, 20, 25],
    },
}

spinmaster = {
    "name": "P.H. Young Spinmaster 7'2\"",
    "type": "Spinning-Rod",
    "length": 86.0,
    "action_length": 74.0,
    "pieces": 2,
    "ferrule1_size": "13/64",
    "station_increment": 5,
    "notes": (
        "Paul H. Young Spinmaster, 7'2\", 3.35 oz, lightly flamed, 12 1/8\" "
        "sliding-ring handle, stepped 13/64 ferrule, semi-parabolic action; "
        "loads through the handle and \"throws a nice 5wt\" per the poster. "
        "Measured and miked from a vintage rod by the poster (\"Gary\"). The "
        "ferrule sits between the 40\" and 46\" stations (exact location not "
        "given, so none is recorded and mill sections are approximate). "
        "Stations end at 74\", where the handle begins. Geometry (hex vs. "
        "quad) is not stated. Guide spacing from the tip was posted as 5.x, "
        "12 1/8, 19.x, 28 5/8, 37.x with some fractions garbled in the "
        "archive, so it is not recorded."
    ),
    "dimensions": s_dim,
    "stations": s_st,
    "stresses": [],
    "guide_spacings": [],
    "guide_sizes": [],
    "provenance": {
        "source": SOURCE,
        "author": "\"Gary\" (surname not given), measurements of a P.H. Young rod",
        "source_url": "https://www.hexrod.net/Tapers/More_rodmakers_tapers/156.html",
        "collection": "More rodmakers tapers #156 (Paul Young Spinning Rod Tapers, 2001-03-01)",
        "license": LICENSE,
        "imported": IMPORT_DATE,
    },
}

OUT.write_text(json.dumps([holder, spinmaster], indent=2), encoding="utf-8")
print(f"wrote {OUT} (2 models)")
