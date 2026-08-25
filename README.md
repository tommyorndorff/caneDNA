# caneDNA

An open-source taper explorer for split-bamboo fly rods — a modern reimagining of
Larry Tusoni's [RodDNA](http://www.highsierrarods.com/roddna.html). The long-term
goal is to explore and design rod patterns: 4-piece 5wt travel rods, double-handed
spey rods with split joints, single-handed spey trout rods, and more.

## Status: v0 — browse, filter & compare

A GUI (Rust + [egui](https://github.com/emilk/egui)) that loads the full taper
library, lets you search/filter rods (by type, line weight, pieces), and overlay
any number of tapers on one plot to compare their profiles. Runs both as a **native
desktop app** and as a **static web app** (WASM) deployable to Cloudflare Pages.

## Layout

```
data/
  raw/                 original source files (RodDNA XML, drtapers.xlsx, taper sheets)
  sources/             per-source typed JSON, one file per importer (generated)
  tapers.json          merged, deduped library the app loads (generated)
scripts/
  convert_tapers.py      RodDNA XML       -> data/sources/roddna.json
  import_hexrod.py       drtapers.xlsx    -> data/sources/hexrod.json
  import_taper_sheets.py taper sheets xlsx-> data/sources/taper_sheets.json
  build_library.py       merge + dedup    -> data/tapers.json
crates/
  roddna-core/         data model + JSON loading (no GUI deps; unit-tested)
  roddna-gui/          eframe/egui app — native desktop AND web (WASM)
    index.html         web entry (Trunk)
    Trunk.toml         web build config
    _headers           Cloudflare Pages caching / MIME
.github/workflows/
  deploy-web.yml       build WASM + deploy to Cloudflare Pages on push to main
```

## Data

**874 rod models** merged from four sources (see Data Sources below), covering
Fly / Dry-Fly / Spey / Spinning / Casting rods in Hex / Penta / Quad / Rectangular
construction. Each record carries length, line weight, ferrule specs, notes, the
`dimensions` taper (flat-to-flat cross-section, in inches, at each station), and a
per-record `provenance` block for attribution.

The build merges the per-source files and dedupes **dimension-aware**: two records
collapse only when they share a name *and* their tapers match within a tolerance
(default 0.0015", tune with `--tol`); the higher-priority source (Hexrod) wins.
Same-named rods whose tapers genuinely differ are both kept and disambiguated with
a source tag, e.g. `Chubb 9' 3/2 (Hexrod)` vs `Chubb 9' 3/2 (RodDNA)` — the original
name is preserved in `provenance.orig_name`. Pass `--keep-all` to keep everything.

## Data Sources & attribution

Every taper carries its own `provenance` (source, author, URL, collection, license,
import date). Please preserve attribution if you reuse this data.

| Source | Author | License | Notes |
|--------|--------|---------|-------|
| [RodDNA v2.0](http://www.highsierrarods.com/roddna.html) | Larry Tusoni | Free (released without registration) | Extracted from the RodDNA installer JAR |
| [David Ray's Taper Library (Hexrod)](https://www.hexrod.net/Tapers/drtapers/index.html) | compiled by David Ray | **Unspecified** — hobbyist compilation, attributed pending clarification | `drtapers.xlsx` |
| 2019 Bamboo Taper Sheets | Tom W. Morgan (© 2005) | See workbook copyright notice | Tip/butt section sheets; raw sections kept under `provenance.sections` |
| [Bamboo Rod Building with Bob Clay](https://courses.anchoredoutdoors.com/courses/bamboo-rod-building-with-bob-clay) | Bob Clay | Course material; used for reference/education | Single example "universal" 12' taper from the course |

> Note: the Hexrod library has no stated license. It is included here with
> attribution while permission/licensing is confirmed with the source.

## Casting knowledge base (RMA archive)

To ground future taper exploration in decades of organic maker experience,
caneDNA mines the **Rodmakers (RMA) listserv archive** (1995–2004, ~80k messages
from [hexrod.net](https://www.hexrod.net/RMA_allmsg/index.html)) for how specific
rods actually *cast*. `build_casting_kb.py` captures sentences that mention a
library maker alongside casting-descriptive language (action, loads, delicate,
tip-heavy, presentation, …), keeping each quote's citation (year, author,
subject), and writes `data/kb/casting_kb.json` aggregated by maker.

```sh
python3 scripts/fetch_rma.py            # download the archive (~20 MB, gitignored)
python3 scripts/parse_rma.py --stats    # sanity-check parsing (80,329 messages)
python3 scripts/build_casting_kb.py     # -> data/kb/casting_kb.json (45 makers)
```

The large archive is gitignored (fetch it locally); the derived, attributed KB is
committed. This is v1 (maker-level, sentence co-occurrence) — surfacing it next to
tapers in the GUI and linking at the model level are the next steps.

## Build & run (native)

```sh
cargo test -p roddna-core     # parse + validate the merged library
cargo run  -p roddna-gui      # launch the desktop taper explorer
```

## Web build & deploy (Cloudflare Pages)

The **same GUI crate** compiles to WebAssembly and runs in the browser — the data
is embedded in the binary, so the site is fully static (no backend).

```sh
rustup target add wasm32-unknown-unknown
cargo install --locked trunk
cd crates/roddna-gui
trunk serve                   # dev: http://localhost:8080
trunk build --release         # prod: writes static files to dist/
```

Deploy `crates/roddna-gui/dist/` to Cloudflare Pages, either way:

- **Automatic (CI):** `.github/workflows/deploy-web.yml` builds and deploys on
  every push to `main`. Add repo secrets `CLOUDFLARE_API_TOKEN` (scope: *Cloudflare
  Pages: Edit*) and `CLOUDFLARE_ACCOUNT_ID`; the Pages project is named `canedna`.
- **Manual:** `npx wrangler pages deploy crates/roddna-gui/dist --project-name=canedna`

`_headers` sets the correct `application/wasm` content-type and long-lived caching.
If you host under a subpath rather than a domain root, build with
`trunk build --release --public-url /your-subpath/`.

Regenerate the library from the raw sources (needs Python + `openpyxl`):

```sh
python3 scripts/convert_tapers.py
python3 scripts/import_hexrod.py --all        # or --maker Cattanach
python3 scripts/import_taper_sheets.py
python3 scripts/build_library.py              # merge + dedupe -> data/tapers.json
```

## Roadmap (next)

- **Release automation:** adopt [Conventional Commits](https://www.conventionalcommits.org)
  and cut tagged releases (SemVer) from commit history (e.g. release-please).
- **Compiled release artifacts:** attach built binaries to each tagged release —
  native builds (macOS/Linux/Windows) and the WASM web bundle.
- **Casting knowledge base:** _v2 done_ — selecting a rod shows cited casting
  feedback (RMA archive) beneath the taper plot, matched at the **model level**
  when possible (e.g. "Payne 98") and falling back to the maker, with **action
  tags** (fast/slow/parabolic/delicate/…) summarised and per-snippet. Next: filter
  the rod list by action, and tighten model-match precision.
### RodDNA parity gaps → priorities

See [`docs/COMPARISON.md`](docs/COMPARISON.md) for the full caneDNA-vs-RodDNA
feature matrix. Gap-driven order:

1. _Done_ — **Garrison stress-curve engine** (`roddna-core`): `Taper::stress_curve()`
   reconstructs RodDNA's own casting-load model across Hex/Quad/Penta (recovered by
   decompiling `RodDNA_v20.jar`), validated to ~6% median error against the 58
   records that ship stored stresses. Foundation for taper design. Not yet done:
   extending to a full **casting deflection analysis** (hexrod.net-style):
   deflected rod shape + stress-vs-station graphs from adjustable MOE/impact-factor inputs.
2. _Done_ — **Taper design/edit mode** — an in-memory design session (GUI):
   clone a seed taper, edit stations, apply scale (`Taper::scaled`), insert
   ferrule stations (`Taper::insert_station`), with live-recomputed
   Profile/Stress/Dimension Changes/Mill Settings.
3. _Done_ — **Planing-form settings** — `Taper::planing_form_depths()`
   reconstructs RodDNA's per-geometry V-groove depth formula (Hex/Quad/Penta,
   recovered by decompiling `RodDNA_v20.jar`).
4. _Done_ — **Guide-spacing calculator** — RodDNA's own feature turned out to
   be a lookup table, not a formula, so `Taper::guide_spacing()` is an
   original static-deflection calculator instead: spacing grows toward the
   butt as the rod's self-weight sag under an adjustable threshold allows.
5. _Done_ — **Export** — `Taper::to_csv()`/`to_station_file()`, both carrying a
   provenance header. Native saves via a file dialog; web downloads via a
   Blob. A printable/PDF sheet remains a later item.
6. _Done_ — **Dimension-change (delta) chart** — hexrod.net-style bar+line view of
   station-to-station dimension change with ferrule markers.
7. **Mine decompiled RodDNA for Mill-Settings ideas** — check the RodDNA app
   internals (decompiled for the stress-formula work in #1) for Morgan Hand
   Mill logic beyond what `Taper::mill_settings`/`mill_sections` already
   replicate.

### Design focus: trout-spey tapers

Designing our own **single-hand spey** and **double-hand trout-spey** tapers — see
[`docs/SPEY_DESIGN.md`](docs/SPEY_DESIGN.md) for the seed → stress → tune method,
the reference tapers we're starting from, and the concrete build steps. Depends on
gaps #1–#2 above.

- Reconcile tip/butt sections into ferrule-accurate full tapers.
