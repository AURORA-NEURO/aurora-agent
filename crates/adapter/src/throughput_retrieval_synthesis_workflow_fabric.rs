//! Prospective high-throughput retrieval-and-synthesis workflow fabric.
//!
//! Atlas feature `AFA-adapter-P02-F15`: a resumable A2 workflow around the
//! bounded throughput retrieval copilot. Queue admission, overflow retention,
//! checkpoints, budget exhaustion, compensation, replay, and release effects
//! are first-class product state rather than hidden implementation details.

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

use crate::retrieval_synthesis::EvidenceSynthesisDisposition;
use crate::throughput_retrieval_synthesis_research_copilot::{
    run_throughput_retrieval_synthesis_research_copilot,
    ThroughputRetrievalSynthesisResearchCopilotRequest,
};

pub const FEATURE_ID: &str = "AFA-adapter-P02-F15";
pub const CONTRACT_VERSION: &str = "adapter-throughput-retrieval-synthesis-workflow-fabric/1.0";
pub const INPUT_SCHEMA: &str = "ScopedRetrievalQuery3@1";
pub const OUTPUT_SCHEMA: &str = "EvidenceSynthesis4@1";
pub const CANONICAL_STAGES: [&str; 6] = [
    "stage:checkpoint",
    "stage:admit-throughput-batch",
    "stage:reconcile-queue",
    "stage:compile-synthesis",
    "stage:persist-artifact",
    "stage:validate-output",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputRetrievalSynthesisWorkflowRequest {
    pub request: ThroughputRetrievalSynthesisResearchCopilotRequest,
    pub workflow_id: String,
    pub requested_stage_order: Vec<String>,
    pub checkpoint_id: String,
    pub checkpoint_seq: u64,
    pub budget_units: u32,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputRetrievalSynthesisWorkflowReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub query_id: String,
    pub batch_id: String,
    pub checkpoint_id: String,
    pub checkpoint_seq: u64,
    pub capacity: u32,
    pub queue_digest: ContentHash,
    pub comparability_digest: ContentHash,
    pub disposition: EvidenceSynthesisDisposition,
    pub stage_order: Vec<String>,
    pub plan_order: Vec<String>,
    pub completed_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub compensation_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub omitted_order: Vec<String>,
    pub overflow_order: Vec<String>,
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
pub enum ThroughputRetrievalSynthesisWorkflowError {
    #[error("invalid throughput retrieval workflow request: {0}")]
    Invalid(String),
    #[error("throughput retrieval workflow artifact failed: {0}")]
    Artifact(String),
    #[error("throughput retrieval workflow copilot failed: {0}")]
    Copilot(String),
}

fn sorted_unique(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_valid(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().chars().all(|c| c.is_ascii_hexdigit())
}

impl ThroughputRetrievalSynthesisWorkflowReceipt {
    pub fn validate(&self) -> Result<(), ThroughputRetrievalSynthesisWorkflowError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.query_id.trim().is_empty()
            || self.batch_id.trim().is_empty()
            || self.checkpoint_id.trim().is_empty()
            || self.checkpoint_seq == 0
            || self.capacity == 0
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
            return Err(ThroughputRetrievalSynthesisWorkflowError::Invalid(
                "throughput workflow identity, queue, checkpoint, stages, plan, locality, budget, or effects are incomplete".into(),
            ));
        }
        for values in [
            &self.plan_order,
            &self.blocked_order,
            &self.compensation_order,
            &self.candidate_order,
            &self.selected_order,
            &self.omitted_order,
            &self.overflow_order,
            &self.uncertainty_order,
            &self.negative_order,
            &self.contradictory_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if !sorted_unique(values) {
                return Err(ThroughputRetrievalSynthesisWorkflowError::Invalid(
                    "throughput workflow collections are not canonically ordered".into(),
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
            return Err(ThroughputRetrievalSynthesisWorkflowError::Invalid(
                "throughput workflow evidence states do not partition candidates".into(),
            ));
        }
        if self
            .overflow_order
            .iter()
            .any(|id| !self.omitted_order.contains(id))
        {
            return Err(ThroughputRetrievalSynthesisWorkflowError::Invalid(
                "throughput overflow must be an omitted candidate subset".into(),
            ));
        }
        if self.required_budget != self.plan_order.len() as u32 {
            return Err(ThroughputRetrievalSynthesisWorkflowError::Invalid(
                "throughput workflow budget does not cover its plan".into(),
            ));
        }
        let blocked = !self.blocked_order.is_empty();
        if blocked && !self.completed_order.is_empty() {
            return Err(ThroughputRetrievalSynthesisWorkflowError::Invalid(
                "blocked throughput workflow cannot report completed stages".into(),
            ));
        }
        if !blocked && self.completed_order != self.stage_order {
            return Err(ThroughputRetrievalSynthesisWorkflowError::Invalid(
                "unblocked throughput workflow must complete every stage".into(),
            ));
        }
        for digest in [
            &self.queue_digest,
            &self.comparability_digest,
            &self.synthesis_receipt_digest,
            &self.checkpoint_digest,
            &self.workflow_digest,
            &self.replay_identity,
            &self.artifact.content_hash,
        ] {
            if !digest_valid(digest) {
                return Err(ThroughputRetrievalSynthesisWorkflowError::Invalid(
                    "throughput workflow digest is invalid".into(),
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
            return Err(ThroughputRetrievalSynthesisWorkflowError::Invalid(
                "throughput workflow effect does not match disposition and compensation".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ThroughputRetrievalSynthesisWorkflowError::Artifact(error.to_string()))
    }
}

pub fn throughput_retrieval_synthesis_workflow_fabric_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "adapter".into(),
        consumers: ["AURORA extension developer".into(), "throughput research operator".into()].into(),
        behavior: "runs a resumable A2 prospective high-throughput retrieval and synthesis workflow with bounded queue admission, overflow retention, checkpoints, budget compensation, replay, and omission-preserving receipts".into(),
        value: "provides a separately versioned high-throughput workflow surface that prevents queue pressure and partial evidence from becoming silent success or clinical decisions".into(),
        inputs: vec![
            TypedPort { name: "scoped_retrieval_query".into(), schema: INPUT_SCHEMA.into(), required: true },
            TypedPort { name: "batch_admission".into(), schema: "ResearchBatchAdmission@1".into(), required: true },
        ],
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

pub fn schedule_throughput_retrieval_synthesis_workflow(
    request: &ThroughputRetrievalSynthesisWorkflowRequest,
) -> Result<ThroughputRetrievalSynthesisWorkflowReceipt, ThroughputRetrievalSynthesisWorkflowError>
{
    validate_request(request)?;
    let copilot = run_throughput_retrieval_synthesis_research_copilot(&request.request)
        .map_err(|error| ThroughputRetrievalSynthesisWorkflowError::Copilot(error.to_string()))?;
    let stage_order = CANONICAL_STAGES
        .iter()
        .map(|s| (*s).to_string())
        .collect::<Vec<_>>();
    let plan_order = BTreeSet::from([
        "plan:stage:checkpoint".to_string(),
        "plan:stage:admit-throughput-batch".to_string(),
        "plan:stage:reconcile-queue".to_string(),
        "plan:stage:compile-synthesis".to_string(),
        "plan:stage:persist-artifact".to_string(),
        "plan:stage:validate-output".to_string(),
        "plan:retain-overflow".to_string(),
        "plan:retain-unresolved-evidence".to_string(),
        "plan:persist-replayable-artifact".to_string(),
    ])
    .into_iter()
    .collect::<Vec<_>>();
    let required_budget = plan_order.len() as u32;
    let mut blocked_order = BTreeSet::new();
    let mut compensation = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = copilot
        .uncertainty_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if !request.policy_allow {
        blocked_order.insert("stage:policy-gate".to_string());
        omissions.insert("workflow:policy-denied".to_string());
        compensation.insert("compensate:research-work:policy-review".to_string());
    }
    if !request.protected_closure {
        blocked_order.insert("stage:protected-closure-gate".to_string());
        omissions.insert("workflow:protected-closure-incomplete".to_string());
        uncertainty.insert("workflow:protected-closure-unmeasured".to_string());
    }
    if !request.raw_data_local {
        blocked_order.insert("stage:locality-gate".to_string());
        omissions.insert("workflow:raw-data-locality-failed".to_string());
    }
    if request.budget_units < required_budget {
        blocked_order.insert("stage:budget-gate".to_string());
        omissions.insert("workflow:budget-exhausted".to_string());
        uncertainty.insert("workflow:budget-unmeasured".to_string());
        compensation.insert("compensate:research-work:budget-exhausted".to_string());
    }
    if copilot.disposition == EvidenceSynthesisDisposition::Blocked {
        blocked_order.insert("stage:approval-or-copilot-gate".to_string());
    }
    for id in &copilot.overflow_order {
        omissions.insert(format!("evidence:{id}:overflow"));
    }
    for id in &copilot.omitted_order {
        omissions.insert(format!("evidence:{id}:omitted"));
    }
    for id in &copilot.contradictory_order {
        omissions.insert(format!("evidence:{id}:contradictory"));
    }
    if !copilot.overflow_order.is_empty() {
        compensation.insert("compensate:research-work:retain-overflow".to_string());
    }
    if !copilot.omitted_order.is_empty() || !copilot.uncertainty_order.is_empty() {
        compensation.insert("compensate:research-work:retain-unresolved-evidence".to_string());
    }
    let blocked_order = blocked_order.into_iter().collect::<Vec<_>>();
    let compensation_order = compensation.into_iter().collect::<Vec<_>>();
    let omissions = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let disposition = if blocked_order.is_empty() {
        copilot.disposition.clone()
    } else {
        EvidenceSynthesisDisposition::Blocked
    };
    let completed_order = if blocked_order.is_empty() {
        stage_order.clone()
    } else {
        Vec::new()
    };
    let checkpoint_digest = ContentHash::of_value(&json!({
        "workflow_id": request.workflow_id,
        "checkpoint_id": request.checkpoint_id,
        "checkpoint_seq": request.checkpoint_seq,
        "stage_order": stage_order,
        "replay_identity": request.replay_identity,
    }))
    .map_err(|error| ThroughputRetrievalSynthesisWorkflowError::Artifact(error.to_string()))?;
    let synthesis_receipt_digest =
        ContentHash::of_value(&serde_json::to_value(&copilot).map_err(|error| {
            ThroughputRetrievalSynthesisWorkflowError::Artifact(error.to_string())
        })?)
        .map_err(|error| ThroughputRetrievalSynthesisWorkflowError::Artifact(error.to_string()))?;
    let workflow_digest = ContentHash::of_value(&json!({
        "workflow_id": request.workflow_id,
        "batch_id": request.request.batch_id,
        "plan_order": plan_order,
        "completed_order": completed_order,
        "blocked_order": blocked_order,
        "compensation_order": compensation_order,
        "checkpoint_digest": checkpoint_digest,
        "queue_digest": request.request.queue_digest,
        "budget_units": request.budget_units,
        "required_budget": required_budget,
        "replay_identity": request.replay_identity,
    }))
    .map_err(|error| ThroughputRetrievalSynthesisWorkflowError::Artifact(error.to_string()))?;
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
        "batch_id": request.request.batch_id,
        "checkpoint_id": request.checkpoint_id,
        "checkpoint_seq": request.checkpoint_seq,
        "capacity": request.request.capacity,
        "queue_digest": request.request.queue_digest,
        "comparability_digest": request.request.comparability_digest,
        "disposition": disposition,
        "stage_order": stage_order,
        "plan_order": plan_order,
        "completed_order": completed_order,
        "blocked_order": blocked_order,
        "compensation_order": compensation_order,
        "candidate_order": copilot.candidate_order,
        "selected_order": copilot.selected_order,
        "omitted_order": copilot.omitted_order,
        "overflow_order": copilot.overflow_order,
        "uncertainty_order": copilot.uncertainty_order,
        "negative_order": copilot.negative_order,
        "contradictory_order": copilot.contradictory_order,
        "synthesis_receipt_digest": synthesis_receipt_digest,
        "checkpoint_digest": checkpoint_digest,
        "workflow_digest": workflow_digest,
        "replay_identity": request.replay_identity,
        "budget_units": request.budget_units,
        "required_budget": required_budget,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": copilot.negative_order,
        "effect_receipts": effect_receipts,
        "raw_data_local": true,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "adapter-throughput-retrieval-workflow:{}",
            request.workflow_id
        ),
        "application/vnd.aurora.throughput-retrieval-synthesis-workflow+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ThroughputRetrievalSynthesisWorkflowError::Artifact(error.to_string()))?;
    let receipt = ThroughputRetrievalSynthesisWorkflowReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request.synthesis_request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        query_id: request.request.synthesis_request.query.query_id.clone(),
        batch_id: request.request.batch_id.clone(),
        checkpoint_id: request.checkpoint_id.clone(),
        checkpoint_seq: request.checkpoint_seq,
        capacity: request.request.capacity,
        queue_digest: request.request.queue_digest.clone(),
        comparability_digest: request.request.comparability_digest.clone(),
        disposition,
        stage_order,
        plan_order,
        completed_order,
        blocked_order,
        compensation_order,
        candidate_order: copilot.candidate_order.clone(),
        selected_order: copilot.selected_order.clone(),
        omitted_order: copilot.omitted_order.clone(),
        overflow_order: copilot.overflow_order.clone(),
        uncertainty_order: copilot.uncertainty_order.clone(),
        negative_order: copilot.negative_order.clone(),
        contradictory_order: copilot.contradictory_order.clone(),
        synthesis_receipt_digest,
        checkpoint_digest,
        workflow_digest,
        replay_identity: request.replay_identity.clone(),
        budget_units: request.budget_units,
        required_budget,
        omissions,
        uncertainty,
        negative_evidence: copilot.negative_order.clone(),
        effect_receipts,
        artifact,
        raw_data_local: request.raw_data_local,
        boundary: request.boundary.clone(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(
    request: &ThroughputRetrievalSynthesisWorkflowRequest,
) -> Result<(), ThroughputRetrievalSynthesisWorkflowError> {
    if request.workflow_id.trim().is_empty()
        || request.checkpoint_id.trim().is_empty()
        || request.checkpoint_seq == 0
        || request.budget_units == 0
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || !request.request.synthesis_request.raw_data_local
        || request.requested_stage_order
            != CANONICAL_STAGES
                .iter()
                .map(|s| (*s).to_string())
                .collect::<Vec<_>>()
        || request.request.checkpoint_seq != request.checkpoint_seq
        || !digest_valid(&request.replay_identity)
        || request.request.replay_identity != request.replay_identity
    {
        return Err(ThroughputRetrievalSynthesisWorkflowError::Invalid(
            "throughput workflow identity, canonical stages, checkpoint, budget, replay, locality, or boundary is invalid".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retrieval_synthesis::{
        EvidenceSynthesisRequest, RetrievalCandidate, ScopedRetrievalQuery,
    };
    use bioprism_foundation::{EvidenceAvailability, PolicyDecision};

    fn request() -> ThroughputRetrievalSynthesisWorkflowRequest {
        let replay = ContentHash::of_bytes(b"f15-replay");
        let comparability = ContentHash::of_bytes(b"f15-comparability");
        let queue = ContentHash::of_bytes(b"f15-queue");
        let checkpoint = ContentHash::of_bytes(b"f15-checkpoint");
        ThroughputRetrievalSynthesisWorkflowRequest {
            request: ThroughputRetrievalSynthesisResearchCopilotRequest {
                synthesis_request: EvidenceSynthesisRequest {
                    request_id: "request:f15".into(),
                    query: ScopedRetrievalQuery {
                        query_id: "query:f15".into(),
                        requester: "researcher:f15".into(),
                        intent: "process prospective preclinical retrieval batch".into(),
                        study_ids: vec!["study:f15".into()],
                        required_modalities: vec!["imaging".into(), "omics".into()],
                        comparability_profile: "profile:f15".into(),
                        max_results: 4,
                    },
                    candidates: (0..4)
                        .map(|index| RetrievalCandidate {
                            evidence_id: format!("evidence:f15-{index}"),
                            study_id: "study:f15".into(),
                            modality: if index % 2 == 0 { "imaging" } else { "omics" }.into(),
                            comparability_profile: "profile:f15".into(),
                            digest: Some(ContentHash::of_bytes(format!("f15-{index}").as_bytes())),
                            availability: EvidenceAvailability::Available,
                            relevance_score: 90u16 - index as u16,
                            negative_result: index == 3,
                            locator: format!("local://f15/{index}"),
                        })
                        .collect(),
                    policy_decision: PolicyDecision::Allow,
                    protected_closure_satisfied: true,
                    raw_data_local: true,
                    boundary: PRECLINICAL_BOUNDARY.into(),
                },
                copilot_id: "copilot:f15".into(),
                agent_id: "agent:f15".into(),
                recommendation_mode: "evidence-ranked-read-only".into(),
                approval_required: true,
                schema_profile: "ScopedRetrievalQuery@1".into(),
                canonicalization: "aurora-json-canonical-v1".into(),
                consumer: "AURORA extension developer".into(),
                algorithm_version: "2026.1".into(),
                required_modalities: vec!["imaging".into(), "omics".into()],
                tool_id: "research-tool:f15".into(),
                approval_token: "approval:f15".into(),
                comparability_digest: comparability,
                batch_id: "batch:f15".into(),
                checkpoint_seq: 3,
                capacity: 2,
                queue_digest: queue,
                checkpoint_digest: checkpoint,
                requested_output: "EvidenceSynthesis1@1".into(),
                budget_units: 12,
                replay_identity: replay.clone(),
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            workflow_id: "workflow:f15".into(),
            requested_stage_order: CANONICAL_STAGES.iter().map(|s| (*s).to_string()).collect(),
            checkpoint_id: "checkpoint:f15".into(),
            checkpoint_seq: 3,
            budget_units: 12,
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
            throughput_retrieval_synthesis_workflow_fabric_manifest().autonomy_tier,
            AutonomyTier::A2
        );
    }
    #[test]
    fn schedules_throughput_workflow() {
        let receipt = schedule_throughput_retrieval_synthesis_workflow(&request()).unwrap();
        assert_eq!(receipt.disposition, EvidenceSynthesisDisposition::Passed);
        assert!(receipt.effect_receipts[0].starts_with("compensate:"));
    }
    #[test]
    fn retains_overflow() {
        let receipt = schedule_throughput_retrieval_synthesis_workflow(&request()).unwrap();
        assert!(!receipt.overflow_order.is_empty());
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item.contains("overflow")));
    }
    #[test]
    fn policy_blocks() {
        let mut input = request();
        input.policy_allow = false;
        let receipt = schedule_throughput_retrieval_synthesis_workflow(&input).unwrap();
        assert_eq!(receipt.disposition, EvidenceSynthesisDisposition::Blocked);
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn budget_blocks_with_compensation() {
        let mut input = request();
        input.budget_units = 1;
        let receipt = schedule_throughput_retrieval_synthesis_workflow(&input).unwrap();
        assert_eq!(receipt.disposition, EvidenceSynthesisDisposition::Blocked);
        assert!(receipt
            .compensation_order
            .iter()
            .any(|item| item.contains("budget-exhausted")));
    }
    #[test]
    fn replay_is_stable() {
        let input = request();
        let first = schedule_throughput_retrieval_synthesis_workflow(&input).unwrap();
        let second = schedule_throughput_retrieval_synthesis_workflow(&input).unwrap();
        assert_eq!(first.workflow_digest, second.workflow_digest);
        assert_eq!(first.artifact.content_hash, second.artifact.content_hash);
    }
    #[test]
    fn rejects_checkpoint_mismatch() {
        let mut input = request();
        input.checkpoint_seq = 4;
        assert!(schedule_throughput_retrieval_synthesis_workflow(&input).is_err());
    }
}
