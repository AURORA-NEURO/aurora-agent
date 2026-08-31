//! Prospective high-throughput evidence-surveillance workflow fabric.
//!
//! Atlas feature `AFA-adapter-P01-F15`.  This is a product-level workflow
//! boundary around the EvidenceFeed3 copilot: queue admission, checkpoints,
//! capacity overflow, compensation, policy receipts, and deterministic replay
//! are exposed as a separately consumable capability.

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

use crate::throughput_evidence_surveillance_research_copilot::{
    canonical_throughput_evidence_surveillance_research_copilot_request,
    run_throughput_evidence_surveillance_research_copilot,
    ThroughputEvidenceSurveillanceResearchCopilotRequest, ThroughputResearchCopilotDisposition,
};

pub const FEATURE_ID: &str = "AFA-adapter-P01-F15";
pub const CONTRACT_VERSION: &str = "adapter-throughput-evidence-surveillance-workflow-fabric/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceFeed3@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet4@1";
const CANONICAL_STAGES: [&str; 4] = [
    "stage:checkpoint",
    "stage:admit-capacity",
    "stage:surveil-evidence",
    "stage:publish-receipt",
];
const MAX_TEXT_BYTES: usize = 512;
const MAX_ITEMS: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputEvidenceSurveillanceWorkflowRequest {
    pub request: ThroughputEvidenceSurveillanceResearchCopilotRequest,
    pub workflow_id: String,
    pub requested_stage_order: Vec<String>,
    pub checkpoint_id: String,
    pub budget_units: u32,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputEvidenceSurveillanceWorkflowReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub input: ThroughputEvidenceSurveillanceWorkflowRequest,
    pub input_digest: ContentHash,
    pub request_id: String,
    pub workflow_id: String,
    pub agent_id: String,
    pub batch_id: String,
    pub checkpoint_id: String,
    pub checkpoint_seq: u64,
    pub budget_units: u32,
    pub required_budget: u32,
    pub disposition: ThroughputResearchCopilotDisposition,
    pub stage_order: Vec<String>,
    pub plan_order: Vec<String>,
    pub completed_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub compensation_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub denied_order: Vec<String>,
    pub overflow_order: Vec<String>,
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
pub enum ThroughputEvidenceSurveillanceWorkflowError {
    #[error("invalid throughput evidence workflow request: {0}")]
    Invalid(String),
    #[error("throughput evidence workflow artifact failed: {0}")]
    Artifact(String),
    #[error("throughput evidence workflow copilot failed: {0}")]
    Copilot(String),
}

fn validate_text(
    field: &str,
    value: &str,
) -> Result<(), ThroughputEvidenceSurveillanceWorkflowError> {
    if value.is_empty() || value.trim() != value {
        return Err(ThroughputEvidenceSurveillanceWorkflowError::Invalid(
            format!("{field} must be non-empty and trimmed"),
        ));
    }
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(ThroughputEvidenceSurveillanceWorkflowError::Invalid(
            format!("{field} is outside its bounded text contract"),
        ));
    }
    Ok(())
}

fn validate_unique_strings(
    field: &str,
    values: &[String],
) -> Result<(), ThroughputEvidenceSurveillanceWorkflowError> {
    if values.len() > MAX_ITEMS {
        return Err(ThroughputEvidenceSurveillanceWorkflowError::Invalid(
            format!("{field} exceeds its item bound"),
        ));
    }
    let mut unique = BTreeSet::new();
    for value in values {
        validate_text(field, value)?;
        if !unique.insert(value) {
            return Err(ThroughputEvidenceSurveillanceWorkflowError::Invalid(
                format!("{field} contains duplicate values"),
            ));
        }
    }
    Ok(())
}

fn validate_sorted_strings(
    field: &str,
    values: &[String],
) -> Result<(), ThroughputEvidenceSurveillanceWorkflowError> {
    validate_unique_strings(field, values)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ThroughputEvidenceSurveillanceWorkflowError::Invalid(
            format!("{field} ordering is not canonical"),
        ));
    }
    Ok(())
}

fn validate_digest(
    field: &str,
    digest: &ContentHash,
) -> Result<(), ThroughputEvidenceSurveillanceWorkflowError> {
    if digest.as_str().len() != 64
        || !digest
            .as_str()
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(ThroughputEvidenceSurveillanceWorkflowError::Invalid(
            format!("{field} must be a 64-character hex digest"),
        ));
    }
    Ok(())
}

