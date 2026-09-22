//! Uncertainty-aware adaptive dose-surface planning for preclinical glioma assays.
//!
//! The existing dose-response and Bliss features analyze declared observations.  This feature
//! closes the next experimental loop: it estimates an unmeasured combination cell from nearby
//! local response-surface evidence, carries replicate debt and residual uncertainty forward, and
//! selects a diverse next batch with an upper-confidence acquisition score.  It is a planning
//! capability for institution-local assays, never a patient dose recommendation or a claim about
//! treatment efficacy.

use super::synergy::DosePair;
use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P06-F31";
pub const OUTPUT_SCHEMA: &str = "GliomaAdaptiveDoseSurface1@1";
pub const MAX_OBSERVATIONS: usize = 32_768;
pub const MAX_CANDIDATES: usize = 4_096;
pub const MAX_NEIGHBORS: usize = 4;
pub const MAX_DOSE_MILLI: u32 = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoseSurfaceObservation {
    pub observation_id: String,
    pub unit_id: String,
    pub batch_id: String,
    pub model_system: GliomaModelSystem,
    pub dose_a_milli: u32,
    pub dose_b_milli: u32,
    pub response_milli: u16,
    pub uncertainty_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveDoseSurfaceRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub observations: Vec<DoseSurfaceObservation>,
    pub candidate_pairs: Vec<DosePair>,
    pub min_replicates_per_cell: usize,
    pub budget_units: u32,
    pub cost_per_pair_units: u32,
    pub max_next_pairs: usize,
    pub target_response_milli: u16,
    pub exploration_weight_milli: u16,
    pub max_estimate_uncertainty_milli: u16,
    pub max_total_dose_milli: u32,
    pub min_pair_separation_milli: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DoseSurfaceCellState {
    Observed,
    UnderReplicated,
    Interpolated,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoseSurfaceEstimate {
    pub dose_pair: DosePair,
    pub state: DoseSurfaceCellState,
    pub observed_replicates: usize,
    pub predicted_response_milli: u16,
    pub uncertainty_milli: u16,
    pub upper_confidence_milli: u16,
    pub acquisition_score_milli: u64,
    pub neighbor_order: Vec<DosePair>,
    pub rationale_order: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdaptiveDoseSurfaceDisposition {
    ReadyForValidation,
    NeedsObservations,
    NoAdmissiblePairs,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdaptiveDoseSurfacePlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub observed_order: Vec<DosePair>,
    pub candidate_order: Vec<DosePair>,
    pub selected_order: Vec<DosePair>,
    pub estimates: Vec<DoseSurfaceEstimate>,
    pub target_response_milli: u16,
    pub selected_cost_units: u32,
    pub budget_units: u32,
    pub remaining_budget_units: u32,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: AdaptiveDoseSurfaceDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AdaptiveDoseSurfaceError {
    #[error("adaptive dose-surface request is invalid: {0}")]
    InvalidRequest(String),
    #[error("adaptive dose-surface observation is invalid: {0}")]
    InvalidObservation(String),
    #[error("adaptive dose-surface output is invalid: {0}")]
    InvalidOutput(String),
    #[error("adaptive dose-surface digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn mean(values: &[&DoseSurfaceObservation]) -> u16 {
    if values.is_empty() {
        0
    } else {
        (values
            .iter()
            .map(|observation| u64::from(observation.response_milli))
            .sum::<u64>()
            / values.len() as u64) as u16
    }
}

fn residual_mad(values: &[&DoseSurfaceObservation], center: u16) -> u16 {
    let mut residuals = values
        .iter()
        .map(|observation| {
            (i32::from(observation.response_milli) - i32::from(center)).unsigned_abs() as u16
        })
        .collect::<Vec<_>>();
    residuals.sort_unstable();
    residuals.get(residuals.len() / 2).copied().unwrap_or(0)
}

fn distance(left: DosePair, right: DosePair) -> u32 {
    left.dose_a_milli
        .abs_diff(right.dose_a_milli)
        .saturating_add(left.dose_b_milli.abs_diff(right.dose_b_milli))
}

fn digest_input(output: &AdaptiveDoseSurfacePlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "model_system": output.model_system,
        "observed_order": output.observed_order,
        "candidate_order": output.candidate_order,
        "selected_order": output.selected_order,
        "estimates": output.estimates,
        "target_response_milli": output.target_response_milli,
        "selected_cost_units": output.selected_cost_units,
        "budget_units": output.budget_units,
        "remaining_budget_units": output.remaining_budget_units,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn validate_request(request: &AdaptiveDoseSurfaceRequest) -> Result<(), AdaptiveDoseSurfaceError> {
    if request.objective.trim().is_empty()
        || request.observations.len() > MAX_OBSERVATIONS
        || request.candidate_pairs.is_empty()
        || request.candidate_pairs.len() > MAX_CANDIDATES
        || request.min_replicates_per_cell == 0
        || request.budget_units == 0
        || request.cost_per_pair_units == 0
        || request.max_next_pairs == 0
        || request.max_next_pairs > request.candidate_pairs.len()
        || request.target_response_milli > 1_000
        || request.exploration_weight_milli > 1_000
        || request.max_estimate_uncertainty_milli == 0
        || request.max_estimate_uncertainty_milli > 1_000
        || request.max_total_dose_milli == 0
        || request.min_pair_separation_milli > request.max_total_dose_milli
    {
        return Err(AdaptiveDoseSurfaceError::InvalidRequest(
            "objective, bounded observations/candidates, positive budget/cost/floors, and dose caps are required".into(),
        ));
    }
    let mut pairs = BTreeSet::new();
    for pair in &request.candidate_pairs {
        if pair.dose_a_milli == 0
            || pair.dose_b_milli == 0
            || pair.dose_a_milli > MAX_DOSE_MILLI
            || pair.dose_b_milli > MAX_DOSE_MILLI
            || pair.dose_a_milli.saturating_add(pair.dose_b_milli) > request.max_total_dose_milli
            || !pairs.insert(*pair)
        {
            return Err(AdaptiveDoseSurfaceError::InvalidRequest(
                "candidate dose pairs must be positive, unique, bounded, and below the total-dose cap".into(),
            ));
        }
    }
    let mut observation_ids = BTreeSet::new();
    for observation in &request.observations {
        if observation.observation_id.trim().is_empty()
            || observation.unit_id.trim().is_empty()
            || observation.batch_id.trim().is_empty()
            || observation.model_system != request.model_system
            || observation.dose_a_milli > MAX_DOSE_MILLI
            || observation.dose_b_milli > MAX_DOSE_MILLI
            || observation.response_milli > 1_000
            || observation.uncertainty_milli == 0
            || observation.uncertainty_milli > 1_000
            || !observation_ids.insert(observation.observation_id.clone())
        {
            return Err(AdaptiveDoseSurfaceError::InvalidObservation(
                "observation identity, model binding, dose, response, uncertainty, or uniqueness is invalid".into(),
            ));
        }
    }
    Ok(())
}

fn estimate_pair(
    pair: DosePair,
    groups: &BTreeMap<DosePair, Vec<&DoseSurfaceObservation>>,
    request: &AdaptiveDoseSurfaceRequest,
) -> DoseSurfaceEstimate {
    let direct = groups.get(&pair).cloned().unwrap_or_default();
    let observed_replicates = direct.len();
    let (predicted_response_milli, uncertainty_milli, state, neighbor_order, mut rationale) =
        if observed_replicates >= request.min_replicates_per_cell {
            let predicted = mean(&direct);
            let uncertainty = direct
                .iter()
                .map(|observation| u16::max(observation.uncertainty_milli, 1))
                .max()
                .unwrap_or(1)
                .max(residual_mad(&direct, predicted));
            (
                predicted,
                uncertainty.min(1_000),
                DoseSurfaceCellState::Observed,
                vec![pair],
                vec!["replicate-floor-met; no new cell is required".into()],
            )
        } else {
            let mut neighbors = groups
                .iter()
                .map(|(neighbor_pair, values)| {
                    (*neighbor_pair, values, distance(pair, *neighbor_pair))
                })
                .filter(|(_, _, neighbor_distance)| *neighbor_distance > 0)
                .collect::<Vec<_>>();
            neighbors
                .sort_by(|left, right| left.2.cmp(&right.2).then_with(|| left.0.cmp(&right.0)));
            neighbors.truncate(MAX_NEIGHBORS);
            let neighbor_order = neighbors
                .iter()
                .map(|(neighbor_pair, _, _)| *neighbor_pair)
                .collect::<Vec<_>>();
            let mut weighted_response = 0_u128;
            let mut weighted_uncertainty = 0_u128;
            let mut weight_total = 0_u128;
            let mut residual = 0_u16;
            for (_, values, neighbor_distance) in &neighbors {
                let weight = 1_000_000_u128 / (u128::from(*neighbor_distance) + 1);
                let center = mean(values);
                weighted_response =
                    weighted_response.saturating_add(weight.saturating_mul(u128::from(center)));
                weighted_uncertainty = weighted_uncertainty.saturating_add(
                    weight.saturating_mul(u128::from(
                        values
                            .iter()
                            .map(|observation| observation.uncertainty_milli)
                            .max()
                            .unwrap_or(1),
                    )),
                );
                residual = residual.max(residual_mad(values, center));
                weight_total = weight_total.saturating_add(weight);
            }
            let predicted = if weight_total == 0 {
                0
            } else {
                weighted_response
                    .checked_div(weight_total)
                    .unwrap_or(0)
                    .min(1_000) as u16
            };
            let distance_penalty = neighbors
                .last()
                .map(|(_, _, neighbor_distance)| (*neighbor_distance / 1_000).min(1_000) as u16)
                .unwrap_or(1_000);
            let weighted_noise = if weight_total == 0 {
                1_000
            } else {
                weighted_uncertainty
                    .checked_div(weight_total)
                    .unwrap_or(0)
                    .min(1_000) as u16
            };
            let uncertainty = weighted_noise
                .max(residual)
                .saturating_add(distance_penalty)
                .min(1_000);
            let state = if neighbors.is_empty() {
                DoseSurfaceCellState::Unresolved
            } else if observed_replicates > 0 {
                DoseSurfaceCellState::UnderReplicated
            } else {
                DoseSurfaceCellState::Interpolated
            };
            let mut rationale = Vec::new();
            if observed_replicates > 0 {
                rationale.push("replicate-floor-debt-remains".into());
            }
            if neighbors.is_empty() {
                rationale.push("no-neighboring-response-cell".into());
            } else if neighbors.len() < 2 {
                rationale.push("sparse-neighborhood".into());
            } else {
                rationale.push("inverse-distance-response-surface-interpolation".into());
            }
            (predicted, uncertainty, state, neighbor_order, rationale)
        };
    let upper_confidence_milli = predicted_response_milli
        .saturating_add(
            ((u32::from(uncertainty_milli) * u32::from(request.exploration_weight_milli)) / 1_000)
                .min(1_000) as u16,
        )
        .min(1_000);
    let acquisition_score_milli = if matches!(state, DoseSurfaceCellState::Observed) {
        0
    } else {
        let target_margin =
            u64::from(upper_confidence_milli.saturating_sub(request.target_response_milli));
        target_margin
            .saturating_mul(1_000)
            .saturating_add(
                u64::from(uncertainty_milli) * u64::from(request.exploration_weight_milli),
            )
            .saturating_add(u64::from(upper_confidence_milli))
    };
    if uncertainty_milli > request.max_estimate_uncertainty_milli {
        rationale.push("estimate-exceeds-uncertainty-ceiling".into());
    }
    rationale.sort();
    rationale.dedup();
    DoseSurfaceEstimate {
        dose_pair: pair,
        state,
        observed_replicates,
        predicted_response_milli,
        uncertainty_milli,
        upper_confidence_milli,
        acquisition_score_milli,
        neighbor_order,
        rationale_order: rationale,
    }
}

impl AdaptiveDoseSurfacePlan {
    pub fn validate(&self) -> Result<(), AdaptiveDoseSurfaceError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.target_response_milli > 1_000
            || !canonical(&self.observed_order)
            || !canonical(&self.candidate_order)
            || !canonical(&self.selected_order)
            || self.estimates.len() != self.candidate_order.len()
            || self
                .estimates
                .iter()
                .map(|estimate| estimate.dose_pair)
                .collect::<Vec<_>>()
                != self.candidate_order
            || self.selected_cost_units > self.budget_units
            || self.remaining_budget_units != self.budget_units - self.selected_cost_units
            || self.estimates.iter().any(|estimate| {
                estimate.predicted_response_milli > 1_000
                    || estimate.uncertainty_milli > 1_000
                    || estimate.upper_confidence_milli > 1_000
                    || estimate.rationale_order.is_empty()
                    || estimate.neighbor_order.len()
                        != estimate
                            .neighbor_order
                            .iter()
                            .collect::<BTreeSet<_>>()
                            .len()
                    || estimate
                        .rationale_order
                        .iter()
                        .any(|reason| reason.trim().is_empty())
            })
        {
            return Err(AdaptiveDoseSurfaceError::InvalidOutput(
                "identity, ordering, estimate, budget, or rationale bounds are invalid".into(),
            ));
        }
        let candidates = self.candidate_order.iter().collect::<BTreeSet<_>>();
        let selected = self.selected_order.iter().collect::<BTreeSet<_>>();
        if selected.iter().any(|pair| !candidates.contains(pair))
            || selected.len() != self.selected_order.len()
            || self
                .estimates
                .iter()
                .filter(|estimate| selected.contains(&estimate.dose_pair))
                .any(|estimate| matches!(estimate.state, DoseSurfaceCellState::Observed))
        {
            return Err(AdaptiveDoseSurfaceError::InvalidOutput(
                "selected cells must be unique candidate cells that still need validation".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| AdaptiveDoseSurfaceError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(AdaptiveDoseSurfaceError::InvalidOutput(
                "adaptive dose-surface digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Plan a diverse next batch of dose-combination cells from local preclinical observations.
pub fn plan_adaptive_glioma_dose_surface(
    request: &AdaptiveDoseSurfaceRequest,
) -> Result<AdaptiveDoseSurfacePlan, AdaptiveDoseSurfaceError> {
    validate_request(request)?;
    let mut groups = BTreeMap::<DosePair, Vec<&DoseSurfaceObservation>>::new();
    for observation in &request.observations {
        groups
            .entry(DosePair {
                dose_a_milli: observation.dose_a_milli,
                dose_b_milli: observation.dose_b_milli,
            })
            .or_default()
            .push(observation);
    }
    let observed_order = groups.keys().copied().collect::<Vec<_>>();
    let candidate_order = request
        .candidate_pairs
        .iter()
        .copied()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let estimates = candidate_order
        .iter()
        .copied()
        .map(|pair| estimate_pair(pair, &groups, request))
        .collect::<Vec<_>>();
    let mut ranked = estimates
        .iter()
        .filter(|estimate| {
            !matches!(estimate.state, DoseSurfaceCellState::Observed)
                && estimate.uncertainty_milli <= request.max_estimate_uncertainty_milli
        })
        .cloned()
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        right
            .acquisition_score_milli
            .cmp(&left.acquisition_score_milli)
            .then_with(|| left.dose_pair.cmp(&right.dose_pair))
    });
    let max_by_budget = (request.budget_units / request.cost_per_pair_units) as usize;
    let selection_cap = request.max_next_pairs.min(max_by_budget);
    let mut selected_order = Vec::new();
    for estimate in ranked {
        if selected_order.len() >= selection_cap {
            break;
        }
        if selected_order.iter().any(|selected| {
            distance(*selected, estimate.dose_pair) < request.min_pair_separation_milli
        }) {
            continue;
        }
        selected_order.push(estimate.dose_pair);
    }
    selected_order.sort_unstable();
    let selected_cost_units =
        (selected_order.len() as u32).saturating_mul(request.cost_per_pair_units);
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    if !groups.contains_key(&DosePair {
        dose_a_milli: 0,
        dose_b_milli: 0,
    }) {
        uncertainty.insert("vehicle-control-not-observed-in-surface-input".into());
    }
    if estimates.iter().any(|estimate| {
        matches!(estimate.state, DoseSurfaceCellState::Unresolved)
            && estimate.uncertainty_milli > request.max_estimate_uncertainty_milli
    }) {
        uncertainty.insert("one-or-more-candidate-cells-have-no-safe-neighborhood".into());
    }
    if selected_order.is_empty() {
        if max_by_budget == 0 {
            negative_evidence.insert("budget-cannot-fund-one-dose-surface-cell".into());
        } else if estimates
            .iter()
            .all(|estimate| matches!(estimate.state, DoseSurfaceCellState::Observed))
        {
            negative_evidence
                .insert("all-candidate-cells-have-replicate-complete-observations".into());
        } else {
            uncertainty.insert("no-candidate-cell-clears-selection-gates".into());
        }
    }
    let disposition = if !selected_order.is_empty() {
        AdaptiveDoseSurfaceDisposition::ReadyForValidation
    } else if estimates
        .iter()
        .any(|estimate| !matches!(estimate.state, DoseSurfaceCellState::Observed))
    {
        AdaptiveDoseSurfaceDisposition::NeedsObservations
    } else {
        AdaptiveDoseSurfaceDisposition::NoAdmissiblePairs
    };
    let mut output = AdaptiveDoseSurfacePlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        observed_order,
        candidate_order,
        selected_order,
        estimates,
        target_response_milli: request.target_response_milli,
        selected_cost_units,
        budget_units: request.budget_units,
        remaining_budget_units: request.budget_units.saturating_sub(selected_cost_units),
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-adaptive-dose-surface"),
    };
    output.estimates.sort_by_key(|estimate| estimate.dose_pair);
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| AdaptiveDoseSurfaceError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation(
        id: &str,
        a: u32,
        b: u32,
        response: u16,
        uncertainty: u16,
    ) -> DoseSurfaceObservation {
        DoseSurfaceObservation {
            observation_id: id.into(),
            unit_id: format!("unit-{id}"),
            batch_id: format!("batch-{id}"),
            model_system: GliomaModelSystem::Organoid,
            dose_a_milli: a,
            dose_b_milli: b,
            response_milli: response,
            uncertainty_milli: uncertainty,
        }
    }

    fn request() -> AdaptiveDoseSurfaceRequest {
        AdaptiveDoseSurfaceRequest {
            objective: "map an organoid glioma combination response surface".into(),
            model_system: GliomaModelSystem::Organoid,
            observations: vec![
                observation("control", 0, 0, 0, 20),
                observation("low", 100, 100, 300, 35),
                observation("high", 300, 300, 760, 40),
            ],
            candidate_pairs: vec![
                DosePair {
                    dose_a_milli: 100,
                    dose_b_milli: 100,
                },
                DosePair {
                    dose_a_milli: 200,
                    dose_b_milli: 200,
                },
                DosePair {
                    dose_a_milli: 400,
                    dose_b_milli: 400,
                },
            ],
            min_replicates_per_cell: 2,
            budget_units: 4,
            cost_per_pair_units: 2,
            max_next_pairs: 2,
            target_response_milli: 600,
            exploration_weight_milli: 700,
            max_estimate_uncertainty_milli: 1_000,
            max_total_dose_milli: 1_000,
            min_pair_separation_milli: 100,
        }
    }

    #[test]
    fn planner_interpolates_surface_and_is_replay_stable() {
        let first = plan_adaptive_glioma_dose_surface(&request()).unwrap();
        let second = plan_adaptive_glioma_dose_surface(&request()).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            first.disposition,
            AdaptiveDoseSurfaceDisposition::ReadyForValidation
        );
        assert_eq!(first.selected_order.len(), 2);
        assert!(first.estimates.iter().any(|estimate| {
            estimate.dose_pair
                == (DosePair {
                    dose_a_milli: 200,
                    dose_b_milli: 200,
                })
                && matches!(estimate.state, DoseSurfaceCellState::Interpolated)
        }));
        first.validate().unwrap();
    }

    #[test]
    fn planner_keeps_sparse_surface_uncertainty_explicit() {
        let mut request = request();
        request.observations.clear();
        request.max_estimate_uncertainty_milli = 100;
        let plan = plan_adaptive_glioma_dose_surface(&request).unwrap();
        assert_eq!(
            plan.disposition,
            AdaptiveDoseSurfaceDisposition::NeedsObservations
        );
        assert!(plan.selected_order.is_empty());
        assert!(plan
            .uncertainty
            .iter()
            .any(|item| item.contains("no-safe-neighborhood")));
    }

    #[test]
    fn planner_refuses_dose_cap_violations() {
        let mut request = request();
        request.candidate_pairs.push(DosePair {
            dose_a_milli: 900,
            dose_b_milli: 900,
        });
        assert!(matches!(
            plan_adaptive_glioma_dose_surface(&request),
            Err(AdaptiveDoseSurfaceError::InvalidRequest(_))
        ));
    }
}
