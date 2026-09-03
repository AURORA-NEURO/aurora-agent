//! Local single-study evidence-surveillance workflow fabric.
//!
//! Atlas feature `AFA-adapter-P01-F13`: resumable local orchestration around
//! `EvidenceFeed1`, with deterministic stages, checkpoint identity, budget
//! admission, compensation, and replayable schedule receipts.

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

use crate::evidence_surveillance::{
    canonical_evidence_feed_request, run_evidence_surveillance, EvidenceFeedRequest,
    EvidenceSurveillanceDisposition,
};

pub const FEATURE_ID: &str = "AFA-adapter-P01-F13";
pub const CONTRACT_VERSION: &str = "adapter-local-evidence-surveillance-workflow-fabric/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceFeed1@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet4@1";
const CANONICAL_STAGES: [&str; 4] = [
    "stage:checkpoint",
    "stage:persist-artifact",
    "stage:surveil-evidence",
    "stage:validate-input",
];
const MAX_TEXT_BYTES: usize = 512;
const MAX_ITEMS: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalEvidenceSurveillanceWorkflowRequest {
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
pub struct LocalEvidenceSurveillanceWorkflowReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub input: LocalEvidenceSurveillanceWorkflowRequest,
    pub input_digest: ContentHash,
    pub request_id: String,
    pub workflow_id: String,
    pub study_id: String,
    pub scope: String,
    pub checkpoint_id: String,
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
pub enum LocalEvidenceSurveillanceWorkflowError {
    #[error("invalid local evidence workflow request: {0}")]
    Invalid(String),
    #[error("local evidence workflow artifact failed: {0}")]
    Artifact(String),
    #[error("local evidence workflow engine failed: {0}")]
    Engine(String),
}

fn validate_text(field: &str, value: &str) -> Result<(), LocalEvidenceSurveillanceWorkflowError> {
    if value.is_empty() || value.trim() != value {
        return Err(LocalEvidenceSurveillanceWorkflowError::Invalid(format!(
            "{field} must be non-empty and trimmed"
        )));
    }
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(LocalEvidenceSurveillanceWorkflowError::Invalid(format!(
            "{field} is outside its bounded text contract"
        )));
    }
    Ok(())
}

