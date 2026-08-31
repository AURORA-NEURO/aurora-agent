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
const COPILOT_CONTENT_TYPE: &str = "application/vnd.aurora.qualified-evidence-set-3+json";
const MAX_TEXT_BYTES: usize = 512;

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
            || self.candidate_order.is_empty()
            || self.plan_order.is_empty()
            || self.plan_order.len() != self.action_order.len()
            || self.tool_order.len() != 1
            || self.effect_receipts.len() != 1
            || self.budget_units == 0
        {
            return Err(FederatedCopilotError::Invalid("federated copilot identity, envelope, bounded plan, tool, locality, budget, or effects are incomplete".into()));
        }
        for (value, field) in [
            (&self.request_id, "request_id"),
            (&self.operator_id, "operator_id"),
            (&self.federation_id, "federation_id"),
            (&self.institution_id, "institution_id"),
            (&self.purpose, "purpose"),
            (&self.semantic_profile, "semantic_profile"),
            (&self.endpoint, "endpoint"),
            (&self.boundary, "boundary"),
        ] {
            validate_text(value, field)?;
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
            validate_sorted_unique(values, "federated copilot collection")?;
        }
        if self
            .plan_order
            .iter()
            .zip(&self.action_order)
            .any(|(plan, action)| plan.strip_prefix("plan:") != action.strip_prefix("action:"))
            || !self
                .plan_order
                .iter()
                .any(|plan| plan == "plan:exchange-permitted-summary")
        {
            return Err(FederatedCopilotError::Invalid(
                "federated copilot plan and action orders are not paired canonically".into(),
            ));
        }
        let candidate_keys = identity_keys(&self.candidate_order);
        let admitted_keys = identity_keys(&self.admitted_order);
        let blocked_keys = identity_keys(&self.blocked_order);
        let unknown_keys = identity_keys(&self.unknown_order);
        if admitted_keys
            .union(&blocked_keys)
            .cloned()
            .collect::<BTreeSet<_>>()
            != candidate_keys
            || !admitted_keys.is_disjoint(&blocked_keys)
            || !unknown_keys.is_subset(&blocked_keys)
            || self.aggregate_order.len() != self.admitted_order.len()
        {
            return Err(FederatedCopilotError::Invalid(
                "federated copilot state is not a disjoint candidate partition".into(),
            ));
        }
        if self
            .aggregate_order
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
            || self
                .aggregate_order
                .iter()
                .any(|value| value.as_str().len() != 64)
        {
            return Err(FederatedCopilotError::Invalid(
                "federated aggregate ordering or digest is invalid".into(),
            ));
        }
        for digest in [
            &self.envelope_digest,
            &self.evidence_receipt_digest,
            &self.plan_digest,
            &self.approval_reference,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(FederatedCopilotError::Invalid(
                    "federated copilot digest is invalid".into(),
                ));
            }
        }
        if !self.raw_data_local {
            return Err(FederatedCopilotError::Invalid(
                "federated copilot receipts must declare local emitted data".into(),
            ));
        }
        let expected_effect = if self.disposition != FederatedEvidenceDisposition::Blocked
            && self.disposition != FederatedEvidenceDisposition::Unknown
            && !self.admitted_order.is_empty()
        {
            format!("invoke:declared-tool:{}", self.tool_order[0])
        } else {
            "block:unsafe-release".into()
        };
        if self.effect_receipts != [expected_effect] {
            return Err(FederatedCopilotError::Invalid(
                "federated copilot effect does not match disposition and admission".into(),
            ));
        }
        let expected_plan_digest = ContentHash::of_value(&json!({
            "request_id": self.request_id,
            "federation_id": self.federation_id,
            "plan_order": self.plan_order,
            "action_order": self.action_order,
            "tool_order": self.tool_order,
            "approval_reference": self.approval_reference,
            "replay_identity": self.replay_identity,
        }))
        .map_err(|error| FederatedCopilotError::Artifact(error.to_string()))?;
        if self.plan_digest != expected_plan_digest {
            return Err(FederatedCopilotError::Invalid(
                "federated copilot plan digest is not bound to the plan".into(),
            ));
        }
        let expected_artifact_id = format!("brain-federated-evidence-copilot:{}", self.request_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != COPILOT_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(FederatedCopilotError::Invalid(
                "federated copilot artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| FederatedCopilotError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
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

fn validate_text(value: &str, field: &str) -> Result<(), FederatedCopilotError> {
    if value.trim() != value
        || value.is_empty()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(FederatedCopilotError::Invalid(format!(
            "{field} is empty, padded, oversized, or contains control characters"
        )));
    }
    Ok(())
}

fn validate_unique(values: &[String], field: &str) -> Result<(), FederatedCopilotError> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_text(value, field)?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(FederatedCopilotError::Invalid(format!(
                "{field} contains a duplicate or case-colliding identity"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_unique(values: &[String], field: &str) -> Result<(), FederatedCopilotError> {
    validate_unique(values, field)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(FederatedCopilotError::Invalid(format!(
            "{field} is not in canonical order"
        )));
    }
    Ok(())
}

fn identity_keys(values: &[String]) -> BTreeSet<String> {
    values
        .iter()
        .map(|value| value.to_ascii_lowercase())
        .collect()
}

fn receipt_payload(receipt: &FederatedCopilotReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "operator_id": receipt.operator_id,
        "federation_id": receipt.federation_id,
        "institution_id": receipt.institution_id,
        "purpose": receipt.purpose,
        "semantic_profile": receipt.semantic_profile,
        "endpoint": receipt.endpoint,
        "disposition": receipt.disposition,
        "plan_order": receipt.plan_order,
        "action_order": receipt.action_order,
        "tool_order": receipt.tool_order,
        "candidate_order": receipt.candidate_order,
        "admitted_order": receipt.admitted_order,
        "blocked_order": receipt.blocked_order,
        "unknown_order": receipt.unknown_order,
        "aggregate_order": receipt.aggregate_order,
        "envelope_digest": receipt.envelope_digest,
        "evidence_receipt_digest": receipt.evidence_receipt_digest,
        "plan_digest": receipt.plan_digest,
        "approval_reference": receipt.approval_reference,
        "replay_identity": receipt.replay_identity,
        "budget_units": receipt.budget_units,
        "omissions": receipt.omissions,
        "uncertainty": receipt.uncertainty,
        "negative_evidence": receipt.negative_evidence,
        "effect_receipts": receipt.effect_receipts,
        "raw_data_local": receipt.raw_data_local,
        "boundary": receipt.boundary,
    })
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
    if u64::from(request.budget_units) < u64::try_from(action_order.len()).unwrap_or(u64::MAX) {
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
        && u64::from(request.budget_units) >= u64::try_from(action_order.len()).unwrap_or(u64::MAX)
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
    let effect_receipts = if disposition != FederatedEvidenceDisposition::Blocked
        && disposition != FederatedEvidenceDisposition::Unknown
        && !evidence.admitted_order.is_empty()
    {
        vec![format!("invoke:declared-tool:{}", request.declared_tool_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let evidence_digest = evidence
        .digest()
        .map_err(|error| FederatedCopilotError::Engine(error.to_string()))?;
    let plan_digest = ContentHash::of_value(&json!({
        "request_id": request.request.request_id,
        "federation_id": request.request.federation_id,
        "plan_order": plan_vec,
        "action_order": action_vec,
        "tool_order": tool_vec,
        "approval_reference": request.approval_reference,
        "replay_identity": request.replay_identity,
    }))
    .map_err(|error| FederatedCopilotError::Artifact(error.to_string()))?;
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request.request_id,
        "operator_id": request.operator_id,
        "federation_id": request.request.federation_id,
        "institution_id": request.request.institution_id,
        "purpose": request.request.purpose,
        "semantic_profile": request.request.semantic_profile,
        "endpoint": request.request.endpoint,
        "disposition": disposition,
        "plan_order": plan_vec,
        "action_order": action_vec,
        "tool_order": tool_vec,
        "candidate_order": evidence.candidate_order,
        "admitted_order": evidence.admitted_order,
        "blocked_order": evidence.blocked_order,
        "unknown_order": evidence.unknown_order,
        "aggregate_order": evidence.aggregate_order,
        "envelope_digest": evidence.artifact.content_hash,
        "evidence_receipt_digest": evidence_digest,
        "plan_digest": plan_digest,
        "approval_reference": request.approval_reference,
        "replay_identity": request.replay_identity,
        "budget_units": request.budget_units,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": negative,
        "effect_receipts": effect_receipts,
        "raw_data_local": true,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "brain-federated-evidence-copilot:{}",
            request.request.request_id
        ),
        COPILOT_CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| FederatedCopilotError::Artifact(error.to_string()))?;
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
        plan_order: plan_vec,
        action_order: action_vec,
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
        effect_receipts,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &FederatedCopilotRequest) -> Result<(), FederatedCopilotError> {
    if request.action_allow_list.is_empty()
        || request.max_actions == 0
        || request.max_actions > MAX_ACTIONS
        || request.budget_units == 0
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.request.boundary != PRECLINICAL_BOUNDARY
        || request.request.replay_identity != request.replay_identity
        || request.request.policy_allow != request.policy_allow
        || request.request.protected_closure != request.protected_closure
        || request.request.raw_data_local != request.raw_data_local
        || request.approval_reference == ContentHash::of_bytes(&[])
    {
        return Err(FederatedCopilotError::Invalid("federated copilot operator, declared tool, approval, bounded actions, budget, replay, policy, locality, or boundary is incomplete".into()));
    }
    for (value, field) in [
        (&request.request.request_id, "request_id"),
        (&request.operator_id, "operator_id"),
        (&request.request.federation_id, "federation_id"),
        (&request.request.institution_id, "institution_id"),
        (&request.request.purpose, "purpose"),
        (&request.request.semantic_profile, "semantic_profile"),
        (&request.request.endpoint, "endpoint"),
        (&request.declared_tool_id, "declared_tool_id"),
        (&request.boundary, "boundary"),
    ] {
        validate_text(value, field)?;
    }
    validate_unique(&request.action_allow_list, "action_allow_list")?;
    validate_unique(&request.request.allowed_artifacts, "allowed_artifacts")?;
    for digest in [&request.approval_reference, &request.replay_identity] {
        if digest.as_str().len() != 64 {
            return Err(FederatedCopilotError::Invalid(
                "federated copilot approval or replay digest is invalid".into(),
            ));
        }
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
    fn locality_failure_is_blocked_and_retained() {
        let mut input = request(vec![observation("a", EvidenceState::Supported)]);
        input.raw_data_local = false;
        input.request.raw_data_local = false;
        let receipt = compile_federated_evidence_copilot(&input).unwrap();
        assert_eq!(receipt.disposition, FederatedEvidenceDisposition::Blocked);
        assert!(receipt.raw_data_local);
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|item| item == "request:raw-data-locality-failed"));
        receipt.validate().unwrap();
    }
    #[test]
    fn plan_and_payload_drift_are_rejected() {
        let receipt = compile_federated_evidence_copilot(&request(vec![observation(
            "a",
            EvidenceState::Supported,
        )]))
        .unwrap();
        let mut plan_drift = receipt.clone();
        plan_drift.action_order.pop();
        assert!(plan_drift.validate().is_err());

        let mut payload_drift = receipt;
        payload_drift.endpoint = "https://other.example/research".into();
        assert!(payload_drift.validate().is_err());
    }
    #[test]
    fn padded_operator_identity_is_rejected() {
        let mut input = request(vec![observation("a", EvidenceState::Supported)]);
        input.operator_id = " operator:researcher".into();
        assert!(compile_federated_evidence_copilot(&input).is_err());
    }
    #[test]
    fn approval_identity_is_required() {
        let mut input = request(vec![observation("a", EvidenceState::Supported)]);
        input.approval_reference = ContentHash::of_bytes(&[]);
        assert!(compile_federated_evidence_copilot(&input).is_err());
    }
}
