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

The first stage that *creates* a taper rather than measuring one: given a
target, solve for the station dimensions that hit it. Manual today via
`Taper::scaled` + per-station edits in design mode.

**First cut — spec (decided):**

- **Target = a flat stress curve** at a user-chosen psi. This is the classic
  Garrison "flatten the stress curve" design move, and A1 (`stress_curve`) is
  the best-validated engine to solve against. Frequency (A2) and shape (A2b)
  targets are deferred — a single frequency is badly underdetermined, and A2b
  has no ground truth.
- **Solver = fixed-point inversion, not a generic optimizer.** The physics
  factorizes: `stress_i = M_i / (k · apex(d_i)³)`, and the moment `M_i` depends
  on the dimensions only weakly (through bamboo self-weight; the dominant
  tip/line/ferrule loads are dimension-independent). So each station inverts in
  near-closed-form to hit a target stress `T`:
  `d_i = (M_i / (k·T))^(1/3) / apex_conversion`. Recompute `M` with the new
  dimensions and repeat 2–3× to absorb the self-weight feedback. Simpler,
  faster, and more physically motivated than Gauss-Newton / global search,
  which we'd only reach for if a target proves un-invertible.
- **Constraint = monotonic** (dimension non-decreasing tip→butt), the one
  property nearly every real taper has; it rules out nonsensical solutions.
  Tip dimension is *not* pinned and no explicit smoothness penalty is applied
  in the first cut — a flat-stress target over a smooth moment field already
  yields a smooth taper.
- **API:** `Taper::solve_to_stress(target_psi, &SolveParams) -> Taper`, built on
  the existing shared `casting_moments()`. Wires into `DesignState` design mode
  as a "Solve to flat stress" control with live Stress-tab re-render.

Later extensions once the first cut lands: clone-another-rod's-stress target,
optional smoothness/tip-pin constraints, and frequency/grain-window objectives
(the trout-spey case, where "does it load in its head range" matters most).

### C — KB action model (calibrate to real rods) — ✅ first cut done

`Taper::action_profile()` turns the raw physics into the caster's vocabulary —
**fast / medium / full-flex** — from *where the Garrison stress curve peaks*
(the classic action tell: tip peak = fast, mid/butt peak = full/parabolic),
measured on the de-padded taper as a fraction of length.

**What the KB actually supports.** The intent was to calibrate physics against
the casting KB's action tags. Investigating the 316 library rods that carry
both physics inputs and a KB tag showed the tags are mostly **maker-reputation
level**, not per-taper: `parabolic`/`delicate` dominate and don't track
geometry. So the KB is *not* a training target. But the clean speed terms do
separate on the peak-stress metric — rods the KB calls **fast** peak at a
median ~0.13 of length from the tip, **slow** at ~0.53 — and that split
calibrates the `Fast < 0.25 ≤ Medium < 0.40 ≤ Full` thresholds. Frequency (A2)
and a peakiness ratio were tested and did *not* separate the classes, so they're
reported as context only. The KB's own tags are surfaced beside the physics
class (via `CastingKb::for_taper`) as corroboration, honestly labelled as crowd
feedback, not ground truth. GUI: an "Action" tab.

Later: per-station action along the rod, line-rating inference, and tightening
the calibration if/when model-level (not maker-level) KB coverage improves.

### D — LLM-first design assistant — ✅ first cut done

`Library::design(&DesignRequest, …)` turns a spec — line weight, length, piece
count, action — into a taper. Rather than synthesize from nothing, it does what
a rodmaker does: **pick the closest library taper as a seed and adapt it.**
Seeds are scored on line-weight match (dominant), action fit (peak-stress
location vs. the requested class's calibrated centre — Fast 0.13 / Medium 0.32
/ Full 0.53, from C), length closeness and pieces; only rods with the stress
inputs are eligible. The winner is rescaled to the requested length (shape
preserved), its line weight / pieces / ferrules set, and the result carries the
adapted taper, the achieved `ActionProfile`, and a plain-language rationale.

**This is the engine, not the words.** It's the deterministic tool an LLM would
call: the model parses "a soft 5-wt 7-footer" into a `DesignRequest` and
narrates the `DesignResult`; a GUI "Design assistant" form stands in for that
today, opening the result straight into design mode (B's solver + per-station
edits) to refine. Wiring an actual language model on top — via MCP or an
embedded model — is the remaining, separable step.

Later: solve directly to a target action (needs B extended beyond flat-stress),
grain-window objectives for spey, and multi-seed blending.

## Status snapshot

| Stage | What | Status |
|-------|------|--------|
| A1 | Static stress curve | ✅ done |
| A2 | Modal / dynamic engine | ✅ done |
| A2b | Casting deflection analysis | ✅ done |
| B  | Inverse-design optimizer | ✅ done (flat-stress) |
| C  | KB action model | ✅ done (first cut) |
| D  | LLM design assistant | ✅ done (seed-driven engine) |
