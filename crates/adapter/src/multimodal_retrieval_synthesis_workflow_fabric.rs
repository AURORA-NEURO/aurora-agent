//! Multimodal multi-study retrieval-and-synthesis workflow fabric.
//!
//! Atlas feature `AFA-adapter-P02-F14`: an A2 resumable workflow around the
//! multimodal retrieval copilot.  The fabric is independently callable and
//! records canonical stages, budget admission, compensation, replay, and
//! content-addressed local artifacts without collapsing incomparable or
//! negative evidence into a pass.

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

use crate::multimodal_retrieval_synthesis_research_copilot::{
    run_multimodal_retrieval_synthesis_research_copilot,
    MultimodalRetrievalSynthesisResearchCopilotRequest,
};
use crate::retrieval_synthesis::EvidenceSynthesisDisposition;

pub const FEATURE_ID: &str = "AFA-adapter-P02-F14";
pub const CONTRACT_VERSION: &str = "adapter-multimodal-retrieval-synthesis-workflow-fabric/1.0";
pub const INPUT_SCHEMA: &str = "ScopedRetrievalQuery2@1";
pub const OUTPUT_SCHEMA: &str = "EvidenceSynthesis4@1";
pub const CANONICAL_STAGES: [&str; 4] = [
    "stage:checkpoint",
    "stage:validate-comparability",
    "stage:compile-synthesis",
    "stage:persist-artifact",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalRetrievalSynthesisWorkflowRequest {
    pub request: MultimodalRetrievalSynthesisResearchCopilotRequest,
    pub workflow_id: String,
    pub requested_stage_order: Vec<String>,
    pub checkpoint_id: String,
    pub budget_units: u32,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalRetrievalSynthesisWorkflowReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub query_id: String,
    pub study_order: Vec<String>,
    pub required_modalities: Vec<String>,
    pub comparability_digest: ContentHash,
    pub checkpoint_id: String,
    pub disposition: EvidenceSynthesisDisposition,
    pub stage_order: Vec<String>,
    pub plan_order: Vec<String>,
    pub completed_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub compensation_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub omitted_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_order: Vec<String>,
    pub contradictory_order: Vec<String>,
    pub synthesis_receipt_digest: ContentHash,
    pub checkpoint_digest: ContentHash,
    pub workflow_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub budget_units: u32,
    pub required_budget: u32,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum MultimodalRetrievalSynthesisWorkflowError {
    #[error("invalid multimodal retrieval workflow request: {0}")]
    Invalid(String),
    #[error("multimodal retrieval workflow artifact failed: {0}")]
    Artifact(String),
    #[error("multimodal retrieval workflow copilot failed: {0}")]
    Copilot(String),
}

fn sorted_unique(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_valid(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().chars().all(|c| c.is_ascii_hexdigit())
}

impl MultimodalRetrievalSynthesisWorkflowReceipt {
    pub fn validate(&self) -> Result<(), MultimodalRetrievalSynthesisWorkflowError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.query_id.trim().is_empty()
            || self.study_order.len() < 2
            || !sorted_unique(&self.study_order)
            || self.required_modalities.len() < 2
            || !sorted_unique(&self.required_modalities)
            || self.checkpoint_id.trim().is_empty()
            || self.stage_order
                != CANONICAL_STAGES
                    .iter()
                    .map(|s| (*s).to_string())
                    .collect::<Vec<_>>()
            || self.plan_order.is_empty()
            || self.effect_receipts.is_empty()
            || self.budget_units == 0
            || self.required_budget == 0
        {
            return Err(MultimodalRetrievalSynthesisWorkflowError::Invalid(
                "workflow identity, multi-study comparability, stages, plan, locality, budget, or effects are incomplete".into(),
            ));
        }
        for values in [
            &self.plan_order,
            &self.blocked_order,
            &self.compensation_order,
            &self.candidate_order,
            &self.selected_order,
            &self.omitted_order,
            &self.uncertainty_order,
            &self.negative_order,
            &self.contradictory_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if !sorted_unique(values) {
                return Err(MultimodalRetrievalSynthesisWorkflowError::Invalid(
                    "workflow collections are not canonically ordered".into(),
                ));
            }
        }
        let classified = self
            .selected_order
            .iter()
            .chain(self.omitted_order.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        if classified
            != self
                .candidate_order
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>()
        {
            return Err(MultimodalRetrievalSynthesisWorkflowError::Invalid(
                "workflow evidence states do not partition candidates".into(),
            ));
        }
        if self.required_budget != self.plan_order.len() as u32 {
            return Err(MultimodalRetrievalSynthesisWorkflowError::Invalid(
                "workflow budget does not cover its plan".into(),
            ));
        }
        let blocked = !self.blocked_order.is_empty();
        if blocked && !self.completed_order.is_empty() {
            return Err(MultimodalRetrievalSynthesisWorkflowError::Invalid(
                "blocked workflow cannot report completed stages".into(),
            ));
        }
        if !blocked && self.completed_order != self.stage_order {
            return Err(MultimodalRetrievalSynthesisWorkflowError::Invalid(
                "unblocked workflow must complete every stage".into(),
            ));
        }
        for digest in [
            &self.comparability_digest,
            &self.synthesis_receipt_digest,
            &self.checkpoint_digest,
            &self.workflow_digest,
            &self.replay_identity,
            &self.artifact.content_hash,
        ] {
            if !digest_valid(digest) {
                return Err(MultimodalRetrievalSynthesisWorkflowError::Invalid(
                    "workflow digest is invalid".into(),
                ));
            }
        }
        let expected_effect = if self.disposition == EvidenceSynthesisDisposition::Blocked {
            "block:unsafe-release".to_string()
        } else if !self.compensation_order.is_empty() {
            format!("compensate:research-work:{}", self.workflow_id)
        } else {
            format!("schedule:research-work:{}", self.workflow_id)
        };
        if self.effect_receipts != vec![expected_effect] {
            return Err(MultimodalRetrievalSynthesisWorkflowError::Invalid(
                "workflow effect does not match disposition and compensation".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| MultimodalRetrievalSynthesisWorkflowError::Artifact(error.to_string()))
    }
}

pub fn multimodal_retrieval_synthesis_workflow_fabric_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "adapter".into(),
        consumers: ["integration engineer".into(), "preclinical researcher".into()].into(),
        behavior: "runs a resumable A2 multimodal multi-study retrieval and synthesis workflow with comparability, checkpoint, budget, compensation, replay, and omission-preserving receipts".into(),
        value: "provides a separately versioned workflow protocol for auditable imaging, omics, and assay synthesis without silently treating incomparable or missing evidence as a pass".into(),
        inputs: vec![TypedPort { name: "scoped_retrieval_query".into(), schema: INPUT_SCHEMA.into(), required: true }],
        outputs: vec![TypedPort { name: "evidence_synthesis".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(),
        permissions: ["execute:approved-workflows".into(), "read:local-research-artifacts".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "cwl".into(), state: EvidenceState::Supported, locator: Some("https://www.commonwl.org/specification/".into()) }],
        authority_requirements: Vec::new(),
        autonomy_tier: AutonomyTier::A2,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn schedule_multimodal_retrieval_synthesis_workflow(
    request: &MultimodalRetrievalSynthesisWorkflowRequest,
) -> Result<MultimodalRetrievalSynthesisWorkflowReceipt, MultimodalRetrievalSynthesisWorkflowError>
{
    validate_request(request)?;
    let synthesis = run_multimodal_retrieval_synthesis_research_copilot(&request.request)
        .map_err(|error| MultimodalRetrievalSynthesisWorkflowError::Copilot(error.to_string()))?;
    let stage_order = CANONICAL_STAGES
        .iter()
        .map(|s| (*s).to_string())
        .collect::<Vec<_>>();
    let plan = BTreeSet::from([
        "plan:persist-replayable-artifact".to_string(),
        "plan:retain-comparability-state".to_string(),
        "plan:stage:checkpoint".to_string(),
        "plan:stage:validate-comparability".to_string(),
        "plan:stage:compile-synthesis".to_string(),
        "plan:stage:persist-artifact".to_string(),
    ]);
    let plan_order = plan.iter().cloned().collect::<Vec<_>>();
    let required_budget = plan_order.len() as u32;
    let mut blocked_order = Vec::new();
    let mut completed_order = Vec::new();
    let mut compensation = BTreeSet::new();
    let mut omissions = synthesis
        .omitted_order
        .iter()
        .map(|id| format!("evidence:{id}:omitted"))
        .collect::<BTreeSet<_>>();
    let mut uncertainty = synthesis
        .uncertainty_order
        .iter()
        .map(|id| format!("evidence:{id}:uncertain"))
        .collect::<BTreeSet<_>>();
    let comparability_blocked = synthesis.disposition == EvidenceSynthesisDisposition::Blocked;
    if !request.policy_allow {
        blocked_order.push("stage:policy-gate".into());
    }
    if !request.protected_closure {
        blocked_order.push("stage:protected-closure-gate".into());
        uncertainty.insert("workflow:protected-closure-unmeasured".into());
    }
    if !request.raw_data_local {
        blocked_order.push("stage:locality-gate".into());
    }
    if request.budget_units < required_budget {
        blocked_order.push("stage:budget-gate".into());
        uncertainty.insert("workflow:budget-unmeasured".into());
    }
    if comparability_blocked {
        blocked_order.push("stage:comparability-gate".into());
    }
    if blocked_order.is_empty() {
        completed_order = stage_order.clone();
    } else {
        compensation.insert("compensate:retain-comparability-state".into());
    }
    if !synthesis.omitted_order.is_empty() {
        compensation.insert("compensate:retain-omitted-evidence".into());
    }
    if !synthesis.uncertainty_order.is_empty() {
        compensation.insert("compensate:retain-uncertainty".into());
    }
    omissions.extend(
        synthesis
            .contradictory_order
            .iter()
            .map(|id| format!("evidence:{id}:contradictory")),
    );
    let blocked_order = {
        blocked_order.sort();
        blocked_order.dedup();
        blocked_order
    };
    let compensation_order = compensation.into_iter().collect::<Vec<_>>();
    let omissions = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence = synthesis.negative_order.clone();
    let checkpoint_digest = ContentHash::of_value(&json!({"workflow_id": request.workflow_id, "checkpoint_id": request.checkpoint_id, "stage_order": stage_order, "replay_identity": request.replay_identity}))
        .map_err(|error| MultimodalRetrievalSynthesisWorkflowError::Artifact(error.to_string()))?;
    let synthesis_receipt_digest =
        ContentHash::of_value(&serde_json::to_value(&synthesis).map_err(|error| {
            MultimodalRetrievalSynthesisWorkflowError::Artifact(error.to_string())
        })?)
        .map_err(|error| MultimodalRetrievalSynthesisWorkflowError::Artifact(error.to_string()))?;
    let workflow_digest = ContentHash::of_value(&json!({"workflow_id": request.workflow_id, "plan_order": plan_order, "completed_order": completed_order, "blocked_order": blocked_order, "compensation_order": compensation_order, "checkpoint_digest": checkpoint_digest, "budget_units": request.budget_units, "required_budget": required_budget, "comparability_digest": request.request.comparability_digest, "replay_identity": request.replay_identity}))
        .map_err(|error| MultimodalRetrievalSynthesisWorkflowError::Artifact(error.to_string()))?;
    let disposition = if !blocked_order.is_empty() {
        EvidenceSynthesisDisposition::Blocked
    } else {
        synthesis.disposition.clone()
    };
    let effect_receipts = if disposition == EvidenceSynthesisDisposition::Blocked {
        vec!["block:unsafe-release".into()]
    } else if !compensation_order.is_empty() {
        vec![format!("compensate:research-work:{}", request.workflow_id)]
    } else {
        vec![format!("schedule:research-work:{}", request.workflow_id)]
    };
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request.synthesis_request.request_id,
        "workflow_id": request.workflow_id,
        "query_id": request.request.synthesis_request.query.query_id,
        "study_order": request.request.synthesis_request.query.study_ids,
        "required_modalities": request.request.required_modalities,
        "comparability_digest": request.request.comparability_digest,
        "checkpoint_id": request.checkpoint_id,
        "disposition": disposition,
        "stage_order": stage_order,
        "plan_order": plan_order,
        "completed_order": completed_order,
        "blocked_order": blocked_order,
        "compensation_order": compensation_order,
        "candidate_order": synthesis.candidate_order,
        "selected_order": synthesis.selected_order,
        "omitted_order": synthesis.omitted_order,
        "uncertainty_order": synthesis.uncertainty_order,
        "negative_order": synthesis.negative_order,
        "contradictory_order": synthesis.contradictory_order,
        "synthesis_receipt_digest": synthesis_receipt_digest,
        "checkpoint_digest": checkpoint_digest,
        "workflow_digest": workflow_digest,
        "replay_identity": request.replay_identity,
        "budget_units": request.budget_units,
        "required_budget": required_budget,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": negative_evidence,
        "effect_receipts": effect_receipts,
        "raw_data_local": true,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "adapter-multimodal-retrieval-workflow:{}",
            request.workflow_id
        ),
        "application/vnd.aurora.multimodal-retrieval-synthesis-workflow+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| MultimodalRetrievalSynthesisWorkflowError::Artifact(error.to_string()))?;
    let receipt = MultimodalRetrievalSynthesisWorkflowReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request.synthesis_request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        query_id: request.request.synthesis_request.query.query_id.clone(),
        study_order: request.request.synthesis_request.query.study_ids.clone(),
        required_modalities: request.request.required_modalities.clone(),
        comparability_digest: request.request.comparability_digest.clone(),
        checkpoint_id: request.checkpoint_id.clone(),
        disposition,
        stage_order,
        plan_order,
        completed_order,
        blocked_order,
        compensation_order,
        candidate_order: synthesis.candidate_order.clone(),
        selected_order: synthesis.selected_order.clone(),
        omitted_order: synthesis.omitted_order.clone(),
        uncertainty_order: synthesis.uncertainty_order.clone(),
        negative_order: synthesis.negative_order.clone(),
        contradictory_order: synthesis.contradictory_order.clone(),
        synthesis_receipt_digest,
        checkpoint_digest,
        workflow_digest,
        replay_identity: request.replay_identity.clone(),
        budget_units: request.budget_units,
        required_budget,
        omissions,
        uncertainty,
        negative_evidence,
        effect_receipts,
        artifact,
        raw_data_local: request.raw_data_local,
        boundary: request.boundary.clone(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(
    request: &MultimodalRetrievalSynthesisWorkflowRequest,
) -> Result<(), MultimodalRetrievalSynthesisWorkflowError> {
    if request.workflow_id.trim().is_empty()
        || request.checkpoint_id.trim().is_empty()
        || request.budget_units == 0
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || !request.request.synthesis_request.raw_data_local
        || request.request.synthesis_request.query.study_ids.len() < 2
        || request.request.required_modalities.len() < 2
        || request.requested_stage_order
            != CANONICAL_STAGES
                .iter()
                .map(|s| (*s).to_string())
                .collect::<Vec<_>>()
        || !digest_valid(&request.replay_identity)
        || !digest_valid(&request.request.comparability_digest)
    {
        return Err(MultimodalRetrievalSynthesisWorkflowError::Invalid("workflow identity, multi-study comparability, stage order, budget, locality, replay, or boundary is invalid".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::multimodal_retrieval_synthesis_research_copilot::MultimodalRetrievalSynthesisResearchCopilotRequest;
    use crate::retrieval_synthesis::{
        EvidenceSynthesisRequest, RetrievalCandidate, ScopedRetrievalQuery,
    };
    use bioprism_foundation::{EvidenceAvailability, PolicyDecision};

    fn request() -> MultimodalRetrievalSynthesisWorkflowRequest {
        let replay = ContentHash::of_bytes(b"f14-replay");
        let comparability = ContentHash::of_bytes(b"f14-comparability");
        MultimodalRetrievalSynthesisWorkflowRequest {
            request: MultimodalRetrievalSynthesisResearchCopilotRequest {
                synthesis_request: EvidenceSynthesisRequest {
                    request_id: "request:f14".into(),
                    query: ScopedRetrievalQuery {
                        query_id: "query:f14".into(),
                        requester: "researcher:f14".into(),
                        intent: "compare preclinical imaging and omics studies".into(),
                        study_ids: vec!["study:a".into(), "study:b".into()],
                        required_modalities: vec!["imaging".into(), "omics".into()],
                        comparability_profile: "profile:f14".into(),
                        max_results: 3,
                    },
                    candidates: vec![
                        RetrievalCandidate {
                            evidence_id: "evidence:f14-a".into(),
                            study_id: "study:a".into(),
                            modality: "imaging".into(),
                            comparability_profile: "profile:f14".into(),
                            digest: Some(ContentHash::of_bytes(b"f14-a")),
                            availability: EvidenceAvailability::Available,
                            relevance_score: 91,
                            negative_result: true,
                            locator: "local://f14/a".into(),
                        },
                        RetrievalCandidate {
                            evidence_id: "evidence:f14-b".into(),
                            study_id: "study:b".into(),
                            modality: "omics".into(),
                            comparability_profile: "profile:f14".into(),
                            digest: Some(ContentHash::of_bytes(b"f14-b")),
                            availability: EvidenceAvailability::Available,
                            relevance_score: 88,
                            negative_result: false,
                            locator: "local://f14/b".into(),
                        },
                    ],
                    policy_decision: PolicyDecision::Allow,
                    protected_closure_satisfied: true,
                    raw_data_local: true,
                    boundary: PRECLINICAL_BOUNDARY.into(),
                },
                copilot_id: "copilot:f14".into(),
                agent_id: "agent:f14".into(),
                recommendation_mode: "evidence-ranked-read-only".into(),
                approval_required: true,
                schema_profile: "ScopedRetrievalQuery@1".into(),
                canonicalization: "aurora-json-canonical-v1".into(),
                consumer: "integration engineer".into(),
                algorithm_version: "2026.1".into(),
                required_modalities: vec!["imaging".into(), "omics".into()],
                tool_id: "research-tool:f14".into(),
                approval_token: "approval:f14".into(),
                comparability_digest: comparability,
                requested_output: "EvidenceSynthesis1@1".into(),
                budget_units: 8,
                replay_identity: replay.clone(),
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            workflow_id: "workflow:f14".into(),
            requested_stage_order: CANONICAL_STAGES.iter().map(|s| (*s).to_string()).collect(),
            checkpoint_id: "checkpoint:f14".into(),
            budget_units: 8,
            replay_identity: replay,
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn manifest_is_a2() {
        assert_eq!(
            multimodal_retrieval_synthesis_workflow_fabric_manifest().autonomy_tier,
            AutonomyTier::A2
        );
    }
    #[test]
    fn schedules_multimodal_workflow() {
        let receipt = schedule_multimodal_retrieval_synthesis_workflow(&request()).unwrap();
        assert_eq!(receipt.disposition, EvidenceSynthesisDisposition::Passed);
        assert!(receipt.study_order.len() >= 2);
        assert!(receipt.effect_receipts[0].starts_with("schedule:"));
    }
    #[test]
    fn preserves_negative_evidence() {
        let receipt = schedule_multimodal_retrieval_synthesis_workflow(&request()).unwrap();
        assert!(receipt
            .negative_evidence
            .contains(&"evidence:f14-a".to_string()));
    }
    #[test]
    fn policy_blocks() {
        let mut input = request();
        input.policy_allow = false;
        let receipt = schedule_multimodal_retrieval_synthesis_workflow(&input).unwrap();
        assert_eq!(receipt.disposition, EvidenceSynthesisDisposition::Blocked);
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn budget_compensates() {
        let mut input = request();
        input.budget_units = 1;
        let receipt = schedule_multimodal_retrieval_synthesis_workflow(&input).unwrap();
        assert_eq!(receipt.disposition, EvidenceSynthesisDisposition::Blocked);
        assert!(receipt
            .compensation_order
            .iter()
            .any(|v| v == "compensate:retain-comparability-state"));
    }
    #[test]
    fn replay_is_stable() {
        let input = request();
        let a = schedule_multimodal_retrieval_synthesis_workflow(&input).unwrap();
        let b = schedule_multimodal_retrieval_synthesis_workflow(&input).unwrap();
        assert_eq!(a.workflow_digest, b.workflow_digest);
        assert_eq!(a.artifact.content_hash, b.artifact.content_hash);
    }
    #[test]
    fn rejects_single_study() {
        let mut input = request();
        input.request.synthesis_request.query.study_ids.truncate(1);
        assert!(schedule_multimodal_retrieval_synthesis_workflow(&input).is_err());
    }
}
