//! Core data model for RodDNA tapers.
//!
//! Deserializes `data/tapers.json` (produced by `scripts/convert_tapers.py`
//! from the original RodDNA v2.0 XML libraries) into typed Rust structs.

use serde::{Deserialize, Serialize};

/// A single rod model / taper.
///
/// Fields mirror the original RodDNA XML schema. Most numeric fields are
/// optional because a handful of records leave them blank.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Taper {
    pub name: Option<String>,
    #[serde(rename = "type")]
    pub rod_type: Option<String>,
    pub const_type: Option<String>,

    /// Overall length in inches (e.g. 102.0 == 8'6").
    pub length: Option<f64>,
    pub action_length: Option<f64>,
    pub line_weight: Option<f64>,
    pub line_length: Option<f64>,
    pub line_cast: Option<f64>,
    pub pieces: Option<f64>,

    pub ferrule_type: Option<String>,
    pub ferrule1_size: Option<String>,
    pub ferrule2_size: Option<String>,
    pub ferrule3_size: Option<String>,
    pub ferrule1_loc: Option<f64>,
    pub ferrule2_loc: Option<f64>,
    pub ferrule3_loc: Option<f64>,
    pub tiptop_size: Option<f64>,

    pub lwv: Option<f64>,
    pub rav: Option<f64>,
    pub tip_impact_factor: Option<f64>,
    pub bamboo_density: Option<f64>,
    pub tip_weight: Option<f64>,
    pub station_multiplier: Option<f64>,
    pub station_bias: Option<f64>,
    pub station_increment: Option<f64>,
    pub db_number: Option<f64>,

    pub notes: Option<String>,

    /// Flat-to-flat cross-section (inches) at each station.
    #[serde(default)]
    pub dimensions: Vec<f64>,
    /// Station positions (inches from tip), one per `dimensions` entry.
    #[serde(default)]
    pub stations: Vec<f64>,
    #[serde(default)]
    pub stresses: Vec<f64>,
    #[serde(default)]
    pub guide_spacings: Vec<f64>,
    #[serde(default)]
    pub guide_sizes: Vec<f64>,

    /// Attribution + informational metadata about where this taper came from.
    #[serde(default)]
    pub provenance: Option<Provenance>,
}

/// Where a taper was sourced from, carried per-record so attribution survives
/// merging multiple libraries. Extra keys from future sources are preserved.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Provenance {
    /// Human-readable library name, e.g. "RodDNA v2.0".
    pub source: Option<String>,
    pub author: Option<String>,
    pub source_url: Option<String>,
    /// File/collection within the source library.
    pub collection: Option<String>,
    pub license: Option<String>,
    /// ISO date this record was imported into caneDNA.
    pub imported: Option<String>,
    /// The source's own record id, if any.
    pub source_id: Option<f64>,
    /// Any additional source-specific metadata, preserved verbatim.
    #[serde(flatten, default)]
    pub extra: std::collections::BTreeMap<String, serde_json::Value>,
}

impl Taper {
    /// Number of taper stations that actually have a dimension.
    pub fn point_count(&self) -> usize {
        self.dimensions.len()
    }

    /// (station, dimension) pairs, zipped and truncated to the shorter of the two.
    ///
    /// Trailing zero-dimension points are dropped: some imported sources pad
    /// fixed-size station arrays with `0.0` past the rod's real taper length,
    /// which would otherwise plot as a spurious plunge to zero.
    ///
    /// Trailing *repeated-dimension* padding (a `0.286, 0.286, 0.286` tail) is
    /// deliberately **kept** here: the RodDNA-sourced records ship a stored
    /// `stresses` array computed over exactly these padded stations, so A1
    /// ([`stress_curve`](Self::stress_curve)) must see them to stay aligned
    /// with that reference. The physics engines that measure the *real* rod
    /// length (modal, deflection) strip it instead — see [`depadded`].
    ///
    /// [`depadded`]: Self::depadded
    pub fn profile(&self) -> Vec<[f64; 2]> {
        let mut points: Vec<[f64; 2]> = self
            .stations
            .iter()
            .zip(self.dimensions.iter())
            .map(|(&s, &d)| [s, d])
            .collect();
        while points.len() > 1 && points.last().map(|p| p[1]) == Some(0.0) {
            points.pop();
        }
        points
    }

    /// A clone with trailing fixed-width **padding** trimmed off the station
    /// arrays, so the profile ends at the rod's real taper length.
    ///
    /// Many imported sources pad past the last real station by repeating the
    /// last dimension (e.g. `…, 0.256, 0.286, 0.286, 0.286`); left in, those
    /// phantom inches inflate the integration length of the dynamic engines
    /// (modal frequency, deflected shape) and make two nominally identical rods
    /// look different. A trailing run of equal dimensions is treated as padding
    /// **only when the taper was still increasing into it** (the point before
    /// the run is strictly smaller) — that distinguishes real padding-at-the-max
    /// from a genuinely uniform or intentionally cylindrical section, which is
    /// left untouched. Trailing zeros are dropped either way.
    ///
    /// A1 stress deliberately does **not** use this (see [`profile`]); the new
    /// A2/A2b engines do.
    ///
    /// [`profile`]: Self::profile
    pub fn depadded(&self) -> Taper {
        let prof = self.profile(); // trailing zeros already removed
        let mut keep = prof.len();
        if keep >= 2 {
            let v = prof[keep - 1][1];
            // Walk back over the trailing run of equal dimensions.
            let mut k = keep - 1;
            while k > 0 && (prof[k - 1][1] - v).abs() < 1e-9 {
                k -= 1;
            }
            // Strip it only if the taper grew into the flat (padding at the
            // max), keeping the first point of the run.
            if k > 0 && prof[k - 1][1] < v - 1e-9 {
                keep = k + 1;
            }
        }
        let mut t = self.clone();
        t.stations.truncate(keep);
        t.dimensions.truncate(keep);
        t
    }

    /// A clone with every dimension linearly rescaled: `d' = d * multiplier +
    /// bias`, clamped at zero. Stations are untouched — this changes the
    /// rod's power/stiffness at a given length, not its length. The starting
    /// point for taper design: scale a seed taper up/down before reshaping
    /// individual stations.
    pub fn scaled(&self, multiplier: f64, bias: f64) -> Taper {
        let mut t = self.clone();
        for d in &mut t.dimensions {
            *d = (*d * multiplier + bias).max(0.0);
        }
        t
    }

    /// Inserts a new profile point at `station`, with its dimension linearly
    /// interpolated from the existing curve, keeping `stations`/`dimensions`
    /// sorted by station. No-op (returns `false`) if a point already exists
    /// there (within 1e-6"). Used to carve out an explicit breakpoint for a
    /// new ferrule location when designing a taper, so `mill_sections`/
    /// `ferrules` have a real station to split on.
    pub fn insert_station(&mut self, station: f64) -> bool {
        let profile = self.profile();
        if profile
            .iter()
            .any(|p| (p[0] - station).abs() < 1e-6)
        {
            return false;
        }
        let Some(dimension) = interpolate(&profile, station) else {
            return false;
        };
        let idx = self
            .stations
            .iter()
            .position(|&s| s > station)
            .unwrap_or(self.stations.len());
        self.stations.insert(idx, station);
        self.dimensions.insert(idx, dimension);
        true
    }

    /// A short, human-friendly grouping of this taper's `provenance.source`,
    /// collapsing the source library's own version strings into one bucket —
    /// e.g. "RodDNA v2.0" and "RodDNA v1.4 update" both become "RodDNA", and
    /// "David Ray's Taper Library (Hexrod)" becomes "Hexrod". Used to filter
    /// the library by where a taper came from. Returns the raw source string
    /// for any source not recognised here, and `None` if the record carries no
    /// source.
    pub fn source_group(&self) -> Option<String> {
        let src = self.provenance.as_ref()?.source.as_deref()?;
        let lower = src.to_lowercase();
        let label = if lower.contains("hexrod") {
            "Hexrod"
        } else if lower.contains("roddna") {
            "RodDNA"
        } else if lower.contains("taper sheets") {
            "Taper Sheets"
        } else if lower.contains("bob clay") {
            "Bob Clay"
        } else {
            return Some(src.to_string());
        };
        Some(label.to_string())
    }

    /// The maker token used to link a taper to the casting KB: the first word of
    /// the rod name (mirrors `scripts/build_casting_kb.py`).
    pub fn maker(&self) -> Option<String> {
        let name = self.name.as_deref()?;
        let tok = name
            .split([' ', ','])
            .next()?
            .trim_matches(|c: char| !c.is_alphanumeric());
        if tok.is_empty() {
            None
        } else {
            Some(tok.to_string())
        }
    }

    /// Lowercased "maker model" key from the first two name tokens, matching the
    /// keys in the casting KB's `models` map (e.g. "Payne 98" -> "payne 98").
    pub fn model_key(&self) -> Option<String> {
        let name = self.name.as_deref()?;
        fn strip(s: &str) -> &str {
            s.trim_matches(|c: char| !c.is_alphanumeric())
        }
        let mut toks = name.split([' ', ',']).filter(|t| !t.is_empty());
        let a = strip(toks.next()?);
        let b = strip(toks.next()?);
        if a.is_empty() || b.is_empty() {
            None
        } else {
            Some(format!("{} {}", a.to_lowercase(), b.to_lowercase()))
        }
    }

    /// Morgan Hand Mill settings for each station, given rough/finish oversize
    /// allowances (Tom Morgan's "2019 Bamboo Taper Sheets" workbook defaults:
    /// 0.07", 0.03"). `strip_depth` is the mill dial setting — the per-geometry
    /// strip depth (Hex = dimension/2, Quad/Penta larger, matching
    /// `planing_form_depths`); `total_increase` is the cumulative rise from the
    /// butt end to each station, i.e. the anvil setting.
    pub fn mill_settings(&self, rough_allowance: f64, finish_allowance: f64) -> Vec<MillSetting> {
        let geometry = PlaningFormGeometry::for_const_type(self.const_type.as_deref());
        settings_for_points(&self.profile(), geometry, rough_allowance, finish_allowance)
    }

    /// Per-piece Morgan Hand Mill sections (Tip / Mid n / Butt), split at
    /// ferrule locations when the record has them, else evenly by piece
    /// count. Each internal boundary shares the two profile points bracketing
    /// the ferrule with its neighbor, since a builder needs a station of
    /// reference on both sides of the ferrule joint.
    pub fn mill_sections(&self, rough_allowance: f64, finish_allowance: f64) -> Vec<MillSection> {
        let profile = self.profile();
        let geometry = PlaningFormGeometry::for_const_type(self.const_type.as_deref());
        let pieces = self.pieces.unwrap_or(1.0).round().max(1.0) as usize;
        if pieces <= 1 || profile.len() < pieces {
            return vec![MillSection {
                label: "Full rod".into(),
                approximate: false,
                settings: settings_for_points(&profile, geometry, rough_allowance, finish_allowance),
            }];
        }

        let ferrule_locs: Vec<f64> = [self.ferrule1_loc, self.ferrule2_loc, self.ferrule3_loc]
            .into_iter()
            .flatten()
            .filter(|&v| v != 0.0)
            .take(pieces - 1)
            .collect();
        let (cuts, approximate): (Vec<usize>, bool) = if ferrule_locs.len() == pieces - 1 {
            let cuts = ferrule_locs
                .iter()
                .map(|&loc| {
                    profile
                        .iter()
                        .position(|p| p[0] >= loc)
                        .unwrap_or(profile.len() - 1)
                        .max(1)
                })
                .collect();
            (cuts, false)
        } else {
            let n = profile.len();
            let cuts = (1..pieces).map(|k| (k * n / pieces).max(1)).collect();
            (cuts, true)
        };

        let mut bounds = vec![0usize];
        bounds.extend(cuts);
        bounds.push(profile.len());

        (0..pieces)
            .map(|k| {
                let start = if k == 0 {
                    0
                } else {
                    bounds[k].saturating_sub(1)
                };
                let end = if k == pieces - 1 {
                    profile.len()
                } else {
                    (bounds[k + 1] + 1).min(profile.len())
                };
                MillSection {
                    label: section_label(k, pieces),
                    approximate,
                    settings: settings_for_points(
                        &profile[start..end],
                        geometry,
                        rough_allowance,
                        finish_allowance,
                    ),
                }
            })
            .collect()
    }

