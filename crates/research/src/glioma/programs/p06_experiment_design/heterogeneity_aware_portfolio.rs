//! Heterogeneity-aware experiment portfolio allocation for preclinical glioma research.
//!
//! This composition capability turns declared assay candidates into a reproducible, budgeted
//! portfolio while reserving capacity for replication. It uses a bounded beam over replicate
//! counts, rewards model-system and stratum diversity, penalizes heterogeneity and risk, and
//! reports stress power rather than pretending a planning score is observed biology. It never
//! dispatches an assay or makes a clinical decision.

use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const COMPOSITION_ID: &str = "glioma-heterogeneity-aware-experiment-portfolio";
pub const OUTPUT_SCHEMA: &str = "GliomaHeterogeneityAwareExperimentPortfolio1@1";
pub const MAX_CANDIDATES: usize = 512;
pub const MAX_STRATA: usize = 128;
pub const MAX_SELECTED_ARMS: usize = 128;
const BEAM_WIDTH: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeterogeneityExperimentStratum {
    pub stratum_id: String,
    pub model_system: GliomaModelSystem,
    pub prior_milli: u16,
    pub heterogeneity_milli: u16,
    pub required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeterogeneityExperimentCandidate {
    pub candidate_id: String,
    pub arm_id: String,
    pub stratum_id: String,
    pub modality: GliomaModality,
    pub independence_group: String,
    pub cost_units_per_replicate: u32,
    pub max_replicates: u16,
    pub expected_effect_milli: u32,
    pub effect_uncertainty_milli: u32,
    pub reproducibility_milli: u16,
    pub risk_milli: u16,
    pub available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeterogeneityAwareExperimentPortfolioRequest {
    pub objective: String,
    pub budget_units: u64,
    pub replication_reserve_fraction_milli: u16,
    pub min_model_systems: usize,
    pub min_power_milli: u16,
    pub max_risk_milli: u16,
    pub max_selected_arms: usize,
    pub strata: Vec<HeterogeneityExperimentStratum>,
    pub candidates: Vec<HeterogeneityExperimentCandidate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HeterogeneityPortfolioDisposition {
    Qualified,
    Partial,
    Underpowered,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeterogeneityPortfolioSelection {
    pub candidate_id: String,
    pub arm_id: String,
    pub stratum_id: String,
    pub model_system: GliomaModelSystem,
    pub modality: GliomaModality,
    pub planned_replicates: u16,
    pub discovery_cost_units: u64,
    pub estimated_information_milli: u64,
    pub utility_milli: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeterogeneityPortfolioDeferral {
    pub candidate_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeterogeneityPowerStressPoint {
    pub scenario_id: String,
    pub heterogeneity_multiplier_milli: u16,
    pub estimated_power_milli: u16,
    pub uncertainty_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeterogeneityAwareExperimentPortfolio {
    pub composition_id: String,
    pub output_schema: String,
    pub objective: String,
    pub selected: Vec<HeterogeneityPortfolioSelection>,
    pub deferred: Vec<HeterogeneityPortfolioDeferral>,
    pub model_system_order: Vec<GliomaModelSystem>,
    pub stratum_order: Vec<String>,
    pub modality_order: Vec<GliomaModality>,
    pub discovery_budget_units: u64,
    pub discovery_spent_units: u64,
    pub replication_reserve_units: u64,
    pub estimated_power_milli: u16,
    pub heterogeneity_penalty_milli: u16,
    pub stress: Vec<HeterogeneityPowerStressPoint>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: HeterogeneityPortfolioDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum HeterogeneityAwareExperimentPortfolioError {
    #[error("heterogeneity-aware portfolio request is invalid: {0}")]
    InvalidRequest(String),
    #[error("heterogeneity-aware portfolio output is invalid: {0}")]
    InvalidOutput(String),
    #[error("heterogeneity-aware portfolio digest failed: {0}")]
    Digest(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct BeamState {
    selected: Vec<(usize, u16)>,
    cost_units: u64,
    utility_milli: u64,
    model_systems: BTreeSet<GliomaModelSystem>,
    strata: BTreeSet<String>,
    groups: BTreeSet<String>,
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn sorted_unique(mut values: Vec<String>) -> Vec<String> {
    values.sort();
    values.dedup();
    values
}

fn digest_input(output: &HeterogeneityAwareExperimentPortfolio) -> serde_json::Value {
    serde_json::json!({
        "composition_id": output.composition_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "selected": output.selected,
        "deferred": output.deferred,
        "model_system_order": output.model_system_order,
        "stratum_order": output.stratum_order,
        "modality_order": output.modality_order,
        "discovery_budget_units": output.discovery_budget_units,
        "discovery_spent_units": output.discovery_spent_units,
        "replication_reserve_units": output.replication_reserve_units,
        "estimated_power_milli": output.estimated_power_milli,
        "heterogeneity_penalty_milli": output.heterogeneity_penalty_milli,
        "stress": output.stress,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn validate_request(
    request: &HeterogeneityAwareExperimentPortfolioRequest,
) -> Result<(), HeterogeneityAwareExperimentPortfolioError> {
    if request.objective.trim().is_empty()
        || request.budget_units == 0
        || request.replication_reserve_fraction_milli > 900
        || request.min_model_systems == 0
        || request.min_power_milli > 1_000
        || request.max_risk_milli > 1_000
        || request.max_selected_arms == 0
        || request.max_selected_arms > MAX_SELECTED_ARMS
        || request.strata.is_empty()
        || request.strata.len() > MAX_STRATA
        || request.candidates.is_empty()
        || request.candidates.len() > MAX_CANDIDATES
    {
        return Err(HeterogeneityAwareExperimentPortfolioError::InvalidRequest(
            "objective, positive budget, bounded reserve/risk/power, strata, candidates, and arm limit are required".into(),
        ));
    }
    let mut strata = BTreeSet::new();
    for stratum in &request.strata {
        if stratum.stratum_id.trim().is_empty()
            || !strata.insert(stratum.stratum_id.clone())
            || stratum.prior_milli == 0
            || stratum.heterogeneity_milli > 1_000
        {
            return Err(HeterogeneityAwareExperimentPortfolioError::InvalidRequest(
                "strata require unique ids, positive priors, and bounded heterogeneity".into(),
            ));
        }
    }
    let mut candidates = BTreeSet::new();
    for candidate in &request.candidates {
        if candidate.candidate_id.trim().is_empty()
            || candidate.arm_id.trim().is_empty()
            || candidate.independence_group.trim().is_empty()
            || !strata.contains(&candidate.stratum_id)
            || !candidates.insert(candidate.candidate_id.clone())
            || candidate.cost_units_per_replicate == 0
            || candidate.max_replicates == 0
            || candidate.expected_effect_milli == 0
            || candidate.effect_uncertainty_milli == 0
            || candidate.reproducibility_milli > 1_000
            || candidate.risk_milli > 1_000
        {
            return Err(HeterogeneityAwareExperimentPortfolioError::InvalidRequest(
                "candidates require unique ids, known strata, positive cost/effect/uncertainty/replicates, and bounded risk/reproducibility".into(),
            ));
        }
    }
    Ok(())
}

fn candidate_information(candidate: &HeterogeneityExperimentCandidate, replicates: u16) -> u64 {
    let replication_gain = u64::from(replicates)
        .saturating_mul(1_000)
        .saturating_div(u64::from(replicates).saturating_add(1));
    u64::from(candidate.expected_effect_milli)
        .saturating_mul(u64::from(candidate.reproducibility_milli))
        .saturating_mul(replication_gain)
        .saturating_div(u64::from(candidate.effect_uncertainty_milli).max(1))
}

fn state_score(state: &BeamState, required_strata: &BTreeSet<String>) -> u128 {
    let required_covered = state.strata.intersection(required_strata).count() as u128;
    let diversity = (state.model_systems.len() as u128)
        .saturating_mul(80_000)
        .saturating_add((state.strata.len() as u128).saturating_mul(20_000))
        .saturating_add(required_covered.saturating_mul(150_000));
    u128::from(state.utility_milli)
        .saturating_mul(1_000)
        .saturating_add(diversity)
        .saturating_sub(u128::from(state.cost_units).saturating_mul(10))
}

fn validate_output(
    output: &HeterogeneityAwareExperimentPortfolio,
) -> Result<(), HeterogeneityAwareExperimentPortfolioError> {
    if output.composition_id != COMPOSITION_ID
        || output.output_schema != OUTPUT_SCHEMA
        || output.objective.trim().is_empty()
        || output.selected.len() > MAX_SELECTED_ARMS
        || !canonical(&output.model_system_order)
        || !canonical(&output.stratum_order)
        || !canonical(&output.modality_order)
        || output.discovery_spent_units > output.discovery_budget_units
        || output.replication_reserve_units == 0
        || output.estimated_power_milli > 1_000
        || output.heterogeneity_penalty_milli > 1_000
        || output.stress.len() != 3
        || output.stress.windows(2).any(|pair| {
            pair[0].heterogeneity_multiplier_milli >= pair[1].heterogeneity_multiplier_milli
        })
        || output
            .negative_evidence
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || output.uncertainty.windows(2).any(|pair| pair[0] >= pair[1])
        || output.selected.windows(2).any(|pair| {
            pair[0].utility_milli < pair[1].utility_milli
                || (pair[0].utility_milli == pair[1].utility_milli
                    && pair[0].candidate_id > pair[1].candidate_id)
        })
    {
        return Err(HeterogeneityAwareExperimentPortfolioError::InvalidOutput(
            "identity, ordering, bounds, stress cardinality, or budget accounting is invalid"
                .into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(output))
        .map_err(|error| HeterogeneityAwareExperimentPortfolioError::Digest(error.to_string()))?;
    if expected != output.digest {
        return Err(HeterogeneityAwareExperimentPortfolioError::InvalidOutput(
            "portfolio output digest is not content-addressed".into(),
        ));
    }
    Ok(())
}

impl HeterogeneityAwareExperimentPortfolio {
    pub fn validate(&self) -> Result<(), HeterogeneityAwareExperimentPortfolioError> {
        validate_output(self)
    }
}

/// Allocate a diverse glioma assay portfolio while protecting a replication reserve. The search
/// is deterministic, bounded, and planning-only; no assay result or biological effect is inferred.
pub fn plan_glioma_heterogeneity_aware_experiment_portfolio(
    request: &HeterogeneityAwareExperimentPortfolioRequest,
) -> Result<HeterogeneityAwareExperimentPortfolio, HeterogeneityAwareExperimentPortfolioError> {
    validate_request(request)?;
    let strata = request
        .strata
        .iter()
        .map(|stratum| (stratum.stratum_id.clone(), stratum))
        .collect::<BTreeMap<_, _>>();
    let required_strata = request
        .strata
        .iter()
        .filter(|stratum| stratum.required)
        .map(|stratum| stratum.stratum_id.clone())
        .collect::<BTreeSet<_>>();
    let replication_reserve_units = request
        .budget_units
        .saturating_mul(u64::from(request.replication_reserve_fraction_milli))
        .saturating_div(1_000)
        .max(1);
    let discovery_budget_units = request
        .budget_units
        .saturating_sub(replication_reserve_units);
    let mut deferred = Vec::new();
    let mut eligible = Vec::new();
    for (index, candidate) in request.candidates.iter().enumerate() {
        if !candidate.available {
            deferred.push(HeterogeneityPortfolioDeferral {
                candidate_id: candidate.candidate_id.clone(),
                reason: "candidate-unavailable".into(),
            });
        } else if candidate.risk_milli > request.max_risk_milli {
            deferred.push(HeterogeneityPortfolioDeferral {
                candidate_id: candidate.candidate_id.clone(),
                reason: "risk-ceiling".into(),
            });
        } else {
            eligible.push(index);
        }
    }
    let empty = BeamState {
        selected: Vec::new(),
        cost_units: 0,
        utility_milli: 0,
        model_systems: BTreeSet::new(),
        strata: BTreeSet::new(),
        groups: BTreeSet::new(),
    };
    let mut states = vec![empty];
    for index in eligible {
        let candidate = &request.candidates[index];
        let mut next = states.clone();
        for state in &states {
            if state.selected.len() >= request.max_selected_arms
                || state.groups.contains(&candidate.independence_group)
            {
                continue;
            }
            for replicates in 1..=candidate.max_replicates.min(32) {
                let cost = u64::from(candidate.cost_units_per_replicate)
                    .saturating_mul(u64::from(replicates));
                if state.cost_units.saturating_add(cost) > discovery_budget_units {
                    break;
                }
                let mut selected = state.selected.clone();
                selected.push((index, replicates));
                let mut model_systems = state.model_systems.clone();
                model_systems.insert(strata[&candidate.stratum_id].model_system);
                let mut stratum_ids = state.strata.clone();
                stratum_ids.insert(candidate.stratum_id.clone());
                let mut groups = state.groups.clone();
                groups.insert(candidate.independence_group.clone());
                let diversity_bonus = if model_systems.len() > state.model_systems.len() {
                    25_000
                } else {
                    0
                };
                next.push(BeamState {
                    selected,
                    cost_units: state.cost_units.saturating_add(cost),
                    utility_milli: state
                        .utility_milli
                        .saturating_add(candidate_information(candidate, replicates))
                        .saturating_add(diversity_bonus),
                    model_systems,
                    strata: stratum_ids,
                    groups,
                });
            }
        }
        next.sort_by(|left, right| {
            state_score(right, &required_strata)
                .cmp(&state_score(left, &required_strata))
                .then_with(|| left.selected.cmp(&right.selected))
        });
        next.dedup_by(|left, right| left.selected == right.selected);
        next.truncate(BEAM_WIDTH);
        states = next;
    }
    let selected_state = states
        .into_iter()
        .max_by(|left, right| {
            state_score(left, &required_strata)
                .cmp(&state_score(right, &required_strata))
                .then_with(|| right.selected.cmp(&left.selected))
        })
        .unwrap_or_else(|| BeamState {
            selected: Vec::new(),
            cost_units: 0,
            utility_milli: 0,
            model_systems: BTreeSet::new(),
            strata: BTreeSet::new(),
            groups: BTreeSet::new(),
        });
    let mut selected = selected_state
        .selected
        .iter()
        .map(|(index, replicates)| {
            let candidate = &request.candidates[*index];
            let information = candidate_information(candidate, *replicates);
            HeterogeneityPortfolioSelection {
                candidate_id: candidate.candidate_id.clone(),
                arm_id: candidate.arm_id.clone(),
                stratum_id: candidate.stratum_id.clone(),
                model_system: strata[&candidate.stratum_id].model_system,
                modality: candidate.modality,
                planned_replicates: *replicates,
                discovery_cost_units: u64::from(candidate.cost_units_per_replicate)
                    .saturating_mul(u64::from(*replicates)),
                estimated_information_milli: information,
                utility_milli: information,
            }
        })
        .collect::<Vec<_>>();
    selected.sort_by(|left, right| {
        right
            .utility_milli
            .cmp(&left.utility_milli)
            .then_with(|| left.candidate_id.cmp(&right.candidate_id))
    });
    let selected_ids = selected
        .iter()
        .map(|selection| selection.candidate_id.clone())
        .collect::<BTreeSet<_>>();
    for candidate in &request.candidates {
        if !selected_ids.contains(&candidate.candidate_id)
            && !deferred
                .iter()
                .any(|item| item.candidate_id == candidate.candidate_id)
        {
            deferred.push(HeterogeneityPortfolioDeferral {
                candidate_id: candidate.candidate_id.clone(),
                reason: "beam-deferred-or-budget".into(),
            });
        }
    }
    deferred.sort_by(|left, right| left.candidate_id.cmp(&right.candidate_id));
    let discovery_spent_units = selected
        .iter()
        .map(|selection| selection.discovery_cost_units)
        .sum::<u64>();
    let weighted_information = selected
        .iter()
        .map(|selection| selection.estimated_information_milli)
        .sum::<u64>();
    let heterogeneity_penalty = selected
        .iter()
        .map(|selection| u64::from(strata[&selection.stratum_id].heterogeneity_milli))
        .sum::<u64>()
        .saturating_div(selected.len().max(1) as u64)
        .min(1_000) as u16;
    let estimated_power = weighted_information
        .saturating_div(selected.len().max(1) as u64)
        .min(1_000)
        .saturating_sub(u64::from(heterogeneity_penalty) / 2) as u16;
    let mut stress = vec![
        HeterogeneityPowerStressPoint {
            scenario_id: "low_heterogeneity".into(),
            heterogeneity_multiplier_milli: 500,
            estimated_power_milli: estimated_power
                .saturating_add(heterogeneity_penalty / 4)
                .min(1_000),
            uncertainty_milli: 1_000_u16.saturating_sub(estimated_power),
        },
        HeterogeneityPowerStressPoint {
            scenario_id: "declared_heterogeneity".into(),
            heterogeneity_multiplier_milli: 1_000,
            estimated_power_milli: estimated_power,
            uncertainty_milli: 1_000_u16.saturating_sub(estimated_power),
        },
        HeterogeneityPowerStressPoint {
            scenario_id: "stress_heterogeneity".into(),
            heterogeneity_multiplier_milli: 1_500,
            estimated_power_milli: estimated_power.saturating_sub(heterogeneity_penalty / 2),
            uncertainty_milli: 1_000_u16
                .saturating_sub(estimated_power.saturating_sub(heterogeneity_penalty / 2)),
        },
    ];
    stress.sort_by_key(|point| point.heterogeneity_multiplier_milli);
    let mut uncertainty = Vec::new();
    let mut negative_evidence = Vec::new();
    if selected.is_empty() {
        negative_evidence.push("no candidate fits the discovery budget and risk envelope".into());
    }
    if selected_state.model_systems.len() < request.min_model_systems {
        uncertainty.push(format!(
            "model-system coverage {} below required {}",
            selected_state.model_systems.len(),
            request.min_model_systems
        ));
    }
    let missing_required = required_strata
        .difference(&selected_state.strata)
        .cloned()
        .collect::<Vec<_>>();
    if !missing_required.is_empty() {
        uncertainty.push(format!(
            "required strata not covered: {}",
            missing_required.join(", ")
        ));
    }
    if estimated_power < request.min_power_milli {
        uncertainty.push(format!(
            "estimated planning power {} milli below required {}",
            estimated_power, request.min_power_milli
        ));
    }
    uncertainty.push(format!(
        "{} budget units remain protected for replication",
        replication_reserve_units
    ));
    let disposition = if selected.is_empty() {
        HeterogeneityPortfolioDisposition::Blocked
    } else if !missing_required.is_empty()
        || selected_state.model_systems.len() < request.min_model_systems
        || estimated_power < request.min_power_milli
    {
        HeterogeneityPortfolioDisposition::Underpowered
    } else if !deferred.is_empty() {
        HeterogeneityPortfolioDisposition::Partial
    } else {
        HeterogeneityPortfolioDisposition::Qualified
    };
    let mut output = HeterogeneityAwareExperimentPortfolio {
        composition_id: COMPOSITION_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        selected,
        deferred,
        model_system_order: selected_state.model_systems.into_iter().collect(),
        stratum_order: selected_state.strata.into_iter().collect(),
        modality_order: request
            .candidates
            .iter()
            .filter(|candidate| selected_ids.contains(&candidate.candidate_id))
            .map(|candidate| candidate.modality)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        discovery_budget_units,
        discovery_spent_units,
        replication_reserve_units,
        estimated_power_milli: estimated_power,
        heterogeneity_penalty_milli: heterogeneity_penalty,
        stress,
        negative_evidence: sorted_unique(negative_evidence),
        uncertainty: sorted_unique(uncertainty),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-heterogeneity-aware-portfolio"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| HeterogeneityAwareExperimentPortfolioError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> HeterogeneityAwareExperimentPortfolioRequest {
        HeterogeneityAwareExperimentPortfolioRequest {
            objective: "validate invasion mechanism across model systems".into(),
            budget_units: 30,
            replication_reserve_fraction_milli: 200,
            min_model_systems: 2,
            min_power_milli: 100,
            max_risk_milli: 700,
            max_selected_arms: 3,
            strata: vec![
                HeterogeneityExperimentStratum {
                    stratum_id: "organoid".into(),
                    model_system: GliomaModelSystem::Organoid,
                    prior_milli: 500,
                    heterogeneity_milli: 200,
                    required: true,
                },
                HeterogeneityExperimentStratum {
                    stratum_id: "in_silico".into(),
                    model_system: GliomaModelSystem::InSilico,
                    prior_milli: 500,
                    heterogeneity_milli: 500,
                    required: true,
                },
            ],
            candidates: vec![
                HeterogeneityExperimentCandidate {
                    candidate_id: "organoid-invasion".into(),
                    arm_id: "invasion".into(),
                    stratum_id: "organoid".into(),
                    modality: GliomaModality::Imaging,
                    independence_group: "site-a".into(),
                    cost_units_per_replicate: 4,
                    max_replicates: 3,
                    expected_effect_milli: 800,
                    effect_uncertainty_milli: 100,
                    reproducibility_milli: 900,
                    risk_milli: 100,
                    available: true,
                },
                HeterogeneityExperimentCandidate {
                    candidate_id: "insilico-invasion".into(),
                    arm_id: "invasion".into(),
                    stratum_id: "in_silico".into(),
                    modality: GliomaModality::Computational,
                    independence_group: "compute-a".into(),
                    cost_units_per_replicate: 2,
                    max_replicates: 3,
                    expected_effect_milli: 650,
                    effect_uncertainty_milli: 120,
                    reproducibility_milli: 950,
                    risk_milli: 50,
                    available: true,
                },
                HeterogeneityExperimentCandidate {
                    candidate_id: "unsafe-arm".into(),
                    arm_id: "unsafe".into(),
                    stratum_id: "organoid".into(),
                    modality: GliomaModality::FunctionalPerturbation,
                    independence_group: "site-b".into(),
                    cost_units_per_replicate: 1,
                    max_replicates: 1,
                    expected_effect_milli: 900,
                    effect_uncertainty_milli: 100,
                    reproducibility_milli: 900,
                    risk_milli: 900,
                    available: true,
                },
            ],
        }
    }

    #[test]
    fn portfolio_preserves_replication_reserve_and_model_diversity() {
        let output = plan_glioma_heterogeneity_aware_experiment_portfolio(&request())
            .expect("portfolio should compile");
        assert!(output.replication_reserve_units >= 1);
        assert!(output.model_system_order.len() >= 2);
        assert!(output
            .deferred
            .iter()
            .any(|item| item.candidate_id == "unsafe-arm"));
        assert!(output.validate().is_ok());
    }

    #[test]
    fn portfolio_reports_underpowered_when_budget_cannot_cover_diversity() {
        let mut request = request();
        request.budget_units = 3;
        request.min_power_milli = 900;
        let output = plan_glioma_heterogeneity_aware_experiment_portfolio(&request)
            .expect("underpowered portfolio should remain a typed result");
        assert_eq!(
            output.disposition,
            HeterogeneityPortfolioDisposition::Underpowered
        );
        assert!(!output.uncertainty.is_empty());
    }

    #[test]
    fn portfolio_never_silently_drops_required_stratum() {
        let mut request = request();
        request
            .candidates
            .retain(|candidate| candidate.stratum_id == "organoid");
        request.min_model_systems = 1;
        request.min_power_milli = 1;
        let output = plan_glioma_heterogeneity_aware_experiment_portfolio(&request)
            .expect("required-stratum omission should remain a typed result");
        assert_eq!(
            output.disposition,
            HeterogeneityPortfolioDisposition::Underpowered
        );
        assert!(output
            .uncertainty
            .iter()
            .any(|item| item.contains("in_silico")));
    }
}
