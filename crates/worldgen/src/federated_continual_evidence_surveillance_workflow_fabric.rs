//! Federated continual evidence-surveillance workflow fabric.
//!
//! Atlas feature `AFA-worldgen-P01-F16`.  This workflow wraps the federated
//! continual copilot with an explicit checkpointed orchestration boundary.
//! Federation is aggregate-only: raw observations never enter the envelope,
//! and quorum, purpose, signer, locality, replay, and approval failures stay
//! observable instead of being silently downgraded.

use std::collections::BTreeSet;

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

use crate::federated_continual_evidence_surveillance_research_copilot::{
    run_federated_continual_evidence_surveillance_research_copilot,
    FederatedContinualEvidenceSurveillanceResearchCopilotRequest,
    FederatedContinualResearchCopilotDisposition,
};

pub const FEATURE_ID: &str = "AFA-worldgen-P01-F16";
pub const CONTRACT_VERSION: &str =
    "worldgen-federated-continual-evidence-surveillance-workflow-fabric/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceFeed4@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet4@1";
const CANONICAL_STAGES: [&str; 4] = [
    "stage:checkpoint",
    "stage:admit-federation",
    "stage:surveil-evidence",
    "stage:seal-envelope",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContinualEvidenceSurveillanceWorkflowRequest {
    pub request: FederatedContinualEvidenceSurveillanceResearchCopilotRequest,
    pub workflow_id: String,
    pub requested_stage_order: Vec<String>,
    pub checkpoint_id: String,
    pub budget_units: u32,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContinualEvidenceSurveillanceWorkflowReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub agent_id: String,
    pub federation_id: String,
    pub purpose: String,
    pub endpoint: String,
    pub disposition: FederatedContinualResearchCopilotDisposition,
    pub stage_order: Vec<String>,
    pub plan_order: Vec<String>,
    pub completed_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub compensation_order: Vec<String>,
    pub peer_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub denied_order: Vec<String>,
    pub aggregate_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub copilot_run_digest: ContentHash,
    pub checkpoint_digest: ContentHash,
    pub federation_envelope_digest: ContentHash,
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
pub enum FederatedContinualEvidenceSurveillanceWorkflowError {
    #[error("invalid federated continual evidence workflow request: {0}")]
    Invalid(String),
    #[error("federated continual evidence workflow artifact failed: {0}")]
    Artifact(String),
    #[error("federated continual evidence workflow copilot failed: {0}")]
    Copilot(String),
}

impl FederatedContinualEvidenceSurveillanceWorkflowReceipt {
    pub fn validate(&self) -> Result<(), FederatedContinualEvidenceSurveillanceWorkflowError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.agent_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.endpoint.trim().is_empty()
            || self.stage_order.is_empty()
            || self.plan_order.is_empty()
            || self.completed_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(
                FederatedContinualEvidenceSurveillanceWorkflowError::Invalid(
                    "federation identity, workflow stages, locality, or effects are incomplete"
                        .into(),
                ),
            );
        }
        if self.stage_order
            != CANONICAL_STAGES
                .iter()
                .map(|stage| (*stage).to_string())
                .collect::<Vec<_>>()
        {
            return Err(
                FederatedContinualEvidenceSurveillanceWorkflowError::Invalid(
                    "workflow stage order is not canonical".into(),
                ),
            );
        }
        for values in [
            &self.plan_order,
            &self.completed_order,
            &self.blocked_order,
            &self.compensation_order,
            &self.peer_order,
            &self.candidate_order,
            &self.selected_order,
            &self.unresolved_order,
            &self.denied_order,
            &self.aggregate_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(
                    FederatedContinualEvidenceSurveillanceWorkflowError::Invalid(
                        "federated workflow ordering is not canonical".into(),
                    ),
                );
            }
        }
        for value in [
            &self.replay_identity,
            &self.copilot_run_digest,
            &self.checkpoint_digest,
            &self.federation_envelope_digest,
            &self.workflow_digest,
            &self.artifact.content_hash,
        ] {
            if value.as_str().len() != 64 {
                return Err(
                    FederatedContinualEvidenceSurveillanceWorkflowError::Invalid(
                        "federated workflow digest is invalid".into(),
                    ),
                );
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("schedule:research-work:")
                && !effect.starts_with("compensate:research-work:")
                && effect != "block:unsafe-release"
        }) {
            return Err(
                FederatedContinualEvidenceSurveillanceWorkflowError::Invalid(
                    "federated workflow effect is outside schedule/compensation gate".into(),
                ),
            );
        }
        self.artifact.validate_metadata().map_err(|error| {
            FederatedContinualEvidenceSurveillanceWorkflowError::Artifact(error.to_string())
        })
    }
}

