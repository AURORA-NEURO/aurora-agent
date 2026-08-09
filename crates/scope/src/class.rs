//! Classification of scope dimensions.
//!
//! Blueprint 43.03 requires validity contexts to be a *composable typed base*, not optional
//! metadata: `B ⊆ I × R × S × T × C × O × P`. A dimension whose class is unknown is reported
//! as [`ScopeClass::Unclassified`] rather than silently treated as an opaque string, because
//! protected closure (43.13) is defined per class and an unclassified dimension cannot be
//! proven to be closed over.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScopeClass {
    Identity,
    Region,
    Specimen,
    Time,
    Coordinate,
    Ontology,
    Policy,
    Unclassified,
}

impl ScopeClass {
    pub const CANONICAL: [ScopeClass; 7] = [
        ScopeClass::Identity,
        ScopeClass::Region,
        ScopeClass::Specimen,
        ScopeClass::Time,
        ScopeClass::Coordinate,
        ScopeClass::Ontology,
        ScopeClass::Policy,
    ];

    pub fn is_classified(self) -> bool {
        self != ScopeClass::Unclassified
    }

    pub fn as_str(self) -> &'static str {
        match self {
            ScopeClass::Identity => "identity",
            ScopeClass::Region => "region",
            ScopeClass::Specimen => "specimen",
            ScopeClass::Time => "time",
            ScopeClass::Coordinate => "coordinate",
            ScopeClass::Ontology => "ontology",
            ScopeClass::Policy => "policy",
            ScopeClass::Unclassified => "unclassified",
        }
    }
}

/// Maps a dimension name to its class.
///
/// The default table covers the neuro-oncology vocabulary the OncoWorld packs use. A world
/// may extend it; it may not reclassify a canonical name, because protected closure rules are
/// written against the class and silent reclassification would move evidence out of closure.
#[derive(Debug, Clone)]
pub struct DimensionRegistry {
    entries: std::collections::BTreeMap<String, ScopeClass>,
}

impl Default for DimensionRegistry {
    fn default() -> Self {
        let mut entries = std::collections::BTreeMap::new();
        for (name, class) in DEFAULT_DIMENSIONS {
            entries.insert((*name).to_string(), *class);
        }
        DimensionRegistry { entries }
    }
}

impl DimensionRegistry {
    pub fn classify(&self, name: &str) -> ScopeClass {
        self.entries
            .get(name)
            .copied()
            .unwrap_or(ScopeClass::Unclassified)
    }

    pub fn register(&mut self, name: impl Into<String>, class: ScopeClass) -> Result<(), String> {
        let name = name.into();
        if let Some(existing) = self.entries.get(&name) {
            if *existing != class && is_default_dimension(&name) {
                return Err(format!(
                    "cannot reclassify canonical dimension {name:?} from {} to {}",
                    existing.as_str(),
                    class.as_str()
                ));
            }
        }
        self.entries.insert(name, class);
        Ok(())
    }

    pub fn unclassified<'a, I: IntoIterator<Item = &'a str>>(&self, names: I) -> Vec<String> {
        names
            .into_iter()
            .filter(|n| !self.classify(n).is_classified())
            .map(|n| n.to_string())
            .collect()
    }
}

fn is_default_dimension(name: &str) -> bool {
    DEFAULT_DIMENSIONS.iter().any(|(n, _)| *n == name)
}

const DEFAULT_DIMENSIONS: &[(&str, ScopeClass)] = &[
    ("cohort", ScopeClass::Identity),
    ("subject", ScopeClass::Identity),
    ("patient", ScopeClass::Identity),
    ("case", ScopeClass::Identity),
    ("site", ScopeClass::Identity),
    ("scanner", ScopeClass::Identity),
    ("study", ScopeClass::Identity),
    ("lesion", ScopeClass::Region),
    ("region", ScopeClass::Region),
    ("roi", ScopeClass::Region),
    ("slice", ScopeClass::Region),
    ("specimen", ScopeClass::Specimen),
    ("aliquot", ScopeClass::Specimen),
    ("block", ScopeClass::Specimen),
    ("sample", ScopeClass::Specimen),
    ("assay", ScopeClass::Specimen),
    ("time", ScopeClass::Time),
    ("timepoint", ScopeClass::Time),
    ("valid_time", ScopeClass::Time),
    ("record_time", ScopeClass::Time),
    ("release_time", ScopeClass::Time),
    ("decision_time", ScopeClass::Time),
    ("frame", ScopeClass::Coordinate),
    ("coordinate_frame", ScopeClass::Coordinate),
    ("orientation", ScopeClass::Coordinate),
    ("space", ScopeClass::Coordinate),
    ("units", ScopeClass::Coordinate),
    ("genome_build", ScopeClass::Coordinate),
    ("ontology", ScopeClass::Ontology),
    ("ontology_version", ScopeClass::Ontology),
    ("classifier_version", ScopeClass::Ontology),
    ("pipeline_version", ScopeClass::Ontology),
    ("schema_version", ScopeClass::Ontology),
    ("policy", ScopeClass::Policy),
    ("consent", ScopeClass::Policy),
    ("purpose", ScopeClass::Policy),
    ("residency", ScopeClass::Policy),
    ("role", ScopeClass::Policy),
    ("visibility", ScopeClass::Policy),
];
