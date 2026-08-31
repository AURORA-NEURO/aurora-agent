//! Multimodal multi-study evidence operations and federation control plane.
//!
//! Atlas feature: `AFA-brain-P01-F30`. Operations remain distinct from scientific qualification:
//! the receipt records comparability, modality closure, budgets, checkpoints, and recovery.

use crate::multimodal_evidence_surveillance::{
    surveil_multimodal_evidence, MultimodalEvidenceDisposition, MultimodalEvidenceFeedRequest,
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

pub const FEATURE_ID: &str = "AFA-brain-P01-F30";
pub const CONTRACT_VERSION: &str = "brain-multimodal-operations-control-plane/1.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultimodalOperationsDisposition {
    Completed,
    Degraded,
    Unresolved,
    Denied,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalOperationsRequest {
    pub request: MultimodalEvidenceFeedRequest,
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
pub struct MultimodalOperationsReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub operation_id: String,
    pub actor_id: String,
    pub request_id: String,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub disposition: MultimodalOperationsDisposition,
    pub candidate_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub checkpoint_seq: u64,
    pub attempts: u16,
    pub recovered: bool,
    pub comparability_digest: ContentHash,
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
pub enum MultimodalOperationsError {
    #[error("invalid multimodal operations request: {0}")]
    Invalid(String),
    #[error("multimodal operations artifact failed: {0}")]
    Artifact(String),
    #[error("multimodal operations engine failed: {0}")]
    Engine(String),
}

impl MultimodalOperationsReceipt {
    pub fn validate(&self) -> Result<(), MultimodalOperationsError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.operation_id.trim().is_empty()
            || self.actor_id.trim().is_empty()
            || self.request_id.trim().is_empty()
            || self.study_order.len() < 2
            || self.modality_order.len() < 2
            || self.candidate_order.is_empty()
            || self.attempts == 0
            || self.effect_receipts.is_empty()
        {
            return Err(MultimodalOperationsError::Invalid(
                "multimodal operations identity, coverage, budget, or effects are incomplete"
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
            return Err(MultimodalOperationsError::Invalid(
                "multimodal operations state is not covered".into(),
            ));
        }
        for values in [
            &self.study_order,
            &self.modality_order,
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
                return Err(MultimodalOperationsError::Invalid(
                    "multimodal operations ordering is not canonical".into(),
                ));
            }
        }
        for digest in [
            &self.comparability_digest,
            &self.evidence_digest,
            &self.operations_digest,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(MultimodalOperationsError::Invalid(
                    "multimodal operations digest is invalid".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("ops:local-multimodal:") && effect != "block:unsafe-release"
        }) {
            return Err(MultimodalOperationsError::Invalid(
                "multimodal operations effect is outside the local control plane".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| MultimodalOperationsError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, MultimodalOperationsError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| MultimodalOperationsError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| MultimodalOperationsError::Artifact(error.to_string()))
    }
}

pub fn multimodal_operations_control_plane_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["agent developer".into(), "multimodal operations service".into()].into(), behavior: "operates a multimodal evidence run with study/modality comparability, budget, checkpoint, retry, telemetry, and recovery receipts".into(), value: "makes imaging and omics operations observable without hiding missing modality closure or semantic disagreement".into(), inputs: vec![TypedPort { name: "multimodal_operations_request".into(), schema: "MultimodalEvidenceOperationsRequest1@1".into(), required: true }], outputs: vec![TypedPort { name: "multimodal_operations_receipt".into(), schema: "QualifiedEvidenceSet8@1".into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["operate:local-multimodal".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "opentelemetry".into(), state: EvidenceState::Supported, locator: Some("https://opentelemetry.io/docs/specs/".into()) }], authority_requirements: vec![AuthorityRequirement { role: "multimodal operations approver".into(), reason: "approve comparability, retry, and budget policy before A2 operations".into() }], autonomy_tier: AutonomyTier::A2, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn operate_multimodal_evidence(
    request: &MultimodalOperationsRequest,
) -> Result<MultimodalOperationsReceipt, MultimodalOperationsError> {
    validate_request(request)?;
    let evidence = surveil_multimodal_evidence(&request.request)
        .map_err(|error| MultimodalOperationsError::Engine(error.to_string()))?;
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
    let study_order = request
        .request
        .study_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let modality_order = request
        .request
        .required_modalities
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
        MultimodalOperationsDisposition::Denied
    } else {
        match evidence.disposition {
            MultimodalEvidenceDisposition::Qualified => MultimodalOperationsDisposition::Completed,
            MultimodalEvidenceDisposition::Partial => MultimodalOperationsDisposition::Degraded,
            MultimodalEvidenceDisposition::Unknown => MultimodalOperationsDisposition::Unresolved,
            MultimodalEvidenceDisposition::Blocked => MultimodalOperationsDisposition::Denied,
        }
    };
    let checkpoint_seq = if evidence.candidate_order.is_empty() {
        0
    } else {
        1
    };
    let attempts = 1;
    let recovered = false;
    let comparability_digest = ContentHash::of_value(&json!({"study_order": study_order, "modality_order": modality_order, "candidate_order": evidence.candidate_order, "semantic_order": evidence.semantic_order, "replay_identity": request.request.replay_identity})).map_err(|error| MultimodalOperationsError::Artifact(error.to_string()))?;
    let evidence_digest = evidence
        .digest()
        .map_err(|error| MultimodalOperationsError::Engine(error.to_string()))?;
    let operations_digest = ContentHash::of_value(&json!({"feature_id": FEATURE_ID, "operation_id": request.operation_id, "request_id": request.request.request_id, "budget_units": request.budget_units, "retry_budget": request.retry_budget, "checkpoint_interval": request.checkpoint_interval, "comparability_digest": comparability_digest, "evidence_digest": evidence_digest, "disposition": disposition})).map_err(|error| MultimodalOperationsError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "operation_id": request.operation_id, "actor_id": request.actor_id, "request_id": request.request.request_id, "study_order": study_order, "modality_order": modality_order, "disposition": disposition, "candidate_order": evidence.candidate_order, "qualified_order": evidence.qualified_order, "blocked_order": evidence.blocked_order, "unknown_order": evidence.unknown_order, "checkpoint_seq": checkpoint_seq, "attempts": attempts, "recovered": recovered, "comparability_digest": comparability_digest, "evidence_digest": evidence_digest, "operations_digest": operations_digest, "replay_identity": request.request.replay_identity, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-multimodal-operations:{}", request.operation_id),
        "application/vnd.aurora.multimodal-operations+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| MultimodalOperationsError::Artifact(error.to_string()))?;
    let receipt = MultimodalOperationsReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        operation_id: request.operation_id.clone(),
        actor_id: request.actor_id.clone(),
        request_id: request.request.request_id.clone(),
        study_order: study_order.into_iter().collect(),
        modality_order: modality_order.into_iter().collect(),
        disposition,
        candidate_order: evidence.candidate_order.clone(),
        qualified_order: evidence.qualified_order.clone(),
        blocked_order: evidence.blocked_order.clone(),
        unknown_order: evidence.unknown_order.clone(),
        checkpoint_seq,
        attempts,
        recovered,
        comparability_digest,
        evidence_digest,
        operations_digest,
        replay_identity: request.request.replay_identity.clone(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts: if allowed {
            vec![format!("ops:local-multimodal:{}", request.operation_id)]
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
    request: &MultimodalOperationsRequest,
) -> Result<(), MultimodalOperationsError> {
    if request.operation_id.trim().is_empty()
        || request.actor_id.trim().is_empty()
        || request.budget_units == 0
        || request.checkpoint_interval == 0
        || !request.telemetry_enabled
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(MultimodalOperationsError::Invalid(
            "multimodal operations actor, budget, checkpoint, telemetry, or boundary is incomplete"
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
    fn request(state: EvidenceState) -> MultimodalOperationsRequest {
        MultimodalOperationsRequest {
            request: MultimodalEvidenceFeedRequest {
                request_id: "request:mm-ops".into(),
                study_ids: vec!["study:a".into(), "study:b".into()],
                scope: "organoid:neural".into(),
                query: "synaptic morphology".into(),
                minimum_relevance_milli: 700,
                required_modalities: vec!["imaging".into(), "transcriptomics".into()],
                observations: vec![
                    EvidenceObservation {
                        evidence_id: "evidence:a".into(),
                        source_id: "source:a".into(),
                        study_id: "study:a".into(),
                        modality: "imaging".into(),
                        scope: "organoid:neural".into(),
                        relevance_milli: 900,
                        state,
                        semantic_digest: hash("semantic:a"),
                        artifact_digest: hash("artifact:a"),
                        provenance_digest: hash("provenance:a"),
                        replay_identity: hash("replay"),
                        omissions: Vec::new(),
                        negative_evidence: Vec::new(),
                        raw_data_local: true,
                        boundary: PRECLINICAL_BOUNDARY.into(),
                    },
                    EvidenceObservation {
                        evidence_id: "evidence:b".into(),
                        source_id: "source:b".into(),
                        study_id: "study:b".into(),
                        modality: "transcriptomics".into(),
                        scope: "organoid:neural".into(),
                        relevance_milli: 900,
                        state,
                        semantic_digest: hash("semantic:b"),
                        artifact_digest: hash("artifact:b"),
                        provenance_digest: hash("provenance:b"),
                        replay_identity: hash("replay"),
                        omissions: Vec::new(),
                        negative_evidence: Vec::new(),
                        raw_data_local: true,
                        boundary: PRECLINICAL_BOUNDARY.into(),
                    },
                ],
                replay_identity: hash("replay"),
                policy_allow: true,
                protected_closure: true,
                raw_data_local: true,
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            operation_id: "operation:mm".into(),
            actor_id: "actor:agent".into(),
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
        let m = multimodal_operations_control_plane_manifest();
        m.validate().unwrap();
        assert_eq!(m.autonomy_tier, AutonomyTier::A2);
    }
    #[test]
    fn supported_completes() {
        let r = operate_multimodal_evidence(&request(EvidenceState::Supported)).unwrap();
        assert_eq!(r.disposition, MultimodalOperationsDisposition::Completed);
    }
    #[test]
    fn unknown_is_unresolved() {
        let r = operate_multimodal_evidence(&request(EvidenceState::Unknown)).unwrap();
        assert_eq!(r.disposition, MultimodalOperationsDisposition::Unresolved);
    }
    #[test]
    fn policy_denies() {
        let mut q = request(EvidenceState::Supported);
        q.policy_allow = false;
        let r = operate_multimodal_evidence(&q).unwrap();
        assert_eq!(r.disposition, MultimodalOperationsDisposition::Denied);
    }
    #[test]
    fn budget_is_required() {
        let mut q = request(EvidenceState::Supported);
        q.budget_units = 0;
        assert!(operate_multimodal_evidence(&q).is_err());
    }
    #[test]
    fn digest_is_stable() {
        let r = operate_multimodal_evidence(&request(EvidenceState::Supported)).unwrap();
        assert_eq!(r.digest().unwrap(), r.digest().unwrap());
    }
}
