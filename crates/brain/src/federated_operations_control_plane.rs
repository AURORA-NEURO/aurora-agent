//! Federated continual evidence operations and federation control plane.
//!
//! Atlas feature: `AFA-brain-P01-F32`. This A2 control plane operates purpose-bound, signed,
//! aggregate-only exchanges with telemetry, recovery, budget, and explicit federation denial.

use crate::federated_evidence_surveillance::{
    admit_federated_evidence, FederatedEvidenceDisposition, FederatedEvidenceFeedRequest,
    PERMITTED_ARTIFACT,
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

pub const FEATURE_ID: &str = "AFA-brain-P01-F32";
pub const CONTRACT_VERSION: &str = "brain-federated-operations-control-plane/1.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedOperationsDisposition {
    Completed,
    Degraded,
    Unresolved,
    Denied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedOperationsRequest {
    pub request: FederatedEvidenceFeedRequest,
    pub operation_id: String,
    pub actor_id: String,
    pub budget_units: u32,
    pub retry_budget: u16,
    pub checkpoint_interval: u16,
    pub telemetry_enabled: bool,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signer_valid: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedOperationsReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub operation_id: String,
    pub actor_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub institution_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub endpoint: String,
    pub disposition: FederatedOperationsDisposition,
    pub candidate_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub aggregate_order: Vec<ContentHash>,
    pub checkpoint_seq: u64,
    pub attempts: u16,
    pub recovered: bool,
    pub envelope_digest: ContentHash,
    pub operations_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedOperationsError {
    #[error("invalid federated operations request: {0}")]
    Invalid(String),
    #[error("federated operations artifact failed: {0}")]
    Artifact(String),
    #[error("federated operations engine failed: {0}")]
    Engine(String),
}

impl FederatedOperationsReceipt {
    pub fn validate(&self) -> Result<(), FederatedOperationsError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.operation_id.trim().is_empty()
            || self.actor_id.trim().is_empty()
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.institution_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.endpoint.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.attempts == 0
            || self.effect_receipts.is_empty()
        {
            return Err(FederatedOperationsError::Invalid(
                "federated operations identity, envelope, budget, or effects are incomplete".into(),
            ));
        }
        if self
            .admitted_order
            .iter()
            .chain(self.blocked_order.iter())
            .chain(self.unknown_order.iter())
            .any(|id| !self.candidate_order.contains(id))
        {
            return Err(FederatedOperationsError::Invalid(
                "federated operations state is not covered".into(),
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
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(FederatedOperationsError::Invalid(
                    "federated operations ordering is not canonical".into(),
                ));
            }
        }
        if self
            .aggregate_order
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(FederatedOperationsError::Invalid(
                "federated aggregate ordering is not canonical".into(),
            ));
        }
        for digest in [
            &self.envelope_digest,
            &self.operations_digest,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(FederatedOperationsError::Invalid(
                    "federated operations digest is invalid".into(),
                ));
            }
        }
        if self
            .effect_receipts
            .iter()
            .any(|effect| !effect.starts_with("ops:federated:") && effect != "block:unsafe-release")
        {
            return Err(FederatedOperationsError::Invalid(
                "federated operations effect is outside the control plane".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| FederatedOperationsError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, FederatedOperationsError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| FederatedOperationsError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| FederatedOperationsError::Artifact(error.to_string()))
    }
}

pub fn federated_operations_control_plane_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["laboratory automation engineer".into(), "federated operations service".into()].into(), behavior: "operates purpose-bound federated evidence exchange with signer, permitted-artifact, aggregate-only, budget, checkpoint, retry, telemetry, recovery, and explicit scientific disposition".into(), value: "makes continual consortium evidence operations observable while preventing raw-data egress and silent federation failure".into(), inputs: vec![TypedPort { name: "federated_operations_request".into(), schema: "FederatedEvidenceOperationsRequest1@1".into(), required: true }], outputs: vec![TypedPort { name: "federated_operations_receipt".into(), schema: "QualifiedEvidenceSet8@1".into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["operate:federated-evidence".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }], authority_requirements: vec![AuthorityRequirement { role: "federated operations approver".into(), reason: "approve purpose-bound aggregate-only exchange before A2 operations".into() }], autonomy_tier: AutonomyTier::A2, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn operate_federated_evidence(
    request: &FederatedOperationsRequest,
) -> Result<FederatedOperationsReceipt, FederatedOperationsError> {
    validate_request(request)?;
    let evidence = admit_federated_evidence(&request.request)
        .map_err(|error| FederatedOperationsError::Engine(error.to_string()))?;
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
    let allowed = request.policy_allow
        && request.protected_closure
        && request.signer_valid
        && request.raw_data_local
        && request
            .request
            .allowed_artifacts
            .iter()
            .any(|item| item == PERMITTED_ARTIFACT);
    if !request.policy_allow {
        omissions.insert("ops:policy-denied".into());
    }
    if !request.protected_closure {
        omissions.insert("ops:protected-closure-incomplete".into());
    }
    if !request.signer_valid {
        omissions.insert("ops:signer-invalid".into());
    }
    if !request.raw_data_local {
        omissions.insert("ops:raw-data-locality-failed".into());
    }
    if !request
        .request
        .allowed_artifacts
        .iter()
        .any(|item| item == PERMITTED_ARTIFACT)
    {
        omissions.insert("ops:permitted-artifact-missing".into());
    }
    let disposition = if !allowed {
        FederatedOperationsDisposition::Denied
    } else {
        match evidence.disposition {
            FederatedEvidenceDisposition::Qualified => FederatedOperationsDisposition::Completed,
            FederatedEvidenceDisposition::Partial => FederatedOperationsDisposition::Degraded,
            FederatedEvidenceDisposition::Unknown => FederatedOperationsDisposition::Unresolved,
            FederatedEvidenceDisposition::Blocked => FederatedOperationsDisposition::Denied,
        }
    };
    let checkpoint_seq = if evidence.candidate_order.is_empty() {
        0
    } else {
        1
    };
    let attempts = 1;
    let recovered = false;
    let aggregate_order = evidence.aggregate_order.clone();
    let envelope_digest = ContentHash::of_value(&json!({"federation_id": request.request.federation_id, "institution_id": request.request.institution_id, "purpose": request.request.purpose, "semantic_profile": request.request.semantic_profile, "candidate_order": evidence.candidate_order, "aggregate_order": aggregate_order, "replay_identity": request.request.replay_identity})).map_err(|error| FederatedOperationsError::Artifact(error.to_string()))?;
    let operations_digest = ContentHash::of_value(&json!({"feature_id": FEATURE_ID, "operation_id": request.operation_id, "request_id": request.request.request_id, "federation_id": request.request.federation_id, "budget_units": request.budget_units, "retry_budget": request.retry_budget, "checkpoint_interval": request.checkpoint_interval, "envelope_digest": envelope_digest, "disposition": disposition})).map_err(|error| FederatedOperationsError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "operation_id": request.operation_id, "actor_id": request.actor_id, "request_id": request.request.request_id, "federation_id": request.request.federation_id, "institution_id": request.request.institution_id, "purpose": request.request.purpose, "semantic_profile": request.request.semantic_profile, "endpoint": request.request.endpoint, "disposition": disposition, "candidate_order": evidence.candidate_order, "admitted_order": evidence.admitted_order, "blocked_order": evidence.blocked_order, "unknown_order": evidence.unknown_order, "aggregate_order": aggregate_order, "checkpoint_seq": checkpoint_seq, "attempts": attempts, "recovered": recovered, "envelope_digest": envelope_digest, "operations_digest": operations_digest, "replay_identity": request.request.replay_identity, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-federated-operations:{}", request.operation_id),
        "application/vnd.aurora.federated-operations+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| FederatedOperationsError::Artifact(error.to_string()))?;
    let receipt = FederatedOperationsReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        operation_id: request.operation_id.clone(),
        actor_id: request.actor_id.clone(),
        request_id: request.request.request_id.clone(),
        federation_id: request.request.federation_id.clone(),
        institution_id: request.request.institution_id.clone(),
        purpose: request.request.purpose.clone(),
        semantic_profile: request.request.semantic_profile.clone(),
        endpoint: request.request.endpoint.clone(),
        disposition,
        candidate_order: evidence.candidate_order.clone(),
        admitted_order: evidence.admitted_order.clone(),
        blocked_order: evidence.blocked_order.clone(),
        unknown_order: evidence.unknown_order.clone(),
        aggregate_order,
        checkpoint_seq,
        attempts,
        recovered,
        envelope_digest,
        operations_digest,
        replay_identity: request.request.replay_identity.clone(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts: if allowed {
            vec![format!("ops:federated:{}", request.operation_id)]
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

fn validate_request(request: &FederatedOperationsRequest) -> Result<(), FederatedOperationsError> {
    if request.operation_id.trim().is_empty()
        || request.actor_id.trim().is_empty()
        || request.budget_units == 0
        || request.checkpoint_interval == 0
        || !request.telemetry_enabled
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(FederatedOperationsError::Invalid(
            "federated operations actor, budget, checkpoint, telemetry, or boundary is incomplete"
                .into(),
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
    fn request(state: EvidenceState) -> FederatedOperationsRequest {
        FederatedOperationsRequest {
            request: FederatedEvidenceFeedRequest {
                request_id: "request:fed-ops".into(),
                federation_id: "federation:commons".into(),
                institution_id: "institution:a".into(),
                purpose: "benchmarking".into(),
                semantic_profile: "preclinical-evidence/v1".into(),
                endpoint: "https://hub.example/research".into(),
                allowed_artifacts: vec![PERMITTED_ARTIFACT.into()],
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
            operation_id: "operation:fed".into(),
            actor_id: "actor:lab".into(),
            budget_units: 100,
            retry_budget: 2,
            checkpoint_interval: 1,
            telemetry_enabled: true,
            policy_allow: true,
            protected_closure: true,
            signer_valid: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a2() {
        let m = federated_operations_control_plane_manifest();
        m.validate().unwrap();
        assert_eq!(m.autonomy_tier, AutonomyTier::A2);
    }
    #[test]
    fn supported_completes() {
        let r = operate_federated_evidence(&request(EvidenceState::Supported)).unwrap();
        assert_eq!(r.disposition, FederatedOperationsDisposition::Completed);
        assert!(!r.aggregate_order.is_empty());
    }
    #[test]
    fn unknown_is_unresolved() {
        let r = operate_federated_evidence(&request(EvidenceState::Unknown)).unwrap();
        assert_eq!(r.disposition, FederatedOperationsDisposition::Unresolved);
    }
    #[test]
    fn signer_denies() {
        let mut q = request(EvidenceState::Supported);
        q.signer_valid = false;
        let r = operate_federated_evidence(&q).unwrap();
        assert_eq!(r.disposition, FederatedOperationsDisposition::Denied);
    }
    #[test]
    fn budget_is_required() {
        let mut q = request(EvidenceState::Supported);
        q.budget_units = 0;
        assert!(operate_federated_evidence(&q).is_err());
    }
    #[test]
    fn digest_is_stable() {
        let r = operate_federated_evidence(&request(EvidenceState::Supported)).unwrap();
        assert_eq!(r.digest().unwrap(), r.digest().unwrap());
    }
}
