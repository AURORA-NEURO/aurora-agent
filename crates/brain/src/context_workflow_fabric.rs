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
            || !self.raw_data_local
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
            return Err(ContextWorkflowError::Invalid("workflow identity, stage plan, budget, locality, or effects are incomplete".into()));
        }
        if self.stage_order.iter().any(|stage| stage.trim().is_empty())
            || self.stage_order.windows(2).any(|pair| pair[0] == pair[1])
            || self.completed_order.iter().any(|stage| !self.stage_order.contains(stage))
            || self.blocked_order.iter().any(|stage| !self.stage_order.contains(stage))
        {
            return Err(ContextWorkflowError::Invalid("workflow stage coverage is invalid".into()));
        }
        for values in [&self.plan_order, &self.blocked_order, &self.compensation_order, &self.omissions, &self.uncertainty, &self.negative_evidence, &self.effect_receipts] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(ContextWorkflowError::Invalid("workflow vectors are not canonical".into()));
            }
        }
        if self.completed_order.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(ContextWorkflowError::Invalid("workflow completed order contains duplicates".into()));
        }
        if self.completed_order.iter().any(|stage| self.blocked_order.contains(stage)) {
            return Err(ContextWorkflowError::Invalid("workflow stage is both completed and blocked".into()));
        }
        for digest in [&self.checkpoint_digest, &self.workflow_digest, &self.context_digest, &self.replay_identity] {
            if digest.as_str().len() != 64 {
                return Err(ContextWorkflowError::Invalid("workflow digest is invalid".into()));
            }
        }
        if self.effect_receipts.iter().any(|effect| !effect.starts_with("schedule:context-workflow:") && !effect.starts_with("compensate:context-workflow:") && effect != "block:unsafe-release") {
            return Err(ContextWorkflowError::Invalid("workflow effect is outside schedule/compensation gate".into()));
        }
        self.artifact.validate_metadata().map_err(|error| ContextWorkflowError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, ContextWorkflowError> {
        self.validate()?;
        let value = serde_json::to_value(self).map_err(|error| ContextWorkflowError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value).map_err(|error| ContextWorkflowError::Artifact(error.to_string()))
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
    let ids = stages.iter().map(|stage| stage.stage_id.clone()).collect::<BTreeSet<_>>();
    if ids.len() != stages.len() || ids.iter().any(|id| id.trim().is_empty()) {
        return Err(ContextWorkflowError::Invalid("workflow stage identifiers must be unique and non-empty".into()));
    }
    let mut indegree = BTreeMap::new();
    let mut outgoing: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for stage in stages {
        let dependencies = stage.depends_on.iter().cloned().collect::<BTreeSet<_>>();
        if dependencies.len() != stage.depends_on.len() || dependencies.contains(&stage.stage_id) || dependencies.iter().any(|dependency| !ids.contains(dependency)) {
            return Err(ContextWorkflowError::Invalid(format!("stage {} has invalid dependencies", stage.stage_id)));
        }
        indegree.insert(stage.stage_id.clone(), dependencies.len());
        for dependency in dependencies { outgoing.entry(dependency).or_default().insert(stage.stage_id.clone()); }
    }
    let mut ready = indegree.iter().filter(|(_, degree)| **degree == 0).map(|(id, _)| id.clone()).collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(stages.len());
    while let Some(id) = ready.pop_first() {
        order.push(id.clone());
        if let Some(children) = outgoing.get(&id) { for child in children { let degree = indegree.get_mut(child).expect("child exists"); *degree -= 1; if *degree == 0 { ready.insert(child.clone()); } } }
    }
    if order.len() != stages.len() { return Err(ContextWorkflowError::Invalid("workflow dependency cycle detected".into())); }
    Ok(order)
}

pub fn compile_context_workflow(request: &ContextWorkflowRequest) -> Result<ContextWorkflowReceipt, ContextWorkflowError> {
    if request.request_id.trim().is_empty() || request.workflow_id.trim().is_empty() || request.query_id.trim().is_empty() || request.goal.trim().is_empty() || request.checkpoint_id.trim().is_empty() || request.stages.is_empty() || request.budget_units == 0 || request.context_digest.as_str().len() != 64 || request.replay_identity.as_str().len() != 64 || request.boundary != PRECLINICAL_BOUNDARY {
        return Err(ContextWorkflowError::Invalid("workflow identity, stages, budget, replay, or boundary is invalid".into()));
    }
    let stage_order = topological_order(&request.stages)?;
    let stage_map = request.stages.iter().map(|stage| (stage.stage_id.clone(), stage)).collect::<BTreeMap<_, _>>();
    let mut plan = BTreeSet::new();
    for stage in &stage_order { plan.insert(format!("plan:execute:{}", stage)); }
    let total_budget = request.stages.iter().try_fold(0u32, |total, stage| total.checked_add(stage.budget_units)).ok_or_else(|| ContextWorkflowError::Invalid("workflow stage budget overflow".into()))?;
    let gates_open = request.policy_allow && request.protected_closure && request.raw_data_local;
    let mut completed = Vec::new();
    let mut blocked = BTreeSet::new();
    let mut consumed = 0u32;
    for stage_id in &stage_order {
        let stage = stage_map[stage_id];
        if !gates_open || consumed.checked_add(stage.budget_units).is_none() || consumed + stage.budget_units > request.budget_units { blocked.insert(stage_id.clone()); } else { consumed += stage.budget_units; completed.push(stage_id.clone()); }
    }
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let negative = BTreeSet::new();
    if request.budget_units < total_budget { omissions.insert("workflow:budget-exhausted".into()); }
    if !request.policy_allow { omissions.insert("workflow:policy-denied".into()); }
    if !request.protected_closure { omissions.insert("workflow:protected-closure-incomplete".into()); }
    if !request.raw_data_local { omissions.insert("workflow:raw-data-locality-failed".into()); }
    if !blocked.is_empty() { uncertainty.insert("workflow:blocked-stages-retained-for-replay".into()); }
    let disposition = if !gates_open { "blocked" } else if blocked.is_empty() { "admitted" } else { "refinement_required" };
    let mut compensation = BTreeSet::new();
    if !completed.is_empty() && disposition != "admitted" { compensation.insert(format!("compensate:context-workflow:{}:retain-checkpoint", request.workflow_id)); }
    let checkpoint_digest = ContentHash::of_value(&json!({"workflow_id": request.workflow_id, "checkpoint_id": request.checkpoint_id, "stage_order": stage_order, "completed_order": completed, "blocked_order": blocked, "replay_identity": request.replay_identity})).map_err(|error| ContextWorkflowError::Artifact(error.to_string()))?;
    let workflow_digest = ContentHash::of_value(&json!({"workflow_id": request.workflow_id, "plan_order": plan, "checkpoint_digest": checkpoint_digest, "budget_units": request.budget_units, "consumed_budget_units": consumed, "replay_identity": request.replay_identity})).map_err(|error| ContextWorkflowError::Artifact(error.to_string()))?;
    let effect_receipts = if disposition == "admitted" { vec![format!("schedule:context-workflow:{}", request.workflow_id)] } else { let mut effects = compensation.iter().cloned().collect::<Vec<_>>(); effects.push("block:unsafe-release".into()); effects.sort(); effects };
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "workflow_id": request.workflow_id, "query_id": request.query_id, "goal": request.goal, "disposition": disposition, "stage_order": stage_order, "plan_order": plan, "completed_order": completed, "blocked_order": blocked, "compensation_order": compensation, "checkpoint_digest": checkpoint_digest, "workflow_digest": workflow_digest, "context_digest": request.context_digest, "replay_identity": request.replay_identity, "budget_units": request.budget_units, "consumed_budget_units": consumed, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(format!("brain-context-workflow:{}", request.workflow_id), "application/vnd.aurora.context-workflow-receipt+json", &payload, Vec::new(), Vec::new()).map_err(|error| ContextWorkflowError::Artifact(error.to_string()))?;
    let receipt = ContextWorkflowReceipt { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), contract_version: CONTRACT_VERSION.into(), feature_id: FEATURE_ID.into(), request_id: request.request_id.clone(), workflow_id: request.workflow_id.clone(), query_id: request.query_id.clone(), goal: request.goal.clone(), disposition: disposition.into(), stage_order, plan_order: plan.into_iter().collect(), completed_order: completed, blocked_order: blocked.into_iter().collect(), compensation_order: compensation.into_iter().collect(), checkpoint_digest, workflow_digest, context_digest: request.context_digest.clone(), replay_identity: request.replay_identity.clone(), budget_units: request.budget_units, consumed_budget_units: consumed, omissions: omissions.into_iter().collect(), uncertainty: uncertainty.into_iter().collect(), negative_evidence: negative.into_iter().collect(), effect_receipts, artifact, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY.into() };
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash { ContentHash::of_bytes(value.as_bytes()) }
    fn request() -> ContextWorkflowRequest { ContextWorkflowRequest { request_id: "request:context-workflow".into(), workflow_id: "workflow:one".into(), query_id: "query:one".into(), goal: "compile one study context".into(), checkpoint_id: "checkpoint:zero".into(), stages: vec![ContextWorkflowStage { stage_id: "stage:compile".into(), depends_on: vec!["stage:validate".into()], budget_units: 2, required: true }, ContextWorkflowStage { stage_id: "stage:validate".into(), depends_on: Vec::new(), budget_units: 1, required: true }], budget_units: 3, context_digest: hash("context"), replay_identity: hash("replay"), policy_allow: true, protected_closure: true, raw_data_local: true, boundary: PRECLINICAL_BOUNDARY.into() } }
    #[test] fn manifest_is_a1() { assert_eq!(context_workflow_fabric_manifest().autonomy_tier, AutonomyTier::A1); }
    #[test] fn topological_plan_is_deterministic() { let receipt = compile_context_workflow(&request()).unwrap(); assert_eq!(receipt.disposition, "admitted"); assert_eq!(receipt.stage_order, vec!["stage:validate", "stage:compile"]); }
    #[test] fn budget_retains_blocked_stage_and_compensation() { let mut value = request(); value.budget_units = 1; let receipt = compile_context_workflow(&value).unwrap(); assert_eq!(receipt.disposition, "refinement_required"); assert_eq!(receipt.blocked_order.len(), 1); assert!(!receipt.compensation_order.is_empty()); }
    #[test] fn policy_denial_blocks() { let mut value = request(); value.policy_allow = false; let receipt = compile_context_workflow(&value).unwrap(); assert_eq!(receipt.disposition, "blocked"); assert!(receipt.effect_receipts.iter().any(|effect| effect == "block:unsafe-release")); }
    #[test] fn dependency_cycle_is_rejected() { let mut value = request(); value.stages[1].depends_on = vec!["stage:compile".into()]; assert!(matches!(compile_context_workflow(&value), Err(ContextWorkflowError::Invalid(message)) if message.contains("cycle"))); }
    #[test] fn digest_is_stable() { let receipt = compile_context_workflow(&request()).unwrap(); assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap()); }
}
