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
| **Stress curves** (Garrison) | ✅ | 🟡 | caneDNA stores stresses when present but does **not compute** them yet — inputs are available (`lwv`,`rav`,`tip_impact_factor`,`bamboo_density`,`tip_weight`) |
| Taper editing / transforms | ✅ | ❌ | RodDNA: linear taper, station&dimensions, dimensions-only, station multiplier/bias |
| **Planing-form settings** (60° form depths) | ✅ | ❌ | Depth-at-station for setting the forms; RodDNA prints a report |
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
| **Dimension-change (delta) chart** | ❌ | ❌ | hexrod.net-style bar+line chart of station-to-station dimension change every 5", with ferrule-location markers — see roadmap |

## Gap-driven roadmap (priority order)

1. **Garrison stress-curve engine** (`roddna-core`) — compute the stress curve from
   the taper + already-present inputs, for Hex first then Quad/Penta. Unlocks
   stress overlays and is the foundation for principled taper design. *(RodDNA parity + enables spey work.)*
   Extend to a full **casting deflection analysis**, matching hexrod.net's
   report: per-station angle, horizontal/vertical deflection, curvature, and
   stress, driven by adjustable modulus-of-elasticity and impact-factor (G)
   inputs. Visualize as (a) the deflected rod shape plotted in physical
   space (horiz/vert deflection per station) and (b) a stress-vs-station
   line graph — both alongside the existing Chart/Station Data/Mill Settings
   tabs.
2. **Taper design/edit mode** — edit stations, apply scale (multiplier/bias), and
   add/split ferrule stations. Enables designing new tapers (see `SPEY_DESIGN.md`).
3. **Planing-form settings** — 60° form depth at each station for the chosen geometry.
4. **Guide-spacing calculator** — from length/line/# guides, with a static-deflection option.
5. **Export** — CSV + a Hexrod-style station file; later a printable/PDF sheet.
6. **Dimension-change (delta) chart** — a new view alongside Chart/Station
   Data/Mill Settings showing the station-to-station change in flat-to-flat
   dimension (bar chart, one bar per 5" span, with a connecting line and
   value labels), plus vertical markers for each `Taper::ferrules()` location
   — hexrod.net's "Dimension Changes Every 5 Inches" report is the reference.
   Useful for spotting abrupt/uneven taper steps (e.g. a jump right after a
   ferrule) that a raw dimension chart can hide. Low-cost: the underlying
   diff is trivial from `profile()`, and ferrule markers reuse `ferrules()`
   from the Mill Settings work.

These are reflected in the README roadmap.
