//! Local experiment-design workflow fabric (`AFA-bioethics-P09-F13`).
//!
//! This product compiles a typed, caller-provided design into a deterministic resumable schedule.
//! It checks dependency closure, power/evidence metadata, policy, approval, budget, and locality;
//! it does not execute experiments or instruments.

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-bioethics-P09-F13";
pub const CONTRACT_VERSION: &str =
    "bioethics-local-single-study-experiment-design-workflow-fabric/1.0";
pub const INPUT_SCHEMA: &str = "ExperimentObjective1@1";
pub const OUTPUT_SCHEMA: &str = "ExecutableExperimentDesign4@1";
pub const CONTENT_TYPE: &str =
    "application/vnd.aurora.bioethics-executable-experiment-design-4+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkflowStep1 {
    pub step_id: String,
    pub depends_on: Vec<String>,
    pub declared_effect: String,
    pub duration_budget: u32,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentObjective1 {
    pub objective_id: String,
    pub target_scope: String,
    pub semantic_profile: String,
    pub evidence_state: EvidenceState,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub power_milli: u16,
    pub steps: Vec<WorkflowStep1>,
    pub omission_order: Vec<String>,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentDesignWorkflowRequest1 {
    pub schema_version: String,
    pub request_id: String,
    pub consumer: String,
    pub purpose: String,
    pub target_scope: String,
    pub semantic_profile: String,
    pub minimum_power_milli: u16,
    pub replay_identity: ContentHash,
    pub policy_allowed: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub budget: u32,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
    pub objective: ExperimentObjective1,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutableExperimentDesignArtifact4 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutableExperimentDesign4 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub consumer: String,
    pub purpose: String,
    pub target_scope: String,
    pub semantic_profile: String,
    pub disposition: String,
    pub step_order: Vec<String>,
    pub ready_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub design_digest: ContentHash,
    pub artifact: ExecutableExperimentDesignArtifact4,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ExperimentDesignWorkflowError {
    #[error("invalid experiment-design workflow request or receipt: {0}")]
    Invalid(String),
    #[error("experiment-design workflow artifact failed: {0}")]
    Artifact(String),
}

fn ordered(v: &[String]) -> bool {
    v.windows(2).all(|w| w[0] < w[1])
}
fn valid_digest(v: &ContentHash) -> bool {
    v.as_str().len() == 64 && v.as_str().bytes().all(|b| b.is_ascii_hexdigit())
}

pub fn experiment_design_workflow_fabric_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "bioethics".into(),
        consumers: ["research data steward".into(), "workflow operator".into(), "protocol scientist".into()].into(),
        behavior: "compile typed preclinical experiment objectives into deterministic, resumable workflow schedules with evidence and policy gates".into(),
        value: "gives research teams a replayable design schedule while preventing unsupported, cyclic, or unauthorized experimental work from being released".into(),
        inputs: vec![TypedPort { name: "experiment_objective".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "executable_experiment_design".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: [Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["execute:approved-workflows".into()].into(), determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "cwl".into(), state: EvidenceState::Supported, locator: Some("https://www.commonwl.org/specification/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Protocol, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

fn topological(steps: &[WorkflowStep1]) -> Result<Vec<String>, ExperimentDesignWorkflowError> {
    let ids = steps
        .iter()
        .map(|s| s.step_id.clone())
        .collect::<BTreeSet<_>>();
    if ids.len() != steps.len() || ids.iter().any(|x| x.trim().is_empty()) {
        return Err(ExperimentDesignWorkflowError::Invalid(
            "step ids must be unique and non-empty".into(),
        ));
    }
    let by_id = steps
        .iter()
        .map(|s| (s.step_id.clone(), s))
        .collect::<BTreeMap<_, _>>();
    let mut indegree = ids
        .iter()
        .map(|id| (id.clone(), 0usize))
        .collect::<BTreeMap<_, _>>();
    let mut edges: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for s in steps {
        if !ordered(&s.depends_on)
            || s.declared_effect.trim().is_empty()
            || s.duration_budget == 0
            || s.depends_on.iter().any(|d| !ids.contains(d))
        {
            return Err(ExperimentDesignWorkflowError::Invalid(format!(
                "invalid dependencies or budget for step {}",
                s.step_id
            )));
        }
        for d in &s.depends_on {
            edges
                .entry(d.clone())
                .or_default()
                .insert(s.step_id.clone());
            *indegree.get_mut(&s.step_id).expect("known id") += 1;
        }
    }
    let mut ready = indegree
        .iter()
        .filter(|(_, n)| **n == 0)
        .map(|(id, _)| id.clone())
        .collect::<BTreeSet<_>>();
    let mut out = Vec::new();
    while let Some(id) = ready.iter().next().cloned() {
        ready.remove(&id);
        out.push(id.clone());
        if let Some(children) = edges.get(&id) {
            for child in children {
                let n = indegree.get_mut(child).expect("known child");
                *n -= 1;
                if *n == 0 {
                    ready.insert(child.clone());
                }
            }
        }
    }
    if out.len() != by_id.len() {
        return Err(ExperimentDesignWorkflowError::Invalid(
            "workflow dependency cycle detected".into(),
        ));
    }
    Ok(out)
}

impl ExecutableExperimentDesign4 {
    pub fn validate(&self) -> Result<(), ExperimentDesignWorkflowError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.content_type != CONTENT_TYPE
            || !self.raw_data_local
            || !self.aggregate_only
            || !matches!(
                self.disposition.as_str(),
                "qualified" | "partial" | "blocked"
            )
            || self.step_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(ExperimentDesignWorkflowError::Invalid(
                "design identity, locality, steps, disposition, or effects are incomplete".into(),
            ));
        }
        for v in [
            &self.step_order,
            &self.ready_order,
            &self.blocked_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !ordered(v) {
                return Err(ExperimentDesignWorkflowError::Invalid(
                    "design ordering is not canonical".into(),
                ));
            }
        }
        let ids = self.step_order.iter().cloned().collect::<BTreeSet<_>>();
        let states = self
            .ready_order
            .iter()
            .chain(&self.blocked_order)
            .cloned()
            .collect::<Vec<_>>();
        if ids.len() != self.step_order.len()
            || states.len() != ids.len()
            || states.iter().cloned().collect::<BTreeSet<_>>() != ids
        {
            return Err(ExperimentDesignWorkflowError::Invalid(
                "step states do not partition".into(),
            ));
        }
        if !valid_digest(&self.replay_identity)
            || !valid_digest(&self.design_digest)
            || self.artifact.content_hash != self.design_digest
            || !self.artifact.provenance_digests.iter().all(valid_digest)
        {
            return Err(ExperimentDesignWorkflowError::Artifact(
                "design digest is inconsistent".into(),
            ));
        }
        if self
            .effect_receipts
            .iter()
            .any(|e| e != "block:unsafe-release" && !e.starts_with("schedule:research-work:"))
        {
            return Err(ExperimentDesignWorkflowError::Invalid(
                "design effect is outside workflow gate".into(),
            ));
        }
        if self.disposition == "qualified"
            && self.effect_receipts != [format!("schedule:research-work:{}", self.request_id)]
        {
            return Err(ExperimentDesignWorkflowError::Invalid(
                "qualified design effect is invalid".into(),
            ));
        }
        if self.disposition != "qualified" && self.effect_receipts != ["block:unsafe-release"] {
            return Err(ExperimentDesignWorkflowError::Invalid(
                "non-qualified design must block".into(),
            ));
        }
        Ok(())
    }
}

pub fn compile_experiment_design_workflow(
    r: &ExperimentDesignWorkflowRequest1,
) -> Result<ExecutableExperimentDesign4, ExperimentDesignWorkflowError> {
    if r.schema_version != INPUT_SCHEMA
        || [
            &r.request_id,
            &r.consumer,
            &r.purpose,
            &r.target_scope,
            &r.semantic_profile,
        ]
        .iter()
        .any(|v| v.trim().is_empty())
        || r.minimum_power_milli == 0
        || !valid_digest(&r.replay_identity)
        || r.budget == 0
        || r.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(ExperimentDesignWorkflowError::Invalid(
            "workflow identity, bounds, replay, budget, or boundary is invalid".into(),
        ));
    }
    let o = &r.objective;
    if o.objective_id.trim().is_empty()
        || o.target_scope != r.target_scope
        || o.semantic_profile != r.semantic_profile
        || !valid_digest(&o.artifact_digest)
        || !valid_digest(&o.provenance_digest)
        || o.replay_identity != r.replay_identity
        || o.power_milli < r.minimum_power_milli
        || o.steps.is_empty()
        || !ordered(&o.omission_order)
    {
        return Err(ExperimentDesignWorkflowError::Invalid(
            "objective scope, evidence, power, replay, or artifact closure is invalid".into(),
        ));
    }
    let _execution_order = topological(&o.steps)?;
    let step_order = o
        .steps
        .iter()
        .map(|step| step.step_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut ready = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut omissions = o.omission_order.iter().cloned().collect::<BTreeSet<_>>();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    if o.negative_result {
        negative.insert(format!("{}:negative-result", o.objective_id));
    }
    let total: u32 = o.steps.iter().map(|s| s.duration_budget).sum();
    for s in &o.steps {
        if s.required && total > r.budget {
            blocked.insert(s.step_id.clone());
            omissions.insert(format!("{}:budget-exhausted", s.step_id));
        } else if !matches!(
            o.evidence_state,
            EvidenceState::Proven | EvidenceState::Supported
        ) {
            blocked.insert(s.step_id.clone());
            uncertainty.insert(format!("{}:evidence-state", s.step_id));
        } else {
            ready.insert(s.step_id.clone());
        }
    }
    for (ok, label) in [
        (r.policy_allowed, "workflow:policy-denied"),
        (r.protected_closure, "workflow:protected-closure-incomplete"),
        (r.signed_approval, "workflow:signed-approval-missing"),
        (r.raw_data_local, "workflow:raw-data-not-local"),
        (r.aggregate_only, "workflow:aggregate-only-required"),
    ] {
        if !ok {
            omissions.insert(label.into());
        }
    }
    let global_block = !r.policy_allowed
        || !r.protected_closure
        || !r.signed_approval
        || !r.raw_data_local
        || !r.aggregate_only;
    let disposition = if global_block || !blocked.is_empty() {
        "blocked"
    } else if ready.is_empty() {
        "partial"
    } else {
        "qualified"
    };
    if global_block {
        blocked.extend(step_order.iter().cloned());
        ready.clear();
    }
    if disposition != "qualified" {
        omissions.insert("workflow:design-closure-not-ready".into());
    }
    let payload = json!({"step_order":step_order,"ready_order":ready,"blocked_order":blocked,"omission_order":omissions,"uncertainty_order":uncertainty,"negative_evidence_order":negative,"replay_identity":r.replay_identity});
    let digest = ContentHash::of_value(&payload)
        .map_err(|e| ExperimentDesignWorkflowError::Artifact(e.to_string()))?;
    let strings = |k: &str| {
        payload[k]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    };
    let out = ExecutableExperimentDesign4 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: r.request_id.clone(),
        consumer: r.consumer.clone(),
        purpose: r.purpose.clone(),
        target_scope: r.target_scope.clone(),
        semantic_profile: r.semantic_profile.clone(),
        disposition: disposition.into(),
        step_order: strings("step_order"),
        ready_order: strings("ready_order"),
        blocked_order: strings("blocked_order"),
        omission_order: strings("omission_order"),
        uncertainty_order: strings("uncertainty_order"),
        negative_evidence_order: strings("negative_evidence_order"),
        replay_identity: r.replay_identity.clone(),
        design_digest: digest.clone(),
        artifact: ExecutableExperimentDesignArtifact4 {
            artifact_id: format!("bioethics-experiment-design:{}", r.request_id),
            content_type: CONTENT_TYPE.into(),
            content_hash: digest,
            semantic_loss: if disposition == "qualified" {
                Vec::new()
            } else {
                vec!["design-not-scheduled".into()]
            },
            provenance_digests: vec![o.provenance_digest.clone()],
            boundary: PRECLINICAL_BOUNDARY.into(),
        },
        effect_receipts: if disposition == "qualified" {
            vec![format!("schedule:research-work:{}", r.request_id)]
        } else {
            vec!["block:unsafe-release".into()]
        },
        raw_data_local: true,
        aggregate_only: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    out.validate()?;
    Ok(out)
}

pub fn compile_experiment_design_workflow_json(
    value: &serde_json::Value,
) -> Result<serde_json::Value, String> {
    let r: ExperimentDesignWorkflowRequest1 = serde_json::from_value(value.clone())
        .map_err(|e| format!("invalid experiment design workflow request: {e}"))?;
    serde_json::to_value(compile_experiment_design_workflow(&r).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())
}
pub fn validate_experiment_design_workflow_json(
    value: &serde_json::Value,
) -> Result<ExecutableExperimentDesign4, String> {
    let out: ExecutableExperimentDesign4 = serde_json::from_value(value.clone())
        .map_err(|e| format!("invalid experiment design workflow receipt: {e}"))?;
    out.validate().map_err(|e| e.to_string())?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn h(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn req() -> ExperimentDesignWorkflowRequest1 {
        ExperimentDesignWorkflowRequest1 {
            schema_version: INPUT_SCHEMA.into(),
            request_id: "design-1".into(),
            consumer: "steward".into(),
            purpose: "schedule organoid assay".into(),
            target_scope: "organoid".into(),
            semantic_profile: "cwl".into(),
            minimum_power_milli: 800,
            replay_identity: h("r"),
            policy_allowed: true,
            protected_closure: true,
            signed_approval: true,
            budget: 20,
            raw_data_local: true,
            aggregate_only: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
            objective: ExperimentObjective1 {
                objective_id: "o1".into(),
                target_scope: "organoid".into(),
                semantic_profile: "cwl".into(),
                evidence_state: EvidenceState::Supported,
                artifact_digest: h("a"),
                provenance_digest: h("p"),
                replay_identity: h("r"),
                power_milli: 900,
                steps: vec![
                    WorkflowStep1 {
                        step_id: "collect".into(),
                        depends_on: vec![],
                        declared_effect: "compute".into(),
                        duration_budget: 5,
                        required: true,
                    },
                    WorkflowStep1 {
                        step_id: "analyze".into(),
                        depends_on: vec!["collect".into()],
                        declared_effect: "compute".into(),
                        duration_budget: 5,
                        required: true,
                    },
                ],
                omission_order: vec![],
                negative_result: false,
            },
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            experiment_design_workflow_fabric_manifest().autonomy_tier,
            AutonomyTier::A1
        )
    }
    #[test]
    fn topological_design_qualifies() {
        let o = compile_experiment_design_workflow(&req()).unwrap();
        assert_eq!(o.disposition, "qualified");
        assert_eq!(o.ready_order, vec!["analyze", "collect"])
    }
    #[test]
    fn cycle_is_rejected() {
        let mut r = req();
        r.objective.steps[0].depends_on = vec!["analyze".into()];
        assert!(compile_experiment_design_workflow(&r).is_err())
    }
    #[test]
    fn policy_blocks() {
        let mut r = req();
        r.policy_allowed = false;
        assert_eq!(
            compile_experiment_design_workflow(&r).unwrap().disposition,
            "blocked"
        )
    }
}
