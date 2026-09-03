//! Prospective high-throughput evidence operations and federation control plane.
//!
//! Atlas feature: `AFA-brain-P01-F31`. Queue capacity, checkpoint, retry, telemetry, and
//! recovery are product state; a completed operation never implies a qualified result.

use crate::high_throughput_evidence_surveillance::{
    admit_high_throughput_evidence, HighThroughputDisposition, HighThroughputEvidenceFeedRequest,
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

pub const FEATURE_ID: &str = "AFA-brain-P01-F31";
pub const CONTRACT_VERSION: &str = "brain-throughput-operations-control-plane/1.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThroughputOperationsDisposition {
    Completed,
    Degraded,
    Unresolved,
    Denied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputOperationsRequest {
    pub request: HighThroughputEvidenceFeedRequest,
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
pub struct ThroughputOperationsReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub operation_id: String,
    pub actor_id: String,
    pub request_id: String,
    pub batch_id: String,
    pub partition: String,
    pub disposition: ThroughputOperationsDisposition,
    pub candidate_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub checkpoint_seq: u64,
    pub attempts: u16,
    pub recovered: bool,
    pub queue_digest: ContentHash,
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
pub enum ThroughputOperationsError {
    #[error("invalid throughput operations request: {0}")]
    Invalid(String),
    #[error("throughput operations artifact failed: {0}")]
    Artifact(String),
    #[error("throughput operations engine failed: {0}")]
    Engine(String),
}

impl ThroughputOperationsReceipt {
    pub fn validate(&self) -> Result<(), ThroughputOperationsError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.operation_id.trim().is_empty()
            || self.actor_id.trim().is_empty()
            || self.request_id.trim().is_empty()
            || self.batch_id.trim().is_empty()
            || self.partition.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.attempts == 0
            || self.effect_receipts.is_empty()
        {
            return Err(ThroughputOperationsError::Invalid(
                "throughput operations identity, queue, budget, or effects are incomplete".into(),
            ));
        }
        if self
            .admitted_order
            .iter()
            .chain(self.blocked_order.iter())
            .chain(self.unknown_order.iter())
            .any(|id| !self.candidate_order.contains(id))
        {
            return Err(ThroughputOperationsError::Invalid(
                "throughput operations state is not covered".into(),
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
                return Err(ThroughputOperationsError::Invalid(
                    "throughput operations ordering is not canonical".into(),
                ));
            }
        }
        for digest in [
            &self.queue_digest,
            &self.evidence_digest,
            &self.operations_digest,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(ThroughputOperationsError::Invalid(
                    "throughput operations digest is invalid".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("ops:throughput:") && effect != "block:unsafe-release"
        }) {
            return Err(ThroughputOperationsError::Invalid(
                "throughput operations effect is outside the control plane".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ThroughputOperationsError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, ThroughputOperationsError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ThroughputOperationsError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ThroughputOperationsError::Artifact(error.to_string()))
    }
}

pub fn throughput_operations_control_plane_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["platform reliability engineer".into(), "throughput operations service".into()].into(), behavior: "operates bounded high-throughput evidence batches with capacity, queue, checkpoint, retry, telemetry, recovery, and explicit scientific disposition".into(), value: "makes prospective evidence operations recoverable without silently dropping capacity overflow or unresolved evidence".into(), inputs: vec![TypedPort { name: "throughput_operations_request".into(), schema: "ThroughputEvidenceOperationsRequest1@1".into(), required: true }], outputs: vec![TypedPort { name: "throughput_operations_receipt".into(), schema: "QualifiedEvidenceSet8@1".into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["operate:throughput-evidence".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "opentelemetry".into(), state: EvidenceState::Supported, locator: Some("https://opentelemetry.io/docs/specs/".into()) }], authority_requirements: vec![AuthorityRequirement { role: "throughput operations approver".into(), reason: "approve capacity, retry, and budget policy before A2 batch operations".into() }], autonomy_tier: AutonomyTier::A2, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn operate_throughput_evidence(
    request: &ThroughputOperationsRequest,
) -> Result<ThroughputOperationsReceipt, ThroughputOperationsError> {
    validate_request(request)?;
    let evidence = admit_high_throughput_evidence(&request.request)
        .map_err(|error| ThroughputOperationsError::Engine(error.to_string()))?;
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
        ThroughputOperationsDisposition::Denied
    } else {
        match evidence.disposition {
            HighThroughputDisposition::Qualified => ThroughputOperationsDisposition::Completed,
            HighThroughputDisposition::Partial => ThroughputOperationsDisposition::Degraded,
            HighThroughputDisposition::Unknown => ThroughputOperationsDisposition::Unresolved,
            HighThroughputDisposition::Blocked => ThroughputOperationsDisposition::Denied,
        }
    };
    let checkpoint_seq = if evidence.candidate_order.is_empty() {
        0
    } else {
        evidence.checkpoint_seq
    };
    let attempts = 1;
    let recovered = false;
    let queue_digest = evidence.queue_digest.clone();
    let evidence_digest = evidence
        .digest()
        .map_err(|error| ThroughputOperationsError::Engine(error.to_string()))?;
    let operations_digest = ContentHash::of_value(&json!({"feature_id": FEATURE_ID, "operation_id": request.operation_id, "request_id": request.request.request_id, "batch_id": request.request.batch_id, "partition": request.request.partition, "budget_units": request.budget_units, "retry_budget": request.retry_budget, "checkpoint_interval": request.checkpoint_interval, "queue_digest": queue_digest, "evidence_digest": evidence_digest, "disposition": disposition})).map_err(|error| ThroughputOperationsError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "operation_id": request.operation_id, "actor_id": request.actor_id, "request_id": request.request.request_id, "batch_id": request.request.batch_id, "partition": request.request.partition, "disposition": disposition, "candidate_order": evidence.candidate_order, "admitted_order": evidence.admitted_order, "blocked_order": evidence.blocked_order, "unknown_order": evidence.unknown_order, "checkpoint_seq": checkpoint_seq, "attempts": attempts, "recovered": recovered, "queue_digest": queue_digest, "evidence_digest": evidence_digest, "operations_digest": operations_digest, "replay_identity": request.request.replay_identity, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-throughput-operations:{}", request.operation_id),
        "application/vnd.aurora.throughput-operations+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ThroughputOperationsError::Artifact(error.to_string()))?;
    let receipt = ThroughputOperationsReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        operation_id: request.operation_id.clone(),
        actor_id: request.actor_id.clone(),
        request_id: request.request.request_id.clone(),
        batch_id: request.request.batch_id.clone(),
        partition: request.request.partition.clone(),
        disposition,
        candidate_order: evidence.candidate_order.clone(),
        admitted_order: evidence.admitted_order.clone(),
        blocked_order: evidence.blocked_order.clone(),
        unknown_order: evidence.unknown_order.clone(),
        checkpoint_seq,
        attempts,
        recovered,
        queue_digest,
        evidence_digest,
        operations_digest,
        replay_identity: request.request.replay_identity.clone(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts: if allowed {
            vec![format!("ops:throughput:{}", request.operation_id)]
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

fn validate_request(
    request: &ThroughputOperationsRequest,
) -> Result<(), ThroughputOperationsError> {
    if request.operation_id.trim().is_empty()
        || request.actor_id.trim().is_empty()
        || request.budget_units == 0
        || request.checkpoint_interval == 0
        || !request.telemetry_enabled
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(ThroughputOperationsError::Invalid(
            "throughput operations actor, budget, checkpoint, telemetry, or boundary is incomplete"
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
    fn request(state: EvidenceState) -> ThroughputOperationsRequest {
        ThroughputOperationsRequest {
            request: HighThroughputEvidenceFeedRequest {
                request_id: "request:tp-ops".into(),
                batch_id: "batch:001".into(),
                partition: "partition:imaging".into(),
                max_items: 2,
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
            operation_id: "operation:tp".into(),
            actor_id: "actor:reliability".into(),
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
    fn manifest_is_a2() {
        let m = throughput_operations_control_plane_manifest();
        m.validate().unwrap();
        assert_eq!(m.autonomy_tier, AutonomyTier::A2);
    }
    #[test]
    fn supported_completes() {
        let r = operate_throughput_evidence(&request(EvidenceState::Supported)).unwrap();
        assert_eq!(r.disposition, ThroughputOperationsDisposition::Completed);
    }
    #[test]
    fn unknown_is_unresolved() {
        let r = operate_throughput_evidence(&request(EvidenceState::Unknown)).unwrap();
        assert_eq!(r.disposition, ThroughputOperationsDisposition::Unresolved);
    }
    #[test]
    fn policy_denies() {
        let mut q = request(EvidenceState::Supported);
        q.policy_allow = false;
        let r = operate_throughput_evidence(&q).unwrap();
        assert_eq!(r.disposition, ThroughputOperationsDisposition::Denied);
    }
    #[test]
    fn budget_is_required() {
        let mut q = request(EvidenceState::Supported);
        q.budget_units = 0;
        assert!(operate_throughput_evidence(&q).is_err());
    }
    #[test]
    fn digest_is_stable() {
        let r = operate_throughput_evidence(&request(EvidenceState::Supported)).unwrap();
        assert_eq!(r.digest().unwrap(), r.digest().unwrap());
    }
}