pub fn federated_continual_evidence_surveillance_workflow_fabric_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "worldgen".into(),
        consumers: ["AURORA extension developer".into(), "federation operator".into()].into(),
        behavior: "orchestrates a federated continual EvidenceFeed4 workflow with purpose-bound aggregate-only admission, peer quorum, checkpointing, signed approval, compensation, and replay receipts".into(),
        value: "makes continual multi-institution evidence surveillance independently deployable while preserving raw-data locality, omission evidence, and an explicit boundary between local observations and permitted federation envelopes".into(),
        inputs: vec![TypedPort { name: "federated_continual_evidence_workflow_request".into(), schema: INPUT_SCHEMA.into(), required: true }],
        outputs: vec![TypedPort { name: "qualified_federated_evidence_workflow_set".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(),
        permissions: ["schedule:research-work".into(), "execute:approved-workflows".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "cwl".into(), state: EvidenceState::Supported, locator: Some("https://www.commonwl.org/specification/".into()) }],
        authority_requirements: vec![AuthorityRequirement { role: "federated continual workflow approver".into(), reason: "approve declared aggregate-only exchange effects only after purpose, quorum, policy, protected closure, locality, replay, and signed-approval gates close".into() }],
        autonomy_tier: AutonomyTier::A2,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn schedule_federated_continual_evidence_surveillance_workflow(
    request: &FederatedContinualEvidenceSurveillanceWorkflowRequest,
) -> Result<
    FederatedContinualEvidenceSurveillanceWorkflowReceipt,
    FederatedContinualEvidenceSurveillanceWorkflowError,
> {
    validate_request(request)?;
    let copilot = run_federated_continual_evidence_surveillance_research_copilot(&request.request)
        .map_err(|error| {
            FederatedContinualEvidenceSurveillanceWorkflowError::Copilot(error.to_string())
        })?;
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
    if copilot.aggregate_order.is_empty() {
        plan.insert("plan:retain-unresolved-federated-evidence".into());
        compensation.insert("compensate:research-work:retain-unresolved-evidence".into());
    } else {
        plan.insert("plan:seal-permitted-aggregate-envelope".into());
    }
    if request.budget_units < plan.len() as u32 {
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
    let disposition = copilot.disposition;
    let blocked_order = if disposition == FederatedContinualResearchCopilotDisposition::Blocked {
        vec!["stage:release".into()]
    } else {
        Vec::new()
    };
    let plan_order = plan.into_iter().collect::<Vec<_>>();
    let completed_order = completed.into_iter().collect::<Vec<_>>();
    let compensation_order = compensation.into_iter().collect::<Vec<_>>();
    let checkpoint_digest = ContentHash::of_value(&json!({"workflow_id":request.workflow_id,"checkpoint_id":request.checkpoint_id,"stage_order":stage_order,"replay_identity":request.replay_identity})).map_err(|error| FederatedContinualEvidenceSurveillanceWorkflowError::Artifact(error.to_string()))?;
    let copilot_run_digest =
        ContentHash::of_value(&serde_json::to_value(&copilot).map_err(|error| {
            FederatedContinualEvidenceSurveillanceWorkflowError::Artifact(error.to_string())
        })?)
        .map_err(|error| {
            FederatedContinualEvidenceSurveillanceWorkflowError::Artifact(error.to_string())
        })?;
    let federation_envelope_digest = ContentHash::of_value(&json!({"federation_id":request.request.federation_id,"purpose":request.request.purpose,"endpoint":request.request.endpoint,"peer_order":copilot.peer_order,"aggregate_order":copilot.aggregate_order,"raw_data_local":request.request.raw_data_local,"allowed_artifacts":request.request.allowed_artifacts})).map_err(|error| FederatedContinualEvidenceSurveillanceWorkflowError::Artifact(error.to_string()))?;
    let workflow_digest = ContentHash::of_value(&json!({"workflow_id":request.workflow_id,"plan_order":plan_order,"completed_order":completed_order,"compensation_order":compensation_order,"checkpoint_digest":checkpoint_digest,"federation_envelope_digest":federation_envelope_digest,"copilot_run_digest":copilot_run_digest,"budget_units":request.budget_units})).map_err(|error| FederatedContinualEvidenceSurveillanceWorkflowError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request.request_id,"workflow_id":request.workflow_id,"agent_id":request.request.agent_id,"federation_id":request.request.federation_id,"purpose":request.request.purpose,"endpoint":request.request.endpoint,"disposition":disposition,"stage_order":stage_order,"plan_order":plan_order,"completed_order":completed_order,"blocked_order":blocked_order,"compensation_order":compensation_order,"peer_order":copilot.peer_order,"candidate_order":copilot.candidate_order,"selected_order":copilot.selected_order,"unresolved_order":copilot.unresolved_order,"denied_order":copilot.denied_order,"aggregate_order":copilot.aggregate_order,"replay_identity":request.replay_identity,"copilot_run_digest":copilot_run_digest,"checkpoint_digest":checkpoint_digest,"federation_envelope_digest":federation_envelope_digest,"workflow_digest":workflow_digest,"omissions":omissions,"uncertainty":copilot.uncertainty,"negative_evidence":copilot.negative_evidence,"boundary":PRECLINICAL_BOUNDARY,"raw_data_local":true,"federation_export":"aggregate-only"});
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "worldgen-federated-continual-evidence-workflow:{}",
            request.workflow_id
        ),
        "application/vnd.aurora.worldgen.federated-continual-research-workflow+json",
        &payload,
        vec![],
        vec![],
    )
    .map_err(|error| {
        FederatedContinualEvidenceSurveillanceWorkflowError::Artifact(error.to_string())
    })?;
    let effect_receipts = if disposition == FederatedContinualResearchCopilotDisposition::Blocked {
        vec!["block:unsafe-release".into()]
    } else if !compensation_order.is_empty() {
        vec![format!("compensate:research-work:{}", request.workflow_id)]
    } else {
        vec![format!("schedule:research-work:{}", request.workflow_id)]
    };
    let receipt = FederatedContinualEvidenceSurveillanceWorkflowReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        agent_id: request.request.agent_id.clone(),
        federation_id: request.request.federation_id.clone(),
        purpose: request.request.purpose.clone(),
        endpoint: request.request.endpoint.clone(),
        disposition,
        stage_order,
        plan_order,
        completed_order,
        blocked_order,
        compensation_order,
        peer_order: copilot.peer_order.clone(),
        candidate_order: copilot.candidate_order.clone(),
        selected_order: copilot.selected_order.clone(),
        unresolved_order: copilot.unresolved_order.clone(),
        denied_order: copilot.denied_order.clone(),
        aggregate_order: copilot.aggregate_order.clone(),
        replay_identity: request.replay_identity.clone(),
        copilot_run_digest,
        checkpoint_digest,
        federation_envelope_digest,
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
    request: &FederatedContinualEvidenceSurveillanceWorkflowRequest,
) -> Result<(), FederatedContinualEvidenceSurveillanceWorkflowError> {
    if request.workflow_id.trim().is_empty()
        || request.checkpoint_id.trim().is_empty()
        || request.budget_units == 0
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.request.boundary != PRECLINICAL_BOUNDARY
        || !request.request.raw_data_local
    {
        return Err(FederatedContinualEvidenceSurveillanceWorkflowError::Invalid("workflow identity, checkpoint, budget, locality, or preclinical boundary is invalid".into()));
    }
    let expected = CANONICAL_STAGES
        .iter()
        .map(|stage| (*stage).to_string())
        .collect::<Vec<_>>();
    if request.requested_stage_order != expected {
        return Err(
            FederatedContinualEvidenceSurveillanceWorkflowError::Invalid(
                "workflow stage order is not canonical".into(),
            ),
        );
    }
    if request.request.replay_identity.as_str().len() != 64
        || request.replay_identity.as_str().len() != 64
    {
        return Err(
            FederatedContinualEvidenceSurveillanceWorkflowError::Invalid(
                "replay identity is invalid".into(),
            ),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::federated_continual_evidence_surveillance_research_copilot::FederatedCopilotEvidenceContribution;

    fn request() -> FederatedContinualEvidenceSurveillanceWorkflowRequest {
        FederatedContinualEvidenceSurveillanceWorkflowRequest {
            request: FederatedContinualEvidenceSurveillanceResearchCopilotRequest {
                request_id: "req-16".into(),
                agent_id: "agent-16".into(),
                federation_id: "fed-16".into(),
                purpose: "preclinical-evidence-surveillance".into(),
                endpoint: "https://local.example/federation".into(),
                semantic_profile: "profile-1".into(),
                allowed_artifacts: vec!["summary".into()],
                min_peer_quorum: 2,
                declared_tools: vec!["evidence.aggregate".into()],
                requested_tool: "evidence.aggregate".into(),
                max_tool_calls: 2,
                dry_run: true,
                approval_reference: None,
                approval_granted: false,
                contributions: vec![
                    FederatedCopilotEvidenceContribution {
                        peer_id: "peer-a".into(),
                        institution_id: "inst-a".into(),
                        source_id: "source-a".into(),
                        semantic_profile: "profile-1".into(),
                        artifact_kind: "summary".into(),
                        digest: Some(ContentHash::of_bytes(b"source-a")),
                        signed: true,
                        permitted_artifact: true,
                        aggregate_only: true,
                        evidence_state: EvidenceState::Supported,
                        negative_result: false,
                    },
                    FederatedCopilotEvidenceContribution {
                        peer_id: "peer-b".into(),
                        institution_id: "inst-b".into(),
                        source_id: "source-b".into(),
                        semantic_profile: "profile-1".into(),
                        artifact_kind: "summary".into(),
                        digest: Some(ContentHash::of_bytes(b"source-b")),
                        signed: true,
                        permitted_artifact: true,
                        aggregate_only: true,
                        evidence_state: EvidenceState::Supported,
                        negative_result: false,
                    },
                ],
                policy_allow: true,
                protected_closure: true,
                raw_data_local: true,
                replay_identity: ContentHash::of_bytes(b"copilot-16"),
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            workflow_id: "workflow-16".into(),
            requested_stage_order: CANONICAL_STAGES
                .iter()
                .map(|stage| (*stage).to_string())
                .collect(),
            checkpoint_id: "checkpoint-16".into(),
            budget_units: 8,
            replay_identity: ContentHash::of_bytes(b"workflow-16"),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a2_and_aggregate_only() {
        let m = federated_continual_evidence_surveillance_workflow_fabric_manifest();
        assert_eq!(m.autonomy_tier, AutonomyTier::A2);
        assert!(m.validate().is_ok());
        assert!(m.behavior.contains("aggregate-only"));
    }
    #[test]
    fn schedules_quorum_workflow() {
        let receipt =
            schedule_federated_continual_evidence_surveillance_workflow(&request()).unwrap();
        assert_eq!(receipt.feature_id, FEATURE_ID);
        assert_eq!(receipt.peer_order.len(), 2);
        assert!(!receipt.federation_envelope_digest.as_str().is_empty());
    }
    #[test]
    fn policy_denial_blocks_release() {
        let mut r = request();
        r.request.policy_allow = false;
        let receipt = schedule_federated_continual_evidence_surveillance_workflow(&r).unwrap();
        assert_eq!(
            receipt.disposition,
            FederatedContinualResearchCopilotDisposition::Blocked
        );
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn quorum_failure_is_blocked() {
        let mut r = request();
        r.request.min_peer_quorum = 3;
        let receipt = schedule_federated_continual_evidence_surveillance_workflow(&r).unwrap();
        assert_eq!(
            receipt.disposition,
            FederatedContinualResearchCopilotDisposition::Blocked
        );
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item.contains("peer-quorum")));
    }
    #[test]
    fn budget_adds_compensation() {
        let mut r = request();
        r.budget_units = 1;
        let receipt = schedule_federated_continual_evidence_surveillance_workflow(&r).unwrap();
        assert!(receipt
            .compensation_order
            .iter()
            .any(|item| item.contains("budget-exhausted")));
        assert!(receipt.effect_receipts[0].starts_with("compensate:"));
    }
    #[test]
    fn stage_order_is_rejected() {
        let mut r = request();
        r.requested_stage_order.reverse();
        assert!(schedule_federated_continual_evidence_surveillance_workflow(&r).is_err());
    }
    #[test]
    fn replay_is_stable() {
        let r = request();
        assert_eq!(
            schedule_federated_continual_evidence_surveillance_workflow(&r).unwrap(),
            schedule_federated_continual_evidence_surveillance_workflow(&r).unwrap()
        );
    }
}
