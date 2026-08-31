//! Prospective high-throughput experiment-design research workbench (`AFA-epistemic-P09-F19`).
//!
//! This is a bounded researcher-facing planning surface.  It ranks caller-supplied, digest-only
//! power-aware design candidates and emits an executable-design contract; it never schedules
//! animals, consumes material, controls instruments, or makes a clinical decision.

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, SemanticLoss, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-epistemic-P09-F19";
pub const CONTRACT_VERSION: &str =
    "epistemic-prospective-high-throughput-experiment-design-research-workbench/1.0";
pub const INPUT_SCHEMA: &str = "ExperimentObjective3@1";
pub const OUTPUT_SCHEMA: &str = "ExecutableExperimentDesign5@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.executable-experiment-design-5+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentObjective3 {
    pub schema_version: String,
    pub request_id: String,
    pub researcher_id: String,
    pub study_program: String,
    pub purpose: String,
    pub scope: String,
    pub semantic_profile: String,
    pub required_candidate_order: Vec<String>,
    pub required_factor_order: Vec<String>,
    pub baseline_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub budget_units: u32,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PowerDesignCandidate5 {
    pub candidate_id: String,
    pub label: String,
    pub factor_order: Vec<String>,
    pub sample_size: u32,
    pub power_milli: u16,
    pub variance_milli: u16,
    pub attrition_milli: u16,
    pub evidence_state: EvidenceState,
    pub design_digest: ContentHash,
    pub baseline_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub semantic_profile: String,
    pub expected_cost_units: u32,
    pub signed: bool,
    pub comparable: bool,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutableExperimentDesign5 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub researcher_id: String,
    pub study_program: String,
    pub purpose: String,
    pub scope: String,
    pub semantic_profile: String,
    pub disposition: String,
    pub candidate_order: Vec<String>,
    pub ranked_order: Vec<String>,
    pub executable_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub missing_candidate_order: Vec<String>,
    pub missing_factor_order: Vec<String>,
    pub plan_order: Vec<String>,
    pub evidence_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub baseline_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub plan_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub budget_units: u32,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ExperimentDesignWorkbenchError {
    #[error("invalid experiment-design workbench request or receipt: {0}")]
    Invalid(String),
    #[error("experiment-design workbench artifact failed: {0}")]
    Artifact(String),
}

fn invalid(message: impl Into<String>) -> ExperimentDesignWorkbenchError {
    ExperimentDesignWorkbenchError::Invalid(message.into())
}
fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}
fn valid_digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().bytes().all(|byte| byte.is_ascii_hexdigit())
}
fn digest(value: &Value) -> Result<ContentHash, ExperimentDesignWorkbenchError> {
    ContentHash::of_value(value)
        .map_err(|error| ExperimentDesignWorkbenchError::Artifact(error.to_string()))
}

