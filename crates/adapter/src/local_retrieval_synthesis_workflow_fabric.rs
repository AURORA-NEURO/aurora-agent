//! Local single-study retrieval-and-synthesis workflow fabric.
//!
//! Atlas feature `AFA-adapter-P02-F13`: a resumable A1 workflow around the
//! typed retrieval copilot. It gives operators durable stages, checkpoint and
//! replay identities, budget admission, compensation, and fail-closed policy
//! receipts while preserving every evidence omission and negative result.

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

use crate::local_retrieval_synthesis_research_copilot::{
    run_local_retrieval_synthesis_research_copilot, LocalRetrievalSynthesisResearchCopilotError,
    LocalRetrievalSynthesisResearchCopilotReceipt, LocalRetrievalSynthesisResearchCopilotRequest,
};
use crate::retrieval_synthesis::EvidenceSynthesisDisposition;

pub const FEATURE_ID: &str = "AFA-adapter-P02-F13";
pub const CONTRACT_VERSION: &str = "adapter-local-retrieval-synthesis-workflow-fabric/1.0";
pub const INPUT_SCHEMA: &str = "ScopedRetrievalQuery1@1";
pub const OUTPUT_SCHEMA: &str = "EvidenceSynthesis4@1";
const CANONICAL_STAGES: [&str; 4] = [
    "stage:checkpoint",
    "stage:compile-synthesis",
    "stage:persist-artifact",
    "stage:validate-input",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalRetrievalSynthesisWorkflowRequest {
    pub request: LocalRetrievalSynthesisResearchCopilotRequest,
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
pub struct LocalRetrievalSynthesisWorkflowReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub query_id: String,
    pub study_id: String,
    pub scope: String,
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
pub enum LocalRetrievalSynthesisWorkflowError {
    #[error("invalid local retrieval workflow request: {0}")]
    Invalid(String),
    #[error("local retrieval workflow artifact failed: {0}")]
    Artifact(String),
    #[error("local retrieval workflow copilot failed: {0}")]
    Copilot(String),
}

fn sorted_unique(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_valid(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().chars().all(|c| c.is_ascii_hexdigit())
}

impl LocalRetrievalSynthesisWorkflowReceipt {
    pub fn validate(&self) -> Result<(), LocalRetrievalSynthesisWorkflowError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.query_id.trim().is_empty()
            || self.study_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.checkpoint_id.trim().is_empty()
            || self.stage_order.is_empty()
            || self.plan_order.is_empty()
            || self.effect_receipts.is_empty()
            || self.budget_units == 0
            || self.required_budget == 0
        {
            return Err(LocalRetrievalSynthesisWorkflowError::Invalid(
                "workflow identity, stages, plan, locality, budget, or effects are incomplete"
                    .into(),
            ));
        }
        let expected_stages = CANONICAL_STAGES
            .iter()
            .map(|stage| (*stage).to_string())
            .collect::<Vec<_>>();
        if self.stage_order != expected_stages {
            return Err(LocalRetrievalSynthesisWorkflowError::Invalid(
                "workflow stage order is not canonical".into(),
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
                return Err(LocalRetrievalSynthesisWorkflowError::Invalid(
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
            return Err(LocalRetrievalSynthesisWorkflowError::Invalid(
                "workflow evidence states do not partition candidates".into(),
            ));
        }
        if self.required_budget != self.plan_order.len() as u32 {
            return Err(LocalRetrievalSynthesisWorkflowError::Invalid(
                "workflow budget does not cover its plan".into(),
            ));
        }
        let blocked = !self.blocked_order.is_empty();
        if blocked && !self.completed_order.is_empty() {
            return Err(LocalRetrievalSynthesisWorkflowError::Invalid(
                "blocked workflow cannot report completed stages".into(),
            ));
        }
        if !blocked && self.completed_order != self.stage_order {
            return Err(LocalRetrievalSynthesisWorkflowError::Invalid(
                "unblocked workflow must complete every stage".into(),
            ));
        }
        for digest in [
            &self.synthesis_receipt_digest,
            &self.checkpoint_digest,
            &self.workflow_digest,
            &self.replay_identity,
            &self.artifact.content_hash,
        ] {
            if !digest_valid(digest) {
                return Err(LocalRetrievalSynthesisWorkflowError::Invalid(
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
            return Err(LocalRetrievalSynthesisWorkflowError::Invalid(
                "workflow effect does not match disposition and compensation".into(),
            ));
        }
        let expected_checkpoint = ContentHash::of_value(&json!({
            "workflow_id": self.workflow_id,
            "checkpoint_id": self.checkpoint_id,
            "stage_order": self.stage_order,
            "replay_identity": self.replay_identity,
        }))
        .map_err(|error| LocalRetrievalSynthesisWorkflowError::Artifact(error.to_string()))?;
        if self.checkpoint_digest != expected_checkpoint {
            return Err(LocalRetrievalSynthesisWorkflowError::Invalid(
                "checkpoint digest does not match workflow identity".into(),
            ));
        }
        let expected_workflow = ContentHash::of_value(&json!({
            "workflow_id": self.workflow_id,
            "plan_order": self.plan_order,
            "completed_order": self.completed_order,
            "blocked_order": self.blocked_order,
            "compensation_order": self.compensation_order,
            "checkpoint_digest": self.checkpoint_digest,
            "budget_units": self.budget_units,
            "required_budget": self.required_budget,
            "replay_identity": self.replay_identity,
        }))
        .map_err(|error| LocalRetrievalSynthesisWorkflowError::Artifact(error.to_string()))?;
        if self.workflow_digest != expected_workflow {
            return Err(LocalRetrievalSynthesisWorkflowError::Invalid(
                "workflow digest does not match plan and budget state".into(),
            ));
        }
        if self.artifact.artifact_id
            != format!("adapter-local-retrieval-workflow:{}", self.workflow_id)
            || self.artifact.content_type
                != "application/vnd.aurora.local-retrieval-synthesis-workflow+json"
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(LocalRetrievalSynthesisWorkflowError::Artifact(
                "workflow artifact is not bound to its receipt".into(),
            ));
        }
        let payload = json!({
            "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
            "contract_version": CONTRACT_VERSION,
            "feature_id": FEATURE_ID,
            "request_id": self.request_id,
            "workflow_id": self.workflow_id,
            "query_id": self.query_id,
            "study_id": self.study_id,
            "scope": self.scope,
            "checkpoint_id": self.checkpoint_id,
            "disposition": self.disposition,
            "stage_order": self.stage_order,
            "plan_order": self.plan_order,
            "completed_order": self.completed_order,
            "blocked_order": self.blocked_order,
            "compensation_order": self.compensation_order,
            "candidate_order": self.candidate_order,
            "selected_order": self.selected_order,
            "omitted_order": self.omitted_order,
            "uncertainty_order": self.uncertainty_order,
            "negative_order": self.negative_order,
            "contradictory_order": self.contradictory_order,
            "synthesis_receipt_digest": self.synthesis_receipt_digest,
            "checkpoint_digest": self.checkpoint_digest,
            "workflow_digest": self.workflow_digest,
            "replay_identity": self.replay_identity,
            "budget_units": self.budget_units,
            "required_budget": self.required_budget,
            "omissions": self.omissions,
            "uncertainty": self.uncertainty,
            "negative_evidence": self.negative_evidence,
            "effect_receipts": self.effect_receipts,
            "raw_data_local": self.raw_data_local,
            "boundary": self.boundary,
        });
        self.artifact
            .verify_payload(&payload)
            .map_err(|error| LocalRetrievalSynthesisWorkflowError::Artifact(error.to_string()))?;
        self.artifact
            .validate_metadata()
            .map_err(|error| LocalRetrievalSynthesisWorkflowError::Artifact(error.to_string()))
    }
}

pub fn local_retrieval_synthesis_workflow_fabric_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "adapter".into(),
        consumers: ["consortium administrator".into(), "workflow operator".into()].into(),
        behavior: "schedules a checkpointed local single-study retrieval workflow with deterministic stages, budget admission, compensation, replay, and omission-preserving synthesis receipts".into(),
        value: "turns typed evidence synthesis into a resumable operator workflow without hiding omissions, negative results, or policy failures".into(),
        inputs: vec![
            TypedPort { name: "retrieval_workflow_request".into(), schema: INPUT_SCHEMA.into(), required: true },
            TypedPort { name: "workflow_checkpoint".into(), schema: "ResearchWorkflowCheckpoint@1".into(), required: true },
        ],
        outputs: vec![TypedPort { name: "evidence_synthesis_workflow_receipt".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(),
        permissions: ["execute:approved-workflows".into(), "read:local-research-artifacts".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "cwl".into(), state: EvidenceState::Supported, locator: Some("https://www.commonwl.org/specification/".into()) }],
        authority_requirements: Vec::new(),
        autonomy_tier: AutonomyTier::A1,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn schedule_local_retrieval_synthesis_workflow(
    request: &LocalRetrievalSynthesisWorkflowRequest,
) -> Result<LocalRetrievalSynthesisWorkflowReceipt, LocalRetrievalSynthesisWorkflowError> {
    validate_request(request)?;
    let synthesis: LocalRetrievalSynthesisResearchCopilotReceipt =
        run_local_retrieval_synthesis_research_copilot(&request.request).map_err(
            |error: LocalRetrievalSynthesisResearchCopilotError| {
                LocalRetrievalSynthesisWorkflowError::Copilot(error.to_string())
            },
        )?;
    let stage_order = CANONICAL_STAGES
        .iter()
        .map(|stage| (*stage).to_string())
        .collect::<Vec<_>>();
    let mut plan = stage_order
        .iter()
        .map(|stage| format!("plan:{stage}"))
        .collect::<BTreeSet<_>>();
    plan.insert("plan:retain-evidence-state".into());
    plan.insert("plan:persist-replayable-artifact".into());
    let plan_order = plan.into_iter().collect::<Vec<_>>();
    let required_budget = plan_order.len() as u32;
    let budget_blocked = request.budget_units < required_budget;
    let policy_blocked =
        !request.policy_allow || !request.protected_closure || !request.raw_data_local;
    let synthesis_blocked = synthesis.disposition == EvidenceSynthesisDisposition::Blocked;
    let blocked_gate = budget_blocked || policy_blocked || synthesis_blocked;
    let disposition = if blocked_gate {
        EvidenceSynthesisDisposition::Blocked
    } else {
        synthesis.disposition
    };
    let completed_order = if blocked_gate {
        Vec::new()
    } else {
        stage_order.clone()
    };
    let blocked_order = if blocked_gate {
        vec!["stage:release".into()]
    } else {
        Vec::new()
    };
    let mut compensation = BTreeSet::new();
    if budget_blocked {
        compensation.insert("compensate:research-work:budget-exhausted".into());
    }
    if !synthesis.selected_order.is_empty() && !synthesis.omitted_order.is_empty() {
        compensation.insert("compensate:research-work:retain-omitted-evidence".into());
    }
    if !request.policy_allow {
        compensation.insert("compensate:research-work:policy-review".into());
    }
    let compensation_order = compensation.into_iter().collect::<Vec<_>>();
    let mut omissions = synthesis
        .omitted_order
        .iter()
        .map(|id| format!("evidence:{id}:omitted"))
        .collect::<Vec<_>>();
    if !request.policy_allow {
        omissions.push("workflow:policy-denied".into());
    }
    if !request.protected_closure {
        omissions.push("workflow:protected-closure-incomplete".into());
    }
    if !request.raw_data_local {
        omissions.push("workflow:raw-data-locality-failed".into());
    }
    if budget_blocked {
        omissions.push("workflow:budget-exhausted".into());
    }
    omissions.sort();
    omissions.dedup();
    let mut uncertainty = synthesis.uncertainty_order.clone();
    if budget_blocked {
        uncertainty.push("workflow:budget-unmeasured".into());
    }
    uncertainty.sort();
    uncertainty.dedup();
    let negative = synthesis.negative_order.clone();
    let checkpoint_digest = ContentHash::of_value(&json!({
        "workflow_id": request.workflow_id,
        "checkpoint_id": request.checkpoint_id,
        "stage_order": stage_order,
        "replay_identity": request.replay_identity,
    }))
    .map_err(|error| LocalRetrievalSynthesisWorkflowError::Artifact(error.to_string()))?;
    let synthesis_receipt_digest = ContentHash::of_value(
        &serde_json::to_value(&synthesis)
            .map_err(|error| LocalRetrievalSynthesisWorkflowError::Artifact(error.to_string()))?,
    )
    .map_err(|error| LocalRetrievalSynthesisWorkflowError::Artifact(error.to_string()))?;
    let workflow_digest = ContentHash::of_value(&json!({
        "workflow_id": request.workflow_id,
        "plan_order": plan_order,
        "completed_order": completed_order,
        "blocked_order": blocked_order,
        "compensation_order": compensation_order,
        "checkpoint_digest": checkpoint_digest,
        "budget_units": request.budget_units,
        "required_budget": required_budget,
        "replay_identity": request.replay_identity,
    }))
    .map_err(|error| LocalRetrievalSynthesisWorkflowError::Artifact(error.to_string()))?;
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
        "study_id": request.request.synthesis_request.query.study_ids.first().cloned().unwrap_or_default(),
        "scope": request.request.synthesis_request.query.intent,
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
        "negative_evidence": negative,
        "effect_receipts": effect_receipts,
        "raw_data_local": true,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("adapter-local-retrieval-workflow:{}", request.workflow_id),
        "application/vnd.aurora.local-retrieval-synthesis-workflow+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| LocalRetrievalSynthesisWorkflowError::Artifact(error.to_string()))?;
    let receipt = LocalRetrievalSynthesisWorkflowReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request.synthesis_request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        query_id: request.request.synthesis_request.query.query_id.clone(),
        study_id: request
            .request
            .synthesis_request
            .query
            .study_ids
            .first()
            .cloned()
            .unwrap_or_default(),
        scope: request.request.synthesis_request.query.intent.clone(),
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
        negative_evidence: negative,
        effect_receipts,
        artifact,
        raw_data_local: request.raw_data_local,
        boundary: request.boundary.clone(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(
    request: &LocalRetrievalSynthesisWorkflowRequest,
) -> Result<(), LocalRetrievalSynthesisWorkflowError> {
    if request.workflow_id.trim().is_empty()
        || request.checkpoint_id.trim().is_empty()
        || request.budget_units == 0
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || !request.request.synthesis_request.raw_data_local
        || request.request.synthesis_request.query.study_ids.len() != 1
        || request.request.synthesis_request.query.max_results == 0
        || request.requested_stage_order
            != CANONICAL_STAGES
                .iter()
                .map(|stage| (*stage).to_string())
                .collect::<Vec<_>>()
        || !digest_valid(&request.replay_identity)
    {
        return Err(LocalRetrievalSynthesisWorkflowError::Invalid(
            "workflow identity, single-study query, stage order, budget, locality, replay, or boundary is invalid".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local_retrieval_synthesis_research_copilot::LocalRetrievalSynthesisResearchCopilotRequest;
    use crate::retrieval_synthesis::{
        EvidenceSynthesisRequest, RetrievalCandidate, ScopedRetrievalQuery,
    };
    use bioprism_foundation::{EvidenceAvailability, PolicyDecision};

    fn request() -> LocalRetrievalSynthesisWorkflowRequest {
        LocalRetrievalSynthesisWorkflowRequest {
            request: LocalRetrievalSynthesisResearchCopilotRequest {
                synthesis_request: EvidenceSynthesisRequest {
                    request_id: "request:f13".into(),
                    query: ScopedRetrievalQuery {
                        query_id: "query:f13".into(),
                        requester: "researcher:f13".into(),
                        intent: "retrieve one preclinical study".into(),
                        study_ids: vec!["study:f13".into()],
                        required_modalities: vec!["imaging".into()],
                        comparability_profile: "profile:f13".into(),
                        max_results: 2,
                    },
                    candidates: vec![RetrievalCandidate {
                        evidence_id: "evidence:f13".into(),
                        study_id: "study:f13".into(),
                        modality: "imaging".into(),
                        comparability_profile: "profile:f13".into(),
                        digest: Some(ContentHash::of_bytes(b"f13")),
                        availability: EvidenceAvailability::Available,
                        relevance_score: 91,
                        negative_result: true,
                        locator: "local://f13".into(),
                    }],
                    policy_decision: PolicyDecision::Allow,
                    protected_closure_satisfied: true,
                    raw_data_local: true,
                    boundary: PRECLINICAL_BOUNDARY.into(),
                },
                copilot_id: "copilot:f13".into(),
                agent_id: "agent:f13".into(),
                recommendation_mode: "evidence-ranked-read-only".into(),
                approval_required: true,
                schema_profile: "ScopedRetrievalQuery@1".into(),
                canonicalization: "aurora-json-canonical-v1".into(),
                consumer: "consortium administrator".into(),
                algorithm_version: "2026.1".into(),
                requested_output: "EvidenceSynthesis1@1".into(),
                budget_units: 4,
                replay_identity: ContentHash::of_bytes(b"replay:f13"),
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            workflow_id: "workflow:f13".into(),
            requested_stage_order: CANONICAL_STAGES
                .iter()
                .map(|stage| (*stage).to_string())
                .collect(),
            checkpoint_id: "checkpoint:f13".into(),
            budget_units: 6,
            replay_identity: ContentHash::of_bytes(b"workflow-replay:f13"),
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            local_retrieval_synthesis_workflow_fabric_manifest().autonomy_tier,
            AutonomyTier::A1
        );
    }
    #[test]
    fn schedules_local_workflow() {
        let receipt = schedule_local_retrieval_synthesis_workflow(&request()).unwrap();
        assert!(receipt.effect_receipts[0].starts_with("schedule:"));
        assert_eq!(receipt.selected_order, vec!["evidence:f13"]);
    }
    #[test]
    fn preserves_negative_evidence() {
        let receipt = schedule_local_retrieval_synthesis_workflow(&request()).unwrap();
        assert_eq!(receipt.negative_order, vec!["evidence:f13"]);
    }
    #[test]
    fn policy_blocks() {
        let mut value = request();
        value.policy_allow = false;
        let receipt = schedule_local_retrieval_synthesis_workflow(&value).unwrap();
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn budget_blocks_with_compensation() {
        let mut value = request();
        value.budget_units = 1;
        let receipt = schedule_local_retrieval_synthesis_workflow(&value).unwrap();
        assert_eq!(receipt.disposition, EvidenceSynthesisDisposition::Blocked);
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item == "workflow:budget-exhausted"));
    }
    #[test]
    fn replay_is_stable() {
        let value = request();
        assert_eq!(
            schedule_local_retrieval_synthesis_workflow(&value).unwrap(),
            schedule_local_retrieval_synthesis_workflow(&value).unwrap()
        );
    }
    #[test]
    fn rejects_multiple_studies() {
        let mut value = request();
        value
            .request
            .synthesis_request
            .query
            .study_ids
            .push("study:extra".into());
        assert!(schedule_local_retrieval_synthesis_workflow(&value).is_err());
    }
}
