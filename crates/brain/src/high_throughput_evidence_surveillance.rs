//! Prospective high-throughput evidence-surveillance admission.
//!
//! Atlas feature: `AFA-brain-P01-F03`. Batches are bounded, replayable, and deterministic;
//! capacity pressure is an explicit omission rather than silent data loss.

use crate::evidence_surveillance::EvidenceObservation;
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

pub const FEATURE_ID: &str = "AFA-brain-P01-F03";
pub const CONTRACT_VERSION: &str = "brain-evidence-surveillance-throughput/1.0";
pub const MAX_BATCH_ITEMS: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HighThroughputEvidenceFeedRequest {
    pub request_id: String,
    pub batch_id: String,
    pub partition: String,
    pub max_items: usize,
    pub observations: Vec<EvidenceObservation>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HighThroughputDisposition {
    Qualified,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HighThroughputEvidenceReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub batch_id: String,
    pub partition: String,
    pub disposition: HighThroughputDisposition,
    pub candidate_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub relevance_order: Vec<u16>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub checkpoint_seq: u64,
    pub queue_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum HighThroughputEvidenceError {
    #[error("invalid high-throughput evidence request: {0}")]
    Invalid(String),
    #[error("high-throughput evidence artifact failed: {0}")]
    Artifact(String),
}

impl HighThroughputEvidenceReceipt {
    pub fn validate(&self) -> Result<(), HighThroughputEvidenceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.batch_id.trim().is_empty()
            || self.partition.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.relevance_order.len() != self.candidate_order.len()
            || self.effect_receipts.is_empty()
        {
            return Err(HighThroughputEvidenceError::Invalid(
                "identity, batch partition, ranking, locality, or effects are incomplete".into(),
            ));
        }
        if self
            .admitted_order
            .iter()
            .chain(self.blocked_order.iter())
            .chain(self.unknown_order.iter())
            .any(|id| !self.candidate_order.contains(id))
        {
            return Err(HighThroughputEvidenceError::Invalid(
                "batch state is not covered by candidate order".into(),
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
                return Err(HighThroughputEvidenceError::Invalid(
                    "batch ordering is not canonical".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("read:local-research-artifacts:")
                && effect != "block:unsafe-release"
        }) {
            return Err(HighThroughputEvidenceError::Invalid(
                "effect is outside the throughput gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| HighThroughputEvidenceError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, HighThroughputEvidenceError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| HighThroughputEvidenceError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| HighThroughputEvidenceError::Artifact(error.to_string()))
    }
}

pub fn high_throughput_evidence_surveillance_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(),
        consumers: ["laboratory automation engineer".into(), "research operations steward".into()].into(),
        behavior: "admits bounded prospective evidence batches with deterministic queue ordering, checkpoint identity, and explicit capacity omissions".into(),
        value: "supports safe high-throughput evidence surveillance without silent drops or unauthorized effects".into(),
        inputs: vec![TypedPort { name: "evidence_feed".into(), schema: "EvidenceFeed3@1".into(), required: true }], outputs: vec![TypedPort { name: "qualified_evidence_set".into(), schema: "QualifiedEvidenceSet1@1".into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["read:local-research-artifacts".into()].into(), determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "json-schema".into(), state: EvidenceState::Supported, locator: Some("https://json-schema.org/specification".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn admit_high_throughput_evidence(
    request: &HighThroughputEvidenceFeedRequest,
) -> Result<HighThroughputEvidenceReceipt, HighThroughputEvidenceError> {
    validate_request(request)?;
    let mut observations = request.observations.clone();
    observations.sort_by(|left, right| {
        right
            .relevance_milli
            .cmp(&left.relevance_milli)
            .then(left.evidence_id.cmp(&right.evidence_id))
    });
    let candidate_order = observations
        .iter()
        .map(|item| item.evidence_id.clone())
        .collect::<Vec<_>>();
    let relevance_order = observations
        .iter()
        .map(|item| item.relevance_milli)
        .collect::<Vec<_>>();
    let mut admitted = Vec::new();
    let mut blocked = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for (index, observation) in observations.iter().enumerate() {
        if index >= request.max_items {
            blocked.insert(observation.evidence_id.clone());
            omissions.insert(format!(
                "evidence:{}:batch-capacity-exceeded",
                observation.evidence_id
            ));
            continue;
        }
        let ok = request.policy_allow
            && request.protected_closure
            && request.raw_data_local
            && observation.raw_data_local
            && observation.state == EvidenceState::Supported
            && observation.replay_identity == request.replay_identity
            && observation.omissions.is_empty()
            && observation.negative_evidence.is_empty();
        if ok {
            admitted.push(observation.evidence_id.clone());
        } else {
            blocked.insert(observation.evidence_id.clone());
            if matches!(
                observation.state,
                EvidenceState::Unknown | EvidenceState::Speculative
            ) {
                unknown.insert(observation.evidence_id.clone());
                uncertainty.insert(
                    format!(
                        "evidence:{}:state-{:?}-not-admitted",
                        observation.evidence_id, observation.state
                    )
                    .to_ascii_lowercase(),
                );
            }
            if observation.state == EvidenceState::Contradicted {
                negative.insert(format!(
                    "evidence:{}:contradicted-negative-evidence",
                    observation.evidence_id
                ));
            }
            if observation.replay_identity != request.replay_identity {
                uncertainty.insert(format!(
                    "evidence:{}:replay-mismatch",
                    observation.evidence_id
                ));
            }
            if !observation.omissions.is_empty() {
                uncertainty.insert(format!(
                    "evidence:{}:protected-closure-incomplete",
                    observation.evidence_id
                ));
            }
            if !observation.negative_evidence.is_empty() {
                negative.insert(format!(
                    "evidence:{}:negative-result-retained",
                    observation.evidence_id
                ));
            }
        }
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
    let disposition =
        if !request.policy_allow || !request.protected_closure || !request.raw_data_local {
            HighThroughputDisposition::Blocked
        } else if admitted.is_empty() {
            HighThroughputDisposition::Unknown
        } else if blocked.is_empty()
            && omissions.is_empty()
            && uncertainty.is_empty()
            && negative.is_empty()
        {
            HighThroughputDisposition::Qualified
        } else {
            HighThroughputDisposition::Partial
        };
    let checkpoint_seq = request.observations.len() as u64;
    let queue_digest = ContentHash::of_value(&json!({"batch_id": request.batch_id, "partition": request.partition, "candidate_order": candidate_order, "checkpoint_seq": checkpoint_seq, "replay_identity": request.replay_identity})).map_err(|error| HighThroughputEvidenceError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "batch_id": request.batch_id, "partition": request.partition, "disposition": disposition, "candidate_order": candidate_order, "admitted_order": admitted, "blocked_order": blocked, "unknown_order": unknown, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "checkpoint_seq": checkpoint_seq, "queue_digest": queue_digest, "replay_identity": request.replay_identity, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-throughput-evidence:{}", request.request_id),
        "application/vnd.aurora.high-throughput-evidence-receipt+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| HighThroughputEvidenceError::Artifact(error.to_string()))?;
    let has_admitted = !admitted.is_empty();
    let receipt = HighThroughputEvidenceReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        batch_id: request.batch_id.clone(),
        partition: request.partition.clone(),
        disposition,
        candidate_order,
        admitted_order: admitted,
        blocked_order: blocked.into_iter().collect(),
        unknown_order: unknown.into_iter().collect(),
        relevance_order,
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        checkpoint_seq,
        queue_digest,
        replay_identity: request.replay_identity.clone(),
        effect_receipts: if has_admitted {
            vec![format!(
                "read:local-research-artifacts:{}",
                request.request_id
            )]
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
    request: &HighThroughputEvidenceFeedRequest,
) -> Result<(), HighThroughputEvidenceError> {
    if request.request_id.trim().is_empty()
        || request.batch_id.trim().is_empty()
        || request.partition.trim().is_empty()
        || request.max_items == 0
        || request.max_items > MAX_BATCH_ITEMS
        || request.observations.is_empty()
        || request.observations.len() > MAX_BATCH_ITEMS
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(HighThroughputEvidenceError::Invalid(
            "batch identity, partition, capacity, observations, or boundary is incomplete".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for observation in &request.observations {
        if observation.evidence_id.trim().is_empty()
            || observation.boundary != PRECLINICAL_BOUNDARY
            || !ids.insert(observation.evidence_id.clone())
        {
            return Err(HighThroughputEvidenceError::Invalid(format!(
                "observation {} is invalid or duplicated",
                observation.evidence_id
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
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
    fn request(observations: Vec<EvidenceObservation>) -> HighThroughputEvidenceFeedRequest {
        HighThroughputEvidenceFeedRequest {
            request_id: "request:throughput".into(),
            batch_id: "batch:001".into(),
            partition: "partition:imaging".into(),
            max_items: 2,
            observations,
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1_and_typed() {
        let manifest = high_throughput_evidence_surveillance_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A1);
    }
    #[test]
    fn complete_batch_is_qualified_and_checkpointed() {
        let receipt = admit_high_throughput_evidence(&request(vec![
            observation("b", EvidenceState::Supported),
            observation("a", EvidenceState::Supported),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, HighThroughputDisposition::Qualified);
        assert_eq!(receipt.checkpoint_seq, 2);
        assert_eq!(receipt.candidate_order, vec!["evidence:a", "evidence:b"]);
    }
    #[test]
    fn capacity_pressure_is_explicit() {
        let receipt = admit_high_throughput_evidence(&request(vec![
            observation("a", EvidenceState::Supported),
            observation("b", EvidenceState::Supported),
            observation("c", EvidenceState::Supported),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, HighThroughputDisposition::Partial);
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item.contains("capacity")));
    }
    #[test]
    fn unknown_and_contradicted_are_retained() {
        let receipt = admit_high_throughput_evidence(&request(vec![
            observation("a", EvidenceState::Unknown),
            observation("b", EvidenceState::Contradicted),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, HighThroughputDisposition::Unknown);
        assert!(!receipt.unknown_order.is_empty());
        assert!(!receipt.negative_evidence.is_empty());
    }
    #[test]
    fn policy_denial_blocks_release() {
        let mut input = request(vec![observation("a", EvidenceState::Supported)]);
        input.policy_allow = false;
        let receipt = admit_high_throughput_evidence(&input).unwrap();
        assert_eq!(receipt.disposition, HighThroughputDisposition::Blocked);
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn duplicate_observation_is_rejected() {
        let mut duplicate = observation("a", EvidenceState::Supported);
        duplicate.source_id = "source:other".into();
        assert!(admit_high_throughput_evidence(&request(vec![
            observation("a", EvidenceState::Supported),
            duplicate
        ]))
        .is_err());
    }
}