    /// How each rod section registers on the Morgan Hand Mill bed: where the
    /// section's stations land on the mill's fixed #0-#13 grid, and the
    /// suggested A-K hold-down / mill-stop letter to start at.
    ///
    /// Grounded in the MHM manual (see `data/kb/mhm_kb.json`, `docs/MHM.md`):
    /// the bed has 14 stations on 5-inch centers; a section's **ferrule/tiptop
    /// point registers at station #12** with the strip overhanging ~3 in toward
    /// the tip end (#13), and the taper runs from #12 back toward the butt one
    /// station per 5 in. A section needs the **extension bed** if it can't fit
    /// (butt would fall past station #0) — e.g. a long one-piece rod.
    ///
    /// The A-K letter is the physical **start / mill-stop position**: you screw
    /// the strip down at the *lowest* letter that gives enough anvil space, so a
    /// section needing all 13 stations starts at A and shorter sections step up
    /// the letters. Because the strip's tip is fixed near #13, the letter tracks
    /// its BUTT hold-down and so depends on the *strip* length (finished section
    /// + `MHM_STRIP_ALLOWANCE_IN` of cutoffs/hold-down), which is why a rod's
    /// tip and butt share a letter: `clamp(13 - round((len + 8)/5), A..K)`. A
    /// normal 7 ft 2-piece rod lands on hole D (both sections); the longest
    /// sections / rough cutting use A. `letter_estimated` stays true: the exact
    /// hole shifts a little with how the strip is trimmed.
    ///
    /// One entry per `mill_sections` piece (Tip / Mid n / Butt / Full rod).
    pub fn mill_bed_layouts(&self, rough_allowance: f64, finish_allowance: f64) -> Vec<MillBedLayout> {
        self.mill_sections(rough_allowance, finish_allowance)
            .into_iter()
            .map(|section| {
                let stations: Vec<f64> = section.settings.iter().map(|s| s.station).collect();
                let length_in = match (stations.first(), stations.last()) {
                    (Some(&a), Some(&b)) => b - a,
                    _ => 0.0,
                };
                // Number of 5-in bed stations the taper spans.
                let station_span = (length_in / MHM_STATION_SPACING_IN).round().max(0.0) as usize;
                let butt_station = MHM_TIPTOP_STATION as i32 - station_span as i32;
                let fits_standard_bed = butt_station >= 0;
                // Lowest letter that gives enough anvil space, keyed off the
                // strip length (finished section + cutoffs/hold-down) so a rod's
                // tip and butt share a letter: all 13 stations -> A, shorter
                // strips step up. Longest sections (incl. extension bed) -> A.
                let strip_span =
                    ((length_in + MHM_STRIP_ALLOWANCE_IN) / MHM_STATION_SPACING_IN).round() as i32;
                let letter_index = (13 - strip_span).clamp(0, MHM_HOLD_DOWN_LETTERS as i32 - 1);
                let letter = (b'A' + letter_index as u8) as char;
                MillBedLayout {
                    label: section.label,
                    section_length_in: length_in,
                    station_span,
                    tiptop_station: MHM_TIPTOP_STATION,
                    butt_station,
                    tip_overhang_in: MHM_TIP_OVERHANG_IN,
                    fits_standard_bed,
                    needs_extension_bed: !fits_standard_bed,
                    letter,
                    letter_estimated: true,
                }
            })
            .collect()
    }

    /// Station-to-station change in flat-to-flat dimension: one entry per
    /// interior profile point, `(midpoint_station, delta)` where `delta` is
    /// the dimension at that point minus the dimension at the previous point.
    /// Mirrors hexrod.net's "Dimension Changes Every 5 Inches" report; the
    /// midpoint station labels each bar between the two stations it spans.
    pub fn dimension_deltas(&self) -> Vec<DimensionDelta> {
        let profile = self.profile();
        profile
            .windows(2)
            .map(|w| {
                let [s0, d0] = w[0];
                let [s1, d1] = w[1];
                DimensionDelta {
                    station: (s0 + s1) / 2.0,
                    from_station: s0,
                    to_station: s1,
                    delta: d1 - d0,
                }
            })
            .collect()
    }

    /// Planing-form V-groove depth at each station, for setting up a
    /// traditional bamboo planing form: the rodmaker planes each strip's
    /// outer face flush with the form's rail, which sets the strip's
    /// cross-section to exactly this depth.
    ///
    /// Recovered from RodDNA v2.0's own report (decompiling
    /// `com.tusoni.RodDNA.printing.PrintPlaningFormSettings`, since this
    /// formula isn't documented anywhere the source data ships). Only
    /// Hex/Quad/Penta are supported — RodDNA itself refuses to print a
    /// planing report for any other geometry ("Planing reports are only
    /// available for Hex, Quad and Penta geometries!"), because the
    /// dimension-to-depth relationship is a real per-geometry conversion,
    /// not a uniform half-dimension:
    /// - Hex: `dimension / 2` (the inradius — a strip sits apex-down, flat
    ///   face planed level with the rail).
    /// - Quad: `dimension / 2 * sqrt(2)` (the circumradius — RodDNA models
    ///   the Quad strip corner-down rather than flat-down).
    /// - Penta: `dimension / 1.809753` (RodDNA's own constant, matching a
    ///   regular pentagon's circumradius/side-length relationship it derives
    ///   internally as `1.903 * 0.951`).
    ///
    /// Each depth is then offset by the taper's own `station_bias *
    /// station_multiplier` (RodDNA reuses these two fields — otherwise
    /// unrelated to the stress calc, see `stress_curve` — as a planing-form
    /// adjustment). Returns an empty vec for unsupported geometries or a
    /// taper with no profile.
    ///
    /// Unlike `stress_curve`, there's no stored ground truth in the library
    /// to validate this against — `data/tapers.json` carries no planing-form
    /// depths anywhere. Unit tests below check the formula was transcribed
    /// correctly against hand-computed expected values, not against real
    /// builder data.
    pub fn planing_form_depths(&self) -> Vec<PlaningFormSetting> {
        let profile = self.profile();
        if profile.is_empty() {
            return Vec::new();
        }
        let Some(geometry) = PlaningFormGeometry::for_const_type(self.const_type.as_deref())
        else {
            return Vec::new();
        };
        let adjustment = self.station_bias.unwrap_or(0.0) * self.station_multiplier.unwrap_or(1.0);
        profile
            .into_iter()
            .map(|[station, dimension]| PlaningFormSetting {
                station,
                dimension,
                depth: geometry.depth(dimension) + adjustment,
            })
            .collect()
    }

    /// Guide placements from a static-deflection calculator: marching from
    /// the tip, each span is the longest run that keeps the rod's own
    /// self-weight sag at midspan under `params.max_sag_in`, treating the
    /// span as a simply-supported beam under uniform load (the standard
    /// `5wL⁴/384EI` beam formula). Spacing grows toward the butt because the
    /// rod gets stiffer there.
    ///
    /// This is an original caneDNA calculator, not a RodDNA port: investigating
    /// RodDNA's own "guide spacing" feature (decompiling
    /// `com.tusoni.RodDNA.models.ModelsDialog`/`GuidesXML`) found it's a
    /// bundled lookup table keyed by piece count and floor-matched rod
    /// length, not a physics calculation — nothing to port faithfully, and
    /// the library's stored `guide_spacings` on 49 records can diverge from
    /// even that table (apparent hand-edits), so they aren't a reliable
    /// validation oracle either. This implementation is a genuine
    /// static-deflection method instead.
    ///
    /// Simplifying assumptions (this is a design aid, not a precision
    /// instrument):
    /// - The cross-section is treated as an equivalent solid circular rod of
    ///   the same inradius (`dimension / 2`) for its bending stiffness
    ///   (`I = π/4 · r⁴`), rather than the exact hex/quad/penta section — a
    ///   common simplification for guide-spacing rules of thumb.
    /// - Cross-sectional area uses the same `0.866 · dimension²` hex-area
    ///   coefficient `stress_curve` already applies to every geometry.
    /// - `params.modulus_psi` (bamboo's modulus of elasticity) isn't a stored
    ///   per-taper field — real cane varies roughly 3–6 million psi; the
    ///   default is a commonly cited average, adjustable by the caller.
    /// - `self.bamboo_density` is used when present, else the same 0.668
    ///   lb/in³ fallback `stress_curve`'s validation set is built on.
    ///
    /// Stops at `action_length` (the working/flexing length, excluding the
    /// handle) if present, else the profile's last station. Returns an empty
    /// vec if the taper has no profile.
    pub fn guide_spacing(&self, params: &GuideSpacingParams) -> Vec<GuidePlacement> {
        let profile = self.profile();
        if profile.len() < 2 {
            return Vec::new();
        }
        let action_length = self
            .action_length
            .filter(|&l| l > 0.0)
            .unwrap_or_else(|| profile.last().unwrap()[0]);
        let density = self.bamboo_density.unwrap_or(0.668);

        let mut placements = vec![GuidePlacement {
            station: 0.0,
            span_from_previous: 0.0,
        }];
        let mut position = 0.0;
        // A generous cap on iterations, not a target: guards against a
        // pathological taper (near-zero dimension) looping forever rather
        // than limiting normal output, which typically needs 8-12 guides.
        for _ in 0..500 {
            if position >= action_length {
                break;
            }
            let dimension = interpolate(&profile, position).unwrap_or(0.0);
            let radius = dimension / 2.0;
            let moment_of_inertia = std::f64::consts::PI / 4.0 * radius.powi(4);
            let area = dimension * dimension * 0.866;
            let unit_weight = area * density;
            if moment_of_inertia <= 0.0 || unit_weight <= 0.0 {
                break;
            }
            let span = (384.0 * params.modulus_psi * moment_of_inertia * params.max_sag_in
                / (5.0 * unit_weight))
                .powf(0.25);
            // Floor avoids a degenerate near-zero step from an extreme
            // input; cap prevents overshooting the working length.
            let span = span.max(0.5).min(action_length - position);
            position += span;
            placements.push(GuidePlacement {
                station: position,
                span_from_previous: span,
            });
        }
        placements
    }

    /// A shared `#`-prefixed metadata block (name, type/construction/length/
    /// line/pieces, provenance) used by both export formats, so attribution
    /// travels with the taper wherever it's exported to.
    fn export_header(&self) -> String {
        let mut lines = vec![
            "# caneDNA taper export".to_string(),
            format!("# Name: {}", self.name.as_deref().unwrap_or("(unnamed)")),
            format!(
                "# Type: {} | Construction: {} | Length: {} | Line: {} | Pieces: {}",
                self.rod_type.as_deref().unwrap_or("—"),
                self.const_type.as_deref().unwrap_or("—"),
                self.length.map_or("—".to_string(), |l| format!("{l}\"")),
                fmt_opt(self.line_weight),
                fmt_opt(self.pieces),
            ),
        ];
        if let Some(p) = &self.provenance {
            lines.push(format!(
                "# Source: {} ({})",
                p.source.as_deref().unwrap_or("unknown"),
                p.author.as_deref().unwrap_or("unknown"),
            ));
        }
        lines.join("\n")
    }

    /// CSV export: the shared metadata header as `#`-prefixed comment lines
    /// (skippable by any CSV reader that ignores leading `#` rows, a common
    /// convention for hobby/scientific data exchange), then a header row and
    /// one `station,dimension` row per profile point.
    pub fn to_csv(&self) -> String {
        let mut out = self.export_header();
        out.push_str("\nStation (in),Dimension (in)\n");
        for [station, dimension] in self.profile() {
            out.push_str(&format!("{station:.2},{dimension:.4}\n"));
        }
        out
    }

    /// Plain-text station file: the shared metadata header, then one
    /// whitespace-separated `station  dimension` line per profile point —
    /// the simple station/dimension list rodmakers commonly exchange.
    ///
    /// This is *not* a verified byte-for-byte reproduction of any specific
    /// rodmaking software's native file format (no such spec is available
    /// to us); it's an honestly-labeled plain list in that spirit, portable
    /// to any text editor or spreadsheet.
    pub fn to_station_file(&self) -> String {
        let mut out = self.export_header();
        out.push_str("\n# Station (in)\tDimension (in)\n");
        for [station, dimension] in self.profile() {
            out.push_str(&format!("{station:.2}\t{dimension:.4}\n"));
        }
        out
    }

