//! Federated continual evidence-surveillance workflow fabric.
//!
//! Atlas feature `AFA-adapter-P01-F16`.  This workflow wraps the federated
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
    canonical_federated_continual_evidence_surveillance_research_copilot_request,
    run_federated_continual_evidence_surveillance_research_copilot,
    FederatedContinualEvidenceSurveillanceResearchCopilotRequest,
    FederatedContinualResearchCopilotDisposition,
};

pub const FEATURE_ID: &str = "AFA-adapter-P01-F16";
pub const CONTRACT_VERSION: &str =
    "adapter-federated-continual-evidence-surveillance-workflow-fabric/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceFeed4@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet4@1";
const CANONICAL_STAGES: [&str; 4] = [
    "stage:checkpoint",
    "stage:admit-federation",
    "stage:surveil-evidence",
    "stage:seal-envelope",
];
const MAX_TEXT_BYTES: usize = 512;
const MAX_ITEMS: usize = 16_384;

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
    pub input: FederatedContinualEvidenceSurveillanceWorkflowRequest,
    pub input_digest: ContentHash,
    pub request_id: String,
    pub workflow_id: String,
    pub agent_id: String,
    pub federation_id: String,
    pub purpose: String,
    pub endpoint: String,
    pub semantic_profile: String,
    pub allowed_artifacts: Vec<String>,
    pub min_peer_quorum: usize,
    pub checkpoint_id: String,
    pub budget_units: u32,
    pub required_budget: u32,
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

fn validate_text(
    field: &str,
    value: &str,
) -> Result<(), FederatedContinualEvidenceSurveillanceWorkflowError> {
    if value.is_empty() || value.trim() != value {
        return Err(
            FederatedContinualEvidenceSurveillanceWorkflowError::Invalid(format!(
                "{field} must be non-empty and trimmed"
            )),
        );
    }
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(
            FederatedContinualEvidenceSurveillanceWorkflowError::Invalid(format!(
                "{field} is outside its bounded text contract"
            )),
        );
    }
    Ok(())
}

fn validate_unique_strings(
    field: &str,
    values: &[String],
) -> Result<(), FederatedContinualEvidenceSurveillanceWorkflowError> {
    if values.len() > MAX_ITEMS {
        return Err(
            FederatedContinualEvidenceSurveillanceWorkflowError::Invalid(format!(
                "{field} exceeds its item bound"
            )),
        );
    }
    let mut unique = BTreeSet::new();
    for value in values {
        validate_text(field, value)?;
        if !unique.insert(value) {
            return Err(
                FederatedContinualEvidenceSurveillanceWorkflowError::Invalid(format!(
                    "{field} contains duplicate values"
                )),
            );
        }
    }
    Ok(())
}

fn validate_sorted_strings(
    field: &str,
    values: &[String],
) -> Result<(), FederatedContinualEvidenceSurveillanceWorkflowError> {
    validate_unique_strings(field, values)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(
            FederatedContinualEvidenceSurveillanceWorkflowError::Invalid(format!(
                "{field} ordering is not canonical"
            )),
        );
    }
    Ok(())
}

fn validate_digest(
    field: &str,
    digest: &ContentHash,
) -> Result<(), FederatedContinualEvidenceSurveillanceWorkflowError> {
    if digest.as_str().len() != 64
        || !digest
            .as_str()
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(
            FederatedContinualEvidenceSurveillanceWorkflowError::Invalid(format!(
                "{field} must be a 64-character hex digest"
            )),
        );
    }
    Ok(())
}

fn federated_workflow_input_digest(
    request: &FederatedContinualEvidenceSurveillanceWorkflowRequest,
) -> Result<ContentHash, FederatedContinualEvidenceSurveillanceWorkflowError> {
    let canonical = canonical_federated_continual_evidence_surveillance_workflow_request(request);
    let value = serde_json::to_value(canonical).map_err(|error| {
        FederatedContinualEvidenceSurveillanceWorkflowError::Artifact(error.to_string())
    })?;
    ContentHash::of_value(&value).map_err(|error| {
        FederatedContinualEvidenceSurveillanceWorkflowError::Artifact(error.to_string())
    })
}

