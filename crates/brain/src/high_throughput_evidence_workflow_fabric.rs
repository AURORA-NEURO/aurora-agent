//! Prospective high-throughput evidence-surveillance workflow fabric.
//!
//! Atlas feature: `AFA-brain-P01-F15`. The fabric schedules bounded batch admission with
//! queue/checkpoint identity and capacity compensation under A2 approval.

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

pub const FEATURE_ID: &str = "AFA-brain-P01-F15";
pub const CONTRACT_VERSION: &str = "brain-high-throughput-evidence-workflow-fabric/1.0";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet3@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HighThroughputWorkflowRequest {
    pub request: HighThroughputEvidenceFeedRequest,
    pub workflow_id: String,
    pub requested_stage_order: Vec<String>,
    pub checkpoint_id: String,
    pub approval_reference: ContentHash,
    pub budget_units: u32,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HighThroughputWorkflowReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub batch_id: String,
    pub partition: String,
    pub disposition: HighThroughputDisposition,
    pub stage_order: Vec<String>,
    pub plan_order: Vec<String>,
    pub completed_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub compensation_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub checkpoint_seq: u64,
    pub queue_digest: ContentHash,
    pub checkpoint_digest: ContentHash,
    pub workflow_digest: ContentHash,
    pub approval_reference: ContentHash,
    pub replay_identity: ContentHash,
    pub budget_units: u32,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum HighThroughputWorkflowError {
    #[error("invalid high-throughput workflow request: {0}")]
    Invalid(String),
    #[error("high-throughput workflow artifact failed: {0}")]
    Artifact(String),
    #[error("high-throughput workflow engine failed: {0}")]
    Engine(String),
}

impl HighThroughputWorkflowReceipt {
    pub fn validate(&self) -> Result<(), HighThroughputWorkflowError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.batch_id.trim().is_empty()
            || self.partition.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.stage_order.is_empty()
            || self.plan_order.is_empty()
            || self.completed_order.is_empty()
            || self.effect_receipts.is_empty()
            || self.budget_units == 0
        {
            return Err(HighThroughputWorkflowError::Invalid("throughput workflow identity, batch, stages, plan, locality, budget, or effects are incomplete".into()));
        }
        if self
            .admitted_order
            .iter()
            .chain(self.blocked_order.iter())
            .chain(self.unknown_order.iter())
            .any(|id| !self.candidate_order.contains(id))
        {
            return Err(HighThroughputWorkflowError::Invalid(
                "throughput workflow state is not covered by candidates".into(),
            ));
        }
        for values in [
            &self.stage_order,
            &self.plan_order,
            &self.completed_order,
            &self.blocked_order,
            &self.compensation_order,
            &self.candidate_order,
            &self.admitted_order,
            &self.unknown_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(HighThroughputWorkflowError::Invalid(
                    "throughput workflow ordering is not canonical".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("schedule:research-work:")
                && !effect.starts_with("compensate:research-work:")
                && effect != "block:unsafe-release"
        }) {
            return Err(HighThroughputWorkflowError::Invalid(
                "throughput workflow effect is outside schedule/compensation gate".into(),
            ));
        }
        if self.disposition == HighThroughputDisposition::Qualified
            && !self
                .effect_receipts
                .iter()
                .any(|effect| effect.starts_with("schedule:research-work:"))
        {
            return Err(HighThroughputWorkflowError::Invalid(
                "qualified throughput workflow requires schedule receipt".into(),
            ));
        }
        if self.disposition == HighThroughputDisposition::Blocked
            && self.effect_receipts != ["block:unsafe-release"]
        {
            return Err(HighThroughputWorkflowError::Invalid(
                "blocked throughput workflow must be explicitly blocked".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| HighThroughputWorkflowError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, HighThroughputWorkflowError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| HighThroughputWorkflowError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| HighThroughputWorkflowError::Artifact(error.to_string()))
    }
}

pub fn high_throughput_evidence_workflow_fabric_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["platform reliability engineer".into(), "batch workflow steward".into()].into(), behavior: "schedules bounded EvidenceFeed3 batch admission with queue/checkpoint identity, capacity compensation, and replay receipts".into(), value: "orchestrates prospective evidence streams without silent capacity loss or unreviewed batch effects".into(), inputs: vec![TypedPort { name: "throughput_workflow_request".into(), schema: "ResearchWorkflowSpec3@1".into(), required: true }], outputs: vec![TypedPort { name: "qualified_evidence_set".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["schedule:research-work".into(), "read:local-research-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "cwl-v1.2".into(), state: EvidenceState::Supported, locator: Some("https://www.commonwl.org/specification/".into()) }], authority_requirements: vec![AuthorityRequirement { role: "batch workflow approver".into(), reason: "approve capacity and retry policy before scheduling prospective work".into() }], autonomy_tier: AutonomyTier::A2, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn compile_high_throughput_evidence_workflow(
    request: &HighThroughputWorkflowRequest,
) -> Result<HighThroughputWorkflowReceipt, HighThroughputWorkflowError> {
    validate_request(request)?;
    let evidence = admit_high_throughput_evidence(&request.request)
        .map_err(|error| HighThroughputWorkflowError::Engine(error.to_string()))?;
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
    let stage_order: Vec<String> = vec![
        "stage:admit-batch".into(),
        "stage:checkpoint".into(),
        "stage:persist-queue".into(),
        "stage:validate-input".into(),
    ];
    let mut plan_order = BTreeSet::new();
    let mut completed_order = BTreeSet::new();
    let mut compensation_order = BTreeSet::new();
    for stage in &stage_order {
        plan_order.insert(format!("plan:{stage}"));
        completed_order.insert(stage.clone());
    }
    let actionable = request.policy_allow
        && request.protected_closure
        && request.raw_data_local
        && request.approval_reference != ContentHash::of_bytes(&[])
        && request.budget_units >= plan_order.len() as u32
        && evidence.disposition != HighThroughputDisposition::Blocked;
    if evidence.disposition == HighThroughputDisposition::Partial
        && request.policy_allow
        && request.protected_closure
        && request.raw_data_local
    {
        compensation_order.insert("compensate:research-work:capacity-overflow".into());
        omissions.insert("workflow:capacity-overflow-requires-compensation".into());
    }
    if !actionable {
        omissions.insert("workflow:batch-not-schedulable".into());
    }
    if request.budget_units < plan_order.len() as u32 {
        omissions.insert("workflow:budget-exhausted".into());
    }
    if request.approval_reference == ContentHash::of_bytes(&[]) {
        omissions.insert("workflow:approval-missing".into());
    }
    if !request.policy_allow {
        omissions.insert("workflow:policy-denied".into());
    }
    if !request.protected_closure {
        omissions.insert("workflow:protected-closure-incomplete".into());
    }
    if !request.raw_data_local {
        omissions.insert("workflow:raw-data-locality-failed".into());
    }
    plan_order.insert(if evidence.admitted_order.is_empty() {
        "plan:retain-unknown-batch".into()
    } else {
        "plan:publish-admitted-batch".into()
    });
    let disposition = if !actionable {
        HighThroughputDisposition::Blocked
    } else {
        evidence.disposition
    };
    let plan_vec = plan_order.into_iter().collect::<Vec<_>>();
    let completed_vec = completed_order.into_iter().collect::<Vec<_>>();
    let checkpoint_digest = ContentHash::of_value(&json!({"workflow_id": request.workflow_id, "checkpoint_id": request.checkpoint_id, "checkpoint_seq": evidence.checkpoint_seq, "queue_digest": evidence.queue_digest, "stage_order": stage_order, "replay_identity": request.replay_identity})).map_err(|error| HighThroughputWorkflowError::Artifact(error.to_string()))?;
    let workflow_digest = ContentHash::of_value(&json!({"workflow_id": request.workflow_id, "plan_order": plan_vec, "completed_order": completed_vec, "checkpoint_digest": checkpoint_digest, "approval_reference": request.approval_reference, "budget_units": request.budget_units, "replay_identity": request.replay_identity})).map_err(|error| HighThroughputWorkflowError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request.request_id, "workflow_id": request.workflow_id, "batch_id": request.request.batch_id, "partition": request.request.partition, "disposition": disposition, "stage_order": stage_order, "plan_order": plan_vec, "completed_order": completed_vec, "compensation_order": compensation_order, "candidate_order": evidence.candidate_order, "admitted_order": evidence.admitted_order, "blocked_order": evidence.blocked_order, "unknown_order": evidence.unknown_order, "checkpoint_seq": evidence.checkpoint_seq, "queue_digest": evidence.queue_digest, "checkpoint_digest": checkpoint_digest, "workflow_digest": workflow_digest, "approval_reference": request.approval_reference, "replay_identity": request.replay_identity, "budget_units": request.budget_units, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "brain-high-throughput-evidence-workflow:{}",
            request.workflow_id
        ),
        "application/vnd.aurora.high-throughput-research-workflow-receipt+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| HighThroughputWorkflowError::Artifact(error.to_string()))?;
    let has_compensation = !compensation_order.is_empty();
    let receipt = HighThroughputWorkflowReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        batch_id: request.request.batch_id.clone(),
        partition: request.request.partition.clone(),
        disposition,
        stage_order: payload
            .get("stage_order")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default(),
        plan_order: payload
            .get("plan_order")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default(),
        completed_order: payload
            .get("completed_order")
            .and_then(|v| serde_json::from_value(v.clone()).ok())
            .unwrap_or_default(),
        blocked_order: evidence.blocked_order.clone(),
        compensation_order: compensation_order.into_iter().collect(),
        candidate_order: evidence.candidate_order.clone(),
        admitted_order: evidence.admitted_order.clone(),
        unknown_order: evidence.unknown_order.clone(),
        checkpoint_seq: evidence.checkpoint_seq,
        queue_digest: evidence.queue_digest.clone(),
        checkpoint_digest,
        workflow_digest,
        approval_reference: request.approval_reference.clone(),
        replay_identity: request.replay_identity.clone(),
        budget_units: request.budget_units,
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts: if disposition == HighThroughputDisposition::Qualified {
            vec![format!("schedule:research-work:{}", request.workflow_id)]
        } else if has_compensation {
            vec![format!("compensate:research-work:{}", request.workflow_id)]
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
    request: &HighThroughputWorkflowRequest,
) -> Result<(), HighThroughputWorkflowError> {
    if request.workflow_id.trim().is_empty()
        || request.checkpoint_id.trim().is_empty()
        || request.requested_stage_order.len() != 4
        || request
            .requested_stage_order
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || request.budget_units == 0
        || request.approval_reference == ContentHash::of_bytes(&[])
        || request.request.replay_identity != request.replay_identity
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(HighThroughputWorkflowError::Invalid("throughput workflow identity, canonical stages, approval, budget, replay, or boundary is incomplete".into()));
    }
    if request.requested_stage_order
        != vec![
            "stage:admit-batch",
            "stage:checkpoint",
            "stage:persist-queue",
            "stage:validate-input",
        ]
    {
        return Err(HighThroughputWorkflowError::Invalid(
            "throughput workflow stages do not match the versioned fabric protocol".into(),
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
    fn request(state: EvidenceState) -> HighThroughputWorkflowRequest {
        HighThroughputWorkflowRequest {
            request: HighThroughputEvidenceFeedRequest {
                request_id: "request:throughput-workflow".into(),
                batch_id: "batch:001".into(),
                partition: "partition:imaging".into(),
                max_items: 1,
                observations: vec![observation("a", state)],
                replay_identity: hash("replay"),
                policy_allow: true,
                protected_closure: true,
                raw_data_local: true,
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            workflow_id: "workflow:throughput".into(),
            requested_stage_order: vec![
                "stage:admit-batch".into(),
                "stage:checkpoint".into(),
                "stage:persist-queue".into(),
                "stage:validate-input".into(),
            ],
            checkpoint_id: "checkpoint:1".into(),
            approval_reference: hash("signed-approval"),
            budget_units: 8,
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a2_and_queue_scoped() {
        let manifest = high_throughput_evidence_workflow_fabric_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A2);
    }
    #[test]
    fn supported_batch_is_scheduled() {
        let receipt =
            compile_high_throughput_evidence_workflow(&request(EvidenceState::Supported)).unwrap();
        assert!(receipt.effect_receipts[0].starts_with("schedule:research-work:"));
    }
    #[test]
    fn capacity_overflow_compensates() {
        let mut input = request(EvidenceState::Supported);
        input
            .request
            .observations
            .push(observation("b", EvidenceState::Supported));
        let receipt = compile_high_throughput_evidence_workflow(&input).unwrap();
        assert!(!receipt.compensation_order.is_empty());
    }
    #[test]
    fn policy_denial_blocks() {
        let mut input = request(EvidenceState::Supported);
        input.policy_allow = false;
        let receipt = compile_high_throughput_evidence_workflow(&input).unwrap();
        assert_eq!(receipt.disposition, HighThroughputDisposition::Blocked);
    }
    #[test]
    fn stage_protocol_is_required() {
        let mut input = request(EvidenceState::Supported);
        input.requested_stage_order.reverse();
        assert!(compile_high_throughput_evidence_workflow(&input).is_err());
    }
    #[test]
    fn approval_is_required() {
        let mut input = request(EvidenceState::Supported);
        input.approval_reference = ContentHash::of_bytes(&[]);
        assert!(compile_high_throughput_evidence_workflow(&input).is_err());
    }
}
