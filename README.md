# caneDNA

An open-source taper explorer for split-bamboo fly rods — a modern reimagining of
Larry Tusoni's [RodDNA](http://www.highsierrarods.com/roddna.html). The long-term
goal is to explore and design rod patterns: 4-piece 5wt travel rods, double-handed
spey rods with split joints, single-handed spey trout rods, and more.

## Status: v0 — browse + plot

A native GUI (Rust + [egui](https://github.com/emilk/egui)) that loads the full
taper library and lets you search/filter rods and plot the taper profile of any one.

## Layout

```
data/
  raw/                 original RodDNA XML libraries (extracted from RodDNA_v20.jar)
  tapers.json          619 tapers, typed JSON (generated)
scripts/
  convert_tapers.py    XML -> tapers.json converter (repeatable)
crates/
  roddna-core/         data model + JSON loading (no GUI deps; unit-tested)
  roddna-gui/          eframe/egui desktop app (browse + taper plot)
```

## Data

619 rod models (527 from the v2.0 library + 92 from the v1.4 update), covering
Fly / Dry-Fly / Spey / Spinning / Casting rods in Hex / Penta / Quad / Rectangular
construction. Each record carries length, line weight, ferrule specs, notes, and the
`dimensions` taper (flat-to-flat cross-section, in inches, at each station).

Provenance: RodDNA v2.0 by Larry Tusoni (highsierrarods.com), released free. See
`data/raw/` for the original XML and `data/tapers.json` `meta` for attribution.

## Build & run

```sh
cargo test -p roddna-core     # parse + validate the library
cargo run  -p roddna-gui      # launch the taper explorer
```

Regenerate the JSON from the raw XML:

```sh
python3 scripts/convert_tapers.py
```

## Roadmap (next)

- Overlay/compare multiple tapers on one chart.
- Stress-curve computation (Garrison-style) rather than only stored `stresses`.
- Design mode: edit a taper, add/split ferrule stations (spey split joints).
- Export a taper (CSV / Hexrod-style station file) for the shop.
- Optional WASM build for a web version.
