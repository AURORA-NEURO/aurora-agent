//! Clone-outcome-driven continuation planning for the autonomous preclinical workflow.
//!
//! This is the orchestration bridge after clone-panel interpretation: unresolved or contradictory
//! cells become bounded next actions with dependency closure, budget accounting, approval gates,
//! instrument/federation policy, and explicit omissions. The planner compiles a plan only; an
//! institution-owned executor remains responsible for any real assay or data movement.

use crate::glioma::programs::p10_interpretation_replication::{
    ClonePanelOutcomeAnalysis, ClonePanelOutcomeDisposition,
};
use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P07-F28";
pub const OUTPUT_SCHEMA: &str = "GliomaCloneContinuationPlan1@1";
pub const MAX_CANDIDATES: usize = 4_096;
pub const MAX_SELECTED: usize = 256;
pub const MAX_BUDGET_MILLI: u64 = 1_000_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloneContinuationActionKind {
    Measure,
    Retest,
    Replicate,
    Simulate,
    FederatedReview,
    EvidenceRelease,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloneContinuationCandidate {
    pub action_id: String,
    pub target_id: String,
    pub kind: CloneContinuationActionKind,
    pub cost_milli: u64,
    pub information_gain_milli: u16,
    pub risk_tier: u8,
    pub dependencies: Vec<String>,
    pub requires_instrument: bool,
    pub requires_federation: bool,
    pub description: String,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloneContinuationRequest {
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub budget_milli: u64,
    pub max_selected: usize,
    pub max_risk_tier: u8,
    pub require_approval: bool,
    pub allow_instrument_actions: bool,
    pub allow_federation: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloneContinuationActionStatus {
    Selected,
    ApprovalRequired,
    Deferred,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloneContinuationDecision {
    pub action_id: String,
    pub target_id: String,
    pub kind: CloneContinuationActionKind,
    pub status: CloneContinuationActionStatus,
    pub dependency_order: Vec<String>,
    pub score_milli: u64,
    pub cost_milli: u64,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloneContinuationDisposition {
    Qualified,
    ApprovalRequired,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloneContinuationPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub source_outcome_digest: ContentHash,
    pub candidate_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub ready_order: Vec<String>,
    pub pending_approval_order: Vec<String>,
    pub deferred_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unresolved_target_order: Vec<String>,
    pub decisions: Vec<CloneContinuationDecision>,
    pub total_cost_milli: u64,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub disposition: CloneContinuationDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CloneContinuationError {
    #[error("clone continuation request is invalid: {0}")]
    InvalidRequest(String),
    #[error("clone continuation candidate is invalid: {0}")]
    InvalidCandidate(String),
    #[error("clone continuation output is invalid: {0}")]
    InvalidOutput(String),
    #[error("clone continuation dependency graph is invalid: {0}")]
    Dependency(String),
    #[error("clone continuation digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn valid_identifier(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:-@>".contains(&byte))
}

fn digest_input(output: &CloneContinuationPlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "study_id": output.study_id,
        "model_system": output.model_system,
        "source_outcome_digest": output.source_outcome_digest,
        "candidate_order": output.candidate_order,
        "selected_order": output.selected_order,
        "ready_order": output.ready_order,
        "pending_approval_order": output.pending_approval_order,
        "deferred_order": output.deferred_order,
        "blocked_order": output.blocked_order,
        "unresolved_target_order": output.unresolved_target_order,
        "decisions": output.decisions,
        "total_cost_milli": output.total_cost_milli,
        "uncertainty": output.uncertainty,
        "negative_evidence": output.negative_evidence,
        "disposition": output.disposition,
    })
}

fn validate_request(request: &CloneContinuationRequest) -> Result<(), CloneContinuationError> {
    if !valid_identifier(&request.study_id)
        || request.budget_milli == 0
        || request.budget_milli > MAX_BUDGET_MILLI
        || request.max_selected == 0
        || request.max_selected > MAX_SELECTED
    {
        return Err(CloneContinuationError::InvalidRequest(
            "study, budget, and selection bounds are invalid".into(),
        ));
    }
    Ok(())
}

fn validate_candidate(
    candidate: &CloneContinuationCandidate,
) -> Result<(), CloneContinuationError> {
    if !valid_identifier(&candidate.action_id)
        || !valid_identifier(&candidate.target_id)
        || candidate.cost_milli == 0
        || candidate.cost_milli > MAX_BUDGET_MILLI
        || candidate.information_gain_milli == 0
        || candidate.information_gain_milli > 1_000
        || candidate
            .dependencies
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || candidate
            .dependencies
            .iter()
            .any(|dependency| !valid_identifier(dependency) || dependency == &candidate.action_id)
        || candidate.description.trim().is_empty()
    {
        return Err(CloneContinuationError::InvalidCandidate(
            "candidate identity, target, cost, information, dependencies, risk, or description is invalid".into(),
        ));
    }
    candidate
        .artifact
        .validate()
        .map_err(|error| CloneContinuationError::InvalidCandidate(error.to_string()))
}

fn dependency_order(
    action_id: &str,
    candidates: &BTreeMap<String, CloneContinuationCandidate>,
    visiting: &mut BTreeSet<String>,
    visited: &mut BTreeSet<String>,
    order: &mut Vec<String>,
) -> Result<(), CloneContinuationError> {
    if visited.contains(action_id) {
        return Ok(());
    }
    if !visiting.insert(action_id.to_string()) {
        return Err(CloneContinuationError::Dependency(format!(
            "dependency cycle reaches {action_id}"
        )));
    }
    let candidate = candidates.get(action_id).ok_or_else(|| {
        CloneContinuationError::Dependency(format!("unknown dependency {action_id}"))
    })?;
    for dependency in &candidate.dependencies {
        dependency_order(dependency, candidates, visiting, visited, order)?;
    }
    visiting.remove(action_id);
    visited.insert(action_id.to_string());
    order.push(action_id.to_string());
    Ok(())
}

impl CloneContinuationPlan {
    pub fn validate(&self) -> Result<(), CloneContinuationError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !valid_identifier(&self.study_id)
            || !canonical(&self.candidate_order)
            || !canonical(&self.ready_order)
            || !canonical(&self.pending_approval_order)
            || !canonical(&self.deferred_order)
            || !canonical(&self.blocked_order)
            || !canonical(&self.unresolved_target_order)
            || !canonical(&self.uncertainty)
            || !canonical(&self.negative_evidence)
            || self.selected_order.len() > MAX_SELECTED
            || self.total_cost_milli > MAX_BUDGET_MILLI
            || self.decisions.len() != self.candidate_order.len()
        {
            return Err(CloneContinuationError::InvalidOutput(
                "plan identity, ordering, bounds, or cardinality invariant failed".into(),
            ));
        }
        let candidates = self.candidate_order.iter().collect::<BTreeSet<_>>();
        let selected = self.selected_order.iter().collect::<BTreeSet<_>>();
        let deferred = self.deferred_order.iter().collect::<BTreeSet<_>>();
        let blocked = self.blocked_order.iter().collect::<BTreeSet<_>>();
        if !selected.is_subset(&candidates)
            || !deferred.is_subset(&candidates)
            || !blocked.is_subset(&candidates)
            || selected.intersection(&deferred).next().is_some()
            || selected.intersection(&blocked).next().is_some()
            || self.decisions.iter().any(|decision| {
                !candidates.contains(&decision.action_id) || decision.score_milli == 0
            })
        {
            return Err(CloneContinuationError::InvalidOutput(
                "plan action partitions or decisions do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| CloneContinuationError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(CloneContinuationError::InvalidOutput(
                "continuation plan digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Compile a bounded dependency-closed continuation plan from clone-panel outcome evidence.
/// This function selects no physical effect; it only produces typed work for a gated executor.
pub fn plan_glioma_clone_continuation(
    request: &CloneContinuationRequest,
    outcome: &ClonePanelOutcomeAnalysis,
    candidates: &[CloneContinuationCandidate],
) -> Result<CloneContinuationPlan, CloneContinuationError> {
    validate_request(request)?;
    outcome
        .validate()
        .map_err(|error| CloneContinuationError::InvalidOutput(error.to_string()))?;
    if outcome.study_id != request.study_id || outcome.model_system != request.model_system {
        return Err(CloneContinuationError::InvalidRequest(
            "outcome study/model binding does not match continuation request".into(),
        ));
    }
    if candidates.is_empty() || candidates.len() > MAX_CANDIDATES {
        return Err(CloneContinuationError::InvalidRequest(
            "candidate set is empty or exceeds the bounded continuation limit".into(),
        ));
    }
    let target_order = outcome.next_action_order.clone();
    let targets = target_order.iter().cloned().collect::<BTreeSet<_>>();
    let mut map = BTreeMap::new();
    for candidate in candidates {
        validate_candidate(candidate)?;
        if map
            .insert(candidate.action_id.clone(), candidate.clone())
            .is_some()
        {
            return Err(CloneContinuationError::InvalidCandidate(
                "action identifiers must be unique".into(),
            ));
        }
    }
    for candidate in map.values() {
        for dependency in &candidate.dependencies {
            if !map.contains_key(dependency) {
                return Err(CloneContinuationError::Dependency(format!(
                    "{} depends on unknown action {}",
                    candidate.action_id, dependency
                )));
            }
        }
    }
    let mut rank = Vec::new();
    for candidate in map.values() {
        let score =
            (u64::from(candidate.information_gain_milli) * 1_000_000) / candidate.cost_milli.max(1);
        rank.push((
            score,
            candidate.information_gain_milli,
            candidate.action_id.clone(),
        ));
    }
    rank.sort_by(|left, right| {
        right
            .0
            .cmp(&left.0)
            .then_with(|| right.1.cmp(&left.1))
            .then_with(|| left.2.cmp(&right.2))
    });
    let mut selected_ids = BTreeSet::new();
    let mut selected_order = Vec::new();
    let mut total_cost = 0u64;
    let mut decisions = BTreeMap::<String, CloneContinuationDecision>::new();
    let mut deferred_order = BTreeSet::new();
    let mut blocked_order = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();
    for (_, _, action_id) in rank {
        let candidate = &map[&action_id];
        let mut closure = Vec::new();
        dependency_order(
            &action_id,
            &map,
            &mut BTreeSet::new(),
            &mut BTreeSet::new(),
            &mut closure,
        )?;
        if !targets.contains(&candidate.target_id) {
            blocked_order.insert(action_id.clone());
            uncertainty.insert(format!("target-not-requested:{}", candidate.action_id));
            decisions.insert(
                action_id.clone(),
                CloneContinuationDecision {
                    action_id: action_id.clone(),
                    target_id: candidate.target_id.clone(),
                    kind: candidate.kind,
                    status: CloneContinuationActionStatus::Blocked,
                    dependency_order: closure,
                    score_milli: 1,
                    cost_milli: candidate.cost_milli,
                    reason: "candidate does not address a declared continuation target".into(),
                },
            );
            continue;
        }
        let gate_block = closure.iter().filter_map(|id| {
            let item = &map[id];
            if item.risk_tier > request.max_risk_tier {
                Some(format!(
                    "risk-tier:{}>{}",
                    item.action_id, request.max_risk_tier
                ))
            } else if item.requires_instrument && !request.allow_instrument_actions {
                Some(format!("instrument-disabled:{}", item.action_id))
            } else if item.requires_federation && !request.allow_federation {
                Some(format!("federation-disabled:{}", item.action_id))
            } else {
                None
            }
        });
        let gate_reasons = gate_block.collect::<Vec<_>>();
        if !gate_reasons.is_empty() {
            blocked_order.insert(action_id.clone());
            uncertainty.extend(gate_reasons.iter().cloned());
            decisions.insert(
                action_id.clone(),
                CloneContinuationDecision {
                    action_id: action_id.clone(),
                    target_id: candidate.target_id.clone(),
                    kind: candidate.kind,
                    status: CloneContinuationActionStatus::Blocked,
                    dependency_order: closure,
                    score_milli: 1,
                    cost_milli: candidate.cost_milli,
                    reason: gate_reasons.join(","),
                },
            );
            continue;
        }
        let new_closure = closure
            .iter()
            .filter(|id| !selected_ids.contains(*id))
            .cloned()
            .collect::<Vec<_>>();
        let closure_cost = new_closure.iter().map(|id| map[id].cost_milli).sum::<u64>();
        if selected_ids.len() + new_closure.len() > request.max_selected {
            deferred_order.insert(action_id.clone());
            uncertainty.insert(format!("selection-limit:{}", action_id));
            decisions.insert(
                action_id.clone(),
                CloneContinuationDecision {
                    action_id: action_id.clone(),
                    target_id: candidate.target_id.clone(),
                    kind: candidate.kind,
                    status: CloneContinuationActionStatus::Deferred,
                    dependency_order: closure,
                    score_milli: 1,
                    cost_milli: candidate.cost_milli,
                    reason: "dependency closure exceeds the action-count bound".into(),
                },
            );
            continue;
        }
        if total_cost.saturating_add(closure_cost) > request.budget_milli {
            deferred_order.insert(action_id.clone());
            uncertainty.insert(format!("budget-shortfall:{}", action_id));
            decisions.insert(
                action_id.clone(),
                CloneContinuationDecision {
                    action_id: action_id.clone(),
                    target_id: candidate.target_id.clone(),
                    kind: candidate.kind,
                    status: CloneContinuationActionStatus::Deferred,
                    dependency_order: closure,
                    score_milli: 1,
                    cost_milli: candidate.cost_milli,
                    reason: "dependency closure exceeds the remaining continuation budget".into(),
                },
            );
            continue;
        }
        for id in &new_closure {
            selected_ids.insert(id.clone());
            selected_order.push(id.clone());
            total_cost = total_cost.saturating_add(map[id].cost_milli);
            let item = &map[id];
            let status = if request.require_approval {
                CloneContinuationActionStatus::ApprovalRequired
            } else {
                CloneContinuationActionStatus::Selected
            };
            decisions.insert(
                id.clone(),
                CloneContinuationDecision {
                    action_id: id.clone(),
                    target_id: item.target_id.clone(),
                    kind: item.kind,
                    status,
                    dependency_order: closure.clone(),
                    score_milli: 1,
                    cost_milli: item.cost_milli,
                    reason: "selected by deterministic information-gain per cost with dependency closure".into(),
                },
            );
        }
    }
    for candidate in map.values() {
        if !selected_ids.contains(&candidate.action_id)
            && !deferred_order.contains(&candidate.action_id)
            && !blocked_order.contains(&candidate.action_id)
        {
            deferred_order.insert(candidate.action_id.clone());
            decisions.insert(
                candidate.action_id.clone(),
                CloneContinuationDecision {
                    action_id: candidate.action_id.clone(),
                    target_id: candidate.target_id.clone(),
                    kind: candidate.kind,
                    status: CloneContinuationActionStatus::Deferred,
                    dependency_order: vec![candidate.action_id.clone()],
                    score_milli: 1,
                    cost_milli: candidate.cost_milli,
                    reason: "not admitted after higher-ranked continuation actions".into(),
                },
            );
        }
    }
    let selected_targets = selected_ids
        .iter()
        .map(|id| map[id].target_id.clone())
        .collect::<BTreeSet<_>>();
    let unresolved_target_order = targets
        .difference(&selected_targets)
        .cloned()
        .collect::<Vec<_>>();
    for target in &unresolved_target_order {
        uncertainty.insert(format!("unresolved-target:{target}"));
        negative_evidence.insert(format!("no-admitted-action-for:{target}"));
    }
    if matches!(
        outcome.disposition,
        ClonePanelOutcomeDisposition::Negative | ClonePanelOutcomeDisposition::Unresolved
    ) {
        uncertainty.insert(format!(
            "source-outcome-disposition:{:?}",
            outcome.disposition
        ));
    }
    let candidate_order = map.keys().cloned().collect::<Vec<_>>();
    let decisions = candidate_order
        .iter()
        .map(|id| decisions.remove(id).expect("decision for every candidate"))
        .collect::<Vec<_>>();
    let ready_order = decisions
        .iter()
        .filter(|decision| decision.status == CloneContinuationActionStatus::Selected)
        .map(|decision| decision.action_id.clone())
        .collect::<Vec<_>>();
    let pending_approval_order = decisions
        .iter()
        .filter(|decision| decision.status == CloneContinuationActionStatus::ApprovalRequired)
        .map(|decision| decision.action_id.clone())
        .collect::<Vec<_>>();
    let disposition = if selected_order.is_empty() {
        CloneContinuationDisposition::Unresolved
    } else if request.require_approval && !pending_approval_order.is_empty() {
        CloneContinuationDisposition::ApprovalRequired
    } else if unresolved_target_order.is_empty() && blocked_order.is_empty() {
        CloneContinuationDisposition::Qualified
    } else {
        CloneContinuationDisposition::Partial
    };
    let mut output = CloneContinuationPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        study_id: request.study_id.clone(),
        model_system: request.model_system,
        source_outcome_digest: outcome.digest.clone(),
        candidate_order,
        selected_order,
        ready_order,
        pending_approval_order,
        deferred_order: deferred_order.into_iter().collect(),
        blocked_order: blocked_order.into_iter().collect(),
        unresolved_target_order,
        decisions,
        total_cost_milli: total_cost,
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative_evidence.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-clone-continuation"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| CloneContinuationError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p05_mechanism_exploration::{
        analyze_glioma_clonal_evolution, ClonalEvolutionRequest, CloneMarker, CloneMarkerState,
        CloneProfile,
    };
    use crate::glioma::programs::p06_experiment_design::{
        plan_glioma_clone_perturbation_panel, ClonePerturbationCandidate, ClonePerturbationKind,
        ClonePerturbationPanelRequest,
    };
    use crate::glioma::programs::p10_interpretation_replication::{
        analyze_glioma_clone_panel_outcomes, ClonePanelMeasurementState, ClonePanelObservation,
        ClonePanelOutcomeRequest,
    };
    use bioprism_ids::ContentHash;

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: id.into(),
            content_hash: ContentHash::of_bytes(id.as_bytes()),
            content_type: "application/json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn outcome() -> ClonePanelOutcomeAnalysis {
        let profile = |id: &str, clone_id: &str, timepoint: u32, markers: &[&str]| CloneProfile {
            profile_id: id.into(),
            study_id: "continuation-study".into(),
            sample_lineage: "lineage-a".into(),
            clone_id: clone_id.into(),
            timepoint,
            model_system: GliomaModelSystem::Organoid,
            abundance_milli: if timepoint == 0 { 400 } else { 600 },
            artifact: artifact(id),
            markers: markers
                .iter()
                .map(|marker_id| CloneMarker {
                    marker_id: (*marker_id).into(),
                    state: CloneMarkerState::Present,
                    confidence_milli: 900,
                })
                .collect(),
        };
        let graph = analyze_glioma_clonal_evolution(
            &ClonalEvolutionRequest {
                study_id: "continuation-study".into(),
                model_system: GliomaModelSystem::Organoid,
                min_shared_markers: 1,
                min_parent_score_milli: 500,
                max_time_gap: 5,
                min_abundance_milli: 1,
                allow_parallel_branches: true,
                max_parent_candidates: 2,
            },
            &[
                profile("root", "clone-a", 0, &["egfr", "tp53"]),
                profile("ec", "clone-b", 1, &["egfr", "tp53", "ecDNA"]),
                profile("pt", "clone-c", 1, &["egfr", "tp53", "pten"]),
            ],
        )
        .unwrap();
        let panel = plan_glioma_clone_perturbation_panel(
            &ClonePerturbationPanelRequest {
                study_id: "continuation-study".into(),
                model_system: GliomaModelSystem::Organoid,
                budget_milli: 10,
                min_coverage_milli: 500,
                max_selected: 2,
                require_branch_coverage: true,
                allow_uncertain_targets: true,
            },
            &graph,
            &[
                ClonePerturbationCandidate {
                    candidate_id: "ec-panel".into(),
                    kind: ClonePerturbationKind::Inhibit,
                    target_marker_order: vec!["ecDNA".into()],
                    cost_milli: 5,
                    expected_effect_milli: 900,
                    purpose: "branch assay".into(),
                    artifact: artifact("ec-panel"),
                },
                ClonePerturbationCandidate {
                    candidate_id: "pt-panel".into(),
                    kind: ClonePerturbationKind::Inhibit,
                    target_marker_order: vec!["pten".into()],
                    cost_milli: 5,
                    expected_effect_milli: 900,
                    purpose: "branch assay".into(),
                    artifact: artifact("pt-panel"),
                },
            ],
        )
        .unwrap();
        let mut observations = panel
            .branch_coverage
            .iter()
            .flat_map(|branch| {
                branch
                    .selected_candidate_order
                    .iter()
                    .flat_map(move |candidate_id| {
                        (0..2).map(move |replicate| ClonePanelObservation {
                            observation_id: format!("obs-{candidate_id}-{replicate}"),
                            study_id: "continuation-study".into(),
                            model_system: GliomaModelSystem::Organoid,
                            candidate_id: candidate_id.clone(),
                            branch_id: branch.branch_id.clone(),
                            replicate_id: format!("r{replicate}"),
                            state: ClonePanelMeasurementState::Measured,
                            effect_milli: -800,
                            uncertainty_milli: 100,
                            artifact: artifact(&format!("obs-{candidate_id}-{replicate}")),
                        })
                    })
            })
            .collect::<Vec<_>>();
        // Leave one selected cell unmeasured so the continuation planner receives a real
        // unresolved target instead of a qualified terminal outcome.
        observations.pop();
        analyze_glioma_clone_panel_outcomes(
            &ClonePanelOutcomeRequest {
                study_id: "continuation-study".into(),
                model_system: GliomaModelSystem::Organoid,
                min_replicates: 2,
                effect_threshold_milli: 500,
                max_uncertainty_milli: 200,
                require_all_selected: true,
                require_all_branches: true,
            },
            &panel,
            &observations,
        )
        .unwrap()
    }

    fn request() -> CloneContinuationRequest {
        CloneContinuationRequest {
            study_id: "continuation-study".into(),
            model_system: GliomaModelSystem::Organoid,
            budget_milli: 10,
            max_selected: 3,
            max_risk_tier: 2,
            require_approval: false,
            allow_instrument_actions: false,
            allow_federation: false,
        }
    }

    fn candidate(
        action_id: &str,
        target_id: &str,
        cost_milli: u64,
        dependencies: Vec<String>,
    ) -> CloneContinuationCandidate {
        CloneContinuationCandidate {
            action_id: action_id.into(),
            target_id: target_id.into(),
            kind: CloneContinuationActionKind::Measure,
            cost_milli,
            information_gain_milli: 900,
            risk_tier: 1,
            dependencies,
            requires_instrument: false,
            requires_federation: false,
            description: "measure the declared unresolved clone-panel cell".into(),
            artifact: artifact(action_id),
        }
    }

    #[test]
    fn continuation_closes_dependencies_and_replays() {
        let outcome = outcome();
        let target = outcome.next_action_order[0].clone();
        let candidates = vec![
            candidate("measure-prereq", &target, 2, Vec::new()),
            candidate(
                "measure-followup",
                &target,
                3,
                vec!["measure-prereq".into()],
            ),
        ];
        let first = plan_glioma_clone_continuation(&request(), &outcome, &candidates).unwrap();
        let second = plan_glioma_clone_continuation(&request(), &outcome, &candidates).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.disposition, CloneContinuationDisposition::Qualified);
        assert_eq!(
            first.selected_order,
            vec!["measure-prereq", "measure-followup"]
        );
    }

    #[test]
    fn approval_gate_is_explicit_without_execution() {
        let outcome = outcome();
        let target = outcome.next_action_order[0].clone();
        let mut request = request();
        request.require_approval = true;
        let output = plan_glioma_clone_continuation(
            &request,
            &outcome,
            &[candidate("measure", &target, 2, Vec::new())],
        )
        .unwrap();
        assert_eq!(
            output.disposition,
            CloneContinuationDisposition::ApprovalRequired
        );
        assert_eq!(output.ready_order.len(), 0);
        assert_eq!(output.pending_approval_order, vec!["measure"]);
    }

    #[test]
    fn instrument_gate_blocks_candidate_and_preserves_target() {
        let outcome = outcome();
        let target = outcome.next_action_order[0].clone();
        let mut item = candidate("instrument", &target, 2, Vec::new());
        item.requires_instrument = true;
        let output = plan_glioma_clone_continuation(&request(), &outcome, &[item]).unwrap();
        assert_eq!(output.disposition, CloneContinuationDisposition::Unresolved);
        assert_eq!(output.blocked_order, vec!["instrument"]);
        assert!(output
            .uncertainty
            .iter()
            .any(|item| item == "instrument-disabled:instrument"));
    }

    #[test]
    fn dependency_cycle_is_rejected() {
        let outcome = outcome();
        let target = outcome.next_action_order[0].clone();
        let result = plan_glioma_clone_continuation(
            &request(),
            &outcome,
            &[
                candidate("a", &target, 2, vec!["b".into()]),
                candidate("b", &target, 2, vec!["a".into()]),
            ],
        );
        assert!(matches!(result, Err(CloneContinuationError::Dependency(_))));
    }
}
