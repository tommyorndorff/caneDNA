# Morgan Hand Mill — how it works (digest)

A factual digest of the **Morgan Hand Mill (MHM) Manual** (Tom Morgan Rodsmiths,
chapters A–R, rev. 2005–2013), written in our own words to ground caneDNA's Mill
Settings tab and the planned **anvil-position visualizer**. Structured data
lives in [`data/kb/mhm_kb.json`](../data/kb/mhm_kb.json).

> **Attribution / copyright.** The MHM manual is copyrighted (Tom Morgan
> Rodsmiths, <https://www.troutrods.com>). We do **not** redistribute it — the raw
> PDFs and their extracted text stay local under `data/raw/mhm/` (gitignored). Only
> this derived digest and the JSON KB are committed. Facts and procedures are
> summarized for reference/education; buy the mill to get the manual.

## What the Hand Mill is

A hand-powered milling machine for bamboo strips — an alternative to the
traditional planing form. Two self-aligning parts: an **adjustable plane** holding
two carbide cutters that bevel both sides of a strip at once, and an **adjustable
bed** that holds the strip and sets the taper by rising above a reference **base**.
Swappable cutter heads make 8-, 6-, 5-, or 4-strip rods; Morgan uses a **61.5°**
included angle for 6-strip (not the traditional 60°) to hide glue seams.

## The core idea: taper vs. strip height (two separate things)

This is the concept that trips people up, and it's why caneDNA computes what it
does:

1. **Set the taper** — with a dial indicator on the plane, raise the bed at each
   **5-inch station** to a target height above the base. The "setting" at a station
   is the **cumulative rise of the bed** relative to a reference station, i.e. how
   much *lower* the finished strip is there than at the thick reference end. This
   equals caneDNA's `MillSetting.total_increase`.
2. **Mill to height** — the bed's rise does **not** set the strip's thickness. You
   cut the strip down and **measure its height** (half the flat-to-flat dimension)
   as you go, stopping at target; a hard stop then repeats it for every strip.

Morgan's manual reviews RodDNA's printed MHM report and says to **use the Station #,
Increment, Rod Dim, and Increase columns but disregard "Form Depth" and "Settings"**
(Form Depth should be strip height; the Settings column is wrong on some tapers).
caneDNA sidesteps that by computing `strip_depth` (per-geometry) and `total_increase`
itself — see [`docs/COMPARISON.md`](COMPARISON.md) roadmap #7.

## The bed: stations #0–#13

The steel adjustable bed (5⁄8 × 3⁄4 × 68 in) has **14 stations, #0–#13, on 5-inch
centers (0–60 in)**. Station #0 has no adjusting screw and is locked tight to the
base; the taper is referenced from ~station #2 (= 0.000") and rises toward the tip
(#13). Push screws (5⁄16-18) raise the bed; cap screws (1⁄4-20) lock it. The taper is
carried a station or two **past** the rod's real end so strips have extra length for
gluing.

## The anvils and the A–K letters (key for the visualizer)

Plastic (HDPE) **anvils sit on the bed and only support the strip — they do not set
the taper**, so they don't wear as the taper changes. The mill ships with five:
butt- and tip-roughing, butt-finishing, and two tip-finishing. Roughing anvils have
**3** hold-down positions (long/medium/short); **finishing anvils have 11**, and the
base carries **11 tapped "mill-stop" holes labeled A–K** matching them.

The **mill stop** screws into one lettered hole and (a) keeps the cutters from
hitting the strip's hold-down screw and (b) gives an automatic plane stop sized to
the section. Choosing the letter:

- The strip's **tip always anchors between bed stations #12 and #13** (as close to
  #13 as possible) so the anvil stays widest at its tip.
- The stop letter is read off the hold-down screw's position and **usually equals
  the hold-down letter**. Untapered rough cutting uses hole **A** (rightmost).
- **Worked example (7 ft rod):** strip in hole **D** (4th from the right) → the
  ferrule/tiptop lands at station #12 with ~3 in past it; butt taper set from
  station #3.
- The **extension bed** (one-piece rods to ~7 ft 6 in) uses hole **B** (rough/finish
  butt) and **A** (finish tip).

→ *Anvil-visualizer model:* register the section's tip near station #13, then count
A–K hold-down positions (A = rightmost) back to the strip's butt to pick the letter.

*Confirmed on a physical mill* (right-hand model, anvil marked "RB") and by the owner:
the bed carries **two** stamped systems — **numbers = the taper stations (1–13)** and
**letters A–K = the start / mill-stop positions**. The letters sit at **2.5″ pitch**
(half a station), so the odd letters land on the 5″ stations: **A1, C2, E3, G4, I5,
K6** (A = station 1 … K = station 6), covering only the tip-most ~25″. You **start a
finish pass ~hole D** (~7.5″ from the tip, just past the tiptop at #12 — the manual's
"start ~6″ from the tip"); **rough-cutting and one-piece / extension-bed setups start at
A**. So the start letter is roughly fixed by build type — it's the **station span** that
grows with section length, not the letter.

## Milling workflow (chapter L)

Cut **butt → tip**, many light passes (~0.010"/pass roughing, 0.001–0.002"
finishing), grip behind the adjusting head (never on it), pressure centered. Rough
on a wide roughing anvil to a 61.5° bevel just wider than the anvil (strip + stop in
hole A), add the spring-loaded **hold-down shoe** once an apex forms, then switch to
a finishing anvil and set the taper. Measure strip height with the **aluminum
measuring block** (grooves 61+/73+/91+ for 6/5/4-strip; calibrate with a 0.100" #38
drill). Finish in two stages — ~0.015" oversize, then fresh sharp cutters to size —
remove apex "fuzz" before gluing, and saw off the thick held-down butt.

## Accessories (chapters N–Q1)

- **Hollow fluter (N)** and **Magic Star cutter (P)** hollow the strip centers to
  cut weight (leave the rod **solid under ferrules**; increase section area ~2–5% to
  offset lost stiffness; never move the bed >0.020" between stations).
- **Swelled-butt kit (O)** — shims raise the finishing anvil for swells up to
  0.120" on the rod, over a 2.5" transition.
- **Extension bed (Q)** — one-piece rods to ~7 ft 6 in, cut across both anvils with
  the tip/butt transition between stations #7–#8.
- **Enamel scraper (Q1)** — 60° groove anvils to shave enamel flat.

## Bamboo prep (chapter K), briefly

Mill butt → tip; mark the butt. Reject nodes with dark heat marks; power fibers must
run the full glued-section depth. Straighten with heat (gun 350–450 °F), remove
twist (especially the first 18" of butts), heat-treat to light brown, and rest
treated strips ≥ 1 week before milling. Rough sections ≥ 6" over finished length
(+~2" at the butt for the hold-down screw).

## How this maps into caneDNA

| Manual concept | caneDNA today |
|---|---|
| Per-station "setting" (cumulative bed rise) | `MillSetting.total_increase` |
| Strip height (½ flat-to-flat, per geometry) | `MillSetting.strip_depth` (roadmap #7) |
| Bed stations every 5 in, referenced near the thick end | Mill Settings tab rows |
| Anvil A–K hold-down / mill-stop letters | **Anvil Layout tab** (`Taper::mill_bed_layouts`) — letter is a calibrated estimate |
| "Disregard RodDNA's Form Depth/Settings" | why we compute settings ourselves |

_Source: Morgan Hand Mill Manual, chapters A–R. Regenerate the local text with
`python3 scripts/build_mhm_kb.py` after placing the PDFs in `data/raw/mhm/pdf/`._
