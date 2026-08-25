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
}