    /// Ferrule size/type/location info for each ferrule slot that's actually
    /// set. Unused slots are stored as `0.0` location / `"None"` size rather
    /// than `null`, so those placeholders are skipped rather than shown.
    pub fn ferrules(&self) -> Vec<FerruleInfo> {
        let profile = self.profile();
        [
            (1, self.ferrule1_loc, &self.ferrule1_size),
            (2, self.ferrule2_loc, &self.ferrule2_size),
            (3, self.ferrule3_loc, &self.ferrule3_size),
        ]
        .into_iter()
        .filter_map(|(index, loc, size)| {
            let location = loc.filter(|&v| v != 0.0)?;
            let size = size.as_deref().filter(|s| !s.is_empty() && *s != "None")?;
            let dimension_at_location = interpolate(&profile, location)?;
            let is_hex = self
                .const_type
                .as_deref()
                .map(|s| s.to_lowercase().starts_with("hex"))
                .unwrap_or(false);
            let outside_diameter_apexes = if is_hex {
                Some(dimension_at_location * 2.0 / 3.0_f64.sqrt())
            } else {
                None
            };
            Some(FerruleInfo {
                index,
                size: size.to_string(),
                ferrule_type: self.ferrule_type.clone(),
                location,
                dimension_at_location,
                outside_diameter_apexes,
            })
        })
        .collect()
    }

    /// Garrison bending stress at each station, reconstructed from RodDNA
    /// v2.0's own casting-load model (recovered by decompiling
    /// `com.tusoni.RodDNA.models.ModelsCalc`, since neither the formula nor
    /// the units of `lwv`/`rav` are documented anywhere the source data
    /// ships — those two fields turn out to be unrelated derived taper
    /// classifiers, not stress inputs).
    ///
    /// The model treats the rod as a cantilever fixed at the butt, loaded by:
    /// a concentrated tip load (fly line being cast + the tip-top guide,
    /// scaled by `tip_impact_factor`), a distributed line-weight load along
    /// the cast length, a small fixed "varnish + guides" distributed load,
    /// concentrated ferrule loads at their stations, and the bamboo's own
    /// frustum-segment self-weight. Bending moment at each inch is the sum of
    /// every load's (weight × lever arm) tip-ward of that station; stress is
    /// `moment / (apex_dimension^3 * geometry_factor)`, where `apex_dimension`
    /// converts the stored flat-to-flat width to the across-corners diameter
    /// RodDNA's own per-geometry constants (0.12 Hex / 0.1667 Quad / 0.0956
    /// Penta / ...) are defined against, each then bumped by a small
    /// residual fit against real records (see `geometry_factor`).
    ///
    /// Returns `(station, stress_psi)` pairs, one per `profile()` point.
    /// Returns an empty vec if required inputs (line weight/length/cast,
    /// impact factor, bamboo density, tip weight, geometry) are missing.
    ///
    /// Fit and validated against the 58 RodDNA v2.0/v1.4 records that ship
    /// their own `stresses` (spanning Hex/Quad/Penta, 1149 station-points):
    /// median relative error ~6%, p90 ~29%. A handful of very-fine-tip
    /// ("Midge") records run far worse at their tip-most stations, where a
    /// tiny absolute dimension error is cubed into a large relative stress
    /// error — a numerical-sensitivity artifact of the tip, not a formula
    /// problem. Two known program-option inputs (the ferrule catalog's
    /// starting size, and any user-overridden line weight/ferrule-weight
    /// tables) aren't recoverable from the taper data and are approximated
    /// with RodDNA's shipped defaults.
    pub fn stress_curve(&self) -> Vec<[f64; 2]> {
        let Some(m) = self.casting_moments(None) else {
            return Vec::new();
        };
        let (geometry_factor, sides) = geometry_factor(self.const_type.as_deref());
        // RodDNA's per-geometry constant applies to the across-corners
        // diameter, not the flat-to-flat width this crate stores.
        let apex_conversion = 1.0 / (std::f64::consts::PI / sides).cos();

        let stress_per_inch: Vec<f64> = (0..m.action_length)
            .map(|i| {
                let apex_dim = m.dim[i] * apex_conversion;
                m.moment[i] / (apex_dim.powi(3) * geometry_factor)
            })
            .collect();

        m.profile
            .iter()
            .map(|&[station, _]| [station, stress_per_inch[m.index_for(station)]])
            .collect()
    }

    /// Per-inch casting bending-moment field, shared by [`stress_curve`] (A1)
    /// and [`casting_deflection`] (A2b). Reconstructs RodDNA's load model:
    /// concentrated tip load (fly line held in the air + tip weight),
    /// distributed line weight, a small varnish/guide distributed load,
    /// concentrated ferrule weights, and the bamboo's own frustum self-weight —
    /// each scaled by the tip impact factor. The moment at station `i+1"` is
    /// the sum of every load's `weight × lever arm` tip-ward of it, in oz·in.
    ///
    /// `impact_override` replaces the taper's stored `tip_impact_factor` (the
    /// deflection view exposes it as an adjustable "how hard is the cast"
    /// knob); `None` uses the stored value. Returns `None` when any required
    /// physics input is missing.
    ///
    /// [`stress_curve`]: Self::stress_curve
    /// [`casting_deflection`]: Self::casting_deflection
    fn casting_moments(&self, impact_override: Option<f64>) -> Option<CastingMoments> {
        let profile = self.profile();
        if profile.len() < 2 {
            return None;
        }
        let (line_weight, line_length, line_cast) =
            (self.line_weight?, self.line_length?, self.line_cast?);
        let (tip_impact_factor, bamboo_density, tip_weight) =
            (self.tip_impact_factor?, self.bamboo_density?, self.tip_weight?);
        let tip_impact_factor = impact_override.unwrap_or(tip_impact_factor);
        let line_weight_idx = line_weight.round() as usize;
        if line_weight_idx < 1 || line_weight_idx > LINE_WEIGHTS_GRAINS.len() {
            return None;
        }

        let action_length = profile.last().unwrap()[0].round() as usize;
        if action_length < 1 {
            return None;
        }

        // Per-inch dimension curve, linearly interpolated between the
        // station-spaced profile points (RodDNA computes moments at 1"
        // resolution regardless of the station spacing used for input).
        let dim: Vec<f64> = (0..=action_length)
            .map(|i| interpolate(&profile, i as f64).unwrap_or(0.0))
            .collect();

        let line_weight_grains = LINE_WEIGHTS_GRAINS[line_weight_idx - 1];
        let tip_multiplier = (line_weight_grains / 437.0 / line_length * line_cast + tip_weight)
            * tip_impact_factor;
        let line_unit_weight = line_weight_grains / 437.0 / (line_cast * 12.0);

        let mut tip_moments = vec![0.0; action_length];
        let mut line_moments = vec![0.0; action_length];
        let mut vg_moments = vec![0.0; action_length];
        const VG_FACTOR: f64 = 0.001573;
        for i in 0..action_length {
            let arm = (i + 1) as f64;
            tip_moments[i] = arm * tip_multiplier;
            line_moments[i] = line_unit_weight * arm * (arm * 0.5) * tip_impact_factor;
            vg_moments[i] = (0..=i)
                .map(|k| VG_FACTOR * ((i - k + 1) as f64 + 0.5) * tip_impact_factor)
                .sum();
        }

        let mut ferrule_moments = vec![0.0; action_length];
        for (loc, size) in [
            (self.ferrule1_loc, &self.ferrule1_size),
            (self.ferrule2_loc, &self.ferrule2_size),
            (self.ferrule3_loc, &self.ferrule3_size),
        ] {
            let Some(loc) = loc.filter(|&v| v != 0.0) else {
                continue;
            };
            let Some(weight) = size.as_deref().and_then(ferrule_weight) else {
                continue;
            };
            let start = loc.floor().max(0.0) as usize;
            for i in start..action_length {
                ferrule_moments[i] += weight * ((i as f64 - loc) * tip_impact_factor);
            }
        }

        // Bamboo self-weight: each 1" segment is a frustum between
        // consecutive per-inch dimensions; volume/COG use the standard
        // frustum formulas (0.866 == area coefficient for a flat-to-flat
        // hex cross-section, reused as-is for every geometry, matching
        // RodDNA's own implementation).
        let bamboo_inch_weight_factor = bamboo_density / 3.0;
        let mut bamboo_weight = vec![0.0; action_length];
        let mut cog = vec![0.0; action_length];
        for i in 1..action_length {
            let (d1, d2) = (dim[i - 1], dim[i]);
            let a1 = d1 * d1 * 0.866;
            let a2 = d2 * d2 * 0.866;
            let cross = (a1 * a2).sqrt();
            let vol = a1 + a2 + cross;
            bamboo_weight[i - 1] = vol * bamboo_inch_weight_factor;
            cog[i - 1] = 0.25 * (a2 + 2.0 * cross + 3.0 * a1) / vol;
        }
        let mut bamboo_moments = vec![0.0; action_length];
        for i in 0..action_length {
            bamboo_moments[i] = (0..=i)
                .map(|j| bamboo_weight[j] * ((i - j + 1) as f64 + cog[j]) * tip_impact_factor)
                .sum();
        }

        let moment: Vec<f64> = (0..action_length)
            .map(|i| {
                tip_moments[i]
                    + line_moments[i]
                    + vg_moments[i]
                    + ferrule_moments[i]
                    + bamboo_moments[i]
            })
            .collect();

        Some(CastingMoments {
            profile,
            dim,
            moment,
            action_length,
        })
    }

    /// **A2b — casting deflection analysis.** The static deflected shape of the
    /// rod under the same casting load [`stress_curve`](Self::stress_curve)
    /// uses for bending stress, à la hexrod.net's deflection report. Where the
    /// modal analysis ([`modal_analysis`](Self::modal_analysis)) gives a
    /// frequency, this gives the *shape* a caster sees: how far and at what
    /// angle each station swings under load.
    ///
    /// Method: bending curvature `κ(x) = M(x) / (E·I(x))` from the shared
    /// casting-moment field and the true per-geometry second moment of area,
    /// then a large-deflection march from the clamped butt to the tip —
    /// `θ += κ·ds`, position `+= (cos θ, sin θ)·ds` — so horizontal and
    /// vertical deflection separate the way hexrod reports them (rather than a
    /// small-angle `y(x)` that overstates tip travel on a deep-bending rod).
    /// Coordinates are relative to the butt, which sits at the origin with the
    /// rod initially pointing along +x.
    ///
    /// `youngs_modulus_psi` and `impact_factor` are the adjustable inputs
    /// (bamboo modulus varies; impact factor is the load multiplier, default
    /// static 1.0). Returns one entry per `profile()` station, tip-last, or an
    /// empty vec if the physics inputs `stress_curve` needs are missing. Like
    /// `modal_analysis`, there's no stored ground truth to validate against —
    /// it's the same validated moment field divided by a physical `EI`.
    pub fn casting_deflection(&self, params: &DeflectionParams) -> Vec<DeflectionStation> {
        // Measure the real rod: drop fixed-width padding first (see `depadded`).
        let this = self.depadded();
        let Some(m) = this.casting_moments(Some(params.impact_factor)) else {
            return Vec::new();
        };
        let (_factor, sides) = geometry_factor(this.const_type.as_deref());
        let e = params.youngs_modulus_psi;

        // Per-inch curvature (1/in). M is in oz·in; /16 -> lbf·in so that
        // M/(E·I) is dimensionless per inch. I is the true polygon second
        // moment of area at that inch's dimension.
        let curvature: Vec<f64> = (0..m.action_length)
            .map(|i| {
                let (_area, inertia) = polygon_section(m.dim[i], sides);
                if inertia <= 0.0 {
                    0.0
                } else {
                    (m.moment[i] / OZ_PER_LB) / (e * inertia)
                }
            })
            .collect();

        // March from the clamped butt (highest station index) toward the tip,
        // accumulating angle and position. `curvature[i]` is the curvature at
        // the section ~`i+1"` from the tip; the clamp is at `action_length`.
        let n = m.action_length;
        let mut angle = 0.0_f64;
        let mut hx = 0.0_f64;
        let mut vy = 0.0_f64;
        // Per-inch-station deflection state, indexed by section i.
        let mut state = vec![(0.0_f64, 0.0_f64, 0.0_f64); n]; // (angle, hx, vy)
        for i in (0..n).rev() {
            angle += curvature[i]; // ds = 1"
            hx += angle.cos();
            vy += angle.sin();
            state[i] = (angle, hx, vy);
        }

        m.profile
            .iter()
            .map(|&[station, _]| {
                let idx = m.index_for(station);
                let (angle, hx, vy) = state[idx];
                DeflectionStation {
                    station,
                    angle_deg: angle.to_degrees(),
                    curvature_per_in: curvature[idx],
                    horizontal_in: hx,
                    vertical_in: vy,
                    moment_oz_in: m.moment[idx],
                }
            })
            .collect()
    }