pub fn experiment_design_research_workbench_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "epistemic".into(), consumers: ["downstream AURORA crate maintainer".into(), "preclinical design scientist".into(), "research workbench operator".into()].into(), behavior: "rank typed power-aware experiment designs into a deterministic prospective workbench contract with explicit omissions, uncertainty, and evidence gates".into(), value: "makes high-throughput design trade-offs and power shortfalls auditable before any laboratory action".into(), inputs: vec![TypedPort { name: "experiment_objective".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "executable_experiment_design".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["view:authorized-research-state".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "json-schema".into(), state: EvidenceState::Supported, locator: Some("https://json-schema.org/draft/2020-12/schema".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

impl ExecutableExperimentDesign5 {
    pub fn validate(&self) -> Result<(), ExperimentDesignWorkbenchError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.content_type != CONTENT_TYPE
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.researcher_id.trim().is_empty()
            || self.study_program.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.ranked_order.len() != self.candidate_order.len()
            || self.plan_order.is_empty()
            || self.evidence_order.is_empty()
            || self.effect_receipts.is_empty()
            || self.budget_units == 0
            || !["qualified", "partial", "blocked"].contains(&self.disposition.as_str())
        {
            return Err(invalid(
                "workbench identity, plan, evidence, locality, budget, or effects are incomplete",
            ));
        }
        for values in [
            &self.candidate_order,
            &self.executable_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.missing_candidate_order,
            &self.missing_factor_order,
            &self.plan_order,
            &self.evidence_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(invalid("workbench ordering is not canonical"));
            }
        }
        let ids = self
            .candidate_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let partitions = self
            .executable_order
            .iter()
            .chain(&self.unresolved_order)
            .chain(&self.blocked_order)
            .cloned()
            .collect::<Vec<_>>();
        if ids.len() != self.candidate_order.len()
            || partitions.len() != ids.len()
            || partitions.iter().cloned().collect::<BTreeSet<_>>() != ids
            || self.ranked_order.iter().cloned().collect::<BTreeSet<_>>() != ids
        {
            return Err(invalid("candidate states do not partition"));
        }
        if !valid_digest(&self.baseline_digest)
            || !valid_digest(&self.replay_identity)
            || !valid_digest(&self.plan_digest)
            || self.artifact.content_hash != self.plan_digest
        {
            return Err(ExperimentDesignWorkbenchError::Artifact(
                "workbench digest or artifact hash is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ExperimentDesignWorkbenchError::Artifact(error.to_string()))?;
        if self.disposition == "qualified" {
            if self.effect_receipts.len() != 1
                || !self.effect_receipts[0].starts_with("view:design-plan:")
            {
                return Err(invalid("qualified workbench effect is invalid"));
            }
        } else if self.effect_receipts != ["block:unsafe-release"] {
            return Err(invalid("non-qualified workbench must block release"));
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<ContentHash, ExperimentDesignWorkbenchError> {
        self.validate()?;
        serde_json::to_value(self)
            .map_err(|error| ExperimentDesignWorkbenchError::Artifact(error.to_string()))
            .and_then(|value| digest(&value))
    }
}

pub fn compile_experiment_design_workbench(
    objective: &ExperimentObjective3,
    candidates: &[PowerDesignCandidate5],
) -> Result<ExecutableExperimentDesign5, ExperimentDesignWorkbenchError> {
    if objective.schema_version != INPUT_SCHEMA
        || objective.request_id.trim().is_empty()
        || objective.researcher_id.trim().is_empty()
        || objective.study_program.trim().is_empty()
        || objective.purpose.trim().is_empty()
        || objective.scope.trim().is_empty()
        || objective.semantic_profile.trim().is_empty()
        || objective.required_candidate_order.is_empty()
        || !canonical(&objective.required_candidate_order)
        || !canonical(&objective.required_factor_order)
        || !valid_digest(&objective.baseline_digest)
        || !valid_digest(&objective.replay_identity)
        || objective.budget_units == 0
        || !objective.raw_data_local
        || objective.boundary != PRECLINICAL_BOUNDARY
        || candidates.is_empty()
    {
        return Err(invalid("objective identity, required closure, digest, budget, locality, or boundary is invalid"));
    }
    let mut rows = candidates.to_vec();
    rows.sort_by(|left, right| {
        right
            .power_milli
            .cmp(&left.power_milli)
            .then(right.sample_size.cmp(&left.sample_size))
            .then(left.candidate_id.cmp(&right.candidate_id))
    });
    let candidate_order = {
        let mut ids = rows
            .iter()
            .map(|row| row.candidate_id.clone())
            .collect::<Vec<_>>();
        ids.sort();
        ids
    };
    if candidate_order.windows(2).any(|pair| pair[0] == pair[1])
        || candidate_order.iter().any(|id| id.trim().is_empty())
    {
        return Err(invalid(
            "candidate identifiers must be unique and non-empty",
        ));
    }
    let ranked_order = rows
        .iter()
        .map(|row| row.candidate_id.clone())
        .collect::<Vec<_>>();
    let mut executable = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut evidence = BTreeSet::new();
    let mut plan = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    for row in &rows {
        let id = row.candidate_id.clone();
        evidence.insert(format!("{}:{:?}", id, row.evidence_state).to_ascii_lowercase());
        provenance.insert(row.provenance_digest.clone());
        omissions.extend(row.omissions.iter().map(|item| format!("{id}:{item}")));
        uncertainty.extend(row.uncertainty.iter().map(|item| format!("{id}:{item}")));
        if row.negative_result || row.evidence_state == EvidenceState::Contradicted {
            negative.insert(format!("{id}:negative-result"));
        }
        let missing_factor = objective
            .required_factor_order
            .iter()
            .filter(|factor| !row.factor_order.contains(factor))
            .cloned()
            .collect::<Vec<_>>();
        if !missing_factor.is_empty() {
            omissions.extend(
                missing_factor
                    .iter()
                    .map(|factor| format!("{id}:missing-factor:{factor}")),
            );
        }
        let hard = !row.signed
            || !row.comparable
            || row.baseline_digest != objective.baseline_digest
            || row.replay_identity != objective.replay_identity
            || row.semantic_profile != objective.semantic_profile
            || row.expected_cost_units > objective.budget_units;
        if hard {
            blocked.insert(id);
            uncertainty.insert(format!(
                "{}:authorization-comparability-replay-or-budget-blocked",
                row.candidate_id
            ));
        } else if !missing_factor.is_empty()
            || !matches!(
                row.evidence_state,
                EvidenceState::Proven | EvidenceState::Supported
            )
            || row.power_milli < 800
            || !valid_digest(&row.design_digest)
            || !valid_digest(&row.provenance_digest)
        {
            unresolved.insert(id);
            uncertainty.insert(format!(
                "{}:power-evidence-or-closure-incomplete",
                row.candidate_id
            ));
        } else {
            executable.insert(id.clone());
            plan.insert(format!(
                "{}:n{}:power{}",
                id, row.sample_size, row.power_milli
            ));
        }
    }
    let mut missing_candidate = BTreeSet::new();
    for required in &objective.required_candidate_order {
        if !candidate_order.contains(required) {
            missing_candidate.insert(required.clone());
            omissions.insert(format!("request:missing-candidate:{required}"));
        }
    }
    let global_block =
        !objective.policy_allow || !objective.protected_closure || !objective.raw_data_local;
    if global_block {
        blocked.extend(candidate_order.iter().cloned());
        executable.clear();
        unresolved.clear();
        omissions.insert("request:policy-protected-closure-or-locality-blocked".into());
    }
    let executable_order = executable.into_iter().collect::<Vec<_>>();
    let unresolved_order = unresolved.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let missing_candidate_order = missing_candidate.into_iter().collect::<Vec<_>>();
    let missing_factor_order = objective
        .required_factor_order
        .iter()
        .filter(|factor| !rows.iter().any(|row| row.factor_order.contains(factor)))
        .cloned()
        .collect::<Vec<_>>();
    let disposition = if global_block || executable_order.is_empty() && unresolved_order.is_empty()
    {
        "blocked"
    } else if !missing_candidate_order.is_empty()
        || !missing_factor_order.is_empty()
        || !blocked_order.is_empty()
        || !unresolved_order.is_empty()
    {
        "partial"
    } else {
        "qualified"
    };
    if disposition != "qualified" {
        omissions.insert("request:design-closure-not-ready".into());
    }
    let omission_order = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty_order = uncertainty.into_iter().collect::<Vec<_>>();
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":objective.request_id,"researcher_id":objective.researcher_id,"study_program":objective.study_program,"purpose":objective.purpose,"scope":objective.scope,"semantic_profile":objective.semantic_profile,"disposition":disposition,"candidate_order":candidate_order,"ranked_order":ranked_order,"executable_order":executable_order,"unresolved_order":unresolved_order,"blocked_order":blocked_order,"missing_candidate_order":missing_candidate_order,"missing_factor_order":missing_factor_order,"plan_order":plan.into_iter().collect::<Vec<_>>(),"evidence_order":evidence.into_iter().collect::<Vec<_>>(),"omission_order":omission_order,"uncertainty_order":uncertainty_order,"negative_evidence_order":negative.into_iter().collect::<Vec<_>>(),"baseline_digest":objective.baseline_digest,"replay_identity":objective.replay_identity,"budget_units":objective.budget_units,"raw_data_local":true,"boundary":PRECLINICAL_BOUNDARY});
    let plan_digest = digest(&payload)?;
    let semantic_loss = payload["omission_order"]
        .as_array()
        .cloned()
        .unwrap_or_default()
        .into_iter()
        .filter_map(|value| {
            value.as_str().map(|field| SemanticLoss {
                field: field.into(),
                reason: "design closure or evidence omission".into(),
                severity: bioprism_foundation::LossSeverity::Unknown,
            })
        })
        .collect::<Vec<_>>();
    let provenance = provenance
        .into_iter()
        .enumerate()
        .map(|(index, digest)| bioprism_foundation::ProvenanceLink {
            source_id: format!("design-candidate:{index}"),
            relation: "declared-by-researcher".into(),
            digest,
        })
        .collect::<Vec<_>>();
    let artifact = TypedResearchArtifact {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        artifact_id: format!("executable-experiment-design-5:{}", objective.request_id),
        content_type: CONTENT_TYPE.into(),
        content_hash: plan_digest.clone(),
        semantic_loss,
        provenance,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let mut output = payload;
    output["plan_digest"] = json!(plan_digest);
    output["artifact"] = serde_json::to_value(artifact)
        .map_err(|error| ExperimentDesignWorkbenchError::Artifact(error.to_string()))?;
    output["effect_receipts"] = json!(if disposition == "qualified" {
        vec![format!("view:design-plan:{}", objective.request_id)]
    } else {
        vec!["block:unsafe-release".to_string()]
    });
    let receipt: ExecutableExperimentDesign5 = serde_json::from_value(output)
        .map_err(|error| ExperimentDesignWorkbenchError::Artifact(error.to_string()))?;
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn h(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn objective() -> ExperimentObjective3 {
        ExperimentObjective3 {
            schema_version: INPUT_SCHEMA.into(),
            request_id: "req-1".into(),
            researcher_id: "researcher".into(),
            study_program: "organoid-screen".into(),
            purpose: "power-design".into(),
            scope: "preclinical".into(),
            semantic_profile: "design-v1".into(),
            required_candidate_order: vec!["c-a".into()],
            required_factor_order: vec!["dose".into()],
            baseline_digest: h("baseline"),
            replay_identity: h("replay"),
            budget_units: 100,
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    fn candidate(id: &str) -> PowerDesignCandidate5 {
        PowerDesignCandidate5 {
            candidate_id: id.into(),
            label: id.into(),
            factor_order: vec!["dose".into()],
            sample_size: 24,
            power_milli: 900,
            variance_milli: 100,
            attrition_milli: 100,
            evidence_state: EvidenceState::Supported,
            design_digest: h(id),
            baseline_digest: h("baseline"),
            provenance_digest: h("prov"),
            replay_identity: h("replay"),
            semantic_profile: "design-v1".into(),
            expected_cost_units: 10,
            signed: true,
            comparable: true,
            omissions: vec![],
            uncertainty: vec![],
            negative_result: false,
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            experiment_design_research_workbench_manifest().autonomy_tier,
            AutonomyTier::A1
        )
    }
    #[test]
    fn qualified_plan() {
        let r = compile_experiment_design_workbench(&objective(), &[candidate("c-a")]).unwrap();
        assert_eq!(r.disposition, "qualified");
        assert!(r.effect_receipts[0].starts_with("view:design-plan:"));
    }
    #[test]
    fn ranking_is_deterministic() {
        let mut low = candidate("c-b");
        low.power_milli = 850;
        let r =
            compile_experiment_design_workbench(&objective(), &[low, candidate("c-a")]).unwrap();
        assert_eq!(r.ranked_order, vec!["c-a", "c-b"]);
    }
    #[test]
    fn missing_factor_is_partial() {
        let mut c = candidate("c-a");
        c.factor_order.clear();
        let r = compile_experiment_design_workbench(&objective(), &[c]).unwrap();
        assert_eq!(r.disposition, "partial");
        assert!(!r.missing_factor_order.is_empty());
    }
    #[test]
    fn policy_blocks() {
        let mut o = objective();
        o.policy_allow = false;
        let r = compile_experiment_design_workbench(&o, &[candidate("c-a")]).unwrap();
        assert_eq!(r.disposition, "blocked");
        assert_eq!(r.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn unknown_evidence_is_retained() {
        let mut c = candidate("c-a");
        c.evidence_state = EvidenceState::Unknown;
        let r = compile_experiment_design_workbench(&objective(), &[c]).unwrap();
        assert_eq!(r.disposition, "partial");
        assert!(!r.uncertainty_order.is_empty());
    }
    #[test]
    fn digest_is_deterministic() {
        let a = compile_experiment_design_workbench(&objective(), &[candidate("c-a")]).unwrap();
        let b = compile_experiment_design_workbench(&objective(), &[candidate("c-a")]).unwrap();
        assert_eq!(a.plan_digest, b.plan_digest);
    }
}
