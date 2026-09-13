//! Budgeted portfolio planning for reproducible glioma computation DAGs.
//!
//! The P09 executor runs a declared computation graph. This module decides which analyses to run
//! first when a study has more possible multimodal analyses than its compute/time budget allows.
//! It closes prerequisite dependencies, rewards information and coverage debt, preserves required
//! tasks, and refuses missing/cyclic/non-deterministic work under the declared policy. It only
//! compiles an execution order; it never runs external code or moves raw data.

use super::execution::{ComputationOperation, ComputationTask};
use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P09-F11";
pub const OUTPUT_SCHEMA: &str = "GliomaComputationPortfolioPlan1@1";
pub const MAX_CANDIDATES: usize = 4_096;
pub const MAX_MODALITIES: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationCandidate {
    pub candidate_id: String,
    pub task: ComputationTask,
    pub modality: GliomaModality,
    pub information_gain_milli: u32,
    pub uncertainty_reduction_milli: u32,
    pub coverage_debt_milli: u32,
    pub redundancy_group: String,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationPortfolioRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub budget_units: u64,
    pub duration_ticks: u64,
    pub max_tasks: usize,
    pub max_modalities: usize,
    pub min_modalities: usize,
    pub information_weight_milli: u16,
    pub uncertainty_weight_milli: u16,
    pub coverage_weight_milli: u16,
    pub cost_penalty_milli: u16,
    pub duration_penalty_milli: u16,
    pub require_deterministic: bool,
    pub completed_order: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputationCandidateDisposition {
    Selected,
    Deferred,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationCandidateScore {
    pub candidate_id: String,
    pub operation: ComputationOperation,
    pub utility_milli: i64,
    pub dependency_count: u16,
    pub modality: GliomaModality,
    pub disposition: ComputationCandidateDisposition,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputationPortfolioDisposition {
    Qualified,
    Partial,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputationPortfolioPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub candidate_order: Vec<String>,
    pub scores: Vec<ComputationCandidateScore>,
    pub selected_order: Vec<String>,
    pub dependency_order: Vec<String>,
    pub deferred_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub modality_order: Vec<GliomaModality>,
    pub budget_used_units: u64,
    pub duration_used_ticks: u64,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: ComputationPortfolioDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ComputationPortfolioError {
    #[error("computation portfolio request is invalid: {0}")]
    InvalidRequest(String),
    #[error("computation portfolio graph is invalid: {0}")]
    InvalidGraph(String),
    #[error("computation portfolio output is invalid: {0}")]
    InvalidOutput(String),
    #[error("computation portfolio digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(plan: &ComputationPortfolioPlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": plan.feature_id,
        "output_schema": plan.output_schema,
        "objective": plan.objective,
        "model_system": plan.model_system,
        "candidate_order": plan.candidate_order,
        "scores": plan.scores,
        "selected_order": plan.selected_order,
        "dependency_order": plan.dependency_order,
        "deferred_order": plan.deferred_order,
        "blocked_order": plan.blocked_order,
        "unresolved_order": plan.unresolved_order,
        "modality_order": plan.modality_order,
        "budget_used_units": plan.budget_used_units,
        "duration_used_ticks": plan.duration_used_ticks,
        "negative_evidence": plan.negative_evidence,
        "uncertainty": plan.uncertainty,
        "disposition": plan.disposition,
    })
}

impl ComputationPortfolioPlan {
    pub fn validate(&self) -> Result<(), ComputationPortfolioError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !canonical(&self.candidate_order)
            || !canonical(&self.deferred_order)
            || !canonical(&self.blocked_order)
            || !canonical(&self.unresolved_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || !canonical(&self.modality_order)
            || self.scores.len() != self.candidate_order.len()
            || self.scores.iter().any(|score| {
                score.candidate_id.trim().is_empty() || score.rationale.trim().is_empty()
            })
        {
            return Err(ComputationPortfolioError::InvalidOutput(
                "identity, ordering, score cardinality, or rationale bounds are invalid".into(),
            ));
        }
        let all = self
            .candidate_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let score_ids = self
            .scores
            .iter()
            .map(|score| score.candidate_id.clone())
            .collect::<BTreeSet<_>>();
        if all != score_ids {
            return Err(ComputationPortfolioError::InvalidOutput(
                "candidate and score identities do not reconcile".into(),
            ));
        }
        let selected = self.selected_order.iter().cloned().collect::<BTreeSet<_>>();
        let deferred = self.deferred_order.iter().cloned().collect::<BTreeSet<_>>();
        let blocked = self.blocked_order.iter().cloned().collect::<BTreeSet<_>>();
        let unresolved = self
            .unresolved_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        if selected.len() != self.selected_order.len()
            || deferred.len() != self.deferred_order.len()
            || selected.intersection(&deferred).next().is_some()
            || selected.intersection(&blocked).next().is_some()
            || selected.intersection(&unresolved).next().is_some()
            || deferred.intersection(&blocked).next().is_some()
            || deferred.intersection(&unresolved).next().is_some()
            || blocked.intersection(&unresolved).next().is_some()
            || selected
                .union(&deferred)
                .cloned()
                .chain(blocked.iter().cloned())
                .chain(unresolved.iter().cloned())
                .collect::<BTreeSet<_>>()
                != all
            || self
                .dependency_order
                .windows(2)
                .any(|pair| pair[0] == pair[1])
        {
            return Err(ComputationPortfolioError::InvalidOutput(
                "candidate partitions or dependency order do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ComputationPortfolioError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ComputationPortfolioError::InvalidOutput(
                "computation portfolio digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &ComputationPortfolioRequest,
    candidates: &[ComputationCandidate],
) -> Result<(), ComputationPortfolioError> {
    if request.objective.trim().is_empty()
        || request.budget_units == 0
        || request.duration_ticks == 0
        || request.max_tasks == 0
        || request.max_tasks > MAX_CANDIDATES
        || request.min_modalities > request.max_modalities
        || request.max_modalities == 0
        || request.max_modalities > MAX_MODALITIES
        || candidates.is_empty()
        || candidates.len() > MAX_CANDIDATES
        || request
            .completed_order
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || candidates.iter().any(|candidate| {
            candidate.candidate_id.trim().is_empty()
                || candidate.task.task_id != candidate.candidate_id
                || candidate.task.model_system != request.model_system
                || candidate.task.output_schema.trim().is_empty()
                || candidate.redundancy_group.trim().is_empty()
                || candidate.task.estimated_cost_units == 0
                || candidate.task.estimated_duration_ticks == 0
        })
    {
        return Err(ComputationPortfolioError::InvalidRequest(
            "objective, bounded resources, modality limits, canonical completion, and typed candidates are required".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    if candidates
        .iter()
        .any(|candidate| !ids.insert(candidate.candidate_id.clone()))
    {
        return Err(ComputationPortfolioError::InvalidRequest(
            "candidate ids must be unique".into(),
        ));
    }
    Ok(())
}

fn utility(candidate: &ComputationCandidate, request: &ComputationPortfolioRequest) -> i64 {
    let positive = u128::from(candidate.information_gain_milli)
        .saturating_mul(u128::from(request.information_weight_milli))
        .saturating_add(
            u128::from(candidate.uncertainty_reduction_milli)
                .saturating_mul(u128::from(request.uncertainty_weight_milli)),
        )
        .saturating_add(
            u128::from(candidate.coverage_debt_milli)
                .saturating_mul(u128::from(request.coverage_weight_milli)),
        );
    let penalty = u128::from(candidate.task.estimated_cost_units)
        .saturating_mul(u128::from(request.cost_penalty_milli))
        .saturating_add(
            u128::from(candidate.task.estimated_duration_ticks)
                .saturating_mul(u128::from(request.duration_penalty_milli)),
        );
    positive.saturating_sub(penalty).min(i64::MAX as u128) as i64
}

fn closure(
    id: &str,
    candidates: &BTreeMap<String, &ComputationCandidate>,
    completed: &BTreeSet<String>,
    visiting: &mut BTreeSet<String>,
    ordered: &mut Vec<String>,
) -> Result<(), String> {
    if completed.contains(id) || ordered.iter().any(|item| item == id) {
        return Ok(());
    }
    if !visiting.insert(id.to_string()) {
        return Err(format!("dependency cycle reaches {id}"));
    }
    let candidate = candidates
        .get(id)
        .ok_or_else(|| format!("missing dependency {id}"))?;
    let mut dependencies = candidate.task.depends_on.clone();
    dependencies.sort();
    for dependency in dependencies {
        closure(&dependency, candidates, completed, visiting, ordered)?;
    }
    visiting.remove(id);
    ordered.push(id.to_string());
    Ok(())
}

/// Plan a deterministic resource-bounded computation portfolio and prerequisite order.
pub fn plan_glioma_computation_portfolio(
    request: &ComputationPortfolioRequest,
    candidates: &[ComputationCandidate],
) -> Result<ComputationPortfolioPlan, ComputationPortfolioError> {
    validate_request(request, candidates)?;
    let mut candidate_map = BTreeMap::new();
    for candidate in candidates {
        candidate_map.insert(candidate.candidate_id.clone(), candidate);
    }
    let completed = request
        .completed_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut ordered = candidates.iter().collect::<Vec<_>>();
    ordered.sort_by(|left, right| {
        right
            .required
            .cmp(&left.required)
            .then_with(|| utility(right, request).cmp(&utility(left, request)))
            .then_with(|| left.candidate_id.cmp(&right.candidate_id))
    });
    let mut scores =
        candidates
            .iter()
            .map(|candidate| ComputationCandidateScore {
                candidate_id: candidate.candidate_id.clone(),
                operation: candidate.task.operation,
                utility_milli: utility(candidate, request),
                dependency_count: candidate.task.depends_on.len().min(u16::MAX as usize) as u16,
                modality: candidate.modality,
                disposition: ComputationCandidateDisposition::Unresolved,
                rationale:
                    "candidate utility is compared after prerequisite closure and resource gates"
                        .into(),
            })
            .collect::<Vec<_>>();
    scores.sort_by(|left, right| left.candidate_id.cmp(&right.candidate_id));
    let mut selected = BTreeSet::new();
    let mut dependency_order = Vec::new();
    let mut deferred = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut modalities = BTreeSet::new();
    let mut budget_used = 0_u64;
    let mut duration_used = 0_u64;
    let completed_ids = completed.clone();
    for candidate in ordered {
        if completed.contains(&candidate.candidate_id) {
            let score = scores
                .iter_mut()
                .find(|score| score.candidate_id == candidate.candidate_id)
                .expect("score exists");
            score.disposition = ComputationCandidateDisposition::Blocked;
            score.rationale = "already completed in caller-supplied replay state".into();
            blocked.insert(candidate.candidate_id.clone());
            continue;
        }
        if request.require_deterministic && !candidate.task.deterministic {
            let score = scores
                .iter_mut()
                .find(|score| score.candidate_id == candidate.candidate_id)
                .expect("score exists");
            score.disposition = ComputationCandidateDisposition::Unresolved;
            score.rationale = "non-deterministic task is held by reproducibility policy".into();
            unresolved.insert(candidate.candidate_id.clone());
            uncertainty.insert(format!("{}:non-deterministic", candidate.candidate_id));
            continue;
        }
        let mut closure_order = Vec::new();
        if let Err(reason) = closure(
            &candidate.candidate_id,
            &candidate_map,
            &completed_ids,
            &mut BTreeSet::new(),
            &mut closure_order,
        ) {
            let score = scores
                .iter_mut()
                .find(|score| score.candidate_id == candidate.candidate_id)
                .expect("score exists");
            score.disposition = ComputationCandidateDisposition::Blocked;
            score.rationale = reason.clone();
            blocked.insert(candidate.candidate_id.clone());
            negative.insert(format!("{}:{reason}", candidate.candidate_id));
            continue;
        }
        let new_ids = closure_order
            .iter()
            .filter(|id| !selected.contains(*id) && !completed.contains(*id))
            .cloned()
            .collect::<Vec<_>>();
        if request.require_deterministic
            && closure_order
                .iter()
                .any(|id| !candidate_map[id].task.deterministic)
        {
            let score = scores
                .iter_mut()
                .find(|score| score.candidate_id == candidate.candidate_id)
                .expect("score exists");
            score.disposition = ComputationCandidateDisposition::Unresolved;
            score.rationale =
                "a prerequisite closure contains a non-deterministic task held by reproducibility policy"
                    .into();
            unresolved.insert(candidate.candidate_id.clone());
            uncertainty.insert(format!(
                "{}:non-deterministic-prerequisite",
                candidate.candidate_id
            ));
            continue;
        }
        let new_cost = new_ids
            .iter()
            .map(|id| candidate_map[id].task.estimated_cost_units)
            .sum::<u64>();
        let new_duration = new_ids
            .iter()
            .map(|id| candidate_map[id].task.estimated_duration_ticks)
            .sum::<u64>();
        let new_modalities = new_ids
            .iter()
            .map(|id| candidate_map[id].modality)
            .collect::<BTreeSet<_>>();
        if selected.len() + new_ids.len() > request.max_tasks
            || budget_used.saturating_add(new_cost) > request.budget_units
            || duration_used.saturating_add(new_duration) > request.duration_ticks
            || modalities.len() + new_modalities.difference(&modalities).count()
                > request.max_modalities
        {
            let score = scores
                .iter_mut()
                .find(|score| score.candidate_id == candidate.candidate_id)
                .expect("score exists");
            if candidate.required {
                score.disposition = ComputationCandidateDisposition::Blocked;
                score.rationale =
                    "required candidate cannot fit the declared resource envelope".into();
                blocked.insert(candidate.candidate_id.clone());
                negative.insert(format!("{}:required-resource-cap", candidate.candidate_id));
            } else {
                score.disposition = ComputationCandidateDisposition::Deferred;
                score.rationale = "deferred because prerequisite closure exceeds budget, time, task, or modality capacity".into();
                deferred.insert(candidate.candidate_id.clone());
            }
            continue;
        }
        for id in &new_ids {
            selected.insert(id.clone());
            dependency_order.push(id.clone());
            modalities.insert(candidate_map[id].modality);
        }
        budget_used = budget_used.saturating_add(new_cost);
        duration_used = duration_used.saturating_add(new_duration);
        scores
            .iter_mut()
            .find(|score| score.candidate_id == candidate.candidate_id)
            .expect("score exists")
            .disposition = ComputationCandidateDisposition::Selected;
    }
    let mut selected_ids = selected.iter().cloned().collect::<Vec<_>>();
    selected_ids.retain(|id| candidate_map.contains_key(id));
    for score in &mut scores {
        if score.disposition == ComputationCandidateDisposition::Unresolved
            && !unresolved.contains(&score.candidate_id)
            && !blocked.contains(&score.candidate_id)
            && !deferred.contains(&score.candidate_id)
        {
            deferred.insert(score.candidate_id.clone());
            score.disposition = ComputationCandidateDisposition::Deferred;
        }
    }
    let disposition = if selected_ids.is_empty() && !blocked.is_empty() {
        ComputationPortfolioDisposition::Blocked
    } else if !unresolved.is_empty() || !blocked.is_empty() || !deferred.is_empty() {
        ComputationPortfolioDisposition::Partial
    } else if modalities.len() < request.min_modalities {
        uncertainty.insert("minimum-modality-coverage-not-reached".into());
        ComputationPortfolioDisposition::Unresolved
    } else {
        ComputationPortfolioDisposition::Qualified
    };
    let mut candidate_order = candidates
        .iter()
        .map(|candidate| candidate.candidate_id.clone())
        .collect::<Vec<_>>();
    candidate_order.sort();
    let mut plan = ComputationPortfolioPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        candidate_order,
        scores,
        selected_order: selected_ids,
        dependency_order,
        deferred_order: deferred.into_iter().collect(),
        blocked_order: blocked.into_iter().collect(),
        unresolved_order: unresolved.into_iter().collect(),
        modality_order: modalities.into_iter().collect(),
        budget_used_units: budget_used,
        duration_used_ticks: duration_used,
        negative_evidence: negative.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-computation-portfolio"),
    };
    plan.digest = ContentHash::of_value(&digest_input(&plan))
        .map_err(|error| ComputationPortfolioError::Digest(error.to_string()))?;
    plan.validate()?;
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn task(id: &str, operation: ComputationOperation, depends_on: Vec<&str>) -> ComputationTask {
        ComputationTask {
            task_id: id.into(),
            operation,
            model_system: GliomaModelSystem::Organoid,
            depends_on: depends_on.into_iter().map(str::to_string).collect(),
            input_artifact_ids: vec![format!("input-{id}")],
            output_schema: format!("{id}@1"),
            estimated_cost_units: 2,
            estimated_duration_ticks: 10,
            deterministic: true,
        }
    }

    fn request() -> ComputationPortfolioRequest {
        ComputationPortfolioRequest {
            objective: "choose a reproducible multimodal analysis portfolio".into(),
            model_system: GliomaModelSystem::Organoid,
            budget_units: 8,
            duration_ticks: 40,
            max_tasks: 4,
            max_modalities: 3,
            min_modalities: 2,
            information_weight_milli: 5,
            uncertainty_weight_milli: 3,
            coverage_weight_milli: 2,
            cost_penalty_milli: 1,
            duration_penalty_milli: 1,
            require_deterministic: true,
            completed_order: Vec::new(),
        }
    }

    fn candidate(
        id: &str,
        task: ComputationTask,
        modality: GliomaModality,
        gain: u32,
    ) -> ComputationCandidate {
        ComputationCandidate {
            candidate_id: id.into(),
            task,
            modality,
            information_gain_milli: gain,
            uncertainty_reduction_milli: 500,
            coverage_debt_milli: 300,
            redundancy_group: id.into(),
            required: false,
        }
    }

    #[test]
    fn closes_prerequisites_and_replays_under_input_permutation() {
        let mut candidates = vec![
            candidate(
                "normalize",
                task("normalize", ComputationOperation::Normalize, vec![]),
                GliomaModality::Transcriptomics,
                400,
            ),
            candidate(
                "integrate",
                task(
                    "integrate",
                    ComputationOperation::Integrate,
                    vec!["normalize"],
                ),
                GliomaModality::Spatial,
                900,
            ),
            candidate(
                "segment",
                task("segment", ComputationOperation::Segment, vec![]),
                GliomaModality::Imaging,
                700,
            ),
        ];
        let first = plan_glioma_computation_portfolio(&request(), &candidates).unwrap();
        candidates.reverse();
        let second = plan_glioma_computation_portfolio(&request(), &candidates).unwrap();
        assert_eq!(first, second);
        assert!(
            first
                .dependency_order
                .iter()
                .position(|id| id == "normalize")
                .unwrap()
                < first
                    .dependency_order
                    .iter()
                    .position(|id| id == "integrate")
                    .unwrap()
        );
        first.validate().unwrap();
    }

    #[test]
    fn missing_dependency_is_blocked_and_non_determinism_is_unresolved() {
        let mut nondeterministic = task("stochastic", ComputationOperation::ModelFit, vec![]);
        nondeterministic.deterministic = false;
        let candidates = vec![
            candidate(
                "missing",
                task("missing", ComputationOperation::Validate, vec!["absent"]),
                GliomaModality::Genomics,
                900,
            ),
            candidate(
                "stochastic",
                nondeterministic,
                GliomaModality::Proteomics,
                900,
            ),
        ];
        let plan = plan_glioma_computation_portfolio(&request(), &candidates).unwrap();
        assert!(plan.blocked_order.contains(&"missing".into()));
        assert!(plan.unresolved_order.contains(&"stochastic".into()));
        assert!(!plan.negative_evidence.is_empty());
    }
}
