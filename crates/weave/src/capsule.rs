//! Context Capsules.
//!
//! Blueprint 23.09 (recipient projection) and 23.30 (selective disclosure). A capsule is what one
//! participant is allowed to see, projected for its role — not a copy of the shared context.
//!
//! This is where Weave and FIBER meet, and it is the reason the projection can be trusted. A
//! capsule is built from a compiled Decision Section, so it inherits the certificate: the recipient
//! learns what was withheld *from the world* by the compiler, and separately what was withheld
//! *from them* by the projection. Withholding is never silent in either direction — which is the
//! difference between selective disclosure and simply not telling someone something.

use bioprism_ids::ContentHash;
use bioprism_section::{DecisionSection, Layer, RenderContext};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;

/// A clearance label carried by evidence and held by participants.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Label(pub String);

impl Label {
    pub fn new(text: impl Into<String>) -> Self {
        Label(text.into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Recipient {
    pub participant: String,
    pub role: String,
    /// Labels this recipient may see. Evidence tagged with a label outside this set is withheld.
    pub clearance: BTreeSet<String>,
    /// Deepest layer this recipient may descend to.
    pub max_layer: Layer,
}

impl Recipient {
    pub fn new(participant: impl Into<String>, role: impl Into<String>) -> Self {
        Recipient {
            participant: participant.into(),
            role: role.into(),
            clearance: BTreeSet::new(),
            max_layer: Layer::L2,
        }
    }

    pub fn cleared_for(mut self, label: impl Into<String>) -> Self {
        self.clearance.insert(label.into());
        self
    }

    pub fn up_to(mut self, layer: Layer) -> Self {
        self.max_layer = layer;
        self
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WithheldItem {
    pub id: String,
    pub required_labels: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextCapsule {
    pub recipient: String,
    pub role: String,
    pub layer: Layer,
    /// The projected section.
    pub content: Value,
    /// Evidence removed by the projection, with the labels that would have been needed.
    ///
    /// Populated even when empty so a recipient can tell "nothing was withheld" from "withholding
    /// was not tracked".
    pub withheld: Vec<WithheldItem>,
    /// Whether the compiler's own omissions still support a sufficiency claim.
    pub upstream_supports_sufficiency: bool,
    pub capsule_sha256: String,
}

impl ContextCapsule {
    /// Projects a Decision Section for one recipient.
    ///
    /// Two independent reductions apply: the recipient's layer ceiling, and their clearance. Both
    /// are reported.
    pub fn project(
        section: &DecisionSection,
        context: &RenderContext,
        recipient: &Recipient,
        requested: Layer,
    ) -> ContextCapsule {
        let layer = requested.min(recipient.max_layer);
        let mut content = section.render(layer, context);

        let mut withheld = Vec::new();
        for capsule in &section.selected_evidence {
            let required: Vec<String> = capsule
                .tags
                .iter()
                .filter(|tag| !recipient.clearance.contains(tag.as_str()))
                .cloned()
                .collect();
            if !required.is_empty() {
                withheld.push(WithheldItem {
                    id: capsule.id.clone(),
                    required_labels: required,
                });
            }
        }

        let blocked: BTreeSet<&str> = withheld.iter().map(|item| item.id.as_str()).collect();
        if let Some(map) = content.as_object_mut() {
            for field in ["evidence", "evidence_inventory", "provenance"] {
                if let Some(Value::Array(items)) = map.get_mut(field) {
                    items.retain(|item| {
                        item.get("id")
                            .and_then(Value::as_str)
                            .map(|id| !blocked.contains(id))
                            .unwrap_or(true)
                    });
                }
            }
            map.insert(
                "withheld_by_projection".into(),
                json!({
                    "count": withheld.len(),
                    "items": withheld,
                }),
            );
        }

        let digest = ContentHash::of_value(&content)
            .map(|hash| hash.as_str().to_string())
            .unwrap_or_default();

        ContextCapsule {
            recipient: recipient.participant.clone(),
            role: recipient.role.clone(),
            layer,
            content,
            withheld,
            upstream_supports_sufficiency: context.supports_sufficiency_claim,
            capsule_sha256: digest,
        }
    }

    /// Whether this recipient saw every selected item.
    pub fn is_complete(&self) -> bool {
        self.withheld.is_empty()
    }

    /// Whether the recipient can make a sufficiency claim from what it received.
    ///
    /// False whenever the projection withheld anything, regardless of the compiler's own verdict:
    /// a participant reasoning from a filtered capsule cannot vouch for completeness it did not
    /// observe.
    pub fn supports_sufficiency_claim(&self) -> bool {
        self.upstream_supports_sufficiency && self.is_complete()
    }
}
