//! Federated continual retrieval-and-synthesis workflow fabric.
//!
//! Atlas feature `AFA-adapter-P02-F16`: a resumable A2 workflow around the
//! purpose-bound federated continual copilot. It exchanges only permitted
//! aggregate metadata, proves peer/quorum and locality gates, and preserves
//! every omission, uncertainty, overflow, contradiction, and negative result.

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

use crate::federated_continual_retrieval_synthesis_research_copilot::{
    run_federated_continual_retrieval_synthesis_research_copilot,
    FederatedContinualRetrievalSynthesisResearchCopilotRequest,
};
use crate::retrieval_synthesis::EvidenceSynthesisDisposition;

pub const FEATURE_ID: &str = "AFA-adapter-P02-F16";
pub const CONTRACT_VERSION: &str =
    "adapter-federated-continual-retrieval-synthesis-workflow-fabric/1.0";
pub const INPUT_SCHEMA: &str = "ScopedRetrievalQuery4@1";
pub const OUTPUT_SCHEMA: &str = "EvidenceSynthesis4@1";
pub const CANONICAL_STAGES: [&str; 7] = [
    "stage:checkpoint",
    "stage:validate-federation",
    "stage:admit-peer-quorum",
    "stage:compile-synthesis",
    "stage:seal-aggregate-envelope",
    "stage:persist-artifact",
    "stage:validate-output",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContinualRetrievalSynthesisWorkflowRequest {
    pub request: FederatedContinualRetrievalSynthesisResearchCopilotRequest,
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
pub struct FederatedContinualRetrievalSynthesisWorkflowReceipt {
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
    pub federation_id: String,
    pub purpose: String,
    pub peer_ids: Vec<String>,
    pub min_peer_quorum: u32,
    pub aggregate_only: bool,
    pub endpoint: String,
    pub federation_digest: ContentHash,
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
pub enum FederatedContinualRetrievalSynthesisWorkflowError {
    #[error("invalid federated continual retrieval workflow request: {0}")]
    Invalid(String),
    #[error("federated continual retrieval workflow artifact failed: {0}")]
    Artifact(String),
    #[error("federated continual retrieval workflow copilot failed: {0}")]
    Copilot(String),
}

fn sorted_unique(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}
fn digest_valid(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().chars().all(|c| c.is_ascii_hexdigit())
}

impl FederatedContinualRetrievalSynthesisWorkflowReceipt {
    pub fn validate(&self) -> Result<(), FederatedContinualRetrievalSynthesisWorkflowError> {
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
            || self.federation_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.peer_ids.is_empty()
            || self.min_peer_quorum == 0
            || self.peer_ids.len() < self.min_peer_quorum as usize
            || !sorted_unique(&self.peer_ids)
            || !self.aggregate_only
            || self.endpoint.trim().is_empty()
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
            return Err(FederatedContinualRetrievalSynthesisWorkflowError::Invalid("federated workflow identity, peer quorum, aggregate-only, stages, budget, locality, or effects are incomplete".into()));
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
                return Err(FederatedContinualRetrievalSynthesisWorkflowError::Invalid(
                    "federated workflow collections are not canonically ordered".into(),
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
            return Err(FederatedContinualRetrievalSynthesisWorkflowError::Invalid(
                "federated workflow evidence states do not partition candidates".into(),
            ));
        }
        if self
            .overflow_order
            .iter()
            .any(|id| !self.omitted_order.contains(id))
        {
            return Err(FederatedContinualRetrievalSynthesisWorkflowError::Invalid(
                "federated overflow must be an omitted candidate subset".into(),
            ));
        }
        if self.required_budget != self.plan_order.len() as u32 {
            return Err(FederatedContinualRetrievalSynthesisWorkflowError::Invalid(
                "federated workflow budget does not cover its plan".into(),
            ));
        }
        let blocked = !self.blocked_order.is_empty();
        if blocked && !self.completed_order.is_empty() {
            return Err(FederatedContinualRetrievalSynthesisWorkflowError::Invalid(
                "blocked federated workflow cannot report completed stages".into(),
            ));
        }
        if !blocked && self.completed_order != self.stage_order {
            return Err(FederatedContinualRetrievalSynthesisWorkflowError::Invalid(
                "unblocked federated workflow must complete every stage".into(),
            ));
        }
        for digest in [
            &self.queue_digest,
            &self.comparability_digest,
            &self.federation_digest,
            &self.synthesis_receipt_digest,
            &self.checkpoint_digest,
            &self.workflow_digest,
            &self.replay_identity,
            &self.artifact.content_hash,
        ] {
            if !digest_valid(digest) {
                return Err(FederatedContinualRetrievalSynthesisWorkflowError::Invalid(
                    "federated workflow digest is invalid".into(),
                ));
            }
        }
        let expected = if self.disposition == EvidenceSynthesisDisposition::Blocked {
            "block:unsafe-release".to_string()
        } else if !self.compensation_order.is_empty() {
            format!("compensate:research-work:{}", self.workflow_id)
        } else {
            format!("schedule:research-work:{}", self.workflow_id)
        };
        if self.effect_receipts != vec![expected] {
            return Err(FederatedContinualRetrievalSynthesisWorkflowError::Invalid(
                "federated workflow effect does not match disposition and compensation".into(),
            ));
        }
        self.artifact.validate_metadata().map_err(|error| {
            FederatedContinualRetrievalSynthesisWorkflowError::Artifact(error.to_string())
        })
    }
}

pub fn federated_continual_retrieval_synthesis_workflow_fabric_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "adapter".into(),
        consumers: ["preclinical researcher".into(), "federation operator".into()].into(),
        behavior: "runs a resumable A2 federated continual retrieval and synthesis workflow with purpose-bound peer quorum, aggregate-only envelopes, checkpoints, queue capacity, budget compensation, replay, and omission-preserving receipts".into(),
        value: "provides a governed consortium workflow that exchanges permitted aggregate research metadata without moving raw observations or making clinical decisions".into(),
        inputs: vec![TypedPort { name: "scoped_retrieval_query".into(), schema: INPUT_SCHEMA.into(), required: true }, TypedPort { name: "federation_workflow".into(), schema: "FederationEnvelope@1".into(), required: true }],
        outputs: vec![TypedPort { name: "evidence_synthesis".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(),
        permissions: ["execute:approved-workflows".into(), "exchange:permitted-aggregate-metadata".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "ga4gh-drs-1.3".into(), state: EvidenceState::Supported, locator: Some("https://ga4gh.github.io/data-repository-service-schemas/preview/release/drs-1.3.0/docs/".into()) }],
        authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A2,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn schedule_federated_continual_retrieval_synthesis_workflow(
    request: &FederatedContinualRetrievalSynthesisWorkflowRequest,
) -> Result<
    FederatedContinualRetrievalSynthesisWorkflowReceipt,
    FederatedContinualRetrievalSynthesisWorkflowError,
> {
    validate_request(request)?;
    let copilot = run_federated_continual_retrieval_synthesis_research_copilot(&request.request)
        .map_err(|error| {
            FederatedContinualRetrievalSynthesisWorkflowError::Copilot(error.to_string())
        })?;
    let stage_order = CANONICAL_STAGES
        .iter()
        .map(|s| (*s).to_string())
        .collect::<Vec<_>>();
    let plan_order = BTreeSet::from([
        "plan:stage:checkpoint".to_string(),
        "plan:stage:validate-federation".to_string(),
        "plan:stage:admit-peer-quorum".to_string(),
        "plan:stage:compile-synthesis".to_string(),
        "plan:stage:seal-aggregate-envelope".to_string(),
        "plan:stage:persist-artifact".to_string(),
        "plan:stage:validate-output".to_string(),
        "plan:retain-overflow".to_string(),
        "plan:retain-denied-federation".to_string(),
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
        blocked_order.insert("stage:policy-gate".into());
        omissions.insert("workflow:policy-denied".into());
        compensation.insert("compensate:research-work:policy-review".into());
    }
    if !request.protected_closure {
        blocked_order.insert("stage:protected-closure-gate".into());
        omissions.insert("workflow:protected-closure-incomplete".into());
        uncertainty.insert("workflow:protected-closure-unmeasured".into());
    }
    if !request.raw_data_local {
        blocked_order.insert("stage:locality-gate".into());
        omissions.insert("workflow:raw-data-locality-failed".into());
    }
    if request.budget_units < required_budget {
        blocked_order.insert("stage:budget-gate".into());
        omissions.insert("workflow:budget-exhausted".into());
        uncertainty.insert("workflow:budget-unmeasured".into());
        compensation.insert("compensate:research-work:budget-exhausted".into());
    }
    if request.request.peer_ids.len() < request.request.min_peer_quorum as usize {
        blocked_order.insert("stage:peer-quorum-gate".into());
        omissions.insert("federation:peer-quorum-incomplete".into());
        compensation.insert("compensate:research-work:retain-denied-federation".into());
    }
    if !request.request.aggregate_only {
        blocked_order.insert("stage:aggregate-only-gate".into());
        omissions.insert("federation:raw-observation-exchange-denied".into());
    }
    if copilot.disposition == EvidenceSynthesisDisposition::Blocked {
        blocked_order.insert("stage:approval-or-copilot-gate".into());
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
        compensation.insert("compensate:research-work:retain-overflow".into());
    }
    if !copilot.omitted_order.is_empty() || !copilot.uncertainty_order.is_empty() {
        compensation.insert("compensate:research-work:retain-unresolved-evidence".into());
    }
    let blocked_order = blocked_order.into_iter().collect::<Vec<_>>();
    let compensation_order = compensation.into_iter().collect::<Vec<_>>();
    let omissions = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let disposition = if blocked_order.is_empty() {
        copilot.disposition
    } else {
        EvidenceSynthesisDisposition::Blocked
    };
    let completed_order = if blocked_order.is_empty() {
        stage_order.clone()
    } else {
        Vec::new()
    };
    let checkpoint_digest = ContentHash::of_value(&json!({"workflow_id":request.workflow_id,"checkpoint_id":request.checkpoint_id,"checkpoint_seq":request.checkpoint_seq,"stage_order":stage_order,"replay_identity":request.replay_identity})).map_err(|error| FederatedContinualRetrievalSynthesisWorkflowError::Artifact(error.to_string()))?;
    let synthesis_receipt_digest =
        ContentHash::of_value(&serde_json::to_value(&copilot).map_err(|error| {
            FederatedContinualRetrievalSynthesisWorkflowError::Artifact(error.to_string())
        })?)
        .map_err(|error| {
            FederatedContinualRetrievalSynthesisWorkflowError::Artifact(error.to_string())
        })?;
    let workflow_digest = ContentHash::of_value(&json!({"workflow_id":request.workflow_id,"batch_id":request.request.batch_id,"federation_id":request.request.federation_id,"peer_ids":request.request.peer_ids,"plan_order":plan_order,"completed_order":completed_order,"blocked_order":blocked_order,"compensation_order":compensation_order,"checkpoint_digest":checkpoint_digest,"queue_digest":request.request.queue_digest,"federation_digest":request.request.federation_digest,"budget_units":request.budget_units,"required_budget":required_budget,"replay_identity":request.replay_identity})).map_err(|error| FederatedContinualRetrievalSynthesisWorkflowError::Artifact(error.to_string()))?;
    let effect_receipts = if disposition == EvidenceSynthesisDisposition::Blocked {
        vec!["block:unsafe-release".into()]
    } else if !compensation_order.is_empty() {
        vec![format!("compensate:research-work:{}", request.workflow_id)]
    } else {
        vec![format!("schedule:research-work:{}", request.workflow_id)]
    };
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request.synthesis_request.request_id,"workflow_id":request.workflow_id,"query_id":request.request.synthesis_request.query.query_id,"batch_id":request.request.batch_id,"checkpoint_id":request.checkpoint_id,"checkpoint_seq":request.checkpoint_seq,"capacity":request.request.capacity,"queue_digest":request.request.queue_digest,"comparability_digest":request.request.comparability_digest,"federation_id":request.request.federation_id,"purpose":request.request.purpose,"peer_ids":request.request.peer_ids,"min_peer_quorum":request.request.min_peer_quorum,"aggregate_only":request.request.aggregate_only,"endpoint":request.request.endpoint,"federation_digest":request.request.federation_digest,"disposition":disposition,"stage_order":stage_order,"plan_order":plan_order,"completed_order":completed_order,"blocked_order":blocked_order,"compensation_order":compensation_order,"candidate_order":copilot.candidate_order,"selected_order":copilot.selected_order,"omitted_order":copilot.omitted_order,"overflow_order":copilot.overflow_order,"uncertainty_order":copilot.uncertainty_order,"negative_order":copilot.negative_order,"contradictory_order":copilot.contradictory_order,"synthesis_receipt_digest":synthesis_receipt_digest,"checkpoint_digest":checkpoint_digest,"workflow_digest":workflow_digest,"replay_identity":request.replay_identity,"budget_units":request.budget_units,"required_budget":required_budget,"omissions":omissions,"uncertainty":uncertainty,"negative_evidence":copilot.negative_order,"effect_receipts":effect_receipts,"raw_data_local":true,"boundary":PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "adapter-federated-continual-retrieval-workflow:{}",
            request.workflow_id
        ),
        "application/vnd.aurora.federated-continual-retrieval-synthesis-workflow+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| {
        FederatedContinualRetrievalSynthesisWorkflowError::Artifact(error.to_string())
    })?;
    let receipt = FederatedContinualRetrievalSynthesisWorkflowReceipt {
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
        federation_id: request.request.federation_id.clone(),
        purpose: request.request.purpose.clone(),
        peer_ids: request.request.peer_ids.clone(),
        min_peer_quorum: request.request.min_peer_quorum,
        aggregate_only: request.request.aggregate_only,
        endpoint: request.request.endpoint.clone(),
        federation_digest: request.request.federation_digest.clone(),
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
    request: &FederatedContinualRetrievalSynthesisWorkflowRequest,
) -> Result<(), FederatedContinualRetrievalSynthesisWorkflowError> {
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
        || !request.request.aggregate_only
        || request.request.peer_ids.len() < request.request.min_peer_quorum as usize
    {
        return Err(FederatedContinualRetrievalSynthesisWorkflowError::Invalid("federated workflow identity, canonical stages, quorum, aggregate-only, checkpoint, budget, replay, locality, or boundary is invalid".into()));
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
    fn request() -> FederatedContinualRetrievalSynthesisWorkflowRequest {
        let replay = ContentHash::of_bytes(b"f16-replay");
        let comp = ContentHash::of_bytes(b"f16-comp");
        let queue = ContentHash::of_bytes(b"f16-queue");
        let checkpoint = ContentHash::of_bytes(b"f16-checkpoint");
        let federation = ContentHash::of_bytes(b"f16-federation");
        FederatedContinualRetrievalSynthesisWorkflowRequest {
            request: FederatedContinualRetrievalSynthesisResearchCopilotRequest {
                synthesis_request: EvidenceSynthesisRequest {
                    request_id: "request:f16".into(),
                    query: ScopedRetrievalQuery {
                        query_id: "query:f16".into(),
                        requester: "researcher:f16".into(),
                        intent: "federated continual preclinical retrieval".into(),
                        study_ids: vec!["study:f16".into()],
                        required_modalities: vec!["imaging".into(), "omics".into()],
                        comparability_profile: "profile:f16".into(),
                        max_results: 3,
                    },
                    candidates: (0..3)
                        .map(|i| RetrievalCandidate {
                            evidence_id: format!("evidence:f16-{i}"),
                            study_id: "study:f16".into(),
                            modality: if i % 2 == 0 { "imaging" } else { "omics" }.into(),
                            comparability_profile: "profile:f16".into(),
                            digest: Some(ContentHash::of_bytes(format!("f16-{i}").as_bytes())),
                            availability: EvidenceAvailability::Available,
                            relevance_score: 90u16 - i as u16,
                            negative_result: i == 2,
                            locator: format!("local://f16/{i}"),
                        })
                        .collect(),
                    policy_decision: PolicyDecision::Allow,
                    protected_closure_satisfied: true,
                    raw_data_local: true,
                    boundary: PRECLINICAL_BOUNDARY.into(),
                },
                copilot_id: "copilot:f16".into(),
                agent_id: "agent:f16".into(),
                recommendation_mode: "evidence-ranked-read-only".into(),
                approval_required: true,
                schema_profile: "ScopedRetrievalQuery@1".into(),
                canonicalization: "aurora-json-canonical-v1".into(),
                consumer: "preclinical researcher".into(),
                algorithm_version: "2026.1".into(),
                required_modalities: vec!["imaging".into(), "omics".into()],
                tool_id: "research-tool:f16".into(),
                approval_token: "approval:f16".into(),
                comparability_digest: comp,
                batch_id: "batch:f16".into(),
                checkpoint_seq: 4,
                capacity: 3,
                queue_digest: queue,
                checkpoint_digest: checkpoint,
                federation_id: "federation:f16".into(),
                purpose: "preclinical-research".into(),
                peer_ids: vec!["peer:a".into(), "peer:b".into(), "peer:c".into()],
                min_peer_quorum: 2,
                aggregate_only: true,
                endpoint: "https://local.invalid/aggregate".into(),
                federation_digest: federation,
                requested_output: "EvidenceSynthesis1@1".into(),
                budget_units: 14,
                replay_identity: replay.clone(),
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            workflow_id: "workflow:f16".into(),
            requested_stage_order: CANONICAL_STAGES.iter().map(|s| (*s).to_string()).collect(),
            checkpoint_id: "checkpoint:f16".into(),
            checkpoint_seq: 4,
            budget_units: 14,
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
            federated_continual_retrieval_synthesis_workflow_fabric_manifest().autonomy_tier,
            AutonomyTier::A2
        );
    }
    #[test]
    fn schedules_federated_workflow() {
        let r = schedule_federated_continual_retrieval_synthesis_workflow(&request()).unwrap();
        assert!(!r.peer_ids.is_empty());
        assert!(
            r.effect_receipts[0].starts_with("schedule:")
                || r.effect_receipts[0].starts_with("compensate:")
        );
    }
    #[test]
    fn aggregate_only_is_retained() {
        let r = schedule_federated_continual_retrieval_synthesis_workflow(&request()).unwrap();
        assert!(r.aggregate_only);
        assert!(r.federation_digest.as_str().len() == 64);
    }
    #[test]
    fn quorum_blocks() {
        let mut i = request();
        i.request.peer_ids.truncate(1);
        i.request.min_peer_quorum = 2;
        assert!(schedule_federated_continual_retrieval_synthesis_workflow(&i).is_err());
    }
    #[test]
    fn policy_blocks() {
        let mut i = request();
        i.policy_allow = false;
        let r = schedule_federated_continual_retrieval_synthesis_workflow(&i).unwrap();
        assert_eq!(r.disposition, EvidenceSynthesisDisposition::Blocked);
        assert_eq!(r.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn budget_blocks() {
        let mut i = request();
        i.budget_units = 1;
        let r = schedule_federated_continual_retrieval_synthesis_workflow(&i).unwrap();
        assert_eq!(r.disposition, EvidenceSynthesisDisposition::Blocked);
        assert!(r
            .compensation_order
            .iter()
            .any(|x| x.contains("budget-exhausted")));
    }
    #[test]
    fn replay_is_stable() {
        let i = request();
        let a = schedule_federated_continual_retrieval_synthesis_workflow(&i).unwrap();
        let b = schedule_federated_continual_retrieval_synthesis_workflow(&i).unwrap();
        assert_eq!(a.workflow_digest, b.workflow_digest);
        assert_eq!(a.artifact.content_hash, b.artifact.content_hash);
    }
}
