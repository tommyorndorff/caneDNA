# caneDNA vs. RodDNA — feature comparison

RodDNA (Larry Tusoni, highsierrarods.com) is the origin of our taper data and the
reference point for features. This compares its capabilities (grounded in the
bundled RodDNA v2.0 help file and the app's data files) against caneDNA today, and
turns the gaps into a prioritized roadmap.

Legend: ✅ have · 🟡 partial · ❌ gap · ⛔ intentionally out of scope

## Feature matrix

| Capability | RodDNA | caneDNA | Notes |
|---|---|---|---|
| Taper/model library | ✅ | ✅ | caneDNA merges 3 sources (873 rods) with per-taper **provenance**; RodDNA is one library |
| Browse / search / filter | ✅ | ✅ | caneDNA filters by type/line-wt/pieces |
| Dimension (taper) chart | ✅ | ✅ | |
| **Overlay-compare** many tapers | 🟡 | ✅ | caneDNA overlays arbitrary selections on one plot |
| **Stress curves** (Garrison) | ✅ | ✅ | `Taper::stress_curve()` reconstructs RodDNA's own casting-load model (recovered by decompiling `RodDNA_v20.jar`); median ~6% relative error against the 58 records that ship stored stresses. A GUI "Stress" tab overlays it like the Chart tab |
| Taper editing / transforms | ✅ | ❌ | RodDNA: linear taper, station&dimensions, dimensions-only, station multiplier/bias |
| **Planing-form settings** (60° form depths) | ✅ | ✅ | `Taper::planing_form_depths()` reconstructs RodDNA's own per-geometry depth formula (Hex/Quad/Penta, recovered by decompiling `RodDNA_v20.jar`'s `PrintPlaningFormSettings`). A GUI "Planing Form" tab shows station/dimension/depth for the selected rod |
| **Guide spacing** calc + report | 🟡 | ✅ | `Taper::guide_spacing()` is an original static-deflection calculator (RodDNA's own "guide spacing" turned out to be a lookup table, not a formula — see roadmap #4). A GUI "Guide Spacing" tab shows the computed placements next to any stored `guide_spacings` for comparison |
| Ferrule sizing / placement | ✅ | 🟡 | Fields captured & shown; no sizing logic |
| Multi-geometry (Hex/Penta/Quad/Rect) | ✅ | 🟡 | Data carries `const_type`; plotting is flat-to-flat (geometry-agnostic). Stress/planing will need geometry |
| Reports / print / PDF export | ✅ | ❌ | RodDNA uses iText/JFreeReport; caneDNA has no taper export yet |
| Taper export (CSV / shop format) | 🟡 | ✅ | `Taper::to_csv()`/`to_station_file()`, both carrying a provenance comment header. Native saves via a file dialog (`rfd`); web triggers a browser download (Blob + anchor click) — no backend either way |
| **Casting knowledge base** | ❌ | ✅ | caneDNA links RMA-listserv casting feedback to makers/models w/ action tags — novel |
| Cross-platform native + **web (WASM)** | ❌ (Java desktop) | ✅ | Single binary + static Cloudflare-hosted web app |
| Reproducible data pipeline | ❌ | ✅ | Scripted importers + merge/dedup |
| Versioned releases + artifacts | ❌ | ✅ | Conventional Commits → release-please, native + WASM builds |
| Customers / vendors / rods DBs | ✅ | ⛔ | Business-management side; not a goal |
| Network / chat / registration | ✅ | ⛔ | Obsolete; not a goal |
| Morgan Hand Mill (MHM) settings | ✅ | ✅ | caneDNA computes MHM dial settings natively (adjustable rough/finish oversize allowances) rather than replicating RodDNA's printed report format. Strip depth is per-geometry (Hex/Quad/Penta), matching RodDNA's `PrintMHMSettings` — see roadmap #7 |
| **Dimension-change (delta) chart** | ❌ | ✅ | hexrod.net-style bar+line chart of station-to-station dimension change every 5", with ferrule-location markers |

## Gap-driven roadmap (priority order)

1. ✅ **Garrison stress-curve engine** (`roddna-core`) — `Taper::stress_curve()`
   reconstructs RodDNA's own casting-load model (cantilever bending moment from
   tip/line/varnish-guides/ferrule/bamboo-self-weight loads, over a per-geometry
   section modulus), recovered by decompiling `RodDNA_v20.jar` since neither the
   formula nor `lwv`/`rav`'s meaning are documented anywhere the source data
   ships. Validated against the 58 records that ship stored `stresses`: median
   ~6% relative error, p90 ~29%. A GUI "Stress" tab overlays it like Chart.
   *(RodDNA parity + foundation for spey design work.)*

   Not done: extending to a full **casting deflection analysis** matching
   hexrod.net's report (per-station angle, horizontal/vertical deflection,
   curvature, driven by adjustable modulus-of-elasticity and impact-factor
   inputs, visualized as a deflected rod shape) — a natural follow-up once
   design mode (#2) needs it.
2. ✅ **Taper design/edit mode** — an in-memory `DesignState` session (GUI): clone
   a seed taper (or start from the decided spey target), edit stations, apply
   scale (`Taper::scaled`), and insert ferrule stations (`Taper::insert_station`)
   with live-recomputed Profile/Stress/Dimension Changes/Mill Settings tabs.
   Nothing persists past the session — export (#5) is how a design leaves the
   app. Enables designing new tapers (see `SPEY_DESIGN.md`).
3. ✅ **Planing-form settings** — `Taper::planing_form_depths()` reconstructs
   RodDNA's per-geometry V-groove depth formula, recovered by decompiling
   `PrintPlaningFormSettings` from `RodDNA_v20.jar`: Hex is the inradius
   (`dimension / 2`), Quad the circumradius (`dimension / 2 * sqrt(2)`), Penta
   RodDNA's own constant (`dimension / 1.809753`) — RodDNA itself only
   supports these three geometries, and so does caneDNA's implementation.
   Each depth is offset by the taper's own `station_bias * station_multiplier`
   (RodDNA reuses those fields for this, unrelated to their absence from the
   stress calc). No stored ground truth exists to validate against, unlike
   stress curves — tests check the formula transcription, not real builder
   data. A GUI "Planing Form" tab shows station/dimension/depth.
4. ✅ **Guide-spacing calculator** — investigating RodDNA's own "guide spacing"
   feature (decompiling `ModelsDialog`/`GuidesXML`) found it's a bundled lookup
   table keyed by piece count and floor-matched rod length, not a formula —
   and the library's stored `guide_spacings` on 49 records can diverge from
   even that table (apparent hand-edits), so there's nothing reliable to port
   or validate against. `Taper::guide_spacing()` is instead an original
   static-deflection calculator: marching from the tip, each span is the
   longest run that keeps the rod's own self-weight sag under an adjustable
   threshold (standard simply-supported-beam formula, treating the local
   cross-section as an equivalent solid circle). A GUI "Guide Spacing" tab
   shows the computed placements next to any stored `guide_spacings`.
5. ✅ **Export** — `Taper::to_csv()` and `Taper::to_station_file()`, both with a
   `#`-prefixed provenance/metadata header so attribution travels with the
   file. The station file is an honestly-labeled plain station/dimension
   list, not a verified reproduction of any specific rodmaking software's
   native format (no such spec was available to check against — see
   `to_station_file`'s doc comment). GUI export buttons appear next to a
   selected library rod and in the design panel; native saves via `rfd`'s
   file dialog, web triggers a Blob-based browser download. A printable/PDF
   sheet remains a later item.
6. ✅ **Dimension-change (delta) chart** — a view alongside Chart/Station
   Data/Mill Settings showing the station-to-station change in flat-to-flat
   dimension (bar chart, one bar per 5" span, with a connecting line and
   value labels), plus vertical markers for each `Taper::ferrules()` location
   — hexrod.net's "Dimension Changes Every 5 Inches" report is the reference.
7. ✅ **Mine the decompiled RodDNA app for Mill-Settings ideas** — read
   `com.tusoni.RodDNA.printing.PrintMHMSettings` (decompiled from
   `RodDNA_v20.jar`). Finding: RodDNA's MHM "Strip Dim" column is the **same
   per-geometry strip-depth conversion** its planing-form report uses (Hex
   `dim/2`, Quad `dim·√2/2`, Penta `dim/1.809753`) — but caneDNA's
   `settings_for_points` was using `dim/2` for **all** geometries, understating
   the strip depth for the 60 Quad/Penta rods (~7% of the library). Fixed:
   mill settings now reuse `PlaningFormGeometry::depth` (fall back to `dim/2`
   for hepta/octa/rect/unspecified). Deliberately **not** ported: RodDNA has no
   rough/finish-allowance concept (it uses bias·multiplier instead — caneDNA
   keeps the richer Morgan-taper-sheet allowance model), and its JFreeReport
   PDF layout (station-# countdown, extrapolated run-out rows) is presentation,
   not a computation gap. Research-only otherwise: the decompiled sources live
   outside the repo.

These are reflected in the README roadmap.