    /// **B — inverse design: solve for a flat stress curve.** Returns a copy of
    /// this taper whose station dimensions have been reshaped so its Garrison
    /// stress curve ([`stress_curve`](Self::stress_curve)) is as close to a
    /// uniform `target_psi` as the loads allow — the classic "flatten the
    /// stress curve" design move, run automatically instead of by hand.
    ///
    /// Method: a fixed-point inversion, not a generic optimizer. Stress varies
    /// as `1 / d³` at a fixed load, so to move a station's stress onto the
    /// target its dimension is scaled by `(stress / target)^(1/3)` — a
    /// Newton-in-log(d) step. The step is driven by [`stress_curve`]'s own
    /// output rather than an open-loop formula, so it self-corrects for the
    /// weak dimension→moment coupling (bamboo self-weight) and for the report's
    /// per-inch sampling, and it converges in a handful of passes (the target
    /// exponent makes each pass nearly exact).
    ///
    /// With `params.monotonic` (the default) the result is forced
    /// non-decreasing tip→butt — the one property nearly every real taper has;
    /// it mainly cleans up tip-region noise, since a flat-stress taper is
    /// naturally monotonic (`d³ ∝ M`, and `M` grows toward the butt). Only the
    /// `dimensions` change; stations, geometry, and all metadata are preserved,
    /// so every derived view (mill settings, planing form, …) stays valid.
    ///
    /// Returns `None` if the taper lacks the physics inputs `stress_curve`
    /// needs (same gate), or if `target_psi` isn't positive.
    ///
    /// [`stress_curve`]: Self::stress_curve
    pub fn solve_to_stress(&self, target_psi: f64, params: &SolveParams) -> Option<Taper> {
        if target_psi <= 0.0 || self.casting_moments(None).is_none() {
            return None;
        }
        let mut out = self.clone();
        let iterations = params.iterations.max(1);
        let n_pts = out.profile().len();

        for _ in 0..iterations {
            // `stress_curve` returns one (station, stress) pair per profile
            // point, aligned with `dimensions[0..n_pts]`.
            let curve = out.stress_curve();
            if curve.is_empty() {
                return None;
            }
            for (j, &[_station, stress]) in curve.iter().enumerate().take(n_pts) {
                if stress > 0.0 {
                    out.dimensions[j] *= (stress / target_psi).cbrt();
                }
            }
            if params.monotonic {
                // Cumulative max from the tip: never let the taper shrink
                // toward the butt.
                let mut running = 0.0_f64;
                for d in out.dimensions.iter_mut().take(n_pts) {
                    running = running.max(*d);
                    *d = running;
                }
            }
        }
        Some(out)
    }

    /// **A2 — dynamic / modal analysis.** Estimates the rod's fundamental
    /// bending frequency (and the effective tip-referred mass/stiffness that
    /// go with it) by treating the rod as a variable-cross-section
    /// Euler–Bernoulli cantilever, clamped at the butt and free at the tip.
    ///
    /// Where [`stress_curve`](Self::stress_curve) (A1) answers "how hard is
    /// each section working under a casting load," this answers "how does the
    /// rod *move*" — the fast/slow action and recovery a caster actually
    /// feels. A stiffer, lighter rod rings at a higher frequency and recovers
    /// faster; a soft full-flex rod is slow and low.
    ///
    /// Method: the Rayleigh quotient
    /// `ω² = ∫ EI(x) ψ''(x)² dx / (∫ m(x) ψ(x)² dx + M_tip ψ(L)²)`
    /// with the closed-form first mode shape of a *uniform* cantilever as the
    /// assumed shape `ψ`. For a genuinely prismatic rod this reproduces the
    /// exact Euler–Bernoulli frequency (verified in tests); for a real taper
    /// it's a well-behaved upper-bound estimate — good enough to *rank*
    /// tapers by action and to drive inverse design (stage B), the intended
    /// use, not a substitute for a full FE modal solve.
    ///
    /// `EI(x)` uses a true per-geometry cross-section (regular-polygon area and
    /// second moment of area from the flat-to-flat `dimensions`), not the
    /// hex-area approximation A1 reuses from RodDNA. Young's modulus is a
    /// parameter (bamboo varies a lot); specific weight defaults to the
    /// taper's own `bamboo_density`, and a tip point mass from `tip_weight`.
    ///
    /// Returns `None` if the profile is too short (< 2 points) or has no
    /// length. There is **no stored ground truth** for frequency in the
    /// library (unlike A1's `stresses`), so this is validated by construction
    /// against the analytic prismatic-beam solution, not against real records.
    pub fn modal_analysis(&self, params: &ModalParams) -> Option<ModalAnalysis> {
        // Measure the real rod: drop fixed-width padding first (see `depadded`).
        let this = self.depadded();
        let profile = this.profile();
        if profile.len() < 2 {
            return None;
        }
        // Clamp at the butt (largest station), free at the tip (station 0);
        // the active bending length is the taper's own extent.
        let length = profile.last().unwrap()[0];
        if length <= 0.0 {
            return None;
        }
        let (_factor, sides) = geometry_factor(this.const_type.as_deref());

        // Specific weight (oz/in^3) -> mass density (lbf·s²/in per in³).
        let specific_weight_oz = params
            .specific_weight_oz_in3
            .or(this.bamboo_density)
            .unwrap_or(DEFAULT_BAMBOO_SPECIFIC_WEIGHT_OZ);
        let mass_density = (specific_weight_oz / OZ_PER_LB) / GRAVITY_IN_S2;
        let youngs = params.youngs_modulus_psi;

        // March in 1" steps like `stress_curve`; x is distance from the
        // clamped butt, so x = length - station.
        let n = length.round() as usize;
        if n < 1 {
            return None;
        }
        let step = length / n as f64;

        // Uniform-cantilever first mode: ψ(x) = cosh βx - cos βx
        //   - σ (sinh βx - sin βx), with βL = 1.8751041 and σ = 0.7340955.
        let beta = FIRST_MODE_BETA_L / length;
        let sigma = FIRST_MODE_SIGMA;
        let shape = |x: f64| -> (f64, f64) {
            let bx = beta * x;
            let (s, c, sh, ch) = (bx.sin(), bx.cos(), bx.sinh(), bx.cosh());
            let psi = ch - c - sigma * (sh - s);
            // ψ'' = β² [cosh βx + cos βx - σ(sinh βx + sin βx)]
            let psi2 = beta * beta * (ch + c - sigma * (sh + s));
            (psi, psi2)
        };

        // Trapezoidal integration of numerator (∫ EI ψ''²) and the
        // distributed part of the denominator (∫ m ψ²).
        let mut numer = 0.0;
        let mut denom = 0.0;
        let sample = |i: usize| -> (f64, f64) {
            let station = (i as f64) * step;
            let x = length - station;
            let d = interpolate(&profile, station).unwrap_or(0.0);
            let (area, inertia) = polygon_section(d, sides);
            let (psi, psi2) = shape(x);
            let ei = youngs * inertia;
            let m = mass_density * area;
            (ei * psi2 * psi2, m * psi * psi)
        };
        let (mut prev_n, mut prev_d) = sample(0);
        for i in 1..=n {
            let (cur_n, cur_d) = sample(i);
            numer += 0.5 * (prev_n + cur_n) * step;
            denom += 0.5 * (prev_d + cur_d) * step;
            prev_n = cur_n;
            prev_d = cur_d;
        }

        // Tip point mass (line-top guide + tip section) at the free end.
        let tip_mass = (this.tip_weight.unwrap_or(0.0) / OZ_PER_LB) / GRAVITY_IN_S2;
        let (psi_tip, _) = shape(length);
        denom += tip_mass * psi_tip * psi_tip;

        if denom <= 0.0 || numer <= 0.0 {
            return None;
        }
        let omega2 = numer / denom;
        let omega = omega2.sqrt();
        let frequency_hz = omega / (2.0 * std::f64::consts::PI);

        // Tip-referred effective (modal) mass: normalize the generalized mass
        // by the tip amplitude so k = ω²·m reads as an equivalent tip spring.
        let effective_mass = denom / (psi_tip * psi_tip);
        let effective_stiffness = omega2 * effective_mass;

        Some(ModalAnalysis {
            frequency_hz,
            period_ms: 1000.0 / frequency_hz,
            effective_mass_oz: effective_mass * GRAVITY_IN_S2 * OZ_PER_LB,
            effective_stiffness_lb_in: effective_stiffness,
            tip_mass_oz: this.tip_weight.unwrap_or(0.0),
            active_length_in: length,
        })
    }
}

/// AFTMA fly-line weight standard, in grains, indexed by line weight number
/// (index 0 == a 1-weight). RodDNA's shipped default table; the app allows
/// overriding it via a program option we can't recover from taper data.
const LINE_WEIGHTS_GRAINS: [f64; 13] = [
    166.0, 217.0, 285.0, 359.0, 439.0, 509.0, 587.0, 668.0, 779.0, 859.0, 939.0, 1019.0, 1099.0,
];

/// Standard nickel-silver ferrule weights (oz), indexed by size in 64ths
/// starting at 1/64". RodDNA's shipped default table.
const STANDARD_FERRULE_WEIGHTS: [f64; 17] = [
    0.085, 0.126, 0.162, 0.194, 0.225, 0.271, 0.328, 0.358, 0.379, 0.404, 0.465, 0.526, 0.587,
    0.648, 0.709, 0.779, 0.849,
];

/// Ferrule weight (oz) for a size string like `"11/64"`. Truncated-ferrule
/// weights aren't modeled (rare in the library; falls back to standard).
fn ferrule_weight(size: &str) -> Option<f64> {
    if size.is_empty() || size.eq_ignore_ascii_case("none") {
        return None;
    }
    let numerator: usize = size.split('/').next()?.trim().parse().ok()?;
    STANDARD_FERRULE_WEIGHTS.get(numerator.checked_sub(1)?).copied()
}

/// RodDNA's per-geometry stress section-modulus constant and side count.
/// Defaults to Hex when unrecognized (matches RodDNA's own fallback).
///
/// `stress_curve` combines each constant with an across-corners conversion
/// (`1 / cos(pi / sides)`, applied to the stored flat-to-flat dimension
/// before cubing — RodDNA's constants are defined against that axis, not
/// flat-to-flat). The values below aren't RodDNA's raw 0.12/0.0956/0.1667;
/// they're fit directly against the 58 RodDNA-shipped `stresses` records
/// (median computed/known ratio == 1.0 for Hex/Penta/Quad), which lands
/// ~5-6% below what combining the raw constant with the conversion in
/// isolation would predict. We can't attribute that residual to a specific
/// missing term in the decompiled source (candidates: the ferrule catalog's
/// program-configurable starting size, or floating-point/rounding
/// differences from the original Java) — see `stress_curve`'s doc comment
/// for overall accuracy. Hepta/Octa have no library records to validate
/// against, so they get the same average residual, unvalidated.
fn geometry_factor(const_type: Option<&str>) -> (f64, f64) {
    match const_type.map(|s| s.to_lowercase()) {
        Some(s) if s.starts_with("penta") => (0.0514, 5.0),
        Some(s) if s.contains("quad") => (0.0617, 4.0),
        Some(s) if s.starts_with("hepta") => (0.0735, 7.0),
        Some(s) if s.starts_with("octa") => (0.0899, 8.0),
        _ => (0.0829, 6.0),
    }
}

/// Shared per-inch casting bending-moment field (see `Taper::casting_moments`),
/// consumed by both the stress curve (A1) and the deflection analysis (A2b).
struct CastingMoments {
    /// The taper's `profile()` points (station, dimension), tip-first.
    profile: Vec<[f64; 2]>,
    /// Per-inch flat-to-flat dimension, index 0..=action_length.
    dim: Vec<f64>,
    /// Per-inch total bending moment (oz·in), index 0..action_length; entry
    /// `i` is the moment at the section ~`i+1"` from the tip.
    moment: Vec<f64>,
    action_length: usize,
}

impl CastingMoments {
    /// Per-inch array index for a profile `station` (inches from tip), matching
    /// RodDNA's 1"-resolution moment grid.
    fn index_for(&self, station: f64) -> usize {
        if station <= 0.0 {
            0
        } else {
            (station.round() as usize)
                .saturating_sub(1)
                .min(self.action_length - 1)
        }
    }
}

