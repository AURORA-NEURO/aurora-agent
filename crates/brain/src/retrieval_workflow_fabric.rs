//! Local retrieval-and-synthesis workflow fabric.
//!
//! Atlas feature: `AFA-brain-P02-F13`. The fabric turns the deterministic retrieval engine
//! into a checkpointed, resumable workflow with explicit compensation for unresolved evidence.

use crate::retrieval_synthesis::{
    synthesize_retrieval, ScopedRetrievalQuery, SynthesisDisposition,
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

pub const FEATURE_ID: &str = "AFA-brain-P02-F13";
pub const CONTRACT_VERSION: &str = "brain-retrieval-workflow-fabric/1.0";
pub const OUTPUT_SCHEMA: &str = "RetrievalWorkflowReceipt1@1";
pub const MAX_STAGES: usize = 8;
const WORKFLOW_CONTENT_TYPE: &str = "application/vnd.aurora.retrieval-workflow-receipt+json";
const MAX_TEXT_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalWorkflowRequest {
    pub request: ScopedRetrievalQuery,
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
pub struct RetrievalWorkflowReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub checkpoint_id: String,
    pub study_id: String,
    pub scope: String,
    pub disposition: SynthesisDisposition,
    pub stage_order: Vec<String>,
    pub plan_order: Vec<String>,
    pub completed_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub compensation_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub ranked_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub synthesis_digest: ContentHash,
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
pub enum RetrievalWorkflowError {
    #[error("invalid retrieval workflow request: {0}")]
    Invalid(String),
    #[error("retrieval workflow artifact failed: {0}")]
    Artifact(String),
    #[error("retrieval workflow engine failed: {0}")]
    Engine(String),
}

impl RetrievalWorkflowReceipt {
    pub fn validate(&self) -> Result<(), RetrievalWorkflowError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
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
            return Err(RetrievalWorkflowError::Invalid(
                "workflow identity, stages, plan, locality, budget, or effects are incomplete"
                    .into(),
            ));
        }
        for (value, field) in [
            (&self.request_id, "request_id"),
            (&self.workflow_id, "workflow_id"),
            (&self.study_id, "study_id"),
            (&self.scope, "scope"),
            (&self.boundary, "boundary"),
        ] {
            validate_text(value, field)?;
        }
        for (values, field) in [
            (&self.stage_order, "stage_order"),
            (&self.plan_order, "plan_order"),
            (&self.completed_order, "completed_order"),
            (&self.blocked_order, "blocked_order"),
            (&self.compensation_order, "compensation_order"),
            (&self.candidate_order, "candidate_order"),
            (&self.omissions, "omissions"),
            (&self.uncertainty, "uncertainty"),
            (&self.negative_evidence, "negative_evidence"),
            (&self.effect_receipts, "effect_receipts"),
        ] {
            validate_sorted_unique(values, field)?;
        }
        for (values, field) in [
            (&self.ranked_order, "ranked_order"),
            (&self.qualified_order, "qualified_order"),
            (&self.unknown_order, "unknown_order"),
        ] {
            validate_unique(values, field)?;
        }
        let candidate_values = self
            .candidate_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let ranked_values = self.ranked_order.iter().cloned().collect::<BTreeSet<_>>();
        let qualified_values = self
            .qualified_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let blocked_values = self.blocked_order.iter().cloned().collect::<BTreeSet<_>>();
        let unknown_values = self.unknown_order.iter().cloned().collect::<BTreeSet<_>>();
        if ranked_values != candidate_values {
            return Err(RetrievalWorkflowError::Invalid(
                "retrieval workflow ranked order must contain every candidate exactly once".into(),
            ));
        }
        if !qualified_values.is_subset(&candidate_values)
            || !blocked_values.is_subset(&candidate_values)
            || !unknown_values.is_subset(&blocked_values)
            || !qualified_values.is_disjoint(&blocked_values)
            || qualified_values
                .union(&blocked_values)
                .cloned()
                .collect::<BTreeSet<_>>()
                != candidate_values
        {
            return Err(RetrievalWorkflowError::Invalid(
                "retrieval workflow candidate states must partition candidates".into(),
            ));
        }
        for digest in [
            &self.synthesis_digest,
            &self.checkpoint_digest,
            &self.workflow_digest,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(RetrievalWorkflowError::Invalid(
                    "retrieval workflow digest is invalid".into(),
                ));
            }
        }
        let expected_effect_receipts = if self.disposition == SynthesisDisposition::Qualified {
            vec![format!("schedule:retrieval-work:{}", self.workflow_id)]
        } else if self.disposition != SynthesisDisposition::Blocked
            && !self.compensation_order.is_empty()
        {
            self.compensation_order.clone()
        } else {
            vec!["block:unsafe-release".into()]
        };
        if self.effect_receipts != expected_effect_receipts {
            return Err(RetrievalWorkflowError::Invalid(
                "retrieval workflow effects do not match disposition and compensation".into(),
            ));
        }
        if !self.raw_data_local
            && (self.disposition != SynthesisDisposition::Blocked
                || !self
                    .omissions
                    .iter()
                    .any(|item| item == "workflow:raw-data-locality-failed"))
        {
            return Err(RetrievalWorkflowError::Invalid(
                "non-local retrieval workflows must be blocked and retain locality evidence".into(),
            ));
        }
        let expected_checkpoint_digest = ContentHash::of_value(&json!({
            "workflow_id": self.workflow_id,
            "checkpoint_id": self.checkpoint_id,
            "stage_order": self.stage_order,
            "replay_identity": self.replay_identity,
        }))
        .map_err(|error| RetrievalWorkflowError::Artifact(error.to_string()))?;
        if self.checkpoint_digest != expected_checkpoint_digest {
            return Err(RetrievalWorkflowError::Invalid(
                "retrieval workflow checkpoint digest is not bound to checkpoint state".into(),
            ));
        }
        let expected_workflow_digest = ContentHash::of_value(&json!({
            "workflow_id": self.workflow_id,
            "disposition": self.disposition,
            "plan_order": self.plan_order,
            "completed_order": self.completed_order,
            "blocked_order": self.blocked_order,
            "compensation_order": self.compensation_order,
            "checkpoint_digest": self.checkpoint_digest,
            "synthesis_digest": self.synthesis_digest,
            "budget_units": self.budget_units,
            "replay_identity": self.replay_identity,
            "raw_data_local": self.raw_data_local,
        }))
        .map_err(|error| RetrievalWorkflowError::Artifact(error.to_string()))?;
        if self.workflow_digest != expected_workflow_digest {
            return Err(RetrievalWorkflowError::Invalid(
                "retrieval workflow digest is not bound to workflow state".into(),
            ));
        }
        let expected_artifact_id = format!("brain-retrieval-workflow:{}", self.workflow_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != WORKFLOW_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(RetrievalWorkflowError::Invalid(
                "retrieval workflow artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| RetrievalWorkflowError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| RetrievalWorkflowError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, RetrievalWorkflowError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| RetrievalWorkflowError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| RetrievalWorkflowError::Artifact(error.to_string()))
    }
}

pub fn retrieval_workflow_fabric_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["retrieval workflow operator".into(), "workflow reliability engineer".into()].into(), behavior: "schedules a checkpointed local ScopedRetrievalQuery workflow with deterministic stages, compensation, and replay receipts".into(), value: "turns retrieval synthesis into a resumable research workflow without hiding omissions or executing external effects".into(), inputs: vec![TypedPort { name: "retrieval_workflow_request".into(), schema: "ResearchWorkflowSpec1@1".into(), required: true }], outputs: vec![TypedPort { name: "retrieval_workflow_receipt".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["schedule:retrieval-work".into(), "read:local-research-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn compile_retrieval_workflow(
    request: &RetrievalWorkflowRequest,
) -> Result<RetrievalWorkflowReceipt, RetrievalWorkflowError> {
    validate_request(request)?;
    let synthesis = synthesize_retrieval(&request.request)
        .map_err(|error| RetrievalWorkflowError::Engine(error.to_string()))?;
    let stage_order = request.requested_stage_order.clone();
    let mut plan_order = stage_order
        .iter()
        .map(|stage| format!("plan:{stage}"))
        .collect::<BTreeSet<_>>();
    let completed_order = stage_order.iter().cloned().collect::<BTreeSet<_>>();
    let mut blocked_order = synthesis
        .blocked_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut compensation_order = BTreeSet::new();
    let mut omissions = synthesis.omissions.iter().cloned().collect::<BTreeSet<_>>();
    let uncertainty = synthesis
        .uncertainty
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let negative = synthesis
        .negative_evidence
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if synthesis.qualified_order.is_empty() {
        plan_order.insert("plan:retain-unresolved-retrieval".into());
        compensation_order.insert("compensate:retrieval-work:retain-unresolved-evidence".into());
        omissions.insert("workflow:no-qualified-retrieval-to-schedule".into());
        blocked_order.extend(synthesis.unknown_order.iter().cloned());
    } else if synthesis.disposition != SynthesisDisposition::Qualified {
        plan_order.insert("plan:retain-partial-retrieval".into());
        compensation_order.insert("compensate:retrieval-work:retain-partial-evidence".into());
    } else {
        plan_order.insert("plan:publish-qualified-local-retrieval".into());
    }
    let plan_count = u64::try_from(plan_order.len()).unwrap_or(u64::MAX);
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
    let actionable = u64::from(request.budget_units) >= plan_count
        && request.policy_allow
        && request.protected_closure
        && request.raw_data_local
        && synthesis.disposition != SynthesisDisposition::Blocked;
    let disposition = if !actionable {
        SynthesisDisposition::Blocked
    } else {
        synthesis.disposition
    };
    if disposition == SynthesisDisposition::Blocked {
        compensation_order.clear();
    }
    let plan_order = plan_order.into_iter().collect::<Vec<_>>();
    let completed_order = completed_order.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked_order.into_iter().collect::<Vec<_>>();
    let compensation_order = compensation_order.into_iter().collect::<Vec<_>>();
    let synthesis_digest = synthesis
        .digest()
        .map_err(|error| RetrievalWorkflowError::Engine(error.to_string()))?;
    let checkpoint_digest = ContentHash::of_value(&json!({"workflow_id": request.workflow_id, "checkpoint_id": request.checkpoint_id, "stage_order": stage_order, "replay_identity": request.replay_identity}))
        .map_err(|error| RetrievalWorkflowError::Artifact(error.to_string()))?;
    let workflow_digest = ContentHash::of_value(&json!({"workflow_id": request.workflow_id, "disposition": disposition, "plan_order": plan_order, "completed_order": completed_order, "blocked_order": blocked_order, "compensation_order": compensation_order, "checkpoint_digest": checkpoint_digest, "synthesis_digest": synthesis_digest, "budget_units": request.budget_units, "replay_identity": request.replay_identity, "raw_data_local": true}))
        .map_err(|error| RetrievalWorkflowError::Artifact(error.to_string()))?;
    let effect_receipts = if disposition == SynthesisDisposition::Qualified {
        vec![format!("schedule:retrieval-work:{}", request.workflow_id)]
    } else if disposition != SynthesisDisposition::Blocked && !compensation_order.is_empty() {
        compensation_order.clone()
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request.request_id, "workflow_id": request.workflow_id, "checkpoint_id": request.checkpoint_id, "study_id": request.request.study_id, "scope": request.request.scope, "disposition": disposition, "stage_order": stage_order, "plan_order": plan_order, "completed_order": completed_order, "blocked_order": blocked_order, "compensation_order": compensation_order, "candidate_order": synthesis.candidate_order, "ranked_order": synthesis.ranked_order, "qualified_order": synthesis.qualified_order, "unknown_order": synthesis.unknown_order, "synthesis_digest": synthesis_digest, "checkpoint_digest": checkpoint_digest, "workflow_digest": workflow_digest, "replay_identity": request.replay_identity, "budget_units": request.budget_units, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "effect_receipts": effect_receipts, "raw_data_local": true, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-retrieval-workflow:{}", request.workflow_id),
        WORKFLOW_CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| RetrievalWorkflowError::Artifact(error.to_string()))?;
    let receipt = RetrievalWorkflowReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        checkpoint_id: request.checkpoint_id.clone(),
        study_id: request.request.study_id.clone(),
        scope: request.request.scope.clone(),
        disposition,
        stage_order: request.requested_stage_order.clone(),
        plan_order,
        completed_order,
        blocked_order,
        compensation_order,
        candidate_order: synthesis.candidate_order,
        ranked_order: synthesis.ranked_order,
        qualified_order: synthesis.qualified_order,
        unknown_order: synthesis.unknown_order,
        synthesis_digest,
        checkpoint_digest,
        workflow_digest,
        replay_identity: request.replay_identity.clone(),
        budget_units: request.budget_units,
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &RetrievalWorkflowRequest) -> Result<(), RetrievalWorkflowError> {
    let expected = [
        "stage:checkpoint",
        "stage:retrieve-local-candidates",
        "stage:synthesize-evidence",
        "stage:validate-output",
    ];
    for (value, field) in [
        (&request.workflow_id, "workflow_id"),
        (&request.checkpoint_id, "checkpoint_id"),
        (&request.boundary, "boundary"),
    ] {
        validate_text(value, field)?;
    }
    if request.request.boundary != PRECLINICAL_BOUNDARY
        || request.requested_stage_order.len() != expected.len()
        || request.requested_stage_order != expected
        || request.budget_units == 0
        || request.request.replay_identity != request.replay_identity
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(RetrievalWorkflowError::Invalid(
            "retrieval workflow identity, canonical stages, budget, replay, or boundary is incomplete".into(),
        ));
    }
    if request.replay_identity.as_str().len() != 64 {
        return Err(RetrievalWorkflowError::Invalid(
            "retrieval workflow replay identity digest is invalid".into(),
        ));
    }
    Ok(())
}

fn validate_text(value: &str, field: &str) -> Result<(), RetrievalWorkflowError> {
    if value.trim().is_empty()
        || value != value.trim()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(RetrievalWorkflowError::Invalid(format!(
            "{field} must be bounded, non-empty text without padding or control characters"
        )));
    }
    Ok(())
}

fn validate_unique(values: &[String], field: &str) -> Result<(), RetrievalWorkflowError> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_text(value, field)?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(RetrievalWorkflowError::Invalid(format!(
                "{field} contains duplicate or case-colliding values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_unique(values: &[String], field: &str) -> Result<(), RetrievalWorkflowError> {
    validate_unique(values, field)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(RetrievalWorkflowError::Invalid(format!(
            "{field} is not in canonical order"
        )));
    }
    Ok(())
}

fn receipt_payload(receipt: &RetrievalWorkflowReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "workflow_id": receipt.workflow_id,
        "checkpoint_id": receipt.checkpoint_id,
        "study_id": receipt.study_id,
        "scope": receipt.scope,
        "disposition": receipt.disposition,
        "stage_order": receipt.stage_order,
        "plan_order": receipt.plan_order,
        "completed_order": receipt.completed_order,
        "blocked_order": receipt.blocked_order,
        "compensation_order": receipt.compensation_order,
        "candidate_order": receipt.candidate_order,
        "ranked_order": receipt.ranked_order,
        "qualified_order": receipt.qualified_order,
        "unknown_order": receipt.unknown_order,
        "synthesis_digest": receipt.synthesis_digest,
        "checkpoint_digest": receipt.checkpoint_digest,
        "workflow_digest": receipt.workflow_digest,
        "replay_identity": receipt.replay_identity,
        "budget_units": receipt.budget_units,
        "omissions": receipt.omissions,
        "uncertainty": receipt.uncertainty,
        "negative_evidence": receipt.negative_evidence,
        "effect_receipts": receipt.effect_receipts,
        "raw_data_local": receipt.raw_data_local,
        "boundary": receipt.boundary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retrieval_synthesis::RetrievalCandidate;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn request(state: EvidenceState) -> RetrievalWorkflowRequest {
        RetrievalWorkflowRequest {
            request: ScopedRetrievalQuery {
                request_id: "request:retrieval-workflow".into(),
                study_id: "study:organoid".into(),
                scope: "organoid:neural".into(),
                query: "synaptic density".into(),
                minimum_support_milli: 700,
                candidates: vec![RetrievalCandidate {
                    evidence_id: "evidence:a".into(),
                    source_id: "source:a".into(),
                    study_id: "study:organoid".into(),
                    scope: "organoid:neural".into(),
                    modality: "imaging".into(),
                    support_milli: 900,
                    state,
                    semantic_digest: hash("semantic"),
                    artifact_digest: hash("artifact"),
                    provenance_digest: hash("provenance"),
                    replay_identity: hash("replay"),
                    omissions: Vec::new(),
                    negative_evidence: Vec::new(),
                    raw_data_local: true,
                    boundary: PRECLINICAL_BOUNDARY.into(),
                }],
                replay_identity: hash("replay"),
                policy_allow: true,
                protected_closure: true,
                raw_data_local: true,
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            workflow_id: "workflow:retrieval".into(),
            requested_stage_order: vec![
                "stage:checkpoint".into(),
                "stage:retrieve-local-candidates".into(),
                "stage:synthesize-evidence".into(),
                "stage:validate-output".into(),
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
    fn manifest_is_a1() {
        let manifest = retrieval_workflow_fabric_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A1);
    }
    #[test]
    fn supported_retrieval_is_scheduled() {
        let receipt = compile_retrieval_workflow(&request(EvidenceState::Supported)).unwrap();
        assert_eq!(receipt.disposition, SynthesisDisposition::Qualified);
        assert!(receipt.effect_receipts[0].starts_with("schedule:retrieval-work:"));
    }
    #[test]
    fn unknown_retrieval_compensates() {
        let receipt = compile_retrieval_workflow(&request(EvidenceState::Unknown)).unwrap();
        assert_eq!(receipt.disposition, SynthesisDisposition::Unknown);
        assert!(!receipt.compensation_order.is_empty());
    }
    #[test]
    fn policy_denial_blocks() {
        let mut input = request(EvidenceState::Supported);
        input.policy_allow = false;
        let receipt = compile_retrieval_workflow(&input).unwrap();
        assert_eq!(receipt.disposition, SynthesisDisposition::Blocked);
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }

    #[test]
    fn locality_failure_is_blocked_and_retained() {
        let mut input = request(EvidenceState::Supported);
        input.raw_data_local = false;
        let receipt = compile_retrieval_workflow(&input).unwrap();
        assert_eq!(receipt.disposition, SynthesisDisposition::Blocked);
        assert!(receipt.raw_data_local);
        assert!(receipt
            .omissions
            .iter()
            .any(|value| value == "workflow:raw-data-locality-failed"));
        assert!(receipt.validate().is_ok());
    }

    #[test]
    fn workflow_artifact_payload_is_bound() {
        let mut receipt = compile_retrieval_workflow(&request(EvidenceState::Supported)).unwrap();
        receipt.workflow_id = "workflow:tampered".into();
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn case_mismatched_candidate_identity_is_rejected() {
        let mut receipt = compile_retrieval_workflow(&request(EvidenceState::Supported)).unwrap();
        receipt.ranked_order[0] = receipt.ranked_order[0].to_ascii_uppercase();
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn stage_protocol_is_required() {
        let mut input = request(EvidenceState::Supported);
        input.requested_stage_order.reverse();
        assert!(compile_retrieval_workflow(&input).is_err());
    }
    #[test]
    fn replay_mismatch_is_rejected() {
        let mut input = request(EvidenceState::Supported);
        input.replay_identity = hash("different");
        assert!(compile_retrieval_workflow(&input).is_err());
    }
}