fn workflow_input_digest(
    request: &ThroughputEvidenceSurveillanceWorkflowRequest,
) -> Result<ContentHash, ThroughputEvidenceSurveillanceWorkflowError> {
    let canonical = canonical_throughput_evidence_surveillance_workflow_request(request);
    let value = serde_json::to_value(canonical).map_err(|error| {
        ThroughputEvidenceSurveillanceWorkflowError::Artifact(error.to_string())
    })?;
    ContentHash::of_value(&value)
        .map_err(|error| ThroughputEvidenceSurveillanceWorkflowError::Artifact(error.to_string()))
}

fn canonical_throughput_evidence_surveillance_workflow_request(
    request: &ThroughputEvidenceSurveillanceWorkflowRequest,
) -> ThroughputEvidenceSurveillanceWorkflowRequest {
    let mut canonical = request.clone();
    canonical.request =
        canonical_throughput_evidence_surveillance_research_copilot_request(&canonical.request);
    canonical
}

impl ThroughputEvidenceSurveillanceWorkflowReceipt {
    pub fn validate(&self) -> Result<(), ThroughputEvidenceSurveillanceWorkflowError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.agent_id.trim().is_empty()
            || self.batch_id.trim().is_empty()
            || self.checkpoint_id.trim().is_empty()
            || self.checkpoint_seq == 0
            || self.budget_units == 0
            || self.required_budget == 0
            || self.stage_order.is_empty()
            || self.plan_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(ThroughputEvidenceSurveillanceWorkflowError::Invalid(
                "workflow identity, queue checkpoint, locality, stages, or effects are incomplete"
                    .into(),
            ));
        }
        validate_text("request_id", &self.request_id)?;
        validate_text("workflow_id", &self.workflow_id)?;
        validate_text("agent_id", &self.agent_id)?;
        validate_text("batch_id", &self.batch_id)?;
        validate_text("checkpoint_id", &self.checkpoint_id)?;
        validate_text("boundary", &self.boundary)?;
        if self.stage_order
            != CANONICAL_STAGES
                .iter()
                .map(|stage| (*stage).to_string())
                .collect::<Vec<_>>()
        {
            return Err(ThroughputEvidenceSurveillanceWorkflowError::Invalid(
                "workflow stage order is not canonical".into(),
            ));
        }
        validate_sorted_strings("plan_order", &self.plan_order)?;
        validate_sorted_strings("blocked_order", &self.blocked_order)?;
        validate_sorted_strings("compensation_order", &self.compensation_order)?;
        validate_sorted_strings("candidate_order", &self.candidate_order)?;
        validate_sorted_strings("selected_order", &self.selected_order)?;
        validate_sorted_strings("unresolved_order", &self.unresolved_order)?;
        validate_sorted_strings("denied_order", &self.denied_order)?;
        validate_sorted_strings("overflow_order", &self.overflow_order)?;
        validate_sorted_strings("omissions", &self.omissions)?;
        validate_sorted_strings("uncertainty", &self.uncertainty)?;
        validate_sorted_strings("negative_evidence", &self.negative_evidence)?;
        validate_sorted_strings("effect_receipts", &self.effect_receipts)?;
        if self.required_budget != self.plan_order.len() as u32 {
            return Err(ThroughputEvidenceSurveillanceWorkflowError::Invalid(
                "workflow required budget does not match its plan".into(),
            ));
        }
        let workflow_blocked = !self.blocked_order.is_empty();
        if workflow_blocked && !self.completed_order.is_empty() {
            return Err(ThroughputEvidenceSurveillanceWorkflowError::Invalid(
                "blocked workflow cannot report completed stages".into(),
            ));
        }
        if !workflow_blocked && self.completed_order != self.stage_order {
            return Err(ThroughputEvidenceSurveillanceWorkflowError::Invalid(
                "unblocked workflow must complete every canonical stage".into(),
            ));
        }
        for value in [
            &self.replay_identity,
            &self.copilot_run_digest,
            &self.checkpoint_digest,
            &self.workflow_digest,
            &self.artifact.content_hash,
        ] {
            validate_digest("workflow receipt digest", value)?;
        }
        let expected_effect = if self.disposition == ThroughputResearchCopilotDisposition::Blocked {
            "block:unsafe-release".to_string()
        } else if !self.compensation_order.is_empty() || !self.blocked_order.is_empty() {
            format!("compensate:research-work:{}", self.workflow_id)
        } else {
            format!("schedule:research-work:{}", self.workflow_id)
        };
        if self.effect_receipts != vec![expected_effect] {
            return Err(ThroughputEvidenceSurveillanceWorkflowError::Invalid(
                "workflow effect does not match its plan and disposition".into(),
            ));
        }
        let expected_checkpoint = ContentHash::of_value(&json!({
            "workflow_id": self.workflow_id,
            "checkpoint_id": self.checkpoint_id,
            "checkpoint_seq": self.checkpoint_seq,
            "stage_order": self.stage_order,
            "replay_identity": self.replay_identity,
        }))
        .map_err(|error| {
            ThroughputEvidenceSurveillanceWorkflowError::Artifact(error.to_string())
        })?;
        if self.checkpoint_digest != expected_checkpoint {
            return Err(ThroughputEvidenceSurveillanceWorkflowError::Invalid(
                "workflow checkpoint digest does not match its queue identity".into(),
            ));
        }
        let expected_workflow = ContentHash::of_value(&json!({
            "workflow_id": self.workflow_id,
            "plan_order": self.plan_order,
            "completed_order": self.completed_order,
            "blocked_order": self.blocked_order,
            "compensation_order": self.compensation_order,
            "checkpoint_digest": self.checkpoint_digest,
            "copilot_run_digest": self.copilot_run_digest,
            "budget_units": self.budget_units,
            "required_budget": self.required_budget,
        }))
        .map_err(|error| {
            ThroughputEvidenceSurveillanceWorkflowError::Artifact(error.to_string())
        })?;
        if self.workflow_digest != expected_workflow {
            return Err(ThroughputEvidenceSurveillanceWorkflowError::Invalid(
                "workflow digest does not match its plan and budget state".into(),
            ));
        }
        if self.artifact.artifact_id
            != format!("adapter-throughput-evidence-workflow:{}", self.workflow_id)
            || self.artifact.content_type
                != "application/vnd.aurora.throughput-research-workflow+json"
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(ThroughputEvidenceSurveillanceWorkflowError::Artifact(
                "workflow artifact is not bound to its receipt".into(),
            ));
        }
        let payload = json!({
            "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
            "contract_version": CONTRACT_VERSION,
            "feature_id": FEATURE_ID,
            "request_id": self.request_id,
            "workflow_id": self.workflow_id,
            "agent_id": self.agent_id,
            "batch_id": self.batch_id,
            "checkpoint_id": self.checkpoint_id,
            "checkpoint_seq": self.checkpoint_seq,
            "budget_units": self.budget_units,
            "required_budget": self.required_budget,
            "disposition": self.disposition,
            "stage_order": self.stage_order,
            "plan_order": self.plan_order,
            "completed_order": self.completed_order,
            "blocked_order": self.blocked_order,
            "compensation_order": self.compensation_order,
            "candidate_order": self.candidate_order,
            "selected_order": self.selected_order,
            "unresolved_order": self.unresolved_order,
            "denied_order": self.denied_order,
            "overflow_order": self.overflow_order,
            "replay_identity": self.replay_identity,
            "copilot_run_digest": self.copilot_run_digest,
            "checkpoint_digest": self.checkpoint_digest,
            "workflow_digest": self.workflow_digest,
            "omissions": self.omissions,
            "uncertainty": self.uncertainty,
            "negative_evidence": self.negative_evidence,
            "effect_receipts": self.effect_receipts,
            "boundary": PRECLINICAL_BOUNDARY,
            "raw_data_local": self.raw_data_local,
        });
        self.artifact.verify_payload(&payload).map_err(|error| {
            ThroughputEvidenceSurveillanceWorkflowError::Artifact(error.to_string())
        })?;
        self.artifact.validate_metadata().map_err(|error| {
            ThroughputEvidenceSurveillanceWorkflowError::Artifact(error.to_string())
        })?;
        if self.input_digest != workflow_input_digest(&self.input)? {
            return Err(ThroughputEvidenceSurveillanceWorkflowError::Invalid(
                "throughput workflow retained input digest mismatch".into(),
            ));
        }
        let expected = build_throughput_evidence_surveillance_workflow(&self.input)?;
        if self != &expected {
            return Err(ThroughputEvidenceSurveillanceWorkflowError::Invalid(
                "throughput workflow receipt does not match its retained input".into(),
            ));
        }
        Ok(())
    }
}