/// Tunable inputs for [`Taper::casting_deflection`] (A2b): the bamboo modulus
/// and a load multiplier.
#[derive(Debug, Clone, PartialEq)]
pub struct DeflectionParams {
    /// Young's modulus of the bamboo, psi (see [`ModalParams`]).
    pub youngs_modulus_psi: f64,
    /// Load multiplier applied to the whole casting-moment field. `1.0`
    /// (the default) is the **static** deflected shape — the rod held out with
    /// the line hanging, no dynamic amplification — which is what a "deflected
    /// rod" picture usually shows. Raising it mimics a harder, dynamically
    /// loaded cast (Garrison's `tip_impact_factor` is ~3–4; the stress curve
    /// uses that value, but for a *shape* it curls the rod unrealistically far,
    /// so deflection defaults to static instead).
    pub impact_factor: f64,
}

impl Default for DeflectionParams {
    fn default() -> Self {
        DeflectionParams {
            youngs_modulus_psi: 2.4e6,
            impact_factor: 1.0,
        }
    }
}

/// One station of [`Taper::casting_deflection`] (A2b). `horizontal_in` /
/// `vertical_in` are the deflected position of this station relative to the
/// butt (butt at origin, rod initially along +x), so plotting the pairs draws
/// the bent rod; `angle_deg` is the local tangent angle there.
#[derive(Debug, Clone, PartialEq)]
pub struct DeflectionStation {
    pub station: f64,
    /// Local tangent angle relative to the (horizontal) butt axis, degrees.
    pub angle_deg: f64,
    /// Bending curvature `M/(E·I)` at this station, 1/in.
    pub curvature_per_in: f64,
    /// Deflected-shape coordinate along the initial rod axis, inches from butt.
    pub horizontal_in: f64,
    /// Deflected-shape coordinate transverse to the initial axis (the bend),
    /// inches.
    pub vertical_in: f64,
    /// Casting bending moment at this station, oz·in.
    pub moment_oz_in: f64,
}

/// Tunable inputs for [`Taper::solve_to_stress`] (stage B).
#[derive(Debug, Clone, PartialEq)]
pub struct SolveParams {
    /// Fixed-point iterations to absorb the self-weight feedback. Converges
    /// fast (the dimension→moment coupling is weak); ~4 is ample.
    pub iterations: usize,
    /// Force the solved taper non-decreasing tip→butt (the default). Off leaves
    /// the raw per-station inversion, which can dip slightly at a noisy tip.
    pub monotonic: bool,
}

impl Default for SolveParams {
    fn default() -> Self {
        SolveParams {
            iterations: 4,
            monotonic: true,
        }
    }
}

/// Tunable inputs for [`Taper::modal_analysis`] (A2). Bamboo's stiffness varies
/// widely by culm, grade, and heat-treatment, so Young's modulus is exposed
/// rather than baked in.
#[derive(Debug, Clone, PartialEq)]
pub struct ModalParams {
    /// Young's modulus of the bamboo, psi. Split-cane is typically ~2.0e6 to
    /// ~4.0e6 psi along the fiber; the default is a mid-range value.
    pub youngs_modulus_psi: f64,
    /// Override the specific weight (oz/in³). `None` uses the taper's own
    /// `bamboo_density`, falling back to RodDNA's shipped default.
    pub specific_weight_oz_in3: Option<f64>,
}

impl Default for ModalParams {
    fn default() -> Self {
        ModalParams {
            youngs_modulus_psi: 2.4e6,
            specific_weight_oz_in3: None,
        }
    }
}

/// Result of [`Taper::modal_analysis`] (A2) — the rod's fundamental bending
/// mode, expressed both as a frequency and as an equivalent tip spring/mass.
#[derive(Debug, Clone, PartialEq)]
pub struct ModalAnalysis {
    /// Fundamental (first-mode) bending frequency, Hz. Higher = faster action.
    pub frequency_hz: f64,
    /// Period of that mode, milliseconds (`1000 / frequency_hz`) — a rough
    /// proxy for recovery time.
    pub period_ms: f64,
    /// Effective modal mass referred to the tip, oz.
    pub effective_mass_oz: f64,
    /// Equivalent tip stiffness `k = ω²·m`, lbf/in.
    pub effective_stiffness_lb_in: f64,
    /// The tip point mass included in the model, oz (from `tip_weight`).
    pub tip_mass_oz: f64,
    /// Cantilever length used (tip-to-butt taper extent), inches.
    pub active_length_in: f64,
}

/// First-mode eigenvalue `βL` and shape constant `σ` of a uniform cantilever
/// (clamped-free), used as the assumed shape in the Rayleigh quotient.
const FIRST_MODE_BETA_L: f64 = 1.875_104_1;
const FIRST_MODE_SIGMA: f64 = 0.734_095_5;
/// Standard gravity in inches/s², to convert weight (lbf) to mass.
const GRAVITY_IN_S2: f64 = 386.088;
const OZ_PER_LB: f64 = 16.0;
/// RodDNA's shipped default bamboo specific weight (oz/in³).
const DEFAULT_BAMBOO_SPECIFIC_WEIGHT_OZ: f64 = 0.668;

/// Area (in²) and second moment of area `I` (in⁴) about a centroidal axis of a
/// regular `sides`-gon whose flat-to-flat width (apothem × 2) is `dimension`.
///
/// For any regular polygon with ≥ 3 sides the second-moment tensor is
/// isotropic — the same `I` about every centroidal in-plane axis — so a single
/// value is meaningful regardless of how the strip is clocked. Derived from the
/// apothem `a = dimension / 2` and half-angle `θ = π/sides`:
///   `A = n·a²·tanθ`,  `I = (n·a⁴/4)·tanθ·(1 + tan²θ/3)`.
/// (Checks out against the square `s⁴/12` and hex `(√3/2)·w²` area.)
fn polygon_section(dimension: f64, sides: f64) -> (f64, f64) {
    if dimension <= 0.0 || sides < 3.0 {
        return (0.0, 0.0);
    }
    let a = dimension / 2.0;
    let theta = std::f64::consts::PI / sides;
    let t = theta.tan();
    let area = sides * a * a * t;
    let inertia = sides * a.powi(4) / 4.0 * t * (1.0 + t * t / 3.0);
    (area, inertia)
}

/// Fixed finish+enamel allowance from the source spreadsheet, never user-adjustable.
const MHM_ENAMEL_ALLOWANCE: f64 = 0.003;

fn settings_for_points(
    points: &[[f64; 2]],
    geometry: Option<PlaningFormGeometry>,
    rough_allowance: f64,
    finish_allowance: f64,
) -> Vec<MillSetting> {
    // The strip depth milled per station is a per-geometry conversion of the
    // flat-to-flat dimension, not a uniform half — RodDNA's own MHM report
    // (`PrintMHMSettings`) uses the same conversion as its planing-form report.
    // Hex is dimension/2 (the inradius); Quad/Penta differ. Fall back to
    // dimension/2 for geometries the planing form doesn't cover (hepta/octa/
    // rectangular/unspecified), preserving prior behavior for those.
    let depth = |dimension: f64| geometry.map_or(dimension / 2.0, |g| g.depth(dimension));
    let n = points.len();
    let butt_depth = points.last().map(|p| depth(p[1])).unwrap_or(0.0);
    points
        .iter()
        .enumerate()
        .map(|(i, &[station, dimension])| {
            let strip_depth = depth(dimension);
            MillSetting {
                station,
                anvil_number: n - 1 - i,
                dimension,
                strip_depth,
                rough_oversize: strip_depth + rough_allowance,
                finish_oversize: strip_depth + finish_allowance,
                finish_enamel: strip_depth + MHM_ENAMEL_ALLOWANCE,
                total_increase: butt_depth - strip_depth,
            }
        })
        .collect()
}

fn section_label(k: usize, pieces: usize) -> String {
    match (k, pieces) {
        (0, _) => "Tip".into(),
        (k, p) if k == p - 1 => "Butt".into(),
        (k, _) => format!("Mid {k}"),
    }
}

/// Formats an optional numeric field for export/display, dropping a
/// trailing `.0` for whole numbers (line weight, pieces) and falling back to
/// an em dash when absent.
fn fmt_opt(v: Option<f64>) -> String {
    match v {
        Some(x) if x.fract() == 0.0 => format!("{}", x as i64),
        Some(x) => format!("{x}"),
        None => "—".to_string(),
    }
}

/// Linear interpolation of dimension at an arbitrary station within the
/// profile's range (ferrule locations often fall between station grid
/// points). Clamps to the nearest endpoint outside the range.
fn interpolate(profile: &[[f64; 2]], station: f64) -> Option<f64> {
    if profile.is_empty() {
        return None;
    }
    if station <= profile[0][0] {
        return Some(profile[0][1]);
    }
    for w in profile.windows(2) {
        let [s0, d0] = w[0];
        let [s1, d1] = w[1];
        if station >= s0 && station <= s1 {
            if s1 == s0 {
                return Some(d0);
            }
            let t = (station - s0) / (s1 - s0);
            return Some(d0 + t * (d1 - d0));
        }
    }
    Some(profile.last().unwrap()[1])
}

/// One row of Morgan Hand Mill dial settings, derived from a taper's profile.
#[derive(Debug, Clone, PartialEq)]
pub struct MillSetting {
    pub station: f64,
    /// Anvil/mill station number, descending from tip (highest) to butt (0).
    pub anvil_number: usize,
    pub dimension: f64,
    /// Depth of the strip milled at this station: the per-geometry conversion
    /// of the flat-to-flat `dimension` (Hex = dimension/2, Quad/Penta larger).
    /// Equals `dimension / 2` for hex and for geometries without a planing-form
    /// conversion.
    pub strip_depth: f64,
    pub rough_oversize: f64,
    pub finish_oversize: f64,
    pub finish_enamel: f64,
    pub total_increase: f64,
}

/// One piece's worth of Morgan Hand Mill settings (Tip / Mid n / Butt).
#[derive(Debug, Clone, PartialEq)]
pub struct MillSection {
    pub label: String,
    /// True if no ferrule-location data existed and stations were split evenly.
    pub approximate: bool,
    pub settings: Vec<MillSetting>,
}

/// The Morgan Hand Mill bed has 14 adjusting stations, #0-#13, on 5-inch
/// centers (numbers 1-13 are stamped on the bed; see `docs/MHM.md`).
pub const MHM_STATION_SPACING_IN: f64 = 5.0;
/// A section's ferrule/tiptop point registers at bed station #12.
pub const MHM_TIPTOP_STATION: u8 = 12;
/// The strip overhangs ~3 in past the tiptop station toward the tip end (#13).
pub const MHM_TIP_OVERHANG_IN: f64 = 3.0;
/// There are 11 lettered start / mill-stop positions, A-K.
pub const MHM_HOLD_DOWN_LETTERS: usize = 11;
/// The A-K start-position letters are stamped at 2.5-inch pitch (half a taper
/// station), so the odd letters coincide with the 5-inch stations: A=station 1,
/// C=2, E=3, G=4, I=5, K=6 (confirmed from a mill photo). Distinct from the
/// numbered taper stations 1-13.
///
/// You screw the strip down at the **lowest** letter that gives enough anvil
/// space: a section needing all 13 stations starts at A, and shorter sections
/// step up the letters. The letter tracks where the strip's BUTT hold-down
/// screw lands (its tip is fixed near #13), so it's driven by the *strip*
/// length, not the taper length — which is why both the tip and butt of one
/// rod share a letter. caneDNA models it as
/// `clamp(13 - round((section_len + MHM_STRIP_ALLOWANCE_IN) / 5), A..K)`,
/// reproducing the manual's 7 ft 2-piece rod (51-in strips -> hole D for both
/// tip and butt) and A for the longest sections / rough cutting.
pub const MHM_LETTER_PITCH_IN: f64 = 2.5;
/// Extra strip length beyond the finished section: ~6 in of end cutoffs plus
/// ~2 in at the butt for the hold-down screw (per the manual's 7 ft example:
/// 43 in finished + 6 + 2 = 51 in). Used to place the butt hold-down letter.
pub const MHM_STRIP_ALLOWANCE_IN: f64 = 8.0;

/// How one rod section registers on the Morgan Hand Mill bed. See
/// `Taper::mill_bed_layouts`.
#[derive(Debug, Clone, PartialEq)]
pub struct MillBedLayout {
    pub label: String,
    pub section_length_in: f64,
    /// Number of 5-inch bed stations the taper spans.
    pub station_span: usize,
    /// Bed station the ferrule/tiptop registers at (always #12).
    pub tiptop_station: u8,
    /// Bed station the section's butt lands on (#12 − span). Negative means the
    /// section is too long for the standard bed.
    pub butt_station: i32,
    /// Strip length past the tiptop station toward the tip end (~3 in).
    pub tip_overhang_in: f64,
    /// True if the section fits the standard 60-inch bed (butt_station ≥ 0).
    pub fits_standard_bed: bool,
    /// True if the section needs the extension bed (or is a one-piece rod).
    pub needs_extension_bed: bool,
    /// Suggested A-K hold-down / mill-stop letter (see `MHM_IN_PER_LETTER`).
    pub letter: char,
    /// Always true: the letter is an estimate, not read from a mill.
    pub letter_estimated: bool,
}

