#!/usr/bin/env python3
"""Parse the Rodmakers (RMA) listserv archive into structured messages.

Source: https://www.hexrod.net/RMA_allmsg/rma_archive.zip — ten yearly text
files (rma1995.txt .. rma2004.txt), each a stream of messages delimited by a
line of asterisks with `DATE:` / `SUBJECT:` / `FROM:` headers followed by a body.

Provides `iter_messages(zip_path)` used by build_casting_kb.py, and a CLI that
prints stats or dumps JSONL:

    python3 scripts/parse_rma.py --stats
    python3 scripts/parse_rma.py --jsonl data/kb/rma_messages.jsonl
"""
import argparse
import json
import re
import sys
import zipfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
ZIP = ROOT / "data" / "raw" / "rma_archive.zip"

SEP = re.compile(r"^\*{40,}\s*$")
HDR = re.compile(r"^(DATE|SUBJECT|FROM):\s*(.*)$")
# "michael@wupsych (Michael Biondo)" or "Wayne Cattanach <waynecatt@aol.com>"
FROM_PAREN = re.compile(r"^\s*(\S+)\s*\((.*)\)\s*$")
FROM_ANGLE = re.compile(r"^\s*(.*?)\s*<(\S+)>\s*$")


def _split_from(value):
    """Return (email, name) best-effort from a FROM header value."""
    if not value:
        return None, None
    m = FROM_PAREN.match(value)
    if m:
        return m.group(1), m.group(2).strip()
    m = FROM_ANGLE.match(value)
    if m:
        return m.group(2), m.group(1).strip()
    if "@" in value.split()[0:1] or "@" in value:
        return value.strip(), None
    return None, value.strip()


def _flush(fields, body_lines, source, idx):
    body = "\n".join(body_lines).strip()
    if not (fields or body):
        return None
    email, name = _split_from(fields.get("FROM", ""))
    return {
        "id": f"{source}#{idx}",
        "year": int(re.search(r"(\d{4})", source).group(1)),
        "date": fields.get("DATE"),
        "subject": fields.get("SUBJECT"),
        "from_email": email,
        "from_name": name,
        "body": body,
        "source_file": source,
    }


def iter_messages(zip_path=ZIP):
    """Yield one dict per message across all yearly files in the archive."""
    with zipfile.ZipFile(zip_path) as zf:
        for name in sorted(zf.namelist()):
            if not name.endswith(".txt"):
                continue
            idx = 0
            fields, body_lines, in_header = {}, [], False
            text = zf.read(name).decode("latin-1")
            lines = text.splitlines()
            # Prime: skip to first separator.
            started = False
            for line in lines:
                if SEP.match(line):
                    if started:
                        msg = _flush(fields, body_lines, name, idx)
                        if msg:
                            yield msg
                        idx += 1
                    started = True
                    fields, body_lines, in_header = {}, [], True
                    continue
                if not started:
                    continue
                if in_header:
                    m = HDR.match(line)
                    if m:
                        fields[m.group(1)] = m.group(2).strip()
                        continue
                    # blank line or first non-header line ends the header block
                    if line.strip() == "":
                        in_header = False
                        continue
                    in_header = False
                    body_lines.append(line)
                else:
                    body_lines.append(line)
            if started:
                msg = _flush(fields, body_lines, name, idx)
                if msg:
                    yield msg


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--stats", action="store_true")
    ap.add_argument("--jsonl", help="write all messages as JSONL to this path")
    args = ap.parse_args()

    if not ZIP.exists():
        sys.exit(f"missing {ZIP} — run scripts/fetch_rma.py first")

    if args.jsonl:
        out = Path(args.jsonl)
        out.parent.mkdir(parents=True, exist_ok=True)
        n = 0
        with out.open("w", encoding="utf-8") as fh:
            for m in iter_messages():
                fh.write(json.dumps(m, ensure_ascii=False) + "\n")
                n += 1
        print(f"wrote {out} ({n} messages)", file=sys.stderr)
        return

    # default: stats
    n, per_year, with_subj = 0, {}, 0
    for m in iter_messages():
        n += 1
        per_year[m["year"]] = per_year.get(m["year"], 0) + 1
        if m["subject"]:
            with_subj += 1
    print(f"messages: {n}", file=sys.stderr)
    print(f"with subject: {with_subj}", file=sys.stderr)
    for y in sorted(per_year):
        print(f"  {y}: {per_year[y]}", file=sys.stderr)


if __name__ == "__main__":
    main()