pub fn throughput_evidence_surveillance_workflow_fabric_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "adapter".into(),
        consumers: ["integration engineer".into(), "queue operator".into()].into(),
        behavior: "orchestrates a prospective high-throughput EvidenceFeed3 workflow with checkpointed capacity admission, overflow compensation, policy gates, and replay receipts".into(),
        value: "turns high-throughput evidence alerts into a separately deployable, fail-closed workflow contract without hiding capacity overflow, missing evidence, or negative results".into(),
        inputs: vec![TypedPort { name: "throughput_evidence_workflow_request".into(), schema: INPUT_SCHEMA.into(), required: true }],
        outputs: vec![TypedPort { name: "qualified_throughput_evidence_set".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(),
        permissions: ["schedule:research-work".into(), "execute:approved-workflows".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "cwl".into(), state: EvidenceState::Supported, locator: Some("https://www.commonwl.org/specification/".into()) }],
        authority_requirements: vec![AuthorityRequirement { role: "throughput evidence workflow approver".into(), reason: "approve declared queue and tool effects only after capacity, protected closure, locality, and replay gates close".into() }],
        autonomy_tier: AutonomyTier::A2,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn schedule_throughput_evidence_surveillance_workflow(
    request: &ThroughputEvidenceSurveillanceWorkflowRequest,
) -> Result<
    ThroughputEvidenceSurveillanceWorkflowReceipt,
    ThroughputEvidenceSurveillanceWorkflowError,
> {
    let receipt = build_throughput_evidence_surveillance_workflow(request)?;
    receipt.validate()?;
    Ok(receipt)
}

fn build_throughput_evidence_surveillance_workflow(
    request: &ThroughputEvidenceSurveillanceWorkflowRequest,
) -> Result<
    ThroughputEvidenceSurveillanceWorkflowReceipt,
    ThroughputEvidenceSurveillanceWorkflowError,
> {
    validate_request(request)?;
    let copilot = run_throughput_evidence_surveillance_research_copilot(&request.request)
        .map_err(|error| ThroughputEvidenceSurveillanceWorkflowError::Copilot(error.to_string()))?;
    let stage_order = CANONICAL_STAGES
        .iter()
        .map(|stage| (*stage).to_string())
        .collect::<Vec<_>>();
    let mut plan = BTreeSet::new();
    let mut compensation = BTreeSet::new();
    for stage in &stage_order {
        plan.insert(format!("plan:{stage}"));
    }
    if copilot.selected_order.is_empty() {
        plan.insert("plan:retain-unresolved-throughput-evidence".into());
        compensation.insert("compensate:research-work:retain-unresolved-evidence".into());
    } else {
        plan.insert("plan:publish-qualified-throughput-artifact".into());
    }
    let required_budget = plan.len() as u32;
    let budget_exhausted = request.budget_units < required_budget;
    if budget_exhausted {
        compensation.insert("compensate:research-work:budget-exhausted".into());
    }
    let disposition = copilot.disposition;
    let mut blocked = BTreeSet::new();
    if disposition == ThroughputResearchCopilotDisposition::Blocked {
        blocked.insert("stage:release".into());
    }
    if budget_exhausted {
        blocked.insert("stage:budget".into());
    }
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let completed_order = if blocked_order.is_empty() {
        stage_order.clone()
    } else {
        Vec::new()
    };
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
    let plan_order = plan.into_iter().collect::<Vec<_>>();
    let compensation_order = compensation.into_iter().collect::<Vec<_>>();
    let checkpoint_digest = ContentHash::of_value(&json!({
        "workflow_id": request.workflow_id,
        "checkpoint_id": request.checkpoint_id,
        "checkpoint_seq": request.request.checkpoint_seq,
        "stage_order": stage_order,
        "replay_identity": request.replay_identity,
    }))
    .map_err(|error| ThroughputEvidenceSurveillanceWorkflowError::Artifact(error.to_string()))?;
    let copilot_run_digest = copilot.run_digest.clone();
    let workflow_digest = ContentHash::of_value(&json!({
        "workflow_id": request.workflow_id,
        "plan_order": plan_order,
        "completed_order": completed_order,
        "blocked_order": blocked_order,
        "compensation_order": compensation_order,
        "checkpoint_digest": checkpoint_digest,
        "copilot_run_digest": copilot_run_digest,
        "budget_units": request.budget_units,
        "required_budget": required_budget,
    }))
    .map_err(|error| ThroughputEvidenceSurveillanceWorkflowError::Artifact(error.to_string()))?;
    let effect_receipts = if disposition == ThroughputResearchCopilotDisposition::Blocked {
        vec!["block:unsafe-release".into()]
    } else if !compensation_order.is_empty() || !blocked_order.is_empty() {
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
        "agent_id": request.request.agent_id,
        "batch_id": request.request.batch_id,
        "checkpoint_id": request.checkpoint_id,
        "checkpoint_seq": request.request.checkpoint_seq,
        "budget_units": request.budget_units,
        "required_budget": required_budget,
        "disposition": disposition,
        "stage_order": stage_order,
        "plan_order": plan_order,
        "completed_order": completed_order,
        "blocked_order": blocked_order,
        "compensation_order": compensation_order,
        "candidate_order": copilot.candidate_order,
        "selected_order": copilot.selected_order,
        "unresolved_order": copilot.unresolved_order,
        "denied_order": copilot.denied_order,
        "overflow_order": copilot.overflow_order,
        "replay_identity": request.replay_identity,
        "copilot_run_digest": copilot_run_digest,
        "checkpoint_digest": checkpoint_digest,
        "workflow_digest": workflow_digest,
        "omissions": omissions,
        "uncertainty": copilot.uncertainty,
        "negative_evidence": copilot.negative_evidence,
        "effect_receipts": effect_receipts,
        "boundary": PRECLINICAL_BOUNDARY,
        "raw_data_local": request.request.raw_data_local,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "adapter-throughput-evidence-workflow:{}",
            request.workflow_id
        ),
        "application/vnd.aurora.throughput-research-workflow+json",
        &payload,
        vec![],
        vec![],
    )
    .map_err(|error| ThroughputEvidenceSurveillanceWorkflowError::Artifact(error.to_string()))?;
    let canonical_request = canonical_throughput_evidence_surveillance_workflow_request(request);
    let receipt = ThroughputEvidenceSurveillanceWorkflowReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        input: canonical_request,
        input_digest: workflow_input_digest(request)?,
        request_id: request.request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        agent_id: request.request.agent_id.clone(),
        batch_id: request.request.batch_id.clone(),
        checkpoint_id: request.checkpoint_id.clone(),
        checkpoint_seq: request.request.checkpoint_seq,
        budget_units: request.budget_units,
        required_budget,
        disposition,
        stage_order,
        plan_order,
        completed_order,
        blocked_order,
        compensation_order,
        candidate_order: copilot.candidate_order.clone(),
        selected_order: copilot.selected_order.clone(),
        unresolved_order: copilot.unresolved_order.clone(),
        denied_order: copilot.denied_order.clone(),
        overflow_order: copilot.overflow_order.clone(),
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
    Ok(receipt)
}