fn validate_unique_strings(
    field: &str,
    values: &[String],
) -> Result<(), LocalEvidenceSurveillanceWorkflowError> {
    if values.len() > MAX_ITEMS {
        return Err(LocalEvidenceSurveillanceWorkflowError::Invalid(format!(
            "{field} exceeds its item bound"
        )));
    }
    let mut unique = BTreeSet::new();
    for value in values {
        validate_text(field, value)?;
        if !unique.insert(value) {
            return Err(LocalEvidenceSurveillanceWorkflowError::Invalid(format!(
                "{field} contains duplicate values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_strings(
    field: &str,
    values: &[String],
) -> Result<(), LocalEvidenceSurveillanceWorkflowError> {
    validate_unique_strings(field, values)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(LocalEvidenceSurveillanceWorkflowError::Invalid(format!(
            "{field} ordering is not canonical"
        )));
    }
    Ok(())
}

fn validate_digest(
    field: &str,
    digest: &ContentHash,
) -> Result<(), LocalEvidenceSurveillanceWorkflowError> {
    if digest.as_str().len() != 64
        || !digest
            .as_str()
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(LocalEvidenceSurveillanceWorkflowError::Invalid(format!(
            "{field} must be a 64-character hex digest"
        )));
    }
    Ok(())
}

fn workflow_input_digest(
    request: &LocalEvidenceSurveillanceWorkflowRequest,
) -> Result<ContentHash, LocalEvidenceSurveillanceWorkflowError> {
    let canonical = canonical_local_evidence_surveillance_workflow_request(request);
    let value = serde_json::to_value(canonical)
        .map_err(|error| LocalEvidenceSurveillanceWorkflowError::Artifact(error.to_string()))?;
    ContentHash::of_value(&value)
        .map_err(|error| LocalEvidenceSurveillanceWorkflowError::Artifact(error.to_string()))
}

fn canonical_local_evidence_surveillance_workflow_request(
    request: &LocalEvidenceSurveillanceWorkflowRequest,
) -> LocalEvidenceSurveillanceWorkflowRequest {
    let mut canonical = request.clone();
    canonical.request = canonical_evidence_feed_request(&canonical.request);
    canonical
}

impl LocalEvidenceSurveillanceWorkflowReceipt {
    pub fn validate(&self) -> Result<(), LocalEvidenceSurveillanceWorkflowError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.study_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.checkpoint_id.trim().is_empty()
            || self.stage_order.is_empty()
            || self.plan_order.is_empty()
            || self.effect_receipts.is_empty()
            || self.budget_units == 0
            || self.required_budget == 0
        {
            return Err(LocalEvidenceSurveillanceWorkflowError::Invalid(
                "workflow identity, stages, plan, locality, budget, or effects are incomplete"
                    .into(),
            ));
        }
        validate_text("request_id", &self.request_id)?;
        validate_text("workflow_id", &self.workflow_id)?;
        validate_text("study_id", &self.study_id)?;
        validate_text("scope", &self.scope)?;
        validate_text("checkpoint_id", &self.checkpoint_id)?;
        validate_text("boundary", &self.boundary)?;
        if self.stage_order
            != CANONICAL_STAGES
                .iter()
                .map(|stage| (*stage).to_string())
                .collect::<Vec<_>>()
        {
            return Err(LocalEvidenceSurveillanceWorkflowError::Invalid(
                "workflow stage order is not canonical".into(),
            ));
        }
        validate_sorted_strings("plan_order", &self.plan_order)?;
        validate_sorted_strings("blocked_order", &self.blocked_order)?;
        validate_sorted_strings("compensation_order", &self.compensation_order)?;
        validate_sorted_strings("candidate_order", &self.candidate_order)?;
        validate_sorted_strings("qualified_order", &self.qualified_order)?;
        validate_sorted_strings("unknown_order", &self.unknown_order)?;
        validate_sorted_strings("omissions", &self.omissions)?;
        validate_sorted_strings("uncertainty", &self.uncertainty)?;
        validate_sorted_strings("negative_evidence", &self.negative_evidence)?;
        validate_sorted_strings("effect_receipts", &self.effect_receipts)?;
        let classified = self
            .qualified_order
            .iter()
            .chain(self.unknown_order.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        if classified != self.candidate_order.iter().cloned().collect()
            || self.required_budget != self.plan_order.len() as u32
        {
            return Err(LocalEvidenceSurveillanceWorkflowError::Invalid(
                "workflow evidence states or budget do not partition the plan".into(),
            ));
        }
        let workflow_blocked = !self.blocked_order.is_empty();
        if workflow_blocked && !self.completed_order.is_empty() {
            return Err(LocalEvidenceSurveillanceWorkflowError::Invalid(
                "blocked local workflow cannot report completed stages".into(),
            ));
        }
        if !workflow_blocked && self.completed_order != self.stage_order {
            return Err(LocalEvidenceSurveillanceWorkflowError::Invalid(
                "unblocked local workflow must complete every canonical stage".into(),
            ));
        }
        for digest in [
            &self.evidence_receipt_digest,
            &self.checkpoint_digest,
            &self.workflow_digest,
            &self.replay_identity,
            &self.artifact.content_hash,
        ] {
            validate_digest("workflow receipt digest", digest)?;
        }
        let expected_effect = if self.disposition == EvidenceSurveillanceDisposition::Blocked {
            "block:unsafe-release".to_string()
        } else if !self.compensation_order.is_empty() || !self.blocked_order.is_empty() {
            format!("compensate:research-work:{}", self.workflow_id)
        } else {
            format!("schedule:research-work:{}", self.workflow_id)
        };
        if self.effect_receipts != vec![expected_effect] {
            return Err(LocalEvidenceSurveillanceWorkflowError::Invalid(
                "workflow effect does not match its plan and disposition".into(),
            ));
        }
        let expected_checkpoint = ContentHash::of_value(&json!({
            "workflow_id": self.workflow_id,
            "checkpoint_id": self.checkpoint_id,
            "stage_order": self.stage_order,
            "replay_identity": self.replay_identity,
        }))
        .map_err(|error| LocalEvidenceSurveillanceWorkflowError::Artifact(error.to_string()))?;
        if self.checkpoint_digest != expected_checkpoint {
            return Err(LocalEvidenceSurveillanceWorkflowError::Invalid(
                "workflow checkpoint digest does not match its identity".into(),
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
        .map_err(|error| LocalEvidenceSurveillanceWorkflowError::Artifact(error.to_string()))?;
        if self.workflow_digest != expected_workflow {
            return Err(LocalEvidenceSurveillanceWorkflowError::Invalid(
                "workflow digest does not match plan and budget state".into(),
            ));
        }
        if self.artifact.artifact_id != format!("adapter-evidence-workflow:{}", self.workflow_id)
            || self.artifact.content_type != "application/vnd.aurora.research-workflow-receipt+json"
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(LocalEvidenceSurveillanceWorkflowError::Artifact(
                "local workflow artifact is not bound to its receipt".into(),
            ));
        }
        let payload = json!({
            "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
            "contract_version": CONTRACT_VERSION,
            "feature_id": FEATURE_ID,
            "request_id": self.request_id,
            "workflow_id": self.workflow_id,
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
            "qualified_order": self.qualified_order,
            "unknown_order": self.unknown_order,
            "evidence_receipt_digest": self.evidence_receipt_digest,
            "checkpoint_digest": self.checkpoint_digest,
            "workflow_digest": self.workflow_digest,
            "replay_identity": self.replay_identity,
            "budget_units": self.budget_units,
            "required_budget": self.required_budget,
            "omissions": self.omissions,
            "uncertainty": self.uncertainty,
            "negative_evidence": self.negative_evidence,
            "effect_receipts": self.effect_receipts,
            "boundary": PRECLINICAL_BOUNDARY,
            "raw_data_local": self.raw_data_local,
        });
        self.artifact
            .verify_payload(&payload)
            .map_err(|error| LocalEvidenceSurveillanceWorkflowError::Artifact(error.to_string()))?;
        self.artifact
            .validate_metadata()
            .map_err(|error| LocalEvidenceSurveillanceWorkflowError::Artifact(error.to_string()))?;
        if self.input_digest != workflow_input_digest(&self.input)? {
            return Err(LocalEvidenceSurveillanceWorkflowError::Invalid(
                "workflow retained input digest mismatch".into(),
            ));
        }
        let expected = build_local_evidence_surveillance_workflow(&self.input)?;
        if self != &expected {
            return Err(LocalEvidenceSurveillanceWorkflowError::Invalid(
                "workflow receipt does not match its retained input".into(),
            ));
        }
        Ok(())
    }
}

pub fn local_evidence_surveillance_workflow_fabric_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(), owner_crate: "adapter".into(),
        consumers: ["preclinical researcher".into(), "workflow reliability engineer".into()].into(),
        behavior: "schedule a checkpointed local EvidenceFeed1 workflow with deterministic stages, budget admission, compensation, and replay receipts".into(),
        value: "turn evidence surveillance into a resumable local operator workflow without hiding omissions or executing external effects".into(),
        inputs: vec![TypedPort { name: "evidence_workflow_request".into(), schema: INPUT_SCHEMA.into(), required: true }],
        outputs: vec![TypedPort { name: "qualified_workflow_evidence_set".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(),
        permissions: ["schedule:research-work".into(), "read:local-research-artifacts".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "cwl".into(), state: EvidenceState::Supported, locator: Some("https://www.commonwl.org/specification/".into()) }],
        authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn schedule_local_evidence_surveillance_workflow(
    request: &LocalEvidenceSurveillanceWorkflowRequest,
) -> Result<LocalEvidenceSurveillanceWorkflowReceipt, LocalEvidenceSurveillanceWorkflowError> {
    let receipt = build_local_evidence_surveillance_workflow(request)?;
    receipt.validate()?;
    Ok(receipt)
}

fn build_local_evidence_surveillance_workflow(
    request: &LocalEvidenceSurveillanceWorkflowRequest,
) -> Result<LocalEvidenceSurveillanceWorkflowReceipt, LocalEvidenceSurveillanceWorkflowError> {
    validate_request(request)?;
    let evidence = run_evidence_surveillance(&request.request)
        .map_err(|error| LocalEvidenceSurveillanceWorkflowError::Engine(error.to_string()))?;
    let stage_order = CANONICAL_STAGES
        .iter()
        .map(|stage| (*stage).to_string())
        .collect::<Vec<_>>();
    let mut plan = BTreeSet::new();
    let mut compensation = BTreeSet::new();
    for stage in &stage_order {
        plan.insert(format!("plan:{stage}"));
    }
    if evidence.selected_source_ids.is_empty() {
        plan.insert("plan:retain-unresolved-evidence".into());
        compensation.insert("compensate:research-work:retain-unresolved-evidence".into());
    } else {
        plan.insert("plan:publish-qualified-local-artifact".into());
    }
    if request.budget_units < plan.len() as u32 {
        plan.insert("plan:budget-review".into());
        compensation.insert("compensate:research-work:budget-exhausted".into());
    }
    let required_budget = plan.len() as u32;
    let mut omissions = evidence.omissions.clone();
    let uncertainty = evidence.uncertainty.clone();
    let negative = evidence.qualified_set.negative_source_ids.clone();
    if !request.policy_allow {
        omissions.push("workflow:policy-denied".into());
    }
    if !request.protected_closure {
        omissions.push("workflow:protected-closure-incomplete".into());
    }
    if !request.raw_data_local {
        omissions.push("workflow:raw-data-locality-failed".into());
    }
    let blocked_gate = !request.policy_allow
        || !request.protected_closure
        || !request.raw_data_local
        || request.budget_units < required_budget
        || evidence.disposition == EvidenceSurveillanceDisposition::Blocked;
    let disposition = if blocked_gate {
        EvidenceSurveillanceDisposition::Blocked
    } else {
        evidence.disposition
    };
    let mut blocked = BTreeSet::new();
    if !request.policy_allow
        || !request.protected_closure
        || !request.raw_data_local
        || evidence.disposition == EvidenceSurveillanceDisposition::Blocked
    {
        blocked.insert("stage:release".into());
    }
    if request.budget_units < required_budget {
        blocked.insert("stage:budget".into());
    }
    let candidate_order = request
        .request
        .feed
        .iter()
        .map(|item| item.source_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let qualified_order = evidence
        .selected_source_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let unknown_order = candidate_order
        .iter()
        .filter(|id| !qualified_order.contains(id))
        .cloned()
        .collect::<Vec<_>>();
    let plan_order = plan.into_iter().collect::<Vec<_>>();
    let completed_order = if blocked.is_empty() {
        stage_order.clone()
    } else {
        Vec::new()
    };
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let compensation_order = compensation.into_iter().collect::<Vec<_>>();
    let mut omissions = omissions;
    omissions.sort();
    omissions.dedup();
    let checkpoint_digest = ContentHash::of_value(&json!({
        "workflow_id": request.workflow_id,
        "checkpoint_id": request.checkpoint_id,
        "stage_order": stage_order,
        "replay_identity": request.replay_identity,
    }))
    .map_err(|error| LocalEvidenceSurveillanceWorkflowError::Artifact(error.to_string()))?;
    let evidence_receipt_digest = evidence
        .digest()
        .map_err(|e| LocalEvidenceSurveillanceWorkflowError::Engine(e.to_string()))?;
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
    .map_err(|error| LocalEvidenceSurveillanceWorkflowError::Artifact(error.to_string()))?;
    let effect_receipts = if disposition == EvidenceSurveillanceDisposition::Blocked {
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
        "request_id": request.request.request_id,
        "workflow_id": request.workflow_id,
        "study_id": request.request.study_id,
        "scope": request.request.intent,
        "checkpoint_id": request.checkpoint_id,
        "disposition": disposition,
        "stage_order": stage_order,
        "plan_order": plan_order,
        "completed_order": completed_order,
        "blocked_order": blocked_order,
        "compensation_order": compensation_order,
        "candidate_order": candidate_order,
        "qualified_order": qualified_order,
        "unknown_order": unknown_order,
        "evidence_receipt_digest": evidence_receipt_digest,
        "checkpoint_digest": checkpoint_digest,
        "workflow_digest": workflow_digest,
        "replay_identity": request.replay_identity,
        "budget_units": request.budget_units,
        "required_budget": required_budget,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": negative,
        "effect_receipts": effect_receipts,
        "boundary": PRECLINICAL_BOUNDARY,
        "raw_data_local": request.raw_data_local,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("adapter-evidence-workflow:{}", request.workflow_id),
        "application/vnd.aurora.research-workflow-receipt+json",
        &payload,
        vec![],
        vec![],
    )
    .map_err(|e| LocalEvidenceSurveillanceWorkflowError::Artifact(e.to_string()))?;
    let canonical_request = canonical_local_evidence_surveillance_workflow_request(request);
    let receipt = LocalEvidenceSurveillanceWorkflowReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        input: canonical_request,
        input_digest: workflow_input_digest(request)?,
        request_id: request.request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        study_id: request.request.study_id.clone(),
        scope: request.request.intent.clone(),
        checkpoint_id: request.checkpoint_id.clone(),
        disposition,
        stage_order,
        plan_order,
        completed_order,
        blocked_order,
        compensation_order,
        candidate_order,
        qualified_order,
        unknown_order,
        evidence_receipt_digest,
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
    Ok(receipt)
}

fn validate_request(
    request: &LocalEvidenceSurveillanceWorkflowRequest,
) -> Result<(), LocalEvidenceSurveillanceWorkflowError> {
    if request.workflow_id.trim().is_empty()
        || request.checkpoint_id.trim().is_empty()
        || request.budget_units == 0
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.request.boundary != PRECLINICAL_BOUNDARY
        || request.request.request_id.trim().is_empty()
        || request.request.study_id.trim().is_empty()
        || request.request.intent.trim().is_empty()
        || request.request.feed.is_empty()
        || !request.request.raw_data_local
    {
        return Err(LocalEvidenceSurveillanceWorkflowError::Invalid(
            "workflow identity, feed, checkpoint, budget, or boundary is invalid".into(),
        ));
    }
    let expected = CANONICAL_STAGES
        .iter()
        .map(|stage| (*stage).to_string())
        .collect::<Vec<_>>();
    if request.requested_stage_order != expected {
        return Err(LocalEvidenceSurveillanceWorkflowError::Invalid(
            "requested stage order must match canonical workflow".into(),
        ));
    }
    validate_text("workflow_id", &request.workflow_id)?;
    validate_text("checkpoint_id", &request.checkpoint_id)?;
    validate_text("boundary", &request.boundary)?;
    validate_text("request_id", &request.request.request_id)?;
    validate_text("study_id", &request.request.study_id)?;
    validate_text("intent", &request.request.intent)?;
    if u64::from(request.budget_units) > MAX_ITEMS as u64 {
        return Err(LocalEvidenceSurveillanceWorkflowError::Invalid(
            "workflow budget exceeds its bound".into(),
        ));
    }
    validate_digest("workflow.replay_identity", &request.replay_identity)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence_surveillance::EvidenceFeedItem;
    use bioprism_foundation::PolicyDecision;
    fn request() -> LocalEvidenceSurveillanceWorkflowRequest {
        LocalEvidenceSurveillanceWorkflowRequest {
            request: EvidenceFeedRequest {
                request_id: "f13-test".into(),
                study_id: "study-1".into(),
                intent: "evidence surveillance".into(),
                required_source_ids: vec![],
                feed: vec![EvidenceFeedItem {
                    source_id: "source-1".into(),
                    source_type: "paper".into(),
                    locator: "local://source-1".into(),
                    digest: Some(ContentHash::of_bytes(b"source-1")),
                    availability: bioprism_foundation::EvidenceAvailability::Available,
                    published_at: "2026-01-01".into(),
                    relevance_score: 90,
                    negative_result: false,
                }],
                policy_decision: PolicyDecision::Allow,
                protected_closure_satisfied: true,
                raw_data_local: true,
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            workflow_id: "workflow-1".into(),
            requested_stage_order: CANONICAL_STAGES
                .iter()
                .map(|stage| (*stage).to_string())
                .collect(),
            checkpoint_id: "checkpoint-1".into(),
            budget_units: 8,
            replay_identity: ContentHash::of_bytes(b"replay"),
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            local_evidence_surveillance_workflow_fabric_manifest().autonomy_tier,
            AutonomyTier::A1
        )
    }
    #[test]
    fn schedules_qualified_feed() {
        assert!(schedule_local_evidence_surveillance_workflow(&request())
            .unwrap()
            .effect_receipts[0]
            .starts_with("schedule:"))
    }
    #[test]
    fn policy_blocks() {
        let mut i = request();
        i.policy_allow = false;
        assert_eq!(
            schedule_local_evidence_surveillance_workflow(&i)
                .unwrap()
                .effect_receipts,
            vec!["block:unsafe-release"]
        )
    }
    #[test]
    fn budget_compensates() {
        let mut i = request();
        i.budget_units = 1;
        let r = schedule_local_evidence_surveillance_workflow(&i).unwrap();
        assert!(r.effect_receipts[0].starts_with("block:"));
        assert!(r.completed_order.is_empty());
        assert!(r.blocked_order.contains(&"stage:budget".to_string()));
    }
    #[test]
    fn missing_feed_is_rejected() {
        let mut i = request();
        i.request.feed.clear();
        assert!(schedule_local_evidence_surveillance_workflow(&i).is_err())
    }
    #[test]
    fn replay_stable() {
        let i = request();
        assert_eq!(
            schedule_local_evidence_surveillance_workflow(&i)
                .unwrap()
                .workflow_digest,
            schedule_local_evidence_surveillance_workflow(&i)
                .unwrap()
                .workflow_digest
        )
    }

    #[test]
    fn reordered_nested_feed_has_stable_identity() {
        let mut reordered = request();
        reordered.request.feed.reverse();
        let first = schedule_local_evidence_surveillance_workflow(&request()).unwrap();
        let second = schedule_local_evidence_surveillance_workflow(&reordered).unwrap();
        assert_eq!(first.input_digest, second.input_digest);
        assert_eq!(first.workflow_digest, second.workflow_digest);
    }

    #[test]
    fn tampered_workflow_digest_is_rejected() {
        let mut receipt = schedule_local_evidence_surveillance_workflow(&request()).unwrap();
        receipt.workflow_digest = ContentHash::of_bytes(b"tampered-workflow");
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn policy_block_does_not_report_completed_stages() {
        let mut value = request();
        value.policy_allow = false;
        let receipt = schedule_local_evidence_surveillance_workflow(&value).unwrap();
        assert!(receipt.completed_order.is_empty());
        assert!(receipt.blocked_order.contains(&"stage:release".to_string()));
    }

    #[test]
    fn receipt_rejects_tampered_retained_workflow_request() {
        let mut receipt = schedule_local_evidence_surveillance_workflow(&request()).unwrap();
        receipt.input.checkpoint_id = "tampered-checkpoint".into();
        let error = receipt.validate().unwrap_err();
        assert!(error.to_string().contains("retained input digest mismatch"));
    }
}
