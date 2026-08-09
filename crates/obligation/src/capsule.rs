//! The BioContext capsule.
//!
//! Blueprint 39.03 defines the portable, role-specific input to a biological inference component:
//! graph-addressed, content-addressed, and *intentionally smaller than the underlying world*. This
//! module assembles the pieces the rest of the crate produces — the obligation frontier, the
//! budget plan, the omission ledger and the sufficiency certificate — into that object, and
//! implements the validation clause, which is the part with teeth.
//!
//! A capsule is invalid when required identity, time or version fields are absent, when the
//! omission ledger lacks a reason for a dropped mandatory candidate, or when its permissions
//! exceed the role contract. [`BioContextCapsule::validate`] covers the first two directly and
//! the third partially: it checks that the sufficiency certificate is scoped to *this* decision
//! and *this* role, because a certificate borrowed from a broader role is the cheapest way to
//! make a narrow projection look adequate.
//!
//! The temporal check is the other one worth naming. A visibility cutoff later than the valid
//! time would let the capsule carry evidence released after the decision was made, which is the
//! leakage 39.08's closure rules exist to prevent.
//!
//! Rendering reuses [`bioprism_section::Layer`] rather than defining a parallel ladder, and keeps
//! that module's invariant: the sufficiency status and the omission count appear at *every* layer,
//! so a consumer that stops reading at L0 still knows what it does not have.
//!
//! Not implemented here: the delta protocol. 39.03 allows a capsule to carry only additions,
//! replacements and retractions against a signed base; [`BioContextCapsule::base_capsule_ref`]
//! records the base, but reconstruction by hash belongs to 39.10's common-ground cache.

use crate::budget::BudgetPlan;
use crate::error::CapsuleError;
use crate::graph::FrontierEntry;
use crate::invariants::ClosureReport;
use crate::ledger::{OmissionLedger, SufficiencyCertificate};
use bioprism_ids::ContentHash;
use bioprism_scope::Timestamp;
use bioprism_section::Layer;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

pub const CAPSULE_SCHEMA_VERSION: &str = "biocontext-capsule/0.1";

/// The role-specific projection handed to one agent for one decision.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BioContextCapsule {
    pub world_ref: String,
    pub decision_ref: String,
    pub role_ref: String,
    /// The signed base this capsule is a delta against, when it is one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base_capsule_ref: Option<String>,
    pub valid_at: Timestamp,
    /// Nothing released after this instant may appear in the capsule.
    pub visibility_cutoff: Timestamp,
    pub objective: String,
    pub output_contract: String,
    pub obligation_frontier: Vec<FrontierEntry>,
    pub closure: ClosureReport,
    pub budget: BudgetPlan,
    pub omission_ledger: OmissionLedger,
    pub sufficiency: SufficiencyCertificate,
    pub compiler_version: String,
}

impl BioContextCapsule {
    /// Applies the validation clause of 39.03.
    pub fn validate(&self) -> Result<(), CapsuleError> {
        for (name, value) in [
            ("world_ref", &self.world_ref),
            ("decision_ref", &self.decision_ref),
            ("role_ref", &self.role_ref),
            ("objective", &self.objective),
            ("output_contract", &self.output_contract),
            ("compiler_version", &self.compiler_version),
        ] {
            if value.trim().is_empty() {
                return Err(CapsuleError::MissingField(name));
            }
        }

        if self.visibility_cutoff > self.valid_at {
            return Err(CapsuleError::CutAfterValidTime {
                cutoff: self.visibility_cutoff.to_rfc3339(),
                valid_at: self.valid_at.to_rfc3339(),
            });
        }

        self.omission_ledger.validate()?;

        if self.sufficiency.decision != self.decision_ref || self.sufficiency.role != self.role_ref
        {
            return Err(CapsuleError::CertificateScopeMismatch {
                decision: self.sufficiency.decision.clone(),
                role: self.sufficiency.role.clone(),
                capsule_decision: self.decision_ref.clone(),
                capsule_role: self.role_ref.clone(),
            });
        }

        Ok(())
    }

