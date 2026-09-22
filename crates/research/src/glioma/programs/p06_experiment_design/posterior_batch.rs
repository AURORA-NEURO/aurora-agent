//! Batch-conditional selection over externally supplied glioma posterior draws.
//!
//! This is a deterministic PDBAL-inspired surrogate, not a reimplementation of BATCHIE and
//! not a glioma efficacy claim. A calibrated institution-local model supplies posterior draws
//! and predictive likelihoods; this module only chooses complementary assays under constraints.

use super::active_learning::FEATURE_ID;
use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const OUTPUT_SCHEMA: &str = "GliomaPosteriorDisagreementBatch1@1";
pub const MAX_CANDIDATES: usize = 256;
pub const MAX_POSTERIOR_DRAWS: usize = 64;
pub const MAX_TARGETS: usize = 1_024;
pub const MAX_OUTCOME_BINS: usize = 32;
const PROBABILITY_SCALE: u64 = 1_000_000;
const WEIGHT_SCALE: u64 = 1_000;
const VALUE_LIMIT: i64 = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PosteriorBatchTarget {
    pub target_id: String,
    pub weight_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PosteriorBatchCandidate {
    pub candidate_id: String,
    pub mechanism_id: String,
    pub output_schema: String,
    pub cost_units: u32,
    pub risk_milli: u16,
    pub completed_replicates: usize,
    pub max_replicates: usize,
    pub redundancy_group: String,
}

/// One calibrated draw from an institution-local posterior model.
/// Predictions follow canonical target order; predictive outcome-bin probabilities are millionths.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PosteriorPredictiveDraw {
    pub draw_id: String,
    pub prior_weight_millionths: u32,
    pub target_predictions_milli: Vec<i64>,
    pub candidate_outcome_probabilities: BTreeMap<String, Vec<u32>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PosteriorBatchRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub budget_units: u32,
    pub max_selections: usize,
    pub max_risk_milli: u16,
    pub min_marginal_reduction_milli: u32,
    pub targets: Vec<PosteriorBatchTarget>,
    pub posterior_draws: Vec<PosteriorPredictiveDraw>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PosteriorBatchCandidateDisposition {
    Selected,
    Deferred,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PosteriorBatchDisposition {
    Qualified,
    Partial,
    NoCandidates,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PosteriorBatchScore {
    pub candidate_id: String,
    pub standalone_reduction_milli: u32,
    pub marginal_at_selection_milli: Option<u32>,
    pub final_batch_marginal_milli: u32,
    pub disposition: PosteriorBatchCandidateDisposition,
    pub rationale: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PosteriorBatchPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub candidate_order: Vec<String>,
    pub draw_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub deferred_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub scores: Vec<PosteriorBatchScore>,
    pub initial_diameter_milli: u32,
    pub final_diameter_milli: u32,
    pub remaining_budget_units: u32,
    pub disposition: PosteriorBatchDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PosteriorBatchError {
    #[error("posterior batch request is invalid: {0}")]
    InvalidRequest(String),
    #[error("posterior batch candidate is invalid: {0}")]
    InvalidCandidate(String),
    #[error("posterior batch model input is invalid: {0}")]
    InvalidPosterior(String),
    #[error("posterior batch output is invalid: {0}")]
    InvalidOutput(String),
}

fn digest_input(plan: &PosteriorBatchPlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": plan.feature_id, "output_schema": plan.output_schema,
        "objective": plan.objective, "model_system": plan.model_system,
        "candidate_order": plan.candidate_order, "draw_order": plan.draw_order,
        "selected_order": plan.selected_order, "deferred_order": plan.deferred_order,
        "blocked_order": plan.blocked_order, "unresolved_order": plan.unresolved_order,
        "scores": plan.scores, "initial_diameter_milli": plan.initial_diameter_milli,
        "final_diameter_milli": plan.final_diameter_milli,
        "remaining_budget_units": plan.remaining_budget_units, "disposition": plan.disposition,
    })
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl PosteriorBatchPlan {
    pub fn validate(&self) -> Result<(), PosteriorBatchError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !canonical(&self.candidate_order)
            || !canonical(&self.draw_order)
            || !canonical(&self.deferred_order)
            || !canonical(&self.blocked_order)
            || !canonical(&self.unresolved_order)
            || self.scores.len() != self.candidate_order.len()
            || self.final_diameter_milli > self.initial_diameter_milli
            || self
                .scores
                .iter()
                .any(|score| score.rationale.trim().is_empty())
        {
            return Err(PosteriorBatchError::InvalidOutput(
                "identity, order, score count, or monotonicity invariant failed".into(),
            ));
        }
        let expected = self
            .candidate_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut partition = BTreeSet::new();
        for id in self
            .selected_order
            .iter()
            .chain(&self.deferred_order)
            .chain(&self.blocked_order)
            .chain(&self.unresolved_order)
        {
            if !partition.insert(id.clone()) {
                return Err(PosteriorBatchError::InvalidOutput(
                    "candidate disposition partitions overlap".into(),
                ));
            }
        }
        let score_ids = self
            .scores
            .iter()
            .map(|score| score.candidate_id.clone())
            .collect::<BTreeSet<_>>();
        if partition != expected
            || score_ids != expected
            || self.selected_order.iter().any(|id| !expected.contains(id))
        {
            return Err(PosteriorBatchError::InvalidOutput(
                "candidate identities do not reconcile across scores and dispositions".into(),
            ));
        }
        let expected_digest = ContentHash::of_value(&digest_input(self))
            .map_err(|error| PosteriorBatchError::InvalidOutput(error.to_string()))?;
        if expected_digest != self.digest {
            return Err(PosteriorBatchError::InvalidOutput("digest mismatch".into()));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy)]
struct PosteriorPair {
    left: usize,
    right: usize,
    mass: u128,
    target_distance_milli: u64,
}

fn weighted_target_distance(left: &[i64], right: &[i64], targets: &[PosteriorBatchTarget]) -> u64 {
    let weighted = left
        .iter()
        .zip(right)
        .zip(targets)
        .map(|((a, b), target)| a.abs_diff(*b) as u128 * target.weight_milli as u128)
        .sum::<u128>();
    (weighted / WEIGHT_SCALE as u128) as u64
}

fn ceil_sqrt(value: u64) -> u64 {
    if value < 2 {
        return value;
    }
    let mut low = 1u64;
    let mut high = value.min(1_000_001);
    while low < high {
        let mid = low + (high - low) / 2;
        if mid >= value.div_ceil(mid) {
            high = mid;
        } else {
            low = mid + 1;
        }
    }
    low
}

fn overlap_by_pair(
    draws: &[PosteriorPredictiveDraw],
    candidate_id: &str,
    pairs: &[PosteriorPair],
) -> Vec<u64> {
    pairs
        .iter()
        .map(|pair| {
            let left = &draws[pair.left].candidate_outcome_probabilities[candidate_id];
            let right = &draws[pair.right].candidate_outcome_probabilities[candidate_id];
            left.iter()
                .zip(right)
                .map(|(a, b)| ceil_sqrt(*a as u64 * *b as u64))
                .sum::<u64>()
                .min(PROBABILITY_SCALE)
        })
        .collect()
}

fn diameter_milli(pairs: &[PosteriorPair], residual_overlap: &[u64]) -> u32 {
    let total_mass = pairs.iter().map(|pair| pair.mass).sum::<u128>();
    if total_mass == 0 {
        return 0;
    }
    let weighted = pairs
        .iter()
        .zip(residual_overlap)
        .map(|(pair, residual)| pair.mass * pair.target_distance_milli as u128 * *residual as u128)
        .sum::<u128>();
    (weighted / (total_mass * PROBABILITY_SCALE as u128)).min(u32::MAX as u128) as u32
}

fn marginal_reduction_milli(
    pairs: &[PosteriorPair],
    residual_overlap: &[u64],
    candidate_overlap: &[u64],
) -> u32 {
    let total_mass = pairs.iter().map(|pair| pair.mass).sum::<u128>();
    if total_mass == 0 {
        return 0;
    }
    let numerator = pairs
        .iter()
        .zip(residual_overlap)
        .zip(candidate_overlap)
        .map(|((pair, residual), overlap)| {
            pair.mass
                * pair.target_distance_milli as u128
                * *residual as u128
                * (PROBABILITY_SCALE - *overlap) as u128
        })
        .sum::<u128>();
    (numerator / (total_mass * PROBABILITY_SCALE as u128 * PROBABILITY_SCALE as u128))
        .min(u32::MAX as u128) as u32
}

fn apply_overlap(residual: &mut [u64], overlap: &[u64]) {
    for (current, factor) in residual.iter_mut().zip(overlap) {
        *current = (*current * *factor)
            .div_ceil(PROBABILITY_SCALE)
            .min(PROBABILITY_SCALE);
    }
}

/// Select a complementary assay batch from model-supplied posterior draws.
///
/// Greedy selection maximizes conservative posterior-diameter reduction per cost. Pairwise
/// Bhattacharyya overlap is multiplied across already selected assays as a conditional-
/// independence surrogate; it is useful for batch diversification, but has no general optimality
/// guarantee and is not equivalent to BATCHIE's PDBAL objective.
pub fn plan_glioma_posterior_batch(
    request: &PosteriorBatchRequest,
    candidates: &[PosteriorBatchCandidate],
) -> Result<PosteriorBatchPlan, PosteriorBatchError> {
    if request.objective.trim().is_empty()
        || request.budget_units == 0
        || request.max_selections == 0
        || request.max_risk_milli > 1_000
        || request.min_marginal_reduction_milli > 2_000_000
        || candidates.len() > MAX_CANDIDATES
    {
        return Err(PosteriorBatchError::InvalidRequest(
            "objective, positive budget and batch size, and bounded thresholds/candidates are required".into(),
        ));
    }
    if request.targets.is_empty()
        || request.targets.len() > MAX_TARGETS
        || request.posterior_draws.len() < 2
        || request.posterior_draws.len() > MAX_POSTERIOR_DRAWS
    {
        return Err(PosteriorBatchError::InvalidPosterior(
            "at least one target and two bounded posterior draws are required".into(),
        ));
    }

    let mut target_positions = request
        .targets
        .iter()
        .enumerate()
        .map(|(index, target)| (target.target_id.clone(), index))
        .collect::<Vec<_>>();
    target_positions.sort_by(|a, b| a.0.cmp(&b.0));
    let targets = target_positions
        .iter()
        .map(|(target_id, original_index)| {
            let target = &request.targets[*original_index];
            if target.target_id.trim().is_empty() {
                return Err(PosteriorBatchError::InvalidPosterior(
                    "target id is empty".into(),
                ));
            }
            Ok(PosteriorBatchTarget {
                target_id: target_id.clone(),
                weight_milli: target.weight_milli,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if targets
        .windows(2)
        .any(|pair| pair[0].target_id == pair[1].target_id)
        || targets
            .iter()
            .map(|target| target.weight_milli as u64)
            .sum::<u64>()
            != WEIGHT_SCALE
        || targets.iter().any(|target| target.weight_milli == 0)
    {
        return Err(PosteriorBatchError::InvalidPosterior(
            "target ids must be unique with positive weights totaling 1,000".into(),
        ));
    }

    let mut candidates = candidates.to_vec();
    candidates.sort_by(|a, b| a.candidate_id.cmp(&b.candidate_id));
    if candidates
        .windows(2)
        .any(|pair| pair[0].candidate_id == pair[1].candidate_id)
    {
        return Err(PosteriorBatchError::InvalidCandidate(
            "candidate ids must be unique".into(),
        ));
    }
    for candidate in &candidates {
        if candidate.candidate_id.trim().is_empty()
            || candidate.mechanism_id.trim().is_empty()
            || candidate.output_schema.trim().is_empty()
            || candidate.redundancy_group.trim().is_empty()
            || candidate.cost_units == 0
            || candidate.risk_milli > 1_000
            || candidate.max_replicates == 0
            || candidate.completed_replicates > candidate.max_replicates
        {
            return Err(PosteriorBatchError::InvalidCandidate(format!(
                "{} has invalid identity, cost, risk, or replicate bounds",
                candidate.candidate_id
            )));
        }
    }

    let candidate_ids = candidates
        .iter()
        .map(|candidate| candidate.candidate_id.clone())
        .collect::<BTreeSet<_>>();
    let mut draws = request.posterior_draws.clone();
    draws.sort_by(|a, b| a.draw_id.cmp(&b.draw_id));
    if draws
        .windows(2)
        .any(|pair| pair[0].draw_id == pair[1].draw_id)
        || draws
            .iter()
            .any(|draw| draw.draw_id.trim().is_empty() || draw.prior_weight_millionths == 0)
        || draws
            .iter()
            .map(|draw| draw.prior_weight_millionths as u64)
            .sum::<u64>()
            != PROBABILITY_SCALE
    {
        return Err(PosteriorBatchError::InvalidPosterior(
            "draw ids must be unique and positive prior weights must total 1,000,000".into(),
        ));
    }
    let original_candidate_keys = candidate_ids;
    for draw in &mut draws {
        if draw.target_predictions_milli.len() != targets.len()
            || draw
                .target_predictions_milli
                .iter()
                .any(|value| value.unsigned_abs() > VALUE_LIMIT as u64)
            || draw
                .candidate_outcome_probabilities
                .keys()
                .cloned()
                .collect::<BTreeSet<_>>()
                != original_candidate_keys
        {
            return Err(PosteriorBatchError::InvalidPosterior(format!(
                "draw {} has a missing/extra assay likelihood or invalid target vector",
                draw.draw_id
            )));
        }
        let old_predictions = draw.target_predictions_milli.clone();
        draw.target_predictions_milli = target_positions
            .iter()
            .map(|(_, old_index)| old_predictions[*old_index])
            .collect();
    }
    let mut bins_by_candidate = BTreeMap::<String, usize>::new();
    for draw in &draws {
        for candidate in &candidates {
            let probabilities = &draw.candidate_outcome_probabilities[&candidate.candidate_id];
            if !(2..=MAX_OUTCOME_BINS).contains(&probabilities.len())
                || probabilities.iter().map(|value| *value as u64).sum::<u64>() != PROBABILITY_SCALE
            {
                return Err(PosteriorBatchError::InvalidPosterior(format!("{} in draw {} must provide 2..={MAX_OUTCOME_BINS} probabilities totaling 1,000,000", candidate.candidate_id, draw.draw_id)));
            }
            if let Some(previous) = bins_by_candidate.get(&candidate.candidate_id) {
                if *previous != probabilities.len() {
                    return Err(PosteriorBatchError::InvalidPosterior(format!(
                        "{} changes predictive bin count across posterior draws",
                        candidate.candidate_id
                    )));
                }
            } else {
                bins_by_candidate.insert(candidate.candidate_id.clone(), probabilities.len());
            }
        }
    }

    let mut pairs = Vec::new();
    for left in 0..draws.len() {
        for right in (left + 1)..draws.len() {
            pairs.push(PosteriorPair {
                left,
                right,
                mass: draws[left].prior_weight_millionths as u128
                    * draws[right].prior_weight_millionths as u128,
                target_distance_milli: weighted_target_distance(
                    &draws[left].target_predictions_milli,
                    &draws[right].target_predictions_milli,
                    &targets,
                ),
            });
        }
    }
    let overlaps = candidates
        .iter()
        .map(|candidate| {
            (
                candidate.candidate_id.clone(),
                overlap_by_pair(&draws, &candidate.candidate_id, &pairs),
            )
        })
        .collect::<BTreeMap<_, _>>();
    let initial_residual = vec![PROBABILITY_SCALE; pairs.len()];
    let initial_diameter_milli = diameter_milli(&pairs, &initial_residual);
    let standalone = candidates
        .iter()
        .map(|candidate| {
            (
                candidate.candidate_id.clone(),
                marginal_reduction_milli(
                    &pairs,
                    &initial_residual,
                    &overlaps[&candidate.candidate_id],
                ),
            )
        })
        .collect::<BTreeMap<_, _>>();

    let mut blocked = BTreeSet::new();
    let mut selected = Vec::<String>::new();
    let mut selected_set = BTreeSet::new();
    let mut selected_groups = BTreeSet::new();
    let mut marginal_at_selection = BTreeMap::<String, u32>::new();
    let mut residual = initial_residual;
    let mut remaining_budget = request.budget_units;
    for candidate in &candidates {
        if candidate.risk_milli > request.max_risk_milli
            || candidate.completed_replicates >= candidate.max_replicates
        {
            blocked.insert(candidate.candidate_id.clone());
        }
    }

    while selected.len() < request.max_selections {
        let mut best: Option<(&PosteriorBatchCandidate, u32)> = None;
        for candidate in &candidates {
            if blocked.contains(&candidate.candidate_id)
                || selected_set.contains(&candidate.candidate_id)
                || selected_groups.contains(&candidate.redundancy_group)
                || candidate.cost_units > remaining_budget
            {
                continue;
            }
            let marginal =
                marginal_reduction_milli(&pairs, &residual, &overlaps[&candidate.candidate_id]);
            let replace = match best {
                None => true,
                Some((previous, previous_marginal)) => {
                    let left = marginal as u128 * previous.cost_units as u128;
                    let right = previous_marginal as u128 * candidate.cost_units as u128;
                    left > right
                        || (left == right && candidate.candidate_id < previous.candidate_id)
                }
            };
            if replace {
                best = Some((candidate, marginal));
            }
        }
        let Some((candidate, marginal)) = best else {
            break;
        };
        if marginal == 0 || marginal < request.min_marginal_reduction_milli {
            break;
        }
        selected.push(candidate.candidate_id.clone());
        selected_set.insert(candidate.candidate_id.clone());
        selected_groups.insert(candidate.redundancy_group.clone());
        marginal_at_selection.insert(candidate.candidate_id.clone(), marginal);
        remaining_budget -= candidate.cost_units;
        apply_overlap(&mut residual, &overlaps[&candidate.candidate_id]);
    }

    let final_diameter_milli = diameter_milli(&pairs, &residual);
    let selected_order = selected.clone();
    let mut deferred_order = Vec::new();
    let mut blocked_order = blocked.iter().cloned().collect::<Vec<_>>();
    let mut unresolved_order = Vec::new();
    let mut scores = Vec::with_capacity(candidates.len());
    for candidate in &candidates {
        let id = &candidate.candidate_id;
        let (disposition, rationale) = if selected_set.contains(id) {
            (PosteriorBatchCandidateDisposition::Selected, "chosen for the largest cost-adjusted conditional reduction in posterior disagreement".to_owned())
        } else if blocked.contains(id) {
            (
                PosteriorBatchCandidateDisposition::Blocked,
                if candidate.risk_milli > request.max_risk_milli {
                    "risk exceeds the declared preclinical batch limit".to_owned()
                } else {
                    "replicate ceiling is already reached".to_owned()
                },
            )
        } else if standalone[id] == 0 {
            unresolved_order.push(id.clone());
            (
                PosteriorBatchCandidateDisposition::Unresolved,
                "the supplied posterior draws show no target disagreement this assay can resolve"
                    .to_owned(),
            )
        } else {
            deferred_order.push(id.clone());
            (PosteriorBatchCandidateDisposition::Deferred, "standalone information exists, but budget, redundancy, batch capacity, or conditional value prevented selection".to_owned())
        };
        scores.push(PosteriorBatchScore {
            candidate_id: id.clone(),
            standalone_reduction_milli: standalone[id],
            marginal_at_selection_milli: marginal_at_selection.get(id).copied(),
            final_batch_marginal_milli: marginal_reduction_milli(&pairs, &residual, &overlaps[id]),
            disposition,
            rationale,
        });
    }
    blocked_order.sort();
    deferred_order.sort();
    unresolved_order.sort();

    let disposition = if candidates.is_empty() {
        PosteriorBatchDisposition::NoCandidates
    } else if selected.len() == candidates.len() {
        PosteriorBatchDisposition::Qualified
    } else if !selected.is_empty() {
        PosteriorBatchDisposition::Partial
    } else if blocked.len() == candidates.len() {
        PosteriorBatchDisposition::Blocked
    } else if unresolved_order.len() == candidates.len() {
        PosteriorBatchDisposition::Unresolved
    } else {
        PosteriorBatchDisposition::Partial
    };
    let mut plan = PosteriorBatchPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        candidate_order: candidates
            .iter()
            .map(|candidate| candidate.candidate_id.clone())
            .collect(),
        draw_order: draws.iter().map(|draw| draw.draw_id.clone()).collect(),
        selected_order,
        deferred_order,
        blocked_order,
        unresolved_order,
        scores,
        initial_diameter_milli,
        final_diameter_milli,
        remaining_budget_units: remaining_budget,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-posterior-batch"),
    };
    plan.digest = ContentHash::of_value(&digest_input(&plan))
        .map_err(|error| PosteriorBatchError::InvalidOutput(error.to_string()))?;
    plan.validate()?;
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fixture() -> (PosteriorBatchRequest, Vec<PosteriorBatchCandidate>) {
        let candidates = ["a-primary", "a-repeat", "b-orthogonal"]
            .into_iter()
            .map(|id| PosteriorBatchCandidate {
                candidate_id: id.into(),
                mechanism_id: format!("mechanism-{id}"),
                output_schema: "GliomaOrganoidAssay1@1".into(),
                cost_units: 1,
                risk_milli: 100,
                completed_replicates: 0,
                max_replicates: 3,
                redundancy_group: format!("distinct-design-{id}"),
            })
            .collect::<Vec<_>>();
        let states = [
            (0, 0, "draw-00"),
            (0, 1, "draw-01"),
            (1, 0, "draw-10"),
            (1, 1, "draw-11"),
        ];
        let posterior_draws = states
            .into_iter()
            .map(|(axis_a, axis_b, draw_id)| {
                let likelihood = |axis: i64| {
                    if axis == 0 {
                        vec![1_000_000, 0]
                    } else {
                        vec![0, 1_000_000]
                    }
                };
                PosteriorPredictiveDraw {
                    draw_id: draw_id.into(),
                    prior_weight_millionths: 250_000,
                    target_predictions_milli: vec![axis_a * 1_000, axis_b * 1_000],
                    candidate_outcome_probabilities: BTreeMap::from([
                        ("a-primary".into(), likelihood(axis_a)),
                        ("a-repeat".into(), likelihood(axis_a)),
                        ("b-orthogonal".into(), likelihood(axis_b)),
                    ]),
                }
            })
            .collect();
        let request = PosteriorBatchRequest {
            objective: "resolve organoid invasion and stemness disagreement".into(),
            model_system: GliomaModelSystem::Organoid,
            budget_units: 2,
            max_selections: 2,
            max_risk_milli: 500,
            min_marginal_reduction_milli: 1,
            targets: vec![
                PosteriorBatchTarget {
                    target_id: "invasion".into(),
                    weight_milli: 500,
                },
                PosteriorBatchTarget {
                    target_id: "stemness".into(),
                    weight_milli: 500,
                },
            ],
            posterior_draws,
        };
        (request, candidates)
    }

    fn pairs_for(
        request: &PosteriorBatchRequest,
    ) -> (
        Vec<PosteriorPredictiveDraw>,
        Vec<PosteriorBatchTarget>,
        Vec<PosteriorPair>,
    ) {
        let mut draws = request.posterior_draws.clone();
        draws.sort_by(|a, b| a.draw_id.cmp(&b.draw_id));
        let mut targets = request.targets.clone();
        targets.sort_by(|a, b| a.target_id.cmp(&b.target_id));
        let mut pairs = Vec::new();
        for left in 0..draws.len() {
            for right in (left + 1)..draws.len() {
                pairs.push(PosteriorPair {
                    left,
                    right,
                    mass: draws[left].prior_weight_millionths as u128
                        * draws[right].prior_weight_millionths as u128,
                    target_distance_milli: weighted_target_distance(
                        &draws[left].target_predictions_milli,
                        &draws[right].target_predictions_milli,
                        &targets,
                    ),
                });
            }
        }
        (draws, targets, pairs)
    }

    #[test]
    fn conditional_batch_selects_orthogonal_assay_where_static_top_k_repeats_axis_a() {
        let (request, candidates) = fixture();
        let plan =
            plan_glioma_posterior_batch(&request, &candidates).expect("posterior batch plan");
        let (draws, _, pairs) = pairs_for(&request);
        let mut static_rank = plan.scores.iter().collect::<Vec<_>>();
        static_rank.sort_by(|a, b| {
            b.standalone_reduction_milli
                .cmp(&a.standalone_reduction_milli)
                .then_with(|| a.candidate_id.cmp(&b.candidate_id))
        });
        let static_top_two = static_rank
            .iter()
            .take(2)
            .map(|score| score.candidate_id.clone())
            .collect::<Vec<_>>();
        let mut static_residual = vec![PROBABILITY_SCALE; pairs.len()];
        for id in &static_top_two {
            apply_overlap(&mut static_residual, &overlap_by_pair(&draws, id, &pairs));
        }

        assert_eq!(static_top_two, vec!["a-primary", "a-repeat"]);
        assert_eq!(plan.selected_order, vec!["a-primary", "b-orthogonal"]);
        assert_eq!(plan.initial_diameter_milli, 666);
        assert_eq!(plan.final_diameter_milli, 0);
        assert!(diameter_milli(&pairs, &static_residual) > plan.final_diameter_milli);
        plan.validate().expect("sealed plan validates");
    }

    #[test]
    fn candidate_order_does_not_change_plan_or_digest() {
        let (request, mut candidates) = fixture();
        let expected = plan_glioma_posterior_batch(&request, &candidates).expect("initial plan");
        candidates.reverse();
        let actual = plan_glioma_posterior_batch(&request, &candidates).expect("permuted plan");
        assert_eq!(actual, expected);
    }

    #[test]
    fn target_order_does_not_change_plan_or_digest() {
        let (mut request, candidates) = fixture();
        let expected = plan_glioma_posterior_batch(&request, &candidates).expect("initial plan");
        request.targets.reverse();
        for draw in &mut request.posterior_draws {
            draw.target_predictions_milli.reverse();
        }
        let actual = plan_glioma_posterior_batch(&request, &candidates).expect("permuted plan");
        assert_eq!(actual, expected);
    }

    #[test]
    fn incomplete_predictive_mass_is_rejected_not_imputed() {
        let (mut request, candidates) = fixture();
        request.posterior_draws[0]
            .candidate_outcome_probabilities
            .remove("b-orthogonal");
        assert!(matches!(
            plan_glioma_posterior_batch(&request, &candidates),
            Err(PosteriorBatchError::InvalidPosterior(_))
        ));
    }

    #[test]
    fn risk_and_replicate_ceiling_block_without_being_scored_as_success() {
        let (request, mut candidates) = fixture();
        candidates[0].risk_milli = 900;
        candidates[1].completed_replicates = candidates[1].max_replicates;
        let plan =
            plan_glioma_posterior_batch(&request, &candidates).expect("bounded partial plan");
        assert_eq!(plan.blocked_order, vec!["a-primary", "a-repeat"]);
        assert!(!plan.selected_order.contains(&"a-primary".to_owned()));
        assert_eq!(plan.disposition, PosteriorBatchDisposition::Partial);
    }

    #[test]
    fn no_target_disagreement_is_explicitly_unresolved() {
        let (mut request, candidates) = fixture();
        for draw in &mut request.posterior_draws {
            draw.target_predictions_milli = vec![100, 200];
        }
        let plan = plan_glioma_posterior_batch(&request, &candidates).expect("unresolved plan");
        assert_eq!(plan.initial_diameter_milli, 0);
        assert_eq!(plan.selected_order, Vec::<String>::new());
        assert_eq!(plan.unresolved_order.len(), 3);
        assert_eq!(plan.disposition, PosteriorBatchDisposition::Unresolved);
    }

    #[test]
    fn integer_square_root_is_conservative() {
        for value in 0..10_000 {
            let root = ceil_sqrt(value);
            assert!(root * root >= value);
            assert!(root == 0 || (root - 1) * (root - 1) < value);
        }
    }
}
