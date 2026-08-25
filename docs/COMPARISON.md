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
| **Guide spacing** calc + report | ✅ | ❌ | Data carries guide fields; no calculator yet |
| Ferrule sizing / placement | ✅ | 🟡 | Fields captured & shown; no sizing logic |
| Multi-geometry (Hex/Penta/Quad/Rect) | ✅ | 🟡 | Data carries `const_type`; plotting is flat-to-flat (geometry-agnostic). Stress/planing will need geometry |
| Reports / print / PDF export | ✅ | ❌ | RodDNA uses iText/JFreeReport; caneDNA has no taper export yet |
| Taper export (CSV / shop format) | 🟡 | ❌ | Roadmap item |
| **Casting knowledge base** | ❌ | ✅ | caneDNA links RMA-listserv casting feedback to makers/models w/ action tags — novel |
| Cross-platform native + **web (WASM)** | ❌ (Java desktop) | ✅ | Single binary + static Cloudflare-hosted web app |
| Reproducible data pipeline | ❌ | ✅ | Scripted importers + merge/dedup |
| Versioned releases + artifacts | ❌ | ✅ | Conventional Commits → release-please, native + WASM builds |
| Customers / vendors / rods DBs | ✅ | ⛔ | Business-management side; not a goal |
| Network / chat / registration | ✅ | ⛔ | Obsolete; not a goal |
| Morgan Hand Mill (MHM) settings | ✅ | ✅ | caneDNA computes MHM dial settings natively (adjustable rough/finish oversize allowances) rather than replicating RodDNA's printed report format |
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
2. **Taper design/edit mode** — edit stations, apply scale (multiplier/bias), and
   add/split ferrule stations. Enables designing new tapers (see `SPEY_DESIGN.md`).
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
4. **Guide-spacing calculator** — from length/line/# guides, with a static-deflection option.
5. **Export** — CSV + a Hexrod-style station file; later a printable/PDF sheet.
6. ✅ **Dimension-change (delta) chart** — a view alongside Chart/Station
   Data/Mill Settings showing the station-to-station change in flat-to-flat
   dimension (bar chart, one bar per 5" span, with a connecting line and
   value labels), plus vertical markers for each `Taper::ferrules()` location
   — hexrod.net's "Dimension Changes Every 5 Inches" report is the reference.
7. **Mine the decompiled RodDNA app for Mill-Settings ideas** — while
   decompiling `RodDNA_v20.jar` (buried in `RodDNAInstaller.jar` →
   `data/RodDNA_v20.zip` → `RodDNA.jar`, via `cfr-decompiler`) for #1's stress
   formula, we saw `com/tusoni/RodDNA/models/` but didn't dig for Mill-Settings
   logic beyond what `Taper::mill_settings`/`mill_sections` already replicate —
   e.g. other allowance presets, printed-report layout worth matching, or
   per-geometry (Quad/Penta) anvil conventions we haven't modeled.
   Research-only: nothing here is committed (the decompiled sources live
   outside the repo), this just tells us whether `roddna-gui`'s Mill Settings
   tab is missing anything RodDNA's did.

These are reflected in the README roadmap.
