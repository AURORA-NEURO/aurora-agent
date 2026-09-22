//! Quality-risk-aware acquisition scheduling for autonomous preclinical glioma workflows.
//!
//! This feature closes the loop between prospective QC forecasting and an executable research
//! plan. It chooses a bounded, deterministic modality order under cost, duration, deadline, and
//! required-modality constraints. The result is a plan for local researcher approval; it never
//! dispatches an instrument, fabricates a missing measurement, or turns a quality forecast into a
//! biological conclusion.

use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P03-F26";
pub const OUTPUT_SCHEMA: &str = "GliomaMultimodalQualityScheduler1@1";
pub const MAX_EPOCHS: usize = 4_096;
pub const MAX_CANDIDATES: usize = 64;
pub const MAX_ALTERNATIVES: usize = 16;
const BEAM_WIDTH: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityAcquisitionCandidate {
    pub modality: GliomaModality,
    pub forecast_quality_milli: u16,
    pub quality_risk_milli: u16,
    pub scientific_value_milli: u16,
    pub cost_units: u32,
    pub duration_units: u32,
    pub deadline_epoch_index: u32,
    pub required: bool,
    pub fallback_modality: Option<GliomaModality>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityScheduleRequest {
    pub objective: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub epoch_order: Vec<String>,
    pub candidates: Vec<QualityAcquisitionCandidate>,
    pub budget_units: u32,
    pub horizon_units: u32,
    pub min_forecast_quality_milli: u16,
    pub max_selected: usize,
    pub max_alternatives: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityScheduleDisposition {
    Ready,
    Conditional,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityScheduleItem {
    pub modality: GliomaModality,
    pub deadline_epoch_index: u32,
    pub forecast_quality_milli: u16,
    pub quality_risk_milli: u16,
    pub scientific_value_milli: u16,
    pub cost_units: u32,
    pub duration_units: u32,
    pub required: bool,
    pub fallback_modality: Option<GliomaModality>,
    pub urgency_milli: u16,
    pub risk_reduction_milli: u16,
    pub priority_score_milli: u64,
    pub action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityScheduleAlternative {
    pub selected_order: Vec<GliomaModality>,
    pub covered_required_count: usize,
    pub total_cost_units: u32,
    pub total_duration_units: u32,
    pub priority_score_milli: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualitySchedulePlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub epoch_order: Vec<String>,
    pub candidate_order: Vec<GliomaModality>,
    pub selected_order: Vec<GliomaModality>,
    pub selected_items: Vec<QualityScheduleItem>,
    pub rejected_order: Vec<GliomaModality>,
    pub covered_required_order: Vec<GliomaModality>,
    pub uncovered_required_order: Vec<GliomaModality>,
    pub total_cost_units: u32,
    pub total_duration_units: u32,
    pub risk_reduction_milli: u16,
    pub alternatives: Vec<QualityScheduleAlternative>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: QualityScheduleDisposition,
    pub next_action: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum QualityScheduleError {
    #[error("quality schedule request is invalid: {0}")]
    InvalidRequest(String),
    #[error("quality schedule output is invalid: {0}")]
    InvalidOutput(String),
    #[error("quality schedule digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn candidate_risk_reduction(candidate: &QualityAcquisitionCandidate, minimum: u16) -> u16 {
    candidate
        .quality_risk_milli
        .saturating_add(minimum.saturating_sub(candidate.forecast_quality_milli))
        .min(1_000)
}

fn urgency(candidate: &QualityAcquisitionCandidate, horizon: u32) -> u16 {
    if horizon == 0 {
        return 1_000;
    }
    1_000u32
        .saturating_sub(
            candidate
                .deadline_epoch_index
                .min(horizon)
                .saturating_mul(1_000)
                / horizon,
        )
        .min(1_000) as u16
}

fn priority_score(
    candidate: &QualityAcquisitionCandidate,
    request: &QualityScheduleRequest,
) -> (u16, u16, u64) {
    let risk = candidate_risk_reduction(candidate, request.min_forecast_quality_milli);
    let urgency = urgency(candidate, request.horizon_units);
    // Scientific value, risk reduction, and deadline urgency are deliberately explicit. Cost and
    // duration are represented in the denominator so a cheap preventive reacquisition can outrank
    // an expensive equivalent while required coverage is handled separately by the beam ranking.
    let value = u64::from(candidate.scientific_value_milli) * 4
        + u64::from(risk) * 5
        + u64::from(urgency) * 3;
    let denominator = u64::from(candidate.cost_units) + u64::from(candidate.duration_units);
    let score = value.saturating_mul(1_000) / denominator.max(1);
    (urgency, risk, score)
}

fn ordered_indices(
    selected: &[usize],
    candidates: &[QualityAcquisitionCandidate],
    request: &QualityScheduleRequest,
) -> Vec<usize> {
    let mut order = selected.to_vec();
    order.sort_by(|left, right| {
        let left_candidate = &candidates[*left];
        let right_candidate = &candidates[*right];
        right_candidate
            .required
            .cmp(&left_candidate.required)
            .then_with(|| {
                left_candidate
                    .deadline_epoch_index
                    .cmp(&right_candidate.deadline_epoch_index)
            })
            .then_with(|| {
                let left_score = priority_score(left_candidate, request).2;
                let right_score = priority_score(right_candidate, request).2;
                right_score.cmp(&left_score)
            })
            .then_with(|| left_candidate.modality.cmp(&right_candidate.modality))
    });
    order
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BeamState {
    selected: Vec<usize>,
    cost: u32,
    duration: u32,
    priority: u64,
    risk_reduction: u32,
    required_count: usize,
}

fn state_rank(state: &BeamState) -> (usize, u64, u32, u32, Vec<usize>) {
    (
        state.required_count,
        state.priority,
        state.risk_reduction,
        state.selected.len() as u32,
        state
            .selected
            .iter()
            .map(|index| usize::MAX.saturating_sub(*index))
            .collect(),
    )
}

fn digest_input(plan: &QualitySchedulePlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": plan.feature_id,
        "output_schema": plan.output_schema,
        "objective": plan.objective,
        "study_id": plan.study_id,
        "model_system": plan.model_system,
        "epoch_order": plan.epoch_order,
        "candidate_order": plan.candidate_order,
        "selected_order": plan.selected_order,
        "selected_items": plan.selected_items,
        "rejected_order": plan.rejected_order,
        "covered_required_order": plan.covered_required_order,
        "uncovered_required_order": plan.uncovered_required_order,
        "total_cost_units": plan.total_cost_units,
        "total_duration_units": plan.total_duration_units,
        "risk_reduction_milli": plan.risk_reduction_milli,
        "alternatives": plan.alternatives,
        "negative_evidence": plan.negative_evidence,
        "uncertainty": plan.uncertainty,
        "disposition": plan.disposition,
        "next_action": plan.next_action,
    })
}

fn validate_request(request: &QualityScheduleRequest) -> Result<(), QualityScheduleError> {
    if request.objective.trim().is_empty()
        || request.study_id.trim().is_empty()
        || request.epoch_order.is_empty()
        || request.epoch_order.len() > MAX_EPOCHS
        || request.candidates.is_empty()
        || request.candidates.len() > MAX_CANDIDATES
        || request.budget_units == 0
        || request.horizon_units == 0
        || request.min_forecast_quality_milli > 1_000
        || request.max_selected == 0
        || request.max_selected > request.candidates.len()
        || request.max_alternatives > MAX_ALTERNATIVES
        || !canonical(&request.epoch_order)
    {
        return Err(QualityScheduleError::InvalidRequest(
            "objective, canonical epochs, bounded candidates, positive budgets, and selection bounds are required".into(),
        ));
    }
    let mut modalities = BTreeSet::new();
    for candidate in &request.candidates {
        if !modalities.insert(candidate.modality)
            || candidate.forecast_quality_milli > 1_000
            || candidate.quality_risk_milli > 1_000
            || candidate.scientific_value_milli > 1_000
            || candidate.cost_units == 0
            || candidate.duration_units == 0
            || candidate.deadline_epoch_index >= request.epoch_order.len() as u32
            || candidate.deadline_epoch_index > request.horizon_units
            || candidate.fallback_modality == Some(candidate.modality)
        {
            return Err(QualityScheduleError::InvalidRequest(
                "candidates require unique modalities, bounded scores, positive resources, valid deadlines, and non-self fallbacks".into(),
            ));
        }
    }
    Ok(())
}

fn validate_output(plan: &QualitySchedulePlan) -> Result<(), QualityScheduleError> {
    if plan.feature_id != FEATURE_ID
        || plan.output_schema != OUTPUT_SCHEMA
        || plan.objective.trim().is_empty()
        || plan.study_id.trim().is_empty()
        || !canonical(&plan.epoch_order)
        || !canonical(&plan.candidate_order)
        || !canonical(&plan.rejected_order)
        || !canonical(&plan.covered_required_order)
        || !canonical(&plan.uncovered_required_order)
        || plan.selected_items.len() != plan.selected_order.len()
        || plan
            .selected_items
            .iter()
            .zip(&plan.selected_order)
            .any(|(item, modality)| item.modality != *modality)
        || plan.alternatives.len() > MAX_ALTERNATIVES
        || !canonical(&plan.negative_evidence)
        || !canonical(&plan.uncertainty)
    {
        return Err(QualityScheduleError::InvalidOutput(
            "identity, canonical ordering, selected-item cardinality, or bounded alternatives are invalid".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(plan))
        .map_err(|error| QualityScheduleError::Digest(error.to_string()))?;
    if expected != plan.digest {
        return Err(QualityScheduleError::InvalidOutput(
            "digest is not bound to quality schedule output".into(),
        ));
    }
    Ok(())
}

impl QualitySchedulePlan {
    pub fn validate(&self) -> Result<(), QualityScheduleError> {
        validate_output(self)
    }
}

/// Plan preventive multimodal reacquisition from forecast quality risk and local resource bounds.
pub fn plan_glioma_multimodal_quality_schedule(
    request: &QualityScheduleRequest,
) -> Result<QualitySchedulePlan, QualityScheduleError> {
    validate_request(request)?;
    let mut candidates = request.candidates.clone();
    candidates.sort_by_key(|candidate| candidate.modality);

    let mut beam = vec![BeamState {
        selected: Vec::new(),
        cost: 0,
        duration: 0,
        priority: 0,
        risk_reduction: 0,
        required_count: 0,
    }];
    for index in 0..candidates.len() {
        let candidate = &candidates[index];
        let mut next = Vec::with_capacity(beam.len() * 2);
        for state in &beam {
            next.push(state.clone());
            if state.selected.len() < request.max_selected
                && state.cost.saturating_add(candidate.cost_units) <= request.budget_units
                && state.duration.saturating_add(candidate.duration_units) <= request.horizon_units
            {
                let (urgency, risk, score) = priority_score(candidate, request);
                next.push(BeamState {
                    selected: state
                        .selected
                        .iter()
                        .copied()
                        .chain(std::iter::once(index))
                        .collect(),
                    cost: state.cost + candidate.cost_units,
                    duration: state.duration + candidate.duration_units,
                    priority: state.priority.saturating_add(score),
                    risk_reduction: state.risk_reduction.saturating_add(u32::from(risk)),
                    required_count: state.required_count + usize::from(candidate.required),
                });
                let _ = urgency;
            }
        }
        next.sort_by(|left, right| state_rank(right).cmp(&state_rank(left)));
        next.dedup_by(|left, right| left.selected == right.selected);
        next.truncate(BEAM_WIDTH);
        beam = next;
    }

    let required_modalities = candidates
        .iter()
        .filter(|candidate| candidate.required)
        .map(|candidate| candidate.modality)
        .collect::<Vec<_>>();
    let mut final_states = beam;
    final_states.sort_by(|left, right| state_rank(right).cmp(&state_rank(left)));
    let chosen = final_states.first().cloned().unwrap_or(BeamState {
        selected: Vec::new(),
        cost: 0,
        duration: 0,
        priority: 0,
        risk_reduction: 0,
        required_count: 0,
    });
    let chosen_indices = ordered_indices(&chosen.selected, &candidates, request);
    let selected_set = chosen.selected.iter().copied().collect::<BTreeSet<_>>();
    let selected_order = chosen_indices
        .iter()
        .map(|index| candidates[*index].modality)
        .collect::<Vec<_>>();
    let rejected_order = candidates
        .iter()
        .enumerate()
        .filter(|(index, _)| !selected_set.contains(index))
        .map(|(_, candidate)| candidate.modality)
        .collect::<Vec<_>>();
    let covered_required_order = required_modalities
        .iter()
        .copied()
        .filter(|modality| selected_order.contains(modality))
        .collect::<Vec<_>>();
    let uncovered_required_order = required_modalities
        .iter()
        .copied()
        .filter(|modality| !selected_order.contains(modality))
        .collect::<Vec<_>>();

    let selected_items = chosen_indices
        .iter()
        .map(|index| {
            let candidate = &candidates[*index];
            let (urgency, risk, score) = priority_score(candidate, request);
            QualityScheduleItem {
                modality: candidate.modality,
                deadline_epoch_index: candidate.deadline_epoch_index,
                forecast_quality_milli: candidate.forecast_quality_milli,
                quality_risk_milli: candidate.quality_risk_milli,
                scientific_value_milli: candidate.scientific_value_milli,
                cost_units: candidate.cost_units,
                duration_units: candidate.duration_units,
                required: candidate.required,
                fallback_modality: candidate.fallback_modality,
                urgency_milli: urgency,
                risk_reduction_milli: risk,
                priority_score_milli: score,
                action: if candidate.forecast_quality_milli < request.min_forecast_quality_milli {
                    "preflight and reacquire before endpoint fusion".into()
                } else {
                    "acquire before the declared modality deadline".into()
                },
            }
        })
        .collect::<Vec<_>>();

    let mut negative = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for modality in &uncovered_required_order {
        negative.insert(format!("required-modality-uncovered:{modality:?}"));
    }
    for candidate in &candidates {
        if candidate.forecast_quality_milli < request.min_forecast_quality_milli {
            uncertainty.insert(format!("forecast-below-floor:{:?}", candidate.modality));
        }
        if !selected_set.contains(
            &candidates
                .iter()
                .position(|item| item.modality == candidate.modality)
                .unwrap_or(usize::MAX),
        ) && candidate.required
        {
            negative.insert(format!(
                "required-resource-infeasible:{:?}",
                candidate.modality
            ));
        }
    }

    let mut alternatives = Vec::new();
    let mut seen_orders = BTreeSet::new();
    for state in final_states {
        let order = ordered_indices(&state.selected, &candidates, request)
            .iter()
            .map(|index| candidates[*index].modality)
            .collect::<Vec<_>>();
        if order == selected_order || !seen_orders.insert(order.clone()) {
            continue;
        }
        alternatives.push(QualityScheduleAlternative {
            selected_order: order,
            covered_required_count: state.required_count,
            total_cost_units: state.cost,
            total_duration_units: state.duration,
            priority_score_milli: state.priority,
        });
        if alternatives.len() >= request.max_alternatives {
            break;
        }
    }

    let disposition = if !uncovered_required_order.is_empty() {
        QualityScheduleDisposition::Blocked
    } else if selected_order.len() < candidates.len()
        || selected_items
            .iter()
            .any(|item| item.forecast_quality_milli < request.min_forecast_quality_milli)
    {
        QualityScheduleDisposition::Conditional
    } else if selected_order.is_empty() {
        QualityScheduleDisposition::Unresolved
    } else {
        QualityScheduleDisposition::Ready
    };
    let next_action = match disposition {
        QualityScheduleDisposition::Ready => "approve the bounded local acquisition schedule",
        QualityScheduleDisposition::Conditional => {
            "review forecast-risk and optional omissions before approving acquisition"
        }
        QualityScheduleDisposition::Blocked => {
            "resolve required modality resource or deadline constraints before acquisition"
        }
        QualityScheduleDisposition::Unresolved => "hold scheduling for researcher adjudication",
    }
    .into();

    let risk_reduction_milli = chosen.risk_reduction.min(1_000) as u16;
    let mut plan = QualitySchedulePlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        study_id: request.study_id.clone(),
        model_system: request.model_system,
        epoch_order: request.epoch_order.clone(),
        candidate_order: candidates
            .iter()
            .map(|candidate| candidate.modality)
            .collect(),
        selected_order,
        selected_items,
        rejected_order,
        covered_required_order,
        uncovered_required_order,
        total_cost_units: chosen.cost,
        total_duration_units: chosen.duration,
        risk_reduction_milli,
        alternatives,
        negative_evidence: negative.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        next_action,
        digest: ContentHash::of_value(&serde_json::Value::Null)
            .map_err(|error| QualityScheduleError::Digest(error.to_string()))?,
    };
    plan.digest = ContentHash::of_value(&digest_input(&plan))
        .map_err(|error| QualityScheduleError::Digest(error.to_string()))?;
    validate_output(&plan)?;
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(candidates: Vec<QualityAcquisitionCandidate>) -> QualityScheduleRequest {
        let max_selected = candidates.len();
        QualityScheduleRequest {
            objective: "schedule preventive acquisition before endpoint fusion".into(),
            study_id: "quality-scheduler-study".into(),
            model_system: GliomaModelSystem::Organoid,
            epoch_order: vec!["epoch-0".into(), "epoch-1".into(), "epoch-2".into()],
            candidates,
            budget_units: 10,
            horizon_units: 3,
            min_forecast_quality_milli: 700,
            max_selected,
            max_alternatives: 3,
        }
    }

    fn candidate(
        modality: GliomaModality,
        required: bool,
        cost_units: u32,
        deadline_epoch_index: u32,
    ) -> QualityAcquisitionCandidate {
        QualityAcquisitionCandidate {
            modality,
            forecast_quality_milli: 600,
            quality_risk_milli: 700,
            scientific_value_milli: 800,
            cost_units,
            duration_units: 1,
            deadline_epoch_index,
            required,
            fallback_modality: None,
        }
    }

    #[test]
    fn selects_required_and_high_risk_modalities_under_budget() {
        let output = plan_glioma_multimodal_quality_schedule(&request(vec![
            candidate(GliomaModality::Genomics, true, 4, 0),
            candidate(GliomaModality::Imaging, false, 2, 1),
            candidate(GliomaModality::Proteomics, false, 9, 2),
        ]))
        .expect("quality schedule");
        assert_eq!(output.disposition, QualityScheduleDisposition::Conditional);
        assert_eq!(output.selected_order[0], GliomaModality::Genomics);
        assert!(output.selected_order.contains(&GliomaModality::Imaging));
        assert!(output.total_cost_units <= 10);
        output.validate().expect("digest and invariants");
    }

    #[test]
    fn blocks_when_required_modality_cannot_fit() {
        let mut request = request(vec![candidate(GliomaModality::Genomics, true, 11, 0)]);
        request.budget_units = 10;
        let output = plan_glioma_multimodal_quality_schedule(&request).expect("blocked plan");
        assert_eq!(output.disposition, QualityScheduleDisposition::Blocked);
        assert_eq!(output.selected_order, Vec::<GliomaModality>::new());
        assert_eq!(
            output.uncovered_required_order,
            vec![GliomaModality::Genomics]
        );
        assert!(output
            .negative_evidence
            .iter()
            .any(|entry| entry.contains("required-modality-uncovered")));
    }

    #[test]
    fn tie_break_and_digest_are_deterministic() {
        let request = request(vec![
            candidate(GliomaModality::Imaging, false, 2, 1),
            candidate(GliomaModality::Genomics, false, 2, 1),
        ]);
        let request = QualityScheduleRequest {
            budget_units: 2,
            ..request
        };
        let first = plan_glioma_multimodal_quality_schedule(&request).expect("first plan");
        let second = plan_glioma_multimodal_quality_schedule(&request).expect("second plan");
        assert_eq!(first, second);
        assert_eq!(first.selected_order, vec![GliomaModality::Genomics]);
    }
}
