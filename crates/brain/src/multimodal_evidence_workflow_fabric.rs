//! Multimodal multi-study evidence-surveillance workflow fabric.
//!
//! Atlas feature: `AFA-brain-P01-F14`. This A2 fabric schedules comparable multimodal
//! evidence work with explicit study/modality closure, approval, checkpoints, and replay.

use crate::multimodal_evidence_surveillance::{
    surveil_multimodal_evidence, MultimodalEvidenceDisposition, MultimodalEvidenceFeedRequest,
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

pub const FEATURE_ID: &str = "AFA-brain-P01-F14";
pub const CONTRACT_VERSION: &str = "brain-multimodal-evidence-workflow-fabric/1.0";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet3@1";
pub const MAX_STAGES: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalWorkflowRequest {
    pub request: MultimodalEvidenceFeedRequest,
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
pub struct MultimodalWorkflowReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub scope: String,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub disposition: MultimodalEvidenceDisposition,
    pub stage_order: Vec<String>,
    pub plan_order: Vec<String>,
    pub completed_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub compensation_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub evidence_receipt_digest: ContentHash,
    pub checkpoint_digest: ContentHash,
    pub workflow_digest: ContentHash,
    pub comparability_digest: ContentHash,
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
pub enum MultimodalWorkflowError {
    #[error("invalid multimodal workflow request: {0}")]
    Invalid(String),
    #[error("multimodal workflow artifact failed: {0}")]
    Artifact(String),
    #[error("multimodal workflow engine failed: {0}")]
    Engine(String),
}

impl MultimodalWorkflowReceipt {
    pub fn validate(&self) -> Result<(), MultimodalWorkflowError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.study_order.len() < 2
            || self.modality_order.len() < 2
            || self.stage_order.is_empty()
            || self.plan_order.is_empty()
            || self.completed_order.is_empty()
            || self.effect_receipts.is_empty()
            || self.budget_units == 0
        {
            return Err(MultimodalWorkflowError::Invalid("multimodal workflow identity, study/modality floors, stages, plan, locality, budget, or effects are incomplete".into()));
        }
        if self
            .qualified_order
            .iter()
            .chain(self.unknown_order.iter())
            .any(|id| !self.candidate_order.contains(id))
        {
            return Err(MultimodalWorkflowError::Invalid(
                "multimodal workflow state is not covered by candidates".into(),
            ));
        }
        for values in [
            &self.study_order,
            &self.modality_order,
            &self.stage_order,
            &self.plan_order,
            &self.completed_order,
            &self.blocked_order,
            &self.compensation_order,
            &self.candidate_order,
            &self.qualified_order,
            &self.unknown_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(MultimodalWorkflowError::Invalid(
                    "multimodal workflow ordering is not canonical".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("schedule:research-work:")
                && !effect.starts_with("compensate:research-work:")
                && effect != "block:unsafe-release"
        }) {
            return Err(MultimodalWorkflowError::Invalid(
                "multimodal workflow effect is outside schedule/compensation gate".into(),
            ));
        }
        if self.disposition == MultimodalEvidenceDisposition::Qualified
            && !self
                .effect_receipts
                .iter()
                .any(|effect| effect.starts_with("schedule:research-work:"))
        {
            return Err(MultimodalWorkflowError::Invalid(
                "qualified multimodal workflow requires schedule receipt".into(),
            ));
        }
        if self.disposition == MultimodalEvidenceDisposition::Blocked
            && self.effect_receipts != ["block:unsafe-release"]
        {
            return Err(MultimodalWorkflowError::Invalid(
                "blocked multimodal workflow must be explicitly blocked".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| MultimodalWorkflowError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, MultimodalWorkflowError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| MultimodalWorkflowError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| MultimodalWorkflowError::Artifact(error.to_string()))
    }
}

pub fn multimodal_evidence_workflow_fabric_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["agent developer".into(), "multimodal workflow steward".into()].into(), behavior: "schedules a checkpointed multimodal EvidenceFeed2 workflow with required study/modality comparability and compensation receipts".into(), value: "orchestrates comparable imaging and omics evidence work without treating missing modalities or incomplete closure as completion".into(), inputs: vec![TypedPort { name: "multimodal_workflow_request".into(), schema: "ResearchWorkflowSpec2@1".into(), required: true }], outputs: vec![TypedPort { name: "qualified_evidence_set".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["schedule:research-work".into(), "read:local-research-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "ome-ngff-rfc5".into(), state: EvidenceState::Supported, locator: Some("https://ngff.openmicroscopy.org/rfc/5/".into()) }], authority_requirements: vec![AuthorityRequirement { role: "multimodal workflow approver".into(), reason: "approve required modality and comparability closure before scheduling".into() }], autonomy_tier: AutonomyTier::A2, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn compile_multimodal_evidence_workflow(
    request: &MultimodalWorkflowRequest,
) -> Result<MultimodalWorkflowReceipt, MultimodalWorkflowError> {
    validate_request(request)?;
    let evidence = surveil_multimodal_evidence(&request.request)
        .map_err(|error| MultimodalWorkflowError::Engine(error.to_string()))?;
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
        "stage:checkpoint".into(),
        "stage:compare-modalities".into(),
        "stage:surveil-evidence".into(),
        "stage:validate-input".into(),
    ];
    let mut plan_order = BTreeSet::new();
    let mut completed_order = BTreeSet::new();
    let mut blocked_order = BTreeSet::new();
    let mut compensation_order = BTreeSet::new();
    for stage in &stage_order {
        plan_order.insert(format!("plan:{stage}"));
        completed_order.insert(stage.clone());
    }
    let modality_order = request.request.required_modalities.clone();
    let comparability_digest = ContentHash::of_value(&json!({"study_order": request.request.study_ids, "required_modalities": request.request.required_modalities, "modality_order": modality_order, "replay_identity": request.replay_identity})).map_err(|error| MultimodalWorkflowError::Artifact(error.to_string()))?;
    if request.policy_allow
        && request.protected_closure
        && request.raw_data_local
        && matches!(
            evidence.disposition,
            MultimodalEvidenceDisposition::Unknown | MultimodalEvidenceDisposition::Partial
        )
    {
        plan_order.insert("plan:retain-multimodal-unknown".into());
        blocked_order.extend(evidence.unknown_order.iter().cloned());
        compensation_order.insert("compensate:research-work:retain-incomplete-modalities".into());
        omissions.insert("workflow:no-comparable-qualified-evidence-to-schedule".into());
    } else {
        plan_order.insert("plan:publish-qualified-multimodal-artifact".into());
    }
    let plan_count = u64::try_from(plan_order.len()).unwrap_or(u64::MAX);
    let actionable = u64::from(request.budget_units) >= plan_count
        && request.approval_reference != ContentHash::of_bytes(&[])
        && request.policy_allow
        && request.protected_closure
        && request.raw_data_local
        && evidence.disposition != MultimodalEvidenceDisposition::Blocked;
    if u64::from(request.budget_units) < plan_count {
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
    let disposition = if !actionable {
        MultimodalEvidenceDisposition::Blocked
    } else {
        evidence.disposition
    };
    let plan_vec = plan_order.into_iter().collect::<Vec<_>>();
    let completed_vec = completed_order.into_iter().collect::<Vec<_>>();
    let evidence_digest = evidence
        .digest()
        .map_err(|error| MultimodalWorkflowError::Engine(error.to_string()))?;
    let checkpoint_digest = ContentHash::of_value(&json!({"workflow_id": request.workflow_id, "checkpoint_id": request.checkpoint_id, "stage_order": stage_order, "comparability_digest": comparability_digest, "replay_identity": request.replay_identity})).map_err(|error| MultimodalWorkflowError::Artifact(error.to_string()))?;
    let workflow_digest = ContentHash::of_value(&json!({"workflow_id": request.workflow_id, "plan_order": plan_vec, "completed_order": completed_vec, "checkpoint_digest": checkpoint_digest, "approval_reference": request.approval_reference, "budget_units": request.budget_units, "replay_identity": request.replay_identity})).map_err(|error| MultimodalWorkflowError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request.request_id, "workflow_id": request.workflow_id, "scope": request.request.scope, "study_order": request.request.study_ids, "modality_order": modality_order, "disposition": disposition, "stage_order": stage_order, "plan_order": plan_vec, "completed_order": completed_vec, "blocked_order": blocked_order, "compensation_order": compensation_order, "candidate_order": evidence.candidate_order, "qualified_order": evidence.qualified_order, "unknown_order": evidence.unknown_order, "evidence_receipt_digest": evidence_digest, "checkpoint_digest": checkpoint_digest, "workflow_digest": workflow_digest, "comparability_digest": comparability_digest, "approval_reference": request.approval_reference, "replay_identity": request.replay_identity, "budget_units": request.budget_units, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-multimodal-evidence-workflow:{}", request.workflow_id),
        "application/vnd.aurora.multimodal-research-workflow-receipt+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| MultimodalWorkflowError::Artifact(error.to_string()))?;
    let has_compensation = !compensation_order.is_empty();
    let receipt = MultimodalWorkflowReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        scope: request.request.scope.clone(),
        study_order: request.request.study_ids.clone(),
        modality_order: modality_order.clone(),
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
        blocked_order: blocked_order.into_iter().collect(),
        compensation_order: compensation_order.into_iter().collect(),
        candidate_order: evidence.candidate_order.clone(),
        qualified_order: evidence.qualified_order.clone(),
        unknown_order: evidence.unknown_order.clone(),
        evidence_receipt_digest: evidence_digest,
        checkpoint_digest,
        workflow_digest,
        comparability_digest,
        approval_reference: request.approval_reference.clone(),
        replay_identity: request.replay_identity.clone(),
        budget_units: request.budget_units,
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts: if disposition == MultimodalEvidenceDisposition::Qualified {
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

fn validate_request(request: &MultimodalWorkflowRequest) -> Result<(), MultimodalWorkflowError> {
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
        return Err(MultimodalWorkflowError::Invalid("multimodal workflow identity, canonical stages, approval, budget, replay, or boundary is incomplete".into()));
    }
    if request.requested_stage_order
        != vec![
            "stage:checkpoint",
            "stage:compare-modalities",
            "stage:surveil-evidence",
            "stage:validate-input",
        ]
    {
        return Err(MultimodalWorkflowError::Invalid(
            "multimodal workflow stages do not match the versioned fabric protocol".into(),
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
    fn observation(
        id: &str,
        study: &str,
        modality: &str,
        state: EvidenceState,
    ) -> EvidenceObservation {
        EvidenceObservation {
            evidence_id: format!("evidence:{id}"),
            source_id: format!("source:{id}"),
            study_id: study.into(),
            modality: modality.into(),
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
    fn request(state: EvidenceState) -> MultimodalWorkflowRequest {
        MultimodalWorkflowRequest {
            request: MultimodalEvidenceFeedRequest {
                request_id: "request:multimodal-workflow".into(),
                study_ids: vec!["study:a".into(), "study:b".into()],
                scope: "organoid:neural".into(),
                query: "synaptic density".into(),
                minimum_relevance_milli: 700,
                required_modalities: vec!["imaging".into(), "transcriptomics".into()],
                observations: vec![observation("a", "study:a", "imaging", state)],
                replay_identity: hash("replay"),
                policy_allow: true,
                protected_closure: true,
                raw_data_local: true,
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            workflow_id: "workflow:multimodal".into(),
            requested_stage_order: vec![
                "stage:checkpoint".into(),
                "stage:compare-modalities".into(),
                "stage:surveil-evidence".into(),
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
    fn manifest_is_a2_and_comparability_scoped() {
        let manifest = multimodal_evidence_workflow_fabric_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A2);
    }
    #[test]
    fn incomplete_modalities_compensate() {
        let receipt =
            compile_multimodal_evidence_workflow(&request(EvidenceState::Supported)).unwrap();
        assert!(!receipt.compensation_order.is_empty());
    }
    #[test]
    fn policy_denial_blocks() {
        let mut input = request(EvidenceState::Supported);
        input.policy_allow = false;
        let receipt = compile_multimodal_evidence_workflow(&input).unwrap();
        assert_eq!(receipt.disposition, MultimodalEvidenceDisposition::Blocked);
    }
    #[test]
    fn stage_protocol_is_required() {
        let mut input = request(EvidenceState::Supported);
        input.requested_stage_order.reverse();
        assert!(compile_multimodal_evidence_workflow(&input).is_err());
    }
    #[test]
    fn approval_is_required() {
        let mut input = request(EvidenceState::Supported);
        input.approval_reference = ContentHash::of_bytes(&[]);
        assert!(compile_multimodal_evidence_workflow(&input).is_err());
    }
    #[test]
    fn replay_mismatch_is_rejected() {
        let mut input = request(EvidenceState::Supported);
        input.replay_identity = hash("different");
        assert!(compile_multimodal_evidence_workflow(&input).is_err());
    }
}
