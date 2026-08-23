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
| MHM report (RodDNA-specific) | ✅ | ⛔ | Not planned |

## Gap-driven roadmap (priority order)

1. **Garrison stress-curve engine** (`roddna-core`) — compute the stress curve from
   the taper + already-present inputs, for Hex first then Quad/Penta. Unlocks
   stress overlays and is the foundation for principled taper design. *(RodDNA parity + enables spey work.)*
2. **Taper design/edit mode** — edit stations, apply scale (multiplier/bias), and
   add/split ferrule stations. Enables designing new tapers (see `SPEY_DESIGN.md`).
3. **Planing-form settings** — 60° form depth at each station for the chosen geometry.
4. **Guide-spacing calculator** — from length/line/# guides, with a static-deflection option.
5. **Export** — CSV + a Hexrod-style station file; later a printable/PDF sheet.

These are reflected in the README roadmap.