fn canonical_federated_continual_evidence_surveillance_workflow_request(
    request: &FederatedContinualEvidenceSurveillanceWorkflowRequest,
) -> FederatedContinualEvidenceSurveillanceWorkflowRequest {
    let mut canonical = request.clone();
    canonical.request =
        canonical_federated_continual_evidence_surveillance_research_copilot_request(
            &canonical.request,
        );
    canonical
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
            || self.semantic_profile.trim().is_empty()
            || self.allowed_artifacts.is_empty()
            || self.min_peer_quorum == 0
            || self.checkpoint_id.trim().is_empty()
            || self.budget_units == 0
            || self.required_budget == 0
            || self.stage_order.is_empty()
            || self.plan_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(
                FederatedContinualEvidenceSurveillanceWorkflowError::Invalid(
                    "federation identity, workflow stages, locality, or effects are incomplete"
                        .into(),
                ),
            );
        }
        validate_text("request_id", &self.request_id)?;
        validate_text("workflow_id", &self.workflow_id)?;
        validate_text("agent_id", &self.agent_id)?;
        validate_text("federation_id", &self.federation_id)?;
        validate_text("purpose", &self.purpose)?;
        validate_text("endpoint", &self.endpoint)?;
        validate_text("semantic_profile", &self.semantic_profile)?;
        validate_text("checkpoint_id", &self.checkpoint_id)?;
        validate_text("boundary", &self.boundary)?;
        validate_sorted_strings("allowed_artifacts", &self.allowed_artifacts)?;
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
        validate_sorted_strings("plan_order", &self.plan_order)?;
        validate_sorted_strings("blocked_order", &self.blocked_order)?;
        validate_sorted_strings("compensation_order", &self.compensation_order)?;
        validate_sorted_strings("peer_order", &self.peer_order)?;
        validate_sorted_strings("candidate_order", &self.candidate_order)?;
        validate_sorted_strings("selected_order", &self.selected_order)?;
        validate_sorted_strings("unresolved_order", &self.unresolved_order)?;
        validate_sorted_strings("denied_order", &self.denied_order)?;
        validate_sorted_strings("aggregate_order", &self.aggregate_order)?;
        validate_sorted_strings("omissions", &self.omissions)?;
        validate_sorted_strings("uncertainty", &self.uncertainty)?;
        validate_sorted_strings("negative_evidence", &self.negative_evidence)?;
        validate_sorted_strings("effect_receipts", &self.effect_receipts)?;
        if self.required_budget != self.plan_order.len() as u32
            || self.aggregate_order != self.selected_order
        {
            return Err(
                FederatedContinualEvidenceSurveillanceWorkflowError::Invalid(
                    "federated workflow budget or aggregate partition is inconsistent".into(),
                ),
            );
        }
        let classified = self
            .selected_order
            .iter()
            .chain(self.unresolved_order.iter())
            .chain(self.denied_order.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        if classified != self.candidate_order.iter().cloned().collect() {
            return Err(
                FederatedContinualEvidenceSurveillanceWorkflowError::Invalid(
                    "federated workflow states do not partition candidates".into(),
                ),
            );
        }
        let workflow_blocked = !self.blocked_order.is_empty();
        if workflow_blocked && !self.completed_order.is_empty() {
            return Err(
                FederatedContinualEvidenceSurveillanceWorkflowError::Invalid(
                    "blocked federated workflow cannot report completed stages".into(),
                ),
            );
        }
        if !workflow_blocked && self.completed_order != self.stage_order {
            return Err(
                FederatedContinualEvidenceSurveillanceWorkflowError::Invalid(
                    "unblocked federated workflow must complete every canonical stage".into(),
                ),
            );
        }
        for value in [
            &self.replay_identity,
            &self.copilot_run_digest,
            &self.checkpoint_digest,
            &self.federation_envelope_digest,
            &self.workflow_digest,
            &self.artifact.content_hash,
        ] {
            validate_digest("federated workflow receipt digest", value)?;
        }
        let expected_effect =
            if self.disposition == FederatedContinualResearchCopilotDisposition::Blocked {
                "block:unsafe-release".to_string()
            } else if !self.compensation_order.is_empty() || !self.blocked_order.is_empty() {
                format!("compensate:research-work:{}", self.workflow_id)
            } else {
                format!("schedule:research-work:{}", self.workflow_id)
            };
        if self.effect_receipts != vec![expected_effect] {
            return Err(
                FederatedContinualEvidenceSurveillanceWorkflowError::Invalid(
                    "federated workflow effect does not match its plan and disposition".into(),
                ),
            );
        }
        let expected_checkpoint = ContentHash::of_value(&json!({
            "workflow_id": self.workflow_id,
            "checkpoint_id": self.checkpoint_id,
            "stage_order": self.stage_order,
            "replay_identity": self.replay_identity,
        }))
        .map_err(|error| {
            FederatedContinualEvidenceSurveillanceWorkflowError::Artifact(error.to_string())
        })?;
        if self.checkpoint_digest != expected_checkpoint {
            return Err(
                FederatedContinualEvidenceSurveillanceWorkflowError::Invalid(
                    "federated workflow checkpoint digest does not match identity".into(),
                ),
            );
        }
        let expected_envelope = ContentHash::of_value(&json!({
            "federation_id": self.federation_id,
            "purpose": self.purpose,
            "endpoint": self.endpoint,
            "peer_order": self.peer_order,
            "aggregate_order": self.aggregate_order,
            "raw_data_local": self.raw_data_local,
            "allowed_artifacts": self.allowed_artifacts,
        }))
        .map_err(|error| {
            FederatedContinualEvidenceSurveillanceWorkflowError::Artifact(error.to_string())
        })?;
        if self.federation_envelope_digest != expected_envelope {
            return Err(
                FederatedContinualEvidenceSurveillanceWorkflowError::Invalid(
                    "federated envelope digest does not match aggregate-only scope".into(),
                ),
            );
        }
        let expected_workflow = ContentHash::of_value(&json!({
            "workflow_id": self.workflow_id,
            "plan_order": self.plan_order,
            "completed_order": self.completed_order,
            "blocked_order": self.blocked_order,
            "compensation_order": self.compensation_order,
            "checkpoint_digest": self.checkpoint_digest,
            "federation_envelope_digest": self.federation_envelope_digest,
            "copilot_run_digest": self.copilot_run_digest,
            "budget_units": self.budget_units,
            "required_budget": self.required_budget,
        }))
        .map_err(|error| {
            FederatedContinualEvidenceSurveillanceWorkflowError::Artifact(error.to_string())
        })?;
        if self.workflow_digest != expected_workflow {
            return Err(
                FederatedContinualEvidenceSurveillanceWorkflowError::Invalid(
                    "federated workflow digest does not match plan and budget state".into(),
                ),
            );
        }
        if self.artifact.artifact_id
            != format!(
                "adapter-federated-continual-evidence-workflow:{}",
                self.workflow_id
            )
            || self.artifact.content_type
                != "application/vnd.aurora.federated-continual-research-workflow+json"
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(
                FederatedContinualEvidenceSurveillanceWorkflowError::Artifact(
                    "federated workflow artifact is not bound to its receipt".into(),
                ),
            );
        }
        let payload = json!({
            "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
            "contract_version": CONTRACT_VERSION,
            "feature_id": FEATURE_ID,
            "request_id": self.request_id,
            "workflow_id": self.workflow_id,
            "agent_id": self.agent_id,
            "federation_id": self.federation_id,
            "purpose": self.purpose,
            "endpoint": self.endpoint,
            "semantic_profile": self.semantic_profile,
            "allowed_artifacts": self.allowed_artifacts,
            "min_peer_quorum": self.min_peer_quorum,
            "checkpoint_id": self.checkpoint_id,
            "budget_units": self.budget_units,
            "required_budget": self.required_budget,
            "disposition": self.disposition,
            "stage_order": self.stage_order,
            "plan_order": self.plan_order,
            "completed_order": self.completed_order,
            "blocked_order": self.blocked_order,
            "compensation_order": self.compensation_order,
            "peer_order": self.peer_order,
            "candidate_order": self.candidate_order,
            "selected_order": self.selected_order,
            "unresolved_order": self.unresolved_order,
            "denied_order": self.denied_order,
            "aggregate_order": self.aggregate_order,
            "replay_identity": self.replay_identity,
            "copilot_run_digest": self.copilot_run_digest,
            "checkpoint_digest": self.checkpoint_digest,
            "federation_envelope_digest": self.federation_envelope_digest,
            "workflow_digest": self.workflow_digest,
            "omissions": self.omissions,
            "uncertainty": self.uncertainty,
            "negative_evidence": self.negative_evidence,
            "effect_receipts": self.effect_receipts,
            "boundary": PRECLINICAL_BOUNDARY,
            "raw_data_local": self.raw_data_local,
            "federation_export": "aggregate-only",
        });
        self.artifact.verify_payload(&payload).map_err(|error| {
            FederatedContinualEvidenceSurveillanceWorkflowError::Artifact(error.to_string())
        })?;
        self.artifact.validate_metadata().map_err(|error| {
            FederatedContinualEvidenceSurveillanceWorkflowError::Artifact(error.to_string())
        })?;
        if self.input_digest != federated_workflow_input_digest(&self.input)? {
            return Err(
                FederatedContinualEvidenceSurveillanceWorkflowError::Invalid(
                    "federated workflow retained input digest mismatch".into(),
                ),
            );
        }
        let expected = build_federated_continual_evidence_surveillance_workflow(&self.input)?;
        if self != &expected {
            return Err(
                FederatedContinualEvidenceSurveillanceWorkflowError::Invalid(
                    "federated workflow receipt does not match its retained input".into(),
                ),
            );
        }
        Ok(())
    }
}

