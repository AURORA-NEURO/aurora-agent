//! Progressive rendering.
//!
//! Blueprint 43.25 makes the Decision Section *layered*, `L0 ⊆ L1 ⊆ L2 ⊆ L3 ⊆ L4`, and 43.27 asks
//! for small ordered attention zones with stable refinement handles. The reason is practical: an
//! agent should be able to see the contract and the obligation inventory before deciding whether
//! it needs the evidence values, and should never be handed raw artifacts it did not ask for.
//!
//! The invariant that makes this safe rather than merely smaller: **omissions are visible at every
//! layer**. L0 already reports how much was excluded and whether the sufficiency claim holds, so an
//! agent that stops reading at L0 still knows what it does not have. Layering hides volume, never
//! the fact of an omission.

use crate::section::DecisionSection;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Layer {
    /// Identity and decision contract: who asked what, of which world, at what cut, and whether
    /// the answer is sufficient. No evidence.
    L0,
    /// Inventory: obligations, and the *names* of the evidence held — ids, variables and tags,
    /// without values. Enough to decide whether to ask for more.
    L1,
    /// Decision evidence: the values the verdict actually rests on.
    L2,
    /// Computed views: the selected factors, provenance, conflicts and the refinement frontier.
    L3,
    /// Governed raw artifacts: the original documents, byte for byte.
    L4,
}

impl Layer {
    pub const ALL: [Layer; 5] = [Layer::L0, Layer::L1, Layer::L2, Layer::L3, Layer::L4];

    pub fn as_str(self) -> &'static str {
        match self {
            Layer::L0 => "l0",
            Layer::L1 => "l1",
            Layer::L2 => "l2",
            Layer::L3 => "l3",
            Layer::L4 => "l4",
        }
    }

    pub fn parse(text: &str) -> Option<Layer> {
        match text.to_ascii_lowercase().as_str() {
            "l0" | "0" => Some(Layer::L0),
            "l1" | "1" => Some(Layer::L1),
            "l2" | "2" => Some(Layer::L2),
            "l3" | "3" => Some(Layer::L3),
            "l4" | "4" => Some(Layer::L4),
            _ => None,
        }
    }

    pub fn next(self) -> Option<Layer> {
        match self {
            Layer::L0 => Some(Layer::L1),
            Layer::L1 => Some(Layer::L2),
            Layer::L2 => Some(Layer::L3),
            Layer::L3 => Some(Layer::L4),
            Layer::L4 => None,
        }
    }

    pub fn purpose(self) -> &'static str {
        match self {
            Layer::L0 => "identity and decision contract",
            Layer::L1 => "obligation and evidence inventory, without values",
            Layer::L2 => "the evidence the verdict rests on",
            Layer::L3 => "selected factors, provenance and the refinement frontier",
            Layer::L4 => "governed raw artifacts",
        }
    }
}

/// Context that only the compiler knows, threaded through so L0 can report sufficiency without
/// the section having to depend on the certificate type.
#[derive(Debug, Clone, Default)]
pub struct RenderContext {
    pub omitted_facts: usize,
    pub total_facts: usize,
    pub supports_sufficiency_claim: bool,
    pub protected_closure_satisfied: bool,
    pub certificate_sha256: Option<String>,
}

impl DecisionSection {
    /// Renders one layer.
    ///
    /// Layers are cumulative in *meaning*, not in bytes: each response carries the header and the
    /// omission summary so it can be read standalone, and adds the fields its level introduces.
    pub fn render(&self, layer: Layer, context: &RenderContext) -> Value {
        let mut map = Map::new();
        map.insert("layer".into(), json!(layer.as_str()));
        map.insert("purpose".into(), json!(layer.purpose()));
        map.insert("world_id".into(), json!(self.world_id));
        map.insert("query_id".into(), json!(self.query_id));
        map.insert("decision_time".into(), json!(self.decision_time));
        map.insert("goal".into(), json!(self.goal));

        map.insert(
            "verdict".into(),
            json!({
                "oracle": self.oracle.oracle_kind,
                "status": self.oracle.status.as_str(),
                "witnesses": self.oracle.witness_kinds(),
            }),
        );

        // Present at every layer by design: an agent that stops here still knows what it lacks.
        map.insert(
            "omissions".into(),
            json!({
                "omitted_facts": context.omitted_facts,
                "total_facts": context.total_facts,
                "supports_sufficiency_claim": context.supports_sufficiency_claim,
                "protected_closure_satisfied": context.protected_closure_satisfied,
                "unresolved_obligations": self.unresolved_obligations.len(),
            }),
        );
        if let Some(digest) = &context.certificate_sha256 {
            map.insert("certificate_sha256".into(), json!(digest));
        }

        if layer >= Layer::L1 {
            map.insert(
                "obligations".into(),
                serde_json::to_value(&self.unresolved_obligations).expect("serialisable"),
            );
            map.insert(
                "evidence_inventory".into(),
                Value::Array(
                    self.selected_evidence
                        .iter()
                        .map(|capsule| {
                            json!({
                                "id": capsule.id,
                                "provides": capsule.provides,
                                "tags": capsule.tags,
                                "scope": capsule.scope,
                            })
                        })
                        .collect(),
                ),
            );
        }

        if layer >= Layer::L2 {
            map.insert(
                "evidence".into(),
                Value::Array(
                    self.selected_evidence
                        .iter()
                        .map(|capsule| {
                            json!({
                                "id": capsule.id,
                                "provides": capsule.provides,
                                "value": capsule.value,
                            })
                        })
                        .collect(),
                ),
            );
            map.insert(
                "witnesses".into(),
                serde_json::to_value(&self.oracle.witnesses).expect("serialisable"),
            );
        }

        if layer >= Layer::L3 {
            map.insert("factors".into(), Value::Array(self.selected_factors.clone()));
            map.insert(
                "provenance".into(),
                Value::Array(
                    self.selected_evidence
                        .iter()
                        .map(|capsule| json!({ "id": capsule.id, "provenance": capsule.provenance }))
                        .collect(),
                ),
            );
            map.insert(
                "refinement_frontier".into(),
                serde_json::to_value(&self.refinement_frontier).expect("serialisable"),
            );
        }

        if layer >= Layer::L4 {
            map.insert("raw_section".into(), self.to_json());
        }

        if let Some(next) = layer.next() {
            map.insert(
                "refine".into(),
                json!({
                    "next_layer": next.as_str(),
                    "adds": next.purpose(),
                    "handle": format!("{}#{}", self.query_id, next.as_str()),
                }),
            );
        }

        Value::Object(map)
    }

    /// Rough token cost of a layer.
    ///
    /// A four-characters-per-token heuristic, not a tokeniser. 39.16 wants a budget controller
    /// backed by real tokenizer adapters; until one exists this is labelled an estimate everywhere
    /// it is reported, so nobody mistakes it for a measurement.
    pub fn estimated_tokens(&self, layer: Layer, context: &RenderContext) -> usize {
        let rendered = self.render(layer, context);
        serde_json::to_string(&rendered).map(|s| s.len()).unwrap_or(0) / 4
    }
}
