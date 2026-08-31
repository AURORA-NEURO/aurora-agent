//! Prospective high-throughput retrieval-and-synthesis research copilot.
//!
//! Atlas feature `AFA-adapter-P02-F11`: a deterministic A2 product surface
//! around the typed retrieval/synthesis contract.  The copilot compares at
//! least two modalities across studies, admits a bounded queue with durable
//! batch/checkpoint identity, emits a bounded approval-aware tool intent,
//! computes only over institution-local candidate metadata, and preserves
//! every omission, uncertainty, contradiction, overflow, and negative result.

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

pub const FEATURE_ID: &str = "AFA-adapter-P02-F11";
pub const CONTRACT_VERSION: &str = "adapter-throughput-retrieval-synthesis-research-copilot/1.0";
pub const INPUT_SCHEMA: &str = "ScopedRetrievalQuery@1";
pub const OUTPUT_SCHEMA: &str = "EvidenceSynthesis1@1";

const EFFECT_PREFIX: &str = "compute:throughput-retrieval-research-copilot:";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputRetrievalSynthesisResearchCopilotRequest {
    pub synthesis_request: EvidenceSynthesisRequest,
    pub copilot_id: String,
    pub agent_id: String,
    pub recommendation_mode: String,
    pub approval_required: bool,
    pub schema_profile: String,
    pub canonicalization: String,
    pub consumer: String,
    pub algorithm_version: String,
    pub required_modalities: Vec<String>,
    pub tool_id: String,
    pub approval_token: String,
    pub comparability_digest: ContentHash,
    pub batch_id: String,
    pub checkpoint_seq: u64,
    pub capacity: u32,
    pub queue_digest: ContentHash,
    pub checkpoint_digest: ContentHash,
    pub requested_output: String,
    pub budget_units: u32,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputRetrievalSynthesisResearchCopilotReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub query_id: String,
    pub copilot_id: String,
    pub agent_id: String,
    pub recommendation_mode: String,
    pub approval_required: bool,
    pub schema_profile: String,
    pub canonicalization: String,
    pub consumer: String,
    pub algorithm_version: String,
    pub required_modalities: Vec<String>,
    pub tool_id: String,
    pub approval_token: String,
    pub comparability_digest: ContentHash,
    pub batch_id: String,
    pub checkpoint_seq: u64,
    pub capacity: u32,
    pub queue_digest: ContentHash,
    pub checkpoint_digest: ContentHash,
    pub requested_output: String,
    pub disposition: EvidenceSynthesisDisposition,
    pub candidate_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub omitted_order: Vec<String>,
    pub overflow_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_order: Vec<String>,
    pub contradictory_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub synthesis_digest: ContentHash,
    pub copilot_digest: ContentHash,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ThroughputRetrievalSynthesisResearchCopilotError {
    #[error("invalid throughput retrieval research copilot request: {0}")]
    Invalid(String),
    #[error("throughput retrieval research copilot artifact failed: {0}")]
    Artifact(String),
    #[error("throughput retrieval research copilot synthesis failed: {0}")]
    Synthesis(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl ThroughputRetrievalSynthesisResearchCopilotReceipt {
    pub fn validate(&self) -> Result<(), ThroughputRetrievalSynthesisResearchCopilotError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.query_id.trim().is_empty()
            || self.copilot_id.trim().is_empty()
            || self.agent_id.trim().is_empty()
            || self.recommendation_mode != "evidence-ranked-read-only"
            || !self.approval_required
            || self.schema_profile != INPUT_SCHEMA
            || self.canonicalization != "aurora-json-canonical-v1"
            || self.consumer.trim().is_empty()
            || self.algorithm_version.trim().is_empty()
            || self.required_modalities.len() < 2
            || !canonical(&self.required_modalities)
            || self.tool_id.trim().is_empty()
            || self.batch_id.trim().is_empty()
            || self.checkpoint_seq == 0
            || self.capacity == 0
            || self.requested_output != OUTPUT_SCHEMA
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(ThroughputRetrievalSynthesisResearchCopilotError::Invalid(
                "throughput retrieval research copilot identity, output, locality, candidates, or effects are incomplete".into(),
            ));
        }
        for values in [
            &self.candidate_order,
            &self.selected_order,
            &self.omitted_order,
            &self.overflow_order,
            &self.uncertainty_order,
            &self.negative_order,
            &self.contradictory_order,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(ThroughputRetrievalSynthesisResearchCopilotError::Invalid(
                    "throughput retrieval research copilot ordering is not canonical".into(),
                ));
            }
        }
        let classified = self
            .selected_order
            .iter()
            .chain(self.omitted_order.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        if classified != self.candidate_order.iter().cloned().collect() {
            return Err(ThroughputRetrievalSynthesisResearchCopilotError::Invalid(
                "throughput retrieval research copilot states do not partition candidates".into(),
            ));
        }
        for digest in [
            &self.replay_identity,
            &self.synthesis_digest,
            &self.copilot_digest,
            &self.comparability_digest,
            &self.queue_digest,
            &self.checkpoint_digest,
            &self.artifact.content_hash,
        ] {
            if digest.as_str().len() != 64 {
                return Err(ThroughputRetrievalSynthesisResearchCopilotError::Invalid(
                    "throughput retrieval research copilot digest is invalid".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with(EFFECT_PREFIX)
                && !effect.starts_with("approval-required:")
                && effect != "block:unsafe-release"
        }) {
            return Err(ThroughputRetrievalSynthesisResearchCopilotError::Invalid(
                "throughput retrieval research copilot effect is outside local computation or approval gate".into(),
            ));
        }
        if self
            .overflow_order
            .iter()
            .any(|id| !self.omitted_order.contains(id))
        {
            return Err(ThroughputRetrievalSynthesisResearchCopilotError::Invalid(
                "throughput overflow must be an omitted candidate subset".into(),
            ));
        }
        self.artifact.validate_metadata().map_err(|error| {
            ThroughputRetrievalSynthesisResearchCopilotError::Artifact(error.to_string())
        })
    }
}

pub fn throughput_retrieval_synthesis_research_copilot_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "adapter".into(),
        consumers: ["integration engineer".into(), "preclinical researcher".into()].into(),
        behavior: "runs a bounded A2 prospective high-throughput research copilot that compares at least two institution-local modalities across studies, admits queue capacity with checkpoint identity, emits an approval-aware tool intent, and retains omissions, uncertainty, contradiction, overflow, and negative results".into(),
        value: "provides a separately versioned auditable copilot surface for high-throughput comparable imaging, omics, and assay evidence synthesis without silently dropping overflow or making clinical decisions".into(),
        inputs: vec![TypedPort {
            name: "scoped_retrieval_query".into(),
            schema: INPUT_SCHEMA.into(),
            required: true,
        }, TypedPort {
            name: "batch_admission".into(),
            schema: "ResearchBatchAdmission@1".into(),
            required: true,
        }],
        outputs: vec![TypedPort {
            name: "evidence_synthesis".into(),
            schema: OUTPUT_SCHEMA.into(),
            required: true,
        }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation].into(),
        permissions: [
            "read:institution-local-research-state".into(),
            "invoke:approved-research-tool".into(),
        ].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference {
            source_id: "json-schema".into(),
            state: EvidenceState::Supported,
            locator: Some("https://json-schema.org/specification".into()),
        }],
        authority_requirements: Vec::new(),
        autonomy_tier: AutonomyTier::A2,
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

pub fn run_throughput_retrieval_synthesis_research_copilot(
    request: &ThroughputRetrievalSynthesisResearchCopilotRequest,
) -> Result<
    ThroughputRetrievalSynthesisResearchCopilotReceipt,
    ThroughputRetrievalSynthesisResearchCopilotError,
> {
    validate_request(request)?;
    let mut bounded_request = request.synthesis_request.clone();
    bounded_request.query.max_results = bounded_request
        .query
        .max_results
        .min(request.capacity as usize);
    let synthesis = compile_evidence_synthesis(&bounded_request).map_err(|error| {
        ThroughputRetrievalSynthesisResearchCopilotError::Synthesis(error.to_string())
    })?;
    let candidate_order = request
        .synthesis_request
        .candidates
        .iter()
        .map(|candidate| candidate.evidence_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let selected_order = synthesis.synthesis.selected_evidence_ids.clone();
    let selected_set = selected_order.iter().cloned().collect::<BTreeSet<_>>();
    let omitted_order = candidate_order
        .iter()
        .filter(|id| !selected_set.contains(*id))
        .cloned()
        .collect::<Vec<_>>();
    let mut ranked_candidates = request
        .synthesis_request
        .candidates
        .iter()
        .map(|candidate| (&candidate.evidence_id, candidate.relevance_score))
        .collect::<Vec<_>>();
    ranked_candidates.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(right.0)));
    let overflow_order = ranked_candidates
        .into_iter()
        .skip(request.capacity as usize)
        .map(|(id, _)| id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let uncertainty_order = synthesis.uncertainty.clone();
    let negative_order = synthesis.synthesis.negative_evidence_ids.clone();
    let contradictory_order = synthesis.synthesis.contradictory_evidence_ids.clone();
    let disposition = if request.approval_token.trim().is_empty() {
        EvidenceSynthesisDisposition::Blocked
    } else {
        synthesis.disposition
    };
    let synthesis_value = serde_json::to_value(&synthesis.synthesis).map_err(|error| {
        ThroughputRetrievalSynthesisResearchCopilotError::Artifact(error.to_string())
    })?;
    let synthesis_digest = ContentHash::of_value(&synthesis_value).map_err(|error| {
        ThroughputRetrievalSynthesisResearchCopilotError::Artifact(error.to_string())
    })?;
    let copilot_digest = ContentHash::of_value(&json!({
        "copilot_id": request.copilot_id,
        "agent_id": request.agent_id,
        "recommendation_mode": request.recommendation_mode,
        "approval_required": request.approval_required,
        "schema_profile": request.schema_profile,
        "canonicalization": request.canonicalization,
        "consumer": request.consumer,
        "algorithm_version": request.algorithm_version,
        "required_modalities": request.required_modalities,
        "tool_id": request.tool_id,
        "approval_token": request.approval_token,
        "comparability_digest": request.comparability_digest,
        "batch_id": request.batch_id,
        "checkpoint_seq": request.checkpoint_seq,
        "capacity": request.capacity,
        "queue_digest": request.queue_digest,
        "checkpoint_digest": request.checkpoint_digest,
        "requested_output": request.requested_output,
        "query_id": request.synthesis_request.query.query_id,
        "replay_identity": request.replay_identity,
        "synthesis_digest": synthesis_digest,
    }))
    .map_err(|error| {
        ThroughputRetrievalSynthesisResearchCopilotError::Artifact(error.to_string())
    })?;
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.synthesis_request.request_id,
        "query_id": request.synthesis_request.query.query_id,
        "copilot_id": request.copilot_id,
        "agent_id": request.agent_id,
        "recommendation_mode": request.recommendation_mode,
        "approval_required": request.approval_required,
        "schema_profile": request.schema_profile,
        "canonicalization": request.canonicalization,
        "consumer": request.consumer,
        "algorithm_version": request.algorithm_version,
        "required_modalities": request.required_modalities,
        "tool_id": request.tool_id,
        "approval_token": request.approval_token,
        "comparability_digest": request.comparability_digest,
        "batch_id": request.batch_id,
        "checkpoint_seq": request.checkpoint_seq,
        "capacity": request.capacity,
        "queue_digest": request.queue_digest,
        "checkpoint_digest": request.checkpoint_digest,
        "requested_output": request.requested_output,
        "disposition": disposition,
        "candidate_order": candidate_order,
        "selected_order": selected_order,
        "omitted_order": omitted_order,
        "overflow_order": overflow_order,
        "uncertainty_order": uncertainty_order,
        "negative_order": negative_order,
        "contradictory_order": contradictory_order,
        "replay_identity": request.replay_identity,
        "synthesis_digest": synthesis_digest,
        "copilot_digest": copilot_digest,
        "omissions": synthesis.omissions,
        "uncertainty": synthesis.uncertainty,
        "raw_data_local": true,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "adapter-throughput-retrieval-research-copilot:{}",
            request.copilot_id
        ),
        "application/vnd.aurora.throughput-retrieval-synthesis-research-copilot+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| {
        ThroughputRetrievalSynthesisResearchCopilotError::Artifact(error.to_string())
    })?;
    let receipt = ThroughputRetrievalSynthesisResearchCopilotReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.synthesis_request.request_id.clone(),
        query_id: request.synthesis_request.query.query_id.clone(),
        copilot_id: request.copilot_id.clone(),
        agent_id: request.agent_id.clone(),
        recommendation_mode: request.recommendation_mode.clone(),
        approval_required: request.approval_required,
        schema_profile: request.schema_profile.clone(),
        canonicalization: request.canonicalization.clone(),
        consumer: request.consumer.clone(),
        algorithm_version: request.algorithm_version.clone(),
        required_modalities: request.required_modalities.clone(),
        tool_id: request.tool_id.clone(),
        approval_token: request.approval_token.clone(),
        comparability_digest: request.comparability_digest.clone(),
        batch_id: request.batch_id.clone(),
        checkpoint_seq: request.checkpoint_seq,
        capacity: request.capacity,
        queue_digest: request.queue_digest.clone(),
        checkpoint_digest: request.checkpoint_digest.clone(),
        requested_output: request.requested_output.clone(),
        disposition,
        candidate_order,
        selected_order,
        omitted_order,
        overflow_order,
        uncertainty_order,
        negative_order,
        contradictory_order,
        replay_identity: request.replay_identity.clone(),
        synthesis_digest,
        copilot_digest,
        effect_receipts: if request.approval_token.trim().is_empty() {
            vec![format!("approval-required:{}", request.tool_id)]
        } else {
            vec![format!("{EFFECT_PREFIX}{}", request.copilot_id)]
        },
        artifact,
        raw_data_local: true,
        boundary: request.boundary.clone(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(
    request: &ThroughputRetrievalSynthesisResearchCopilotRequest,
) -> Result<(), ThroughputRetrievalSynthesisResearchCopilotError> {
    if request.copilot_id.trim().is_empty()
        || request.agent_id.trim().is_empty()
        || request.recommendation_mode != "evidence-ranked-read-only"
        || !request.approval_required
        || request.schema_profile != INPUT_SCHEMA
        || request.canonicalization != "aurora-json-canonical-v1"
        || request.consumer.trim().is_empty()
        || request.algorithm_version.trim().is_empty()
        || request.requested_output != OUTPUT_SCHEMA
        || request.required_modalities.len() < 2
        || !canonical(&request.required_modalities)
        || request.required_modalities != request.synthesis_request.query.required_modalities
        || request.tool_id.trim().is_empty()
        || request.comparability_digest.as_str().len() != 64
        || request.batch_id.trim().is_empty()
        || request.checkpoint_seq == 0
        || request.capacity == 0
        || request.synthesis_request.query.max_results == 0
        || request.queue_digest.as_str().len() != 64
        || request.checkpoint_digest.as_str().len() != 64
        || request.budget_units == 0
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.synthesis_request.boundary != PRECLINICAL_BOUNDARY
        || !request.synthesis_request.raw_data_local
        || request.replay_identity.as_str().len() != 64
    {
        return Err(ThroughputRetrievalSynthesisResearchCopilotError::Invalid(
            "contract identity, output, budget, locality, replay, or boundary is invalid".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retrieval_synthesis::{RetrievalCandidate, ScopedRetrievalQuery};
    use bioprism_foundation::{EvidenceAvailability, PolicyDecision};

    fn request() -> ThroughputRetrievalSynthesisResearchCopilotRequest {
        ThroughputRetrievalSynthesisResearchCopilotRequest {
            synthesis_request: EvidenceSynthesisRequest {
                request_id: "request:f21".into(),
                query: ScopedRetrievalQuery {
                    query_id: "query:multimodal-study".into(),
                    requester: "researcher:f21".into(),
                    intent: "retrieve preclinical evidence".into(),
                    study_ids: vec!["study:f21".into()],
                    required_modalities: vec!["imaging".into(), "transcriptomics".into()],
                    comparability_profile: "profile:f21".into(),
                    max_results: 4,
                },
                candidates: vec![
                    RetrievalCandidate {
                        evidence_id: "evidence:f21".into(),
                        study_id: "study:f21".into(),
                        modality: "imaging".into(),
                        comparability_profile: "profile:f21".into(),
                        digest: Some(ContentHash::of_bytes(b"f21")),
                        availability: EvidenceAvailability::Available,
                        relevance_score: 90,
                        negative_result: true,
                        locator: "local://f21".into(),
                    },
                    RetrievalCandidate {
                        evidence_id: "evidence:f21-transcriptomics".into(),
                        study_id: "study:f21".into(),
                        modality: "transcriptomics".into(),
                        comparability_profile: "profile:f21".into(),
                        digest: Some(ContentHash::of_bytes(b"f21-transcriptomics")),
                        availability: EvidenceAvailability::Available,
                        relevance_score: 88,
                        negative_result: false,
                        locator: "local://f21-transcriptomics".into(),
                    },
                ],
                policy_decision: PolicyDecision::Allow,
                protected_closure_satisfied: true,
                raw_data_local: true,
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            copilot_id: "engine:f21".into(),
            agent_id: "agent:f21".into(),
            recommendation_mode: "evidence-ranked-read-only".into(),
            approval_required: true,
            schema_profile: INPUT_SCHEMA.into(),
            canonicalization: "aurora-json-canonical-v1".into(),
            consumer: "researcher:f21".into(),
            algorithm_version: "2026.1".into(),
            required_modalities: vec!["imaging".into(), "transcriptomics".into()],
            tool_id: "tool:comparability-index".into(),
            approval_token: "approval:f21".into(),
            comparability_digest: ContentHash::of_bytes(b"comparability:f21"),
            batch_id: "batch:f21".into(),
            checkpoint_seq: 7,
            capacity: 1,
            queue_digest: ContentHash::of_bytes(b"queue:f21"),
            checkpoint_digest: ContentHash::of_bytes(b"checkpoint:f21"),
            requested_output: OUTPUT_SCHEMA.into(),
            budget_units: 4,
            replay_identity: ContentHash::of_bytes(b"replay:f21"),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn manifest_is_a2() {
        assert_eq!(
            throughput_retrieval_synthesis_research_copilot_manifest().autonomy_tier,
            AutonomyTier::A2
        );
    }

    #[test]
    fn computes_local_synthesis() {
        let receipt = run_throughput_retrieval_synthesis_research_copilot(&request()).unwrap();
        assert_eq!(receipt.feature_id, FEATURE_ID);
        assert_eq!(receipt.selected_order, vec!["evidence:f21"]);
    }

    #[test]
    fn negative_result_is_retained() {
        let receipt = run_throughput_retrieval_synthesis_research_copilot(&request()).unwrap();
        assert_eq!(receipt.negative_order, vec!["evidence:f21"]);
    }

    #[test]
    fn queue_overflow_is_retained() {
        let receipt = run_throughput_retrieval_synthesis_research_copilot(&request()).unwrap();
        assert_eq!(receipt.overflow_order, vec!["evidence:f21-transcriptomics"]);
        assert!(receipt
            .omitted_order
            .contains(&"evidence:f21-transcriptomics".into()));
    }

    #[test]
    fn policy_block_is_observable() {
        let mut value = request();
        value.synthesis_request.policy_decision = bioprism_foundation::PolicyDecision::Deny;
        let receipt = run_throughput_retrieval_synthesis_research_copilot(&value).unwrap();
        assert_eq!(receipt.disposition, EvidenceSynthesisDisposition::Blocked);
    }

    #[test]
    fn rejects_wrong_output() {
        let mut value = request();
        value.requested_output = "Wrong@1".into();
        assert!(run_throughput_retrieval_synthesis_research_copilot(&value).is_err());
    }

    #[test]
    fn replay_is_stable() {
        let value = request();
        assert_eq!(
            run_throughput_retrieval_synthesis_research_copilot(&value).unwrap(),
            run_throughput_retrieval_synthesis_research_copilot(&value).unwrap()
        );
    }

    #[test]
    fn missing_approval_is_blocked_and_observable() {
        let mut value = request();
        value.approval_token.clear();
        let receipt = run_throughput_retrieval_synthesis_research_copilot(&value).unwrap();
        assert_eq!(receipt.disposition, EvidenceSynthesisDisposition::Blocked);
        assert_eq!(
            receipt.effect_receipts,
            vec!["approval-required:tool:comparability-index"]
        );
    }

    #[test]
    fn requires_multiple_modalities() {
        let mut value = request();
        value.required_modalities = vec!["imaging".into()];
        value.synthesis_request.query.required_modalities = value.required_modalities.clone();
        assert!(run_throughput_retrieval_synthesis_research_copilot(&value).is_err());
    }
}