fn validate_request(
    request: &ThroughputEvidenceSurveillanceWorkflowRequest,
) -> Result<(), ThroughputEvidenceSurveillanceWorkflowError> {
    if request.budget_units == 0
        || u64::from(request.budget_units) > MAX_ITEMS as u64
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.request.boundary != PRECLINICAL_BOUNDARY
        || !request.request.raw_data_local
    {
        return Err(ThroughputEvidenceSurveillanceWorkflowError::Invalid(
            "workflow identity, checkpoint, budget, locality, or preclinical boundary is invalid"
                .into(),
        ));
    }
    validate_text("workflow_id", &request.workflow_id)?;
    validate_text("checkpoint_id", &request.checkpoint_id)?;
    validate_text("boundary", &request.boundary)?;
    let expected = CANONICAL_STAGES
        .iter()
        .map(|stage| (*stage).to_string())
        .collect::<Vec<_>>();
    if request.requested_stage_order != expected {
        return Err(ThroughputEvidenceSurveillanceWorkflowError::Invalid(
            "workflow stage order is not canonical".into(),
        ));
    }
    validate_digest("workflow.replay_identity", &request.replay_identity)?;
    validate_digest("copilot.replay_identity", &request.request.replay_identity)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::throughput_evidence_surveillance_research_copilot::ThroughputCopilotEvidenceObservation;
    use bioprism_foundation::EvidenceAvailability;

    fn request() -> ThroughputEvidenceSurveillanceWorkflowRequest {
        ThroughputEvidenceSurveillanceWorkflowRequest {
            request: ThroughputEvidenceSurveillanceResearchCopilotRequest {
                request_id: "req-15".into(),
                agent_id: "agent-15".into(),
                batch_id: "batch-15".into(),
                checkpoint_seq: 1,
                capacity: 2,
                declared_tools: vec!["evidence.query".into()],
                requested_tool: "evidence.query".into(),
                max_tool_calls: 2,
                dry_run: true,
                approval_reference: None,
                approval_granted: false,
                observations: vec![ThroughputCopilotEvidenceObservation {
                    source_id: "s1".into(),
                    sequence: 1,
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
                replay_identity: ContentHash::of_bytes(b"copilot-15"),
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            workflow_id: "workflow-15".into(),
            requested_stage_order: CANONICAL_STAGES
                .iter()
                .map(|stage| (*stage).to_string())
                .collect(),
            checkpoint_id: "checkpoint-15".into(),
            budget_units: 8,
            replay_identity: ContentHash::of_bytes(b"workflow-15"),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a2() {
        assert_eq!(
            throughput_evidence_surveillance_workflow_fabric_manifest().autonomy_tier,
            AutonomyTier::A2
        );
        assert!(throughput_evidence_surveillance_workflow_fabric_manifest()
            .validate()
            .is_ok());
    }
    #[test]
    fn schedules_queue_receipt() {
        let receipt = schedule_throughput_evidence_surveillance_workflow(&request()).unwrap();
        assert_eq!(receipt.feature_id, FEATURE_ID);
        assert_eq!(receipt.checkpoint_seq, 1);
    }
    #[test]
    fn rejects_stage_reordering() {
        let mut r = request();
        r.requested_stage_order.reverse();
        assert!(schedule_throughput_evidence_surveillance_workflow(&r).is_err());
    }
    #[test]
    fn captures_capacity_compensation() {
        let mut r = request();
        r.request.capacity = 0;
        assert!(schedule_throughput_evidence_surveillance_workflow(&r).is_err());
    }
    #[test]
    fn approval_blocks_effect() {
        let mut r = request();
        r.request.dry_run = false;
        r.request.approval_granted = false;
        let receipt = schedule_throughput_evidence_surveillance_workflow(&r).unwrap();
        assert_eq!(
            receipt.disposition,
            ThroughputResearchCopilotDisposition::Blocked
        );
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn replay_is_stable() {
        let r = request();
        assert_eq!(
            schedule_throughput_evidence_surveillance_workflow(&r).unwrap(),
            schedule_throughput_evidence_surveillance_workflow(&r).unwrap()
        );
    }

    #[test]
    fn reordered_nested_copilot_input_has_stable_identity() {
        let mut reordered = request();
        reordered.request.declared_tools.reverse();
        reordered.request.observations.reverse();
        let first = schedule_throughput_evidence_surveillance_workflow(&request()).unwrap();
        let second = schedule_throughput_evidence_surveillance_workflow(&reordered).unwrap();
        assert_eq!(first.input_digest, second.input_digest);
        assert_eq!(first.workflow_digest, second.workflow_digest);
    }

    #[test]
    fn budget_exhaustion_blocks_stage_completion_and_scheduling() {
        let mut r = request();
        r.budget_units = 1;
        let receipt = schedule_throughput_evidence_surveillance_workflow(&r).unwrap();
        assert!(receipt.completed_order.is_empty());
        assert!(receipt.blocked_order.contains(&"stage:budget".to_string()));
        assert_eq!(
            receipt.effect_receipts,
            vec!["compensate:research-work:workflow-15"]
        );
    }

    #[test]
    fn tampered_workflow_digest_is_rejected() {
        let mut receipt = schedule_throughput_evidence_surveillance_workflow(&request()).unwrap();
        receipt.workflow_digest = ContentHash::of_bytes(b"tampered-workflow");
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn blocked_workflow_does_not_report_completed_stages() {
        let mut r = request();
        r.request.dry_run = false;
        r.request.approval_granted = false;
        let receipt = schedule_throughput_evidence_surveillance_workflow(&r).unwrap();
        assert!(receipt.completed_order.is_empty());
        assert!(receipt.blocked_order.contains(&"stage:release".to_string()));
    }

    #[test]
    fn receipt_rejects_tampered_retained_workflow_request() {
        let mut receipt = schedule_throughput_evidence_surveillance_workflow(&request()).unwrap();
        receipt.input.checkpoint_id = "tampered-checkpoint".into();
        let error = receipt.validate().unwrap_err();
        assert!(error.to_string().contains("retained input digest mismatch"));
    }
}
