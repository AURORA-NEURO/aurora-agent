//! Local single-study verification and safety assurance for glioma mechanism workflows.
//!
//! A compiled workflow is not automatically safe to admit to an institution-local executor.
//! This feature is the substantive pre-dispatch gate: it replays dependency closure, checks
//! local artifact and route boundaries, evaluates freshness and evidence state, enforces risk,
//! approval, compensation, retry, and budget limits, and propagates every failed prerequisite
//! downstream. It emits a deterministic admission plan; it never executes an assay, instrument,
//! network request, or clinical decision.

use super::mechanism_workflow::{MechanismWorkflowNodeStatus, MechanismWorkflowPlan};
use crate::glioma_engine::LocalArtifactRef;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P05-F25";
pub const OUTPUT_SCHEMA: &str = "GliomaMechanismWorkflowAssurance1@1";
pub const MAX_ACTIONS: usize = 1_024;
pub const MAX_OBSERVATIONS: usize = 1_024;
pub const MAX_ROUTE_PREFIXES: usize = 64;
pub const MAX_FAILURES: u16 = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismSafetyEvidenceState {
    Supported,
    Contradicted,
    Unresolved,
    Missing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismAssuranceStatus {
    Admit,
    Review,
    Block,
    Skip,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismAssuranceDisposition {
    Ready,
    Partial,
    Blocked,
    NoAdmissibleActions,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismWorkflowAssurancePolicy {
    pub maximum_risk_milli: u16,
    pub minimum_observation_quality_milli: u16,
    pub maximum_observation_age_epochs: u64,
    pub maximum_prior_failures: u16,
    pub maximum_cost_units: u64,
    pub maximum_admitted_actions: usize,
    pub allow_approval_required: bool,
    pub require_compensation_above_risk_milli: u16,
    pub require_local_inputs: bool,
    pub permitted_execution_route_prefixes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismSafetyObservation {
    pub action_id: String,
    pub observed_epoch: u64,
    pub evidence_state: MechanismSafetyEvidenceState,
    pub quality_milli: u16,
    pub prior_failure_count: u16,
    pub replay_identity: ContentHash,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismWorkflowAssuranceRequest {
    pub objective: String,
    pub workflow: MechanismWorkflowPlan,
    pub policy: MechanismWorkflowAssurancePolicy,
    pub observations: Vec<MechanismSafetyObservation>,
    pub current_epoch: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismAssuranceDecision {
    pub action_id: String,
    pub status: MechanismAssuranceStatus,
    pub safety_score_milli: u16,
    pub observed_epoch: Option<u64>,
    pub challenge_order: Vec<String>,
    pub reasons: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismWorkflowAssurancePlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub workflow_digest: ContentHash,
    pub policy_digest: ContentHash,
    pub current_epoch: u64,
    pub action_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub review_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub skipped_order: Vec<String>,
    pub decisions: Vec<MechanismAssuranceDecision>,
    pub total_admitted_cost_units: u64,
    pub maximum_cost_units: u64,
    pub budget_remaining_units: u64,
    pub coverage_milli: u16,
    pub highest_admitted_risk_milli: u16,
    pub challenge_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: MechanismAssuranceDisposition,
    pub next_route: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MechanismWorkflowAssuranceError {
    #[error("mechanism workflow assurance request is invalid: {0}")]
    InvalidRequest(String),
    #[error("mechanism workflow assurance input is invalid: {0}")]
    InvalidInput(String),
    #[error("mechanism workflow assurance output is invalid: {0}")]
    InvalidOutput(String),
    #[error("mechanism workflow assurance digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_nonempty(values: &[String]) -> bool {
    values.iter().all(|value| !value.trim().is_empty())
        && values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

fn policy_digest(
    policy: &MechanismWorkflowAssurancePolicy,
) -> Result<ContentHash, MechanismWorkflowAssuranceError> {
    ContentHash::of_value(&serde_json::json!(policy))
        .map_err(|error| MechanismWorkflowAssuranceError::Digest(error.to_string()))
}

fn digest_input(output: &MechanismWorkflowAssurancePlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "workflow_digest": output.workflow_digest,
        "policy_digest": output.policy_digest,
        "current_epoch": output.current_epoch,
        "action_order": output.action_order,
        "admitted_order": output.admitted_order,
        "review_order": output.review_order,
        "blocked_order": output.blocked_order,
        "skipped_order": output.skipped_order,
        "decisions": output.decisions,
        "total_admitted_cost_units": output.total_admitted_cost_units,
        "maximum_cost_units": output.maximum_cost_units,
        "budget_remaining_units": output.budget_remaining_units,
        "coverage_milli": output.coverage_milli,
        "highest_admitted_risk_milli": output.highest_admitted_risk_milli,
        "challenge_order": output.challenge_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
        "next_route": output.next_route,
    })
}

fn validate_request(
    request: &MechanismWorkflowAssuranceRequest,
) -> Result<(), MechanismWorkflowAssuranceError> {
    if request.objective.trim().is_empty()
        || request.objective != request.workflow.objective
        || request.current_epoch == 0
        || request.workflow.node_order.is_empty()
        || request.workflow.node_order.len() > MAX_ACTIONS
        || request.observations.len() > MAX_OBSERVATIONS
    {
        return Err(MechanismWorkflowAssuranceError::InvalidRequest(
            "objective/workflow binding, positive epoch, bounded workflow, and observation limits are required".into(),
        ));
    }
    request
        .workflow
        .validate()
        .map_err(|error| MechanismWorkflowAssuranceError::InvalidInput(error.to_string()))?;
    let policy = &request.policy;
    if policy.maximum_risk_milli > 1_000
        || policy.minimum_observation_quality_milli > 1_000
        || policy.maximum_observation_age_epochs == 0
        || policy.maximum_prior_failures > MAX_FAILURES
        || policy.maximum_cost_units == 0
        || policy.maximum_admitted_actions == 0
        || policy.maximum_admitted_actions > MAX_ACTIONS
        || policy.require_compensation_above_risk_milli > 1_000
        || policy.permitted_execution_route_prefixes.is_empty()
        || policy.permitted_execution_route_prefixes.len() > MAX_ROUTE_PREFIXES
        || !canonical(&policy.permitted_execution_route_prefixes)
        || !unique_nonempty(&policy.permitted_execution_route_prefixes)
    {
        return Err(MechanismWorkflowAssuranceError::InvalidInput(
            "risk, freshness, retry, budget, admission, compensation, and route policy bounds are invalid".into(),
        ));
    }
    let action_ids = request
        .workflow
        .node_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut observed_ids = BTreeSet::new();
    for observation in &request.observations {
        if !action_ids.contains(&observation.action_id)
            || observation.observed_epoch == 0
            || observation.observed_epoch > request.current_epoch
            || observation.quality_milli > 1_000
            || observation.prior_failure_count > MAX_FAILURES
            || observation.replay_identity.as_str().len() != 64
            || !observed_ids.insert(observation.action_id.clone())
            || observation.artifact.validate().is_err()
        {
            return Err(MechanismWorkflowAssuranceError::InvalidInput(
                "observations must uniquely cover known actions, be epoch-bounded, content-addressed, and local de-identified artifacts".into(),
            ));
        }
    }
    Ok(())
}

fn route_allowed(route: &str, prefixes: &[String]) -> bool {
    prefixes
        .iter()
        .any(|prefix| route == prefix || route.starts_with(&format!("{prefix}/")))
}

fn push_reason(reasons: &mut Vec<String>, challenge: &str, reason: impl Into<String>) {
    reasons.push(format!("{challenge}:{}", reason.into()));
}

/// Verify and admit a single-study mechanism workflow before local execution.
pub fn assure_glioma_mechanism_workflow(
    request: &MechanismWorkflowAssuranceRequest,
) -> Result<MechanismWorkflowAssurancePlan, MechanismWorkflowAssuranceError> {
    validate_request(request)?;
    let node_map = request
        .workflow
        .nodes
        .iter()
        .map(|node| (node.action.action_id.clone(), node))
        .collect::<BTreeMap<_, _>>();
    let observation_map = request
        .observations
        .iter()
        .map(|observation| (observation.action_id.clone(), observation))
        .collect::<BTreeMap<_, _>>();
    let mut decisions = BTreeMap::<String, MechanismAssuranceDecision>::new();
    let mut admitted = Vec::new();
    let mut review = Vec::new();
    let mut blocked = Vec::new();
    let mut skipped = Vec::new();
    let mut challenge_set = BTreeSet::new();
    let mut negative_evidence = request.workflow.negative_evidence.clone();
    let mut uncertainty = request.workflow.uncertainty.clone();
    let mut total_cost = 0_u64;
    let mut highest_risk = 0_u16;

    for action_id in &request.workflow.topological_order {
        let node = node_map.get(action_id).expect("validated workflow node");
        if node.status != MechanismWorkflowNodeStatus::Scheduled {
            let status = if node.status == MechanismWorkflowNodeStatus::Blocked {
                MechanismAssuranceStatus::Block
            } else {
                MechanismAssuranceStatus::Skip
            };
            let reason = node
                .reason
                .clone()
                .unwrap_or_else(|| "workflow did not schedule this action".into());
            let challenge = if status == MechanismAssuranceStatus::Block {
                "workflow_status_blocked"
            } else {
                "workflow_status_deferred"
            };
            challenge_set.insert(challenge.to_string());
            let decision = MechanismAssuranceDecision {
                action_id: action_id.clone(),
                status,
                safety_score_milli: 0,
                observed_epoch: None,
                challenge_order: vec![challenge.into()],
                reasons: vec![reason],
            };
            if status == MechanismAssuranceStatus::Block {
                blocked.push(action_id.clone());
            } else {
                skipped.push(action_id.clone());
            }
            decisions.insert(action_id.clone(), decision);
            continue;
        }

        let action = &node.action;
        let mut challenges = Vec::new();
        let mut reasons = Vec::new();
        let mut hard_block = false;
        let mut needs_review = false;
        let mut score = 1_000_u16;
        let mut observed_epoch = None;
        macro_rules! mark_block {
            ($challenge:expr, $reason:expr $(,)?) => {{
                let challenge = $challenge;
                challenges.push(challenge.to_string());
                challenge_set.insert(challenge.to_string());
                push_reason(&mut reasons, challenge, $reason);
                hard_block = true;
            }};
        }
        macro_rules! mark_review {
            ($challenge:expr, $reason:expr $(,)?) => {{
                let challenge = $challenge;
                challenges.push(challenge.to_string());
                challenge_set.insert(challenge.to_string());
                push_reason(&mut reasons, challenge, $reason);
                needs_review = true;
            }};
        }

        if action.risk_milli > request.policy.maximum_risk_milli {
            mark_block!("risk_ceiling", format!("risk-milli={}", action.risk_milli));
            negative_evidence.push(format!("{action_id}:risk-ceiling"));
        } else {
            challenges.push("risk_ceiling".into());
            challenge_set.insert("risk_ceiling".into());
        }
        if action.requires_approval && !request.policy.allow_approval_required {
            mark_block!("approval_policy", "approval-required-action-not-authorized",);
            negative_evidence.push(format!("{action_id}:approval-required"));
        } else {
            challenges.push("approval_policy".into());
            challenge_set.insert("approval_policy".into());
        }
        if !route_allowed(
            &action.execution_route,
            &request.policy.permitted_execution_route_prefixes,
        ) {
            mark_block!(
                "route_allowlist",
                format!("route-not-permitted={}", action.execution_route),
            );
            negative_evidence.push(format!("{action_id}:route-not-allowlisted"));
        } else {
            challenges.push("route_allowlist".into());
            challenge_set.insert("route_allowlist".into());
        }
        if request.policy.require_local_inputs && action.input_artifacts.is_empty() {
            mark_block!("local_input_provenance", "no-local-input-artifact");
            negative_evidence.push(format!("{action_id}:missing-local-input"));
        } else {
            challenges.push("local_input_provenance".into());
            challenge_set.insert("local_input_provenance".into());
        }
        if action.risk_milli >= request.policy.require_compensation_above_risk_milli
            && action.compensation_route.is_none()
        {
            mark_block!("compensation_route", "risk-requires-compensation-route",);
            negative_evidence.push(format!("{action_id}:missing-compensation"));
        } else {
            challenges.push("compensation_route".into());
            challenge_set.insert("compensation_route".into());
        }

        if let Some(observation) = observation_map.get(action_id) {
            observed_epoch = Some(observation.observed_epoch);
            let age = request
                .current_epoch
                .saturating_sub(observation.observed_epoch);
            if age > request.policy.maximum_observation_age_epochs {
                mark_review!("observation_freshness", format!("age-epochs={age}"));
                uncertainty.push(format!("{action_id}:stale-observation-age={age}"));
                score = score.min(350);
            } else {
                challenges.push("observation_freshness".into());
                challenge_set.insert("observation_freshness".into());
            }
            if observation.quality_milli < request.policy.minimum_observation_quality_milli {
                mark_review!(
                    "observation_quality",
                    format!("quality-milli={}", observation.quality_milli),
                );
                uncertainty.push(format!(
                    "{action_id}:quality-below-gate={}",
                    observation.quality_milli
                ));
                score = score.min(observation.quality_milli);
            } else {
                challenges.push("observation_quality".into());
                challenge_set.insert("observation_quality".into());
                score = score.min(observation.quality_milli);
            }
            if observation.prior_failure_count > request.policy.maximum_prior_failures {
                mark_block!(
                    "retry_budget",
                    format!("prior-failures={}", observation.prior_failure_count),
                );
                negative_evidence.push(format!("{action_id}:retry-budget-exhausted"));
            } else {
                challenges.push("retry_budget".into());
                challenge_set.insert("retry_budget".into());
            }
            match observation.evidence_state {
                MechanismSafetyEvidenceState::Supported => {
                    challenges.push("evidence_state".into());
                    challenge_set.insert("evidence_state".into());
                }
                MechanismSafetyEvidenceState::Contradicted => {
                    mark_block!("evidence_state", "contradicted-local-evidence");
                    negative_evidence.push(format!("{action_id}:contradicted"));
                }
                MechanismSafetyEvidenceState::Unresolved => {
                    mark_review!("evidence_state", "unresolved-local-evidence");
                    uncertainty.push(format!("{action_id}:unresolved-evidence"));
                }
                MechanismSafetyEvidenceState::Missing => {
                    mark_review!("evidence_state", "missing-local-evidence");
                    uncertainty.push(format!("{action_id}:missing-evidence"));
                }
            }
            challenges.push("replay_identity".into());
            challenge_set.insert("replay_identity".into());
        } else {
            mark_review!("observation_presence", "no-safety-observation");
            uncertainty.push(format!("{action_id}:missing-safety-observation"));
        }

        for dependency in &action.dependency_order {
            match decisions.get(dependency).map(|decision| decision.status) {
                Some(MechanismAssuranceStatus::Admit) => {}
                Some(MechanismAssuranceStatus::Block) => {
                    mark_block!(
                        "dependency_closure",
                        format!("blocked-dependency={dependency}"),
                    );
                    negative_evidence.push(format!("{action_id}:blocked-by={dependency}"));
                }
                Some(MechanismAssuranceStatus::Review | MechanismAssuranceStatus::Skip) | None => {
                    mark_review!(
                        "dependency_closure",
                        format!("unqualified-dependency={dependency}"),
                    );
                    uncertainty.push(format!("{action_id}:dependency-not-admitted={dependency}"));
                }
            }
        }
        if !action.dependency_order.is_empty() {
            challenges.push("dependency_closure".into());
            challenge_set.insert("dependency_closure".into());
        }

        let mut status = if hard_block {
            MechanismAssuranceStatus::Block
        } else if needs_review {
            MechanismAssuranceStatus::Review
        } else {
            MechanismAssuranceStatus::Admit
        };
        if status == MechanismAssuranceStatus::Admit
            && (admitted.len() >= request.policy.maximum_admitted_actions
                || total_cost.saturating_add(action.cost_units) > request.policy.maximum_cost_units)
        {
            status = MechanismAssuranceStatus::Review;
            challenges.push("rolling_budget".into());
            challenge_set.insert("rolling_budget".into());
            push_reason(
                &mut reasons,
                "rolling_budget",
                "admission-count-or-cost-capacity-exhausted",
            );
            uncertainty.push(format!("{action_id}:rolling-budget-hold"));
        } else {
            challenges.push("rolling_budget".into());
            challenge_set.insert("rolling_budget".into());
        }
        challenges.sort();
        challenges.dedup();
        reasons.sort();
        reasons.dedup();
        let decision = MechanismAssuranceDecision {
            action_id: action_id.clone(),
            status,
            safety_score_milli: score,
            observed_epoch,
            challenge_order: challenges,
            reasons,
        };
        match status {
            MechanismAssuranceStatus::Admit => {
                admitted.push(action_id.clone());
                total_cost = total_cost.saturating_add(action.cost_units);
                highest_risk = highest_risk.max(action.risk_milli);
            }
            MechanismAssuranceStatus::Review => review.push(action_id.clone()),
            MechanismAssuranceStatus::Block => blocked.push(action_id.clone()),
            MechanismAssuranceStatus::Skip => skipped.push(action_id.clone()),
        }
        decisions.insert(action_id.clone(), decision);
    }

    admitted.sort();
    review.sort();
    blocked.sort();
    skipped.sort();
    let action_order = request.workflow.node_order.clone();
    let decisions = action_order
        .iter()
        .map(|action_id| decisions.remove(action_id).expect("decision coverage"))
        .collect::<Vec<_>>();
    challenge_set.extend(
        decisions
            .iter()
            .flat_map(|decision| decision.challenge_order.iter().cloned()),
    );
    let mut challenge_order = challenge_set.into_iter().collect::<Vec<_>>();
    challenge_order.sort();
    negative_evidence.sort();
    negative_evidence.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    let scheduled_count = request
        .workflow
        .nodes
        .iter()
        .filter(|node| node.status == MechanismWorkflowNodeStatus::Scheduled)
        .count();
    let coverage_milli = if scheduled_count == 0 {
        0
    } else {
        ((admitted.len() as u64 * 1_000) / scheduled_count as u64).min(1_000) as u16
    };
    let disposition = if admitted.is_empty() {
        if !blocked.is_empty() {
            MechanismAssuranceDisposition::Blocked
        } else {
            MechanismAssuranceDisposition::NoAdmissibleActions
        }
    } else if review.is_empty() && blocked.is_empty() && skipped.is_empty() {
        MechanismAssuranceDisposition::Ready
    } else {
        MechanismAssuranceDisposition::Partial
    };
    let next_route = if !admitted.is_empty() {
        "glioma_mechanism_operating_cycle"
    } else {
        "glioma_mechanism_feedback_replan"
    };
    let mut output = MechanismWorkflowAssurancePlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        workflow_digest: request.workflow.digest.clone(),
        policy_digest: policy_digest(&request.policy)?,
        current_epoch: request.current_epoch,
        action_order,
        admitted_order: admitted,
        review_order: review,
        blocked_order: blocked,
        skipped_order: skipped,
        decisions,
        total_admitted_cost_units: total_cost,
        maximum_cost_units: request.policy.maximum_cost_units,
        budget_remaining_units: request.policy.maximum_cost_units.saturating_sub(total_cost),
        coverage_milli,
        highest_admitted_risk_milli: highest_risk,
        challenge_order,
        negative_evidence,
        uncertainty,
        disposition,
        next_route: next_route.into(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-mechanism-workflow-assurance"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| MechanismWorkflowAssuranceError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

impl MechanismWorkflowAssurancePlan {
    pub fn validate(&self) -> Result<(), MechanismWorkflowAssuranceError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.workflow_digest.as_str().len() != 64
            || self.policy_digest.as_str().len() != 64
            || self.current_epoch == 0
            || !canonical(&self.action_order)
            || !unique_nonempty(&self.action_order)
            || self.decisions.len() != self.action_order.len()
            || self
                .decisions
                .iter()
                .map(|decision| decision.action_id.clone())
                .collect::<Vec<_>>()
                != self.action_order
            || !canonical(&self.admitted_order)
            || !canonical(&self.review_order)
            || !canonical(&self.blocked_order)
            || !canonical(&self.skipped_order)
            || !unique_nonempty(&self.admitted_order)
            || !unique_nonempty(&self.review_order)
            || !unique_nonempty(&self.blocked_order)
            || !unique_nonempty(&self.skipped_order)
            || !canonical(&self.challenge_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.maximum_cost_units == 0
            || self.total_admitted_cost_units > self.maximum_cost_units
            || self
                .total_admitted_cost_units
                .saturating_add(self.budget_remaining_units)
                != self.maximum_cost_units
            || self.coverage_milli > 1_000
            || self.highest_admitted_risk_milli > 1_000
            || self.next_route.trim().is_empty()
        {
            return Err(MechanismWorkflowAssuranceError::InvalidOutput(
                "identity, ordering, decision coverage, budget, score, or digest shape is invalid"
                    .into(),
            ));
        }
        let all_ids = self.action_order.iter().cloned().collect::<BTreeSet<_>>();
        let admitted = self.admitted_order.iter().cloned().collect::<BTreeSet<_>>();
        let review = self.review_order.iter().cloned().collect::<BTreeSet<_>>();
        let blocked = self.blocked_order.iter().cloned().collect::<BTreeSet<_>>();
        let skipped = self.skipped_order.iter().cloned().collect::<BTreeSet<_>>();
        let partition = admitted
            .union(&review)
            .cloned()
            .collect::<BTreeSet<_>>()
            .union(&blocked)
            .cloned()
            .collect::<BTreeSet<_>>()
            .union(&skipped)
            .cloned()
            .collect::<BTreeSet<_>>();
        let status_ids = |status: MechanismAssuranceStatus| {
            self.decisions
                .iter()
                .filter(|decision| decision.status == status)
                .map(|decision| decision.action_id.clone())
                .collect::<BTreeSet<_>>()
        };
        if partition != all_ids
            || admitted.len() + review.len() + blocked.len() + skipped.len() != partition.len()
            || status_ids(MechanismAssuranceStatus::Admit) != admitted
            || status_ids(MechanismAssuranceStatus::Review) != review
            || status_ids(MechanismAssuranceStatus::Block) != blocked
            || status_ids(MechanismAssuranceStatus::Skip) != skipped
            || self.decisions.iter().any(|decision| {
                decision.action_id.trim().is_empty()
                    || decision.safety_score_milli > 1_000
                    || !canonical(&decision.challenge_order)
                    || !unique_nonempty(&decision.challenge_order)
                    || !canonical(&decision.reasons)
                    || !unique_nonempty(&decision.reasons)
            })
        {
            return Err(MechanismWorkflowAssuranceError::InvalidOutput(
                "status partition, decision ordering, or action coverage does not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| MechanismWorkflowAssuranceError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(MechanismWorkflowAssuranceError::Digest(
                "mechanism workflow assurance digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p05_mechanism_exploration::mechanism_workflow::{
        MechanismWorkflowAction, MechanismWorkflowDisposition, MechanismWorkflowNode,
    };
    use crate::glioma_engine::{GliomaModality, GliomaModelSystem};

    fn hash(seed: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"seed": seed})).unwrap()
    }

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: id.into(),
            content_hash: ContentHash::of_bytes(id.as_bytes()),
            content_type: "application/vnd.aurora.glioma-mechanism+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn action(id: &str, dependencies: Vec<String>, risk_milli: u16) -> MechanismWorkflowAction {
        MechanismWorkflowAction {
            action_id: id.into(),
            mechanism_id: "m-invasion".into(),
            model_system: GliomaModelSystem::Organoid,
            modality: GliomaModality::Imaging,
            dependency_order: dependencies,
            cost_units: 1,
            risk_milli,
            requires_approval: false,
            input_artifacts: vec![artifact(&format!("input-{id}"))],
            execution_route: "local/imaging".into(),
            compensation_route: Some("local/compensate".into()),
        }
    }

    fn workflow() -> MechanismWorkflowPlan {
        let first = action("a-first", vec![], 200);
        let second = action("b-second", vec!["a-first".into()], 300);
        let mut output = MechanismWorkflowPlan {
            feature_id: super::super::mechanism_workflow::FEATURE_ID.into(),
            output_schema: super::super::mechanism_workflow::OUTPUT_SCHEMA.into(),
            objective: "assure mechanism workflow".into(),
            replan_digest: hash("replan"),
            node_order: vec!["a-first".into(), "b-second".into()],
            topological_order: vec!["a-first".into(), "b-second".into()],
            execution_waves: vec![vec!["a-first".into()], vec!["b-second".into()]],
            nodes: vec![
                MechanismWorkflowNode {
                    action: first,
                    status: MechanismWorkflowNodeStatus::Scheduled,
                    wave: Some(0),
                    cumulative_cost_units: 1,
                    reason: None,
                },
                MechanismWorkflowNode {
                    action: second,
                    status: MechanismWorkflowNodeStatus::Scheduled,
                    wave: Some(1),
                    cumulative_cost_units: 2,
                    reason: None,
                },
            ],
            scheduled_action_order: vec!["a-first".into(), "b-second".into()],
            deferred_action_order: vec![],
            blocked_action_order: vec![],
            total_cost_units: 2,
            budget_units: 2,
            budget_remaining_units: 0,
            critical_path_units: 2,
            negative_evidence: vec![],
            uncertainty: vec![],
            disposition: MechanismWorkflowDisposition::Ready,
            next_route: "glioma_mechanism_operating_cycle".into(),
            digest: hash("unsealed"),
        };
        output.digest =
            ContentHash::of_value(&super::super::mechanism_workflow::digest_input(&output))
                .unwrap();
        output.validate().unwrap();
        output
    }

    fn policy() -> MechanismWorkflowAssurancePolicy {
        MechanismWorkflowAssurancePolicy {
            maximum_risk_milli: 700,
            minimum_observation_quality_milli: 700,
            maximum_observation_age_epochs: 2,
            maximum_prior_failures: 2,
            maximum_cost_units: 2,
            maximum_admitted_actions: 2,
            allow_approval_required: false,
            require_compensation_above_risk_milli: 700,
            require_local_inputs: true,
            permitted_execution_route_prefixes: vec!["local".into()],
        }
    }

    fn observation(
        id: &str,
        epoch: u64,
        state: MechanismSafetyEvidenceState,
    ) -> MechanismSafetyObservation {
        MechanismSafetyObservation {
            action_id: id.into(),
            observed_epoch: epoch,
            evidence_state: state,
            quality_milli: 900,
            prior_failure_count: 0,
            replay_identity: hash(&format!("replay-{id}")),
            artifact: artifact(&format!("observation-{id}")),
        }
    }

    #[test]
    fn admits_dependency_closed_local_workflow() {
        let request = MechanismWorkflowAssuranceRequest {
            objective: "assure mechanism workflow".into(),
            workflow: workflow(),
            policy: policy(),
            observations: vec![
                observation("a-first", 4, MechanismSafetyEvidenceState::Supported),
                observation("b-second", 4, MechanismSafetyEvidenceState::Supported),
            ],
            current_epoch: 5,
        };
        let plan = assure_glioma_mechanism_workflow(&request).unwrap();
        assert_eq!(plan.admitted_order, vec!["a-first", "b-second"]);
        assert_eq!(plan.coverage_milli, 1_000);
        assert_eq!(plan.disposition, MechanismAssuranceDisposition::Ready);
        plan.validate().unwrap();
    }

    #[test]
    fn contradiction_blocks_downstream_and_stale_evidence_is_reviewed() {
        let request = MechanismWorkflowAssuranceRequest {
            objective: "assure mechanism workflow".into(),
            workflow: workflow(),
            policy: policy(),
            observations: vec![
                observation("a-first", 1, MechanismSafetyEvidenceState::Contradicted),
                observation("b-second", 1, MechanismSafetyEvidenceState::Supported),
            ],
            current_epoch: 5,
        };
        let plan = assure_glioma_mechanism_workflow(&request).unwrap();
        assert_eq!(plan.blocked_order, vec!["a-first", "b-second"]);
        assert!(plan
            .negative_evidence
            .iter()
            .any(|entry| entry.contains("contradicted")));
        assert!(plan
            .uncertainty
            .iter()
            .any(|entry| entry.contains("stale-observation")));
        assert_eq!(plan.next_route, "glioma_mechanism_feedback_replan");
        plan.validate().unwrap();
    }

    #[test]
    fn route_and_budget_gates_hold_without_silent_admission() {
        let mut request = MechanismWorkflowAssuranceRequest {
            objective: "assure mechanism workflow".into(),
            workflow: workflow(),
            policy: policy(),
            observations: vec![
                observation("a-first", 4, MechanismSafetyEvidenceState::Supported),
                observation("b-second", 4, MechanismSafetyEvidenceState::Supported),
            ],
            current_epoch: 5,
        };
        request.policy.maximum_cost_units = 1;
        request.policy.permitted_execution_route_prefixes = vec!["remote".into()];
        let plan = assure_glioma_mechanism_workflow(&request).unwrap();
        assert!(plan.admitted_order.is_empty());
        assert!(plan
            .blocked_order
            .iter()
            .all(|id| id == "a-first" || id == "b-second"));
        assert!(plan
            .negative_evidence
            .iter()
            .any(|entry| entry.contains("route-not-allowlisted")));
        assert_eq!(plan.disposition, MechanismAssuranceDisposition::Blocked);
        plan.validate().unwrap();
    }
}