/// Station-to-station change in flat-to-flat dimension between two adjacent
/// profile points, labeled at their midpoint station.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DimensionDelta {
    pub station: f64,
    pub from_station: f64,
    pub to_station: f64,
    pub delta: f64,
}

/// Planing-form V-groove depth at one station. See `Taper::planing_form_depths`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlaningFormSetting {
    pub station: f64,
    pub dimension: f64,
    pub depth: f64,
}

/// The three geometries RodDNA's own planing-form report supports, each with
/// its own dimension-to-depth conversion. See `Taper::planing_form_depths`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlaningFormGeometry {
    Hex,
    Quad,
    Penta,
}

impl PlaningFormGeometry {
    fn for_const_type(const_type: Option<&str>) -> Option<Self> {
        match const_type.map(|s| s.to_lowercase()) {
            Some(s) if s.starts_with("hex") => Some(Self::Hex),
            Some(s) if s.contains("quad") => Some(Self::Quad),
            Some(s) if s.starts_with("penta") => Some(Self::Penta),
            _ => None,
        }
    }

    fn depth(&self, dimension: f64) -> f64 {
        match self {
            Self::Hex => dimension / 2.0,
            Self::Quad => dimension / 2.0 * std::f64::consts::SQRT_2,
            Self::Penta => dimension / 1.809753,
        }
    }
}

/// Tunable inputs to `Taper::guide_spacing`'s static-deflection calculator.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GuideSpacingParams {
    /// Bamboo modulus of elasticity, psi. Real cane varies roughly
    /// 3,000,000-6,000,000; the default is a commonly cited average.
    pub modulus_psi: f64,
    /// Maximum self-weight sag tolerated at midspan before a guide is
    /// placed, in inches.
    pub max_sag_in: f64,
}

impl Default for GuideSpacingParams {
    fn default() -> Self {
        Self {
            modulus_psi: 4_000_000.0,
            max_sag_in: 0.1,
        }
    }
}

/// One guide's position from `Taper::guide_spacing`. The first entry is
/// always `station: 0.0` (the tip-top); `span_from_previous` is `0.0` for it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GuidePlacement {
    pub station: f64,
    pub span_from_previous: f64,
}

/// Ferrule size/type/location info for one ferrule on a taper.
#[derive(Debug, Clone, PartialEq)]
pub struct FerruleInfo {
    pub index: usize,
    pub size: String,
    pub ferrule_type: Option<String>,
    /// Inches from tip.
    pub location: f64,
    /// Interpolated flat-to-flat dimension at `location`.
    pub dimension_at_location: f64,
    /// Outside diameter measured corner-to-corner (apex to apex), Hex only.
    pub outside_diameter_apexes: Option<f64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Meta {
    pub source: String,
    #[serde(default)]
    pub author: String,
    pub count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Library {
    pub meta: Meta,
    pub models: Vec<Taper>,
}

impl Library {
    /// Parse a library from a JSON string.
    pub fn from_json(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }

    /// Distinct rod types present, sorted.
    pub fn rod_types(&self) -> Vec<String> {
        let mut v: Vec<String> = self
            .models
            .iter()
            .filter_map(|m| m.rod_type.clone())
            .collect();
        v.sort();
        v.dedup();
        v
    }

    /// Distinct provenance source groups present (see `Taper::source_group`),
    /// sorted — the values a source filter offers.
    pub fn source_groups(&self) -> Vec<String> {
        let mut v: Vec<String> = self.models.iter().filter_map(|m| m.source_group()).collect();
        v.sort();
        v.dedup();
        v
    }
}

/// A single cited casting-feedback snippet from the RMA archive.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CastingSnippet {
    pub quote: String,
    /// Action tags detected in this snippet (fast / slow / parabolic / …).
    #[serde(default)]
    pub actions: Vec<String>,
    pub year: Option<i64>,
    pub date: Option<String>,
    pub subject: Option<String>,
    pub author: Option<String>,
}

/// Casting feedback aggregated for one maker or model.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MakerCasting {
    /// Display label (present on model entries, e.g. "Payne 98").
    #[serde(default)]
    pub label: Option<String>,
    #[serde(default)]
    pub mentions_with_casting: u64,
    #[serde(default)]
    pub snippets_shown: u64,
    /// Action-tag counts across all matching sentences, most common first.
    #[serde(default)]
    pub action_counts: std::collections::BTreeMap<String, u64>,
    #[serde(default)]
    pub snippets: Vec<CastingSnippet>,
}

/// The casting knowledge base (from `scripts/build_casting_kb.py`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CastingKb {
    #[serde(default)]
    pub makers: std::collections::BTreeMap<String, MakerCasting>,
    /// Model-level entries keyed by lowercased "maker model" (e.g. "payne 98").
    #[serde(default)]
    pub models: std::collections::BTreeMap<String, MakerCasting>,
}

impl CastingKb {
    pub fn from_json(s: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(s)
    }

    /// Casting feedback for a taper. Prefers a model-level match (maker + model
    /// designator from the first two name tokens); falls back to maker-level.
    /// Returns the display label and the matching entry.
    pub fn for_taper(&self, taper: &Taper) -> Option<(String, &MakerCasting)> {
        if let Some(key) = taper.model_key() {
            if let Some(mc) = self.models.get(&key) {
                let label = mc.label.clone().unwrap_or(key);
                return Some((label, mc));
            }
        }
        let maker = taper.maker()?;
        self.makers
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case(&maker))
            .map(|(k, v)| (k.clone(), v))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_bundled_library() {
        // The JSON lives at repo-root/data/tapers.json.
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/tapers.json");
        let text = std::fs::read_to_string(path).expect("read tapers.json");
        let lib = Library::from_json(&text).expect("parse library");
        assert_eq!(lib.models.len(), lib.meta.count);
        assert!(lib.models.len() > 600);
        // Every model with dimensions should have matching stations.
        for m in &lib.models {
            assert_eq!(m.stations.len(), m.dimensions.len(), "{:?}", m.name);
        }
        assert!(lib.rod_types().iter().any(|t| t == "Spey-Rod"));
        // Every taper must carry attribution.
        for m in &lib.models {
            let p = m.provenance.as_ref().expect("provenance present");
            assert!(p.source.is_some(), "{:?}", m.name);
        }
        // Source groups collapse the version-tagged RodDNA sources into one
        // bucket, and Hexrod/Taper Sheets/Bob Clay are present.
        let groups = lib.source_groups();
        for g in ["Hexrod", "RodDNA", "Taper Sheets", "Bob Clay"] {
            assert!(groups.iter().any(|s| s == g), "missing source group {g}");
        }
    }

