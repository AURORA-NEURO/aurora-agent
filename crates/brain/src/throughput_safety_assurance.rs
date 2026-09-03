//! Prospective high-throughput evidence verification and safety assurance harness.
//!
//! Atlas feature: `AFA-brain-P01-F27`. Capacity, checkpoint, replay, and provenance witnesses
//! are part of the product result; overflow is never silently discarded.

use crate::high_throughput_evidence_surveillance::{
    admit_high_throughput_evidence, HighThroughputDisposition, HighThroughputEvidenceFeedRequest,
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

pub const FEATURE_ID: &str = "AFA-brain-P01-F27";
pub const CONTRACT_VERSION: &str = "brain-throughput-evidence-assurance/1.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThroughputAssuranceVerdict {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputAssuranceReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub batch_id: String,
    pub partition: String,
    pub verdict: ThroughputAssuranceVerdict,
    pub candidate_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub witness_order: Vec<String>,
    pub counterexample_order: Vec<String>,
    pub checkpoint_seq: u64,
    pub queue_digest: ContentHash,
    pub evidence_digest: ContentHash,
    pub verification_digest: ContentHash,
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
pub enum ThroughputAssuranceError {
    #[error("invalid throughput assurance request: {0}")]
    Invalid(String),
    #[error("throughput assurance artifact failed: {0}")]
    Artifact(String),
    #[error("throughput assurance engine failed: {0}")]
    Engine(String),
}

impl ThroughputAssuranceReceipt {
    pub fn validate(&self) -> Result<(), ThroughputAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.batch_id.trim().is_empty()
            || self.partition.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.witness_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(ThroughputAssuranceError::Invalid("throughput assurance identity, witness coverage, locality, or effects are incomplete".into()));
        }
        if self
            .admitted_order
            .iter()
            .chain(self.blocked_order.iter())
            .chain(self.unknown_order.iter())
            .any(|id| !self.candidate_order.contains(id))
        {
            return Err(ThroughputAssuranceError::Invalid(
                "throughput assurance state is not covered by candidates".into(),
            ));
        }
        for values in [
            &self.candidate_order,
            &self.admitted_order,
            &self.blocked_order,
            &self.unknown_order,
            &self.witness_order,
            &self.counterexample_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(ThroughputAssuranceError::Invalid(
                    "throughput assurance ordering is not canonical".into(),
                ));
            }
        }
        for value in [
            &self.queue_digest,
            &self.evidence_digest,
            &self.verification_digest,
            &self.replay_identity,
        ] {
            if value.as_str().len() != 64 {
                return Err(ThroughputAssuranceError::Invalid(
                    "throughput assurance digest is invalid".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("assurance:throughput:") && effect != "block:unsafe-release"
        }) {
            return Err(ThroughputAssuranceError::Invalid(
                "throughput assurance effect is outside the local gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ThroughputAssuranceError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, ThroughputAssuranceError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ThroughputAssuranceError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ThroughputAssuranceError::Artifact(error.to_string()))
    }
}

pub fn throughput_safety_assurance_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["research operations engineer".into(), "throughput release gate".into()].into(), behavior: "verifies bounded high-throughput evidence batches with capacity, checkpoint, replay, provenance, and fail-closed witnesses".into(), value: "prevents queue overflow, stale checkpoints, or incomplete evidence from becoming a qualified release".into(), inputs: vec![TypedPort { name: "throughput_evidence_feed".into(), schema: "EvidenceFeed3@1".into(), required: true }], outputs: vec![TypedPort { name: "throughput_assurance".into(), schema: "QualifiedEvidenceSet7@1".into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["evaluate:capability-runs".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "slsa-provenance-1.2".into(), state: EvidenceState::Supported, locator: Some("https://slsa.dev/spec/v1.2/provenance".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn verify_throughput_safety(
    request: &HighThroughputEvidenceFeedRequest,
) -> Result<ThroughputAssuranceReceipt, ThroughputAssuranceError> {
    let evidence = admit_high_throughput_evidence(request)
        .map_err(|error| ThroughputAssuranceError::Engine(error.to_string()))?;
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
    let mut witnesses = BTreeSet::from([
        "gate:typed-contract".to_string(),
        "gate:capacity-bound".to_string(),
        "gate:checkpoint-continuity".to_string(),
        "gate:provenance".to_string(),
        "gate:replay-identity".to_string(),
        "gate:locality".to_string(),
        "gate:effect-allow-list".to_string(),
    ]);
    let mut counterexamples = BTreeSet::new();
    if request.max_items == 0 {
        counterexamples.insert("counterexample:capacity-zero".into());
        omissions.insert("assurance:capacity-zero".into());
    }
    if request.observations.len() > request.max_items {
        counterexamples.insert("counterexample:capacity-overflow".into());
        omissions.insert("assurance:capacity-overflow".into());
    }
    if !request.policy_allow {
        counterexamples.insert("counterexample:policy-denied".into());
        omissions.insert("assurance:policy-denied".into());
    }
    if !request.protected_closure {
        counterexamples.insert("counterexample:protected-closure-incomplete".into());
        omissions.insert("assurance:protected-closure-incomplete".into());
    }
    if !request.raw_data_local {
        counterexamples.insert("counterexample:raw-data-locality-failed".into());
        omissions.insert("assurance:raw-data-locality-failed".into());
    }
    for observation in &request.observations {
        if observation.replay_identity != request.replay_identity {
            counterexamples.insert(format!(
                "counterexample:{}:replay-mismatch",
                observation.evidence_id
            ));
        }
        if !observation.omissions.is_empty() {
            counterexamples.insert(format!(
                "counterexample:{}:omission",
                observation.evidence_id
            ));
        }
    }
    if evidence.disposition != HighThroughputDisposition::Qualified {
        witnesses.insert("gate:non-qualified-evidence-retained".into());
    }
    let verdict = if !request.policy_allow
        || !request.protected_closure
        || !request.raw_data_local
        || !counterexamples.is_empty()
    {
        ThroughputAssuranceVerdict::Blocked
    } else if evidence.disposition == HighThroughputDisposition::Qualified {
        ThroughputAssuranceVerdict::Qualified
    } else {
        ThroughputAssuranceVerdict::Unresolved
    };
    let evidence_digest = evidence
        .digest()
        .map_err(|error| ThroughputAssuranceError::Engine(error.to_string()))?;
    let verification_digest = ContentHash::of_value(&json!({"feature_id": FEATURE_ID, "request_id": request.request_id, "batch_id": request.batch_id, "checkpoint_seq": evidence.checkpoint_seq, "queue_digest": evidence.queue_digest, "witness_order": witnesses, "counterexample_order": counterexamples, "verdict": verdict}))
        .map_err(|error| ThroughputAssuranceError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "batch_id": request.batch_id, "partition": request.partition, "verdict": verdict, "candidate_order": evidence.candidate_order, "admitted_order": evidence.admitted_order, "blocked_order": evidence.blocked_order, "unknown_order": evidence.unknown_order, "witness_order": witnesses, "counterexample_order": counterexamples, "checkpoint_seq": evidence.checkpoint_seq, "queue_digest": evidence.queue_digest, "evidence_digest": evidence_digest, "verification_digest": verification_digest, "replay_identity": request.replay_identity, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-throughput-assurance:{}", request.request_id),
        "application/vnd.aurora.throughput-assurance+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ThroughputAssuranceError::Artifact(error.to_string()))?;
    let receipt = ThroughputAssuranceReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        batch_id: request.batch_id.clone(),
        partition: request.partition.clone(),
        verdict,
        candidate_order: evidence.candidate_order.clone(),
        admitted_order: evidence.admitted_order.clone(),
        blocked_order: evidence.blocked_order.clone(),
        unknown_order: evidence.unknown_order.clone(),
        witness_order: witnesses.into_iter().collect(),
        counterexample_order: counterexamples.into_iter().collect(),
        checkpoint_seq: evidence.checkpoint_seq,
        queue_digest: evidence.queue_digest.clone(),
        evidence_digest,
        verification_digest,
        replay_identity: request.replay_identity.clone(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts: if verdict == ThroughputAssuranceVerdict::Qualified {
            vec![format!("assurance:throughput:{}", request.request_id)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence_surveillance::EvidenceObservation;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn request(state: EvidenceState) -> HighThroughputEvidenceFeedRequest {
        HighThroughputEvidenceFeedRequest {
            request_id: "request:throughput-assurance".into(),
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
        }
    }
    #[test]
    fn manifest_is_a1() {
        let m = throughput_safety_assurance_manifest();
        m.validate().unwrap();
        assert_eq!(m.autonomy_tier, AutonomyTier::A1);
    }
    #[test]
    fn supported_is_qualified() {
        let r = verify_throughput_safety(&request(EvidenceState::Supported)).unwrap();
        assert_eq!(r.verdict, ThroughputAssuranceVerdict::Qualified);
    }
    #[test]
    fn unknown_is_unresolved() {
        let r = verify_throughput_safety(&request(EvidenceState::Unknown)).unwrap();
        assert_eq!(r.verdict, ThroughputAssuranceVerdict::Unresolved);
    }
    #[test]
    fn capacity_overflow_is_blocked() {
        let mut q = request(EvidenceState::Supported);
        q.max_items = 1;
        let mut second = q.observations[0].clone();
        second.evidence_id = "evidence:b".into();
        q.observations.push(second);
        let r = verify_throughput_safety(&q).unwrap();
        assert_eq!(r.verdict, ThroughputAssuranceVerdict::Blocked);
    }
    #[test]
    fn policy_is_blocked() {
        let mut q = request(EvidenceState::Supported);
        q.policy_allow = false;
        let r = verify_throughput_safety(&q).unwrap();
        assert_eq!(r.verdict, ThroughputAssuranceVerdict::Blocked);
    }
    #[test]
    fn digest_is_stable() {
        let r = verify_throughput_safety(&request(EvidenceState::Supported)).unwrap();
        assert_eq!(r.digest().unwrap(), r.digest().unwrap());
    }
}
