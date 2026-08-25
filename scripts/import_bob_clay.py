#!/usr/bin/env python3
"""Import the "Universal 12' taper" example from Bob Clay's Anchored Outdoors
bamboo rod building course.

Source: hand-transcribed from the course's example taper table (half of the
finished rod's flat-to-flat dimension, every 5" from the tip; the last ~22"
to the butt is level/untapered for the grip). Not a bulk spreadsheet import
like the other sources, so this is a small standalone script rather than a
generic converter — still produces a per-source JSON file so it goes through
the normal build_library.py merge rather than being hand-edited into
data/tapers.json.

Output: data/sources/bob_clay.json
Run build_library.py afterwards to merge into tapers.json.
"""
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
OUT = ROOT / "data" / "sources" / "bob_clay.json"

# (inches from tip, half of finished flat-to-flat dimension in thousandths)
# as given in the course material, taken every 5" from the tip.
HALF_THOUSANDTHS_BY_STATION = [
    (0, 50),
    (5, 57.5),
    (10, 60),
    (15, 72.5),
    (20, 80),
    (25, 87.5),
    (30, 95),
    (35, 102.5),
    (39, 110),
    (45, 117.5),
    (50, 125),
    (55, 132.5),
    (60, 140),
    (65, 147.5),
    (70, 155),
    (75, 162.5),
    (80, 170),
    (85, 177.5),
    (90, 185),
    (95, 192.5),
    (100, 200),
    (105, 207.5),
    (110, 215),
    (115, 222.5),
    (120, 230),
    (122, 233),
]

ROD_LENGTH = 144.0  # 12'
ACTION_LENGTH = 122.0  # taper stops at top of grip; level from here to the butt

stations = [s for s, _ in HALF_THOUSANDTHS_BY_STATION]
dimensions = [round(2 * half / 1000, 4) for _, half in HALF_THOUSANDTHS_BY_STATION]

# Level (untapered) grip section: continue the same 5" station spacing as the
# rest of the taper out to the butt, all at the last tapered dimension,
# rather than jumping straight from 122" to 144" with no intermediate rows.
level_dimension = dimensions[-1]
station = stations[-1] + 5.0
while station < ROD_LENGTH:
    stations.append(station)
    dimensions.append(level_dimension)
    station += 5.0
stations.append(ROD_LENGTH)
dimensions.append(level_dimension)

model = {
    "name": "Bob Clay 12 ft 7/8 wt Example Taper",
    "type": "Spey-Rod",
    "const_type": "Hex",
    "length": ROD_LENGTH,
    "action_length": ACTION_LENGTH,
    "line_weight": 7,
    "pieces": 3,
    "ferrule_type": "Spliced (taped, hockey tape)",
    "notes": (
        "Example “universal” 12' taper from the Bob Clay bamboo rod "
        "building course on Anchored Outdoors "
        "(https://courses.anchoredoutdoors.com/courses/bamboo-rod-building-with-bob-clay). "
        "Rated 7/8wt (spey double rating; recorded here as line_weight 7). "
        "6-sided (60°) hex; if using a Morgan Hand Mill (61.5° form) "
        "deduct 2.3% from these dimensions. For a lighter line deduct 3%, for "
        "a heavier line add 3%. Buildable in 3 or 4 pieces (recorded here as "
        "3) with a spliced ferrule taped with hockey tape rather than a "
        "machined ferrule, so no ferrule size/location is recorded. Level "
        "(untapered) for the last ~22\" to the butt (grip section)."
    ),
    "dimensions": dimensions,
    "stations": stations,
    "stresses": [],
    "guide_spacings": [],
    "guide_sizes": [],
    "provenance": {
        "source": "Bob Clay Bamboo Rod Building Course (Anchored Outdoors)",
        "author": "Bob Clay",
        "source_url": "https://courses.anchoredoutdoors.com/courses/bamboo-rod-building-with-bob-clay",
        "collection": "Example universal 12' taper",
        "license": "Course material; used here for reference/education",
        "imported": "2026-08-24",
    },
}

OUT.write_text(json.dumps([model], indent=2), encoding="utf-8")
print(f"wrote {OUT} (1 model)")
