//! Federated researcher workbench for aggregate-only consortium review.
//!
//! Atlas feature: `AFA-brain-P01-F20`. The workbench exposes institution and exchange
//! diagnostics without moving raw observations or turning a denied exchange into a pass.

use crate::federated_evidence_surveillance::{
    admit_federated_evidence, FederatedEvidenceDisposition, FederatedEvidenceFeedRequest,
};
use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-brain-P01-F20";
pub const CONTRACT_VERSION: &str = "brain-federated-research-workbench/1.0";
pub const VIEW_ORDER: [&str; 3] = [
    "view:aggregate-comparison",
    "view:exchange-lineage",
    "view:institution-coverage",
];
pub const PANEL_ORDER: [&str; 3] = ["panel:aggregate", "panel:institutions", "panel:lineage"];
const ACTION_ORDER: [&str; 3] = [
    "action:render-aggregate-comparison",
    "action:render-exchange-lineage",
    "action:render-institution-coverage",
];
const WORKBENCH_CONTENT_TYPE: &str = "application/vnd.aurora.federated-workbench-receipt+json";
const MAX_TEXT_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedWorkbenchRequest {
    pub request: FederatedEvidenceFeedRequest,
    pub workspace_id: String,
    pub requested_view_order: Vec<String>,
    pub requested_panel_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub budget_units: u32,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedWorkbenchReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workspace_id: String,
    pub federation_id: String,
    pub institution_id: String,
    pub purpose: String,
    pub endpoint: String,
    pub disposition: FederatedEvidenceDisposition,
    pub view_order: Vec<String>,
    pub panel_order: Vec<String>,
    pub action_receipts: Vec<String>,
    pub candidate_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub aggregate_order: Vec<ContentHash>,
    pub evidence_digest: ContentHash,
    pub envelope_digest: ContentHash,
    pub workbench_digest: ContentHash,
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
pub enum FederatedWorkbenchError {
    #[error("invalid federated workbench request: {0}")]
    Invalid(String),
    #[error("federated workbench artifact failed: {0}")]
    Artifact(String),
    #[error("federated workbench engine failed: {0}")]
    Engine(String),
}

impl FederatedWorkbenchReceipt {
    pub fn validate(&self) -> Result<(), FederatedWorkbenchError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.candidate_order.is_empty()
            || self.effect_receipts.len() != 1
            || self.budget_units == 0
            || !self.raw_data_local
        {
            return Err(FederatedWorkbenchError::Invalid("federated workbench identity, exchange views, evidence, locality, budget, or effects are incomplete".into()));
        }
        for (value, field) in [
            (&self.request_id, "request_id"),
            (&self.workspace_id, "workspace_id"),
            (&self.federation_id, "federation_id"),
            (&self.institution_id, "institution_id"),
            (&self.purpose, "purpose"),
            (&self.endpoint, "endpoint"),
            (&self.boundary, "boundary"),
        ] {
            validate_text(value, field)?;
        }
        if self.view_order != VIEW_ORDER
            || self.panel_order != PANEL_ORDER
            || self.action_receipts != ACTION_ORDER
        {
            return Err(FederatedWorkbenchError::Invalid(
                "federated workbench view, panel, or action order is not canonical".into(),
            ));
        }
        for values in [
            &self.candidate_order,
            &self.admitted_order,
            &self.blocked_order,
            &self.unknown_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            validate_sorted_unique(values, "federated workbench collection")?;
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
            return Err(FederatedWorkbenchError::Invalid(
                "federated workbench state is not a disjoint candidate partition".into(),
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
            return Err(FederatedWorkbenchError::Invalid(
                "federated aggregate ordering or digest is invalid".into(),
            ));
        }
        if self
            .omissions
            .iter()
            .any(|item| item == "workbench:raw-data-locality-failed")
            && self.disposition != FederatedEvidenceDisposition::Blocked
        {
            return Err(FederatedWorkbenchError::Invalid(
                "non-local federated workbenches must be blocked and retain locality evidence"
                    .into(),
            ));
        }
        let expected_effect = if self.disposition != FederatedEvidenceDisposition::Blocked {
            format!("view:local-federated-artifacts:{}", self.workspace_id)
        } else {
            "block:unsafe-release".into()
        };
        if self.effect_receipts != [expected_effect] {
            return Err(FederatedWorkbenchError::Invalid(
                "federated workbench effect does not match disposition".into(),
            ));
        }
        for digest in [
            &self.evidence_digest,
            &self.envelope_digest,
            &self.workbench_digest,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(FederatedWorkbenchError::Invalid(
                    "federated workbench digest is invalid".into(),
                ));
            }
        }
        let expected_envelope_digest = ContentHash::of_value(&json!({
            "federation_id": self.federation_id,
            "institution_id": self.institution_id,
            "purpose": self.purpose,
            "aggregate_order": self.aggregate_order,
            "replay_identity": self.replay_identity,
            "raw_data_local": self.raw_data_local,
        }))
        .map_err(|error| FederatedWorkbenchError::Artifact(error.to_string()))?;
        if self.envelope_digest != expected_envelope_digest {
            return Err(FederatedWorkbenchError::Invalid(
                "federated workbench envelope digest is not bound to aggregate state".into(),
            ));
        }
        let expected_workbench_digest = ContentHash::of_value(&json!({
            "workspace_id": self.workspace_id,
            "federation_id": self.federation_id,
            "view_order": self.view_order,
            "panel_order": self.panel_order,
            "action_receipts": self.action_receipts,
            "disposition": self.disposition,
            "evidence_digest": self.evidence_digest,
            "envelope_digest": self.envelope_digest,
            "replay_identity": self.replay_identity,
            "budget_units": self.budget_units,
            "effect_receipts": self.effect_receipts,
            "raw_data_local": self.raw_data_local,
        }))
        .map_err(|error| FederatedWorkbenchError::Artifact(error.to_string()))?;
        if self.workbench_digest != expected_workbench_digest {
            return Err(FederatedWorkbenchError::Invalid(
                "federated workbench digest is not bound to view state".into(),
            ));
        }
        let expected_artifact_id = format!("brain-federated-workbench:{}", self.workspace_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != WORKBENCH_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(FederatedWorkbenchError::Invalid(
                "federated workbench artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| FederatedWorkbenchError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| FederatedWorkbenchError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, FederatedWorkbenchError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| FederatedWorkbenchError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| FederatedWorkbenchError::Artifact(error.to_string()))
    }
}

fn validate_text(value: &str, field: &str) -> Result<(), FederatedWorkbenchError> {
    if value.trim() != value
        || value.is_empty()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(FederatedWorkbenchError::Invalid(format!(
            "{field} is empty, padded, oversized, or contains control characters"
        )));
    }
    Ok(())
}

fn validate_unique(values: &[String], field: &str) -> Result<(), FederatedWorkbenchError> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_text(value, field)?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(FederatedWorkbenchError::Invalid(format!(
                "{field} contains a duplicate or case-colliding identity"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_unique(values: &[String], field: &str) -> Result<(), FederatedWorkbenchError> {
    validate_unique(values, field)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(FederatedWorkbenchError::Invalid(format!(
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

fn receipt_payload(receipt: &FederatedWorkbenchReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "workspace_id": receipt.workspace_id,
        "federation_id": receipt.federation_id,
        "institution_id": receipt.institution_id,
        "purpose": receipt.purpose,
        "endpoint": receipt.endpoint,
        "disposition": receipt.disposition,
        "view_order": receipt.view_order,
        "panel_order": receipt.panel_order,
        "action_receipts": receipt.action_receipts,
        "candidate_order": receipt.candidate_order,
        "admitted_order": receipt.admitted_order,
        "blocked_order": receipt.blocked_order,
        "unknown_order": receipt.unknown_order,
        "aggregate_order": receipt.aggregate_order,
        "evidence_digest": receipt.evidence_digest,
        "envelope_digest": receipt.envelope_digest,
        "workbench_digest": receipt.workbench_digest,
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

pub fn federated_research_workbench_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["consortium researcher".into(), "federation steward".into()].into(), behavior: "renders aggregate comparison, institution coverage, and exchange lineage views without raw-data federation".into(), value: "makes signed federation quality and denial evidence inspectable to researchers while preserving institutional locality".into(), inputs: vec![TypedPort { name: "federated_workbench_request".into(), schema: "ResearchWorkbenchSpec4@1".into(), required: true }], outputs: vec![TypedPort { name: "federated_workbench_receipt".into(), schema: "FederatedWorkbenchReceipt1@1".into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact, Effect::FederationExport].into(), permissions: ["view:local-federated-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A0, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn compile_federated_research_workbench(
    request: &FederatedWorkbenchRequest,
) -> Result<FederatedWorkbenchReceipt, FederatedWorkbenchError> {
    validate_request(request)?;
    let evidence = admit_federated_evidence(&request.request)
        .map_err(|error| FederatedWorkbenchError::Engine(error.to_string()))?;
    let mut omissions = evidence.omissions.iter().cloned().collect::<BTreeSet<_>>();
    let uncertainty = evidence
        .uncertainty
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let negative = evidence
        .negative_evidence
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let view_order = request.requested_view_order.clone();
    let panel_order = request.requested_panel_order.clone();
    let action_receipts = ACTION_ORDER
        .into_iter()
        .map(String::from)
        .collect::<Vec<_>>();
    let actionable = request.policy_allow
        && request.protected_closure
        && request.raw_data_local
        && u64::from(request.budget_units)
            >= u64::try_from(action_receipts.len()).unwrap_or(u64::MAX)
        && evidence.disposition != FederatedEvidenceDisposition::Blocked;
    if u64::from(request.budget_units) < u64::try_from(action_receipts.len()).unwrap_or(u64::MAX) {
        omissions.insert("workbench:budget-exhausted".into());
    }
    if !request.policy_allow {
        omissions.insert("workbench:policy-denied".into());
    }
    if !request.protected_closure {
        omissions.insert("workbench:protected-closure-incomplete".into());
    }
    if !request.raw_data_local {
        omissions.insert("workbench:raw-data-locality-failed".into());
    }
    let disposition = if actionable {
        evidence.disposition
    } else {
        FederatedEvidenceDisposition::Blocked
    };
    let effect_receipts = if disposition != FederatedEvidenceDisposition::Blocked {
        vec![format!(
            "view:local-federated-artifacts:{}",
            request.workspace_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let evidence_digest = evidence
        .digest()
        .map_err(|error| FederatedWorkbenchError::Engine(error.to_string()))?;
    let raw_data_local = true;
    let envelope_digest = ContentHash::of_value(&json!({
        "federation_id": request.request.federation_id,
        "institution_id": request.request.institution_id,
        "purpose": request.request.purpose,
        "aggregate_order": evidence.aggregate_order,
        "replay_identity": request.replay_identity,
        "raw_data_local": raw_data_local,
    }))
    .map_err(|error| FederatedWorkbenchError::Artifact(error.to_string()))?;
    let workbench_digest = ContentHash::of_value(&json!({
        "workspace_id": request.workspace_id,
        "federation_id": request.request.federation_id,
        "view_order": view_order,
        "panel_order": panel_order,
        "action_receipts": action_receipts,
        "disposition": disposition,
        "evidence_digest": evidence_digest,
        "envelope_digest": envelope_digest,
        "replay_identity": request.replay_identity,
        "budget_units": request.budget_units,
        "effect_receipts": effect_receipts,
        "raw_data_local": raw_data_local,
    }))
    .map_err(|error| FederatedWorkbenchError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request.request_id, "workspace_id": request.workspace_id, "federation_id": request.request.federation_id, "institution_id": request.request.institution_id, "purpose": request.request.purpose, "endpoint": request.request.endpoint, "disposition": disposition, "view_order": view_order, "panel_order": panel_order, "action_receipts": action_receipts, "candidate_order": evidence.candidate_order, "admitted_order": evidence.admitted_order, "blocked_order": evidence.blocked_order, "unknown_order": evidence.unknown_order, "aggregate_order": evidence.aggregate_order, "evidence_digest": evidence_digest, "envelope_digest": envelope_digest, "workbench_digest": workbench_digest, "replay_identity": request.replay_identity, "budget_units": request.budget_units, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "effect_receipts": effect_receipts, "raw_data_local": raw_data_local, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-federated-workbench:{}", request.workspace_id),
        WORKBENCH_CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| FederatedWorkbenchError::Artifact(error.to_string()))?;
    let receipt = FederatedWorkbenchReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request.request_id.clone(),
        workspace_id: request.workspace_id.clone(),
        federation_id: request.request.federation_id.clone(),
        institution_id: request.request.institution_id.clone(),
        purpose: request.request.purpose.clone(),
        endpoint: request.request.endpoint.clone(),
        disposition,
        view_order,
        panel_order,
        action_receipts,
        candidate_order: evidence.candidate_order.clone(),
        admitted_order: evidence.admitted_order.clone(),
        blocked_order: evidence.blocked_order.clone(),
        unknown_order: evidence.unknown_order.clone(),
        aggregate_order: evidence.aggregate_order.clone(),
        evidence_digest,
        envelope_digest,
        workbench_digest,
        replay_identity: request.replay_identity.clone(),
        budget_units: request.budget_units,
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts,
        artifact,
        raw_data_local,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &FederatedWorkbenchRequest) -> Result<(), FederatedWorkbenchError> {
    let expected_view_order = VIEW_ORDER
        .iter()
        .map(|value| (*value).to_string())
        .collect::<Vec<_>>();
    let expected_panel_order = PANEL_ORDER
        .iter()
        .map(|value| (*value).to_string())
        .collect::<Vec<_>>();
    if request.requested_view_order != expected_view_order
        || request.requested_panel_order != expected_panel_order
        || request.budget_units == 0
        || request.request.replay_identity != request.replay_identity
        || request.request.policy_allow != request.policy_allow
        || request.request.protected_closure != request.protected_closure
        || request.request.raw_data_local != request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(FederatedWorkbenchError::Invalid("federated workbench identity, canonical views, panels, budget, replay, policy, locality, or boundary is incomplete".into()));
    }
    for (value, field) in [
        (&request.request.request_id, "request_id"),
        (&request.workspace_id, "workspace_id"),
        (&request.request.federation_id, "federation_id"),
        (&request.request.institution_id, "institution_id"),
        (&request.request.purpose, "purpose"),
        (&request.request.semantic_profile, "semantic_profile"),
        (&request.request.endpoint, "endpoint"),
        (&request.boundary, "boundary"),
    ] {
        validate_text(value, field)?;
    }
    validate_unique(&request.request.allowed_artifacts, "allowed_artifacts")?;
    if request.replay_identity.as_str().len() != 64 {
        return Err(FederatedWorkbenchError::Invalid(
            "federated workbench replay digest is invalid".into(),
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
    fn request(state: EvidenceState) -> FederatedWorkbenchRequest {
        FederatedWorkbenchRequest {
            request: FederatedEvidenceFeedRequest {
                request_id: "request:federated-workbench".into(),
                federation_id: "federation:commons".into(),
                institution_id: "institution:a".into(),
                purpose: "benchmarking".into(),
                semantic_profile: "preclinical-evidence/v1".into(),
                endpoint: "https://hub.example/research".into(),
                allowed_artifacts: vec!["qualified-evidence-summary".into()],
                observations: vec![EvidenceObservation {
                    evidence_id: "evidence:a".into(),
                    source_id: "source:a".into(),
                    study_id: "study:organoid".into(),
                    modality: "imaging".into(),
                    scope: "organoid:neural".into(),
                    relevance_milli: 900,
                    state,
                    semantic_digest: hash("semantic"),
                    artifact_digest: hash("artifact"),
                    provenance_digest: hash("provenance"),
                    replay_identity: hash("replay"),
                    omissions: Vec::new(),
                    negative_evidence: Vec::new(),
                    raw_data_local: true,
                    boundary: PRECLINICAL_BOUNDARY.into(),
                }],
                replay_identity: hash("replay"),
                policy_allow: true,
                protected_closure: true,
                signer_valid: true,
                raw_data_local: true,
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            workspace_id: "workspace:federated".into(),
            requested_view_order: VIEW_ORDER
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            requested_panel_order: vec![
                "panel:aggregate".into(),
                "panel:institutions".into(),
                "panel:lineage".into(),
            ],
            replay_identity: hash("replay"),
            budget_units: 8,
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a0_read_only() {
        let manifest = federated_research_workbench_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A0);
    }
    #[test]
    fn aggregate_exchange_is_visible() {
        let receipt =
            compile_federated_research_workbench(&request(EvidenceState::Supported)).unwrap();
        assert_eq!(receipt.disposition, FederatedEvidenceDisposition::Qualified);
        assert!(receipt.effect_receipts[0].starts_with("view:"));
    }
    #[test]
    fn signer_denial_is_visible_and_blocked() {
        let mut input = request(EvidenceState::Supported);
        input.request.signer_valid = false;
        let receipt = compile_federated_research_workbench(&input).unwrap();
        assert_eq!(receipt.disposition, FederatedEvidenceDisposition::Blocked);
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|item| item.contains("signer")));
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn policy_denial_blocks_workbench() {
        let mut input = request(EvidenceState::Supported);
        input.policy_allow = false;
        input.request.policy_allow = false;
        let receipt = compile_federated_research_workbench(&input).unwrap();
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn locality_failure_is_blocked_and_retained() {
        let mut input = request(EvidenceState::Supported);
        input.raw_data_local = false;
        input.request.raw_data_local = false;
        let receipt = compile_federated_research_workbench(&input).unwrap();
        assert_eq!(receipt.disposition, FederatedEvidenceDisposition::Blocked);
        assert!(receipt.raw_data_local);
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item == "workbench:raw-data-locality-failed"));
        receipt.validate().unwrap();
    }
    #[test]
    fn plan_and_payload_drift_are_rejected() {
        let receipt =
            compile_federated_research_workbench(&request(EvidenceState::Supported)).unwrap();
        let mut plan_drift = receipt.clone();
        plan_drift.panel_order.reverse();
        assert!(plan_drift.validate().is_err());

        let mut payload_drift = receipt;
        payload_drift.endpoint = "https://other.example/research".into();
        assert!(payload_drift.validate().is_err());
    }
    #[test]
    fn padded_workspace_identity_is_rejected() {
        let mut input = request(EvidenceState::Supported);
        input.workspace_id = " workspace:federated".into();
        assert!(compile_federated_research_workbench(&input).is_err());
    }
    #[test]
    fn replay_mismatch_is_rejected() {
        let mut input = request(EvidenceState::Supported);
        input.replay_identity = hash("different");
        assert!(compile_federated_research_workbench(&input).is_err());
    }
    #[test]
    fn digest_is_stable() {
        let receipt =
            compile_federated_research_workbench(&request(EvidenceState::Unknown)).unwrap();
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
}
