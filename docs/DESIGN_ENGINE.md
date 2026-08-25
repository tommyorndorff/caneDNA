# caneDNA design engine — roadmap

RodDNA parity is done (see `COMPARISON.md` gap roadmap #1–#7 and the Morgan
Hand Mill work). This document tracks the *forward* work: turning caneDNA from a
taper **browser/editor** into a taper **design** tool — an engine that can
predict how a rod behaves and, eventually, solve for a taper that hits a target
action.

The ordering is deliberate: each stage calibrates or is validated against the
previous one, and everything sits on top of the static Garrison `stress_curve`
already in `roddna-core`.

## Where we're going (targets)

- **5-wt trout rods** — the best-covered slice of the merged library and the
  casting KB, so the natural calibration target for the physics.
- **Trout-spey** — the design frontier. Seeded from the Bob Clay and Zeitner
  Switch Spey tapers already in the library (`SPEY_DESIGN.md`). Here a
  grain-window solver (does the rod load in its rated head range?) matters more
  than for single-hand rods.

## Stages

### A — Physics engines (predict behavior from a taper)

- **A1 · Static stress curve** — ✅ done. `Taper::stress_curve()`: Garrison
  cantilever bending-moment model, ~6% median error vs. the 58 records that
  ship stored `stresses`. Tells you *load* at each station.
- **A2 · Dynamic / modal engine** — ✅ done.
  Euler–Bernoulli variable-section cantilever → fundamental frequency, period,
  and effective (modal) tip mass/stiffness. Where A1 answers "how hard is each
  section working under load," A2 answers "how does the rod *move*" — the thing
  casters actually feel as fast/slow action and recovery. Rayleigh-quotient
  estimate with a uniform-cantilever assumed mode shape; validated against the
  closed-form prismatic-beam frequency. Inputs: per-geometry cross-section area
  and second moment of area `I(x)`, a bamboo Young's modulus (parameter,
  default ~2.4e6 psi), and the taper's own `bamboo_density`/`tip_weight`.
- **A2b · Casting deflection** — ✅ done. `Taper::casting_deflection()`: the
  static deflected *shape* under the casting load, hexrod.net-style. Reuses the
  shared casting-moment field (`casting_moments`, now also backing A1) as
  curvature `κ(x) = M(x)/(E·I(x))`, then a large-deflection butt→tip march
  giving per-station tangent angle, curvature, and horizontal/vertical
  position. Adjustable Young's modulus and load multiplier (default static
  1.0 — the casting `tip_impact_factor` of ~3–4 curls the shape unrealistically
  far, so it's a knob, not the default). GUI "Deflection" tab draws the bent
  rod at equal aspect. Same validation posture as A2 (curvature transcription +
  physical monotonicity checks; no stored shape ground truth).

### B — Inverse design (solve for a taper)

Given a target — a stress curve shape, a frequency, a grain window — search
taper space for a taper that hits it. Starts from a seed (`Taper::scaled` +
per-station edits are the manual version of this today) and optimizes stations
against an A1/A2 objective. This is the first stage that *creates* a taper
rather than measuring one.

### C — KB action model (calibrate to real rods)

Link the physics outputs (A1/A2) to how rods are actually described —
"fast/medium/full flex," line ratings, caster feedback — using the casting KB
(`data/kb/`). Best data coverage is 5-wt trout rods, so that's the calibration
anchor. Turns raw Hz / psi numbers into an action vocabulary the inverse
designer (B) and the assistant (D) can target.

### D — LLM-first design assistant

Natural-language taper design on top of A–C: "design me a medium-action 5-wt
7'6" 2-piece" → seed selection, inverse solve, physics check, explanation. The
engines (A/B) do the math; the model drives them and explains the result.

## Status snapshot

| Stage | What | Status |
|-------|------|--------|
| A1 | Static stress curve | ✅ done |
| A2 | Modal / dynamic engine | ✅ done |
| A2b | Casting deflection analysis | ✅ done |
| B  | Inverse-design optimizer | ⬜ planned |
| C  | KB action model | ⬜ planned |
| D  | LLM design assistant | ⬜ planned |
