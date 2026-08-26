//! Multimodal multi-study evidence-surveillance workflow fabric.
//!
//! Atlas feature `AFA-adapter-P01-F14`: an A2 workflow protocol around the
//! EvidenceFeed2 copilot.  It makes comparability, approval, checkpoints,
//! budget admission, compensation, provenance, and replay first-class product
//! receipts for an operator rather than hidden runner state.

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

use crate::multimodal_evidence_surveillance_research_copilot::{
    run_multimodal_evidence_surveillance_research_copilot,
    MultimodalEvidenceSurveillanceResearchCopilotRequest, MultimodalResearchCopilotDisposition,
};

pub const FEATURE_ID: &str = "AFA-adapter-P01-F14";
pub const CONTRACT_VERSION: &str = "adapter-multimodal-evidence-surveillance-workflow-fabric/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceFeed2@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet4@1";
const CANONICAL_STAGES: [&str; 4] = [
    "stage:checkpoint",
    "stage:persist-artifact",
    "stage:surveil-evidence",
    "stage:validate-comparability",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalEvidenceSurveillanceWorkflowRequest {
    pub request: MultimodalEvidenceSurveillanceResearchCopilotRequest,
    pub workflow_id: String,
    pub requested_stage_order: Vec<String>,
    pub checkpoint_id: String,
    pub budget_units: u32,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalEvidenceSurveillanceWorkflowReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub agent_id: String,
    pub semantic_profile: String,
    pub disposition: MultimodalResearchCopilotDisposition,
    pub stage_order: Vec<String>,
    pub plan_order: Vec<String>,
    pub completed_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub compensation_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub incomparable_order: Vec<String>,
    pub missing_cell_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub copilot_run_digest: ContentHash,
    pub checkpoint_digest: ContentHash,
    pub workflow_digest: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum MultimodalEvidenceSurveillanceWorkflowError {
    #[error("invalid multimodal evidence workflow request: {0}")]
    Invalid(String),
    #[error("multimodal evidence workflow artifact failed: {0}")]
    Artifact(String),
    #[error("multimodal evidence workflow copilot failed: {0}")]
    Copilot(String),
}

impl MultimodalEvidenceSurveillanceWorkflowReceipt {
    pub fn validate(&self) -> Result<(), MultimodalEvidenceSurveillanceWorkflowError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.agent_id.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.stage_order.is_empty()
            || self.plan_order.is_empty()
            || self.completed_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(MultimodalEvidenceSurveillanceWorkflowError::Invalid(
                "workflow identity, stages, comparability, locality, or effects are incomplete"
                    .into(),
            ));
        }
        for values in [
            &self.stage_order,
            &self.plan_order,
            &self.completed_order,
            &self.blocked_order,
            &self.compensation_order,
            &self.candidate_order,
            &self.selected_order,
            &self.unresolved_order,
            &self.incomparable_order,
            &self.missing_cell_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(MultimodalEvidenceSurveillanceWorkflowError::Invalid(
                    "workflow ordering is not canonical".into(),
                ));
            }
        }
        for value in [
            &self.replay_identity,
            &self.copilot_run_digest,
            &self.checkpoint_digest,
            &self.workflow_digest,
            &self.artifact.content_hash,
        ] {
            if value.as_str().len() != 64 {
                return Err(MultimodalEvidenceSurveillanceWorkflowError::Invalid(
                    "workflow digest is invalid".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("schedule:research-work:")
                && !effect.starts_with("compensate:research-work:")
                && effect != "block:unsafe-release"
        }) {
            return Err(MultimodalEvidenceSurveillanceWorkflowError::Invalid(
                "workflow effect is outside schedule/compensation gate".into(),
            ));
        }
        self.artifact.validate_metadata().map_err(|error| {
            MultimodalEvidenceSurveillanceWorkflowError::Artifact(error.to_string())
        })
    }
}

pub fn multimodal_evidence_surveillance_workflow_fabric_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "adapter".into(),
        consumers: ["consortium administrator".into(), "workflow operator".into()].into(),
        behavior: "orchestrates an A2 multimodal EvidenceFeed2 workflow with comparability closure, signed approval, checkpoints, budget admission, compensation, and replay receipts".into(),
        value: "gives multi-study imaging and omics surveillance an independently observable, fail-closed operator workflow without hiding missing cells or unauthorized effects".into(),
        inputs: vec![TypedPort { name: "multimodal_evidence_workflow_request".into(), schema: INPUT_SCHEMA.into(), required: true }],
        outputs: vec![TypedPort { name: "qualified_multimodal_workflow_evidence_set".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(),
        permissions: ["schedule:research-work".into(), "execute:approved-workflows".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "cwl".into(), state: EvidenceState::Supported, locator: Some("https://www.commonwl.org/specification/".into()) }],
        authority_requirements: vec![AuthorityRequirement { role: "multimodal evidence workflow approver".into(), reason: "approve declared tool effects only after comparability, protected closure, locality, and replay gates close".into() }],
        autonomy_tier: AutonomyTier::A2,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn schedule_multimodal_evidence_surveillance_workflow(
    request: &MultimodalEvidenceSurveillanceWorkflowRequest,
) -> Result<
    MultimodalEvidenceSurveillanceWorkflowReceipt,
    MultimodalEvidenceSurveillanceWorkflowError,
> {
    validate_request(request)?;
    let copilot = run_multimodal_evidence_surveillance_research_copilot(&request.request)
        .map_err(|error| MultimodalEvidenceSurveillanceWorkflowError::Copilot(error.to_string()))?;
    let stage_order = CANONICAL_STAGES
        .iter()
        .map(|stage| (*stage).to_string())
        .collect::<Vec<_>>();
    let mut plan = BTreeSet::new();
    let mut completed = BTreeSet::new();
    let mut compensation = BTreeSet::new();
    for stage in &stage_order {
        plan.insert(format!("plan:{stage}"));
        completed.insert(stage.clone());
    }
    if copilot.selected_order.is_empty() {
        plan.insert("plan:retain-unresolved-multimodal-evidence".into());
        compensation.insert("compensate:research-work:retain-unresolved-evidence".into());
    } else {
        plan.insert("plan:publish-qualified-multimodal-artifact".into());
    }
    let required_budget = plan.len() as u32;
    if request.budget_units < required_budget {
        compensation.insert("compensate:research-work:budget-exhausted".into());
    }
    let mut omissions = copilot.omissions.clone();
    if !request.request.policy_allow {
        omissions.push("workflow:policy-denied".into());
    }
    if !request.request.protected_closure {
        omissions.push("workflow:protected-closure-incomplete".into());
    }
    if !request.request.approval_granted && !request.request.dry_run {
        omissions.push("workflow:approval-required".into());
    }
    omissions.sort();
    omissions.dedup();
    let blocked_gate = copilot.disposition == MultimodalResearchCopilotDisposition::Blocked;
    let disposition = if blocked_gate {
        MultimodalResearchCopilotDisposition::Blocked
    } else {
        copilot.disposition
    };
    let blocked_order = if disposition == MultimodalResearchCopilotDisposition::Blocked {
        vec!["stage:release".into()]
    } else {
        Vec::new()
    };
    let plan_order = plan.into_iter().collect::<Vec<_>>();
    let completed_order = completed.into_iter().collect::<Vec<_>>();
    let compensation_order = compensation.into_iter().collect::<Vec<_>>();
    let checkpoint_digest = ContentHash::of_value(&json!({"workflow_id":request.workflow_id,"checkpoint_id":request.checkpoint_id,"stage_order":stage_order,"replay_identity":request.replay_identity})).map_err(|error| MultimodalEvidenceSurveillanceWorkflowError::Artifact(error.to_string()))?;
    let copilot_value = serde_json::to_value(&copilot).map_err(|error| {
        MultimodalEvidenceSurveillanceWorkflowError::Artifact(error.to_string())
    })?;
    let copilot_run_digest = ContentHash::of_value(&copilot_value).map_err(|error| {
        MultimodalEvidenceSurveillanceWorkflowError::Artifact(error.to_string())
    })?;
    let workflow_digest = ContentHash::of_value(&json!({"workflow_id":request.workflow_id,"plan_order":plan_order,"completed_order":completed_order,"compensation_order":compensation_order,"checkpoint_digest":checkpoint_digest,"copilot_run_digest":copilot_run_digest,"budget_units":request.budget_units})).map_err(|error| MultimodalEvidenceSurveillanceWorkflowError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request.request_id,"workflow_id":request.workflow_id,"agent_id":request.request.agent_id,"semantic_profile":request.request.semantic_profile,"disposition":disposition,"stage_order":stage_order,"plan_order":plan_order,"completed_order":completed_order,"blocked_order":blocked_order,"compensation_order":compensation_order,"candidate_order":copilot.candidate_order,"selected_order":copilot.selected_order,"unresolved_order":copilot.unresolved_order,"incomparable_order":copilot.incomparable_order,"missing_cell_order":copilot.missing_cell_order,"replay_identity":request.replay_identity,"copilot_run_digest":copilot_run_digest,"checkpoint_digest":checkpoint_digest,"workflow_digest":workflow_digest,"omissions":omissions,"uncertainty":copilot.uncertainty,"negative_evidence":copilot.negative_evidence,"boundary":PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "adapter-multimodal-evidence-workflow:{}",
            request.workflow_id
        ),
        "application/vnd.aurora.multimodal-research-workflow+json",
        &payload,
        vec![],
        vec![],
    )
    .map_err(|error| MultimodalEvidenceSurveillanceWorkflowError::Artifact(error.to_string()))?;
    let effect_receipts = if disposition == MultimodalResearchCopilotDisposition::Blocked {
        vec!["block:unsafe-release".into()]
    } else if !compensation_order.is_empty() {
        vec![format!("compensate:research-work:{}", request.workflow_id)]
    } else {
        vec![format!("schedule:research-work:{}", request.workflow_id)]
    };
    let receipt = MultimodalEvidenceSurveillanceWorkflowReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        agent_id: request.request.agent_id.clone(),
        semantic_profile: request.request.semantic_profile.clone(),
        disposition,
        stage_order,
        plan_order,
        completed_order,
        blocked_order,
        compensation_order,
        candidate_order: copilot.candidate_order.clone(),
        selected_order: copilot.selected_order.clone(),
        unresolved_order: copilot.unresolved_order.clone(),
        incomparable_order: copilot.incomparable_order.clone(),
        missing_cell_order: copilot.missing_cell_order.clone(),
        replay_identity: request.replay_identity.clone(),
        copilot_run_digest,
        checkpoint_digest,
        workflow_digest,
        omissions,
        uncertainty: copilot.uncertainty.clone(),
        negative_evidence: copilot.negative_evidence.clone(),
        effect_receipts,
        artifact,
        raw_data_local: request.request.raw_data_local,
        boundary: request.boundary.clone(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(
    request: &MultimodalEvidenceSurveillanceWorkflowRequest,
) -> Result<(), MultimodalEvidenceSurveillanceWorkflowError> {
    if request.workflow_id.trim().is_empty()
        || request.checkpoint_id.trim().is_empty()
        || request.budget_units == 0
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.request.boundary != PRECLINICAL_BOUNDARY
        || !request.request.raw_data_local
    {
        return Err(MultimodalEvidenceSurveillanceWorkflowError::Invalid(
            "workflow identity, checkpoint, budget, locality, or preclinical boundary is invalid"
                .into(),
        ));
    }
    let expected = CANONICAL_STAGES
        .iter()
        .map(|stage| (*stage).to_string())
        .collect::<Vec<_>>();
    if request.requested_stage_order != expected {
        return Err(MultimodalEvidenceSurveillanceWorkflowError::Invalid(
            "workflow stage order is not canonical".into(),
        ));
    }
    if request.request.replay_identity.as_str().len() != 64
        || request.replay_identity.as_str().len() != 64
    {
        return Err(MultimodalEvidenceSurveillanceWorkflowError::Invalid(
            "replay identity is invalid".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::multimodal_evidence_surveillance_research_copilot::MultimodalCopilotEvidenceObservation;
    use bioprism_foundation::EvidenceAvailability;

    fn request() -> MultimodalEvidenceSurveillanceWorkflowRequest {
        MultimodalEvidenceSurveillanceWorkflowRequest {
            request: MultimodalEvidenceSurveillanceResearchCopilotRequest {
                request_id: "req-14".into(),
                agent_id: "agent-14".into(),
                semantic_profile: "profile-1".into(),
                required_studies: vec!["study-a".into(), "study-b".into()],
                required_modalities: vec!["imaging".into(), "omics".into()],
                declared_tools: vec!["evidence.query".into()],
                requested_tool: "evidence.query".into(),
                max_tool_calls: 2,
                dry_run: true,
                approval_reference: None,
                approval_granted: false,
                observations: vec![MultimodalCopilotEvidenceObservation {
                    source_id: "s1".into(),
                    study_id: "study-a".into(),
                    modality: "imaging".into(),
                    semantic_profile: "profile-1".into(),
                    source_type: "paper".into(),
                    locator: "local://s1".into(),
                    digest: Some(ContentHash::of_bytes(b"s1")),
                    availability: EvidenceAvailability::Available,
                    evidence_state: EvidenceState::Supported,
                    relevance_score: 95,
                    negative_result: false,
                }],
                min_relevance_score: 50,
                policy_allow: true,
                protected_closure: true,
                raw_data_local: true,
                replay_identity: ContentHash::of_bytes(b"copilot"),
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            workflow_id: "workflow-14".into(),
            requested_stage_order: CANONICAL_STAGES
                .iter()
                .map(|stage| (*stage).to_string())
                .collect(),
            checkpoint_id: "checkpoint-14".into(),
            budget_units: 8,
            replay_identity: ContentHash::of_bytes(b"workflow"),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a2() {
        assert_eq!(
            multimodal_evidence_surveillance_workflow_fabric_manifest().autonomy_tier,
            AutonomyTier::A2
        );
        assert!(multimodal_evidence_surveillance_workflow_fabric_manifest()
            .validate()
            .is_ok());
    }
    #[test]
    fn schedules_with_comparability_receipt() {
        let receipt = schedule_multimodal_evidence_surveillance_workflow(&request()).unwrap();
        assert_eq!(receipt.feature_id, FEATURE_ID);
        assert!(!receipt.stage_order.is_empty());
    }
    #[test]
    fn approval_blocks_external_effect() {
        let mut r = request();
        r.request.dry_run = false;
        r.request.approval_granted = false;
        let receipt = schedule_multimodal_evidence_surveillance_workflow(&r).unwrap();
        assert_eq!(
            receipt.disposition,
            MultimodalResearchCopilotDisposition::Blocked
        );
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn budget_compensates() {
        let mut r = request();
        r.budget_units = 1;
        let receipt = schedule_multimodal_evidence_surveillance_workflow(&r).unwrap();
        assert_eq!(receipt.effect_receipts.len(), 1);
        assert!(receipt.effect_receipts[0].starts_with("compensate:"));
    }
    #[test]
    fn stage_order_is_rejected() {
        let mut r = request();
        r.requested_stage_order.reverse();
        assert!(schedule_multimodal_evidence_surveillance_workflow(&r).is_err());
    }
    #[test]
    fn replay_is_stable() {
        let r = request();
        assert_eq!(
            schedule_multimodal_evidence_surveillance_workflow(&r).unwrap(),
            schedule_multimodal_evidence_surveillance_workflow(&r).unwrap()
        );
    }
}
