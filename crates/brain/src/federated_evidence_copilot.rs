//! Federated continual evidence research copilot.
//!
//! Atlas feature: `AFA-brain-P01-F12`. This A2 capability turns local federated admission
//! into a bounded declared-tool plan. Only permitted aggregate research artifacts may be
//! exchanged; raw observations and clinical decisions remain local and out of scope.

use crate::federated_evidence_surveillance::{
    admit_federated_evidence, FederatedEvidenceDisposition, FederatedEvidenceFeedRequest,
};
use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-brain-P01-F12";
pub const CONTRACT_VERSION: &str = "brain-federated-evidence-research-copilot/1.0";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet3@1";
pub const MAX_ACTIONS: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedCopilotRequest {
    pub request: FederatedEvidenceFeedRequest,
    pub operator_id: String,
    pub action_allow_list: Vec<String>,
    pub declared_tool_id: String,
    pub approval_reference: ContentHash,
    pub max_actions: usize,
    pub budget_units: u32,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedCopilotReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub operator_id: String,
    pub federation_id: String,
    pub institution_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub endpoint: String,
    pub disposition: FederatedEvidenceDisposition,
    pub plan_order: Vec<String>,
    pub action_order: Vec<String>,
    pub tool_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub aggregate_order: Vec<ContentHash>,
    pub envelope_digest: ContentHash,
    pub evidence_receipt_digest: ContentHash,
    pub plan_digest: ContentHash,
    pub approval_reference: ContentHash,
    pub replay_identity: ContentHash,
    pub budget_units: u32,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedCopilotError {
    #[error("invalid federated copilot request: {0}")]
    Invalid(String),
    #[error("federated copilot artifact failed: {0}")]
    Artifact(String),
    #[error("federated copilot engine failed: {0}")]
    Engine(String),
}

impl FederatedCopilotReceipt {
    pub fn validate(&self) -> Result<(), FederatedCopilotError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.operator_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.institution_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.endpoint.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.plan_order.is_empty()
            || self.plan_order.len() != self.action_order.len()
            || self.tool_order.is_empty()
            || self.effect_receipts.is_empty()
            || self.budget_units == 0
        {
            return Err(FederatedCopilotError::Invalid("federated copilot identity, envelope, bounded plan, tool, locality, budget, or effects are incomplete".into()));
        }
        if self
            .admitted_order
            .iter()
            .chain(self.blocked_order.iter())
            .chain(self.unknown_order.iter())
            .any(|id| !self.candidate_order.contains(id))
        {
            return Err(FederatedCopilotError::Invalid(
                "federated copilot state is not covered by candidate order".into(),
            ));
        }
        for values in [
            &self.plan_order,
            &self.action_order,
            &self.tool_order,
            &self.candidate_order,
            &self.admitted_order,
            &self.blocked_order,
            &self.unknown_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(FederatedCopilotError::Invalid(
                    "federated copilot ordering is not canonical".into(),
                ));
            }
        }
        if self
            .aggregate_order
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(FederatedCopilotError::Invalid(
                "federated aggregate ordering is not canonical".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("invoke:declared-tool:") && effect != "block:unsafe-release"
        }) {
            return Err(FederatedCopilotError::Invalid(
                "federated copilot effect is outside declared-tool gate".into(),
            ));
        }
        if self.disposition != FederatedEvidenceDisposition::Blocked
            && !self.admitted_order.is_empty()
            && !self
                .effect_receipts
                .iter()
                .any(|effect| effect.starts_with("invoke:declared-tool:"))
        {
            return Err(FederatedCopilotError::Invalid(
                "admitted federation requires a declared-tool receipt".into(),
            ));
        }
        if self.disposition != FederatedEvidenceDisposition::Qualified
            && self.disposition != FederatedEvidenceDisposition::Partial
            && self.effect_receipts != ["block:unsafe-release"]
        {
            return Err(FederatedCopilotError::Invalid(
                "non-admitted federation must be explicitly blocked".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| FederatedCopilotError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, FederatedCopilotError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| FederatedCopilotError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| FederatedCopilotError::Artifact(error.to_string()))
    }
}

pub fn federated_evidence_research_copilot_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["platform reliability engineer".into(), "federation steward".into()].into(), behavior: "compiles federated EvidenceFeed4 admission into a bounded declared-tool plan with aggregate-only export and explicit signer/policy evidence".into(), value: "automates continual consortium evidence surveillance without moving raw observations or hiding federation denial".into(), inputs: vec![TypedPort { name: "federated_evidence_feed".into(), schema: "EvidenceFeed4@1".into(), required: true }], outputs: vec![TypedPort { name: "qualified_evidence_set".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::ExternalDataAccess, Effect::FederationExport, Effect::WriteLocalArtifact].into(), permissions: ["invoke:declared-tools".into(), "export:permitted-research-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "mcp-2025-06-18".into(), state: EvidenceState::Supported, locator: Some("https://modelcontextprotocol.io/specification/2025-06-18/basic/index".into()) }], authority_requirements: vec![AuthorityRequirement { role: "federation release approver".into(), reason: "authorize purpose-bound aggregate-only exchange after signer and artifact gates close".into() }], autonomy_tier: AutonomyTier::A2, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn compile_federated_evidence_copilot(
    request: &FederatedCopilotRequest,
) -> Result<FederatedCopilotReceipt, FederatedCopilotError> {
    validate_request(request)?;
    let evidence = admit_federated_evidence(&request.request)
        .map_err(|error| FederatedCopilotError::Engine(error.to_string()))?;
    let mut plan_order = BTreeSet::new();
    let mut action_order = BTreeSet::new();
    let mut omissions = evidence.omissions.iter().cloned().collect::<BTreeSet<_>>();
    let mut uncertainty = evidence
        .uncertainty
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut negative = evidence
        .negative_evidence
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    for id in &evidence.admitted_order {
        plan_order.insert(format!("plan:admit-federated:{id}"));
        action_order.insert(format!("action:admit-federated:{id}"));
    }
    if evidence.admitted_order.is_empty() {
        plan_order.insert("plan:retain-federated-unknown".into());
        action_order.insert("action:retain-federated-unknown".into());
    }
    plan_order.insert("plan:exchange-permitted-summary".into());
    action_order.insert("action:exchange-permitted-summary".into());
    if !request
        .action_allow_list
        .iter()
        .any(|item| item == "admit-federated-evidence")
    {
        negative.insert("copilot:admit-federated-evidence-not-allowed".into());
    }
    if !request
        .action_allow_list
        .iter()
        .any(|item| item == "exchange-permitted-summary")
    {
        negative.insert("copilot:exchange-permitted-summary-not-allowed".into());
    }
    if request.budget_units < action_order.len() as u32 {
        omissions.insert("copilot:action-budget-exhausted".into());
    }
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.raw_data_local {
        negative.insert("request:raw-data-locality-failed".into());
    }
    let actionable = request
        .action_allow_list
        .iter()
        .any(|item| item == "admit-federated-evidence")
        && request
            .action_allow_list
            .iter()
            .any(|item| item == "exchange-permitted-summary")
        && request.budget_units >= action_order.len() as u32
        && request.policy_allow
        && request.protected_closure
        && request.raw_data_local
        && !request.declared_tool_id.trim().is_empty();
    let disposition = if !actionable {
        FederatedEvidenceDisposition::Blocked
    } else {
        evidence.disposition
    };
    let plan_vec = plan_order.into_iter().collect::<Vec<_>>();
    let action_vec = action_order.into_iter().collect::<Vec<_>>();
    let tool_vec = vec![request.declared_tool_id.clone()];
    let evidence_digest = evidence
        .digest()
        .map_err(|error| FederatedCopilotError::Engine(error.to_string()))?;
    let plan_digest = ContentHash::of_value(&json!({"request_id": request.request.request_id, "federation_id": request.request.federation_id, "plan_order": plan_vec, "action_order": action_vec, "tool_order": tool_vec, "approval_reference": request.approval_reference, "replay_identity": request.replay_identity})).map_err(|error| FederatedCopilotError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request.request_id, "operator_id": request.operator_id, "federation_id": request.request.federation_id, "institution_id": request.request.institution_id, "purpose": request.request.purpose, "semantic_profile": request.request.semantic_profile, "endpoint": request.request.endpoint, "disposition": disposition, "plan_order": plan_vec, "action_order": action_vec, "tool_order": tool_vec, "candidate_order": evidence.candidate_order, "admitted_order": evidence.admitted_order, "blocked_order": evidence.blocked_order, "unknown_order": evidence.unknown_order, "aggregate_order": evidence.aggregate_order, "envelope_digest": evidence.artifact.content_hash, "evidence_receipt_digest": evidence_digest, "plan_digest": plan_digest, "approval_reference": request.approval_reference, "replay_identity": request.replay_identity, "budget_units": request.budget_units, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "brain-federated-evidence-copilot:{}",
            request.request.request_id
        ),
        "application/vnd.aurora.qualified-evidence-set-3+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| FederatedCopilotError::Artifact(error.to_string()))?;
    let has_effect = actionable && !evidence.admitted_order.is_empty();
    let receipt = FederatedCopilotReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request.request_id.clone(),
        operator_id: request.operator_id.clone(),
        federation_id: request.request.federation_id.clone(),
        institution_id: request.request.institution_id.clone(),
        purpose: request.request.purpose.clone(),
        semantic_profile: request.request.semantic_profile.clone(),
        endpoint: request.request.endpoint.clone(),
        disposition,
        plan_order: payload
            .get("plan_order")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default(),
        action_order: payload
            .get("action_order")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default(),
        tool_order: tool_vec,
        candidate_order: evidence.candidate_order.clone(),
        admitted_order: evidence.admitted_order.clone(),
        blocked_order: evidence.blocked_order.clone(),
        unknown_order: evidence.unknown_order.clone(),
        aggregate_order: evidence.aggregate_order.clone(),
        envelope_digest: evidence.artifact.content_hash.clone(),
        evidence_receipt_digest: evidence_digest,
        plan_digest,
        approval_reference: request.approval_reference.clone(),
        replay_identity: request.replay_identity.clone(),
        budget_units: request.budget_units,
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts: if has_effect {
            vec![format!("invoke:declared-tool:{}", request.declared_tool_id)]
        } else {
            vec!["block:unsafe-release".into()]
        },
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &FederatedCopilotRequest) -> Result<(), FederatedCopilotError> {
    if request.operator_id.trim().is_empty()
        || request.action_allow_list.is_empty()
        || request.declared_tool_id.trim().is_empty()
        || request.max_actions == 0
        || request.max_actions > MAX_ACTIONS
        || request.budget_units == 0
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.request.boundary != PRECLINICAL_BOUNDARY
        || request.request.replay_identity != request.replay_identity
        || request.approval_reference == ContentHash::of_bytes(&[])
    {
        return Err(FederatedCopilotError::Invalid("federated copilot operator, declared tool, approval, bounded actions, budget, replay, or boundary is incomplete".into()));
    }
    if request.request.observations.len() > request.max_actions.saturating_mul(64) {
        return Err(FederatedCopilotError::Invalid(
            "federated evidence feed exceeds bounded plan capacity".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence_surveillance::EvidenceObservation;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn observation(id: &str, state: EvidenceState) -> EvidenceObservation {
        EvidenceObservation {
            evidence_id: format!("evidence:{id}"),
            source_id: format!("source:{id}"),
            study_id: "study:organoid".into(),
            modality: "imaging".into(),
            scope: "organoid:neural".into(),
            relevance_milli: 900,
            state,
            semantic_digest: hash(&format!("semantic:{id}")),
            artifact_digest: hash(&format!("artifact:{id}")),
            provenance_digest: hash(&format!("provenance:{id}")),
            replay_identity: hash("replay"),
            omissions: Vec::new(),
            negative_evidence: Vec::new(),
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    fn request(observations: Vec<EvidenceObservation>) -> FederatedCopilotRequest {
        FederatedCopilotRequest {
            request: FederatedEvidenceFeedRequest {
                request_id: "request:federated-copilot".into(),
                federation_id: "federation:commons".into(),
                institution_id: "institution:a".into(),
                purpose: "benchmarking".into(),
                semantic_profile: "preclinical-evidence/v1".into(),
                endpoint: "https://hub.example/research".into(),
                allowed_artifacts: vec!["qualified-evidence-summary".into()],
                observations,
                replay_identity: hash("replay"),
                policy_allow: true,
                protected_closure: true,
                signer_valid: true,
                raw_data_local: true,
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            operator_id: "operator:researcher".into(),
            action_allow_list: vec![
                "admit-federated-evidence".into(),
                "exchange-permitted-summary".into(),
            ],
            declared_tool_id: "tool:federated-evidence".into(),
            approval_reference: hash("signed-approval"),
            max_actions: 8,
            budget_units: 8,
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a2_and_aggregate_scoped() {
        let manifest = federated_evidence_research_copilot_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A2);
        assert!(manifest.effects.contains(&Effect::FederationExport));
    }
    #[test]
    fn signed_summary_invokes_declared_tool() {
        let receipt = compile_federated_evidence_copilot(&request(vec![observation(
            "a",
            EvidenceState::Supported,
        )]))
        .unwrap();
        assert!(receipt.effect_receipts[0].starts_with("invoke:declared-tool:"));
        assert!(!receipt.aggregate_order.is_empty());
    }
    #[test]
    fn signer_failure_is_retained_and_blocked() {
        let mut input = request(vec![observation("a", EvidenceState::Supported)]);
        input.request.signer_valid = false;
        let receipt = compile_federated_evidence_copilot(&input).unwrap();
        assert_eq!(receipt.disposition, FederatedEvidenceDisposition::Blocked);
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn unknown_evidence_is_retained() {
        let receipt = compile_federated_evidence_copilot(&request(vec![observation(
            "a",
            EvidenceState::Unknown,
        )]))
        .unwrap();
        assert_eq!(receipt.disposition, FederatedEvidenceDisposition::Unknown);
        assert!(!receipt.unknown_order.is_empty());
    }
    #[test]
    fn tool_allowance_denial_blocks() {
        let mut input = request(vec![observation("a", EvidenceState::Supported)]);
        input.action_allow_list = vec!["write-external".into()];
        let receipt = compile_federated_evidence_copilot(&input).unwrap();
        assert_eq!(receipt.disposition, FederatedEvidenceDisposition::Blocked);
    }
    #[test]
    fn approval_identity_is_required() {
        let mut input = request(vec![observation("a", EvidenceState::Supported)]);
        input.approval_reference = ContentHash::of_bytes(&[]);
        assert!(compile_federated_evidence_copilot(&input).is_err());
    }
}
