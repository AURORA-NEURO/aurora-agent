//! Prospective high-throughput retrieval workflow fabric.
//!
//! Atlas feature: `AFA-brain-P02-F15`. Queue capacity, checkpoint identity, and overflow are
//! workflow state, not hidden implementation details.

use crate::retrieval_synthesis::SynthesisDisposition;
use crate::throughput_retrieval_synthesis::{
    synthesize_throughput_retrieval, ThroughputRetrievalQuery,
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

pub const FEATURE_ID: &str = "AFA-brain-P02-F15";
pub const CONTRACT_VERSION: &str = "brain-throughput-retrieval-workflow-fabric/1.0";
pub const OUTPUT_SCHEMA: &str = "ThroughputRetrievalWorkflowReceipt1@1";
pub const STAGE_ORDER: [&str; 5] = [
    "stage:checkpoint",
    "stage:admit-throughput-batch",
    "stage:reconcile-queue",
    "stage:synthesize-evidence",
    "stage:validate-output",
];
const WORKFLOW_CONTENT_TYPE: &str =
    "application/vnd.aurora.throughput-retrieval-workflow-receipt+json";
const MAX_TEXT_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputRetrievalWorkflowRequest {
    pub request: ThroughputRetrievalQuery,
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
pub struct ThroughputRetrievalWorkflowReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub checkpoint_id: String,
    pub batch_id: String,
    pub partition: String,
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
    pub checkpoint_seq: u64,
    pub queue_digest: ContentHash,
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
pub enum ThroughputRetrievalWorkflowError {
    #[error("invalid throughput retrieval workflow request: {0}")]
    Invalid(String),
    #[error("throughput retrieval workflow artifact failed: {0}")]
    Artifact(String),
    #[error("throughput retrieval workflow engine failed: {0}")]
    Engine(String),
}

impl ThroughputRetrievalWorkflowReceipt {
    pub fn validate(&self) -> Result<(), ThroughputRetrievalWorkflowError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.checkpoint_id.trim().is_empty()
            || self.batch_id.trim().is_empty()
            || self.partition.trim().is_empty()
            || self.stage_order.is_empty()
            || self.plan_order.is_empty()
            || self.completed_order.is_empty()
            || self.effect_receipts.is_empty()
            || self.checkpoint_seq == 0
            || self.budget_units == 0
        {
            return Err(ThroughputRetrievalWorkflowError::Invalid("throughput workflow identity, queue, checkpoint, stages, plan, locality, budget, or effects are incomplete".into()));
        }
        for (value, field) in [
            (&self.request_id, "request_id"),
            (&self.workflow_id, "workflow_id"),
            (&self.checkpoint_id, "checkpoint_id"),
            (&self.batch_id, "batch_id"),
            (&self.partition, "partition"),
            (&self.boundary, "boundary"),
        ] {
            validate_text(value, field)?;
        }
        if self.stage_order
            != STAGE_ORDER
                .iter()
                .map(|value| (*value).to_string())
                .collect::<Vec<_>>()
        {
            return Err(ThroughputRetrievalWorkflowError::Invalid(
                "throughput workflow stages are not in canonical order".into(),
            ));
        }
        validate_unique(&self.completed_order, "completed_order")?;
        for (values, field) in [
            (&self.plan_order, "plan_order"),
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
            (&self.unknown_order, "unknown_order"),
        ] {
            validate_unique(values, field)?;
        }
        let candidate_keys = identity_keys(&self.candidate_order);
        if identity_keys(&self.ranked_order) != candidate_keys {
            return Err(ThroughputRetrievalWorkflowError::Invalid(
                "throughput workflow ranked order must contain every candidate exactly once".into(),
            ));
        }
        let qualified_keys = identity_keys(&self.qualified_order);
        let blocked_keys = identity_keys(&self.blocked_order);
        let unknown_keys = identity_keys(&self.unknown_order);
        if !qualified_keys.is_disjoint(&blocked_keys)
            || !unknown_keys.is_subset(&blocked_keys)
            || self
                .ranked_order
                .iter()
                .any(|candidate| !self.candidate_order.contains(candidate))
            || self
                .qualified_order
                .iter()
                .any(|candidate| !self.candidate_order.contains(candidate))
            || self
                .blocked_order
                .iter()
                .any(|candidate| !self.candidate_order.contains(candidate))
            || self
                .unknown_order
                .iter()
                .any(|candidate| !self.blocked_order.contains(candidate))
            || qualified_keys
                .union(&blocked_keys)
                .cloned()
                .collect::<BTreeSet<_>>()
                != candidate_keys
        {
            return Err(ThroughputRetrievalWorkflowError::Invalid(
                "throughput workflow candidate states must partition candidates".into(),
            ));
        }
        for digest in [
            &self.queue_digest,
            &self.synthesis_digest,
            &self.checkpoint_digest,
            &self.workflow_digest,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(ThroughputRetrievalWorkflowError::Invalid(
                    "throughput workflow digest is invalid".into(),
                ));
            }
        }
        let expected_effect_receipts = if self.disposition == SynthesisDisposition::Qualified {
            vec![format!(
                "schedule:throughput-retrieval-work:{}",
                self.workflow_id
            )]
        } else if self.disposition != SynthesisDisposition::Blocked
            && !self.compensation_order.is_empty()
        {
            self.compensation_order.clone()
        } else {
            vec!["block:unsafe-release".into()]
        };
        if self.effect_receipts != expected_effect_receipts {
            return Err(ThroughputRetrievalWorkflowError::Invalid(
                "throughput workflow effects do not match disposition and compensation".into(),
            ));
        }
        if !self.raw_data_local
            && (self.disposition != SynthesisDisposition::Blocked
                || !self
                    .omissions
                    .iter()
                    .any(|item| item == "workflow:raw-data-locality-failed"))
        {
            return Err(ThroughputRetrievalWorkflowError::Invalid(
                "non-local throughput workflows must be blocked and retain locality evidence"
                    .into(),
            ));
        }
        let expected_checkpoint_digest = ContentHash::of_value(&json!({
            "workflow_id": self.workflow_id,
            "checkpoint_id": self.checkpoint_id,
            "checkpoint_seq": self.checkpoint_seq,
            "stage_order": self.stage_order,
            "replay_identity": self.replay_identity,
        }))
        .map_err(|error| ThroughputRetrievalWorkflowError::Artifact(error.to_string()))?;
        if self.checkpoint_digest != expected_checkpoint_digest {
            return Err(ThroughputRetrievalWorkflowError::Invalid(
                "throughput workflow checkpoint digest is not bound to checkpoint state".into(),
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
            "queue_digest": self.queue_digest,
            "synthesis_digest": self.synthesis_digest,
            "budget_units": self.budget_units,
            "replay_identity": self.replay_identity,
            "raw_data_local": self.raw_data_local,
        }))
        .map_err(|error| ThroughputRetrievalWorkflowError::Artifact(error.to_string()))?;
        if self.workflow_digest != expected_workflow_digest {
            return Err(ThroughputRetrievalWorkflowError::Invalid(
                "throughput workflow digest is not bound to workflow state".into(),
            ));
        }
        let expected_artifact_id =
            format!("brain-throughput-retrieval-workflow:{}", self.workflow_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != WORKFLOW_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(ThroughputRetrievalWorkflowError::Invalid(
                "throughput workflow artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ThroughputRetrievalWorkflowError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| ThroughputRetrievalWorkflowError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, ThroughputRetrievalWorkflowError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ThroughputRetrievalWorkflowError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ThroughputRetrievalWorkflowError::Artifact(error.to_string()))
    }
}

pub fn throughput_retrieval_workflow_fabric_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["throughput retrieval operator".into(), "workflow reliability engineer".into()].into(), behavior: "schedules a checkpointed high-throughput retrieval workflow with queue reconciliation, capacity compensation, deterministic synthesis, and replay receipts".into(), value: "turns bounded retrieval batches into resumable production workflows without silent overflow or unsupported admission".into(), inputs: vec![TypedPort { name: "throughput_retrieval_workflow_request".into(), schema: "ResearchWorkflowSpec3@1".into(), required: true }], outputs: vec![TypedPort { name: "throughput_retrieval_workflow_receipt".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["schedule:throughput-retrieval-work".into(), "read:local-research-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "cwl-v1.2".into(), state: EvidenceState::Supported, locator: Some("https://www.commonwl.org/specification/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn compile_throughput_retrieval_workflow(
    request: &ThroughputRetrievalWorkflowRequest,
) -> Result<ThroughputRetrievalWorkflowReceipt, ThroughputRetrievalWorkflowError> {
    validate_request(request)?;
    let synthesis = synthesize_throughput_retrieval(&request.request)
        .map_err(|error| ThroughputRetrievalWorkflowError::Engine(error.to_string()))?;
    let stage_order = request.requested_stage_order.clone();
    let mut plan_order = stage_order
        .iter()
        .map(|stage| format!("plan:{stage}"))
        .collect::<BTreeSet<_>>();
    let completed_order = stage_order.clone();
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
    if synthesis
        .omissions
        .iter()
        .any(|item| item == "batch:capacity-overflow")
    {
        plan_order.insert("plan:reconcile-capacity-overflow".into());
        compensation_order.insert("compensate:throughput-retrieval-work:retain-overflow".into());
    }
    if synthesis.qualified_order.is_empty() {
        plan_order.insert("plan:retain-unresolved-throughput-retrieval".into());
        compensation_order
            .insert("compensate:throughput-retrieval-work:retain-unresolved-evidence".into());
        omissions.insert("workflow:no-qualified-throughput-retrieval-to-schedule".into());
        blocked_order.extend(synthesis.unknown_order.iter().cloned());
    } else if synthesis.disposition != SynthesisDisposition::Qualified {
        plan_order.insert("plan:retain-partial-throughput-retrieval".into());
        compensation_order
            .insert("compensate:throughput-retrieval-work:retain-partial-evidence".into());
    } else {
        plan_order.insert("plan:publish-qualified-throughput-retrieval".into());
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
    let disposition = if actionable {
        synthesis.disposition
    } else {
        SynthesisDisposition::Blocked
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
        .map_err(|error| ThroughputRetrievalWorkflowError::Engine(error.to_string()))?;
    let checkpoint_digest = ContentHash::of_value(&json!({"workflow_id": request.workflow_id, "checkpoint_id": request.checkpoint_id, "checkpoint_seq": request.checkpoint_seq, "stage_order": stage_order, "replay_identity": request.replay_identity})).map_err(|error| ThroughputRetrievalWorkflowError::Artifact(error.to_string()))?;
    let workflow_digest = ContentHash::of_value(&json!({"workflow_id": request.workflow_id, "disposition": disposition, "plan_order": plan_order, "completed_order": completed_order, "blocked_order": blocked_order, "compensation_order": compensation_order, "checkpoint_digest": checkpoint_digest, "queue_digest": synthesis.queue_digest, "synthesis_digest": synthesis_digest, "budget_units": request.budget_units, "replay_identity": request.replay_identity, "raw_data_local": true})).map_err(|error| ThroughputRetrievalWorkflowError::Artifact(error.to_string()))?;
    let effect_receipts = if disposition == SynthesisDisposition::Qualified {
        vec![format!(
            "schedule:throughput-retrieval-work:{}",
            request.workflow_id
        )]
    } else if disposition != SynthesisDisposition::Blocked && !compensation_order.is_empty() {
        compensation_order.clone()
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request.request_id, "workflow_id": request.workflow_id, "checkpoint_id": request.checkpoint_id, "batch_id": request.request.batch_id, "partition": request.request.partition, "disposition": disposition, "stage_order": stage_order, "plan_order": plan_order, "completed_order": completed_order, "blocked_order": blocked_order, "compensation_order": compensation_order, "candidate_order": synthesis.candidate_order, "ranked_order": synthesis.ranked_order, "qualified_order": synthesis.qualified_order, "unknown_order": synthesis.unknown_order, "checkpoint_seq": request.checkpoint_seq, "queue_digest": synthesis.queue_digest, "synthesis_digest": synthesis_digest, "checkpoint_digest": checkpoint_digest, "workflow_digest": workflow_digest, "replay_identity": request.replay_identity, "budget_units": request.budget_units, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "effect_receipts": effect_receipts, "raw_data_local": true, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "brain-throughput-retrieval-workflow:{}",
            request.workflow_id
        ),
        WORKFLOW_CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ThroughputRetrievalWorkflowError::Artifact(error.to_string()))?;
    let receipt = ThroughputRetrievalWorkflowReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        checkpoint_id: request.checkpoint_id.clone(),
        batch_id: request.request.batch_id.clone(),
        partition: request.request.partition.clone(),
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
        checkpoint_seq: request.checkpoint_seq,
        queue_digest: synthesis.queue_digest,
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

fn validate_request(
    request: &ThroughputRetrievalWorkflowRequest,
) -> Result<(), ThroughputRetrievalWorkflowError> {
    for (value, field) in [
        (&request.workflow_id, "workflow_id"),
        (&request.checkpoint_id, "checkpoint_id"),
        (&request.boundary, "boundary"),
    ] {
        validate_text(value, field)?;
    }
    if request.requested_stage_order
        != STAGE_ORDER
            .iter()
            .map(|value| (*value).to_string())
            .collect::<Vec<_>>()
        || request.checkpoint_seq == 0
        || request.budget_units == 0
        || request.request.replay_identity != request.replay_identity
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(ThroughputRetrievalWorkflowError::Invalid("throughput workflow identity, canonical stages, checkpoint, budget, replay, or boundary is incomplete".into()));
    }
    if request.replay_identity.as_str().len() != 64 {
        return Err(ThroughputRetrievalWorkflowError::Invalid(
            "throughput workflow replay identity digest is invalid".into(),
        ));
    }
    Ok(())
}

fn identity_keys(values: &[String]) -> BTreeSet<String> {
    values
        .iter()
        .map(|value| value.to_ascii_lowercase())
        .collect()
}

fn validate_text(value: &str, field: &str) -> Result<(), ThroughputRetrievalWorkflowError> {
    if value.trim().is_empty()
        || value != value.trim()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(ThroughputRetrievalWorkflowError::Invalid(format!(
            "{field} must be bounded, non-empty text without padding or control characters"
        )));
    }
    Ok(())
}

fn validate_unique(values: &[String], field: &str) -> Result<(), ThroughputRetrievalWorkflowError> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_text(value, field)?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(ThroughputRetrievalWorkflowError::Invalid(format!(
                "{field} contains duplicate or case-colliding values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_unique(
    values: &[String],
    field: &str,
) -> Result<(), ThroughputRetrievalWorkflowError> {
    validate_unique(values, field)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ThroughputRetrievalWorkflowError::Invalid(format!(
            "{field} is not in canonical order"
        )));
    }
    Ok(())
}

fn receipt_payload(receipt: &ThroughputRetrievalWorkflowReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "workflow_id": receipt.workflow_id,
        "checkpoint_id": receipt.checkpoint_id,
        "batch_id": receipt.batch_id,
        "partition": receipt.partition,
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
        "checkpoint_seq": receipt.checkpoint_seq,
        "queue_digest": receipt.queue_digest,
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
    fn request(state: EvidenceState) -> ThroughputRetrievalWorkflowRequest {
        let candidates = (0..3)
            .map(|index| RetrievalCandidate {
                evidence_id: format!("evidence:{index}"),
                source_id: format!("source:{index}"),
                study_id: "study:throughput".into(),
                scope: "organoid:neural".into(),
                modality: "imaging".into(),
                support_milli: 900,
                state,
                semantic_digest: hash(&format!("semantic:{index}")),
                artifact_digest: hash(&format!("artifact:{index}")),
                provenance_digest: hash(&format!("provenance:{index}")),
                replay_identity: hash("replay"),
                omissions: Vec::new(),
                negative_evidence: Vec::new(),
                raw_data_local: true,
                boundary: PRECLINICAL_BOUNDARY.into(),
            })
            .collect();
        ThroughputRetrievalWorkflowRequest {
            request: ThroughputRetrievalQuery {
                request_id: "request:throughput-retrieval-workflow".into(),
                batch_id: "batch:1".into(),
                partition: "partition:0".into(),
                max_items: 2,
                minimum_support_milli: 700,
                candidates,
                replay_identity: hash("replay"),
                policy_allow: true,
                protected_closure: true,
                raw_data_local: true,
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            workflow_id: "workflow:throughput-retrieval".into(),
            requested_stage_order: vec![
                "stage:checkpoint".into(),
                "stage:admit-throughput-batch".into(),
                "stage:reconcile-queue".into(),
                "stage:synthesize-evidence".into(),
                "stage:validate-output".into(),
            ],
            checkpoint_id: "checkpoint:1".into(),
            checkpoint_seq: 1,
            budget_units: 12,
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        let manifest = throughput_retrieval_workflow_fabric_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A1);
    }
    #[test]
    fn overflow_is_compensated() {
        let receipt =
            compile_throughput_retrieval_workflow(&request(EvidenceState::Supported)).unwrap();
        assert_eq!(receipt.disposition, SynthesisDisposition::Partial);
        assert!(!receipt.compensation_order.is_empty());
    }
    #[test]
    fn unknown_is_compensated() {
        let receipt =
            compile_throughput_retrieval_workflow(&request(EvidenceState::Unknown)).unwrap();
        assert!(!receipt.compensation_order.is_empty());
    }
    #[test]
    fn policy_denial_blocks() {
        let mut input = request(EvidenceState::Supported);
        input.policy_allow = false;
        let receipt = compile_throughput_retrieval_workflow(&input).unwrap();
        assert_eq!(receipt.disposition, SynthesisDisposition::Blocked);
    }

    #[test]
    fn locality_failure_is_blocked_and_retained() {
        let mut input = request(EvidenceState::Supported);
        input.raw_data_local = false;
        let receipt = compile_throughput_retrieval_workflow(&input).unwrap();
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
        let mut receipt =
            compile_throughput_retrieval_workflow(&request(EvidenceState::Supported)).unwrap();
        receipt.workflow_id = "workflow:tampered".into();
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn case_mismatched_ranked_identity_is_rejected() {
        let mut receipt =
            compile_throughput_retrieval_workflow(&request(EvidenceState::Supported)).unwrap();
        receipt.ranked_order[0] = receipt.ranked_order[0].to_ascii_uppercase();
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn stage_protocol_is_required() {
        let mut input = request(EvidenceState::Supported);
        input.requested_stage_order.reverse();
        assert!(compile_throughput_retrieval_workflow(&input).is_err());
    }
    #[test]
    fn checkpoint_is_required() {
        let mut input = request(EvidenceState::Supported);
        input.checkpoint_seq = 0;
        assert!(compile_throughput_retrieval_workflow(&input).is_err());
    }
}
