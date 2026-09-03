//! The Decision Section: the model-facing context ABI.
//!
//! Blueprint 43.25 makes this the stable unit delivered to a model, an agent or an evaluator —
//! "not an unstructured bundle of retrieved chunks". Evidence is grouped by decision obligation,
//! conflicts and unresolved obligations appear *before* any narrative rendering, and omissions
//! are never hidden.
//!
//! Evidence values are echoed from the world's original documents rather than rebuilt from typed
//! fields, so the delivered bytes and their hash are exactly reproducible.

use crate::verdict::OracleVerdict;
use bioprism_ids::{to_canonical_string, CanonicalError, ContentHash};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

pub const SECTION_SCHEMA_VERSION: &str = "fiber-decision-section/0.1";

/// One typed evidence item with its scope and provenance.
///
/// 43.25 invariant: *every* evidence item has scope and provenance. Neither field is optional,
/// because an unscoped claim cannot be checked for validity and an unsourced one cannot be
/// replayed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceCapsule {
    pub id: String,
    pub provides: String,
    pub value: Value,
    pub scope: Value,
    pub tags: Vec<String>,
    pub provenance: Vec<String>,
}

impl EvidenceCapsule {
    /// Projects a raw world fact into the capsule shape.
    ///
    /// Absent `tags` and `provenance` become empty arrays rather than being dropped, matching
    /// the reference runtime and keeping the field set uniform across capsules.
    pub fn from_raw_fact(raw: &Value) -> Self {
        let get_list = |key: &str| -> Vec<String> {
            raw.get(key)
                .and_then(Value::as_array)
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|v| v.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default()
        };
        EvidenceCapsule {
            id: raw
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            provides: raw
                .get("provides")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
            value: raw.get("value").cloned().unwrap_or(Value::Null),
            scope: raw.get("scope").cloned().unwrap_or_else(|| json!({})),
            tags: get_list("tags"),
            provenance: get_list("provenance"),
        }
    }

    fn to_json(&self) -> Value {
        let mut map = Map::new();
        map.insert("id".into(), json!(self.id));
        map.insert("provides".into(), json!(self.provides));
        map.insert("value".into(), self.value.clone());
        map.insert("scope".into(), self.scope.clone());
        map.insert("tags".into(), json!(self.tags));
        map.insert("provenance".into(), json!(self.provenance));
        Value::Object(map)
    }
}

/// A decision obligation the compiler could not discharge.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum UnresolvedObligation {
    /// A fact the slice required, but which the temporal cut forbids reading.
    InaccessibleAtCut { fact_id: String },
    /// Evidence disagreed and no gluing was possible.
    Obstructed { detail: String },
    /// Policy blocked access to evidence the decision needs.
    PolicyBlocked { detail: String },
}

/// A move that would tighten the decision if taken.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RefinementOption {
    pub action: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub facts: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DecisionSection {
    pub world_id: String,
    pub query_id: String,
    pub decision_time: String,
    pub goal: String,
    pub selected_evidence: Vec<EvidenceCapsule>,
    pub selected_factors: Vec<Value>,
    pub oracle: OracleVerdict,
    pub unresolved_obligations: Vec<UnresolvedObligation>,
    pub refinement_frontier: Vec<RefinementOption>,
}

impl DecisionSection {
    /// Serialises to the `fiber-decision-section/0.1` wire form.
    pub fn to_json(&self) -> Value {
        let mut map = Map::new();
        map.insert("schema_version".into(), json!(SECTION_SCHEMA_VERSION));
        map.insert("world_id".into(), json!(self.world_id));
        map.insert("query_id".into(), json!(self.query_id));
        map.insert("decision_time".into(), json!(self.decision_time));
        map.insert("goal".into(), json!(self.goal));
        map.insert(
            "selected_evidence".into(),
            Value::Array(
                self.selected_evidence
                    .iter()
                    .map(EvidenceCapsule::to_json)
                    .collect(),
            ),
        );
        map.insert(
            "selected_factors".into(),
            Value::Array(self.selected_factors.clone()),
        );
        map.insert(
            "oracle".into(),
            serde_json::to_value(&self.oracle).expect("oracle verdict is serialisable"),
        );
        map.insert(
            "unresolved_obligations".into(),
            serde_json::to_value(&self.unresolved_obligations)
                .expect("obligations are serialisable"),
        );
        map.insert(
            "refinement_frontier".into(),
            serde_json::to_value(&self.refinement_frontier).expect("frontier is serialisable"),
        );
        Value::Object(map)
    }

    pub fn content_hash(&self) -> Result<ContentHash, CanonicalError> {
        ContentHash::of_value(&self.to_json())
    }

    pub fn to_canonical_string(&self) -> Result<String, CanonicalError> {
        to_canonical_string(&self.to_json())
    }

    /// True when the compiler could not discharge every obligation, meaning the consumer must
    /// refine or abstain rather than answer.
    pub fn requires_refinement(&self) -> bool {
        !self.unresolved_obligations.is_empty()
    }

    pub fn evidence_ids(&self) -> Vec<&str> {
        self.selected_evidence
            .iter()
            .map(|e| e.id.as_str())
            .collect()
    }
}
