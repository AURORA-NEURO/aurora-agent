//! Federated continual laboratory-integration research copilot (`AFA-prism-P11-F12`).
//!
//! This A4 boundary admits a single preflighted instrument action only when every signed safety,
//! policy, provenance, replay, budget, and locality gate closes. It emits a bounded declared-tool
//! invocation receipt; a separate instrument gateway performs the actual physical action. No
//! human-subject or clinical workflow is supported.

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-prism-P11-F12";
pub const CONTRACT_VERSION: &str =
    "prism-federated-continual-laboratory-integration-research-copilot/1.0";
pub const INPUT_SCHEMA: &str = "InstrumentActionRequest4@1";
pub const OUTPUT_SCHEMA: &str = "InstrumentActionReceipt3@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.prism-instrument-action-receipt-3+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentActionRequest4 {
    pub schema_version: String,
    pub request_id: String,
    pub consumer: String,
    pub purpose: String,
    pub instrument_id: String,
    pub action_id: String,
    pub declared_tool: String,
    pub scope: String,
    pub semantic_profile: String,
    pub protocol_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub signed_preflight: bool,
    pub interlock_ok: bool,
    pub emergency_stop_ready: bool,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub federation_approved: bool,
    pub budget_units: u64,
    pub max_budget_units: u64,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub negative_result_context: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentActionArtifact3 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentActionReceipt3 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub consumer: String,
    pub purpose: String,
    pub instrument_id: String,
    pub action_id: String,
    pub declared_tool: String,
    pub scope: String,
    pub semantic_profile: String,
    pub disposition: String,
    pub gate_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub action_digest: ContentHash,
    pub artifact: InstrumentActionArtifact3,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum LaboratoryIntegrationError {
    #[error("invalid laboratory integration request or receipt: {0}")]
    Invalid(String),
    #[error("laboratory integration artifact failed: {0}")]
    Artifact(String),
}

fn ordered(values: &[String]) -> bool {
    values.windows(2).all(|w| w[0] < w[1])
}
fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().bytes().all(|b| b.is_ascii_hexdigit())
}

