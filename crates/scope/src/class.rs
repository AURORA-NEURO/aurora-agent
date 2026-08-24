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

    /// Parses a canonical class name.
    ///
    /// `"unclassified"` is deliberately not parseable: it is the *absence* of a classification,
    /// and a document that declared a dimension unclassified would be asserting the very state
    /// the registry already reports for every name it has never seen.
    pub fn parse(name: &str) -> Option<ScopeClass> {
        ScopeClass::CANONICAL
            .into_iter()
            .find(|class| class.as_str() == name)
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

    /// Extends this registry from a dimension-classification document.
    ///
    /// The document is `{"schema_version": "bioprism-scope-dimensions/0.1", "dimensions":
    /// {name: class}}`, where every class is one of the seven canonical names. This is the
    /// data-driven half of blueprint 43.03: the default table covers the neuro-oncology
    /// vocabulary, and a domain with a different vocabulary declares its own dimensions rather
    /// than accepting an `unclassified_scope_dimension` diagnostic on every fact. The
    /// reclassification rule of [`DimensionRegistry::register`] still applies, so a document
    /// cannot silently move a canonical dimension out of protected closure.
    ///
    /// Returns how many dimensions were registered.
    pub fn extend_from_json(&mut self, document: &serde_json::Value) -> Result<usize, String> {
        let map = document
            .as_object()
            .ok_or_else(|| "dimension document is not an object".to_string())?;

        match map.get("schema_version").and_then(serde_json::Value::as_str) {
            Some(DIMENSIONS_SCHEMA_VERSION) => {}
            Some(other) => {
                return Err(format!(
                    "unsupported dimension document schema {other:?}; expected {DIMENSIONS_SCHEMA_VERSION:?}"
                ))
            }
            None => return Err("dimension document declares no schema_version".to_string()),
        }
        if let Some(unknown) = map
            .keys()
            .find(|key| !matches!(key.as_str(), "schema_version" | "dimensions"))
        {
            return Err(format!("undeclared key {unknown:?} in dimension document"));
        }

        let dimensions = map
            .get("dimensions")
            .and_then(serde_json::Value::as_object)
            .ok_or_else(|| "dimension document carries no \"dimensions\" object".to_string())?;

        let mut registered = 0;
        for (name, class_name) in dimensions {
            let class_name = class_name.as_str().ok_or_else(|| {
                format!("dimension {name:?} maps to a non-string class")
            })?;
            let class = ScopeClass::parse(class_name).ok_or_else(|| {
                format!(
                    "dimension {name:?} names unknown class {class_name:?}; the canonical classes are {}",
                    ScopeClass::CANONICAL.map(ScopeClass::as_str).join(", ")
                )
            })?;
            self.register(name.clone(), class)?;
            registered += 1;
        }
        Ok(registered)
    }

    /// The default table extended by a dimension-classification document.
    pub fn from_json(document: &serde_json::Value) -> Result<DimensionRegistry, String> {
        let mut registry = DimensionRegistry::default();
        registry.extend_from_json(document)?;
        Ok(registry)
    }
}

/// The wire version of the dimension-classification document.
pub const DIMENSIONS_SCHEMA_VERSION: &str = "bioprism-scope-dimensions/0.1";

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
