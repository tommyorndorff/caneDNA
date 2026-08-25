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
    /// 0.07", 0.03"). `half_dimension` (dimension / 2) is the mill dial
    /// setting; `total_increase` is the cumulative rise from the butt end to
    /// each station, i.e. the anvil setting.
    pub fn mill_settings(&self, rough_allowance: f64, finish_allowance: f64) -> Vec<MillSetting> {
        settings_for_points(&self.profile(), rough_allowance, finish_allowance)
    }

    /// Per-piece Morgan Hand Mill sections (Tip / Mid n / Butt), split at
    /// ferrule locations when the record has them, else evenly by piece
    /// count. Each internal boundary shares the two profile points bracketing
    /// the ferrule with its neighbor, since a builder needs a station of
    /// reference on both sides of the ferrule joint.
    pub fn mill_sections(&self, rough_allowance: f64, finish_allowance: f64) -> Vec<MillSection> {
        let profile = self.profile();
        let pieces = self.pieces.unwrap_or(1.0).round().max(1.0) as usize;
        if pieces <= 1 || profile.len() < pieces {
            return vec![MillSection {
                label: "Full rod".into(),
                approximate: false,
                settings: settings_for_points(&profile, rough_allowance, finish_allowance),
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
                        rough_allowance,
                        finish_allowance,
                    ),
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
        let profile = self.profile();
        if profile.len() < 2 {
            return Vec::new();
        }
        let (Some(line_weight), Some(line_length), Some(line_cast)) =
            (self.line_weight, self.line_length, self.line_cast)
        else {
            return Vec::new();
        };
        let (Some(tip_impact_factor), Some(bamboo_density), Some(tip_weight)) =
            (self.tip_impact_factor, self.bamboo_density, self.tip_weight)
        else {
            return Vec::new();
        };
        let line_weight_idx = line_weight.round() as usize;
        if line_weight_idx < 1 || line_weight_idx > LINE_WEIGHTS_GRAINS.len() {
            return Vec::new();
        }
        let (geometry_factor, sides) = geometry_factor(self.const_type.as_deref());
        // RodDNA's per-geometry constant applies to the across-corners
        // diameter, not the flat-to-flat width this crate stores.
        let apex_conversion = 1.0 / (std::f64::consts::PI / sides).cos();

        let action_length = profile.last().unwrap()[0].round() as usize;
        if action_length < 1 {
            return Vec::new();
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

        let stress_per_inch: Vec<f64> = (0..action_length)
            .map(|i| {
                let total_moment = tip_moments[i]
                    + line_moments[i]
                    + vg_moments[i]
                    + ferrule_moments[i]
                    + bamboo_moments[i];
                let apex_dim = dim[i] * apex_conversion;
                total_moment / (apex_dim.powi(3) * geometry_factor)
            })
            .collect();

        profile
            .iter()
            .map(|&[station, _]| {
                let idx = if station <= 0.0 {
                    0
                } else {
                    (station.round() as usize)
                        .saturating_sub(1)
                        .min(action_length - 1)
                };
                [station, stress_per_inch[idx]]
            })
            .collect()
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

/// Fixed finish+enamel allowance from the source spreadsheet, never user-adjustable.
const MHM_ENAMEL_ALLOWANCE: f64 = 0.003;

fn settings_for_points(
    points: &[[f64; 2]],
    rough_allowance: f64,
    finish_allowance: f64,
) -> Vec<MillSetting> {
    let n = points.len();
    let butt_half = points.last().map(|p| p[1] / 2.0).unwrap_or(0.0);
    points
        .iter()
        .enumerate()
        .map(|(i, &[station, dimension])| {
            let half_dimension = dimension / 2.0;
            MillSetting {
                station,
                anvil_number: n - 1 - i,
                dimension,
                half_dimension,
                rough_oversize: half_dimension + rough_allowance,
                finish_oversize: half_dimension + finish_allowance,
                finish_enamel: half_dimension + MHM_ENAMEL_ALLOWANCE,
                total_increase: butt_half - half_dimension,
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
    pub half_dimension: f64,
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
        assert!((first.half_dimension - 0.083).abs() < 1e-9);
        assert!((first.rough_oversize - 0.153).abs() < 1e-9);
        assert!((first.finish_oversize - 0.113).abs() < 1e-9);
        assert!((first.finish_enamel - 0.086).abs() < 1e-9);
        assert!((first.total_increase - 0.084).abs() < 1e-9);

        let last = settings.last().unwrap();
        assert_eq!(last.anvil_number, 0);
        assert!((last.half_dimension - 0.167).abs() < 1e-9);
        assert!((last.total_increase - 0.0).abs() < 1e-9);
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
}