pub fn federated_continual_evidence_surveillance_workflow_fabric_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "adapter".into(),
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
    let receipt = build_federated_continual_evidence_surveillance_workflow(request)?;
    receipt.validate()?;
    Ok(receipt)
}

fn build_federated_continual_evidence_surveillance_workflow(
    request: &FederatedContinualEvidenceSurveillanceWorkflowRequest,
) -> Result<
    FederatedContinualEvidenceSurveillanceWorkflowReceipt,
    FederatedContinualEvidenceSurveillanceWorkflowError,
> {
    validate_request(request)?;
    let canonical_request =
        canonical_federated_continual_evidence_surveillance_workflow_request(request);
    let request = &canonical_request;
    let copilot = run_federated_continual_evidence_surveillance_research_copilot(&request.request)
        .map_err(|error| {
            FederatedContinualEvidenceSurveillanceWorkflowError::Copilot(error.to_string())
        })?;
    let stage_order = CANONICAL_STAGES
        .iter()
        .map(|stage| (*stage).to_string())
        .collect::<Vec<_>>();
    let mut plan = BTreeSet::new();
    let mut compensation = BTreeSet::new();
    for stage in &stage_order {
        plan.insert(format!("plan:{stage}"));
    }
    if copilot.aggregate_order.is_empty() {
        plan.insert("plan:retain-unresolved-federated-evidence".into());
        compensation.insert("compensate:research-work:retain-unresolved-evidence".into());
    } else {
        plan.insert("plan:seal-permitted-aggregate-envelope".into());
    }
    if request.budget_units < plan.len() as u32 {
        plan.insert("plan:budget-review".into());
        compensation.insert("compensate:research-work:budget-exhausted".into());
    }
    let required_budget = plan.len() as u32;
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
    let mut blocked = BTreeSet::new();
    if disposition == FederatedContinualResearchCopilotDisposition::Blocked {
        blocked.insert("stage:release".into());
    }
    if request.budget_units < required_budget {
        blocked.insert("stage:budget".into());
    }
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let plan_order = plan.into_iter().collect::<Vec<_>>();
    let completed_order = if blocked_order.is_empty() {
        stage_order.clone()
    } else {
        Vec::new()
    };
    let compensation_order = compensation.into_iter().collect::<Vec<_>>();
    let checkpoint_digest = ContentHash::of_value(&json!({
        "workflow_id": request.workflow_id,
        "checkpoint_id": request.checkpoint_id,
        "stage_order": stage_order,
        "replay_identity": request.replay_identity,
    }))
    .map_err(|error| {
        FederatedContinualEvidenceSurveillanceWorkflowError::Artifact(error.to_string())
    })?;
    let copilot_run_digest = copilot.run_digest.clone();
    let federation_envelope_digest = ContentHash::of_value(&json!({
        "federation_id": request.request.federation_id,
        "purpose": request.request.purpose,
        "endpoint": request.request.endpoint,
        "peer_order": copilot.peer_order,
        "aggregate_order": copilot.aggregate_order,
        "raw_data_local": request.request.raw_data_local,
        "allowed_artifacts": request.request.allowed_artifacts,
    }))
    .map_err(|error| {
        FederatedContinualEvidenceSurveillanceWorkflowError::Artifact(error.to_string())
    })?;
    let workflow_digest = ContentHash::of_value(&json!({
        "workflow_id": request.workflow_id,
        "plan_order": plan_order,
        "completed_order": completed_order,
        "blocked_order": blocked_order,
        "compensation_order": compensation_order,
        "checkpoint_digest": checkpoint_digest,
        "federation_envelope_digest": federation_envelope_digest,
        "copilot_run_digest": copilot_run_digest,
        "budget_units": request.budget_units,
        "required_budget": required_budget,
    }))
    .map_err(|error| {
        FederatedContinualEvidenceSurveillanceWorkflowError::Artifact(error.to_string())
    })?;
    let effect_receipts = if disposition == FederatedContinualResearchCopilotDisposition::Blocked {
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
        "federation_id": request.request.federation_id,
        "purpose": request.request.purpose,
        "endpoint": request.request.endpoint,
        "semantic_profile": request.request.semantic_profile,
        "allowed_artifacts": request.request.allowed_artifacts,
        "min_peer_quorum": request.request.min_peer_quorum,
        "checkpoint_id": request.checkpoint_id,
        "budget_units": request.budget_units,
        "required_budget": required_budget,
        "disposition": disposition,
        "stage_order": stage_order,
        "plan_order": plan_order,
        "completed_order": completed_order,
        "blocked_order": blocked_order,
        "compensation_order": compensation_order,
        "peer_order": copilot.peer_order,
        "candidate_order": copilot.candidate_order,
        "selected_order": copilot.selected_order,
        "unresolved_order": copilot.unresolved_order,
        "denied_order": copilot.denied_order,
        "aggregate_order": copilot.aggregate_order,
        "replay_identity": request.replay_identity,
        "copilot_run_digest": copilot_run_digest,
        "checkpoint_digest": checkpoint_digest,
        "federation_envelope_digest": federation_envelope_digest,
        "workflow_digest": workflow_digest,
        "omissions": omissions,
        "uncertainty": copilot.uncertainty,
        "negative_evidence": copilot.negative_evidence,
        "effect_receipts": effect_receipts,
        "boundary": PRECLINICAL_BOUNDARY,
        "raw_data_local": request.request.raw_data_local,
        "federation_export": "aggregate-only",
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "adapter-federated-continual-evidence-workflow:{}",
            request.workflow_id
        ),
        "application/vnd.aurora.federated-continual-research-workflow+json",
        &payload,
        vec![],
        vec![],
    )
    .map_err(|error| {
        FederatedContinualEvidenceSurveillanceWorkflowError::Artifact(error.to_string())
    })?;
    let input_digest = federated_workflow_input_digest(request)?;
    let receipt = FederatedContinualEvidenceSurveillanceWorkflowReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        input: canonical_request.clone(),
        input_digest,
        request_id: request.request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        agent_id: request.request.agent_id.clone(),
        federation_id: request.request.federation_id.clone(),
        purpose: request.request.purpose.clone(),
        endpoint: request.request.endpoint.clone(),
        semantic_profile: request.request.semantic_profile.clone(),
        allowed_artifacts: request.request.allowed_artifacts.clone(),
        min_peer_quorum: request.request.min_peer_quorum,
        checkpoint_id: request.checkpoint_id.clone(),
        budget_units: request.budget_units,
        required_budget,
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
    Ok(receipt)
}

