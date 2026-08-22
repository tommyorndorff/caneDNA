//! Core data model for RodDNA tapers.
//!
//! Deserializes `data/tapers.json` (produced by `scripts/convert_tapers.py`
//! from the original RodDNA v2.0 XML libraries) into typed Rust structs.

use serde::{Deserialize, Serialize};

/// A single rod model / taper.
///
/// Fields mirror the original RodDNA XML schema. Most numeric fields are
/// optional because a handful of records leave them blank.
#[derive(Debug, Clone, Serialize, Deserialize)]
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

    #[serde(rename = "_source", default)]
    pub source: Option<String>,
}

impl Taper {
    /// Number of taper stations that actually have a dimension.
    pub fn point_count(&self) -> usize {
        self.dimensions.len()
    }

    /// (station, dimension) pairs, zipped and truncated to the shorter of the two.
    pub fn profile(&self) -> Vec<[f64; 2]> {
        self.stations
            .iter()
            .zip(self.dimensions.iter())
            .map(|(&s, &d)| [s, d])
            .collect()
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
    }
}
