//! Reliability- and budget-aware multimodal portfolio selection for preclinical glioma research.
//!
//! Autonomous research should not request every assay. This feature solves a bounded set-cover
//! problem over endpoint evidence dimensions, balancing coverage, reliability, redundancy,
//! throughput, and cost. It returns a typed plan plus explicit uncovered dimensions and budget
//! failures; it does not execute an assay or infer an unmeasured result.

use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P03-F07";
pub const OUTPUT_SCHEMA: &str = "GliomaMultimodalPortfolioPlan1@1";
pub const MAX_DIMENSIONS: usize = 256;
pub const MAX_MODALITIES: usize = 64;
pub const MAX_ALTERNATIVES: usize = 16;
pub const BEAM_WIDTH: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModalityCapability {
    pub modality: GliomaModality,
    pub dimension_order: Vec<String>,
    pub reliability_milli: u16,
    pub cost_units: u32,
    pub throughput_units: u32,
    pub required_for_endpoint: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModalityPortfolioRequest {
    pub objective: String,
    pub endpoint_id: String,
    pub model_system: GliomaModelSystem,
    pub required_dimension_order: Vec<String>,
    pub capabilities: Vec<ModalityCapability>,
    pub max_selected_modalities: usize,
    pub budget_units: u32,
    pub min_dimension_coverage_milli: u16,
    pub min_reliability_milli: u16,
    pub min_redundancy_milli: u16,
    pub max_alternatives: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModalityPortfolioDisposition {
    Ready,
    Conditional,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModalityPortfolioAlternative {
    pub modality_order: Vec<GliomaModality>,
    pub covered_dimension_order: Vec<String>,
    pub uncovered_dimension_order: Vec<String>,
    pub cost_units: u32,
    pub throughput_units: u32,
    pub minimum_reliability_milli: u16,
    pub dimension_coverage_milli: u16,
    pub redundancy_milli: u16,
    pub score_milli: i64,
    pub feasible: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModalityPortfolioPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub endpoint_id: String,
    pub model_system: GliomaModelSystem,
    pub required_dimension_order: Vec<String>,
    pub selected_modality_order: Vec<GliomaModality>,
    pub covered_dimension_order: Vec<String>,
    pub uncovered_dimension_order: Vec<String>,
    pub selected_cost_units: u32,
    pub selected_throughput_units: u32,
    pub selected_minimum_reliability_milli: u16,
    pub selected_dimension_coverage_milli: u16,
    pub selected_redundancy_milli: u16,
    pub selected_score_milli: i64,
    pub alternatives: Vec<ModalityPortfolioAlternative>,
    pub acquisition_order: Vec<GliomaModality>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: ModalityPortfolioDisposition,
    pub next_action: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ModalityPortfolioError {
    #[error("modality portfolio request is invalid: {0}")]
    InvalidRequest(String),
    #[error("modality portfolio input is invalid: {0}")]
    InvalidInput(String),
    #[error("modality portfolio output is invalid: {0}")]
    InvalidOutput(String),
    #[error("modality portfolio digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn fraction_milli(numerator: u64, denominator: u64) -> u16 {
    if denominator == 0 {
        return 0;
    }
    ((numerator.saturating_mul(1_000) / denominator).min(1_000)) as u16
}

fn digest_input(output: &ModalityPortfolioPlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "endpoint_id": output.endpoint_id,
        "model_system": output.model_system,
        "required_dimension_order": output.required_dimension_order,
        "selected_modality_order": output.selected_modality_order,
        "covered_dimension_order": output.covered_dimension_order,
        "uncovered_dimension_order": output.uncovered_dimension_order,
        "selected_cost_units": output.selected_cost_units,
        "selected_throughput_units": output.selected_throughput_units,
        "selected_minimum_reliability_milli": output.selected_minimum_reliability_milli,
        "selected_dimension_coverage_milli": output.selected_dimension_coverage_milli,
        "selected_redundancy_milli": output.selected_redundancy_milli,
        "selected_score_milli": output.selected_score_milli,
        "alternatives": output.alternatives,
        "acquisition_order": output.acquisition_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
        "next_action": output.next_action,
    })
}

fn validate_request(request: &ModalityPortfolioRequest) -> Result<(), ModalityPortfolioError> {
    if request.objective.trim().is_empty()
        || request.endpoint_id.trim().is_empty()
        || request.required_dimension_order.is_empty()
        || request.required_dimension_order.len() > MAX_DIMENSIONS
        || request.capabilities.is_empty()
        || request.capabilities.len() > MAX_MODALITIES
        || request.max_selected_modalities == 0
        || request.max_selected_modalities > request.capabilities.len()
        || request.budget_units == 0
        || request.min_dimension_coverage_milli > 1_000
        || request.min_reliability_milli > 1_000
        || request.min_redundancy_milli > 1_000
        || request.max_alternatives > MAX_ALTERNATIVES
    {
        return Err(ModalityPortfolioError::InvalidRequest(
            "endpoint, dimensions, bounded capabilities, selection/budget limits, and portfolio gates are required".into(),
        ));
    }
    if !canonical(&request.required_dimension_order)
        || request
            .required_dimension_order
            .iter()
            .any(|dimension| dimension.trim().is_empty())
    {
        return Err(ModalityPortfolioError::InvalidRequest(
            "required dimensions must be non-empty and canonical".into(),
        ));
    }
    let dimensions = request
        .required_dimension_order
        .iter()
        .collect::<BTreeSet<_>>();
    let mut modalities = BTreeSet::new();
    for capability in &request.capabilities {
        if !modalities.insert(capability.modality)
            || capability.dimension_order.is_empty()
            || !canonical(&capability.dimension_order)
            || capability
                .dimension_order
                .iter()
                .any(|dimension| !dimensions.contains(dimension) || dimension.trim().is_empty())
            || capability.reliability_milli > 1_000
            || capability.cost_units == 0
            || capability.throughput_units == 0
        {
            return Err(ModalityPortfolioError::InvalidInput(
                "capabilities require unique modalities, canonical endpoint dimensions, bounded reliability, and positive cost/throughput".into(),
            ));
        }
    }
    Ok(())
}

fn validate_output(output: &ModalityPortfolioPlan) -> Result<(), ModalityPortfolioError> {
    if output.feature_id != FEATURE_ID
        || output.output_schema != OUTPUT_SCHEMA
        || output.objective.trim().is_empty()
        || output.endpoint_id.trim().is_empty()
        || !canonical(&output.required_dimension_order)
        || !canonical(&output.selected_modality_order)
        || !canonical(&output.covered_dimension_order)
        || !canonical(&output.uncovered_dimension_order)
        || !canonical(&output.negative_evidence)
        || !canonical(&output.uncertainty)
        || output.selected_dimension_coverage_milli > 1_000
        || output.selected_redundancy_milli > 1_000
        || output.selected_minimum_reliability_milli > 1_000
        || output.alternatives.len() > MAX_ALTERNATIVES
        || output.alternatives.windows(2).any(|pair| {
            pair[0].score_milli < pair[1].score_milli
                || (pair[0].score_milli == pair[1].score_milli
                    && pair[0].modality_order > pair[1].modality_order)
        })
        || output.alternatives.iter().any(|alternative| {
            !canonical(&alternative.modality_order)
                || !canonical(&alternative.covered_dimension_order)
                || !canonical(&alternative.uncovered_dimension_order)
                || alternative.dimension_coverage_milli > 1_000
                || alternative.redundancy_milli > 1_000
                || alternative.minimum_reliability_milli > 1_000
        })
    {
        return Err(ModalityPortfolioError::InvalidOutput(
            "identity, ordering, bounded metrics, or alternative invariants are invalid".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(output))
        .map_err(|error| ModalityPortfolioError::Digest(error.to_string()))?;
    if expected != output.digest {
        return Err(ModalityPortfolioError::InvalidOutput(
            "digest is not bound to modality-portfolio output".into(),
        ));
    }
    Ok(())
}

impl ModalityPortfolioPlan {
    pub fn validate(&self) -> Result<(), ModalityPortfolioError> {
        validate_output(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SearchState {
    selected: Vec<GliomaModality>,
    covered: BTreeSet<String>,
    coverage_counts: BTreeMap<String, u16>,
    cost_units: u32,
    throughput_units: u32,
    minimum_reliability_milli: u16,
}

fn state_key(state: &SearchState) -> String {
    state
        .selected
        .iter()
        .map(|modality| format!("{modality:?}"))
        .collect::<Vec<_>>()
        .join("|")
}

fn state_metrics(
    state: &SearchState,
    required_dimensions: &[String],
    required_modalities: &BTreeSet<GliomaModality>,
) -> (u16, u16, i64, Vec<String>, Vec<String>, bool) {
    let covered_dimension_order = state.covered.iter().cloned().collect::<Vec<_>>();
    let uncovered_dimension_order = required_dimensions
        .iter()
        .filter(|dimension| !state.covered.contains(*dimension))
        .cloned()
        .collect::<Vec<_>>();
    let coverage = fraction_milli(
        covered_dimension_order.len() as u64,
        required_dimensions.len() as u64,
    );
    let redundant = state
        .coverage_counts
        .values()
        .filter(|count| **count >= 2)
        .count();
    let redundancy = fraction_milli(redundant as u64, required_dimensions.len() as u64);
    let required_ok = required_modalities.is_subset(&state.selected.iter().copied().collect());
    let score = i64::from(coverage).saturating_mul(500)
        + i64::from(state.minimum_reliability_milli).saturating_mul(300)
        + i64::from(redundancy).saturating_mul(100)
        + i64::from(state.throughput_units.min(1_000)).saturating_mul(20)
        - i64::from(state.cost_units).saturating_mul(10);
    (
        coverage,
        redundancy,
        score,
        covered_dimension_order,
        uncovered_dimension_order,
        required_ok,
    )
}

/// Select a bounded, reliability-aware multimodal portfolio for a local preclinical glioma
/// endpoint.
pub fn plan_glioma_multimodal_portfolio(
    request: &ModalityPortfolioRequest,
) -> Result<ModalityPortfolioPlan, ModalityPortfolioError> {
    validate_request(request)?;
    let required_modalities = request
        .capabilities
        .iter()
        .filter(|capability| capability.required_for_endpoint)
        .map(|capability| capability.modality)
        .collect::<BTreeSet<_>>();
    let mut states = vec![SearchState {
        selected: Vec::new(),
        covered: BTreeSet::new(),
        coverage_counts: BTreeMap::new(),
        cost_units: 0,
        throughput_units: 0,
        minimum_reliability_milli: 1_000,
    }];
    for capability in &request.capabilities {
        let mut next = states.clone();
        for state in &states {
            if state.selected.len() >= request.max_selected_modalities
                || state.cost_units.saturating_add(capability.cost_units) > request.budget_units
            {
                continue;
            }
            let mut selected = state.selected.clone();
            selected.push(capability.modality);
            selected.sort();
            let mut covered = state.covered.clone();
            let mut coverage_counts = state.coverage_counts.clone();
            for dimension in &capability.dimension_order {
                covered.insert(dimension.clone());
                *coverage_counts.entry(dimension.clone()).or_default() += 1;
            }
            next.push(SearchState {
                selected,
                covered,
                coverage_counts,
                cost_units: state.cost_units.saturating_add(capability.cost_units),
                throughput_units: state
                    .throughput_units
                    .saturating_add(capability.throughput_units),
                minimum_reliability_milli: state
                    .minimum_reliability_milli
                    .min(capability.reliability_milli),
            });
        }
        let mut dedup = BTreeMap::new();
        for state in next {
            dedup.insert(state_key(&state), state);
        }
        let mut ranked = dedup.into_values().collect::<Vec<_>>();
        ranked.sort_by(|left, right| {
            let left_metrics = state_metrics(
                left,
                &request.required_dimension_order,
                &required_modalities,
            );
            let right_metrics = state_metrics(
                right,
                &request.required_dimension_order,
                &required_modalities,
            );
            right_metrics
                .2
                .cmp(&left_metrics.2)
                .then_with(|| left.cost_units.cmp(&right.cost_units))
                .then_with(|| left.selected.cmp(&right.selected))
        });
        ranked.truncate(BEAM_WIDTH);
        states = ranked;
    }

    let mut evaluated = states
        .into_iter()
        .map(|state| {
            let (coverage, redundancy, score, covered, uncovered, required_ok) = state_metrics(
                &state,
                &request.required_dimension_order,
                &required_modalities,
            );
            let feasible = required_ok
                && coverage >= request.min_dimension_coverage_milli
                && state.minimum_reliability_milli >= request.min_reliability_milli
                && redundancy >= request.min_redundancy_milli;
            (
                state, feasible, coverage, redundancy, score, covered, uncovered,
            )
        })
        .collect::<Vec<_>>();
    evaluated.sort_by(|left, right| {
        right
            .1
            .cmp(&left.1)
            .then_with(|| right.4.cmp(&left.4))
            .then_with(|| left.0.cost_units.cmp(&right.0.cost_units))
            .then_with(|| left.0.selected.cmp(&right.0.selected))
    });
    let (selected_state, selected_feasible, coverage, redundancy, score, covered, uncovered) =
        evaluated.first().cloned().ok_or_else(|| {
            ModalityPortfolioError::InvalidOutput("portfolio search returned no state".into())
        })?;
    let selected_minimum_reliability_milli = if selected_state.selected.is_empty() {
        0
    } else {
        selected_state.minimum_reliability_milli
    };
    let alternatives = evaluated
        .iter()
        .skip(1)
        .take(request.max_alternatives)
        .map(
            |(state, feasible, coverage, redundancy, score, covered, uncovered)| {
                ModalityPortfolioAlternative {
                    modality_order: state.selected.clone(),
                    covered_dimension_order: covered.clone(),
                    uncovered_dimension_order: uncovered.clone(),
                    cost_units: state.cost_units,
                    throughput_units: state.throughput_units,
                    minimum_reliability_milli: if state.selected.is_empty() {
                        0
                    } else {
                        state.minimum_reliability_milli
                    },
                    dimension_coverage_milli: *coverage,
                    redundancy_milli: *redundancy,
                    score_milli: *score,
                    feasible: *feasible,
                }
            },
        )
        .collect::<Vec<_>>();

    let mut acquisition_order = request
        .capabilities
        .iter()
        .filter(|capability| !selected_state.selected.contains(&capability.modality))
        .map(|capability| capability.modality)
        .collect::<Vec<_>>();
    acquisition_order.sort();
    acquisition_order.dedup();
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    if !uncovered.is_empty() {
        for dimension in &uncovered {
            negative_evidence.insert(format!("uncovered-dimension:{dimension}"));
        }
    }
    if selected_state.cost_units >= request.budget_units && !selected_feasible {
        uncertainty.insert("budget-boundary-reached-before-all-gates".into());
    }
    if selected_minimum_reliability_milli < request.min_reliability_milli {
        negative_evidence.insert(format!(
            "minimum-reliability-below-gate={selected_minimum_reliability_milli}"
        ));
    }
    if redundancy < request.min_redundancy_milli {
        uncertainty.insert(format!("redundancy-below-gate={redundancy}"));
    }
    let disposition = if selected_feasible {
        ModalityPortfolioDisposition::Ready
    } else if selected_state.selected.is_empty()
        || !required_modalities.is_subset(&selected_state.selected.iter().copied().collect())
    {
        ModalityPortfolioDisposition::Blocked
    } else {
        ModalityPortfolioDisposition::Conditional
    };
    let next_action = match disposition {
        ModalityPortfolioDisposition::Ready => {
            "selected modality portfolio is ready for downstream QC and experiment planning".into()
        }
        ModalityPortfolioDisposition::Conditional => {
            "resolve uncovered dimensions, reliability, or redundancy debt before high-confidence planning".into()
        }
        ModalityPortfolioDisposition::Blocked => {
            "expand budget or capability registry; no admissible modality portfolio was found".into()
        }
        ModalityPortfolioDisposition::Unresolved => {
            "review endpoint dimensions and modality capability evidence before planning".into()
        }
    };
    let mut output = ModalityPortfolioPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        endpoint_id: request.endpoint_id.clone(),
        model_system: request.model_system,
        required_dimension_order: request.required_dimension_order.clone(),
        selected_modality_order: selected_state.selected,
        covered_dimension_order: covered,
        uncovered_dimension_order: uncovered,
        selected_cost_units: selected_state.cost_units,
        selected_throughput_units: selected_state.throughput_units,
        selected_minimum_reliability_milli,
        selected_dimension_coverage_milli: coverage,
        selected_redundancy_milli: redundancy,
        selected_score_milli: score,
        alternatives,
        acquisition_order,
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        next_action,
        digest: ContentHash::of_bytes(b"unsealed-glioma-multimodal-portfolio"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ModalityPortfolioError::Digest(error.to_string()))?;
    validate_output(&output)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(budget_units: u32) -> ModalityPortfolioRequest {
        ModalityPortfolioRequest {
            objective: "select an invasion-mechanism multimodal portfolio".into(),
            endpoint_id: "invasion".into(),
            model_system: GliomaModelSystem::Organoid,
            required_dimension_order: vec!["function".into(), "spatial".into(), "state".into()],
            capabilities: vec![
                ModalityCapability {
                    modality: GliomaModality::Genomics,
                    dimension_order: vec!["state".into()],
                    reliability_milli: 900,
                    cost_units: 2,
                    throughput_units: 800,
                    required_for_endpoint: true,
                },
                ModalityCapability {
                    modality: GliomaModality::Imaging,
                    dimension_order: vec!["spatial".into(), "state".into()],
                    reliability_milli: 850,
                    cost_units: 3,
                    throughput_units: 700,
                    required_for_endpoint: false,
                },
                ModalityCapability {
                    modality: GliomaModality::FunctionalPerturbation,
                    dimension_order: vec!["function".into()],
                    reliability_milli: 800,
                    cost_units: 4,
                    throughput_units: 500,
                    required_for_endpoint: false,
                },
            ],
            max_selected_modalities: 3,
            budget_units,
            min_dimension_coverage_milli: 1_000,
            min_reliability_milli: 800,
            min_redundancy_milli: 0,
            max_alternatives: 4,
        }
    }

    #[test]
    fn portfolio_optimizer_selects_complete_reliable_set() {
        let output = plan_glioma_multimodal_portfolio(&request(9)).unwrap();
        assert_eq!(output.disposition, ModalityPortfolioDisposition::Ready);
        assert_eq!(
            output.selected_modality_order,
            vec![
                GliomaModality::Genomics,
                GliomaModality::Imaging,
                GliomaModality::FunctionalPerturbation
            ]
        );
        assert_eq!(output.selected_dimension_coverage_milli, 1_000);
        output.validate().unwrap();
    }

    #[test]
    fn portfolio_optimizer_preserves_budget_block() {
        let output = plan_glioma_multimodal_portfolio(&request(5)).unwrap();
        assert_eq!(
            output.disposition,
            ModalityPortfolioDisposition::Conditional
        );
        assert!(!output.uncovered_dimension_order.is_empty());
        assert!(output
            .negative_evidence
            .iter()
            .any(|item| item.contains("uncovered-dimension")));
    }
}