fn validate_request(
    request: &FederatedContinualEvidenceSurveillanceWorkflowRequest,
) -> Result<(), FederatedContinualEvidenceSurveillanceWorkflowError> {
    if request.budget_units == 0
        || u64::from(request.budget_units) > MAX_ITEMS as u64
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.request.boundary != PRECLINICAL_BOUNDARY
        || !request.request.raw_data_local
    {
        return Err(FederatedContinualEvidenceSurveillanceWorkflowError::Invalid(
            "workflow identity, checkpoint, budget, locality, or preclinical boundary is invalid"
                .into(),
        ));
    }
    validate_text("workflow_id", &request.workflow_id)?;
    validate_text("checkpoint_id", &request.checkpoint_id)?;
    validate_text("boundary", &request.boundary)?;
    validate_text("request_id", &request.request.request_id)?;
    validate_text("agent_id", &request.request.agent_id)?;
    validate_text("federation_id", &request.request.federation_id)?;
    validate_text("purpose", &request.request.purpose)?;
    validate_text("endpoint", &request.request.endpoint)?;
    validate_text("semantic_profile", &request.request.semantic_profile)?;
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
    validate_digest("workflow.replay_identity", &request.replay_identity)?;
    validate_digest("copilot.replay_identity", &request.request.replay_identity)?;
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
        assert!(receipt.completed_order.is_empty());
        assert!(receipt.blocked_order.contains(&"stage:budget".to_string()));
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

    #[test]
    fn reordered_nested_copilot_input_has_stable_identity() {
        let mut reordered = request();
        reordered.request.allowed_artifacts.reverse();
        reordered.request.declared_tools.reverse();
        reordered.request.contributions.reverse();
        let first =
            schedule_federated_continual_evidence_surveillance_workflow(&request()).unwrap();
        let second =
            schedule_federated_continual_evidence_surveillance_workflow(&reordered).unwrap();
        assert_eq!(first.input_digest, second.input_digest);
        assert_eq!(first.workflow_digest, second.workflow_digest);
    }

    #[test]
    fn tampered_envelope_digest_is_rejected() {
        let mut receipt =
            schedule_federated_continual_evidence_surveillance_workflow(&request()).unwrap();
        receipt.federation_envelope_digest = ContentHash::of_bytes(b"tampered-envelope");
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn tampered_retained_request_is_rejected() {
        let mut receipt =
            schedule_federated_continual_evidence_surveillance_workflow(&request()).unwrap();
        receipt.input.workflow_id = "workflow:tampered".into();
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn blocked_quorum_does_not_report_completed_stages() {
        let mut value = request();
        value.request.min_peer_quorum = 3;
        let receipt = schedule_federated_continual_evidence_surveillance_workflow(&value).unwrap();
        assert!(receipt.completed_order.is_empty());
        assert!(receipt.blocked_order.contains(&"stage:release".to_string()));
    }
}
