//! Prospective high-throughput context compilation research workbench.
//!
//! Atlas feature: `AFA-brain-P03-F19`. The workbench presents a bounded queue of
//! local context jobs and admits batch actions only when readiness, replay,
//! evidence, concurrency, and budget gates all close.

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

pub const FEATURE_ID: &str = "AFA-brain-P03-F19";
pub const CONTRACT_VERSION: &str = "brain-throughput-context-workbench/1.0";
const WORKBENCH_CONTENT_TYPE: &str = "application/vnd.aurora.throughput-context-workbench+json";
const MAX_TEXT_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputContextWorkbenchJob {
    pub job_id: String,
    pub context_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub state: EvidenceState,
    pub ready: bool,
    pub cost_units: u32,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputContextWorkbenchRequest {
    pub session_id: String,
    pub query_id: String,
    pub goal: String,
    pub projection_disposition: String,
    pub jobs: Vec<ThroughputContextWorkbenchJob>,
    pub max_concurrency: u16,
    pub budget_units: u32,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputContextWorkbenchReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub session_id: String,
    pub query_id: String,
    pub goal: String,
    pub disposition: String,
    pub queue_order: Vec<String>,
    pub admitted_job_order: Vec<String>,
    pub blocked_job_order: Vec<String>,
    pub unknown_job_order: Vec<String>,
    pub view_order: Vec<String>,
    pub action_order: Vec<String>,
    pub blocked_action_order: Vec<String>,
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
pub enum ThroughputContextWorkbenchError {
    #[error("invalid throughput workbench request: {0}")]
    Invalid(String),
    #[error("throughput workbench artifact failed: {0}")]
    Artifact(String),
}

impl ThroughputContextWorkbenchReceipt {
    pub fn validate(&self) -> Result<(), ThroughputContextWorkbenchError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.session_id.trim().is_empty()
            || self.query_id.trim().is_empty()
            || self.goal.trim().is_empty()
            || self.queue_order.is_empty()
            || self.view_order.is_empty()
            || self.action_order.is_empty()
            || self.concurrency == 0
            || self.budget_units == 0
            || self.consumed_budget_units > self.budget_units
            || self.effect_receipts.is_empty()
            || !matches!(
                self.disposition.as_str(),
                "ready" | "needs_refinement" | "blocked"
            )
        {
            return Err(ThroughputContextWorkbenchError::Invalid("throughput workbench identity, queue, budget, concurrency, view, action, locality, or disposition is incomplete".into()));
        }
        for (value, field) in [
            (&self.session_id, "session_id"),
            (&self.query_id, "query_id"),
            (&self.goal, "goal"),
            (&self.boundary, "boundary"),
        ] {
            validate_text(value, field)?;
        }
        for (values, field) in [
            (&self.queue_order, "queue_order"),
            (&self.admitted_job_order, "admitted_job_order"),
            (&self.blocked_job_order, "blocked_job_order"),
            (&self.unknown_job_order, "unknown_job_order"),
            (&self.view_order, "view_order"),
            (&self.action_order, "action_order"),
            (&self.blocked_action_order, "blocked_action_order"),
            (&self.omissions, "omissions"),
            (&self.uncertainty, "uncertainty"),
            (&self.negative_evidence, "negative_evidence"),
            (&self.effect_receipts, "effect_receipts"),
        ] {
            validate_sorted_unique(values, field)?;
        }
        let queue = self.queue_order.iter().cloned().collect::<BTreeSet<_>>();
        let mut classified = self
            .admitted_job_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        classified.extend(self.blocked_job_order.iter().cloned());
        classified.extend(self.unknown_job_order.iter().cloned());
        if classified != queue
            || !identity_keys(&self.admitted_job_order)
                .is_disjoint(&identity_keys(&self.blocked_job_order))
            || !identity_keys(&self.admitted_job_order)
                .is_disjoint(&identity_keys(&self.unknown_job_order))
            || !identity_keys(&self.blocked_job_order)
                .is_disjoint(&identity_keys(&self.unknown_job_order))
        {
            return Err(ThroughputContextWorkbenchError::Invalid(
                "throughput jobs do not partition outcomes".into(),
            ));
        }
        for digest in [&self.batch_digest, &self.replay_identity] {
            if digest.as_str().len() != 64 {
                return Err(ThroughputContextWorkbenchError::Invalid(
                    "throughput workbench digest is invalid".into(),
                ));
            }
        }
        let expected_effect_receipts = if self.disposition == "blocked" {
            vec!["block:unsafe-release".into()]
        } else {
            vec![format!(
                "view:local-throughput-workbench:{}",
                self.session_id
            )]
        };
        if self.effect_receipts != expected_effect_receipts {
            return Err(ThroughputContextWorkbenchError::Invalid(
                "throughput workbench effect does not match disposition".into(),
            ));
        }
        if !self.raw_data_local {
            return Err(ThroughputContextWorkbenchError::Invalid(
                "non-local throughput workbenches must be blocked and retain locality evidence"
                    .into(),
            ));
        }
        let expected_batch_digest = ContentHash::of_value(&json!({
            "queue_order": self.queue_order,
            "admitted_order": self.admitted_job_order,
            "blocked_order": self.blocked_job_order,
            "unknown_order": self.unknown_job_order,
            "concurrency": self.concurrency,
            "budget_units": self.budget_units,
            "consumed_budget_units": self.consumed_budget_units,
            "replay_identity": self.replay_identity,
            "raw_data_local": self.raw_data_local,
        }))
        .map_err(|error| ThroughputContextWorkbenchError::Artifact(error.to_string()))?;
        if self.batch_digest != expected_batch_digest {
            return Err(ThroughputContextWorkbenchError::Invalid(
                "throughput workbench batch digest is not bound to queue state".into(),
            ));
        }
        let expected_artifact_id =
            format!("brain-throughput-context-workbench:{}", self.session_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != WORKBENCH_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(ThroughputContextWorkbenchError::Invalid(
                "throughput workbench artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ThroughputContextWorkbenchError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| ThroughputContextWorkbenchError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, ThroughputContextWorkbenchError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ThroughputContextWorkbenchError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ThroughputContextWorkbenchError::Artifact(error.to_string()))
    }
}

pub fn throughput_context_workbench_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["agent developer".into(), "research workflow operator".into()].into(), behavior: "renders a bounded high-throughput context queue with typed job state, admission, budget, concurrency, and safe batch actions".into(), value: "gives researchers and agent developers an auditable prospective context workbench without hiding queue failures or exceeding local authority".into(), inputs: vec![TypedPort { name: "throughput_workbench_request".into(), schema: "ResearchWorkbenchSession1@1".into(), required: true }], outputs: vec![TypedPort { name: "throughput_workbench_receipt".into(), schema: "ThroughputContextWorkbenchReceipt1@1".into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["view:local-throughput-workbench".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "opentelemetry-specification".into(), state: EvidenceState::Supported, locator: Some("https://opentelemetry.io/docs/specs/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A0, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn render_throughput_context_workbench(
    request: &ThroughputContextWorkbenchRequest,
) -> Result<ThroughputContextWorkbenchReceipt, ThroughputContextWorkbenchError> {
    if request.session_id.trim().is_empty()
        || request.query_id.trim().is_empty()
        || request.goal.trim().is_empty()
        || request.jobs.is_empty()
        || request.max_concurrency == 0
        || request.budget_units == 0
        || request.replay_identity.as_str().len() != 64
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(ThroughputContextWorkbenchError::Invalid("throughput workbench identity, jobs, concurrency, budget, replay, or boundary is invalid".into()));
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
        return Err(ThroughputContextWorkbenchError::Invalid(
            "throughput job identifiers must be unique and non-empty".into(),
        ));
    }
    let mut admitted = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut views = BTreeSet::from([
        "view:queue".to_string(),
        "view:job-state".to_string(),
        "view:budget-and-concurrency".to_string(),
        "view:replay-identity".to_string(),
    ]);
    let mut actions = BTreeSet::from([
        "action:inspect-job".to_string(),
        "action:replay-local-batch".to_string(),
    ]);
    let mut blocked_actions = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let locality_failure = !request.raw_data_local || jobs.iter().any(|job| !job.raw_data_local);
    let gates_open = request.policy_allow && !locality_failure;
    let mut consumed = 0u32;
    for job in &jobs {
        if !gates_open || !job.raw_data_local || job.boundary != PRECLINICAL_BOUNDARY {
            blocked.insert(job.job_id.clone());
            omissions.insert(format!("job:{}:policy-locality-blocked", job.job_id));
        } else if job.replay_identity != request.replay_identity {
            unknown.insert(job.job_id.clone());
            uncertainty.insert(format!("job:{}:replay-mismatch", job.job_id));
        } else if !job.ready {
            unknown.insert(job.job_id.clone());
            uncertainty.insert(format!("job:{}:not-ready", job.job_id));
        } else if !matches!(job.state, EvidenceState::Proven | EvidenceState::Supported) {
            if matches!(
                job.state,
                EvidenceState::Speculative | EvidenceState::Unknown
            ) {
                unknown.insert(job.job_id.clone());
                uncertainty.insert(format!("job:{}:evidence-uncertain", job.job_id));
            } else {
                blocked.insert(job.job_id.clone());
                negative.insert(format!("job:{}:contradicted", job.job_id));
            }
        } else if admitted.len() >= usize::from(request.max_concurrency) {
            unknown.insert(job.job_id.clone());
            uncertainty.insert(format!("job:{}:concurrency-window", job.job_id));
        } else if consumed.saturating_add(job.cost_units) > request.budget_units {
            blocked.insert(job.job_id.clone());
            omissions.insert(format!("job:{}:budget-exhausted", job.job_id));
        } else {
            consumed += job.cost_units;
            admitted.insert(job.job_id.clone());
        }
    }
    if locality_failure {
        omissions.insert("workbench:policy-or-locality-blocked".into());
    }
    let disposition = if !gates_open {
        omissions.insert("workbench:policy-or-locality-blocked".into());
        "blocked"
    } else if request.projection_disposition == "admitted" && admitted.len() == queue.len() {
        actions.insert("action:open-decision-section".into());
        actions.insert("action:export-local-batch".into());
        "ready"
    } else {
        actions.insert("action:review-queue-outcomes".into());
        actions.insert("action:request-batch-refinement".into());
        uncertainty.insert("workbench:throughput-projection-not-admitted".into());
        "needs_refinement"
    };
    if disposition == "blocked" {
        blocked_actions.extend([
            "action:open-decision-section".to_string(),
            "action:export-local-batch".to_string(),
            "action:replay-local-batch".to_string(),
        ]);
        actions.clear();
        actions.insert("action:inspect-block-reason".into());
    }
    if !unknown.is_empty() {
        views.insert("view:uncertain-jobs".into());
    }
    if !blocked.is_empty() {
        views.insert("view:blocked-jobs".into());
    }
    let raw_data_local = true;
    let batch_digest = ContentHash::of_value(&json!({"queue_order": queue, "admitted_order": admitted, "blocked_order": blocked, "unknown_order": unknown, "concurrency": request.max_concurrency, "budget_units": request.budget_units, "consumed_budget_units": consumed, "replay_identity": request.replay_identity, "raw_data_local": raw_data_local})).map_err(|error| ThroughputContextWorkbenchError::Artifact(error.to_string()))?;
    let effects = if disposition == "blocked" {
        vec!["block:unsafe-release".into()]
    } else {
        vec![format!(
            "view:local-throughput-workbench:{}",
            request.session_id
        )]
    };
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "session_id": request.session_id, "query_id": request.query_id, "goal": request.goal, "disposition": disposition, "queue_order": queue, "admitted_job_order": admitted, "blocked_job_order": blocked, "unknown_job_order": unknown, "view_order": views, "action_order": actions, "blocked_action_order": blocked_actions, "concurrency": request.max_concurrency, "budget_units": request.budget_units, "consumed_budget_units": consumed, "batch_digest": batch_digest, "replay_identity": request.replay_identity, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "effect_receipts": effects, "raw_data_local": raw_data_local, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-throughput-context-workbench:{}", request.session_id),
        WORKBENCH_CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ThroughputContextWorkbenchError::Artifact(error.to_string()))?;
    let receipt = ThroughputContextWorkbenchReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        session_id: request.session_id.clone(),
        query_id: request.query_id.clone(),
        goal: request.goal.clone(),
        disposition: disposition.into(),
        queue_order: queue,
        admitted_job_order: admitted.into_iter().collect(),
        blocked_job_order: blocked.into_iter().collect(),
        unknown_job_order: unknown.into_iter().collect(),
        view_order: views.into_iter().collect(),
        action_order: actions.into_iter().collect(),
        blocked_action_order: blocked_actions.into_iter().collect(),
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

fn validate_text(value: &str, field: &str) -> Result<(), ThroughputContextWorkbenchError> {
    if value.trim().is_empty()
        || value != value.trim()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(ThroughputContextWorkbenchError::Invalid(format!(
            "{field} must be bounded, non-empty text without padding or control characters"
        )));
    }
    Ok(())
}

fn validate_unique(values: &[String], field: &str) -> Result<(), ThroughputContextWorkbenchError> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_text(value, field)?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(ThroughputContextWorkbenchError::Invalid(format!(
                "{field} contains duplicate or case-colliding values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_unique(
    values: &[String],
    field: &str,
) -> Result<(), ThroughputContextWorkbenchError> {
    validate_unique(values, field)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ThroughputContextWorkbenchError::Invalid(format!(
            "{field} is not in canonical order"
        )));
    }
    Ok(())
}

fn receipt_payload(receipt: &ThroughputContextWorkbenchReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "session_id": receipt.session_id,
        "query_id": receipt.query_id,
        "goal": receipt.goal,
        "disposition": receipt.disposition,
        "queue_order": receipt.queue_order,
        "admitted_job_order": receipt.admitted_job_order,
        "blocked_job_order": receipt.blocked_job_order,
        "unknown_job_order": receipt.unknown_job_order,
        "view_order": receipt.view_order,
        "action_order": receipt.action_order,
        "blocked_action_order": receipt.blocked_action_order,
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
    fn job(id: &str, replay: &ContentHash) -> ThroughputContextWorkbenchJob {
        ThroughputContextWorkbenchJob {
            job_id: id.into(),
            context_digest: replay.clone(),
            replay_identity: replay.clone(),
            state: EvidenceState::Supported,
            ready: true,
            cost_units: 1,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    fn request() -> ThroughputContextWorkbenchRequest {
        let replay = hash("replay");
        ThroughputContextWorkbenchRequest {
            session_id: "session:throughput".into(),
            query_id: "query:throughput".into(),
            goal: "inspect high-throughput context queue".into(),
            projection_disposition: "admitted".into(),
            jobs: vec![job("job:a", &replay), job("job:b", &replay)],
            max_concurrency: 2,
            budget_units: 2,
            replay_identity: replay,
            policy_allow: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a0() {
        assert_eq!(
            throughput_context_workbench_manifest().autonomy_tier,
            AutonomyTier::A0
        );
    }
    #[test]
    fn complete_queue_is_ready() {
        let receipt = render_throughput_context_workbench(&request()).unwrap();
        assert_eq!(receipt.disposition, "ready");
        assert_eq!(receipt.admitted_job_order.len(), 2);
    }
    #[test]
    fn concurrency_window_is_visible() {
        let mut value = request();
        value.max_concurrency = 1;
        let receipt = render_throughput_context_workbench(&value).unwrap();
        assert_eq!(receipt.disposition, "needs_refinement");
        assert!(!receipt.unknown_job_order.is_empty());
    }
    #[test]
    fn policy_denial_blocks_batch_actions() {
        let mut value = request();
        value.policy_allow = false;
        let receipt = render_throughput_context_workbench(&value).unwrap();
        assert_eq!(receipt.disposition, "blocked");
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn digest_is_stable() {
        let receipt = render_throughput_context_workbench(&request()).unwrap();
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
    #[test]
    fn job_locality_failure_blocks_release() {
        let mut input = request();
        input.jobs[0].raw_data_local = false;
        let receipt = render_throughput_context_workbench(&input).unwrap();
        assert_eq!(receipt.disposition, "blocked");
        assert!(receipt.raw_data_local);
        assert!(receipt.validate().is_ok());
    }
    #[test]
    fn workbench_artifact_payload_is_bound() {
        let mut receipt = render_throughput_context_workbench(&request()).unwrap();
        receipt.query_id = "query:tampered".into();
        assert!(receipt.validate().is_err());
    }
}
