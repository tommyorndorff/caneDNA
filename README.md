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

**873 rod models** merged from three sources (see Data Sources below), covering
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

> Note: the Hexrod library has no stated license. It is included here with
> attribution while permission/licensing is confirmed with the source.

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
- **Casting knowledge base:** parse the RodMakers (RMA) forum archive
  ([hexrod.net/RMA_allmsg](https://www.hexrod.net/RMA_allmsg/index.html)) into a
  searchable KB of how specific tapers/rods cast — link organic maker feedback to
  library tapers by name, to inform future taper exploration.
- Reconcile tip/butt sections into ferrule-accurate full tapers.
- Stress-curve computation (Garrison-style) rather than only stored `stresses`.
- Design mode: edit a taper, add/split ferrule stations (spey split joints).
- Export a taper (CSV / Hexrod-style station file) for the shop.
