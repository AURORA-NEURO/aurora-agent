//! Robust intervention portfolio optimisation for preclinical glioma research.
//!
//! A mechanistic perturbation that looks useful in one graph can be actively misleading when
//! another plausible graph predicts the opposite effect.  This feature evaluates each candidate
//! intervention across the declared model ensemble, computes a prior-weighted lower-tail
//! (CVaR-style) effect, and greedily packs only interventions that survive worst-case,
//! agreement, risk, cost, and feasibility gates.  It is an assay-prioritisation product: no
//! perturbation is dispatched and no clinical decision is produced.

use super::counterfactual::{CounterfactualDisposition, CounterfactualIntervention};
use super::ensemble_counterfactual::{
    simulate_glioma_counterfactual_ensemble, CounterfactualEnsembleRequest, CounterfactualModel,
    EnsembleDirection,
};
use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P05-F27";
pub const OUTPUT_SCHEMA: &str = "GliomaRobustInterventionPortfolio1@1";
pub const MAX_CANDIDATES: usize = 1_024;
pub const MAX_MODELS: usize = 64;
pub const MAX_SELECTED: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PortfolioDirection {
    Increase,
    Decrease,
    AbsoluteChange,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RobustInterventionRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub max_iterations: u16,
    pub convergence_tolerance_milli: u16,
    pub damping_milli: u16,
    pub min_edge_confidence_milli: u16,
    pub direction: PortfolioDirection,
    pub budget_units: u64,
    pub max_selected: usize,
    pub min_robust_effect_milli: u64,
    pub min_agreement_milli: u16,
    pub risk_ceiling_milli: u16,
    pub effect_weight_milli: u16,
    pub tail_weight_milli: u16,
    pub worst_case_weight_milli: u16,
    pub feasibility_weight_milli: u16,
    pub risk_penalty_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RobustInterventionCandidate {
    pub candidate_id: String,
    pub label: String,
    pub intervention: CounterfactualIntervention,
    pub target_node_id: String,
    /// Candidates with the same group are treated as redundant alternatives and at most one is
    /// selected.  This prevents an autonomous run from spending the whole budget on one target.
    pub redundancy_group: String,
    pub feasibility_milli: u16,
    pub cost_units: u32,
    pub risk_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RobustInterventionScore {
    pub candidate_id: String,
    pub target_node_id: String,
    pub label: String,
    pub source_simulation_digest_order: Vec<ContentHash>,
    pub expected_effect_milli: i64,
    pub worst_case_effect_milli: i64,
    pub cvar_effect_milli: i64,
    pub best_case_effect_milli: i64,
    pub agreement_milli: u16,
    pub expected_utility_milli: i64,
    pub robust_utility_milli: i64,
    pub feasibility_milli: u16,
    pub cost_units: u32,
    pub risk_milli: u16,
    pub model_qualified: bool,
    pub eligible: bool,
    pub exclusion_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RobustPortfolioDisposition {
    Qualified,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RobustInterventionPortfolio {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub direction: PortfolioDirection,
    pub candidate_order: Vec<String>,
    pub ranked_candidate_order: Vec<String>,
    pub scores: Vec<RobustInterventionScore>,
    pub selected_order: Vec<String>,
    pub deferred_order: Vec<String>,
    pub total_cost_units: u64,
    pub budget_remaining_units: u64,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: RobustPortfolioDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RobustPortfolioError {
    #[error("robust intervention request is invalid: {0}")]
    InvalidRequest(String),
    #[error("robust intervention input is invalid: {0}")]
    InvalidInput(String),
    #[error("robust intervention output is invalid: {0}")]
    InvalidOutput(String),
    #[error("robust intervention digest failed: {0}")]
    Digest(String),
}

fn digest_input(output: &RobustInterventionPortfolio) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "model_system": output.model_system,
        "direction": output.direction,
        "candidate_order": output.candidate_order,
        "ranked_candidate_order": output.ranked_candidate_order,
        "scores": output.scores,
        "selected_order": output.selected_order,
        "deferred_order": output.deferred_order,
        "total_cost_units": output.total_cost_units,
        "budget_remaining_units": output.budget_remaining_units,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn orient(effect: i64, direction: PortfolioDirection) -> i64 {
    match direction {
        PortfolioDirection::Increase => effect,
        PortfolioDirection::Decrease => effect.saturating_neg(),
        PortfolioDirection::AbsoluteChange => effect.unsigned_abs().min(i64::MAX as u64) as i64,
    }
}

fn weighted_mean(values: &[(u16, i64)], total_prior: u64) -> i64 {
    if total_prior == 0 {
        return 0;
    }
    let numerator = values.iter().fold(0_i128, |sum, (prior, value)| {
        sum.saturating_add(i128::from(*prior) * i128::from(*value))
    });
    (numerator / i128::from(total_prior)).clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
}

/// Lower-tail prior-weighted mean.  A quarter of the declared model prior is used as the tail;
/// integer mass and a deterministic model-id tie-break keep cross-language replay exact.
fn weighted_cvar(values: &[(String, u16, i64)], total_prior: u64) -> i64 {
    if values.is_empty() || total_prior == 0 {
        return 0;
    }
    let mut ordered = values.to_vec();
    ordered.sort_by(|left, right| left.2.cmp(&right.2).then_with(|| left.0.cmp(&right.0)));
    let tail_target = (total_prior / 4).max(1);
    let mut remaining = tail_target;
    let mut mass = 0_u64;
    let mut numerator = 0_i128;
    for (_, prior, value) in ordered {
        if remaining == 0 {
            break;
        }
        let used = u64::from(prior).min(remaining);
        numerator = numerator.saturating_add(i128::from(used) * i128::from(value));
        mass = mass.saturating_add(used);
        remaining = remaining.saturating_sub(used);
    }
    if mass == 0 {
        0
    } else {
        (numerator / i128::from(mass)).clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
    }
}

fn weighted_score(
    expected: i64,
    cvar: i64,
    worst: i64,
    candidate: &RobustInterventionCandidate,
    request: &RobustInterventionRequest,
) -> i64 {
    let weight_sum = u128::from(request.effect_weight_milli)
        .saturating_add(u128::from(request.tail_weight_milli))
        .saturating_add(u128::from(request.worst_case_weight_milli));
    if weight_sum == 0 || candidate.cost_units == 0 {
        return i64::MIN;
    }
    let numerator = i128::from(expected)
        .saturating_mul(i128::from(request.effect_weight_milli))
        .saturating_add(i128::from(cvar).saturating_mul(i128::from(request.tail_weight_milli)))
        .saturating_add(
            i128::from(worst).saturating_mul(i128::from(request.worst_case_weight_milli)),
        );
    let blended = numerator / i128::try_from(weight_sum).unwrap_or(i128::MAX);
    let feasibility = i128::from(candidate.feasibility_milli)
        .saturating_mul(i128::from(request.feasibility_weight_milli.max(1)))
        / 1_000;
    let adjusted =
        blended.saturating_mul(feasibility) / i128::from(request.feasibility_weight_milli.max(1));
    let risk_penalty =
        i128::from(candidate.risk_milli).saturating_mul(i128::from(request.risk_penalty_milli));
    let net = adjusted.saturating_sub(risk_penalty);
    (net / i128::from(candidate.cost_units)).clamp(i128::from(i64::MIN), i128::from(i64::MAX))
        as i64
}

impl RobustInterventionPortfolio {
    pub fn validate(&self) -> Result<(), RobustPortfolioError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !canonical(&self.candidate_order)
            || self
                .ranked_candidate_order
                .windows(2)
                .any(|pair| pair[0] == pair[1])
            || !canonical(&self.deferred_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.scores.iter().any(|score| {
                score.candidate_id.trim().is_empty()
                    || score.target_node_id.trim().is_empty()
                    || score.label.trim().is_empty()
                    || score.source_simulation_digest_order.is_empty()
                    || score.agreement_milli > 1_000
                    || score.feasibility_milli > 1_000
                    || score.cost_units == 0
                    || score.risk_milli > 1_000
                    || (score.eligible && score.exclusion_reason.is_some())
                    || (!score.eligible && score.exclusion_reason.is_none())
            })
        {
            return Err(RobustPortfolioError::InvalidOutput(
                "identity, ordering, bounds, or eligibility explanation is invalid".into(),
            ));
        }
        let candidates = self
            .candidate_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let score_ids = self
            .scores
            .iter()
            .map(|score| score.candidate_id.clone())
            .collect::<BTreeSet<_>>();
        let ranked = self
            .ranked_candidate_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let selected = self.selected_order.iter().cloned().collect::<BTreeSet<_>>();
        let deferred = self.deferred_order.iter().cloned().collect::<BTreeSet<_>>();
        if candidates.is_empty()
            || candidates != score_ids
            || candidates != ranked
            || self
                .scores
                .iter()
                .map(|score| score.candidate_id.clone())
                .collect::<Vec<_>>()
                != self.ranked_candidate_order
            || self.scores.windows(2).any(|pair| {
                pair[0].robust_utility_milli < pair[1].robust_utility_milli
                    || (pair[0].robust_utility_milli == pair[1].robust_utility_milli
                        && pair[0].agreement_milli < pair[1].agreement_milli)
            })
            || selected.intersection(&deferred).next().is_some()
            || selected.union(&deferred).cloned().collect::<BTreeSet<_>>() != candidates
            || selected.iter().any(|id| {
                self.scores
                    .iter()
                    .find(|score| &score.candidate_id == id)
                    .is_none_or(|score| !score.eligible)
            })
        {
            return Err(RobustPortfolioError::InvalidOutput(
                "candidate partitions or selected eligibility do not reconcile".into(),
            ));
        }
        if self
            .total_cost_units
            .saturating_add(self.budget_remaining_units)
            == 0
        {
            return Err(RobustPortfolioError::InvalidOutput(
                "portfolio budget accounting is empty".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| RobustPortfolioError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(RobustPortfolioError::InvalidOutput(
                "portfolio digest is not bound to scores and source simulations".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &RobustInterventionRequest,
    models: &[CounterfactualModel],
    candidates: &[RobustInterventionCandidate],
) -> Result<(), RobustPortfolioError> {
    let weight_sum = u32::from(request.effect_weight_milli)
        .saturating_add(u32::from(request.tail_weight_milli))
        .saturating_add(u32::from(request.worst_case_weight_milli));
    if request.objective.trim().is_empty()
        || request.max_iterations == 0
        || request.convergence_tolerance_milli > 1_000
        || request.damping_milli > 1_000
        || request.min_edge_confidence_milli > 1_000
        || request.budget_units == 0
        || request.max_selected == 0
        || request.max_selected > MAX_SELECTED
        || request.min_robust_effect_milli == 0
        || request.min_robust_effect_milli > 1_000
        || request.min_agreement_milli > 1_000
        || request.risk_ceiling_milli > 1_000
        || request.effect_weight_milli > 1_000
        || request.tail_weight_milli > 1_000
        || request.worst_case_weight_milli > 1_000
        || weight_sum == 0
        || request.feasibility_weight_milli == 0
        || request.feasibility_weight_milli > 1_000
        || request.risk_penalty_milli > 1_000
        || models.is_empty()
        || models.len() > MAX_MODELS
        || candidates.is_empty()
        || candidates.len() > MAX_CANDIDATES
    {
        return Err(RobustPortfolioError::InvalidRequest(
            "objective, bounded model ensemble, positive budget/selection, gates, and weights are required".into(),
        ));
    }
    let mut candidate_ids = BTreeSet::new();
    for candidate in candidates {
        if candidate.candidate_id.trim().is_empty()
            || !candidate_ids.insert(candidate.candidate_id.clone())
            || candidate.label.trim().is_empty()
            || candidate.target_node_id.trim().is_empty()
            || candidate.redundancy_group.trim().is_empty()
            || candidate.feasibility_milli == 0
            || candidate.feasibility_milli > 1_000
            || candidate.cost_units == 0
            || candidate.risk_milli > 1_000
        {
            return Err(RobustPortfolioError::InvalidInput(
                "candidate identity, target/group, feasibility, cost, or risk bounds are invalid"
                    .into(),
            ));
        }
    }
    if models.len() > MAX_MODELS {
        return Err(RobustPortfolioError::InvalidInput(
            "model bound exceeded".into(),
        ));
    }
    Ok(())
}

/// Evaluate and greedily select a robust preclinical intervention portfolio.
pub fn plan_glioma_robust_intervention_portfolio(
    request: &RobustInterventionRequest,
    models: &[CounterfactualModel],
    candidates: &[RobustInterventionCandidate],
) -> Result<RobustInterventionPortfolio, RobustPortfolioError> {
    validate_request(request, models, candidates)?;
    let mut ordered_candidates = candidates.to_vec();
    ordered_candidates.sort_by(|left, right| left.candidate_id.cmp(&right.candidate_id));
    let candidate_order = ordered_candidates
        .iter()
        .map(|candidate| candidate.candidate_id.clone())
        .collect::<Vec<_>>();
    let min_effect = request.min_robust_effect_milli.max(1);
    let ensemble_request = CounterfactualEnsembleRequest {
        objective: request.objective.clone(),
        model_system: request.model_system,
        max_iterations: request.max_iterations,
        convergence_tolerance_milli: request.convergence_tolerance_milli,
        damping_milli: request.damping_milli,
        min_edge_confidence_milli: request.min_edge_confidence_milli,
        min_effect_milli: min_effect,
        min_model_agreement_milli: request.min_agreement_milli,
        top_k: 1,
    };
    let mut scored = Vec::with_capacity(ordered_candidates.len());
    let mut negative = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for candidate in &ordered_candidates {
        let ensemble = simulate_glioma_counterfactual_ensemble(
            &ensemble_request,
            models,
            std::slice::from_ref(&candidate.intervention),
        )
        .map_err(|error| RobustPortfolioError::InvalidInput(error.to_string()))?;
        let Some(target) = ensemble
            .targets
            .iter()
            .find(|target| target.node_id == candidate.target_node_id)
        else {
            return Err(RobustPortfolioError::InvalidInput(format!(
                "candidate {} target {} is absent from ensemble",
                candidate.candidate_id, candidate.target_node_id
            )));
        };
        let mut effects = Vec::with_capacity(ensemble.models.len());
        for model in &ensemble.models {
            let effect = model
                .simulation
                .contrasts
                .iter()
                .find(|contrast| contrast.node_id == candidate.target_node_id)
                .map(|contrast| contrast.effect_milli)
                .ok_or_else(|| {
                    RobustPortfolioError::InvalidInput(format!(
                        "model {} omitted target {}",
                        model.model_id, candidate.target_node_id
                    ))
                })?;
            effects.push((
                model.model_id.clone(),
                model.prior_milli,
                effect,
                orient(effect, request.direction),
            ));
        }
        let oriented = effects
            .iter()
            .map(|(_, prior, _, value)| (*prior, *value))
            .collect::<Vec<_>>();
        let expected_utility = weighted_mean(&oriented, ensemble.total_prior_milli);
        let cvar = weighted_cvar(
            &effects
                .iter()
                .map(|(id, prior, _, value)| (id.clone(), *prior, *value))
                .collect::<Vec<_>>(),
            ensemble.total_prior_milli,
        );
        let worst = oriented.iter().map(|(_, value)| *value).min().unwrap_or(0);
        let agreeing_prior = oriented
            .iter()
            .filter(|(_, value)| *value >= min_effect as i64)
            .map(|(prior, _)| u64::from(*prior))
            .sum::<u64>();
        let agreement = agreeing_prior
            .saturating_mul(1_000)
            .checked_div(ensemble.total_prior_milli.max(1))
            .unwrap_or(0)
            .min(1_000) as u16;
        // The ensemble disposition also considers every node in the graph.  A candidate should
        // not be rejected merely because an unrelated node is unchanged or disputed, so the
        // portfolio gate is scoped to model simulation quality plus this candidate's target.
        let model_qualified = ensemble
            .models
            .iter()
            .all(|model| model.simulation.disposition == CounterfactualDisposition::Qualified);
        let robust_utility = weighted_score(expected_utility, cvar, worst, candidate, request);
        let target_direction_ok = match request.direction {
            PortfolioDirection::Increase => target.direction == EnsembleDirection::Increases,
            PortfolioDirection::Decrease => target.direction == EnsembleDirection::Decreases,
            PortfolioDirection::AbsoluteChange => true,
        };
        let eligible = model_qualified
            && candidate.risk_milli <= request.risk_ceiling_milli
            && worst >= min_effect as i64
            && agreement >= request.min_agreement_milli
            && target_direction_ok;
        let exclusion_reason = if eligible {
            None
        } else if candidate.risk_milli > request.risk_ceiling_milli {
            negative.insert(format!("risk-ceiling-blocked:{}", candidate.candidate_id));
            Some("risk-ceiling-blocked".into())
        } else if !model_qualified {
            uncertainty.insert(format!(
                "model-qualification-incomplete:{}",
                candidate.candidate_id
            ));
            Some("model-qualification-incomplete".into())
        } else if worst < min_effect as i64 {
            negative.insert(format!(
                "worst-case-effect-below-floor:{}",
                candidate.candidate_id
            ));
            Some("worst-case-effect-below-floor".into())
        } else {
            uncertainty.insert(format!(
                "model-agreement-below-floor:{}",
                candidate.candidate_id
            ));
            Some("model-agreement-below-floor".into())
        };
        scored.push(RobustInterventionScore {
            candidate_id: candidate.candidate_id.clone(),
            target_node_id: candidate.target_node_id.clone(),
            label: candidate.label.clone(),
            source_simulation_digest_order: ensemble
                .models
                .iter()
                .map(|model| model.simulation.digest.clone())
                .collect(),
            expected_effect_milli: target.weighted_effect_milli,
            worst_case_effect_milli: effects
                .iter()
                .map(|(_, _, effect, _)| *effect)
                .min()
                .unwrap_or(0),
            cvar_effect_milli: match request.direction {
                PortfolioDirection::Decrease => cvar.saturating_neg(),
                PortfolioDirection::Increase => cvar,
                PortfolioDirection::AbsoluteChange => cvar,
            },
            best_case_effect_milli: effects
                .iter()
                .map(|(_, _, effect, _)| *effect)
                .max()
                .unwrap_or(0),
            agreement_milli: agreement,
            expected_utility_milli: expected_utility,
            robust_utility_milli: robust_utility,
            feasibility_milli: candidate.feasibility_milli,
            cost_units: candidate.cost_units,
            risk_milli: candidate.risk_milli,
            model_qualified,
            eligible,
            exclusion_reason,
        });
    }
    scored.sort_by(|left, right| {
        right
            .robust_utility_milli
            .cmp(&left.robust_utility_milli)
            .then_with(|| right.agreement_milli.cmp(&left.agreement_milli))
            .then_with(|| left.candidate_id.cmp(&right.candidate_id))
    });
    let ranked_candidate_order = scored
        .iter()
        .map(|score| score.candidate_id.clone())
        .collect::<Vec<_>>();
    let mut selected_order = Vec::new();
    let mut selected_groups = BTreeSet::new();
    let mut total_cost = 0_u64;
    for score in &scored {
        if selected_order.len() >= request.max_selected || !score.eligible {
            continue;
        }
        let candidate = ordered_candidates
            .iter()
            .find(|candidate| candidate.candidate_id == score.candidate_id)
            .expect("score candidate exists");
        if selected_groups.contains(&candidate.redundancy_group)
            || total_cost.saturating_add(u64::from(candidate.cost_units)) > request.budget_units
        {
            continue;
        }
        selected_groups.insert(candidate.redundancy_group.clone());
        total_cost = total_cost.saturating_add(u64::from(candidate.cost_units));
        selected_order.push(candidate.candidate_id.clone());
    }
    let selected_ids = selected_order.iter().cloned().collect::<BTreeSet<_>>();
    let deferred_order = candidate_order
        .iter()
        .filter(|id| !selected_ids.contains(*id))
        .cloned()
        .collect::<Vec<_>>();
    if selected_order.is_empty() {
        negative.insert("no-intervention-survives-robust-gates".into());
    }
    if scored
        .iter()
        .any(|score| score.eligible && !selected_ids.contains(&score.candidate_id))
    {
        uncertainty.insert("eligible-interventions-deferred-by-budget-or-redundancy".into());
    }
    let eligible_count = scored.iter().filter(|score| score.eligible).count();
    let disposition = if selected_order.is_empty() {
        RobustPortfolioDisposition::Unresolved
    } else if scored.iter().any(|score| !score.eligible) || selected_order.len() < eligible_count {
        RobustPortfolioDisposition::Partial
    } else {
        RobustPortfolioDisposition::Qualified
    };
    let mut output = RobustInterventionPortfolio {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        direction: request.direction,
        candidate_order,
        ranked_candidate_order,
        scores: scored,
        selected_order,
        deferred_order,
        total_cost_units: total_cost,
        budget_remaining_units: request.budget_units.saturating_sub(total_cost),
        negative_evidence: negative.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-robust-intervention-portfolio"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| RobustPortfolioError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p05_mechanism_exploration::graph_propagation::{
        MechanismGraphEdge, MechanismGraphNode, MechanismGraphRelation,
    };
    use crate::glioma_engine::GliomaModality;

    fn node(id: &str, support: u16) -> MechanismGraphNode {
        MechanismGraphNode {
            node_id: id.into(),
            label: id.into(),
            modality: GliomaModality::Genomics,
            prior_milli: 0,
            support_milli: support,
            contradiction_milli: 0,
        }
    }

    fn model(id: &str, support: u16) -> CounterfactualModel {
        CounterfactualModel {
            model_id: id.into(),
            prior_milli: 500,
            nodes: vec![node("egfr", support), node("invasion", 0)],
            edges: vec![MechanismGraphEdge {
                edge_id: format!("{id}-edge"),
                source_node_id: "egfr".into(),
                target_node_id: "invasion".into(),
                relation: MechanismGraphRelation::Activates,
                confidence_milli: 900,
                evidence_order: vec![format!("evidence-{id}")],
            }],
        }
    }

    fn request() -> RobustInterventionRequest {
        RobustInterventionRequest {
            objective: "select robust invasion-suppressing perturbations".into(),
            model_system: GliomaModelSystem::Organoid,
            max_iterations: 100,
            convergence_tolerance_milli: 1,
            damping_milli: 600,
            min_edge_confidence_milli: 500,
            direction: PortfolioDirection::Decrease,
            budget_units: 3,
            max_selected: 2,
            min_robust_effect_milli: 10,
            min_agreement_milli: 750,
            risk_ceiling_milli: 800,
            effect_weight_milli: 500,
            tail_weight_milli: 300,
            worst_case_weight_milli: 200,
            feasibility_weight_milli: 1_000,
            risk_penalty_milli: 1,
        }
    }

    fn candidate(id: &str, group: &str, target: &str, risk: u16) -> RobustInterventionCandidate {
        RobustInterventionCandidate {
            candidate_id: id.into(),
            label: id.into(),
            intervention: CounterfactualIntervention {
                intervention_id: format!("{id}-intervention"),
                node_id: "egfr".into(),
                delta_milli: -600,
                rationale: "test EGFR-to-invasion mechanism".into(),
                evidence_order: vec![format!("evidence-{id}")],
            },
            target_node_id: target.into(),
            redundancy_group: group.into(),
            feasibility_milli: 1_000,
            cost_units: 1,
            risk_milli: risk,
        }
    }

    #[test]
    fn selects_robust_candidate_and_is_replay_stable() {
        let models = vec![model("optimistic", 900), model("conservative", 700)];
        let candidates = vec![candidate("egfr-invasion", "egfr", "invasion", 100)];
        let first =
            plan_glioma_robust_intervention_portfolio(&request(), &models, &candidates).unwrap();
        let second =
            plan_glioma_robust_intervention_portfolio(&request(), &models, &candidates).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.selected_order, vec!["egfr-invasion"]);
        assert_eq!(first.disposition, RobustPortfolioDisposition::Qualified);
        first.validate().unwrap();
    }

    #[test]
    fn risk_and_redundancy_gates_are_explicit() {
        let models = vec![model("optimistic", 900), model("conservative", 700)];
        let candidates = vec![
            candidate("safe", "egfr", "invasion", 100),
            candidate("unsafe", "other", "invasion", 900),
        ];
        let output =
            plan_glioma_robust_intervention_portfolio(&request(), &models, &candidates).unwrap();
        assert_eq!(output.selected_order, vec!["safe"]);
        assert!(output
            .negative_evidence
            .iter()
            .any(|item| item.contains("risk-ceiling-blocked")));
        output.validate().unwrap();
    }
}
