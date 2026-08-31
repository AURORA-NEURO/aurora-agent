//! Prospective high-throughput context workflow fabric.
//!
//! Atlas feature: `AFA-brain-P03-F15`. The batch fabric admits only bounded,
//! policy-authorized local context jobs, preserves deterministic queue order,
//! and retains every item stopped by budget, evidence, or locality gates.

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

pub const FEATURE_ID: &str = "AFA-brain-P03-F15";
pub const CONTRACT_VERSION: &str = "brain-throughput-context-workflow-fabric/1.0";
const WORKFLOW_CONTENT_TYPE: &str = "application/vnd.aurora.throughput-context-workflow+json";
const MAX_TEXT_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputContextJob {
    pub job_id: String,
    pub context_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub state: EvidenceState,
    pub ready: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputContextWorkflowRequest {
    pub request_id: String,
    pub batch_id: String,
    pub query_id: String,
    pub goal: String,
    pub jobs: Vec<ThroughputContextJob>,
    pub max_concurrency: u16,
    pub budget_units: u32,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputContextWorkflowReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub batch_id: String,
    pub query_id: String,
    pub goal: String,
    pub disposition: String,
    pub queue_order: Vec<String>,
    pub scheduled_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub concurrency: u16,
    pub budget_units: u32,
    pub consumed_budget_units: u32,
    pub batch_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ThroughputContextWorkflowError {
    #[error("invalid throughput context workflow request: {0}")]
    Invalid(String),
    #[error("throughput context workflow artifact failed: {0}")]
    Artifact(String),
}

impl ThroughputContextWorkflowReceipt {
    pub fn validate(&self) -> Result<(), ThroughputContextWorkflowError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.request_id.trim().is_empty()
            || self.batch_id.trim().is_empty()
            || self.query_id.trim().is_empty()
            || self.goal.trim().is_empty()
            || self.queue_order.is_empty()
            || self.concurrency == 0
            || self.budget_units == 0
            || self.consumed_budget_units > self.budget_units
            || self.effect_receipts.is_empty()
        {
            return Err(ThroughputContextWorkflowError::Invalid("throughput workflow identity, queue, concurrency, budget, locality, or effects are incomplete".into()));
        }
        for (value, field) in [
            (&self.request_id, "request_id"),
            (&self.batch_id, "batch_id"),
            (&self.query_id, "query_id"),
            (&self.goal, "goal"),
            (&self.boundary, "boundary"),
        ] {
            validate_text(value, field)?;
        }
        for (values, field) in [
            (&self.queue_order, "queue_order"),
            (&self.scheduled_order, "scheduled_order"),
            (&self.blocked_order, "blocked_order"),
            (&self.unknown_order, "unknown_order"),
            (&self.omissions, "omissions"),
            (&self.uncertainty, "uncertainty"),
            (&self.negative_evidence, "negative_evidence"),
            (&self.effect_receipts, "effect_receipts"),
        ] {
            validate_sorted_unique(values, field)?;
        }
        let queue = self.queue_order.iter().cloned().collect::<BTreeSet<_>>();
        let mut classified = self
            .scheduled_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        classified.extend(self.blocked_order.iter().cloned());
        classified.extend(self.unknown_order.iter().cloned());
        if classified != queue
            || !identity_keys(&self.scheduled_order)
                .is_disjoint(&identity_keys(&self.blocked_order))
            || !identity_keys(&self.scheduled_order)
                .is_disjoint(&identity_keys(&self.unknown_order))
            || !identity_keys(&self.blocked_order).is_disjoint(&identity_keys(&self.unknown_order))
        {
            return Err(ThroughputContextWorkflowError::Invalid(
                "throughput jobs do not partition outcomes".into(),
            ));
        }
        for digest in [&self.batch_digest, &self.replay_identity] {
            if digest.as_str().len() != 64 {
                return Err(ThroughputContextWorkflowError::Invalid(
                    "throughput workflow digest is invalid".into(),
                ));
            }
        }
        let expected_effect_receipts = if self.disposition == "admitted" {
            vec![format!(
                "schedule:throughput-context-workflow:{}",
                self.batch_id
            )]
        } else {
            vec!["block:unsafe-release".into()]
        };
        if self.effect_receipts != expected_effect_receipts {
            return Err(ThroughputContextWorkflowError::Invalid(
                "throughput workflow effect does not match disposition".into(),
            ));
        }
        if !self.raw_data_local {
            return Err(ThroughputContextWorkflowError::Invalid(
                "non-local throughput workflows must be blocked and retain locality evidence"
                    .into(),
            ));
        }
        let expected_batch_digest = ContentHash::of_value(&json!({
            "batch_id": self.batch_id,
            "queue_order": self.queue_order,
            "scheduled_order": self.scheduled_order,
            "blocked_order": self.blocked_order,
            "unknown_order": self.unknown_order,
            "concurrency": self.concurrency,
            "consumed_budget_units": self.consumed_budget_units,
            "replay_identity": self.replay_identity,
            "raw_data_local": self.raw_data_local,
        }))
        .map_err(|error| ThroughputContextWorkflowError::Artifact(error.to_string()))?;
        if self.batch_digest != expected_batch_digest {
            return Err(ThroughputContextWorkflowError::Invalid(
                "throughput workflow batch digest is not bound to queue state".into(),
            ));
        }
        let expected_artifact_id = format!("brain-throughput-context-workflow:{}", self.batch_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != WORKFLOW_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(ThroughputContextWorkflowError::Invalid(
                "throughput workflow artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ThroughputContextWorkflowError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| ThroughputContextWorkflowError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, ThroughputContextWorkflowError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ThroughputContextWorkflowError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ThroughputContextWorkflowError::Artifact(error.to_string()))
    }
}

pub fn throughput_context_workflow_fabric_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["laboratory automation engineer".into(), "research workflow operator".into()].into(), behavior: "executes a bounded high-throughput context workflow batch with deterministic queueing, concurrency, budget, and per-job evidence states".into(), value: "turns prospective context compilation into a replayable batch product without dropping failures or exceeding local resource authority".into(), inputs: vec![TypedPort { name: "throughput_context_workflow_request".into(), schema: "ResearchWorkflowSpec1@1".into(), required: true }], outputs: vec![TypedPort { name: "throughput_context_workflow_receipt".into(), schema: "ThroughputContextWorkflowReceipt1@1".into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["schedule:throughput-context-workflow".into(), "read:local-research-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "opentelemetry-specification".into(), state: EvidenceState::Supported, locator: Some("https://opentelemetry.io/docs/specs/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn compile_throughput_context_workflow(
    request: &ThroughputContextWorkflowRequest,
) -> Result<ThroughputContextWorkflowReceipt, ThroughputContextWorkflowError> {
    if request.request_id.trim().is_empty()
        || request.batch_id.trim().is_empty()
        || request.query_id.trim().is_empty()
        || request.goal.trim().is_empty()
        || request.jobs.is_empty()
        || request.max_concurrency == 0
        || request.budget_units == 0
        || request.replay_identity.as_str().len() != 64
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(ThroughputContextWorkflowError::Invalid("throughput workflow identity, jobs, concurrency, budget, replay, or boundary is invalid".into()));
    }
    let mut jobs = request.jobs.clone();
    jobs.sort_by(|left, right| left.job_id.cmp(&right.job_id));
    let queue = jobs
        .iter()
        .map(|job| job.job_id.clone())
        .collect::<Vec<_>>();
    if queue.windows(2).any(|pair| pair[0] == pair[1])
        || queue.iter().any(|id| id.trim().is_empty())
    {
        return Err(ThroughputContextWorkflowError::Invalid(
            "throughput job identifiers must be unique and non-empty".into(),
        ));
    }
    let queue_count = u32::try_from(queue.len()).map_err(|_| {
        ThroughputContextWorkflowError::Invalid(
            "throughput queue size exceeds workflow budget width".into(),
        )
    })?;
    let mut scheduled = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let locality_failure = !request.raw_data_local || jobs.iter().any(|job| !job.raw_data_local);
    let gates_open = request.policy_allow && request.protected_closure && !locality_failure;
    let mut consumed = 0u32;
    for job in &jobs {
        if !gates_open || !job.raw_data_local || job.boundary != PRECLINICAL_BOUNDARY {
            blocked.insert(job.job_id.clone());
            omissions.insert(format!("job:{}:policy-locality-gate-blocked", job.job_id));
        } else if job.replay_identity != request.replay_identity {
            unknown.insert(job.job_id.clone());
            uncertainty.insert(format!("job:{}:replay-mismatch", job.job_id));
        } else if !job.ready {
            unknown.insert(job.job_id.clone());
            uncertainty.insert(format!("job:{}:not-ready", job.job_id));
        } else if matches!(job.state, EvidenceState::Proven | EvidenceState::Supported)
            && consumed < request.budget_units
        {
            scheduled.insert(job.job_id.clone());
            consumed += 1;
        } else if matches!(
            job.state,
            EvidenceState::Speculative | EvidenceState::Unknown
        ) {
            unknown.insert(job.job_id.clone());
            uncertainty.insert(format!("job:{}:evidence-uncertain", job.job_id));
        } else if job.state == EvidenceState::Contradicted {
            blocked.insert(job.job_id.clone());
            negative.insert(format!("job:{}:contradicted", job.job_id));
        } else {
            blocked.insert(job.job_id.clone());
            omissions.insert(format!("job:{}:budget-exhausted", job.job_id));
        }
    }
    let disposition = if !gates_open {
        "blocked"
    } else if scheduled.len() == queue.len() {
        "admitted"
    } else {
        "refinement_required"
    };
    if request.budget_units < queue_count {
        omissions.insert("workflow:budget-exhausted".into());
    }
    if !request.policy_allow {
        omissions.insert("workflow:policy-denied".into());
    }
    if !request.protected_closure {
        omissions.insert("workflow:protected-closure-incomplete".into());
    }
    if locality_failure {
        omissions.insert("workflow:raw-data-locality-failed".into());
        omissions.insert("workflow:policy-or-locality-blocked".into());
    }
    let raw_data_local = true;
    let batch_digest = ContentHash::of_value(&json!({"batch_id": request.batch_id, "queue_order": queue, "scheduled_order": scheduled, "blocked_order": blocked, "unknown_order": unknown, "concurrency": request.max_concurrency, "consumed_budget_units": consumed, "replay_identity": request.replay_identity, "raw_data_local": raw_data_local})).map_err(|error| ThroughputContextWorkflowError::Artifact(error.to_string()))?;
    let effects = if disposition == "admitted" {
        vec![format!(
            "schedule:throughput-context-workflow:{}",
            request.batch_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "batch_id": request.batch_id, "query_id": request.query_id, "goal": request.goal, "disposition": disposition, "queue_order": queue, "scheduled_order": scheduled, "blocked_order": blocked, "unknown_order": unknown, "concurrency": request.max_concurrency, "budget_units": request.budget_units, "consumed_budget_units": consumed, "batch_digest": batch_digest, "replay_identity": request.replay_identity, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "effect_receipts": effects, "raw_data_local": raw_data_local, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-throughput-context-workflow:{}", request.batch_id),
        WORKFLOW_CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ThroughputContextWorkflowError::Artifact(error.to_string()))?;
    let receipt = ThroughputContextWorkflowReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        batch_id: request.batch_id.clone(),
        query_id: request.query_id.clone(),
        goal: request.goal.clone(),
        disposition: disposition.into(),
        queue_order: queue,
        scheduled_order: scheduled.into_iter().collect(),
        blocked_order: blocked.into_iter().collect(),
        unknown_order: unknown.into_iter().collect(),
        concurrency: request.max_concurrency,
        budget_units: request.budget_units,
        consumed_budget_units: consumed,
        batch_digest,
        replay_identity: request.replay_identity.clone(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts: effects,
        artifact,
        raw_data_local,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn identity_keys(values: &[String]) -> BTreeSet<String> {
    values
        .iter()
        .map(|value| value.to_ascii_lowercase())
        .collect()
}

fn validate_text(value: &str, field: &str) -> Result<(), ThroughputContextWorkflowError> {
    if value.trim().is_empty()
        || value != value.trim()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(ThroughputContextWorkflowError::Invalid(format!(
            "{field} must be bounded, non-empty text without padding or control characters"
        )));
    }
    Ok(())
}

fn validate_unique(values: &[String], field: &str) -> Result<(), ThroughputContextWorkflowError> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_text(value, field)?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(ThroughputContextWorkflowError::Invalid(format!(
                "{field} contains duplicate or case-colliding values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_unique(
    values: &[String],
    field: &str,
) -> Result<(), ThroughputContextWorkflowError> {
    validate_unique(values, field)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ThroughputContextWorkflowError::Invalid(format!(
            "{field} is not in canonical order"
        )));
    }
    Ok(())
}

fn receipt_payload(receipt: &ThroughputContextWorkflowReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "batch_id": receipt.batch_id,
        "query_id": receipt.query_id,
        "goal": receipt.goal,
        "disposition": receipt.disposition,
        "queue_order": receipt.queue_order,
        "scheduled_order": receipt.scheduled_order,
        "blocked_order": receipt.blocked_order,
        "unknown_order": receipt.unknown_order,
        "concurrency": receipt.concurrency,
        "budget_units": receipt.budget_units,
        "consumed_budget_units": receipt.consumed_budget_units,
        "batch_digest": receipt.batch_digest,
        "replay_identity": receipt.replay_identity,
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
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn request() -> ThroughputContextWorkflowRequest {
        let replay = hash("replay");
        ThroughputContextWorkflowRequest {
            request_id: "request:throughput-context".into(),
            batch_id: "batch:one".into(),
            query_id: "query:one".into(),
            goal: "compile a prospective context batch".into(),
            jobs: vec![
                ThroughputContextJob {
                    job_id: "job:a".into(),
                    context_digest: hash("a"),
                    replay_identity: replay.clone(),
                    state: EvidenceState::Supported,
                    ready: true,
                    raw_data_local: true,
                    boundary: PRECLINICAL_BOUNDARY.into(),
                },
                ThroughputContextJob {
                    job_id: "job:b".into(),
                    context_digest: hash("b"),
                    replay_identity: replay.clone(),
                    state: EvidenceState::Proven,
                    ready: true,
                    raw_data_local: true,
                    boundary: PRECLINICAL_BOUNDARY.into(),
                },
            ],
            max_concurrency: 2,
            budget_units: 2,
            replay_identity: replay,
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            throughput_context_workflow_fabric_manifest().autonomy_tier,
            AutonomyTier::A1
        );
    }
    #[test]
    fn ready_batch_admits() {
        let receipt = compile_throughput_context_workflow(&request()).unwrap();
        assert_eq!(receipt.disposition, "admitted");
        assert_eq!(receipt.scheduled_order.len(), 2);
    }
    #[test]
    fn budget_blocks_tail() {
        let mut value = request();
        value.budget_units = 1;
        let receipt = compile_throughput_context_workflow(&value).unwrap();
        assert_eq!(receipt.disposition, "refinement_required");
        assert!(!receipt.blocked_order.is_empty());
    }
    #[test]
    fn not_ready_is_unknown() {
        let mut value = request();
        value.jobs[0].ready = false;
        let receipt = compile_throughput_context_workflow(&value).unwrap();
        assert!(receipt.unknown_order.contains(&"job:a".into()));
    }
    #[test]
    fn policy_denial_blocks() {
        let mut value = request();
        value.policy_allow = false;
        let receipt = compile_throughput_context_workflow(&value).unwrap();
        assert_eq!(receipt.disposition, "blocked");
    }
    #[test]
    fn digest_is_stable() {
        let receipt = compile_throughput_context_workflow(&request()).unwrap();
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
    #[test]
    fn job_locality_failure_blocks_release() {
        let mut input = request();
        input.jobs[0].raw_data_local = false;
        let receipt = compile_throughput_context_workflow(&input).unwrap();
        assert_eq!(receipt.disposition, "blocked");
        assert!(receipt.raw_data_local);
        assert!(receipt.validate().is_ok());
    }
    #[test]
    fn workflow_artifact_payload_is_bound() {
        let mut receipt = compile_throughput_context_workflow(&request()).unwrap();
        receipt.query_id = "query:tampered".into();
        assert!(receipt.validate().is_err());
    }
}
