//! Federated continual retrieval-and-synthesis inference engine.
//!
//! Atlas feature `AFA-adapter-P02-F04`: a deterministic A0 product surface
//! around the typed retrieval/synthesis contract.  The engine computes only
//! over institution-local candidate metadata, admits bounded batches with
//! replayable checkpoints, and preserves every omission, uncertainty,
//! contradiction, overflow, and negative result in its output artifact.

use std::collections::BTreeSet;

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

use crate::retrieval_synthesis::{
    compile_evidence_synthesis, EvidenceSynthesisDisposition, EvidenceSynthesisRequest,
};

pub const FEATURE_ID: &str = "AFA-adapter-P02-F04";
pub const CONTRACT_VERSION: &str = "adapter-federated-retrieval-synthesis-inference-engine/1.0";
pub const INPUT_SCHEMA: &str = "ScopedRetrievalQuery@1";
pub const OUTPUT_SCHEMA: &str = "EvidenceSynthesis1@1";

const EFFECT_PREFIX: &str = "compute:federated-retrieval-engine:";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedRetrievalSynthesisInferenceEngineRequest {
    pub synthesis_request: EvidenceSynthesisRequest,
    pub engine_id: String,
    pub algorithm_version: String,
    pub requested_output: String,
    pub federation_id: String,
    pub purpose: String,
    pub peer_order: Vec<String>,
    pub min_peer_quorum: usize,
    pub aggregate_only: bool,
    pub policy_allow: bool,
    pub batch_id: String,
    pub checkpoint_seq: u64,
    pub capacity: usize,
    pub budget_units: u32,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedRetrievalSynthesisInferenceEngineReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub query_id: String,
    pub engine_id: String,
    pub algorithm_version: String,
    pub requested_output: String,
    pub federation_id: String,
    pub purpose: String,
    pub peer_order: Vec<String>,
    pub min_peer_quorum: usize,
    pub aggregate_only: bool,
    pub disposition: EvidenceSynthesisDisposition,
    pub candidate_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub omitted_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_order: Vec<String>,
    pub contradictory_order: Vec<String>,
    pub overflow_order: Vec<String>,
    pub envelope_digest: ContentHash,
    pub batch_id: String,
    pub checkpoint_seq: u64,
    pub capacity: usize,
    pub replay_identity: ContentHash,
    pub synthesis_digest: ContentHash,
    pub engine_digest: ContentHash,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum FederatedRetrievalSynthesisInferenceEngineError {
    #[error("invalid federated retrieval engine request: {0}")]
    Invalid(String),
    #[error("federated retrieval engine artifact failed: {0}")]
    Artifact(String),
    #[error("federated retrieval engine synthesis failed: {0}")]
    Synthesis(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl FederatedRetrievalSynthesisInferenceEngineReceipt {
    pub fn validate(&self) -> Result<(), FederatedRetrievalSynthesisInferenceEngineError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.query_id.trim().is_empty()
            || self.engine_id.trim().is_empty()
            || self.algorithm_version.trim().is_empty()
            || self.requested_output != OUTPUT_SCHEMA
            || self.federation_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.peer_order.is_empty()
            || self.min_peer_quorum == 0
            || self.peer_order.len() < self.min_peer_quorum
            || !self.aggregate_only
            || self.batch_id.trim().is_empty()
            || self.checkpoint_seq == 0
            || self.capacity == 0
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(FederatedRetrievalSynthesisInferenceEngineError::Invalid(
                "federated retrieval engine identity, output, locality, candidates, or effects are incomplete".into(),
            ));
        }
        for values in [
            &self.peer_order,
            &self.candidate_order,
            &self.selected_order,
            &self.omitted_order,
            &self.uncertainty_order,
            &self.negative_order,
            &self.contradictory_order,
            &self.overflow_order,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(FederatedRetrievalSynthesisInferenceEngineError::Invalid(
                    "federated retrieval engine ordering is not canonical".into(),
                ));
            }
        }
        if self.envelope_digest.as_str().len() != 64 {
            return Err(FederatedRetrievalSynthesisInferenceEngineError::Invalid(
                "federated envelope digest is invalid".into(),
            ));
        }
        let classified = self
            .selected_order
            .iter()
            .chain(self.omitted_order.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        if classified != self.candidate_order.iter().cloned().collect() {
            return Err(FederatedRetrievalSynthesisInferenceEngineError::Invalid(
                "federated retrieval engine states do not partition candidates".into(),
            ));
        }
        if self
            .overflow_order
            .iter()
            .any(|id| !self.omitted_order.contains(id))
        {
            return Err(FederatedRetrievalSynthesisInferenceEngineError::Invalid(
                "throughput overflow must be an omitted candidate subset".into(),
            ));
        }
        for digest in [
            &self.replay_identity,
            &self.synthesis_digest,
            &self.engine_digest,
            &self.artifact.content_hash,
        ] {
            if digest.as_str().len() != 64 {
                return Err(FederatedRetrievalSynthesisInferenceEngineError::Invalid(
                    "federated retrieval engine digest is invalid".into(),
                ));
            }
        }
        if self
            .effect_receipts
            .iter()
            .any(|effect| !effect.starts_with(EFFECT_PREFIX) && effect != "block:unsafe-release")
        {
            return Err(FederatedRetrievalSynthesisInferenceEngineError::Invalid(
                "federated retrieval engine effect is outside local computation gate".into(),
            ));
        }
        self.artifact.validate_metadata().map_err(|error| {
            FederatedRetrievalSynthesisInferenceEngineError::Artifact(error.to_string())
        })
    }
}

pub fn federated_retrieval_synthesis_inference_engine_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "adapter".into(),
        consumers: ["integration engineer".into(), "preclinical researcher".into()].into(),
        behavior: "computes a deterministic federated continual retrieval corpus from permitted aggregate contributions with purpose, signer, quorum, locality, and policy gates while retaining omissions, uncertainty, contradiction, and negative results".into(),
        value: "provides a separately versioned, replayable inference-engine surface for policy-separated institutions without moving raw observations or silently substituting defaults".into(),
        inputs: vec![TypedPort {
            name: "scoped_retrieval_query".into(),
            schema: INPUT_SCHEMA.into(),
            required: true,
        }],
        outputs: vec![TypedPort {
            name: "evidence_synthesis".into(),
            schema: OUTPUT_SCHEMA.into(),
            required: true,
        }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation].into(),
        permissions: ["read:institution-local-research-state".into(), "exchange:permitted-aggregate-evidence".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference {
            source_id: "json-schema".into(),
            state: EvidenceState::Supported,
            locator: Some("https://json-schema.org/specification".into()),
        }],
        authority_requirements: Vec::new(),
        autonomy_tier: AutonomyTier::A1,
        surfaces: [
            ResearchSurface::Ui,
            ResearchSurface::Api,
            ResearchSurface::Sdk,
            ResearchSurface::Cli,
            ResearchSurface::Operator,
        ]
        .into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn run_federated_retrieval_synthesis_inference_engine(
    request: &FederatedRetrievalSynthesisInferenceEngineRequest,
) -> Result<
    FederatedRetrievalSynthesisInferenceEngineReceipt,
    FederatedRetrievalSynthesisInferenceEngineError,
> {
    validate_request(request)?;
    let synthesis = compile_evidence_synthesis(&request.synthesis_request).map_err(|error| {
        FederatedRetrievalSynthesisInferenceEngineError::Synthesis(error.to_string())
    })?;
    let candidate_order = request
        .synthesis_request
        .candidates
        .iter()
        .map(|candidate| candidate.evidence_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let selected_order = synthesis
        .synthesis
        .selected_evidence_ids
        .iter()
        .take(request.capacity)
        .cloned()
        .collect::<Vec<_>>();
    let selected_set = selected_order.iter().cloned().collect::<BTreeSet<_>>();
    let omitted_order = candidate_order
        .iter()
        .filter(|id| !selected_set.contains(*id))
        .cloned()
        .collect::<Vec<_>>();
    let overflow_order = candidate_order
        .iter()
        .skip(request.capacity)
        .filter(|id| omitted_order.contains(id))
        .cloned()
        .collect::<Vec<_>>();
    let peer_order = request
        .peer_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let federation_blocked = !request.policy_allow
        || !request.aggregate_only
        || peer_order.len() < request.min_peer_quorum;
    let envelope_digest = ContentHash::of_value(&json!({
        "federation_id": request.federation_id,
        "purpose": request.purpose,
        "peer_order": peer_order,
        "min_peer_quorum": request.min_peer_quorum,
        "aggregate_only": request.aggregate_only,
        "policy_allow": request.policy_allow,
        "raw_data_local": request.synthesis_request.raw_data_local,
    }))
    .map_err(|error| {
        FederatedRetrievalSynthesisInferenceEngineError::Artifact(error.to_string())
    })?;
    let uncertainty_order = synthesis.uncertainty.clone();
    let negative_order = synthesis.synthesis.negative_evidence_ids.clone();
    let contradictory_order = synthesis.synthesis.contradictory_evidence_ids.clone();
    let disposition = if federation_blocked {
        EvidenceSynthesisDisposition::Blocked
    } else {
        synthesis.disposition
    };
    let synthesis_value = serde_json::to_value(&synthesis.synthesis).map_err(|error| {
        FederatedRetrievalSynthesisInferenceEngineError::Artifact(error.to_string())
    })?;
    let synthesis_digest = ContentHash::of_value(&synthesis_value).map_err(|error| {
        FederatedRetrievalSynthesisInferenceEngineError::Artifact(error.to_string())
    })?;
    let engine_digest = ContentHash::of_value(&json!({
        "engine_id": request.engine_id,
        "algorithm_version": request.algorithm_version,
        "requested_output": request.requested_output,
        "batch_id": request.batch_id,
        "checkpoint_seq": request.checkpoint_seq,
        "capacity": request.capacity,
        "federation_id": request.federation_id,
        "purpose": request.purpose,
        "query_id": request.synthesis_request.query.query_id,
        "replay_identity": request.replay_identity,
        "synthesis_digest": synthesis_digest,
        "overflow_order": overflow_order.clone(),
        "envelope_digest": envelope_digest,
    }))
    .map_err(|error| {
        FederatedRetrievalSynthesisInferenceEngineError::Artifact(error.to_string())
    })?;
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.synthesis_request.request_id,
        "query_id": request.synthesis_request.query.query_id,
        "engine_id": request.engine_id,
        "algorithm_version": request.algorithm_version,
        "requested_output": request.requested_output,
        "batch_id": request.batch_id,
        "checkpoint_seq": request.checkpoint_seq,
        "capacity": request.capacity,
        "federation_id": request.federation_id,
        "purpose": request.purpose,
        "peer_order": peer_order,
        "min_peer_quorum": request.min_peer_quorum,
        "aggregate_only": request.aggregate_only,
        "disposition": disposition,
        "candidate_order": candidate_order,
        "selected_order": selected_order,
        "omitted_order": omitted_order,
        "uncertainty_order": uncertainty_order,
        "negative_order": negative_order,
        "contradictory_order": contradictory_order,
        "overflow_order": overflow_order,
        "envelope_digest": envelope_digest,
        "replay_identity": request.replay_identity,
        "synthesis_digest": synthesis_digest,
        "engine_digest": engine_digest,
        "omissions": synthesis.omissions,
        "uncertainty": synthesis.uncertainty,
        "raw_data_local": true,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("adapter-throughput-retrieval-engine:{}", request.engine_id),
        "application/vnd.aurora.throughput-retrieval-synthesis-engine+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| {
        FederatedRetrievalSynthesisInferenceEngineError::Artifact(error.to_string())
    })?;
    let receipt = FederatedRetrievalSynthesisInferenceEngineReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.synthesis_request.request_id.clone(),
        query_id: request.synthesis_request.query.query_id.clone(),
        engine_id: request.engine_id.clone(),
        algorithm_version: request.algorithm_version.clone(),
        requested_output: request.requested_output.clone(),
        federation_id: request.federation_id.clone(),
        purpose: request.purpose.clone(),
        peer_order,
        min_peer_quorum: request.min_peer_quorum,
        aggregate_only: request.aggregate_only,
        batch_id: request.batch_id.clone(),
        checkpoint_seq: request.checkpoint_seq,
        capacity: request.capacity,
        disposition,
        candidate_order,
        selected_order,
        omitted_order,
        uncertainty_order,
        negative_order,
        contradictory_order,
        overflow_order,
        envelope_digest,
        replay_identity: request.replay_identity.clone(),
        synthesis_digest,
        engine_digest,
        effect_receipts: vec![format!("{EFFECT_PREFIX}{}", request.engine_id)],
        artifact,
        raw_data_local: true,
        boundary: request.boundary.clone(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(
    request: &FederatedRetrievalSynthesisInferenceEngineRequest,
) -> Result<(), FederatedRetrievalSynthesisInferenceEngineError> {
    if request.engine_id.trim().is_empty()
        || request.algorithm_version.trim().is_empty()
        || request.requested_output != OUTPUT_SCHEMA
        || request.federation_id.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.peer_order.is_empty()
        || request.min_peer_quorum == 0
        || request.peer_order.len() < request.min_peer_quorum
        || !request.aggregate_only
        || request.budget_units == 0
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.synthesis_request.boundary != PRECLINICAL_BOUNDARY
        || !request.synthesis_request.raw_data_local
        || request.batch_id.trim().is_empty()
        || request.checkpoint_seq == 0
        || request.capacity == 0
        || request.capacity > request.synthesis_request.candidates.len()
        || request.replay_identity.as_str().len() != 64
    {
        return Err(FederatedRetrievalSynthesisInferenceEngineError::Invalid(
            "engine identity, output, budget, locality, replay, or boundary is invalid".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retrieval_synthesis::{RetrievalCandidate, ScopedRetrievalQuery};
    use bioprism_foundation::{EvidenceAvailability, PolicyDecision};

    fn request() -> FederatedRetrievalSynthesisInferenceEngineRequest {
        FederatedRetrievalSynthesisInferenceEngineRequest {
            synthesis_request: EvidenceSynthesisRequest {
                request_id: "request:f21".into(),
                query: ScopedRetrievalQuery {
                    query_id: "query:single-study".into(),
                    requester: "researcher:f21".into(),
                    intent: "retrieve preclinical evidence".into(),
                    study_ids: vec!["study:f21".into()],
                    required_modalities: vec!["imaging".into()],
                    comparability_profile: "profile:f21".into(),
                    max_results: 4,
                },
                candidates: vec![RetrievalCandidate {
                    evidence_id: "evidence:f21".into(),
                    study_id: "study:f21".into(),
                    modality: "imaging".into(),
                    comparability_profile: "profile:f21".into(),
                    digest: Some(ContentHash::of_bytes(b"f21")),
                    availability: EvidenceAvailability::Available,
                    relevance_score: 90,
                    negative_result: true,
                    locator: "local://f21".into(),
                }],
                policy_decision: PolicyDecision::Allow,
                protected_closure_satisfied: true,
                raw_data_local: true,
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            engine_id: "engine:f21".into(),
            algorithm_version: "2026.1".into(),
            requested_output: OUTPUT_SCHEMA.into(),
            federation_id: "federation:f04".into(),
            purpose: "continual retrieval".into(),
            peer_order: vec!["peer-a".into(), "peer-b".into()],
            min_peer_quorum: 2,
            aggregate_only: true,
            policy_allow: true,
            batch_id: "batch:f03".into(),
            checkpoint_seq: 1,
            capacity: 1,
            budget_units: 4,
            replay_identity: ContentHash::of_bytes(b"replay:f21"),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            federated_retrieval_synthesis_inference_engine_manifest().autonomy_tier,
            AutonomyTier::A1
        );
    }

    #[test]
    fn computes_local_synthesis() {
        let receipt = run_federated_retrieval_synthesis_inference_engine(&request()).unwrap();
        assert_eq!(receipt.feature_id, FEATURE_ID);
        assert_eq!(receipt.selected_order, vec!["evidence:f21"]);
    }

    #[test]
    fn negative_result_is_retained() {
        let receipt = run_federated_retrieval_synthesis_inference_engine(&request()).unwrap();
        assert_eq!(receipt.negative_order, vec!["evidence:f21"]);
    }

    #[test]
    fn policy_block_is_observable() {
        let mut value = request();
        value.synthesis_request.policy_decision = bioprism_foundation::PolicyDecision::Deny;
        let receipt = run_federated_retrieval_synthesis_inference_engine(&value).unwrap();
        assert_eq!(receipt.disposition, EvidenceSynthesisDisposition::Blocked);
    }

    #[test]
    fn rejects_wrong_output() {
        let mut value = request();
        value.requested_output = "Wrong@1".into();
        assert!(run_federated_retrieval_synthesis_inference_engine(&value).is_err());
    }

    #[test]
    fn replay_is_stable() {
        let value = request();
        assert_eq!(
            run_federated_retrieval_synthesis_inference_engine(&value).unwrap(),
            run_federated_retrieval_synthesis_inference_engine(&value).unwrap()
        );
    }

    #[test]
    fn rejects_capacity_above_batch() {
        let mut value = request();
        value.capacity = 2;
        assert!(run_federated_retrieval_synthesis_inference_engine(&value).is_err());
    }

    #[test]
    fn federation_policy_is_fail_closed() {
        let mut value = request();
        value.policy_allow = false;
        let receipt = run_federated_retrieval_synthesis_inference_engine(&value).unwrap();
        assert_eq!(receipt.disposition, EvidenceSynthesisDisposition::Blocked);
    }
}
