//! Local single-study evidence-surveillance workflow fabric.
//!
//! Atlas feature: `AFA-brain-P01-F13`. The fabric schedules a typed evidence workflow with
//! checkpoints and compensation receipts. It is a local A1 orchestration product, not an
//! experiment, hypothesis, clinical workflow, or generic implementation task.

use crate::evidence_surveillance::{
    surveil_evidence, EvidenceFeedRequest, EvidenceSurveillanceDisposition,
};
use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-brain-P01-F13";
pub const CONTRACT_VERSION: &str = "brain-evidence-surveillance-workflow-fabric/1.0";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet3@1";
pub const MAX_STAGES: usize = 8;
const WORKFLOW_CONTENT_TYPE: &str = "application/vnd.aurora.research-workflow-receipt+json";
const MAX_ITEMS: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceWorkflowRequest {
    pub request: EvidenceFeedRequest,
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
pub struct EvidenceWorkflowReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub study_id: String,
    pub scope: String,
    pub disposition: EvidenceSurveillanceDisposition,
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
pub enum EvidenceWorkflowError {
    #[error("invalid evidence workflow request: {0}")]
    Invalid(String),
    #[error("evidence workflow artifact failed: {0}")]
    Artifact(String),
    #[error("evidence workflow engine failed: {0}")]
    Engine(String),
}

impl EvidenceWorkflowReceipt {
    pub fn validate(&self) -> Result<(), EvidenceWorkflowError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.study_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.stage_order.is_empty()
            || self.plan_order.is_empty()
            || self.completed_order.is_empty()
            || self.effect_receipts.is_empty()
            || self.budget_units == 0
        {
            return Err(EvidenceWorkflowError::Invalid("workflow identity, stages, plan, checkpoint, locality, budget, or effects are incomplete".into()));
        }
        let collections = [
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
        ];
        if collections.iter().any(|values| values.len() > MAX_ITEMS) {
            return Err(EvidenceWorkflowError::Invalid(
                "workflow collection exceeds the bounded contract limit".into(),
            ));
        }
        let candidates = self.candidate_order.iter().collect::<BTreeSet<_>>();
        let qualified = self.qualified_order.iter().collect::<BTreeSet<_>>();
        let unknown = self.unknown_order.iter().collect::<BTreeSet<_>>();
        if !qualified.is_subset(&candidates)
            || !unknown.is_subset(&candidates)
            || !qualified.is_disjoint(&unknown)
        {
            return Err(EvidenceWorkflowError::Invalid(
                "workflow evidence state is not covered by candidates".into(),
            ));
        }
        for values in collections {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(EvidenceWorkflowError::Invalid(
                    "workflow ordering is not canonical".into(),
                ));
            }
        }
        let expected_stages = vec![
            "stage:checkpoint".to_string(),
            "stage:surveil-evidence".to_string(),
            "stage:validate-input".to_string(),
        ];
        if self.stage_order != expected_stages || self.completed_order != expected_stages {
            return Err(EvidenceWorkflowError::Invalid(
                "workflow stages are not canonical or complete".into(),
            ));
        }
        let mut expected_plan = expected_stages
            .iter()
            .map(|stage| format!("plan:{stage}"))
            .collect::<Vec<_>>();
        if self.qualified_order.is_empty() {
            expected_plan.push("plan:retain-unknown-evidence".into());
        } else {
            expected_plan.push("plan:publish-qualified-local-artifact".into());
        }
        expected_plan.sort();
        if self.plan_order != expected_plan {
            return Err(EvidenceWorkflowError::Invalid(
                "workflow plan does not match evidence state".into(),
            ));
        }
        let expected_blocked = if self.qualified_order.is_empty() {
            self.unknown_order.clone()
        } else {
            Vec::new()
        };
        if self.blocked_order != expected_blocked {
            return Err(EvidenceWorkflowError::Invalid(
                "workflow blocked state does not match evidence state".into(),
            ));
        }
        let expected_compensation: Vec<String> = if self.qualified_order.is_empty() {
            vec!["compensate:research-work:retain-unresolved-evidence".into()]
        } else {
            Vec::new()
        };
        if self.compensation_order != expected_compensation {
            return Err(EvidenceWorkflowError::Invalid(
                "workflow compensation does not match evidence state".into(),
            ));
        }
        let gate_blocked = self.omissions.iter().any(|item| {
            item == "workflow:policy-denied"
                || item == "workflow:protected-closure-incomplete"
                || item == "workflow:raw-data-locality-failed"
        }) || self.negative_evidence.iter().any(|item| {
            item == "request:policy-denied" || item == "request:raw-data-locality-failed"
        }) || self
            .uncertainty
            .iter()
            .any(|item| item == "request:protected-closure-incomplete");
        let expected_disposition = if gate_blocked {
            EvidenceSurveillanceDisposition::Blocked
        } else if self.qualified_order.is_empty() {
            EvidenceSurveillanceDisposition::Unknown
        } else if self.blocked_order.is_empty()
            && self.omissions.is_empty()
            && self.uncertainty.is_empty()
            && self.negative_evidence.is_empty()
        {
            EvidenceSurveillanceDisposition::Qualified
        } else {
            EvidenceSurveillanceDisposition::Partial
        };
        if self.disposition != expected_disposition {
            return Err(EvidenceWorkflowError::Invalid(
                "workflow disposition does not match evidence state or gates".into(),
            ));
        }
        for value in [
            &self.evidence_receipt_digest,
            &self.checkpoint_digest,
            &self.workflow_digest,
            &self.replay_identity,
        ] {
            if value.as_str().len() != 64 {
                return Err(EvidenceWorkflowError::Invalid(
                    "workflow digest length is invalid".into(),
                ));
            }
        }
        let expected_workflow_digest = ContentHash::of_value(&json!({
            "workflow_id": self.workflow_id,
            "plan_order": self.plan_order,
            "completed_order": self.completed_order,
            "checkpoint_digest": self.checkpoint_digest,
            "budget_units": self.budget_units,
            "replay_identity": self.replay_identity
        }))
        .map_err(|error| EvidenceWorkflowError::Artifact(error.to_string()))?;
        if self.workflow_digest != expected_workflow_digest {
            return Err(EvidenceWorkflowError::Invalid(
                "workflow digest does not match its receipt fields".into(),
            ));
        }
        let expected_effect = if self.disposition == EvidenceSurveillanceDisposition::Qualified {
            vec![format!("schedule:research-work:{}", self.workflow_id)]
        } else if !self.compensation_order.is_empty() {
            vec![format!("compensate:research-work:{}", self.workflow_id)]
        } else {
            vec!["block:unsafe-release".into()]
        };
        if self.effect_receipts != expected_effect {
            return Err(EvidenceWorkflowError::Invalid(
                "workflow effect does not match disposition".into(),
            ));
        }
        let expected_artifact_id = format!("brain-evidence-workflow:{}", self.workflow_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != WORKFLOW_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(EvidenceWorkflowError::Invalid(
                "workflow artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| EvidenceWorkflowError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| EvidenceWorkflowError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, EvidenceWorkflowError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| EvidenceWorkflowError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| EvidenceWorkflowError::Artifact(error.to_string()))
    }
}

pub fn evidence_workflow_fabric_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["research workflow operator".into(), "workflow reliability engineer".into()].into(), behavior: "schedules a checkpointed local EvidenceFeed1 workflow with deterministic stages, compensation, and replay receipts".into(), value: "turns evidence surveillance into a resumable operator workflow without hiding omissions or executing external effects".into(), inputs: vec![TypedPort { name: "evidence_workflow_request".into(), schema: "ResearchWorkflowSpec1@1".into(), required: true }], outputs: vec![TypedPort { name: "qualified_evidence_set".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["schedule:research-work".into(), "read:local-research-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn compile_evidence_workflow(
    request: &EvidenceWorkflowRequest,
) -> Result<EvidenceWorkflowReceipt, EvidenceWorkflowError> {
    validate_request(request)?;
    let evidence = surveil_evidence(&request.request)
        .map_err(|error| EvidenceWorkflowError::Engine(error.to_string()))?;
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
        "stage:surveil-evidence".into(),
        "stage:validate-input".into(),
    ];
    let mut plan_order = BTreeSet::new();
    let mut completed_order = BTreeSet::new();
    let mut blocked_order = BTreeSet::new();
    let mut compensation_order = BTreeSet::new();
    for stage in &stage_order {
        plan_order.insert(format!("plan:{stage}"));
    }
    completed_order.extend(stage_order.iter().cloned());
    if evidence.qualified_order.is_empty() {
        plan_order.insert("plan:retain-unknown-evidence".into());
        blocked_order.extend(evidence.unknown_order.iter().cloned());
        compensation_order.insert("compensate:research-work:retain-unresolved-evidence".into());
        omissions.insert("workflow:no-qualified-evidence-to-schedule".into());
    } else {
        plan_order.insert("plan:publish-qualified-local-artifact".into());
    }
    let plan_count = u64::try_from(plan_order.len()).unwrap_or(u64::MAX);
    let actionable = u64::from(request.budget_units) >= plan_count
        && request.policy_allow
        && request.protected_closure
        && request.raw_data_local
        && evidence.disposition != EvidenceSurveillanceDisposition::Blocked;
    if u64::from(request.budget_units) < plan_count {
        omissions.insert("workflow:budget-exhausted".into());
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
        EvidenceSurveillanceDisposition::Blocked
    } else {
        evidence.disposition
    };
    let plan_vec = plan_order.into_iter().collect::<Vec<_>>();
    let completed_vec = completed_order.into_iter().collect::<Vec<_>>();
    let evidence_digest = evidence
        .digest()
        .map_err(|error| EvidenceWorkflowError::Engine(error.to_string()))?;
    let checkpoint_digest = ContentHash::of_value(&json!({"workflow_id": request.workflow_id, "checkpoint_id": request.checkpoint_id, "stage_order": stage_order, "replay_identity": request.replay_identity})).map_err(|error| EvidenceWorkflowError::Artifact(error.to_string()))?;
    let workflow_digest = ContentHash::of_value(&json!({"workflow_id": request.workflow_id, "plan_order": plan_vec, "completed_order": completed_vec, "checkpoint_digest": checkpoint_digest, "budget_units": request.budget_units, "replay_identity": request.replay_identity})).map_err(|error| EvidenceWorkflowError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request.request_id, "workflow_id": request.workflow_id, "study_id": request.request.study_id, "scope": request.request.scope, "disposition": disposition, "stage_order": stage_order, "plan_order": plan_vec, "completed_order": completed_vec, "blocked_order": blocked_order, "compensation_order": compensation_order, "candidate_order": evidence.candidate_order, "qualified_order": evidence.qualified_order, "unknown_order": evidence.unknown_order, "evidence_receipt_digest": evidence_digest, "checkpoint_digest": checkpoint_digest, "workflow_digest": workflow_digest, "replay_identity": request.replay_identity, "budget_units": request.budget_units, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "raw_data_local": true, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-evidence-workflow:{}", request.workflow_id),
        WORKFLOW_CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| EvidenceWorkflowError::Artifact(error.to_string()))?;
    let has_compensation = !compensation_order.is_empty();
    let receipt = EvidenceWorkflowReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        study_id: request.request.study_id.clone(),
        scope: request.request.scope.clone(),
        disposition,
        stage_order,
        plan_order: plan_vec.clone(),
        completed_order: completed_vec.clone(),
        blocked_order: blocked_order.into_iter().collect(),
        compensation_order: compensation_order.into_iter().collect(),
        candidate_order: evidence.candidate_order.clone(),
        qualified_order: evidence.qualified_order.clone(),
        unknown_order: evidence.unknown_order.clone(),
        evidence_receipt_digest: evidence_digest,
        checkpoint_digest,
        workflow_digest,
        replay_identity: request.replay_identity.clone(),
        budget_units: request.budget_units,
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts: if disposition == EvidenceSurveillanceDisposition::Qualified {
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

fn receipt_payload(receipt: &EvidenceWorkflowReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "workflow_id": receipt.workflow_id,
        "study_id": receipt.study_id,
        "scope": receipt.scope,
        "disposition": receipt.disposition,
        "stage_order": receipt.stage_order,
        "plan_order": receipt.plan_order,
        "completed_order": receipt.completed_order,
        "blocked_order": receipt.blocked_order,
        "compensation_order": receipt.compensation_order,
        "candidate_order": receipt.candidate_order,
        "qualified_order": receipt.qualified_order,
        "unknown_order": receipt.unknown_order,
        "evidence_receipt_digest": receipt.evidence_receipt_digest,
        "checkpoint_digest": receipt.checkpoint_digest,
        "workflow_digest": receipt.workflow_digest,
        "replay_identity": receipt.replay_identity,
        "budget_units": receipt.budget_units,
        "omissions": receipt.omissions,
        "uncertainty": receipt.uncertainty,
        "negative_evidence": receipt.negative_evidence,
        "raw_data_local": receipt.raw_data_local,
        "boundary": receipt.boundary,
    })
}

fn validate_request(request: &EvidenceWorkflowRequest) -> Result<(), EvidenceWorkflowError> {
    if request.workflow_id.trim().is_empty()
        || request.checkpoint_id.trim().is_empty()
        || request.requested_stage_order.len() != 3
        || request
            .requested_stage_order
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || request.budget_units == 0
        || request.request.replay_identity != request.replay_identity
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(EvidenceWorkflowError::Invalid(
            "workflow identity, canonical stages, budget, replay, or boundary is incomplete".into(),
        ));
    }
    if request.requested_stage_order
        != vec![
            "stage:checkpoint",
            "stage:surveil-evidence",
            "stage:validate-input",
        ]
    {
        return Err(EvidenceWorkflowError::Invalid(
            "workflow stages do not match the versioned fabric protocol".into(),
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
    fn observation(state: EvidenceState) -> EvidenceObservation {
        EvidenceObservation {
            evidence_id: "evidence:a".into(),
            source_id: "source:a".into(),
            study_id: "study:organoid".into(),
            modality: "imaging".into(),
            scope: "organoid:neural".into(),
            relevance_milli: 900,
            state,
            semantic_digest: hash("semantic"),
            artifact_digest: hash("artifact"),
            provenance_digest: hash("provenance"),
            replay_identity: hash("replay"),
            omissions: Vec::new(),
            negative_evidence: Vec::new(),
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    fn request(state: EvidenceState) -> EvidenceWorkflowRequest {
        EvidenceWorkflowRequest {
            request: EvidenceFeedRequest {
                request_id: "request:workflow".into(),
                study_id: "study:organoid".into(),
                scope: "organoid:neural".into(),
                query: "synaptic density".into(),
                minimum_relevance_milli: 700,
                observations: vec![observation(state)],
                replay_identity: hash("replay"),
                policy_allow: true,
                protected_closure: true,
                raw_data_local: true,
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            workflow_id: "workflow:evidence".into(),
            requested_stage_order: vec![
                "stage:checkpoint".into(),
                "stage:surveil-evidence".into(),
                "stage:validate-input".into(),
            ],
            checkpoint_id: "checkpoint:1".into(),
            budget_units: 8,
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1_and_scheduled() {
        let manifest = evidence_workflow_fabric_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A1);
    }
    #[test]
    fn supported_evidence_is_scheduled() {
        let receipt = compile_evidence_workflow(&request(EvidenceState::Supported)).unwrap();
        assert!(receipt.effect_receipts[0].starts_with("schedule:research-work:"));
        assert_eq!(receipt.completed_order.len(), 3);
    }
    #[test]
    fn unknown_evidence_compensates() {
        let receipt = compile_evidence_workflow(&request(EvidenceState::Unknown)).unwrap();
        assert!(!receipt.compensation_order.is_empty());
    }
    #[test]
    fn policy_denial_blocks() {
        let mut input = request(EvidenceState::Supported);
        input.policy_allow = false;
        let receipt = compile_evidence_workflow(&input).unwrap();
        assert_eq!(
            receipt.disposition,
            EvidenceSurveillanceDisposition::Blocked
        );
    }
    #[test]
    fn stage_protocol_is_required() {
        let mut input = request(EvidenceState::Supported);
        input.requested_stage_order.reverse();
        assert!(compile_evidence_workflow(&input).is_err());
    }
    #[test]
    fn replay_mismatch_is_rejected() {
        let mut input = request(EvidenceState::Supported);
        input.replay_identity = hash("different");
        assert!(compile_evidence_workflow(&input).is_err());
    }
}
