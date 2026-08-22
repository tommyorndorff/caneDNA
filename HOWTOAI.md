# HOWTOAI

How to work effectively on **caneDNA** with an AI coding assistant. Canonical
project context lives in [`AGENTS.md`](AGENTS.md); this guide is the practical
"how do I actually do X" companion.

## Orientation

caneDNA has two halves that meet at one file:

1. A **Python data pipeline** that imports rod tapers from several source files
   and merges them into `data/tapers.json`.
2. A **Rust GUI** (`roddna-gui`, eframe/egui) that embeds `data/tapers.json` and
   renders a browse/filter/overlay taper explorer — the same crate builds a
   native desktop app and a WASM web app.

`roddna-core` sits between them as the typed data model both the tests and the
GUI use.

## Common tasks

### Run the tests
```sh
cargo test -p roddna-core
```
The core test parses `data/tapers.json` and asserts every record has matching
station/dimension arrays, a `provenance.source`, and that Spey rods are present.

### Run the app (native)
```sh
cargo run -p roddna-gui
```

### Run the app (web)
```sh
rustup target add wasm32-unknown-unknown   # once
cargo install --locked trunk               # once
cd crates/roddna-gui
trunk serve                                # dev server at http://localhost:8080
trunk build --release                      # static bundle -> dist/
```

### Regenerate the taper library
Needs Python 3 and `openpyxl`. Never hand-edit the generated JSON — change an
importer (or `build_library.py`) and re-run:
```sh
python3 scripts/convert_tapers.py            # RodDNA XML     -> data/sources/roddna.json
python3 scripts/import_hexrod.py --all       # drtapers.xlsx  -> data/sources/hexrod.json
python3 scripts/import_taper_sheets.py       # taper sheets   -> data/sources/taper_sheets.json
python3 scripts/build_library.py             # merge + dedupe -> data/tapers.json
cargo build -p roddna-gui                    # re-embed the new library
```
`build_library.py --tol <inches>` tunes dedup strictness; `--keep-all` disables
dedup. `import_hexrod.py --maker <name>` imports only one maker.

## Where things live

- **Add a data field:** edit the importer(s) to emit it, add it to the `Taper`
  struct in `crates/roddna-core/src/lib.rs`, rebuild, then surface it in
  `crates/roddna-gui/src/main.rs`.
- **Add a new data source:** write a `scripts/import_<source>.py` that emits a
  list of model dicts (each with a `provenance` block) to
  `data/sources/<source>.json`, drop the raw file in `data/raw/`, then re-run
  `build_library.py`. Update the Data Sources table in `README.md`.
- **Change the UI:** everything is in `crates/roddna-gui/src/main.rs`
  (`App::update`). Keep platform-specific bits behind `cfg(target_arch)`.

## Conventions that matter

- **Conventional Commits** (`feat:`, `fix:`, `chore:`…) — releases and the
  CHANGELOG are generated from commit messages by release-please.
- **Preserve attribution.** Every taper has a `provenance` block; keep it intact
  when transforming data, and credit new sources in `README.md`.
- **`data/tapers.json` is embedded at compile time.** Rebuild the GUI after
  regenerating it.

## Verify your change

- Data change → `cargo test -p roddna-core` (catches malformed records).
- GUI change → `cargo build -p roddna-gui` and, ideally, `trunk build` to confirm
  the web target still compiles.
