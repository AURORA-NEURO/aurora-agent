//! Local single-study context compilation workflow fabric.
//!
//! Atlas feature: `AFA-brain-P03-F13`. This product turns context compilation
//! into a checkpointed, budgeted, deterministic workflow. It schedules only
//! typed local stages and retains compensation evidence when a policy, locality,
//! or budget gate prevents completion.

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-brain-P03-F13";
pub const CONTRACT_VERSION: &str = "brain-context-workflow-fabric/1.0";
const WORKFLOW_CONTENT_TYPE: &str = "application/vnd.aurora.context-workflow-receipt+json";
const MAX_TEXT_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextWorkflowStage {
    pub stage_id: String,
    pub depends_on: Vec<String>,
    pub budget_units: u32,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextWorkflowRequest {
    pub request_id: String,
    pub workflow_id: String,
    pub query_id: String,
    pub goal: String,
    pub checkpoint_id: String,
    pub stages: Vec<ContextWorkflowStage>,
    pub budget_units: u32,
    pub context_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextWorkflowReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub query_id: String,
    pub checkpoint_id: String,
    pub goal: String,
    pub disposition: String,
    pub stage_order: Vec<String>,
    pub plan_order: Vec<String>,
    pub completed_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub compensation_order: Vec<String>,
    pub checkpoint_digest: ContentHash,
    pub workflow_digest: ContentHash,
    pub context_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub budget_units: u32,
    pub consumed_budget_units: u32,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ContextWorkflowError {
    #[error("invalid context workflow request: {0}")]
    Invalid(String),
    #[error("context workflow artifact failed: {0}")]
    Artifact(String),
}

impl ContextWorkflowReceipt {
    pub fn validate(&self) -> Result<(), ContextWorkflowError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.query_id.trim().is_empty()
            || self.goal.trim().is_empty()
            || self.stage_order.is_empty()
            || self.plan_order.is_empty()
            || self.budget_units == 0
            || self.consumed_budget_units > self.budget_units
            || self.effect_receipts.is_empty()
        {
            return Err(ContextWorkflowError::Invalid(
                "workflow identity, stage plan, budget, locality, or effects are incomplete".into(),
            ));
        }
        for (value, field) in [
            (&self.request_id, "request_id"),
            (&self.workflow_id, "workflow_id"),
            (&self.query_id, "query_id"),
            (&self.checkpoint_id, "checkpoint_id"),
            (&self.goal, "goal"),
            (&self.disposition, "disposition"),
            (&self.boundary, "boundary"),
        ] {
            validate_text(value, field)?;
        }
        if !matches!(
            self.disposition.as_str(),
            "admitted" | "refinement_required" | "blocked"
        ) {
            return Err(ContextWorkflowError::Invalid(
                "workflow disposition is outside the contract".into(),
            ));
        }
        validate_unique(&self.stage_order, "stage_order")?;
        validate_unique(&self.completed_order, "completed_order")?;
        validate_sorted_unique(&self.plan_order, "plan_order")?;
        validate_sorted_unique(&self.blocked_order, "blocked_order")?;
        validate_sorted_unique(&self.compensation_order, "compensation_order")?;
        validate_sorted_unique(&self.omissions, "omissions")?;
        validate_sorted_unique(&self.uncertainty, "uncertainty")?;
        validate_sorted_unique(&self.negative_evidence, "negative_evidence")?;
        validate_sorted_unique(&self.effect_receipts, "effect_receipts")?;
        if self
            .completed_order
            .iter()
            .any(|stage| !self.stage_order.contains(stage))
            || self
                .blocked_order
                .iter()
                .any(|stage| !self.stage_order.contains(stage))
        {
            return Err(ContextWorkflowError::Invalid(
                "workflow stage coverage is invalid".into(),
            ));
        }
        let stages = identity_keys(&self.stage_order);
        let completed = identity_keys(&self.completed_order);
        let blocked = identity_keys(&self.blocked_order);
        if completed.union(&blocked).cloned().collect::<BTreeSet<_>>() != stages
            || !completed.is_disjoint(&blocked)
        {
            return Err(ContextWorkflowError::Invalid(
                "workflow stage states do not partition the plan".into(),
            ));
        }
        for digest in [
            &self.checkpoint_digest,
            &self.workflow_digest,
            &self.context_digest,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(ContextWorkflowError::Invalid(
                    "workflow digest is invalid".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("schedule:context-workflow:")
                && !effect.starts_with("compensate:context-workflow:")
                && effect != "block:unsafe-release"
        }) {
            return Err(ContextWorkflowError::Invalid(
                "workflow effect is outside schedule/compensation gate".into(),
            ));
        }
        let expected_effect_receipts = if self.disposition == "admitted" {
            vec![format!("schedule:context-workflow:{}", self.workflow_id)]
        } else {
            let mut effects = self.compensation_order.iter().cloned().collect::<Vec<_>>();
            effects.push("block:unsafe-release".into());
            effects.sort();
            effects
        };
        if self.effect_receipts != expected_effect_receipts {
            return Err(ContextWorkflowError::Invalid(
                "workflow effects do not match disposition and compensation".into(),
            ));
        }
        if !self.raw_data_local {
            return Err(ContextWorkflowError::Invalid(
                "context workflow receipts must declare local emitted data".into(),
            ));
        }
        let expected_checkpoint_digest = ContentHash::of_value(&json!({
            "workflow_id": self.workflow_id,
            "checkpoint_id": self.checkpoint_id,
            "stage_order": self.stage_order,
            "completed_order": self.completed_order,
            "blocked_order": self.blocked_order,
            "disposition": self.disposition,
            "replay_identity": self.replay_identity,
            "raw_data_local": self.raw_data_local,
        }))
        .map_err(|error| ContextWorkflowError::Artifact(error.to_string()))?;
        if self.checkpoint_digest != expected_checkpoint_digest {
            return Err(ContextWorkflowError::Invalid(
                "workflow checkpoint is not bound to stage outcomes".into(),
            ));
        }
        let expected_workflow_digest = ContentHash::of_value(&json!({
            "workflow_id": self.workflow_id,
            "plan_order": self.plan_order,
            "checkpoint_digest": self.checkpoint_digest,
            "budget_units": self.budget_units,
            "consumed_budget_units": self.consumed_budget_units,
            "disposition": self.disposition,
            "context_digest": self.context_digest,
            "replay_identity": self.replay_identity,
            "raw_data_local": self.raw_data_local,
        }))
        .map_err(|error| ContextWorkflowError::Artifact(error.to_string()))?;
        if self.workflow_digest != expected_workflow_digest {
            return Err(ContextWorkflowError::Invalid(
                "workflow digest is not bound to execution state".into(),
            ));
        }
        let expected_artifact_id = format!("brain-context-workflow:{}", self.workflow_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != WORKFLOW_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(ContextWorkflowError::Invalid(
                "workflow artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ContextWorkflowError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| ContextWorkflowError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, ContextWorkflowError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ContextWorkflowError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ContextWorkflowError::Artifact(error.to_string()))
    }
}

pub fn context_workflow_fabric_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(),
        consumers: ["research workflow operator".into(), "platform reliability engineer".into()].into(), behavior: "schedules deterministic checkpointed context-compilation stages with dependency order, bounded budget, and compensation receipts".into(), value: "makes a local Decision-Section preparation workflow resumable and auditable without hiding incomplete context or executing external effects".into(),
        inputs: vec![TypedPort { name: "context_workflow_request".into(), schema: "ResearchWorkflowSpec1@1".into(), required: true }], outputs: vec![TypedPort { name: "context_workflow_receipt".into(), schema: "ContextWorkflowReceipt1@1".into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["schedule:context-workflow".into(), "read:local-research-artifacts".into()].into(), determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

fn topological_order(stages: &[ContextWorkflowStage]) -> Result<Vec<String>, ContextWorkflowError> {
    let ids = stages
        .iter()
        .map(|stage| stage.stage_id.clone())
        .collect::<BTreeSet<_>>();
    if ids.len() != stages.len() || ids.iter().any(|id| id.trim().is_empty()) {
        return Err(ContextWorkflowError::Invalid(
            "workflow stage identifiers must be unique and non-empty".into(),
        ));
    }
    let mut indegree = BTreeMap::new();
    let mut outgoing: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for stage in stages {
        let dependencies = stage.depends_on.iter().cloned().collect::<BTreeSet<_>>();
        if dependencies.len() != stage.depends_on.len()
            || dependencies.contains(&stage.stage_id)
            || dependencies
                .iter()
                .any(|dependency| !ids.contains(dependency))
        {
            return Err(ContextWorkflowError::Invalid(format!(
                "stage {} has invalid dependencies",
                stage.stage_id
            )));
        }
        indegree.insert(stage.stage_id.clone(), dependencies.len());
        for dependency in dependencies {
            outgoing
                .entry(dependency)
                .or_default()
                .insert(stage.stage_id.clone());
        }
    }
    let mut ready = indegree
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(id, _)| id.clone())
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(stages.len());
    while let Some(id) = ready.pop_first() {
        order.push(id.clone());
        if let Some(children) = outgoing.get(&id) {
            for child in children {
                let Some(degree) = indegree.get_mut(child) else {
                    return Err(ContextWorkflowError::Invalid(
                        "workflow adjacency references an uninitialized stage".into(),
                    ));
                };
                *degree -= 1;
                if *degree == 0 {
                    ready.insert(child.clone());
                }
            }
        }
    }
    if order.len() != stages.len() {
        return Err(ContextWorkflowError::Invalid(
            "workflow dependency cycle detected".into(),
        ));
    }
    Ok(order)
}

pub fn compile_context_workflow(
    request: &ContextWorkflowRequest,
) -> Result<ContextWorkflowReceipt, ContextWorkflowError> {
    if request.request_id.trim().is_empty()
        || request.workflow_id.trim().is_empty()
        || request.query_id.trim().is_empty()
        || request.goal.trim().is_empty()
        || request.checkpoint_id.trim().is_empty()
        || request.stages.is_empty()
        || request.budget_units == 0
        || request.context_digest.as_str().len() != 64
        || request.replay_identity.as_str().len() != 64
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(ContextWorkflowError::Invalid(
            "workflow identity, stages, budget, replay, or boundary is invalid".into(),
        ));
    }
    for (value, field) in [
        (&request.request_id, "request_id"),
        (&request.workflow_id, "workflow_id"),
        (&request.query_id, "query_id"),
        (&request.goal, "goal"),
        (&request.checkpoint_id, "checkpoint_id"),
        (&request.boundary, "boundary"),
    ] {
        validate_text(value, field)?;
    }
    for stage in &request.stages {
        validate_text(&stage.stage_id, "stage.stage_id")?;
        validate_unique(&stage.depends_on, "stage.depends_on")?;
    }
    let stage_order = topological_order(&request.stages)?;
    let stage_map = request
        .stages
        .iter()
        .map(|stage| (stage.stage_id.clone(), stage))
        .collect::<BTreeMap<_, _>>();
    let mut plan = BTreeSet::new();
    for stage in &stage_order {
        plan.insert(format!("plan:execute:{}", stage));
    }
    let total_budget = request
        .stages
        .iter()
        .try_fold(0u32, |total, stage| total.checked_add(stage.budget_units))
        .ok_or_else(|| ContextWorkflowError::Invalid("workflow stage budget overflow".into()))?;
    let locality_gate = request.raw_data_local;
    let gates_open = request.policy_allow && request.protected_closure && locality_gate;
    let mut completed = Vec::new();
    let mut blocked = BTreeSet::new();
    let mut consumed = 0u32;
    for stage_id in &stage_order {
        let stage = stage_map[stage_id];
        if !gates_open
            || consumed.checked_add(stage.budget_units).is_none()
            || consumed + stage.budget_units > request.budget_units
        {
            blocked.insert(stage_id.clone());
        } else {
            consumed += stage.budget_units;
            completed.push(stage_id.clone());
        }
    }
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let negative = BTreeSet::new();
    if request.budget_units < total_budget {
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
    if !blocked.is_empty() {
        uncertainty.insert("workflow:blocked-stages-retained-for-replay".into());
    }
    let disposition = if !gates_open {
        "blocked"
    } else if blocked.is_empty() {
        "admitted"
    } else {
        "refinement_required"
    };
    let mut compensation = BTreeSet::new();
    if !completed.is_empty() && disposition != "admitted" {
        compensation.insert(format!(
            "compensate:context-workflow:{}:retain-checkpoint",
            request.workflow_id
        ));
    }
    let stage_order = stage_order;
    let plan_order = plan.into_iter().collect::<Vec<_>>();
    let completed_order = completed;
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let compensation_order = compensation.into_iter().collect::<Vec<_>>();
    let omissions = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence = negative.into_iter().collect::<Vec<_>>();
    let raw_data_local = true;
    let checkpoint_digest = ContentHash::of_value(&json!({"workflow_id": request.workflow_id, "checkpoint_id": request.checkpoint_id, "stage_order": stage_order, "completed_order": completed_order, "blocked_order": blocked_order, "disposition": disposition, "replay_identity": request.replay_identity, "raw_data_local": raw_data_local})).map_err(|error| ContextWorkflowError::Artifact(error.to_string()))?;
    let workflow_digest = ContentHash::of_value(&json!({"workflow_id": request.workflow_id, "plan_order": plan_order, "checkpoint_digest": checkpoint_digest, "budget_units": request.budget_units, "consumed_budget_units": consumed, "disposition": disposition, "context_digest": request.context_digest, "replay_identity": request.replay_identity, "raw_data_local": raw_data_local})).map_err(|error| ContextWorkflowError::Artifact(error.to_string()))?;
    let effect_receipts = if disposition == "admitted" {
        vec![format!("schedule:context-workflow:{}", request.workflow_id)]
    } else {
        let mut effects = compensation_order.iter().cloned().collect::<Vec<_>>();
        effects.push("block:unsafe-release".into());
        effects.sort();
        effects
    };
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "workflow_id": request.workflow_id, "query_id": request.query_id, "checkpoint_id": request.checkpoint_id, "goal": request.goal, "disposition": disposition, "stage_order": stage_order, "plan_order": plan_order, "completed_order": completed_order, "blocked_order": blocked_order, "compensation_order": compensation_order, "checkpoint_digest": checkpoint_digest, "workflow_digest": workflow_digest, "context_digest": request.context_digest, "replay_identity": request.replay_identity, "budget_units": request.budget_units, "consumed_budget_units": consumed, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative_evidence, "effect_receipts": effect_receipts, "raw_data_local": raw_data_local, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-context-workflow:{}", request.workflow_id),
        WORKFLOW_CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ContextWorkflowError::Artifact(error.to_string()))?;
    let receipt = ContextWorkflowReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        query_id: request.query_id.clone(),
        checkpoint_id: request.checkpoint_id.clone(),
        goal: request.goal.clone(),
        disposition: disposition.into(),
        stage_order,
        plan_order,
        completed_order,
        blocked_order,
        compensation_order,
        checkpoint_digest,
        workflow_digest,
        context_digest: request.context_digest.clone(),
        replay_identity: request.replay_identity.clone(),
        budget_units: request.budget_units,
        consumed_budget_units: consumed,
        omissions,
        uncertainty,
        negative_evidence,
        effect_receipts,
        artifact,
        raw_data_local,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_text(value: &str, field: &str) -> Result<(), ContextWorkflowError> {
    if value.trim().is_empty()
        || value != value.trim()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(ContextWorkflowError::Invalid(format!(
            "{field} must be bounded, non-empty text without padding or control characters"
        )));
    }
    Ok(())
}

fn validate_unique(values: &[String], field: &str) -> Result<(), ContextWorkflowError> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_text(value, field)?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(ContextWorkflowError::Invalid(format!(
                "{field} contains duplicate or case-colliding values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_unique(values: &[String], field: &str) -> Result<(), ContextWorkflowError> {
    validate_unique(values, field)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ContextWorkflowError::Invalid(format!(
            "{field} is not in canonical order"
        )));
    }
    Ok(())
}

fn identity_keys(values: &[String]) -> BTreeSet<String> {
    values
        .iter()
        .map(|value| value.to_ascii_lowercase())
        .collect()
}

fn receipt_payload(receipt: &ContextWorkflowReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "workflow_id": receipt.workflow_id,
        "query_id": receipt.query_id,
        "checkpoint_id": receipt.checkpoint_id,
        "goal": receipt.goal,
        "disposition": receipt.disposition,
        "stage_order": receipt.stage_order,
        "plan_order": receipt.plan_order,
        "completed_order": receipt.completed_order,
        "blocked_order": receipt.blocked_order,
        "compensation_order": receipt.compensation_order,
        "checkpoint_digest": receipt.checkpoint_digest,
        "workflow_digest": receipt.workflow_digest,
        "context_digest": receipt.context_digest,
        "replay_identity": receipt.replay_identity,
        "budget_units": receipt.budget_units,
        "consumed_budget_units": receipt.consumed_budget_units,
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
    fn request() -> ContextWorkflowRequest {
        ContextWorkflowRequest {
            request_id: "request:context-workflow".into(),
            workflow_id: "workflow:one".into(),
            query_id: "query:one".into(),
            goal: "compile one study context".into(),
            checkpoint_id: "checkpoint:zero".into(),
            stages: vec![
                ContextWorkflowStage {
                    stage_id: "stage:compile".into(),
                    depends_on: vec!["stage:validate".into()],
                    budget_units: 2,
                    required: true,
                },
                ContextWorkflowStage {
                    stage_id: "stage:validate".into(),
                    depends_on: Vec::new(),
                    budget_units: 1,
                    required: true,
                },
            ],
            budget_units: 3,
            context_digest: hash("context"),
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            context_workflow_fabric_manifest().autonomy_tier,
            AutonomyTier::A1
        );
    }
    #[test]
    fn topological_plan_is_deterministic() {
        let receipt = compile_context_workflow(&request()).unwrap();
        assert_eq!(receipt.disposition, "admitted");
        assert_eq!(receipt.stage_order, vec!["stage:validate", "stage:compile"]);
    }
    #[test]
    fn budget_retains_blocked_stage_and_compensation() {
        let mut value = request();
        value.budget_units = 1;
        let receipt = compile_context_workflow(&value).unwrap();
        assert_eq!(receipt.disposition, "refinement_required");
        assert_eq!(receipt.blocked_order.len(), 1);
        assert!(!receipt.compensation_order.is_empty());
    }
    #[test]
    fn policy_denial_blocks() {
        let mut value = request();
        value.policy_allow = false;
        let receipt = compile_context_workflow(&value).unwrap();
        assert_eq!(receipt.disposition, "blocked");
        assert!(receipt
            .effect_receipts
            .iter()
            .any(|effect| effect == "block:unsafe-release"));
    }
    #[test]
    fn dependency_cycle_is_rejected() {
        let mut value = request();
        value.stages[1].depends_on = vec!["stage:compile".into()];
        assert!(
            matches!(compile_context_workflow(&value), Err(ContextWorkflowError::Invalid(message)) if message.contains("cycle"))
        );
    }
    #[test]
    fn digest_is_stable() {
        let receipt = compile_context_workflow(&request()).unwrap();
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
    #[test]
    fn locality_failure_is_blocked_and_retained() {
        let mut value = request();
        value.raw_data_local = false;
        let receipt = compile_context_workflow(&value).unwrap();
        assert_eq!(receipt.disposition, "blocked");
        assert!(receipt.raw_data_local);
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item == "workflow:raw-data-locality-failed"));
        assert!(receipt.validate().is_ok());
    }
    #[test]
    fn workflow_artifact_payload_is_bound() {
        let mut receipt = compile_context_workflow(&request()).unwrap();
        receipt.goal = "tampered goal".into();
        assert!(receipt.validate().is_err());
    }
}
