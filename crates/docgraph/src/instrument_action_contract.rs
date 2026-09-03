//! Typed laboratory-instrument action contract model.
//!
//! Atlas feature: `AFA-docgraph-P11-F08`.
//!
//! This schema/serializer boundary gives extension developers a stable, content-addressed
//! representation of prospective instrument actions. It validates shape, compatibility, evidence,
//! and policy metadata without contacting hardware or dispatching a physical action.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, EvidenceReference,
    EvidenceState, LossSeverity, ProvenanceLink, ResearchSurface, SemanticLoss, TypedPort,
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-docgraph-P11-F08";
pub const CONTRACT_VERSION: &str =
    "docgraph-federated-continual-laboratory-integration-contract-model/1.0";
pub const INPUT_SCHEMA: &str = "InstrumentActionRequest4@1";
pub const OUTPUT_SCHEMA: &str = "InstrumentActionReceipt2@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.docgraph-instrument-action-receipt-2+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentAction4 {
    pub action_id: String,
    pub endpoint_id: String,
    pub capability_id: String,
    pub study_id: String,
    pub modality: String,
    pub protocol_version: String,
    pub parameters_digest: ContentHash,
    pub artifact_digest: Option<ContentHash>,
    pub provenance_digest: Option<ContentHash>,
    pub replay_identity: ContentHash,
    pub evidence_state: EvidenceState,
    pub scope_compatible: bool,
    pub permitted: bool,
    pub omissions: Vec<String>,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentActionRequest4 {
    pub request_id: String,
    pub requester: String,
    pub purpose: String,
    pub scope: String,
    pub semantic_profile: String,
    pub schema_version: String,
    pub required_action_order: Vec<String>,
    pub required_endpoint_order: Vec<String>,
    pub required_capability_order: Vec<String>,
    pub actions: Vec<InstrumentAction4>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub endpoint_allow: bool,
    pub raw_data_local: bool,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InstrumentActionReceipt2 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub requester: String,
    pub purpose: String,
    pub scope: String,
    pub semantic_profile: String,
    pub disposition: String,
    pub action_order: Vec<String>,
    pub selected_action_order: Vec<String>,
    pub unresolved_action_order: Vec<String>,
    pub blocked_action_order: Vec<String>,
    pub missing_action_order: Vec<String>,
    pub endpoint_order: Vec<String>,
    pub capability_order: Vec<String>,
    pub selected_endpoint_order: Vec<String>,
    pub selected_capability_order: Vec<String>,
    pub missing_endpoint_order: Vec<String>,
    pub missing_capability_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub action_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum InstrumentActionContractError {
    #[error("invalid instrument-action contract: {0}")]
    Invalid(String),
    #[error("instrument-action artifact failed: {0}")]
    Artifact(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl InstrumentActionReceipt2 {
    pub fn validate(&self) -> Result<(), InstrumentActionContractError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.request_id.trim().is_empty()
            || self.requester.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.action_order.is_empty()
            || self.endpoint_order.is_empty()
            || self.capability_order.is_empty()
            || self.effect_receipts.is_empty()
            || !self.raw_data_local
            || self.boundary != PRECLINICAL_BOUNDARY
        {
            return Err(InstrumentActionContractError::Invalid(
                "contract identity, axes, locality, boundary, or effects are incomplete".into(),
            ));
        }
        for values in [
            &self.action_order,
            &self.selected_action_order,
            &self.unresolved_action_order,
            &self.blocked_action_order,
            &self.missing_action_order,
            &self.endpoint_order,
            &self.capability_order,
            &self.selected_endpoint_order,
            &self.selected_capability_order,
            &self.missing_endpoint_order,
            &self.missing_capability_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(InstrumentActionContractError::Invalid(
                    "instrument-action ordering is not canonical".into(),
                ));
            }
        }
        let partition = self
            .selected_action_order
            .iter()
            .chain(self.unresolved_action_order.iter())
            .chain(self.blocked_action_order.iter())
            .chain(self.missing_action_order.iter())
            .cloned()
            .collect::<Vec<_>>();
        if partition.len() != self.action_order.len()
            || partition.iter().collect::<BTreeSet<_>>().len() != partition.len()
            || partition.iter().collect::<BTreeSet<_>>()
                != self.action_order.iter().collect::<BTreeSet<_>>()
        {
            return Err(InstrumentActionContractError::Invalid(
                "action states do not partition the contract".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("validate:typed-instrument-action:")
                && effect != "block:unsafe-release"
        }) {
            return Err(InstrumentActionContractError::Invalid(
                "effect is outside typed contract validation gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| InstrumentActionContractError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, InstrumentActionContractError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|error| InstrumentActionContractError::Artifact(error.to_string()))?,
        )
        .map_err(|error| InstrumentActionContractError::Artifact(error.to_string()))
    }
}

pub fn capability_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "docgraph".into(), consumers: BTreeSet::from(["AURORA extension developer".into(), "instrument integration architect".into(), "schema compatibility operator".into()]), behavior: "serializes and validates typed prospective instrument actions without contacting hardware or dispatching physical effects".into(), value: "gives extensions a stable compatibility boundary that preserves evidence, omission, provenance, replay, and policy decisions".into(), inputs: vec![TypedPort { name: "instrument_action_request".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "instrument_action_receipt".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: BTreeSet::new(), permissions: BTreeSet::from(["read:local-research-artifacts".into()]), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "mcp-2025-06-18".into(), state: EvidenceState::Supported, locator: Some("https://modelcontextprotocol.io/specification/2025-06-18/basic/index".into()) }, EvidenceReference { source_id: "ro-crate-1.3".into(), state: EvidenceState::Supported, locator: Some("https://www.researchobject.org/ro-crate/specification.html".into()) }], authority_requirements: vec![AuthorityRequirement { role: "schema compatibility reviewer".into(), reason: "typed contract migrations require explicit review".into() }], autonomy_tier: AutonomyTier::A1, surfaces: BTreeSet::from([ResearchSurface::Protocol, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Operator]), boundary: PRECLINICAL_BOUNDARY.into() }
}

fn validate_request(
    request: &InstrumentActionRequest4,
) -> Result<(), InstrumentActionContractError> {
    if request.schema_version != INPUT_SCHEMA
        || request.request_id.trim().is_empty()
        || request.requester.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_action_order.is_empty()
        || request.required_endpoint_order.is_empty()
        || request.required_capability_order.is_empty()
        || request.actions.is_empty()
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(InstrumentActionContractError::Invalid(
            "request identity, closure, locality, boundary, or schema is invalid".into(),
        ));
    }
    let ids = request
        .actions
        .iter()
        .map(|action| action.action_id.clone())
        .collect::<Vec<_>>();
    if ids.iter().any(|id| id.trim().is_empty())
        || ids.iter().collect::<BTreeSet<_>>().len() != ids.len()
    {
        return Err(InstrumentActionContractError::Invalid(
            "action identifiers must be present and unique".into(),
        ));
    }
    Ok(())
}

pub fn validate_instrument_actions(
    request: &InstrumentActionRequest4,
) -> Result<InstrumentActionReceipt2, InstrumentActionContractError> {
    validate_request(request)?;
    let mut actions = request.actions.clone();
    actions.sort_by(|left, right| {
        left.endpoint_id
            .cmp(&right.endpoint_id)
            .then(left.capability_id.cmp(&right.capability_id))
            .then(left.action_id.cmp(&right.action_id))
    });
    let action_order = actions
        .iter()
        .map(|action| action.action_id.clone())
        .collect::<Vec<_>>();
    let mut selected = Vec::new();
    let mut unresolved = Vec::new();
    let mut blocked = Vec::new();
    let mut missing = Vec::new();
    let mut omission = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for action in &actions {
        if action.artifact_digest.is_none() || action.provenance_digest.is_none() {
            missing.push(action.action_id.clone());
            omission.insert(format!(
                "{}:artifact-or-provenance-missing",
                action.action_id
            ));
        } else if !action.scope_compatible || !action.permitted {
            blocked.push(action.action_id.clone());
            omission.insert(format!("{}:scope-or-permission-denied", action.action_id));
        } else if action.evidence_state == EvidenceState::Contradicted {
            blocked.push(action.action_id.clone());
            uncertainty.insert(format!("{}:contradicted", action.action_id));
        } else if matches!(
            action.evidence_state,
            EvidenceState::Unknown | EvidenceState::Speculative
        ) || action.replay_identity != request.replay_identity
        {
            unresolved.push(action.action_id.clone());
            uncertainty.insert(format!("{}:unknown-or-replay-mismatch", action.action_id));
        } else {
            selected.push(action.action_id.clone());
            if action.negative_result {
                negative.insert(format!("{}:negative-result", action.action_id));
            }
            omission.extend(
                action
                    .omissions
                    .iter()
                    .map(|entry| format!("{}:{entry}", action.action_id)),
            );
        }
    }
    let endpoint_order = actions
        .iter()
        .map(|action| action.endpoint_id.clone())
        .chain(request.required_endpoint_order.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let capability_order = actions
        .iter()
        .map(|action| action.capability_id.clone())
        .chain(request.required_capability_order.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let present_endpoints = actions
        .iter()
        .map(|action| action.endpoint_id.clone())
        .collect::<BTreeSet<_>>();
    let present_capabilities = actions
        .iter()
        .map(|action| action.capability_id.clone())
        .collect::<BTreeSet<_>>();
    let missing_endpoint_order = request
        .required_endpoint_order
        .iter()
        .filter(|id| !present_endpoints.contains(*id))
        .cloned()
        .collect::<Vec<_>>();
    let missing_capability_order = request
        .required_capability_order
        .iter()
        .filter(|id| !present_capabilities.contains(*id))
        .cloned()
        .collect::<Vec<_>>();
    omission.extend(
        missing_endpoint_order
            .iter()
            .map(|id| format!("endpoint:{id}:missing")),
    );
    omission.extend(
        missing_capability_order
            .iter()
            .map(|id| format!("capability:{id}:missing")),
    );
    omission.extend(
        request
            .adversarial_events
            .iter()
            .map(|event| format!("request:adversarial:{event}")),
    );
    let global_open = request.policy_allow
        && request.protected_closure
        && request.signed_approval
        && request.endpoint_allow
        && request.adversarial_events.is_empty();
    let disposition = if !global_open
        || !blocked.is_empty()
        || !missing_endpoint_order.is_empty()
        || !missing_capability_order.is_empty()
    {
        "blocked"
    } else if !missing.is_empty() || !unresolved.is_empty() {
        "unresolved"
    } else {
        "qualified"
    };
    let effect = if disposition == "qualified" {
        vec![format!(
            "validate:typed-instrument-action:{}",
            request.request_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({"schema_version":OUTPUT_SCHEMA,"request_id":request.request_id,"action_order":action_order,"selected_action_order":selected,"unresolved_action_order":unresolved,"blocked_action_order":blocked,"missing_action_order":missing,"endpoint_order":endpoint_order,"capability_order":capability_order,"disposition":disposition,"replay_identity":request.replay_identity});
    let action_digest = ContentHash::of_value(&payload)
        .map_err(|error| InstrumentActionContractError::Artifact(error.to_string()))?;
    let semantic_loss = omission
        .iter()
        .map(|entry| SemanticLoss {
            field: entry.clone(),
            reason: "typed action was omitted or gated".into(),
            severity: LossSeverity::DecisionRelevant,
        })
        .collect::<Vec<_>>();
    let artifact = TypedResearchArtifact::from_payload(
        format!("instrument-actions:{}", request.request_id),
        CONTENT_TYPE,
        &payload,
        semantic_loss,
        vec![ProvenanceLink {
            source_id: request.request_id.clone(),
            relation: "docgraph-instrument-action-contract".into(),
            digest: action_digest.clone(),
        }],
    )
    .map_err(|error| InstrumentActionContractError::Artifact(error.to_string()))?;
    let receipt = InstrumentActionReceipt2 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        requester: request.requester.clone(),
        purpose: request.purpose.clone(),
        scope: request.scope.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition: disposition.into(),
        action_order: payload["action_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        selected_action_order: payload["selected_action_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        unresolved_action_order: payload["unresolved_action_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        blocked_action_order: payload["blocked_action_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        missing_action_order: payload["missing_action_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        endpoint_order: payload["endpoint_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        capability_order: payload["capability_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap().into())
            .collect(),
        selected_endpoint_order: actions
            .iter()
            .filter(|action| selected.contains(&action.action_id))
            .map(|action| action.endpoint_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        selected_capability_order: actions
            .iter()
            .filter(|action| selected.contains(&action.action_id))
            .map(|action| action.capability_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        missing_endpoint_order,
        missing_capability_order,
        omission_order: omission.into_iter().collect(),
        uncertainty_order: uncertainty.into_iter().collect(),
        negative_evidence_order: negative.into_iter().collect(),
        replay_identity: request.replay_identity.clone(),
        action_digest,
        artifact,
        effect_receipts: effect,
        raw_data_local: request.raw_data_local,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(seed: &str) -> ContentHash {
        ContentHash::of_bytes(seed.as_bytes())
    }
    fn action(id: &str) -> InstrumentAction4 {
        InstrumentAction4 {
            action_id: id.into(),
            endpoint_id: "endpoint:a".into(),
            capability_id: "capability:image".into(),
            study_id: "study:one".into(),
            modality: "imaging".into(),
            protocol_version: "v1".into(),
            parameters_digest: hash(id),
            artifact_digest: Some(hash(&format!("a:{id}"))),
            provenance_digest: Some(hash(&format!("p:{id}"))),
            replay_identity: hash("replay"),
            evidence_state: EvidenceState::Supported,
            scope_compatible: true,
            permitted: true,
            omissions: Vec::new(),
            negative_result: false,
        }
    }
    fn request() -> InstrumentActionRequest4 {
        InstrumentActionRequest4 {
            request_id: "request:actions".into(),
            requester: "extension-developer".into(),
            purpose: "preflight".into(),
            scope: "organoid".into(),
            semantic_profile: "instrument:v1".into(),
            schema_version: INPUT_SCHEMA.into(),
            required_action_order: vec!["action:a".into()],
            required_endpoint_order: vec!["endpoint:a".into()],
            required_capability_order: vec!["capability:image".into()],
            actions: vec![action("action:a")],
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            endpoint_allow: true,
            raw_data_local: true,
            adversarial_events: Vec::new(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn complete_contract_qualifies() {
        let receipt = validate_instrument_actions(&request()).unwrap();
        assert_eq!(receipt.disposition, "qualified");
        assert!(receipt.effect_receipts[0].starts_with("validate:typed-instrument-action:"));
    }
    #[test]
    fn missing_artifact_is_unresolved() {
        let mut value = request();
        value.actions[0].artifact_digest = None;
        assert_eq!(
            validate_instrument_actions(&value).unwrap().disposition,
            "unresolved"
        );
    }
    #[test]
    fn unknown_and_replay_mismatch_remain_unresolved() {
        let mut value = request();
        value.actions[0].evidence_state = EvidenceState::Unknown;
        assert_eq!(
            validate_instrument_actions(&value).unwrap().disposition,
            "unresolved"
        );
    }
    #[test]
    fn policy_and_endpoint_gates_block() {
        let mut value = request();
        value.policy_allow = false;
        assert_eq!(
            validate_instrument_actions(&value).unwrap().disposition,
            "blocked"
        );
    }
    #[test]
    fn missing_required_capability_blocks() {
        let mut value = request();
        value
            .required_capability_order
            .push("capability:omics".into());
        assert_eq!(
            validate_instrument_actions(&value).unwrap().disposition,
            "blocked"
        );
    }
    #[test]
    fn manifest_is_a1_byte_stable_and_effect_free() {
        let manifest = capability_manifest();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A1);
        assert_eq!(manifest.determinism, Determinism::ByteStable);
        assert!(manifest.effects.is_empty());
    }
}