    /// The capsule body, without its own identifier.
    pub fn to_json(&self) -> Value {
        let mut map = Map::new();
        map.insert("schema_version".into(), json!(CAPSULE_SCHEMA_VERSION));
        let body = serde_json::to_value(self).expect("capsule is serialisable");
        if let Value::Object(fields) = body {
            for (key, value) in fields {
                map.insert(key, value);
            }
        }
        Value::Object(map)
    }

    /// `bctx:<sha256>` over the canonical body, per 39.03's `capsule_id` field.
    pub fn capsule_id(&self) -> Result<String, CapsuleError> {
        let hash = ContentHash::of_value(&self.to_json())?;
        Ok(format!("bctx:{}", hash.as_str()))
    }

    /// Renders one layer of the L0–L4 ladder (39.04).
    ///
    /// L0 carries identity, the decision contract and the sufficiency verdict; L1 adds the
    /// obligation frontier; L2 adds the omission ledger rows and the budget allocation; L3 adds
    /// the closure report and the retrieval expansion actions; L4 is the whole capsule.
    ///
    /// Evidence values never appear at any layer here. A capsule addresses evidence through the
    /// graph; the raw artifacts stay behind the tool mediation of 39.12.
    pub fn render(&self, layer: Layer) -> Value {
        let mut map = Map::new();
        map.insert("layer".into(), json!(layer.as_str()));
        map.insert("schema_version".into(), json!(CAPSULE_SCHEMA_VERSION));
        map.insert("world_ref".into(), json!(self.world_ref));
        map.insert("decision_ref".into(), json!(self.decision_ref));
        map.insert("role_ref".into(), json!(self.role_ref));
        map.insert("valid_at".into(), json!(self.valid_at.to_rfc3339()));
        map.insert(
            "visibility_cutoff".into(),
            json!(self.visibility_cutoff.to_rfc3339()),
        );
        map.insert("objective".into(), json!(self.objective));
        map.insert("output_contract".into(), json!(self.output_contract));
        map.insert(
            "sufficiency".into(),
            json!({
                "status": self.sufficiency.status.as_str(),
                "decision": self.sufficiency.decision,
                "role": self.sufficiency.role,
                "unmet_obligations": self.sufficiency.unmet.len(),
                "omitted_candidates": self.omission_ledger.len(),
                "protected_closure_satisfied": self.closure.is_satisfied(),
            }),
        );

        if layer >= Layer::L1 {
            map.insert(
                "obligation_frontier".into(),
                serde_json::to_value(&self.obligation_frontier).expect("frontier is serialisable"),
            );
            map.insert(
                "unmet_obligations".into(),
                serde_json::to_value(&self.sufficiency.unmet).expect("unmet is serialisable"),
            );
        }

        if layer >= Layer::L2 {
            map.insert(
                "omission_ledger".into(),
                serde_json::to_value(&self.omission_ledger).expect("ledger is serialisable"),
            );
            map.insert(
                "budget".into(),
                serde_json::to_value(&self.budget).expect("budget plan is serialisable"),
            );
            map.insert(
                "tokens_withheld".into(),
                json!(self.omission_ledger.estimated_tokens_withheld().label()),
            );
        }

        if layer >= Layer::L3 {
            map.insert(
                "protected_closure".into(),
                serde_json::to_value(&self.closure).expect("closure report is serialisable"),
            );
            map.insert(
                "retrieval_actions".into(),
                json!(self.omission_ledger.retrieval_actions()),
            );
            map.insert("reasons".into(), json!(self.sufficiency.reasons));
        }

        if layer >= Layer::L4 {
            map.insert("capsule".into(), self.to_json());
        }

        if let Some(next) = layer.next() {
            map.insert(
                "refine".into(),
                json!({
                    "next_layer": next.as_str(),
                    "adds": next.purpose(),
                }),
            );
        }

        Value::Object(map)
    }
}
