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
    /// count. Each internal boundary overlaps its neighbor by one profile
    /// point, since a builder needs a station of reference on both sides of
    /// the ferrule joint.
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
            let outside_diameter_apexes = if self.const_type.as_deref() == Some("Hex") {
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
