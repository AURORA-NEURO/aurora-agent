//! Prospective experiment-design research copilot backed by the oracle evidence ladder.
//!
//! Atlas feature: `AFA-oracle-P09-F11`. This is a bounded agent surface: it ranks caller
//! supplied design candidates, retains every unsupported or contradictory candidate, and emits
//! an executable-design artifact only after power, replication, provenance, policy, and tool
//! gates close. It never schedules animals, consumes material, controls instruments, or makes a
//! clinical decision.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect,
    EvidenceReference, EvidenceState, ResearchSurface, TypedPort, TypedResearchArtifact,
    PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-oracle-P09-F11";
pub const CONTRACT_VERSION: &str = "oracle-prospective-experiment-design-research-copilot/1.0";
pub const INPUT_SCHEMA: &str = "ExperimentObjective3@1";
pub const OUTPUT_SCHEMA: &str = "ExecutableExperimentDesign3@1";
pub const MAX_PLAN_STEPS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesignCandidate {
    pub candidate_id: String,
    pub label: String,
    pub factor_order: Vec<String>,
    pub sample_size: u32,
    pub power_milli: u16,
    pub replication_milli: u16,
    pub evidence_state: EvidenceState,
    pub design_digest: ContentHash,
    pub baseline_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub semantic_profile: String,
    pub expected_cost_units: u32,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub local_data: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentObjective {
    pub request_id: String,
    pub operator_id: String,
    pub workflow_id: String,
    pub benchmark_id: String,
    pub purpose: String,
    pub scope: String,
    pub semantic_profile: String,
    pub required_candidate_order: Vec<String>,
    pub required_factor_order: Vec<String>,
    pub baseline_digest: ContentHash,
    pub candidates: Vec<DesignCandidate>,
    pub replay_identity: ContentHash,
    pub declared_tool_id: String,
    pub action_allow_list: Vec<String>,
    pub max_actions: usize,
    pub budget_units: u32,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CopilotDisposition {
    Qualified,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExperimentDesignCopilotReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub operator_id: String,
    pub workflow_id: String,
    pub benchmark_id: String,
    pub purpose: String,
    pub scope: String,
    pub semantic_profile: String,
    pub disposition: CopilotDisposition,
    pub candidate_order: Vec<String>,
    pub ranked_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub missing_candidate_order: Vec<String>,
    pub missing_factor_order: Vec<String>,
    pub plan_order: Vec<String>,
    pub action_order: Vec<String>,
    pub tool_order: Vec<String>,
    pub baseline_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub plan_digest: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub budget_units: u32,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ExperimentDesignCopilotError {
    #[error("invalid experiment-design copilot request: {0}")]
    Invalid(String),
    #[error("experiment-design copilot artifact failed: {0}")]
    Artifact(String),
}

fn invalid(message: impl Into<String>) -> ExperimentDesignCopilotError {
    ExperimentDesignCopilotError::Invalid(message.into())
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

pub fn experiment_design_copilot_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "oracle".into(),
        consumers: ["benchmark curator".into(), "preclinical design scientist".into()]
            .into(),
        behavior: "ranks typed prospective experiment designs with oracle evidence, power, replication, provenance, and declared-tool gates".into(),
        value: "turns an oracle mesh into a bounded design copilot without silently converting unknown evidence into an executable plan".into(),
        inputs: vec![TypedPort { name: "experiment_objective".into(), schema: INPUT_SCHEMA.into(), required: true }],
        outputs: vec![TypedPort { name: "executable_experiment_design".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::ExternalDataAccess, Effect::WriteLocalArtifact].into(),
        permissions: ["invoke:declared-tools".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "mcp-2025-06-18".into(), state: EvidenceState::Supported, locator: Some("https://modelcontextprotocol.io/specification/2025-06-18/basic/index".into()) }],
        authority_requirements: vec![AuthorityRequirement { role: "design-copilot operator".into(), reason: "declared-tool invocation requires purpose-bound institutional authorization".into() }],
        autonomy_tier: AutonomyTier::A2,
        surfaces: [ResearchSurface::McpTool, ResearchSurface::Sdk, ResearchSurface::Api, ResearchSurface::Ui, ResearchSurface::Policy, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

impl ExperimentDesignCopilotReceipt {
    pub fn validate(&self) -> Result<(), ExperimentDesignCopilotError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.operator_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.benchmark_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.ranked_order.len() != self.candidate_order.len()
            || self.plan_order.is_empty()
            || self.plan_order.len() != self.action_order.len()
            || self.tool_order.is_empty()
            || self.effect_receipts.is_empty()
            || self.budget_units == 0
        {
            return Err(invalid("copilot identity, bounded plan, locality, budget, or effects are incomplete"));
        }
        for values in [
            &self.candidate_order,
            &self.admitted_order,
            &self.unknown_order,
            &self.blocked_order,
            &self.missing_candidate_order,
            &self.missing_factor_order,
            &self.plan_order,
            &self.action_order,
            &self.tool_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(invalid("copilot ordering is not canonical"));
            }
        }
        let ids = self.candidate_order.iter().collect::<BTreeSet<_>>();
        let partitions = self
            .admitted_order
            .iter()
            .chain(self.unknown_order.iter())
            .chain(self.blocked_order.iter())
            .collect::<Vec<_>>();
        if partitions.len() != ids.len()
            || partitions.iter().any(|id| !ids.contains(id))
            || partitions.iter().collect::<BTreeSet<_>>().len() != partitions.len()
            || self.ranked_order.iter().collect::<BTreeSet<_>>() != ids
        {
            return Err(invalid("copilot candidate states do not partition candidates"));
        }
        for value in [
            &self.baseline_digest,
            &self.replay_identity,
            &self.plan_digest,
            &self.artifact.content_hash,
        ] {
            if !digest(value) {
                return Err(invalid("copilot digest is invalid"));
            }
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ExperimentDesignCopilotError::Artifact(error.to_string()))?;
        if self.artifact.content_type != "application/vnd.aurora.executable-experiment-design-3+json" {
            return Err(invalid("copilot artifact type is invalid"));
        }
        if self.disposition == CopilotDisposition::Qualified {
            if self.effect_receipts.len() != 1
                || !self.effect_receipts[0].starts_with("invoke:declared-tool:")
            {
                return Err(invalid("qualified copilot effect is invalid"));
            }
        } else if self.effect_receipts != ["block:unsafe-release"] {
            return Err(invalid("non-qualified copilot must block release"));
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, ExperimentDesignCopilotError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ExperimentDesignCopilotError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ExperimentDesignCopilotError::Artifact(error.to_string()))
    }
}

pub fn compile_experiment_design_copilot(
    request: &ExperimentObjective,
) -> Result<ExperimentDesignCopilotReceipt, ExperimentDesignCopilotError> {
    validate_request(request)?;
    let mut candidates = request.candidates.clone();
    candidates.sort_by(|left, right| {
        let left_state = evidence_rank(left.evidence_state);
        let right_state = evidence_rank(right.evidence_state);
        right_state
            .cmp(&left_state)
            .then((right.power_milli as u32 + right.replication_milli as u32).cmp(&(left.power_milli as u32 + left.replication_milli as u32)))
            .then(left.candidate_id.cmp(&right.candidate_id))
    });
    let ranked_order = candidates.iter().map(|candidate| candidate.candidate_id.clone()).collect::<Vec<_>>();
    let mut candidate_order = ranked_order.clone();
    candidate_order.sort();
    let required = request.required_candidate_order.iter().cloned().collect::<BTreeSet<_>>();
    let missing_candidate = required.iter().filter(|id| !candidate_order.contains(id)).cloned().collect::<Vec<_>>();
    let required_factors = request.required_factor_order.iter().cloned().collect::<BTreeSet<_>>();
    let mut admitted = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut spent = 0_u32;
    for candidate in &candidates {
        let id = &candidate.candidate_id;
        for item in &candidate.omissions { omissions.insert(format!("{id}:{item}")); }
        for item in &candidate.uncertainty { uncertainty.insert(format!("{id}:{item}")); }
        for item in &candidate.negative_evidence { negative.insert(format!("{id}:{item}")); }
        if candidate.evidence_state == EvidenceState::Contradicted {
            blocked.insert(id.clone());
            negative.insert(format!("{id}:contradicted-evidence"));
            continue;
        }
        if matches!(candidate.evidence_state, EvidenceState::Unknown | EvidenceState::Speculative) {
            unknown.insert(id.clone());
            uncertainty.insert(format!("{id}:evidence-unresolved"));
            continue;
        }
        let factors = candidate.factor_order.iter().cloned().collect::<BTreeSet<_>>();
        let budget_ok = candidate.expected_cost_units <= request.budget_units.saturating_sub(spent);
        let action_ok = request.action_allow_list.iter().any(|item| item == "generate-experiment-design");
        let valid = candidate.sample_size > 0
            && candidate.power_milli >= 800
            && candidate.replication_milli >= 750
            && candidate.baseline_digest == request.baseline_digest
            && candidate.replay_identity == request.replay_identity
            && candidate.semantic_profile == request.semantic_profile
            && required_factors.is_subset(&factors)
            && candidate.omissions.is_empty()
            && candidate.uncertainty.is_empty()
            && candidate.negative_evidence.is_empty()
            && candidate.local_data
            && budget_ok
            && action_ok;
        if valid && matches!(candidate.evidence_state, EvidenceState::Proven | EvidenceState::Supported) {
            spent = spent.saturating_add(candidate.expected_cost_units);
            admitted.insert(id.clone());
        } else {
            unknown.insert(id.clone());
            if candidate.sample_size == 0 { omissions.insert(format!("{id}:sample-size-missing")); }
            if candidate.power_milli < 800 { uncertainty.insert(format!("{id}:power-threshold-not-met")); }
            if candidate.replication_milli < 750 { uncertainty.insert(format!("{id}:replication-threshold-not-met")); }
            if candidate.baseline_digest != request.baseline_digest { omissions.insert(format!("{id}:baseline-mismatch")); }
            if candidate.replay_identity != request.replay_identity { omissions.insert(format!("{id}:replay-mismatch")); }
            if !required_factors.is_subset(&factors) { omissions.insert(format!("{id}:factor-closure-incomplete")); }
            if !candidate.local_data { blocked.insert(id.clone()); unknown.remove(id); omissions.insert(format!("{id}:locality-denied")); }
            if !budget_ok { omissions.insert(format!("{id}:budget-ceiling-exceeded")); }
            if !action_ok { blocked.insert(id.clone()); unknown.remove(id); negative.insert(format!("{id}:declared-tool-action-denied")); }
        }
    }
    for id in &missing_candidate { omissions.insert(format!("{id}:required-candidate-missing")); }
    let missing_factor = request.required_factor_order.iter().filter(|factor| !candidates.iter().any(|candidate| candidate.factor_order.contains(factor))).cloned().collect::<Vec<_>>();
    for factor in &missing_factor { omissions.insert(format!("required-factor-missing:{factor}")); }
    let action_ok = request.action_allow_list.iter().any(|item| item == "generate-experiment-design");
    if !request.policy_allow { negative.insert("request:policy-denied".into()); }
    if !request.protected_closure { uncertainty.insert("request:protected-closure-incomplete".into()); }
    if !request.signed_approval { uncertainty.insert("request:signed-approval-missing".into()); }
    if !request.raw_data_local { negative.insert("request:raw-data-locality-required".into()); }
    let global_block = !request.policy_allow || !request.protected_closure || !request.signed_approval || !request.raw_data_local || !action_ok || request.max_actions == 0;
    let mut plan = BTreeSet::new();
    let mut actions = BTreeSet::new();
    for id in &candidate_order {
        plan.insert(format!("plan:review-design:{id}"));
        actions.insert(format!("action:review-design:{id}"));
    }
    if admitted.is_empty() { plan.insert("plan:retain-unresolved-designs".into()); actions.insert("action:retain-unresolved-designs".into()); }
    if plan.len() > request.max_actions || plan.len() > MAX_PLAN_STEPS || plan.len() as u32 > request.budget_units { omissions.insert("copilot:plan-budget-exhausted".into()); }
    let mut plan_order = plan.into_iter().collect::<Vec<_>>();
    let mut action_order = actions.into_iter().collect::<Vec<_>>();
    let qualified = !global_block && missing_candidate.is_empty() && missing_factor.is_empty() && !admitted.is_empty() && unknown.is_empty() && blocked.is_empty() && plan_order.len() <= request.max_actions && plan_order.len() as u32 <= request.budget_units;
    let disposition = if qualified { CopilotDisposition::Qualified } else if global_block || plan_order.len() > request.max_actions || plan_order.len() as u32 > request.budget_units { CopilotDisposition::Blocked } else if admitted.is_empty() { CopilotDisposition::Unknown } else { CopilotDisposition::Partial };
    let admitted_order = admitted.into_iter().collect::<Vec<_>>();
    let unknown_order = unknown.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let effects = if disposition == CopilotDisposition::Qualified { vec![format!("invoke:declared-tool:{}", request.declared_tool_id)] } else { vec!["block:unsafe-release".into()] };
    let tool_order = vec![request.declared_tool_id.clone()];
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"operator_id":request.operator_id,"workflow_id":request.workflow_id,"benchmark_id":request.benchmark_id,"purpose":request.purpose,"scope":request.scope,"semantic_profile":request.semantic_profile,"disposition":disposition,"candidate_order":candidate_order,"ranked_order":ranked_order,"admitted_order":admitted_order,"unknown_order":unknown_order,"blocked_order":blocked_order,"missing_candidate_order":missing_candidate,"missing_factor_order":missing_factor,"plan_order":plan_order,"action_order":action_order,"tool_order":tool_order,"baseline_digest":request.baseline_digest,"replay_identity":request.replay_identity,"omissions":omissions,"uncertainty":uncertainty,"negative_evidence":negative,"effect_receipts":effects,"budget_units":request.budget_units,"boundary":PRECLINICAL_BOUNDARY});
    let plan_digest = ContentHash::of_value(&payload).map_err(|error| ExperimentDesignCopilotError::Artifact(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(format!("oracle-experiment-design:{}", request.request_id), "application/vnd.aurora.executable-experiment-design-3+json", &payload, Vec::new(), Vec::new()).map_err(|error| ExperimentDesignCopilotError::Artifact(error.to_string()))?;
    let receipt = ExperimentDesignCopilotReceipt { schema_version:RESEARCH_CONTRACT_SCHEMA_VERSION.into(), contract_version:CONTRACT_VERSION.into(), feature_id:FEATURE_ID.into(), request_id:request.request_id.clone(), operator_id:request.operator_id.clone(), workflow_id:request.workflow_id.clone(), benchmark_id:request.benchmark_id.clone(), purpose:request.purpose.clone(), scope:request.scope.clone(), semantic_profile:request.semantic_profile.clone(), disposition, candidate_order, ranked_order, admitted_order, unknown_order, blocked_order, missing_candidate_order:missing_candidate, missing_factor_order:missing_factor, plan_order:std::mem::take(&mut plan_order), action_order:std::mem::take(&mut action_order), tool_order, baseline_digest:request.baseline_digest.clone(), replay_identity:request.replay_identity.clone(), plan_digest, omissions:omissions.into_iter().collect(), uncertainty:uncertainty.into_iter().collect(), negative_evidence:negative.into_iter().collect(), effect_receipts:effects, artifact, budget_units:request.budget_units, raw_data_local:true, boundary:PRECLINICAL_BOUNDARY.into() };
    receipt.validate()?;
    Ok(receipt)
}

fn evidence_rank(state: EvidenceState) -> u8 {
    match state { EvidenceState::Proven => 4, EvidenceState::Supported => 3, EvidenceState::Speculative => 2, EvidenceState::Unknown => 1, EvidenceState::Contradicted => 0 }
}

fn validate_request(request: &ExperimentObjective) -> Result<(), ExperimentDesignCopilotError> {
    if request.request_id.trim().is_empty() || request.operator_id.trim().is_empty() || request.workflow_id.trim().is_empty() || request.benchmark_id.trim().is_empty() || request.purpose.trim().is_empty() || request.scope.trim().is_empty() || request.semantic_profile.trim().is_empty() || request.required_candidate_order.is_empty() || request.required_factor_order.is_empty() || request.candidates.is_empty() || request.declared_tool_id.trim().is_empty() || request.action_allow_list.is_empty() || request.max_actions == 0 || request.max_actions > MAX_PLAN_STEPS || request.budget_units == 0 || request.boundary != PRECLINICAL_BOUNDARY || !request.raw_data_local || !canonical(&request.required_candidate_order) || !canonical(&request.required_factor_order) || !digest(&request.baseline_digest) || !digest(&request.replay_identity) { return Err(invalid("copilot identity, scope, tool, bounds, digests, locality, or boundary is incomplete")); }
    let mut seen = BTreeSet::new();
    for candidate in &request.candidates { if candidate.candidate_id.trim().is_empty() || !seen.insert(candidate.candidate_id.clone()) || candidate.label.trim().is_empty() || candidate.factor_order.is_empty() || !canonical(&candidate.factor_order) || candidate.power_milli > 1000 || candidate.replication_milli > 1000 || candidate.expected_cost_units == 0 || !digest(&candidate.design_digest) || !digest(&candidate.baseline_digest) || !digest(&candidate.provenance_digest) || !digest(&candidate.replay_identity) || candidate.semantic_profile.trim().is_empty() || !canonical(&candidate.omissions) || !canonical(&candidate.uncertainty) || !canonical(&candidate.negative_evidence) { return Err(invalid(format!("candidate {} is malformed or duplicated", candidate.candidate_id))); } }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn h(value: &str) -> ContentHash { ContentHash::of_bytes(value.as_bytes()) }
    fn candidate(id: &str, state: EvidenceState, power: u16) -> DesignCandidate { DesignCandidate { candidate_id:id.into(), label:format!("design-{id}"), factor_order:vec!["dose".into(),"time".into()], sample_size:32, power_milli:power, replication_milli:800, evidence_state:state, design_digest:h(&format!("design:{id}")), baseline_digest:h("baseline"), provenance_digest:h(&format!("provenance:{id}")), replay_identity:h("replay"), semantic_profile:"preclinical-neural".into(), expected_cost_units:1, omissions:vec![], uncertainty:vec![], negative_evidence:vec![], local_data:true } }
    fn request(candidates: Vec<DesignCandidate>) -> ExperimentObjective { ExperimentObjective { request_id:"request:design-copilot".into(), operator_id:"operator:curator".into(), workflow_id:"workflow:high-throughput-design".into(), benchmark_id:"benchmark:hidden-family".into(), purpose:"benchmarking".into(), scope:"organoid:neural".into(), semantic_profile:"preclinical-neural".into(), required_candidate_order:vec!["candidate:a".into()], required_factor_order:vec!["dose".into(),"time".into()], baseline_digest:h("baseline"), candidates, replay_identity:h("replay"), declared_tool_id:"tool:design-copilot".into(), action_allow_list:vec!["generate-experiment-design".into()], max_actions:8, budget_units:8, policy_allow:true, protected_closure:true, signed_approval:true, raw_data_local:true, boundary:PRECLINICAL_BOUNDARY.into() } }
    #[test] fn manifest_is_a2_and_declared_tool_scoped(){let m=experiment_design_copilot_manifest();m.validate().unwrap();assert_eq!(m.autonomy_tier,AutonomyTier::A2);assert!(m.permissions.contains("invoke:declared-tools"));}
    #[test] fn qualified_design_invokes_tool(){let r=compile_experiment_design_copilot(&request(vec![candidate("candidate:a",EvidenceState::Supported,900)])).unwrap();assert_eq!(r.disposition,CopilotDisposition::Qualified);assert!(r.effect_receipts[0].starts_with("invoke:declared-tool:"));}
    #[test] fn underpowered_design_is_partial(){let r=compile_experiment_design_copilot(&request(vec![candidate("candidate:a",EvidenceState::Supported,700)])).unwrap();assert_eq!(r.disposition,CopilotDisposition::Unknown);assert!(!r.uncertainty.is_empty());}
    #[test] fn unknown_and_contradicted_are_retained(){let r=compile_experiment_design_copilot(&request(vec![candidate("candidate:a",EvidenceState::Unknown,900),candidate("candidate:b",EvidenceState::Contradicted,900)])).unwrap();assert!(r.unknown_order.contains(&"candidate:a".into()));assert!(r.blocked_order.contains(&"candidate:b".into()));}
    #[test] fn policy_denial_blocks(){let mut q=request(vec![candidate("candidate:a",EvidenceState::Supported,900)]);q.policy_allow=false;let r=compile_experiment_design_copilot(&q).unwrap();assert_eq!(r.disposition,CopilotDisposition::Blocked);assert_eq!(r.effect_receipts,vec!["block:unsafe-release"]);}
    #[test] fn duplicate_candidates_rejected(){let q=request(vec![candidate("candidate:a",EvidenceState::Supported,900),candidate("candidate:a",EvidenceState::Supported,900)]);assert!(compile_experiment_design_copilot(&q).is_err());}
    #[test] fn ranking_and_digest_are_deterministic(){let a=compile_experiment_design_copilot(&request(vec![candidate("candidate:b",EvidenceState::Supported,850),candidate("candidate:a",EvidenceState::Supported,900)])).unwrap();let b=compile_experiment_design_copilot(&request(vec![candidate("candidate:a",EvidenceState::Supported,900),candidate("candidate:b",EvidenceState::Supported,850)])).unwrap();assert_eq!(a.ranked_order,b.ranked_order);assert_eq!(a.plan_digest,b.plan_digest);}
}
