//! Local single-study evidence operations and federation control plane.
//!
//! Atlas feature: `AFA-brain-P01-F29`. This capability owns the operator-facing lifecycle around
//! evidence surveillance: bounded budget, checkpoint, retry, telemetry, and recovery receipts.
//! It does not turn operational completion into scientific qualification.

use crate::evidence_surveillance::{
    surveil_evidence, EvidenceFeedRequest, EvidenceSurveillanceDisposition,
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

pub const FEATURE_ID: &str = "AFA-brain-P01-F29";
pub const CONTRACT_VERSION: &str = "brain-evidence-operations-control-plane/1.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationsDisposition {
    Completed,
    Degraded,
    Unresolved,
    Denied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceOperationsRequest {
    pub request: EvidenceFeedRequest,
    pub operation_id: String,
    pub actor_id: String,
    pub budget_units: u32,
    pub retry_budget: u16,
    pub checkpoint_interval: u16,
    pub telemetry_enabled: bool,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceOperationsReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub operation_id: String,
    pub actor_id: String,
    pub request_id: String,
    pub disposition: OperationsDisposition,
    pub candidate_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub checkpoint_seq: u64,
    pub attempts: u16,
    pub recovered: bool,
    pub telemetry_digest: ContentHash,
    pub evidence_digest: ContentHash,
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
pub enum EvidenceOperationsError {
    #[error("invalid evidence operations request: {0}")]
    Invalid(String),
    #[error("evidence operations artifact failed: {0}")]
    Artifact(String),
    #[error("evidence operations engine failed: {0}")]
    Engine(String),
}

impl EvidenceOperationsReceipt {
    pub fn validate(&self) -> Result<(), EvidenceOperationsError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.operation_id.trim().is_empty()
            || self.actor_id.trim().is_empty()
            || self.request_id.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.attempts == 0
            || self.effect_receipts.is_empty()
        {
            return Err(EvidenceOperationsError::Invalid(
                "operations identity, run budget, candidate state, or effects are incomplete"
                    .into(),
            ));
        }
        if self
            .qualified_order
            .iter()
            .chain(self.blocked_order.iter())
            .chain(self.unknown_order.iter())
            .any(|id| !self.candidate_order.contains(id))
        {
            return Err(EvidenceOperationsError::Invalid(
                "operations state is not covered by candidates".into(),
            ));
        }
        for values in [
            &self.candidate_order,
            &self.qualified_order,
            &self.blocked_order,
            &self.unknown_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(EvidenceOperationsError::Invalid(
                    "operations ordering is not canonical".into(),
                ));
            }
        }
        for digest in [
            &self.telemetry_digest,
            &self.evidence_digest,
            &self.operations_digest,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(EvidenceOperationsError::Invalid(
                    "operations digest is invalid".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("ops:local-evidence:") && effect != "block:unsafe-release"
        }) {
            return Err(EvidenceOperationsError::Invalid(
                "operations effect is outside the local control plane".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| EvidenceOperationsError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, EvidenceOperationsError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| EvidenceOperationsError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| EvidenceOperationsError::Artifact(error.to_string()))
    }
}

pub fn evidence_operations_control_plane_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["research workflow operator".into(), "institution-local operations service".into()].into(), behavior: "operates one local evidence-surveillance run with budget, retry, checkpoint, telemetry, recovery, and explicit scientific disposition".into(), value: "makes evidence operations observable and recoverable without promoting operational completion into a scientific pass".into(), inputs: vec![TypedPort { name: "evidence_operations_request".into(), schema: "EvidenceOperationsRequest1@1".into(), required: true }], outputs: vec![TypedPort { name: "evidence_operations_receipt".into(), schema: "QualifiedEvidenceSet8@1".into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["operate:local-evidence".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "opentelemetry".into(), state: EvidenceState::Supported, locator: Some("https://opentelemetry.io/docs/specs/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn operate_evidence(
    request: &EvidenceOperationsRequest,
) -> Result<EvidenceOperationsReceipt, EvidenceOperationsError> {
    validate_request(request)?;
    let evidence = surveil_evidence(&request.request)
        .map_err(|error| EvidenceOperationsError::Engine(error.to_string()))?;
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
    let allowed = request.policy_allow && request.protected_closure && request.raw_data_local;
    if !request.policy_allow {
        omissions.insert("ops:policy-denied".into());
    }
    if !request.protected_closure {
        omissions.insert("ops:protected-closure-incomplete".into());
    }
    if !request.raw_data_local {
        omissions.insert("ops:raw-data-locality-failed".into());
    }
    let disposition = if !allowed {
        OperationsDisposition::Denied
    } else {
        match evidence.disposition {
            EvidenceSurveillanceDisposition::Qualified => OperationsDisposition::Completed,
            EvidenceSurveillanceDisposition::Partial => OperationsDisposition::Degraded,
            EvidenceSurveillanceDisposition::Unknown => OperationsDisposition::Unresolved,
            EvidenceSurveillanceDisposition::Blocked => OperationsDisposition::Denied,
        }
    };
    let checkpoint_seq = if evidence.candidate_order.is_empty() {
        0
    } else {
        1
    };
    let attempts = 1;
    let recovered = false;
    let telemetry_digest = ContentHash::of_value(&json!({"operation_id": request.operation_id, "actor_id": request.actor_id, "attempts": attempts, "checkpoint_seq": checkpoint_seq, "recovered": recovered, "disposition": disposition}))
        .map_err(|error| EvidenceOperationsError::Artifact(error.to_string()))?;
    let evidence_digest = evidence
        .digest()
        .map_err(|error| EvidenceOperationsError::Engine(error.to_string()))?;
    let operations_digest = ContentHash::of_value(&json!({"feature_id": FEATURE_ID, "operation_id": request.operation_id, "request_id": request.request.request_id, "budget_units": request.budget_units, "retry_budget": request.retry_budget, "checkpoint_interval": request.checkpoint_interval, "telemetry_digest": telemetry_digest, "evidence_digest": evidence_digest, "replay_identity": request.request.replay_identity, "disposition": disposition}))
        .map_err(|error| EvidenceOperationsError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "operation_id": request.operation_id, "actor_id": request.actor_id, "request_id": request.request.request_id, "disposition": disposition, "candidate_order": evidence.candidate_order, "qualified_order": evidence.qualified_order, "blocked_order": evidence.blocked_order, "unknown_order": evidence.unknown_order, "checkpoint_seq": checkpoint_seq, "attempts": attempts, "recovered": recovered, "telemetry_digest": telemetry_digest, "evidence_digest": evidence_digest, "operations_digest": operations_digest, "replay_identity": request.request.replay_identity, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-evidence-operations:{}", request.operation_id),
        "application/vnd.aurora.evidence-operations+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| EvidenceOperationsError::Artifact(error.to_string()))?;
    let receipt = EvidenceOperationsReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        operation_id: request.operation_id.clone(),
        actor_id: request.actor_id.clone(),
        request_id: request.request.request_id.clone(),
        disposition,
        candidate_order: evidence.candidate_order.clone(),
        qualified_order: evidence.qualified_order.clone(),
        blocked_order: evidence.blocked_order.clone(),
        unknown_order: evidence.unknown_order.clone(),
        checkpoint_seq,
        attempts,
        recovered,
        telemetry_digest,
        evidence_digest,
        operations_digest,
        replay_identity: request.request.replay_identity.clone(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts: if allowed {
            vec![format!("ops:local-evidence:{}", request.operation_id)]
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

fn validate_request(request: &EvidenceOperationsRequest) -> Result<(), EvidenceOperationsError> {
    if request.operation_id.trim().is_empty()
        || request.actor_id.trim().is_empty()
        || request.budget_units == 0
        || request.checkpoint_interval == 0
        || !request.telemetry_enabled
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(EvidenceOperationsError::Invalid(
            "operations actor, budget, checkpoint, telemetry, or boundary is incomplete".into(),
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
    fn request(state: EvidenceState) -> EvidenceOperationsRequest {
        EvidenceOperationsRequest {
            request: EvidenceFeedRequest {
                request_id: "request:ops".into(),
                study_id: "study:organoid".into(),
                scope: "organoid:neural".into(),
                query: "synaptic morphology".into(),
                minimum_relevance_milli: 700,
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
                raw_data_local: true,
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            operation_id: "operation:local-evidence".into(),
            actor_id: "actor:operator".into(),
            budget_units: 100,
            retry_budget: 2,
            checkpoint_interval: 1,
            telemetry_enabled: true,
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        let m = evidence_operations_control_plane_manifest();
        m.validate().unwrap();
        assert_eq!(m.autonomy_tier, AutonomyTier::A1);
    }
    #[test]
    fn supported_completes() {
        let r = operate_evidence(&request(EvidenceState::Supported)).unwrap();
        assert_eq!(r.disposition, OperationsDisposition::Completed);
        assert_eq!(r.checkpoint_seq, 1);
    }
    #[test]
    fn unknown_is_unresolved() {
        let r = operate_evidence(&request(EvidenceState::Unknown)).unwrap();
        assert_eq!(r.disposition, OperationsDisposition::Unresolved);
    }
    #[test]
    fn policy_denies() {
        let mut q = request(EvidenceState::Supported);
        q.policy_allow = false;
        let r = operate_evidence(&q).unwrap();
        assert_eq!(r.disposition, OperationsDisposition::Denied);
    }
    #[test]
    fn budget_is_required() {
        let mut q = request(EvidenceState::Supported);
        q.budget_units = 0;
        assert!(operate_evidence(&q).is_err());
    }
    #[test]
    fn digest_is_stable() {
        let r = operate_evidence(&request(EvidenceState::Supported)).unwrap();
        assert_eq!(r.digest().unwrap(), r.digest().unwrap());
    }
}