pub fn laboratory_integration_copilot_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "prism".into(), consumers: ["context compiler engineer".into(), "instrument gateway steward".into(), "preclinical lab operator".into()].into(), behavior: "admit a federated continual instrument action only after signed A4 preflight, interlock, emergency-stop, provenance, replay, policy, federation, budget, and locality gates".into(), value: "turns a research intent into a bounded, auditable declared-tool invocation without hiding unsafe physical-action conditions".into(), inputs: vec![TypedPort { name: "instrument_action_request".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "instrument_action_receipt".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["invoke:declared-tools".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "cwl".into(), state: EvidenceState::Supported, locator: Some("https://www.commonwl.org/specification/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A4, surfaces: [ResearchSurface::Ui, ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

impl InstrumentActionReceipt3 {
    pub fn validate(&self) -> Result<(), LaboratoryIntegrationError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || !matches!(self.disposition.as_str(), "qualified" | "blocked")
            || self.gate_order.is_empty()
            || self.effect_receipts.is_empty()
            || [
                &self.request_id,
                &self.consumer,
                &self.purpose,
                &self.instrument_id,
                &self.action_id,
                &self.declared_tool,
                &self.scope,
                &self.semantic_profile,
            ]
            .iter()
            .any(|v| v.trim().is_empty())
        {
            return Err(LaboratoryIntegrationError::Invalid(
                "instrument action identity, gates, locality, or effects are incomplete".into(),
            ));
        }
        for values in [
            &self.gate_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !ordered(values) {
                return Err(LaboratoryIntegrationError::Invalid(
                    "instrument action ordering is not canonical".into(),
                ));
            }
        }
        if !digest(&self.replay_identity)
            || !digest(&self.action_digest)
            || self.artifact.content_hash != self.action_digest
            || self.artifact.content_type != CONTENT_TYPE
            || !self.artifact.provenance_digests.iter().all(digest)
        {
            return Err(LaboratoryIntegrationError::Artifact(
                "instrument action digest is inconsistent".into(),
            ));
        }
        if self.disposition == "qualified"
            && self.effect_receipts != [format!("invoke:declared-tool:{}", self.declared_tool)]
        {
            return Err(LaboratoryIntegrationError::Invalid(
                "qualified instrument invocation effect is invalid".into(),
            ));
        }
        if self.disposition == "blocked" && self.effect_receipts != ["block:unsafe-release"] {
            return Err(LaboratoryIntegrationError::Invalid(
                "blocked instrument action must block".into(),
            ));
        }
        Ok(())
    }
}

pub fn admit_laboratory_integration_action(
    request: &InstrumentActionRequest4,
) -> Result<InstrumentActionReceipt3, LaboratoryIntegrationError> {
    if request.schema_version != INPUT_SCHEMA
        || request.request_id.trim().is_empty()
        || request.consumer.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.instrument_id.trim().is_empty()
        || request.action_id.trim().is_empty()
        || request.declared_tool.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.budget_units == 0
        || request.max_budget_units == 0
        || !digest(&request.protocol_digest)
        || !digest(&request.provenance_digest)
        || !digest(&request.replay_identity)
        || !request.raw_data_local
        || !request.aggregate_only
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(LaboratoryIntegrationError::Invalid(
            "instrument action identity, bounds, digests, locality, or boundary is invalid".into(),
        ));
    }
    let mut gates: BTreeSet<String> = BTreeSet::new();
    let mut omissions: BTreeSet<String> = BTreeSet::new();
    let mut uncertainty: BTreeSet<String> = BTreeSet::new();
    let mut negative: BTreeSet<String> = BTreeSet::new();
    gates.insert("preflight:signed".into());
    gates.insert("preflight:interlock".into());
    gates.insert("preflight:emergency-stop".into());
    gates.insert("preflight:policy".into());
    gates.insert("preflight:provenance".into());
    gates.insert("preflight:replay".into());
    gates.insert("preflight:budget".into());
    gates.insert("preflight:federation".into());
    if request.negative_result_context {
        negative.insert("context:negative-result-preserved".into());
    }
    if !request.signed_preflight {
        omissions.insert("gate:signed-preflight-missing".into());
    }
    if !request.interlock_ok {
        omissions.insert("gate:interlock-failed".into());
    }
    if !request.emergency_stop_ready {
        omissions.insert("gate:emergency-stop-unavailable".into());
    }
    if !request.policy_allow {
        omissions.insert("gate:policy-denied".into());
    }
    if !request.protected_closure {
        omissions.insert("gate:protected-closure-incomplete".into());
    }
    if !request.federation_approved {
        omissions.insert("gate:federation-approval-missing".into());
    }
    if request.budget_units > request.max_budget_units {
        omissions.insert("gate:budget-exceeded".into());
    }
    if !request.signed_preflight
        || !request.interlock_ok
        || !request.emergency_stop_ready
        || !request.policy_allow
        || !request.protected_closure
        || !request.federation_approved
        || request.budget_units > request.max_budget_units
    {
        uncertainty.insert("action:not-safe-to-dispatch".into());
    }
    let disposition = if omissions.is_empty() {
        "qualified"
    } else {
        "blocked"
    };
    let checkpoint = ContentHash::of_value(&json!({"request_id":request.request_id,"instrument_id":request.instrument_id,"action_id":request.action_id,"protocol_digest":request.protocol_digest,"replay_identity":request.replay_identity})).map_err(|e| LaboratoryIntegrationError::Artifact(e.to_string()))?;
    let payload = json!({"gate_order":gates,"omission_order":omissions,"uncertainty_order":uncertainty,"negative_evidence_order":negative,"checkpoint":checkpoint,"replay_identity":request.replay_identity});
    let action_digest = ContentHash::of_value(&payload)
        .map_err(|e| LaboratoryIntegrationError::Artifact(e.to_string()))?;
    let strings = |k: &str| {
        payload[k]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    };
    let receipt = InstrumentActionReceipt3 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        consumer: request.consumer.clone(),
        purpose: request.purpose.clone(),
        instrument_id: request.instrument_id.clone(),
        action_id: request.action_id.clone(),
        declared_tool: request.declared_tool.clone(),
        scope: request.scope.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition: disposition.into(),
        gate_order: strings("gate_order"),
        omission_order: strings("omission_order"),
        uncertainty_order: strings("uncertainty_order"),
        negative_evidence_order: strings("negative_evidence_order"),
        replay_identity: request.replay_identity.clone(),
        action_digest: action_digest.clone(),
        artifact: InstrumentActionArtifact3 {
            artifact_id: format!("prism-instrument-action:{}", request.action_id),
            content_type: CONTENT_TYPE.into(),
            content_hash: action_digest,
            semantic_loss: if disposition == "qualified" {
                Vec::new()
            } else {
                vec!["physical-action-not-dispatched".into()]
            },
            provenance_digests: vec![
                request.provenance_digest.clone(),
                request.protocol_digest.clone(),
            ],
            boundary: PRECLINICAL_BOUNDARY.into(),
        },
        effect_receipts: if disposition == "qualified" {
            vec![format!("invoke:declared-tool:{}", request.declared_tool)]
        } else {
            vec!["block:unsafe-release".into()]
        },
        raw_data_local: true,
        aggregate_only: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn h(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn request() -> InstrumentActionRequest4 {
        InstrumentActionRequest4 {
            schema_version: INPUT_SCHEMA.into(),
            request_id: "action-1".into(),
            consumer: "compiler".into(),
            purpose: "organoid imaging".into(),
            instrument_id: "microscope-1".into(),
            action_id: "capture-1".into(),
            declared_tool: "instrument-gateway".into(),
            scope: "organoid".into(),
            semantic_profile: "instrument:v1".into(),
            protocol_digest: h("protocol"),
            provenance_digest: h("prov"),
            replay_identity: h("replay"),
            signed_preflight: true,
            interlock_ok: true,
            emergency_stop_ready: true,
            policy_allow: true,
            protected_closure: true,
            federation_approved: true,
            budget_units: 1,
            max_budget_units: 2,
            raw_data_local: true,
            aggregate_only: true,
            negative_result_context: false,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a4() {
        assert_eq!(
            laboratory_integration_copilot_manifest().autonomy_tier,
            AutonomyTier::A4
        )
    }
    #[test]
    fn qualified_invocation() {
        let r = admit_laboratory_integration_action(&request()).unwrap();
        assert_eq!(r.disposition, "qualified");
        assert_eq!(
            r.effect_receipts,
            vec!["invoke:declared-tool:instrument-gateway"]
        )
    }
    #[test]
    fn interlock_blocks() {
        let mut q = request();
        q.interlock_ok = false;
        assert_eq!(
            admit_laboratory_integration_action(&q).unwrap().disposition,
            "blocked"
        )
    }
    #[test]
    fn budget_blocks() {
        let mut q = request();
        q.budget_units = 3;
        assert_eq!(
            admit_laboratory_integration_action(&q).unwrap().disposition,
            "blocked"
        )
    }
}