    #[test]
    fn source_group_collapses_versions_and_recognises_libraries() {
        let with_source = |s: &str| Taper {
            provenance: Some(Provenance {
                source: Some(s.to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };
        assert_eq!(
            with_source("RodDNA v1.4 update").source_group().as_deref(),
            Some("RodDNA")
        );
        assert_eq!(
            with_source("RodDNA v2.0").source_group().as_deref(),
            Some("RodDNA")
        );
        assert_eq!(
            with_source("David Ray's Taper Library (Hexrod)")
                .source_group()
                .as_deref(),
            Some("Hexrod")
        );
        assert_eq!(
            with_source("2019 Bamboo Taper Sheets (Tom Morgan)")
                .source_group()
                .as_deref(),
            Some("Taper Sheets")
        );
        // Unknown source falls back to the raw string; no source -> None.
        assert_eq!(
            with_source("Some Future Library").source_group().as_deref(),
            Some("Some Future Library")
        );
        assert_eq!(Taper::default().source_group(), None);
    }

    #[test]
    fn mill_bed_layout_registers_tiptop_at_station_12() {
        // A ~42-inch single section: stations 0..42 every ~5 in. Matches the
        // manual's worked 7' tip example (tiptop -> station #12, hole D).
        let stations: Vec<f64> = vec![0.0, 5.0, 10.0, 15.0, 20.0, 25.0, 30.0, 35.0, 40.0, 42.0];
        let dimensions: Vec<f64> = vec![
            0.192, 0.206, 0.220, 0.234, 0.248, 0.262, 0.276, 0.290, 0.304, 0.310,
        ];
        let t = Taper {
            stations,
            dimensions,
            ..Default::default()
        };
        let layouts = t.mill_bed_layouts(0.07, 0.03);
        assert_eq!(layouts.len(), 1);
        let l = &layouts[0];
        assert_eq!(l.tiptop_station, 12);
        assert!((l.section_length_in - 42.0).abs() < 1e-9);
        assert_eq!(l.station_span, 8); // round(42/5)
        assert_eq!(l.butt_station, 4); // 12 - 8
        assert!(l.fits_standard_bed && !l.needs_extension_bed);
        assert_eq!(l.letter, 'D'); // typical finish-milling start, ~7.5" from tip
        assert!(l.letter_estimated);
    }

    #[test]
    fn mill_bed_layout_flags_extension_bed_for_long_section() {
        // A 70-inch section overruns the 60-inch bed (span 14 > 12).
        let stations: Vec<f64> = (0..=14).map(|i| i as f64 * 5.0).collect();
        let dimensions: Vec<f64> = (0..=14).map(|i| 0.10 + i as f64 * 0.02).collect();
        let t = Taper {
            stations,
            dimensions,
            ..Default::default()
        };
        let l = &t.mill_bed_layouts(0.07, 0.03)[0];
        assert_eq!(l.station_span, 14);
        assert!(l.butt_station < 0);
        assert!(!l.fits_standard_bed && l.needs_extension_bed);
        // A section that overruns the bed (one-piece / extension-bed) starts at A.
        assert_eq!(l.letter, 'A');
    }

    #[test]
    fn casting_kb_links_by_maker() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/kb/casting_kb.json");
        let text = std::fs::read_to_string(path).expect("read casting_kb.json");
        let kb = CastingKb::from_json(&text).expect("parse kb");
        assert!(kb.makers.contains_key("Garrison"));
        let t = Taper {
            name: Some("Garrison 212 8' 5wt".to_string()),
            ..Default::default()
        };
        let (_label, mc) = kb.for_taper(&t).expect("Garrison casting notes");
        assert!(!mc.snippets.is_empty());

        // A well-known model should resolve to a model-level entry.
        let payne = Taper {
            name: Some("Payne 98 7'6\" 5wt".to_string()),
            ..Default::default()
        };
        let (label, _mc) = kb.for_taper(&payne).expect("Payne 98 notes");
        assert_eq!(label, "Payne 98");
    }

    #[test]
    fn mill_settings_matches_morgan_taper_sheet() {
        // Butt section of the "Master" sheet in Tom Morgan's "2019 Bamboo
        // Taper Sheets" workbook, using its default oversize allowances.
        let t = Taper {
            stations: vec![
                -5.0, 0.0, 5.0, 10.0, 15.0, 20.0, 25.0, 30.0, 35.0, 40.0, 45.0, 50.0,
            ],
            dimensions: vec![
                0.166, 0.184, 0.206, 0.214, 0.22, 0.244, 0.258, 0.272, 0.298, 0.316, 0.334, 0.334,
            ],
            ..Default::default()
        };
        let settings = t.mill_settings(0.07, 0.03);
        assert_eq!(settings.len(), 12);

        let first = &settings[0];
        assert_eq!(first.anvil_number, 11);
        assert!((first.strip_depth - 0.083).abs() < 1e-9);
        assert!((first.rough_oversize - 0.153).abs() < 1e-9);
        assert!((first.finish_oversize - 0.113).abs() < 1e-9);
        assert!((first.finish_enamel - 0.086).abs() < 1e-9);
        assert!((first.total_increase - 0.084).abs() < 1e-9);

        let last = settings.last().unwrap();
        assert_eq!(last.anvil_number, 0);
        assert!((last.strip_depth - 0.167).abs() < 1e-9);
        assert!((last.total_increase - 0.0).abs() < 1e-9);
    }

    #[test]
    fn mill_settings_strip_depth_is_geometry_aware() {
        // For non-hex geometries the milled strip depth is the same
        // per-geometry conversion the planing-form report uses, not a flat
        // dimension/2. Quad = dimension/2 * sqrt(2); Penta = dimension/1.809753.
        let dims = vec![0.100, 0.200];
        let stations = vec![0.0, 5.0];
        let quad = Taper {
            stations: stations.clone(),
            dimensions: dims.clone(),
            const_type: Some("Quad".into()),
            ..Default::default()
        };
        let q = quad.mill_settings(0.0, 0.0);
        assert!((q[0].strip_depth - 0.100 / 2.0 * std::f64::consts::SQRT_2).abs() < 1e-9);
        // Oversize and total_increase build on the geometry-aware depth.
        assert!((q[0].rough_oversize - q[0].strip_depth).abs() < 1e-9);
        assert!((q[0].total_increase - (q[1].strip_depth - q[0].strip_depth)).abs() < 1e-9);

        let penta = Taper {
            stations,
            dimensions: dims,
            const_type: Some("Penta".into()),
            ..Default::default()
        };
        let p = penta.mill_settings(0.0, 0.0);
        assert!((p[1].strip_depth - 0.200 / 1.809753).abs() < 1e-9);
        // Hex-less/unknown geometry still falls back to half the dimension.
        assert!(p[0].strip_depth > 0.100 / 2.0);
    }

    fn synthetic_profile(n: usize) -> (Vec<f64>, Vec<f64>) {
        let stations: Vec<f64> = (0..n).map(|i| i as f64 * 5.0).collect();
        let dimensions: Vec<f64> = (0..n).map(|i| 0.07 + i as f64 * 0.01).collect();
        (stations, dimensions)
    }

    #[test]
    fn mill_sections_splits_at_ferrule_locations() {
        let (stations, dimensions) = synthetic_profile(24); // 0..115"
        let t = Taper {
            stations,
            dimensions,
            pieces: Some(2.0),
            ferrule1_loc: Some(42.0),
            ..Default::default()
        };
        let sections = t.mill_sections(0.07, 0.03);
        assert_eq!(sections.len(), 2);

        assert_eq!(sections[0].label, "Tip");
        assert!(!sections[0].approximate);
        assert_eq!(sections[0].settings.first().unwrap().station, 0.0);
        assert_eq!(sections[0].settings.last().unwrap().station, 45.0);

        assert_eq!(sections[1].label, "Butt");
        assert!(!sections[1].approximate);
        assert_eq!(sections[1].settings.first().unwrap().station, 40.0);
        assert_eq!(sections[1].settings.last().unwrap().station, 115.0);

        // Each section restarts its own anvil numbering at 0 for its butt-most station.
        assert_eq!(sections[0].settings.last().unwrap().anvil_number, 0);
        assert_eq!(sections[1].settings.last().unwrap().anvil_number, 0);
    }

    #[test]
    fn mill_sections_falls_back_to_even_split_without_ferrule_data() {
        let (stations, dimensions) = synthetic_profile(24);
        let t = Taper {
            stations,
            dimensions,
            pieces: Some(3.0),
            // ferrule locs left unset (None), matching a record with no ferrule data.
            ..Default::default()
        };
        let sections = t.mill_sections(0.07, 0.03);
        assert_eq!(sections.len(), 3);
        assert_eq!(sections[0].label, "Tip");
        assert_eq!(sections[1].label, "Mid 1");
        assert_eq!(sections[2].label, "Butt");
        assert!(sections.iter().all(|s| s.approximate));
    }

    #[test]
    fn dimension_deltas_diffs_adjacent_stations() {
        let t = Taper {
            stations: vec![0.0, 5.0, 10.0, 15.0],
            dimensions: vec![0.065, 0.08, 0.091, 0.106],
            ..Default::default()
        };
        let deltas = t.dimension_deltas();
        assert_eq!(deltas.len(), 3);
        assert_eq!(deltas[0].from_station, 0.0);
        assert_eq!(deltas[0].to_station, 5.0);
        assert_eq!(deltas[0].station, 2.5);
        assert!((deltas[0].delta - 0.015).abs() < 1e-12);
        assert!((deltas[1].delta - 0.011).abs() < 1e-12);
        assert!((deltas[2].delta - 0.015).abs() < 1e-12);
    }

    #[test]
    fn scaled_applies_multiplier_and_bias_leaving_stations_alone() {
        let t = Taper {
            stations: vec![0.0, 5.0, 10.0],
            dimensions: vec![0.1, 0.2, 0.3],
            ..Default::default()
        };
        let scaled = t.scaled(2.0, 0.01);
        assert_eq!(scaled.stations, t.stations);
        assert!((scaled.dimensions[0] - 0.21).abs() < 1e-12);
        assert!((scaled.dimensions[1] - 0.41).abs() < 1e-12);
        assert!((scaled.dimensions[2] - 0.61).abs() < 1e-12);

        // A large negative bias clamps at zero rather than going negative.
        let clamped = t.scaled(1.0, -1.0);
        assert!(clamped.dimensions.iter().all(|&d| d == 0.0));
    }

    #[test]
    fn insert_station_interpolates_and_stays_sorted() {
        let mut t = Taper {
            stations: vec![0.0, 10.0, 20.0],
            dimensions: vec![0.1, 0.3, 0.5],
            ..Default::default()
        };
        assert!(t.insert_station(5.0));
        assert_eq!(t.stations, vec![0.0, 5.0, 10.0, 20.0]);
        assert!((t.dimensions[1] - 0.2).abs() < 1e-12);

        // Inserting at an existing station is a no-op.
        assert!(!t.insert_station(10.0));
        assert_eq!(t.stations.len(), 4);
    }

    #[test]
    fn planing_form_depths_uses_per_geometry_formula() {
        let base = Taper {
            stations: vec![0.0],
            dimensions: vec![0.2],
            const_type: Some("Hex".to_string()),
            ..Default::default()
        };

        let hex = base.planing_form_depths();
        assert_eq!(hex.len(), 1);
        assert!((hex[0].depth - 0.1).abs() < 1e-9);

        let quad = Taper {
            const_type: Some("Quad".to_string()),
            ..base.clone()
        };
        assert!((quad.planing_form_depths()[0].depth - 0.1 * std::f64::consts::SQRT_2).abs() < 1e-9);

        let penta = Taper {
            const_type: Some("Penta".to_string()),
            ..base.clone()
        };
        assert!((penta.planing_form_depths()[0].depth - 0.2 / 1.809753).abs() < 1e-9);

        // "Two Piece Quad" carries the piece count in the const_type string;
        // it should still be recognised as Quad geometry (contains "quad").
        let two_piece_quad = Taper {
            const_type: Some("Two Piece Quad".to_string()),
            ..base.clone()
        };
        assert!(
            (two_piece_quad.planing_form_depths()[0].depth - 0.1 * std::f64::consts::SQRT_2).abs()
                < 1e-9
        );

        // Unsupported geometries — RodDNA itself refuses these too.
        let rect = Taper {
            const_type: Some("Rectangular".to_string()),
            ..base.clone()
        };
        assert!(rect.planing_form_depths().is_empty());
        let unknown = Taper {
            const_type: None,
            ..base.clone()
        };
        assert!(unknown.planing_form_depths().is_empty());
    }

    #[test]
    fn planing_form_depths_applies_station_bias_and_multiplier() {
        let t = Taper {
            stations: vec![0.0],
            dimensions: vec![0.2],
            const_type: Some("Hex".to_string()),
            station_bias: Some(0.01),
            station_multiplier: Some(2.0),
            ..Default::default()
        };
        // 0.2/2 + 0.01*2.0 == 0.12
        assert!((t.planing_form_depths()[0].depth - 0.12).abs() < 1e-9);
    }

    #[test]
    fn guide_spacing_matches_hand_computed_span_for_constant_dimension() {
        // A constant-dimension rod has constant stiffness, so the beam
        // formula gives the same span at every step; hand-computed with
        // params defaults (E=4e6 psi, max_sag=0.1") and density=0.668:
        // r=0.1, I=pi/4*r^4, area=0.2^2*0.866, w=area*density,
        // span=(384*E*I*0.1/(5*w))^0.25 ~= 17.9696".
        let t = Taper {
            stations: vec![0.0, 200.0],
            dimensions: vec![0.2, 0.2],
            action_length: Some(90.0),
            bamboo_density: Some(0.668),
            ..Default::default()
        };
        let placements = t.guide_spacing(&GuideSpacingParams::default());
        assert_eq!(placements[0].station, 0.0);
        assert_eq!(placements[0].span_from_previous, 0.0);
        assert!(placements.len() > 2);
        // All but the last span should match the hand-computed value; the
        // last is clipped to whatever's left before action_length.
        for p in &placements[1..placements.len() - 1] {
            assert!((p.span_from_previous - 17.96963214235976).abs() < 1e-6);
        }
        assert!(placements.last().unwrap().station <= 90.0);
    }

    #[test]
    fn guide_spacing_grows_toward_the_stiffer_butt() {
        let t = Taper {
            stations: vec![0.0, 30.0, 60.0, 90.0],
            dimensions: vec![0.08, 0.15, 0.22, 0.3],
            action_length: Some(90.0),
            bamboo_density: Some(0.668),
            ..Default::default()
        };
        let placements = t.guide_spacing(&GuideSpacingParams::default());
        let spans: Vec<f64> = placements[1..]
            .iter()
            .map(|p| p.span_from_previous)
            .collect();
        assert!(spans.len() > 2, "expected more than two guide spans");
        // Excludes the last span: it's clipped to whatever distance remains
        // before action_length, which can be shorter than the natural span.
        for w in spans[..spans.len() - 1].windows(2) {
            assert!(
                w[1] + 1e-9 >= w[0],
                "spans should be non-decreasing toward the butt: {spans:?}"
            );
        }
        assert!(placements.iter().all(|p| p.station <= 90.0 + 1e-9));
    }

    #[test]
    fn guide_spacing_returns_empty_for_a_taper_with_no_profile() {
        let t = Taper::default();
        assert!(t.guide_spacing(&GuideSpacingParams::default()).is_empty());
    }

    #[test]
    fn to_csv_includes_provenance_and_station_rows() {
        let t = Taper {
            name: Some("Test Rod".to_string()),
            rod_type: Some("Fly-Rod".to_string()),
            const_type: Some("Hex".to_string()),
            length: Some(90.0),
            line_weight: Some(5.0),
            pieces: Some(2.0),
            stations: vec![0.0, 5.0],
            dimensions: vec![0.065, 0.08],
            provenance: Some(Provenance {
                source: Some("Test Source".to_string()),
                author: Some("Test Author".to_string()),
                ..Default::default()
            }),
            ..Default::default()
        };
        let csv = t.to_csv();
        assert!(csv.starts_with("# caneDNA taper export"));
        assert!(csv.contains("# Name: Test Rod"));
        assert!(csv.contains("Type: Fly-Rod | Construction: Hex | Length: 90\" | Line: 5 | Pieces: 2"));
        assert!(csv.contains("# Source: Test Source (Test Author)"));
        assert!(csv.contains("Station (in),Dimension (in)"));
        assert!(csv.contains("0.00,0.0650"));
        assert!(csv.contains("5.00,0.0800"));
    }

    #[test]
    fn to_station_file_uses_tab_separated_rows() {
        let t = Taper {
            name: Some("Test Rod".to_string()),
            stations: vec![0.0, 5.0],
            dimensions: vec![0.065, 0.08],
            ..Default::default()
        };
        let file = t.to_station_file();
        assert!(file.contains("# Name: Test Rod"));
        assert!(file.contains("0.00\t0.0650"));
        assert!(file.contains("5.00\t0.0800"));
        // No provenance block when the taper has none.
        assert!(!file.contains("# Source:"));
    }

    #[test]
    fn stress_curve_matches_stored_stresses_within_tolerance() {
        // Records whose stored `stresses` array doesn't line up 1:1 with
        // `dimensions`/`stations` (bad source data, not a formula problem).
        let known_bad = ["Hardy Special", "wright & McGill Granger TR's-short tip"];

        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/tapers.json");
        let text = std::fs::read_to_string(path).expect("read tapers.json");
        let lib = Library::from_json(&text).expect("parse library");

        let mut errs: Vec<f64> = Vec::new();
        let mut n_records = 0;
        for m in &lib.models {
            if m.stresses.is_empty() || m.stresses.len() != m.dimensions.len() {
                continue;
            }
            if known_bad.contains(&m.name.as_deref().unwrap_or_default()) {
                continue;
            }
            let curve = m.stress_curve();
            assert_eq!(
                curve.len(),
                m.stresses.len(),
                "{:?}: stress_curve length mismatch",
                m.name
            );
            n_records += 1;
            for (&[_, computed], &known) in curve.iter().zip(m.stresses.iter()) {
                if known == 0.0 {
                    continue;
                }
                errs.push((computed - known).abs() / known);
            }
        }
        assert_eq!(n_records, 58, "expected 58 clean stress-bearing records");

        errs.sort_by(|a, b| a.partial_cmp(b).unwrap());
        let median = errs[errs.len() / 2];
        let p90 = errs[errs.len() * 9 / 10];
        // A faithful reconstruction of RodDNA's own (undocumented) casting
        // model, not a bit-exact reproduction — see `stress_curve`'s doc
        // comment. Generous bounds guard against regressions while
        // tolerating the known long tail on a few fine-tip records.
        assert!(median < 0.15, "median relative error too high: {median}");
        assert!(p90 < 0.5, "p90 relative error too high: {p90}");
    }

    #[test]
    fn ferrules_reports_size_type_and_hex_apex_od() {
        let t = Taper {
            stations: vec![40.0, 45.0],
            dimensions: vec![0.20, 0.22],
            ferrule1_loc: Some(42.0),
            ferrule1_size: Some("13/64".to_string()),
            ferrule_type: Some("NS-(Standard)".to_string()),
            const_type: Some("Hex".to_string()),
            ..Default::default()
        };
        let ferrules = t.ferrules();
        assert_eq!(ferrules.len(), 1);
        let f = &ferrules[0];
        assert_eq!(f.index, 1);
        assert_eq!(f.size, "13/64");
        assert_eq!(f.ferrule_type.as_deref(), Some("NS-(Standard)"));
        assert_eq!(f.location, 42.0);
        assert!((f.dimension_at_location - 0.208).abs() < 1e-9);
        let od = f.outside_diameter_apexes.expect("hex apex OD present");
        assert!((od - 0.208 * 2.0 / 3.0_f64.sqrt()).abs() < 1e-9);

        let quad = Taper {
            const_type: Some("Quad".to_string()),
            ..t
        };
        assert!(quad.ferrules()[0].outside_diameter_apexes.is_none());
    }

    #[test]
    fn ferrules_skips_unset_placeholder_slots() {
        // Unused ferrule slots in the source data are `0.0`/`"None"`, not null.
        let t = Taper {
            stations: vec![0.0, 5.0],
            dimensions: vec![0.07, 0.08],
            ferrule1_loc: Some(0.0),
            ferrule1_size: Some("None".to_string()),
            ..Default::default()
        };
        assert!(t.ferrules().is_empty());
    }

    #[test]
    fn polygon_section_matches_known_formulas() {
        // Square: A = s², I = s⁴/12 (flat-to-flat == side length).
        let (a, i) = polygon_section(2.0, 4.0);
        assert!((a - 4.0).abs() < 1e-9, "square area {a}");
        assert!((i - 16.0 / 12.0).abs() < 1e-9, "square I {i}");
        // Hex: A = (√3/2)·w².
        let (a, _) = polygon_section(3.0, 6.0);
        assert!((a - (3.0_f64.sqrt() / 2.0) * 9.0).abs() < 1e-9, "hex area {a}");
    }

    /// A prismatic (constant-dimension) cantilever must reproduce the
    /// closed-form Euler–Bernoulli fundamental frequency
    /// `f = (βL)² / (2π) · sqrt(EI / (m·L⁴))`, since the Rayleigh quotient with
    /// the exact uniform-beam mode shape is exact for a uniform beam. This is
    /// the validation anchor for A2 (no stored frequency ground truth exists).
    #[test]
    fn modal_frequency_matches_prismatic_beam() {
        let length = 90.0_f64;
        let dim = 0.25;
        let stations: Vec<f64> = (0..=18).map(|i| i as f64 * 5.0).collect();
        let dims = vec![dim; stations.len()];
        let t = Taper {
            const_type: Some("Hex".into()),
            stations,
            dimensions: dims,
            // No tip mass — keep it a pure prismatic beam for the check.
            tip_weight: Some(0.0),
            ..Default::default()
        };
        let params = ModalParams {
            youngs_modulus_psi: 2.4e6,
            specific_weight_oz_in3: Some(0.668),
        };
        let m = t.modal_analysis(&params).expect("modal result");

        // Analytic reference.
        let (area, inertia) = polygon_section(dim, 6.0);
        let mass_per_len = (0.668 / OZ_PER_LB) / GRAVITY_IN_S2 * area;
        let omega = FIRST_MODE_BETA_L.powi(2)
            * (2.4e6 * inertia / (mass_per_len * length.powi(4))).sqrt();
        let f_analytic = omega / (2.0 * std::f64::consts::PI);
        let rel = (m.frequency_hz - f_analytic).abs() / f_analytic;
        assert!(rel < 0.005, "f={} analytic={} rel={}", m.frequency_hz, f_analytic, rel);
        assert!(m.period_ms > 0.0 && m.effective_stiffness_lb_in > 0.0);
    }

    #[test]
    fn casting_deflection_is_physical_and_scales_with_modulus() {
        // Use a real RodDNA-sourced rod that carries the physics inputs.
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/tapers.json");
        let text = std::fs::read_to_string(path).expect("read tapers.json");
        let lib = Library::from_json(&text).expect("parse library");
        let rod = lib
            .models
            .iter()
            .find(|t| !t.stress_curve().is_empty())
            .expect("a rod with physics inputs");

        let params = DeflectionParams {
            youngs_modulus_psi: 2.4e6,
            impact_factor: 1.0,
        };
        let d = rod.casting_deflection(&params);
        assert!(d.len() >= 2, "deflection has stations");

        // Stations come back tip-last (largest station last), matching profile.
        let tip = &d[0];
        let butt = d.last().unwrap();
        assert!(tip.station < butt.station);
        // Clamp end barely bends; the tip swings the most.
        assert!(butt.angle_deg.abs() < tip.angle_deg.abs());
        assert!(butt.vertical_in.abs() < tip.vertical_in.abs());
        assert!(tip.vertical_in > 0.0, "tip deflects under load");

        // Curvature transcription: κ·E·I == moment/16 (oz -> lbf) at each
        // station, using the true polygon second moment of area.
        let (_f, sides) = geometry_factor(rod.const_type.as_deref());
        let profile = rod.profile();
        let action_length = profile.last().unwrap()[0].round() as usize;
        for s in &d {
            // Engine evaluates I on its 1"-resolution grid; replicate the
            // same integer-inch index used internally.
            let idx = if s.station <= 0.0 {
                0
            } else {
                (s.station.round() as usize).saturating_sub(1).min(action_length - 1)
            };
            let dim = interpolate(&profile, idx as f64).unwrap();
            let (_a, inertia) = polygon_section(dim, sides);
            let lhs = s.curvature_per_in * params.youngs_modulus_psi * inertia;
            assert!(
                (lhs - s.moment_oz_in / OZ_PER_LB).abs() < 1e-6,
                "curvature relation at station {}",
                s.station
            );
        }

        // A stiffer rod (2× modulus) bends less: smaller tip deflection.
        let stiff = rod.casting_deflection(&DeflectionParams {
            youngs_modulus_psi: params.youngs_modulus_psi * 2.0,
            impact_factor: 1.0,
        });
        assert!(stiff[0].vertical_in < tip.vertical_in, "stiffer rod deflects less");

        // A harder cast (higher load multiplier) bends more.
        let hard = rod.casting_deflection(&DeflectionParams {
            impact_factor: 1.5,
            ..params.clone()
        });
        assert!(hard[0].vertical_in > tip.vertical_in, "harder cast deflects more");
    }

    #[test]
    fn depadded_trims_padding_but_keeps_real_flats() {
        // Taper that grows then is padded with a repeated max value: the
        // padding is dropped, keeping the first point of the flat run.
        let padded = Taper {
            stations: vec![0.0, 5.0, 10.0, 15.0, 20.0],
            dimensions: vec![0.10, 0.15, 0.20, 0.20, 0.20],
            ..Default::default()
        };
        let p = padded.depadded();
        assert_eq!(p.dimensions, vec![0.10, 0.15, 0.20]);
        assert_eq!(p.stations, vec![0.0, 5.0, 10.0]);

        // A genuinely uniform "rod" never grew into the flat, so nothing is
        // stripped — otherwise the prismatic-beam modal check would collapse.
        let uniform = Taper {
            stations: vec![0.0, 5.0, 10.0, 15.0],
            dimensions: vec![0.20, 0.20, 0.20, 0.20],
            ..Default::default()
        };
        assert_eq!(uniform.depadded().dimensions.len(), 4);

        // A strictly increasing taper (no trailing flat) is untouched.
        let growing = Taper {
            stations: vec![0.0, 5.0, 10.0],
            dimensions: vec![0.10, 0.15, 0.20],
            ..Default::default()
        };
        assert_eq!(growing.depadded().dimensions.len(), 3);

        // Trailing zeros are still dropped.
        let zeros = Taper {
            stations: vec![0.0, 5.0, 10.0, 15.0],
            dimensions: vec![0.10, 0.15, 0.20, 0.0],
            ..Default::default()
        };
        assert_eq!(zeros.depadded().dimensions, vec![0.10, 0.15, 0.20]);
    }

    #[test]
    fn solve_to_stress_flattens_the_curve() {
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../data/tapers.json");
        let text = std::fs::read_to_string(path).expect("read tapers.json");
        let lib = Library::from_json(&text).expect("parse library");
        let seed = lib
            .models
            .iter()
            .find(|t| !t.stress_curve().is_empty())
            .expect("a rod with physics inputs");

        let target = 180_000.0;
        let solved = seed
            .solve_to_stress(target, &SolveParams::default())
            .expect("solvable");

        // Metadata preserved; only dimensions changed.
        assert_eq!(solved.stations, seed.stations);
        assert_eq!(solved.const_type, seed.const_type);
        assert_eq!(solved.dimensions.len(), seed.dimensions.len());

        // The solved curve is flat at the target. Skip the tip-most station:
        // a tiny absolute dimension there cubes into a large relative stress
        // error (the same numerical-sensitivity artifact stress_curve notes).
        let curve = solved.stress_curve();
        assert!(curve.len() > 3);
        for &[station, stress] in curve.iter().skip(2) {
            let rel = (stress - target).abs() / target;
            assert!(rel < 0.02, "station {station}: stress {stress} vs {target} (rel {rel})");
        }

        // Monotonic tip→butt.
        let dims = &solved.dimensions[..solved.profile().len()];
        for w in dims.windows(2) {
            assert!(w[1] >= w[0] - 1e-9, "non-monotonic: {} -> {}", w[0], w[1]);
        }

        // A higher target psi means the rod works harder, so it needs *less*
        // material: every station is thinner than the lower-target solution.
        let softer = seed.solve_to_stress(target * 1.5, &SolveParams::default()).unwrap();
        let a: f64 = solved.dimensions.iter().sum();
        let b: f64 = softer.dimensions.iter().sum();
        assert!(b < a, "higher target psi should give a thinner taper");
    }

    #[test]
    fn solve_to_stress_rejects_bad_inputs() {
        let seed = Taper {
            const_type: Some("Hex".into()),
            stations: vec![0.0, 30.0, 60.0],
            dimensions: vec![0.06, 0.12, 0.18],
            ..Default::default()
        };
        // No physics inputs -> cannot solve.
        assert!(seed.solve_to_stress(180_000.0, &SolveParams::default()).is_none());
    }

    /// A stiffer/heavier-butt taper (real trout rod) should ring faster than a
    /// limp uniform stick of the same length: a monotonicity sanity check that
    /// the taper actually drives the frequency.
    #[test]
    fn modal_ranks_stiffer_rod_higher() {
        let stations: Vec<f64> = (0..=16).map(|i| i as f64 * 5.0).collect();
        let uniform = Taper {
            const_type: Some("Hex".into()),
            stations: stations.clone(),
            dimensions: vec![0.15; stations.len()],
            tip_weight: Some(0.0),
            ..Default::default()
        };
        // Tapered: fine tip growing to a stout butt.
        let tapered_dims: Vec<f64> = stations
            .iter()
            .map(|&s| 0.07 + 0.16 * (s / 80.0))
            .collect();
        let tapered = Taper {
            dimensions: tapered_dims,
            ..uniform.clone()
        };
        let p = ModalParams::default();
        let fu = uniform.modal_analysis(&p).unwrap().frequency_hz;
        let ft = tapered.modal_analysis(&p).unwrap().frequency_hz;
        assert!(ft > fu, "tapered {ft} should exceed uniform {fu}");
    }
}
