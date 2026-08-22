#!/usr/bin/env python3
"""Download the Rodmakers (RMA) listserv archive into data/raw/.

The archive (~20 MB) is a single ZIP of yearly text files, 1995-2004, from
https://www.hexrod.net/RMA_allmsg/ . It is gitignored (large, external); this
script makes fetching it reproducible.
"""
import sys
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
URL = "https://www.hexrod.net/RMA_allmsg/rma_archive.zip"
OUT = ROOT / "data" / "raw" / "rma_archive.zip"


def main():
    OUT.parent.mkdir(parents=True, exist_ok=True)
    print(f"downloading {URL} ...", file=sys.stderr)
    urllib.request.urlretrieve(URL, OUT)
    print(f"wrote {OUT} ({OUT.stat().st_size} bytes)", file=sys.stderr)


if __name__ == "__main__":
    main()
