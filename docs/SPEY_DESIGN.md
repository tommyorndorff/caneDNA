# Developing our own trout-spey tapers

Goal: design original **single-hand spey** (trout, ~10–11', 3–5 wt) and
**double-hand trout-spey** (~11–12'6", 2–5 wt, often 4-piece with split joints)
bamboo tapers in caneDNA — grounded in reference tapers we already hold, in
Garrison stress theory, and validated against the casting knowledge base.

This is the design plan. It depends on the **stress-curve engine** (roadmap #1)
and **design/edit mode** (roadmap #2) from `COMPARISON.md`.

## First design target (decided)

- **11'0" 3-weight trout spey**, 4-piece, **metal (nickel-silver) ferrules**.
- **PNW / BC style** — a **Skagit-style integrated head** (OPST Commando / RIO
  Skagit Trout Max lineage), not European Scandi/long-belly.
- Design to a **head grain window ≈ 250–300 gr** (grains, not AFTMA number).
- Action: **medium-progressive** — loads deep into the butt but keeps mid-section
  recovery to turn over light sink tips.
- **Seed:** the Zeitner 11' Switch Spey (5/6, 4-piece) from the library.

Note: the casting KB (RMA 1995–2004) predates Skagit/trout-spey culture, so it
informs the *action feel* (progressive vs fast, "loads deep") but not modern
integrated-head specifics.

## What we're designing

| Class | Length | Line | Pieces | Notes |
|---|---|---|---|---|
| Single-hand spey (trout) | 10'0"–11'0" | 3–5 wt | 3–4 | one grip; roll/switch/single-spey casts |
| Double-hand trout-spey | 11'0"–12'6" | 2–5 wt (Skagit/Scandi grain windows) | 4 | short two-hander; **split (spliced/ferruled) joints** |

Grain-window thinking matters more than "line weight" for spey: design toward a
head-grain target (e.g. a 11'0" 3-wt trout-spey ≈ 240–300 gr Skagit) rather than a
single AFTMA number.

## Reference tapers we already have

From the merged library (`type = Spey-Rod` or spey-named):

- **Zeitner "Switch Spey"** — 11'0" in **5/6, 7/8, 9/10**, in both **3-piece and
  4-piece**. These switch tapers are the closest starting points for both classes,
  and the 4-piece versions already model multi-ferrule construction.
- **Grantham 10' 6-wt (3pc)** — a short spey, good single-hand-spey anchor.
- **Waara / Warra 12'6" 6-wt**, **Hoergaard 12' 9-wt & 13'6" 10-wt**,
  **Gale & Sons 13'1" 4-wt** — longer/heavier two-handers for extrapolating the butt.

Use these as seeds, not gospel — most are heavier than a trout-spey target, so
expect to scale the whole rod down in power while keeping the butt long.

## Method (recommended: hybrid, seed → stress → tune)

1. **Pick a seed + target.** Start from the nearest reference (e.g. Zeitner 11'
   5/6, 4-piece) and set the target: length, grain window, and an *action goal*
   expressed as a stress-curve shape (flatter = more progressive/parabolic; higher
   mid-stress = faster).
2. **Compute the seed's stress curve** (Garrison) using the inputs already stored
   on each taper (`lwv`, `rav`, `tip_impact_factor`, `bamboo_density`, `tip_weight`,
   station params). This tells us what the reference *does* before we change it.
3. **Retarget.** Adjust the line/grain load and desired stress profile; solve for
   dimensions that produce it. Two levers, cheapest first:
   - **Scale** the seed (station multiplier/bias) to shift power and length.
   - **Local reshaping** of stations to flatten/steepen regions of the stress curve.
4. **Model the joints.** For 4-piece / double-hand, place ferrule or **spliced**
   stations explicitly; carry each section's own stations (we already store tip/butt
   sections for the Tom Morgan sheets — reuse that representation). Add a small
   dimension bump at ferrule stations (or none for splices) so the physical rod is
   buildable.
5. **Sanity-check against organic knowledge.** Compare the candidate's stress
   shape and description against the **casting KB** — e.g. aim for the profile of
   rods described "smooth/medium/loads deep" rather than "fast/tip-heavy", if that's
   the goal. This is where decades of RMA feedback steer the design.
6. **Emit build data.** Once happy, export the taper (CSV / Hexrod-style station
   file) and — later — planing-form depths and guide spacing.

### Why stress-first

Spey/trout-spey rods live or die on how the *butt* loads a heavy head. Designing by
eye on the dimension curve hides that; the stress curve makes the butt's behavior
explicit and lets us target a specific loading feel for a given grain window.

## Concrete next steps (turn this into code)

- [x] **Stress engine** in `roddna-core`: `Taper::stress_curve() -> Vec<[f64;2]>`
      (Garrison, Hex/Quad/Penta), recovered by decompiling `RodDNA_v20.jar`;
      unit-tested against the 58 records that ship stored `stresses` (median
      ~6% error).
- [x] Overlay the computed stress curve in the GUI (new "Stress" plot tab alongside the taper).
- [ ] **Design mode**: load a seed taper, edit stations, live-recompute stress and
      grain response; scale by multiplier/bias; add/split ferrule stations.
- [ ] Seed presets for the two target classes above.
- [ ] Export candidate taper (CSV / station file).

## Open questions to settle with the user

- Line system to target first: **Skagit**, **Scandi**, or a general grain window?
- Preferred action: progressive/parabolic (classic spey) vs. medium-fast?
- Splice vs. metal ferrule for the double-hander's joints?
