//! Local single-study evidence-surveillance workflow fabric.
//!
//! Atlas feature `AFA-worldgen-P01-F13`: resumable local orchestration around
//! `EvidenceFeed1`, with deterministic stages, checkpoint identity, budget
//! admission, compensation, and replayable schedule receipts.

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceAvailability, EvidenceReference,
    EvidenceState, PolicyDecision, ResearchSurface, TypedPort, TypedResearchArtifact,
    PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-worldgen-P01-F13";
pub const CONTRACT_VERSION: &str = "worldgen-local-evidence-surveillance-workflow-fabric/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceFeed1@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet4@1";
const CANONICAL_STAGES: [&str; 4] = [
    "stage:checkpoint",
    "stage:persist-artifact",
    "stage:surveil-evidence",
    "stage:validate-input",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceFeedItem {
    pub source_id: String,
    pub source_type: String,
    pub locator: String,
    pub digest: Option<ContentHash>,
    pub availability: EvidenceAvailability,
    pub published_at: String,
    pub relevance_score: u16,
    pub negative_result: bool,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceFeedRequest {
    pub request_id: String,
    pub study_id: String,
    pub intent: String,
    pub required_source_ids: Vec<String>,
    pub feed: Vec<EvidenceFeedItem>,
    pub policy_decision: PolicyDecision,
    pub protected_closure_satisfied: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceSurveillanceDisposition {
    Passed,
    Blocked,
    Unknown,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualifiedEvidenceSet {
    pub schema_version: String,
    pub set_id: String,
    pub study_id: String,
    pub intent: String,
    pub selected_source_ids: Vec<String>,
    pub selected_source_digests: Vec<Option<ContentHash>>,
    pub evidence_state: EvidenceState,
    pub negative_source_ids: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub ordering_rule: String,
    pub boundary: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceSurveillanceReceipt {
    pub selected_source_ids: Vec<String>,
    pub disposition: EvidenceSurveillanceDisposition,
    pub qualified_set: QualifiedEvidenceSet,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
}
impl EvidenceSurveillanceReceipt {
    pub fn digest(&self) -> Result<ContentHash, String> {
        let value = serde_json::to_value(self).map_err(|e| e.to_string())?;
        ContentHash::of_value(&value).map_err(|e| e.to_string())
    }
}
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum EvidenceSurveillanceError {
    #[error("invalid evidence feed: {0}")]
    Invalid(String),
    #[error("evidence digest failed: {0}")]
    Digest(String),
}
fn run_evidence_surveillance(
    request: &EvidenceFeedRequest,
) -> Result<EvidenceSurveillanceReceipt, EvidenceSurveillanceError> {
    validate_feed_request(request)?;
    let mut feed = request.feed.clone();
    feed.sort_by(|a, b| {
        b.relevance_score
            .cmp(&a.relevance_score)
            .then(a.source_id.cmp(&b.source_id))
    });
    let available = feed
        .iter()
        .filter(|item| {
            item.availability == EvidenceAvailability::Available && item.digest.is_some()
        })
        .collect::<Vec<_>>();
    let selected_source_ids = available
        .iter()
        .map(|item| item.source_id.clone())
        .collect::<Vec<_>>();
    let selected_source_digests = available
        .iter()
        .map(|item| item.digest.clone())
        .collect::<Vec<_>>();
    let missing = request
        .required_source_ids
        .iter()
        .filter(|id| !selected_source_ids.contains(id))
        .cloned()
        .collect::<Vec<_>>();
    let mut omissions = missing
        .iter()
        .map(|id| format!("required evidence source unavailable: {id}"))
        .collect::<Vec<_>>();
    omissions.extend(
        feed.iter()
            .filter(|item| item.availability != EvidenceAvailability::Available)
            .map(|item| {
                format!(
                    "{} evidence source is unavailable: {}",
                    item.source_id, item.locator
                )
            }),
    );
    let mut uncertainty = Vec::new();
    if selected_source_ids.is_empty() {
        uncertainty.push("no available evidence source can support a qualified set".into());
    }
    let blocked = request.policy_decision != PolicyDecision::Allow
        || !request.protected_closure_satisfied
        || !request.raw_data_local;
    let disposition = if blocked {
        omissions.push(
            "policy, protected-closure, or raw-data-locality gate blocked the copilot".into(),
        );
        EvidenceSurveillanceDisposition::Blocked
    } else if selected_source_ids.is_empty() || !missing.is_empty() {
        EvidenceSurveillanceDisposition::Unknown
    } else {
        EvidenceSurveillanceDisposition::Passed
    };
    let negative_source_ids = available
        .iter()
        .filter(|item| item.negative_result)
        .map(|item| item.source_id.clone())
        .collect::<Vec<_>>();
    let qualified_set = QualifiedEvidenceSet {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        set_id: format!("worldgen-qualified-evidence:{}", request.request_id),
        study_id: request.study_id.clone(),
        intent: request.intent.clone(),
        selected_source_ids: selected_source_ids.clone(),
        selected_source_digests,
        evidence_state: if disposition == EvidenceSurveillanceDisposition::Passed {
            EvidenceState::Supported
        } else {
            EvidenceState::Unknown
        },
        negative_source_ids,
        omissions: omissions.clone(),
        uncertainty: uncertainty.clone(),
        ordering_rule: "relevance_score descending, source_id ascending".into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    Ok(EvidenceSurveillanceReceipt {
        selected_source_ids,
        disposition,
        qualified_set,
        omissions,
        uncertainty,
    })
}
fn validate_feed_request(request: &EvidenceFeedRequest) -> Result<(), EvidenceSurveillanceError> {
    if request.request_id.trim().is_empty()
        || request.study_id.trim().is_empty()
        || request.intent.trim().is_empty()
        || request.feed.is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(EvidenceSurveillanceError::Invalid(
            "evidence feed identity, intent, feed, and boundary are required".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for item in &request.feed {
        if item.source_id.trim().is_empty()
            || item.source_type.trim().is_empty()
            || item.locator.trim().is_empty()
            || item.published_at.trim().is_empty()
            || !ids.insert(item.source_id.clone())
        {
            return Err(EvidenceSurveillanceError::Invalid(
                "feed source identities and metadata must be non-empty and unique".into(),
            ));
        }
    }
    Ok(())
}

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

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum LocalEvidenceSurveillanceWorkflowError {
    #[error("invalid local evidence workflow request: {0}")]
    Invalid(String),
    #[error("local evidence workflow artifact failed: {0}")]
    Artifact(String),
    #[error("local evidence workflow engine failed: {0}")]
    Engine(String),
}

fn ordered(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
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
            || self.stage_order.is_empty()
            || self.plan_order.is_empty()
            || self.completed_order.is_empty()
            || self.effect_receipts.is_empty()
            || self.budget_units == 0
        {
            return Err(LocalEvidenceSurveillanceWorkflowError::Invalid(
                "workflow identity, stages, plan, locality, budget, or effects are incomplete"
                    .into(),
            ));
        }
        if self
            .qualified_order
            .iter()
            .chain(self.unknown_order.iter())
            .any(|id| !self.candidate_order.contains(id))
        {
            return Err(LocalEvidenceSurveillanceWorkflowError::Invalid(
                "workflow evidence state is not covered by candidates".into(),
            ));
        }
        for values in [
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
            if !ordered(values) {
                return Err(LocalEvidenceSurveillanceWorkflowError::Invalid(
                    "workflow ordering is not canonical".into(),
                ));
            }
        }
        for digest in [
            &self.evidence_receipt_digest,
            &self.checkpoint_digest,
            &self.workflow_digest,
            &self.replay_identity,
            &self.artifact.content_hash,
        ] {
            if digest.as_str().len() != 64 {
                return Err(LocalEvidenceSurveillanceWorkflowError::Invalid(
                    "workflow digest is invalid".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("schedule:research-work:")
                && !effect.starts_with("compensate:research-work:")
                && effect != "block:unsafe-release"
        }) {
            return Err(LocalEvidenceSurveillanceWorkflowError::Invalid(
                "workflow effect is outside schedule/compensation gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| LocalEvidenceSurveillanceWorkflowError::Artifact(error.to_string()))
    }
}

pub fn local_evidence_surveillance_workflow_fabric_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(), owner_crate: "worldgen".into(),
        consumers: ["research program lead".into(), "world-generation engineer".into()].into(),
        behavior: "schedule a checkpointed local EvidenceFeed1 workflow over structural benchmark worlds with deterministic stages, budget admission, compensation, and replay receipts".into(),
        value: "turn benchmark evidence surveillance into a resumable local operator workflow without hiding omissions, hidden-family gaps, or executing external effects".into(),
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
    validate_request(request)?;
    let evidence = run_evidence_surveillance(&request.request)
        .map_err(|error| LocalEvidenceSurveillanceWorkflowError::Engine(error.to_string()))?;
    let stage_order = CANONICAL_STAGES
        .iter()
        .map(|stage| (*stage).to_string())
        .collect::<Vec<_>>();
    let mut plan = BTreeSet::new();
    let mut completed = BTreeSet::new();
    let blocked = BTreeSet::new();
    let mut compensation = BTreeSet::new();
    for stage in &stage_order {
        plan.insert(format!("plan:{stage}"));
        completed.insert(stage.clone());
    }
    if evidence.selected_source_ids.is_empty() {
        plan.insert("plan:retain-unresolved-evidence".into());
        compensation.insert("compensate:research-work:retain-unresolved-evidence".into());
    } else {
        plan.insert("plan:publish-qualified-local-artifact".into());
    }
    let required_budget = plan.len() as u32;
    if request.budget_units < required_budget {
        plan.insert("plan:budget-review".into());
        compensation.insert("compensate:research-work:budget-exhausted".into());
    }
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
    let candidate_order = request
        .request
        .feed
        .iter()
        .map(|item| item.source_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let qualified_order = evidence.selected_source_ids.clone();
    let unknown_order = candidate_order
        .iter()
        .filter(|id| !qualified_order.contains(id))
        .cloned()
        .collect::<Vec<_>>();
    let plan_order = plan.into_iter().collect::<Vec<_>>();
    let completed_order = completed.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let compensation_order = compensation.into_iter().collect::<Vec<_>>();
    let mut omissions = omissions;
    omissions.sort();
    omissions.dedup();
    let checkpoint_digest=ContentHash::of_value(&json!({"workflow_id":request.workflow_id,"checkpoint_id":request.checkpoint_id,"stage_order":stage_order,"replay_identity":request.replay_identity})).map_err(|e|LocalEvidenceSurveillanceWorkflowError::Artifact(e.to_string()))?;
    let evidence_receipt_digest = evidence
        .digest()
        .map_err(|e| LocalEvidenceSurveillanceWorkflowError::Engine(e.to_string()))?;
    let workflow_digest=ContentHash::of_value(&json!({"workflow_id":request.workflow_id,"plan_order":plan_order,"completed_order":completed_order,"compensation_order":compensation_order,"checkpoint_digest":checkpoint_digest,"budget_units":request.budget_units,"replay_identity":request.replay_identity})).map_err(|e|LocalEvidenceSurveillanceWorkflowError::Artifact(e.to_string()))?;
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request.request_id,"workflow_id":request.workflow_id,"study_id":request.request.study_id,"scope":request.request.intent,"disposition":disposition,"stage_order":stage_order,"plan_order":plan_order,"completed_order":completed_order,"blocked_order":blocked_order,"compensation_order":compensation_order,"candidate_order":candidate_order,"qualified_order":qualified_order,"unknown_order":unknown_order,"evidence_receipt_digest":evidence_receipt_digest,"checkpoint_digest":checkpoint_digest,"workflow_digest":workflow_digest,"replay_identity":request.replay_identity,"budget_units":request.budget_units,"omissions":omissions,"uncertainty":uncertainty,"negative_evidence":negative,"boundary":PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("worldgen-evidence-workflow:{}", request.workflow_id),
        "application/vnd.aurora.worldgen.research-workflow-receipt+json",
        &payload,
        vec![],
        vec![],
    )
    .map_err(|e| LocalEvidenceSurveillanceWorkflowError::Artifact(e.to_string()))?;
    let effect_receipts = if disposition == EvidenceSurveillanceDisposition::Blocked {
        vec!["block:unsafe-release".into()]
    } else if !compensation_order.is_empty() {
        vec![format!("compensate:research-work:{}", request.workflow_id)]
    } else {
        vec![format!("schedule:research-work:{}", request.workflow_id)]
    };
    let receipt = LocalEvidenceSurveillanceWorkflowReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        study_id: request.request.study_id.clone(),
        scope: request.request.intent.clone(),
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
        omissions,
        uncertainty,
        negative_evidence: negative,
        effect_receipts,
        artifact,
        raw_data_local: request.raw_data_local,
        boundary: request.boundary.clone(),
    };
    receipt.validate()?;
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
    if request.replay_identity.as_str().len() != 64 {
        return Err(LocalEvidenceSurveillanceWorkflowError::Invalid(
            "replay identity is invalid".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
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
        assert!(r.effect_receipts[0].starts_with("block:"))
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
}
