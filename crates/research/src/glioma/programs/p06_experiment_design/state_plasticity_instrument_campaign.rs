//! Closed-loop cell-state plasticity campaigns connecting glioma analysis to real assay execution.
//!
//! This is a P06-F19 workflow composition: P10 supplies descriptive longitudinal state-transition
//! evidence, P06 allocates the next state-stratified assay, P07 screens its protocol, and P08
//! authorizes and dispatches the physical operation. State-transition priorities guide sampling;
//! they are neither causal effects nor treatment recommendations.

use super::lineage_acquisition_design::{
    LineagePropagationAcquisitionPolicy, LineagePropagationAcquisitionTarget,
    LineagePropagationAssayResponseModel,
};
use super::simulation_gated_campaign::{
    execute_glioma_simulation_gated_assay_campaign,
    execute_glioma_simulation_gated_assay_campaign_with_lineage_acquisition,
    GliomaInstrumentOutcomeInterpreter, SimulationGatedAssayCampaignError,
    SimulationGatedAssayCampaignInputs, SimulationGatedAssayCampaignRun, SimulationGatedAssayRoute,
};
use super::state_stratified_campaign::{
    GliomaResearchStratum, StratifiedAssayCandidate, StratifiedAssayObservation,
};
use super::transition_guided_campaign::{
    plan_glioma_transition_guided_state_priorities, TransitionGuidedStateCampaignError,
    TransitionGuidedStateCampaignRequest, TransitionGuidedStatePriority,
};
use crate::glioma::programs::p08_instrument_robotics::InstrumentProtocolGateway;
use crate::glioma::programs::p10_interpretation_replication::lineage_propagation::{
    analyze_glioma_lineage_propagation, CoefficientDisposition, LineagePropagationAnalysis,
    LineagePropagationDisposition, LineagePropagationError, LineagePropagationRequest,
    LineagePropagationSnapshot, PpmInterval,
};
use crate::glioma::programs::p10_interpretation_replication::lineage_response_decomposition::{
    analyze_glioma_lineage_response_decomposition, LineageResponseDecomposition,
    LineageResponseDecompositionError, LineageResponseDecompositionRequest,
    LineageResponseFollowUpFocus,
};
use crate::glioma::programs::p10_interpretation_replication::state_transition::{
    analyze_glioma_state_transitions, StateTransitionAnalysis, StateTransitionObservation,
    StateTransitionRequest,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = super::adaptive_information_campaign::FEATURE_ID;
pub const OUTPUT_SCHEMA: &str = "GliomaStatePlasticityInstrumentCampaign1@12";
const SCORE_SCALE: u16 = 1_000;

/// All choices that define one bounded longitudinal-to-instrument glioma research run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaStatePlasticityInstrumentRequest {
    pub transition_analysis: StateTransitionRequest,
    pub assay_campaign: TransitionGuidedStateCampaignRequest,
}

/// Local data and eligible routes consumed by one autonomous campaign run.
#[derive(Debug, Clone, Copy)]
pub struct GliomaStatePlasticityInstrumentInputs<'a> {
    pub longitudinal_observations: &'a [StateTransitionObservation],
    pub strata: &'a [GliomaResearchStratum],
    pub candidates: &'a [StratifiedAssayCandidate],
    pub initial_observations: &'a [StratifiedAssayObservation],
    pub routes: &'a [SimulationGatedAssayRoute],
}

/// Lineage-propagation evidence and posterior state used to value follow-up assays.
#[derive(Debug, Clone, Copy)]
pub struct LineagePropagationGuidedCampaignInputs<'a> {
    pub request: &'a LineagePropagationRequest,
    pub snapshots: &'a [LineagePropagationSnapshot],
    pub response_models: &'a [LineagePropagationAssayResponseModel],
    pub posterior_particle_weights_million: &'a [u32],
}

/// State analysis, explicit sampling rationale, and the complete P06/P07/P08 campaign outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaStatePlasticityInstrumentRun {
    pub feature_id: String,
    pub output_schema: String,
    pub transition_analysis: StateTransitionAnalysis,
    pub priorities: Vec<TransitionGuidedStatePriority>,
    /// Present only for the lineage-propagation-guided entry point.
    pub lineage_propagation_analysis: Option<LineagePropagationAnalysis>,
    /// Present when the P10-F04 decomposition explicitly guided P06 follow-up allocation.
    pub lineage_response_decomposition: Option<LineageResponseDecomposition>,
    /// Auditable decomposition of the transition-priority weight adjustment.
    pub lineage_propagation_priorities: Vec<LineagePropagationStatePriority>,
    pub instrument_campaign: SimulationGatedAssayCampaignRun,
    pub digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineagePropagationStatePriority {
    pub state_id: String,
    pub transition_priority_milli: u16,
    pub propagation_uncertainty_milli: u16,
    pub batch_sensitivity_milli: u16,
    pub batch_robustness_gap_milli: u16,
    pub batch_followup_attention_milli: u16,
    /// Mean P10-F04 follow-up attention across destinations from this source state.
    pub response_followup_attention_milli: Option<u16>,
    /// Number of destination contrasts whose lineage identifiability calls for resolution.
    pub unresolved_response_pair_count: Option<u16>,
    pub applied_priority_milli: u16,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GliomaStatePlasticityInstrumentError {
    #[error("glioma state-plasticity instrument request is invalid: {0}")]
    InvalidInput(String),
    #[error("glioma state-transition analysis failed: {0}")]
    TransitionAnalysis(String),
    #[error("glioma state assay prioritization failed: {0}")]
    Prioritization(String),
    #[error("glioma lineage-propagation analysis failed: {0}")]
    LineagePropagationAnalysis(String),
    #[error("glioma lineage-response decomposition failed: {0}")]
    LineageResponseAnalysis(String),
    #[error("glioma simulation-gated instrument campaign failed: {0}")]
    InstrumentCampaign(String),
    #[error("glioma state-plasticity instrument output is invalid: {0}")]
    InvalidOutput(String),
    #[error("glioma state-plasticity instrument digest failed: {0}")]
    Digest(String),
}

fn digest_input(run: &GliomaStatePlasticityInstrumentRun) -> serde_json::Value {
    serde_json::json!({
        "feature_id": run.feature_id,
        "output_schema": run.output_schema,
        "transition_analysis_digest": run.transition_analysis.digest,
        "priorities": run.priorities,
        "lineage_propagation_analysis_digest": run.lineage_propagation_analysis.as_ref().map(|analysis| &analysis.digest),
        "lineage_response_decomposition_digest": run.lineage_response_decomposition.as_ref().map(|analysis| &analysis.digest),
        "lineage_propagation_priorities": run.lineage_propagation_priorities,
        "instrument_campaign_digest": run.instrument_campaign.digest,
    })
}

fn reprioritize_strata(
    strata: &[GliomaResearchStratum],
    priorities: &[TransitionGuidedStatePriority],
) -> Result<Vec<GliomaResearchStratum>, GliomaStatePlasticityInstrumentError> {
    let weights = priorities
        .iter()
        .map(|priority| (priority.state_id.as_str(), priority.adjusted_priority_milli))
        .collect::<BTreeMap<_, _>>();
    if weights.len() != priorities.len()
        || weights.len() != strata.len()
        || priorities
            .iter()
            .map(|priority| u32::from(priority.adjusted_priority_milli))
            .sum::<u32>()
            != u32::from(SCORE_SCALE)
    {
        return Err(GliomaStatePlasticityInstrumentError::InvalidInput(
            "transition priorities must uniquely and exactly cover the declared assay strata"
                .into(),
        ));
    }

    strata
        .iter()
        .cloned()
        .map(|mut stratum| {
            let Some(weight) = weights.get(stratum.stratum_id.as_str()) else {
                return Err(GliomaStatePlasticityInstrumentError::InvalidInput(
                    "a declared assay stratum has no transition-priority row".into(),
                ));
            };
            stratum.priority_weight_milli = *weight;
            Ok(stratum)
        })
        .collect()
}

fn normalized_interval_uncertainty_milli(interval: PpmInterval, estimate: u64, floor: u64) -> u16 {
    let width = (i128::from(interval.upper) - i128::from(interval.lower)).max(0) as u128;
    let scale = u128::from(estimate.max(floor).max(1));
    ((width.saturating_mul(u128::from(SCORE_SCALE)) / scale).min(u128::from(SCORE_SCALE))) as u16
}

fn source_state_uncertainty_milli(
    state_id: &str,
    state_count: usize,
    control: &crate::glioma::programs::p10_interpretation_replication::lineage_propagation::LineagePropagationOperator,
    treatment: &crate::glioma::programs::p10_interpretation_replication::lineage_propagation::LineagePropagationOperator,
) -> Result<u16, GliomaStatePlasticityInstrumentError> {
    let mut components = Vec::with_capacity(state_count * 2 + 2);
    for operator in [control, treatment] {
        let coefficients = operator
            .coefficients
            .iter()
            .filter(|coefficient| coefficient.from_state == state_id)
            .collect::<Vec<_>>();
        let summary = operator
            .source_state_summaries
            .iter()
            .find(|summary| summary.from_state == state_id)
            .ok_or_else(|| {
                GliomaStatePlasticityInstrumentError::InvalidInput(format!(
                    "lineage propagation has no source-state summary for {state_id}"
                ))
            })?;
        if coefficients.len() != state_count
            || coefficients
                .iter()
                .any(|coefficient| coefficient.disposition != CoefficientDisposition::Estimable)
            || summary.destination_composition.len() != state_count
        {
            return Err(GliomaStatePlasticityInstrumentError::InvalidInput(format!(
                "lineage propagation uncertainty is not estimable for {state_id}"
            )));
        }
        components.extend(coefficients.iter().map(|coefficient| {
            normalized_interval_uncertainty_milli(
                coefficient.bootstrap_interval_ppm,
                coefficient.coefficient_ppm,
                1_000_000,
            )
        }));
        components.push(normalized_interval_uncertainty_milli(
            summary.yield_interval_ppm,
            summary.effective_descendant_yield_ppm,
            1_000_000,
        ));
        let composition_floor = (1_000_000_u64 / state_count as u64).max(1);
        components.extend(summary.destination_composition.iter().map(|share| {
            normalized_interval_uncertainty_milli(
                share.bootstrap_interval_ppm,
                u64::from(share.share_ppm),
                composition_floor,
            )
        }));
    }
    if components.is_empty() {
        return Err(GliomaStatePlasticityInstrumentError::InvalidInput(
            "lineage propagation uncertainty contains no estimable components".into(),
        ));
    }
    Ok((components
        .iter()
        .map(|score| u32::from(*score))
        .sum::<u32>()
        / components.len() as u32) as u16)
}

fn source_state_batch_attention_milli(
    state_id: &str,
    propagation: &LineagePropagationAnalysis,
) -> Result<(u16, u16, u16), GliomaStatePlasticityInstrumentError> {
    let sensitivity = propagation
        .batch_sensitivity
        .iter()
        .find(|row| row.from_state == state_id)
        .ok_or_else(|| {
            GliomaStatePlasticityInstrumentError::InvalidInput(format!(
                "lineage propagation has no assay-batch sensitivity row for {state_id}"
            ))
        })?;
    let deviation_attention = (u128::from(sensitivity.maximum_leave_one_batch_out_deviation_ppm)
        * u128::from(SCORE_SCALE)
        / u128::from(propagation.minimum_effect_ppm.max(1)))
    .min(u128::from(SCORE_SCALE)) as u16;
    let batch_sensitivity_milli = deviation_attention;
    let batch_robustness_gap_milli = sensitivity.robustness_gap_milli.min(SCORE_SCALE);
    let followup_attention = batch_sensitivity_milli.max(batch_robustness_gap_milli);
    Ok((
        batch_sensitivity_milli,
        batch_robustness_gap_milli,
        followup_attention,
    ))
}

fn source_response_attention_milli(
    state_id: &str,
    response: &LineageResponseDecomposition,
) -> Result<(u16, u16), GliomaStatePlasticityInstrumentError> {
    let rows = response
        .decompositions
        .iter()
        .filter(|row| row.from_state == state_id)
        .collect::<Vec<_>>();
    if rows.len() != response.state_order.len() || rows.is_empty() {
        return Err(GliomaStatePlasticityInstrumentError::InvalidInput(format!(
            "P10-F04 must contain one response row for every destination from source state {state_id}"
        )));
    }

    let mut attention_sum = 0_u32;
    let mut unresolved_pairs = 0_u16;
    for row in &rows {
        let attention = match row.follow_up_focus {
            LineageResponseFollowUpFocus::NoTargetedFollowUp => 0,
            LineageResponseFollowUpFocus::ValidateNetYield
            | LineageResponseFollowUpFocus::ValidateStateComposition
            | LineageResponseFollowUpFocus::ValidateBothComponents => SCORE_SCALE,
            LineageResponseFollowUpFocus::IncreaseIndependentReplication => 750,
            LineageResponseFollowUpFocus::ResolveLineageIdentifiability => {
                unresolved_pairs = unresolved_pairs.saturating_add(1);
                SCORE_SCALE
            }
        };
        attention_sum += u32::from(attention);
    }
    Ok(((attention_sum / rows.len() as u32) as u16, unresolved_pairs))
}

fn adjust_transition_priorities_with_response(
    priorities: &mut [TransitionGuidedStatePriority],
    propagation: &LineagePropagationAnalysis,
    response: Option<&LineageResponseDecomposition>,
) -> Result<Vec<LineagePropagationStatePriority>, GliomaStatePlasticityInstrumentError> {
    if priorities.is_empty()
        || priorities
            .iter()
            .map(|row| row.state_id.as_str())
            .collect::<BTreeSet<_>>()
            .len()
            != priorities.len()
        || priorities
            .iter()
            .any(|row| row.adjusted_priority_milli == 0)
        || priorities
            .iter()
            .map(|row| u32::from(row.adjusted_priority_milli))
            .sum::<u32>()
            != u32::from(SCORE_SCALE)
    {
        return Err(GliomaStatePlasticityInstrumentError::InvalidInput(
            "base transition priorities must be unique, positive, and sum to 1000".into(),
        ));
    }
    let state_count = priorities.len();
    let mut weighted = Vec::with_capacity(state_count);
    for priority in priorities.iter() {
        let interval_uncertainty = source_state_uncertainty_milli(
            &priority.state_id,
            state_count,
            &propagation.control,
            &propagation.treatment,
        )?;
        let (batch_sensitivity, batch_gap, batch_followup) =
            source_state_batch_attention_milli(&priority.state_id, propagation)?;
        let response_attention = match response {
            Some(response) => Some(source_response_attention_milli(
                &priority.state_id,
                response,
            )?),
            None => None,
        };
        let uncertainty = match response_attention {
            Some((response_followup, _)) => interval_uncertainty
                .max(batch_followup)
                .max(response_followup),
            None => interval_uncertainty.max(batch_followup),
        };
        let weight = u128::from(priority.adjusted_priority_milli)
            * (u128::from(SCORE_SCALE) + u128::from(uncertainty));
        weighted.push((
            priority.state_id.clone(),
            priority.adjusted_priority_milli,
            uncertainty,
            batch_sensitivity,
            batch_gap,
            batch_followup,
            response_attention.map(|(attention, _)| attention),
            response_attention.map(|(_, unresolved)| unresolved),
            weight,
        ));
    }

    let total_weight = weighted.iter().map(|row| row.8).sum::<u128>();
    if total_weight == 0 || state_count > usize::from(SCORE_SCALE) {
        return Err(GliomaStatePlasticityInstrumentError::InvalidInput(
            "lineage-guided priority weights cannot be normalized".into(),
        ));
    }
    let distributable = u128::from(SCORE_SCALE) - state_count as u128;
    let mut assigned = 0_u128;
    let mut allocations = weighted
        .iter()
        .map(|row| {
            let numerator = row.8 * distributable;
            let quotient = numerator / total_weight;
            assigned += quotient;
            (
                row.0.clone(),
                row.1,
                row.2,
                row.3,
                row.4,
                row.5,
                row.6,
                row.7,
                quotient + 1,
                numerator % total_weight,
            )
        })
        .collect::<Vec<_>>();
    let remainder_count = (distributable - assigned) as usize;
    let mut remainder_order = (0..allocations.len()).collect::<Vec<_>>();
    remainder_order.sort_by(|left, right| {
        allocations[*right]
            .9
            .cmp(&allocations[*left].9)
            .then_with(|| allocations[*left].0.cmp(&allocations[*right].0))
    });
    for index in remainder_order.into_iter().take(remainder_count) {
        allocations[index].8 += 1;
    }

    let mut audit = Vec::with_capacity(state_count);
    for (priority, allocation) in priorities.iter_mut().zip(allocations) {
        let applied = u16::try_from(allocation.8).map_err(|_| {
            GliomaStatePlasticityInstrumentError::InvalidOutput(
                "normalized lineage-guided priority exceeds the fixed score scale".into(),
            )
        })?;
        priority.adjusted_priority_milli = applied;
        audit.push(LineagePropagationStatePriority {
            state_id: allocation.0,
            transition_priority_milli: allocation.1,
            propagation_uncertainty_milli: allocation.2,
            batch_sensitivity_milli: allocation.3,
            batch_robustness_gap_milli: allocation.4,
            batch_followup_attention_milli: allocation.5,
            response_followup_attention_milli: allocation.6,
            unresolved_response_pair_count: allocation.7,
            applied_priority_milli: applied,
        });
    }
    audit.sort_by(|left, right| left.state_id.cmp(&right.state_id));
    if audit
        .iter()
        .map(|row| u32::from(row.applied_priority_milli))
        .sum::<u32>()
        != u32::from(SCORE_SCALE)
    {
        return Err(GliomaStatePlasticityInstrumentError::InvalidOutput(
            "normalized lineage-guided priorities do not sum to 1000".into(),
        ));
    }
    Ok(audit)
}

fn validate_lineage_propagation_for_request(
    propagation: &LineagePropagationAnalysis,
    model_system: &crate::glioma_engine::GliomaModelSystem,
    control_arm: &str,
    treatment_arm: &str,
    state_order: &[String],
) -> Result<(), GliomaStatePlasticityInstrumentError> {
    propagation
        .validate()
        .map_err(|error: LineagePropagationError| {
            GliomaStatePlasticityInstrumentError::LineagePropagationAnalysis(error.to_string())
        })?;
    if &propagation.model_system != model_system
        || propagation.control_arm != control_arm
        || propagation.treatment_arm != treatment_arm
        || propagation.state_order != state_order
    {
        return Err(GliomaStatePlasticityInstrumentError::InvalidInput(
            "lineage propagation and state-transition analyses must match model, arms, and ordered states".into(),
        ));
    }
    if propagation.disposition != LineagePropagationDisposition::Qualified
        || !qualified_operator_is_safe(&propagation.control, &propagation.state_order)
        || !qualified_operator_is_safe(&propagation.treatment, &propagation.state_order)
        || propagation.held_out_forecasts.iter().any(|forecast| {
            forecast.mean_composition_error_ppm > propagation.max_prediction_error_ppm
                || forecast.mean_abundance_error_ppm > propagation.max_prediction_error_ppm
        })
    {
        return Err(GliomaStatePlasticityInstrumentError::InvalidInput(
            "lineage-propagation fit is not qualified for autonomous assay reprioritization".into(),
        ));
    }
    Ok(())
}

fn qualified_operator_is_safe(
    operator: &crate::glioma::programs::p10_interpretation_replication::lineage_propagation::LineagePropagationOperator,
    state_order: &[String],
) -> bool {
    let states = state_order.len();
    operator.converged
        && usize::from(operator.source_design_rank) == states
        && operator.boundary_coefficient_count == 0
        && operator.coefficients.len() == states * states
        && operator.coefficients.iter().all(|coefficient| {
            coefficient.disposition == CoefficientDisposition::Estimable
                && coefficient.bootstrap_interval_ppm.lower >= 0
                && coefficient.bootstrap_interval_ppm.upper
                    >= coefficient.bootstrap_interval_ppm.lower
        })
        && operator.source_state_summaries.len() == states
        && operator
            .source_state_summaries
            .iter()
            .enumerate()
            .all(|(index, summary)| {
                summary.from_state == state_order[index]
                    && summary.yield_interval_ppm.lower >= 0
                    && summary.yield_interval_ppm.upper >= summary.yield_interval_ppm.lower
                    && summary.destination_composition.len() == states
                    && summary.destination_composition.iter().enumerate().all(
                        |(destination_index, share)| {
                            share.to_state == state_order[destination_index]
                                && share.bootstrap_interval_ppm.lower >= 0
                                && share.bootstrap_interval_ppm.upper
                                    >= share.bootstrap_interval_ppm.lower
                        },
                    )
                    && summary
                        .destination_composition
                        .iter()
                        .map(|share| u64::from(share.share_ppm))
                        .sum::<u64>()
                        == 1_000_000
            })
}

impl GliomaStatePlasticityInstrumentRun {
    pub fn validate(&self) -> Result<(), GliomaStatePlasticityInstrumentError> {
        self.transition_analysis.validate().map_err(|error| {
            GliomaStatePlasticityInstrumentError::InvalidOutput(error.to_string())
        })?;
        self.instrument_campaign.validate().map_err(|error| {
            GliomaStatePlasticityInstrumentError::InvalidOutput(error.to_string())
        })?;

        let state_ids = self
            .transition_analysis
            .state_order
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let priority_ids = self
            .priorities
            .iter()
            .map(|priority| priority.state_id.as_str())
            .collect::<BTreeSet<_>>();
        let readiness_ids = self
            .instrument_campaign
            .route_readiness
            .iter()
            .map(|route| route.stratum_id.as_str())
            .collect::<BTreeSet<_>>();
        let priority_sum = self
            .priorities
            .iter()
            .map(|priority| u32::from(priority.adjusted_priority_milli))
            .sum::<u32>();
        let campaign_priorities = self.instrument_campaign.campaign.as_ref().map(|campaign| {
            campaign
                .final_posteriors
                .iter()
                .map(|posterior| {
                    (
                        posterior.stratum_id.as_str(),
                        posterior.priority_weight_milli,
                    )
                })
                .collect::<BTreeMap<_, _>>()
        });
        let lineage_priority_map = self
            .lineage_propagation_priorities
            .iter()
            .map(|priority| (priority.state_id.as_str(), priority))
            .collect::<BTreeMap<_, _>>();
        let lineage_priorities_valid = match &self.lineage_propagation_analysis {
            Some(analysis) => {
                analysis.validate().is_ok()
                    && analysis.disposition == LineagePropagationDisposition::Qualified
                    && analysis.model_system == self.transition_analysis.model_system
                    && analysis.control_arm == self.transition_analysis.control_arm
                    && analysis.treatment_arm == self.transition_analysis.treatment_arm
                    && analysis.state_order == self.transition_analysis.state_order
                    && qualified_operator_is_safe(&analysis.control, &analysis.state_order)
                    && qualified_operator_is_safe(&analysis.treatment, &analysis.state_order)
                    && analysis.held_out_forecasts.iter().all(|forecast| {
                        forecast.mean_composition_error_ppm <= analysis.max_prediction_error_ppm
                            && forecast.mean_abundance_error_ppm
                                <= analysis.max_prediction_error_ppm
                    })
                    && self.lineage_propagation_priorities.len() == self.priorities.len()
                    && lineage_priority_map.len() == self.lineage_propagation_priorities.len()
                    && self.priorities.iter().all(|priority| {
                        lineage_priority_map
                            .get(priority.state_id.as_str())
                            .is_some_and(|lineage_priority| {
                                match (
                                    source_state_uncertainty_milli(
                                        &priority.state_id,
                                        analysis.state_order.len(),
                                        &analysis.control,
                                        &analysis.treatment,
                                    ),
                                    source_state_batch_attention_milli(
                                        &priority.state_id,
                                        analysis,
                                    ),
                                    match self.lineage_response_decomposition.as_ref() {
                                        Some(response) => source_response_attention_milli(
                                            &priority.state_id,
                                            response,
                                        )
                                        .map(|(attention, unresolved)| {
                                            (Some(attention), Some(unresolved))
                                        }),
                                        None => Ok((None, None)),
                                    },
                                ) {
                                    (
                                        Ok(interval_uncertainty),
                                        Ok((batch_sensitivity, batch_gap, batch_followup)),
                                        Ok((response_followup, unresolved_response_pairs)),
                                    ) => {
                                        lineage_priority.applied_priority_milli
                                            == priority.adjusted_priority_milli
                                            && lineage_priority.transition_priority_milli > 0
                                            && lineage_priority.batch_sensitivity_milli
                                                == batch_sensitivity
                                            && lineage_priority.batch_robustness_gap_milli
                                                == batch_gap
                                            && lineage_priority.batch_followup_attention_milli
                                                == batch_followup
                                            && lineage_priority.response_followup_attention_milli
                                                == response_followup
                                            && lineage_priority.unresolved_response_pair_count
                                                == unresolved_response_pairs
                                            && lineage_priority.propagation_uncertainty_milli
                                                == match response_followup {
                                                    Some(response_followup) => interval_uncertainty
                                                        .max(batch_followup)
                                                        .max(response_followup),
                                                    None => {
                                                        interval_uncertainty.max(batch_followup)
                                                    }
                                                }
                                    }
                                    _ => false,
                                }
                            })
                    })
                    && self
                        .lineage_propagation_priorities
                        .iter()
                        .map(|priority| u32::from(priority.transition_priority_milli))
                        .sum::<u32>()
                        == u32::from(SCORE_SCALE)
                    && self
                        .lineage_propagation_priorities
                        .iter()
                        .map(|priority| u32::from(priority.applied_priority_milli))
                        .sum::<u32>()
                        == u32::from(SCORE_SCALE)
            }
            None => {
                self.lineage_propagation_priorities.is_empty()
                    && self.lineage_response_decomposition.is_none()
            }
        };
        let lineage_response_valid = match (
            self.lineage_propagation_analysis.as_ref(),
            self.lineage_response_decomposition.as_ref(),
        ) {
            (Some(propagation), Some(response)) => {
                let expected_input_digest = ContentHash::of_value(&serde_json::json!({
                    "analysis_digest": propagation.digest,
                    "request": LineageResponseDecompositionRequest {
                        baseline_source_composition_ppm: response.baseline_source_composition_ppm.clone(),
                        minimum_component_ppm: response.minimum_component_ppm,
                    }
                }))
                .ok();
                response.validate().is_ok()
                    && expected_input_digest.as_ref() == Some(&response.input_digest)
                    && response.model_system
                        == format!("{:?}", propagation.model_system).to_lowercase()
                    && response.state_order == propagation.state_order
                    && response.control_arm == propagation.control_arm
                    && response.treatment_arm == propagation.treatment_arm
                    && response.interval_days == propagation.interval_days
            }
            (Some(_), None) | (None, None) => true,
            (None, Some(_)) => false,
        };
        let lineage_acquisition_valid = match (
            self.lineage_propagation_analysis.as_ref(),
            self.instrument_campaign.lineage_acquisition_plan.as_ref(),
        ) {
            (Some(analysis), Some(plan)) => {
                plan.analysis_digest == analysis.digest && plan.validate().is_ok()
            }
            (None, None) => true,
            (Some(analysis), None) if self.lineage_response_decomposition.is_some() => {
                analysis.disposition == LineagePropagationDisposition::Qualified
            }
            (Some(_), None) => {
                self.instrument_campaign.campaign.is_none()
                    && self.instrument_campaign.disposition
                        == super::simulation_gated_campaign::SimulationGatedAssayCampaignDisposition::Blocked
            }
            (None, Some(_)) => false,
        };

        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || state_ids != priority_ids
            || priority_ids.len() != self.priorities.len()
            || state_ids != readiness_ids
            || priority_sum != u32::from(SCORE_SCALE)
            || self.priorities.iter().any(|priority| {
                priority.adjusted_priority_milli == 0
                    || priority.adjusted_priority_milli > SCORE_SCALE
                    || priority.support_gap_milli > SCORE_SCALE
                    || priority.batch_sensitivity_distance_milli > SCORE_SCALE
                    || priority.batch_robustness_gap_milli > SCORE_SCALE
                    || priority.batch_followup_attention_milli
                        != priority
                            .batch_sensitivity_distance_milli
                            .max(priority.batch_robustness_gap_milli)
                    || priority.posterior_mean_total_variation_milli > SCORE_SCALE
                    || priority.posterior_expected_transition_distance_milli > SCORE_SCALE
                    || priority.transition_uncertainty_distance_milli > SCORE_SCALE
                    || priority.between_unit_heterogeneity_distance_milli > SCORE_SCALE
                    || priority.transition_attention_milli > SCORE_SCALE
            })
            || campaign_priorities.as_ref().is_some_and(|campaign| {
                campaign.len() != self.priorities.len()
                    || self.priorities.iter().any(|priority| {
                        campaign.get(priority.state_id.as_str())
                            != Some(&priority.adjusted_priority_milli)
                    })
            })
            || !lineage_priorities_valid
            || !lineage_response_valid
            || !lineage_acquisition_valid
        {
            return Err(GliomaStatePlasticityInstrumentError::InvalidOutput(
                "state, priority, route-readiness, campaign-weight, or identity alignment is invalid".into(),
            ));
        }

        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| GliomaStatePlasticityInstrumentError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(GliomaStatePlasticityInstrumentError::InvalidOutput(
                "state-plasticity instrument workflow digest mismatch".into(),
            ));
        }
        Ok(())
    }
}

/// Run a bounded research loop from measured state transitions through adaptive physical assays.
///
/// P10's descriptive state analysis is computed first. Its transparent support/uncertainty
/// priorities are applied to P06 strata. The P06 selector then chooses one assay at a time from
/// candidates whose P07 protocol simulation passed; each selected route is independently
/// revalidated by P08 before dispatch. Only the local outcome interpreter can convert the exact
/// returned instrument artifact into a scientific observation. Simulation output alone never
/// advances a posterior.
pub fn execute_glioma_state_plasticity_instrument_campaign<G, I>(
    request: &GliomaStatePlasticityInstrumentRequest,
    inputs: &GliomaStatePlasticityInstrumentInputs<'_>,
    gateway: &mut G,
    interpreter: &mut I,
) -> Result<GliomaStatePlasticityInstrumentRun, GliomaStatePlasticityInstrumentError>
where
    G: InstrumentProtocolGateway,
    I: GliomaInstrumentOutcomeInterpreter,
{
    execute_state_plasticity_instrument_campaign(
        request,
        inputs,
        None,
        None,
        None,
        gateway,
        interpreter,
    )
}

/// Extends `GAF-GLIOMA-P06-F19` with `GAF-GLIOMA-P10-F04`: use a qualified P10 lineage-propagation
/// result and its yield/composition decomposition to prioritize a bounded P06 assay campaign,
/// then apply the existing P07 simulation and P08 authorization gates. This entry point consumes
/// the investigator's explicit reference mixture; it never infers a baseline from treatment
/// outcomes.
pub fn execute_glioma_lineage_response_guided_state_plasticity_campaign<G, I>(
    request: &GliomaStatePlasticityInstrumentRequest,
    inputs: &GliomaStatePlasticityInstrumentInputs<'_>,
    propagation: &LineagePropagationAnalysis,
    response_request: &LineageResponseDecompositionRequest,
    gateway: &mut G,
    interpreter: &mut I,
) -> Result<GliomaStatePlasticityInstrumentRun, GliomaStatePlasticityInstrumentError>
where
    G: InstrumentProtocolGateway,
    I: GliomaInstrumentOutcomeInterpreter,
{
    validate_lineage_propagation_for_request(
        propagation,
        &request.transition_analysis.model_system,
        &request.transition_analysis.control_arm,
        &request.transition_analysis.treatment_arm,
        &request.transition_analysis.state_order,
    )?;
    let response = analyze_glioma_lineage_response_decomposition(response_request, propagation)
        .map_err(|error: LineageResponseDecompositionError| {
            GliomaStatePlasticityInstrumentError::LineageResponseAnalysis(error.to_string())
        })?;
    execute_state_plasticity_instrument_campaign(
        request,
        inputs,
        Some(propagation.clone()),
        Some(response),
        None,
        gateway,
        interpreter,
    )
}

/// Run the full closed loop after fitting a qualified lineage-resolved finite-interval
/// propagation model. Calibrated assay-outcome likelihoods score each candidate by expected
/// reduction in control/treatment lineage-operator uncertainty per cost. The run updates the
/// finite bootstrap posterior after each measured outcome, gates depleted ensembles by effective
/// sample size, and still dispatches only routes admitted by P07 and authorized by P08.
pub fn execute_glioma_lineage_propagation_guided_state_plasticity_campaign<G, I>(
    request: &GliomaStatePlasticityInstrumentRequest,
    inputs: &GliomaStatePlasticityInstrumentInputs<'_>,
    lineage_inputs: LineagePropagationGuidedCampaignInputs<'_>,
    gateway: &mut G,
    interpreter: &mut I,
) -> Result<GliomaStatePlasticityInstrumentRun, GliomaStatePlasticityInstrumentError>
where
    G: InstrumentProtocolGateway,
    I: GliomaInstrumentOutcomeInterpreter,
{
    execute_glioma_lineage_propagation_guided_state_plasticity_campaign_with_target(
        request,
        inputs,
        lineage_inputs,
        None,
        gateway,
        interpreter,
    )
}

/// Execute the same guarded workflow while optimizing an explicitly weighted destination-state
/// estimand (for example, perturbation-linked emergence of the mesenchymal-like state).
pub fn execute_glioma_lineage_propagation_guided_state_plasticity_campaign_for_target<G, I>(
    request: &GliomaStatePlasticityInstrumentRequest,
    inputs: &GliomaStatePlasticityInstrumentInputs<'_>,
    lineage_inputs: LineagePropagationGuidedCampaignInputs<'_>,
    target: &LineagePropagationAcquisitionTarget,
    gateway: &mut G,
    interpreter: &mut I,
) -> Result<GliomaStatePlasticityInstrumentRun, GliomaStatePlasticityInstrumentError>
where
    G: InstrumentProtocolGateway,
    I: GliomaInstrumentOutcomeInterpreter,
{
    execute_glioma_lineage_propagation_guided_state_plasticity_campaign_with_target(
        request,
        inputs,
        lineage_inputs,
        Some(target),
        gateway,
        interpreter,
    )
}

fn execute_glioma_lineage_propagation_guided_state_plasticity_campaign_with_target<G, I>(
    request: &GliomaStatePlasticityInstrumentRequest,
    inputs: &GliomaStatePlasticityInstrumentInputs<'_>,
    lineage_inputs: LineagePropagationGuidedCampaignInputs<'_>,
    target: Option<&LineagePropagationAcquisitionTarget>,
    gateway: &mut G,
    interpreter: &mut I,
) -> Result<GliomaStatePlasticityInstrumentRun, GliomaStatePlasticityInstrumentError>
where
    G: InstrumentProtocolGateway,
    I: GliomaInstrumentOutcomeInterpreter,
{
    let LineagePropagationGuidedCampaignInputs {
        request: propagation_request,
        snapshots: propagation_snapshots,
        response_models,
        posterior_particle_weights_million,
    } = lineage_inputs;
    if !inputs.initial_observations.is_empty() && posterior_particle_weights_million.is_empty() {
        return Err(GliomaStatePlasticityInstrumentError::InvalidInput(
            "resuming with prior assay observations requires the aligned lineage-posterior particle weights from the previous run".into(),
        ));
    }
    if propagation_request.model_system != request.transition_analysis.model_system
        || propagation_request.control_arm != request.transition_analysis.control_arm
        || propagation_request.treatment_arm != request.transition_analysis.treatment_arm
        || propagation_request.state_order != request.transition_analysis.state_order
    {
        return Err(GliomaStatePlasticityInstrumentError::InvalidInput(
            "lineage propagation and state-transition requests must match model, arms, and ordered states".into(),
        ));
    }
    let propagation =
        analyze_glioma_lineage_propagation(propagation_request, propagation_snapshots).map_err(
            |error| {
                GliomaStatePlasticityInstrumentError::LineagePropagationAnalysis(error.to_string())
            },
        )?;
    let acquisition_policy = match target {
        Some(target) => LineagePropagationAcquisitionPolicy::new_with_target(
            propagation.clone(),
            inputs.candidates,
            response_models,
            target.clone(),
            posterior_particle_weights_million,
            request.assay_campaign.campaign.campaign.max_rounds,
        ),
        None => LineagePropagationAcquisitionPolicy::new(
            propagation.clone(),
            inputs.candidates,
            response_models,
            posterior_particle_weights_million,
            request.assay_campaign.campaign.campaign.max_rounds,
        ),
    }
    .map_err(|error| GliomaStatePlasticityInstrumentError::Prioritization(error.to_string()))?;
    execute_state_plasticity_instrument_campaign(
        request,
        inputs,
        Some(propagation),
        None,
        Some(acquisition_policy),
        gateway,
        interpreter,
    )
}

fn execute_state_plasticity_instrument_campaign<G, I>(
    request: &GliomaStatePlasticityInstrumentRequest,
    inputs: &GliomaStatePlasticityInstrumentInputs<'_>,
    lineage_propagation_analysis: Option<LineagePropagationAnalysis>,
    lineage_response_decomposition: Option<LineageResponseDecomposition>,
    lineage_acquisition_policy: Option<LineagePropagationAcquisitionPolicy>,
    gateway: &mut G,
    interpreter: &mut I,
) -> Result<GliomaStatePlasticityInstrumentRun, GliomaStatePlasticityInstrumentError>
where
    G: InstrumentProtocolGateway,
    I: GliomaInstrumentOutcomeInterpreter,
{
    if request.transition_analysis.model_system
        != request.assay_campaign.campaign.campaign.model_system
    {
        return Err(GliomaStatePlasticityInstrumentError::InvalidInput(
            "longitudinal analysis and assay campaign must target the same preclinical glioma model system".into(),
        ));
    }
    if let Some(propagation) = &lineage_propagation_analysis {
        validate_lineage_propagation_for_request(
            propagation,
            &request.transition_analysis.model_system,
            &request.transition_analysis.control_arm,
            &request.transition_analysis.treatment_arm,
            &request.transition_analysis.state_order,
        )?;
    }

    let transition_analysis = analyze_glioma_state_transitions(
        &request.transition_analysis,
        inputs.longitudinal_observations,
    )
    .map_err(|error| GliomaStatePlasticityInstrumentError::TransitionAnalysis(error.to_string()))?;
    let mut priorities = plan_glioma_transition_guided_state_priorities(
        &request.assay_campaign.transition_guidance,
        &transition_analysis,
        inputs.strata,
    )
    .map_err(|error: TransitionGuidedStateCampaignError| {
        GliomaStatePlasticityInstrumentError::Prioritization(error.to_string())
    })?;
    let lineage_propagation_priorities = match &lineage_propagation_analysis {
        Some(propagation) => adjust_transition_priorities_with_response(
            &mut priorities,
            propagation,
            lineage_response_decomposition.as_ref(),
        )?,
        None => Vec::new(),
    };
    let guided_strata = reprioritize_strata(inputs.strata, &priorities)?;
    let campaign_result = if let Some(acquisition_policy) = lineage_acquisition_policy {
        execute_glioma_simulation_gated_assay_campaign_with_lineage_acquisition(
            &request.assay_campaign.campaign,
            SimulationGatedAssayCampaignInputs {
                strata: &guided_strata,
                candidates: inputs.candidates,
                initial_observations: inputs.initial_observations,
                routes: inputs.routes,
            },
            acquisition_policy,
            gateway,
            interpreter,
        )
    } else {
        execute_glioma_simulation_gated_assay_campaign(
            &request.assay_campaign.campaign,
            SimulationGatedAssayCampaignInputs {
                strata: &guided_strata,
                candidates: inputs.candidates,
                initial_observations: inputs.initial_observations,
                routes: inputs.routes,
            },
            gateway,
            interpreter,
        )
    };
    let instrument_campaign =
        campaign_result.map_err(|error: SimulationGatedAssayCampaignError| {
            GliomaStatePlasticityInstrumentError::InstrumentCampaign(error.to_string())
        })?;

    let mut run = GliomaStatePlasticityInstrumentRun {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        transition_analysis,
        priorities,
        lineage_propagation_analysis,
        lineage_response_decomposition,
        lineage_propagation_priorities,
        instrument_campaign,
        digest: ContentHash::of_bytes(b"unsealed-glioma-state-plasticity-instrument-run"),
    };
    run.digest = ContentHash::of_value(&digest_input(&run))
        .map_err(|error| GliomaStatePlasticityInstrumentError::Digest(error.to_string()))?;
    run.validate()?;
    Ok(run)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p10_interpretation_replication::lineage_propagation::{
        analyze_glioma_lineage_propagation, LineagePropagationSnapshot,
    };
    use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};

    fn artifact(label: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: format!("local:{label}"),
            content_hash: ContentHash::of_bytes(label.as_bytes()),
            content_type: "application/octet-stream".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn propagation_request() -> LineagePropagationRequest {
        LineagePropagationRequest {
            objective: "estimate finite-interval lineage-resolved glioma state propagation".into(),
            model_system: GliomaModelSystem::Organoid,
            control_arm: "control".into(),
            treatment_arm: "perturbation".into(),
            state_order: vec!["npc_like".into(), "mes_like".into()],
            min_units_per_arm: 2,
            min_lineages_per_arm: 4,
            ridge_penalty_ppm: 1,
            max_coefficient_ppm: 5_000_000,
            max_prediction_error_ppm: 300_000,
            bootstrap_replicates: 99,
            bootstrap_seed: ContentHash::of_bytes(b"p06-lineage-propagation-seed"),
            confidence_level_milli: 900,
            minimum_effect_ppm: 25_000,
        }
    }

    fn propagation_snapshots() -> Vec<LineagePropagationSnapshot> {
        propagation_snapshots_with_treatment_matrices([[[6, 4], [3, 5]]; 3])
    }

    fn propagation_snapshots_with_treatment_matrices(
        treatment_matrices: [[[u32; 2]; 2]; 3],
    ) -> Vec<LineagePropagationSnapshot> {
        let mut rows = Vec::new();
        for arm in ["control", "perturbation"] {
            for (unit_index, treatment_matrix) in treatment_matrices.iter().enumerate() {
                let matrix = if arm == "control" {
                    [[8, 2], [1, 7]]
                } else {
                    *treatment_matrix
                };
                for source in 0..2 {
                    let lineage = format!("{arm}-u{unit_index}-l{source}");
                    let mut values = vec![0_u32; 2];
                    values[source] = 100;
                    for (step, day) in [0_u32, 7, 14].into_iter().enumerate() {
                        rows.push(LineagePropagationSnapshot {
                            observation_id: format!("{lineage}-t{day}"),
                            experimental_unit_id: format!("{arm}-u{unit_index}"),
                            lineage_id: lineage.clone(),
                            arm_id: arm.into(),
                            model_system: GliomaModelSystem::Organoid,
                            assay_batch_id: format!("assay-batch-{unit_index}"),
                            timepoint_day: day,
                            state_counts: values.clone(),
                            capture_fraction_ppm: 1_000_000,
                            artifact: artifact(&format!("{lineage}-{day}")),
                        });
                        if step < 2 {
                            values = (0..2)
                                .map(|to| {
                                    (0..2)
                                        .map(|from| matrix[to][from] * values[from])
                                        .sum::<u32>()
                                        / 10
                                })
                                .collect();
                        }
                    }
                }
            }
        }
        rows
    }

    fn transition_priorities() -> Vec<TransitionGuidedStatePriority> {
        ["npc_like", "mes_like"]
            .into_iter()
            .map(|state_id| TransitionGuidedStatePriority {
                state_id: state_id.into(),
                investigator_priority_milli: 500,
                control_transition_count: 10,
                treatment_transition_count: 10,
                control_source_unit_count: 3,
                treatment_source_unit_count: 3,
                support_gap_milli: 0,
                batch_sensitivity_distance_milli: 0,
                batch_robustness_gap_milli: 0,
                batch_followup_attention_milli: 0,
                posterior_mean_total_variation_milli: 0,
                posterior_expected_transition_distance_milli: 0,
                transition_uncertainty_distance_milli: 0,
                between_unit_heterogeneity_distance_milli: 0,
                transition_attention_milli: 0,
                adjusted_priority_milli: 500,
            })
            .collect()
    }

    #[test]
    fn widened_independent_unit_intervals_increase_that_states_assay_weight() {
        let request = propagation_request();
        let baseline =
            analyze_glioma_lineage_propagation(&request, &propagation_snapshots()).unwrap();
        assert_eq!(
            baseline.disposition,
            LineagePropagationDisposition::Qualified
        );

        let mut variable_rows = propagation_snapshots();
        for row in variable_rows.iter_mut().filter(|row| {
            row.arm_id == "control"
                && row.experimental_unit_id == "control-u0"
                && row.lineage_id.ends_with("-l0")
        }) {
            if row.timepoint_day == 7 {
                row.state_counts = vec![100, 0];
            } else if row.timepoint_day == 14 {
                row.state_counts = vec![80, 10];
            }
        }
        let variable = analyze_glioma_lineage_propagation(&request, &variable_rows).unwrap();
        assert_eq!(
            variable.disposition,
            LineagePropagationDisposition::Qualified
        );

        let baseline_uncertainty =
            source_state_uncertainty_milli("npc_like", 2, &baseline.control, &baseline.treatment)
                .unwrap();
        let variable_uncertainty =
            source_state_uncertainty_milli("npc_like", 2, &variable.control, &variable.treatment)
                .unwrap();
        assert!(variable_uncertainty > baseline_uncertainty);

        let mut priorities = transition_priorities();
        let audit =
            adjust_transition_priorities_with_response(&mut priorities, &variable, None).unwrap();
        let npc_weight = priorities
            .iter()
            .find(|priority| priority.state_id == "npc_like")
            .unwrap()
            .adjusted_priority_milli;
        assert!(npc_weight > 500, "npc_weight={npc_weight}, audit={audit:?}");
        assert_eq!(
            priorities
                .iter()
                .map(|row| u32::from(row.adjusted_priority_milli))
                .sum::<u32>(),
            1_000
        );
        assert_eq!(
            audit
                .iter()
                .map(|row| u32::from(row.applied_priority_milli))
                .sum::<u32>(),
            1_000
        );
    }

    #[test]
    fn batch_fragility_is_carried_into_the_next_state_assay_priority() {
        let request = propagation_request();
        let stable =
            analyze_glioma_lineage_propagation(&request, &propagation_snapshots()).unwrap();
        let fragile_rows = propagation_snapshots_with_treatment_matrices([
            [[10, 2], [0, 7]],
            [[10, 2], [0, 7]],
            [[2, 2], [6, 7]],
        ]);
        let fragile = analyze_glioma_lineage_propagation(&request, &fragile_rows).unwrap();
        let npc_batch = fragile
            .batch_sensitivity
            .iter()
            .find(|row| row.from_state == "npc_like")
            .unwrap();
        assert_eq!(
            npc_batch.disposition,
            crate::glioma::programs::p10_interpretation_replication::LineagePropagationBatchSensitivityDisposition::Fragile
        );

        let mut stable_priorities = transition_priorities();
        let stable_audit =
            adjust_transition_priorities_with_response(&mut stable_priorities, &stable, None)
                .unwrap();
        let mut fragile_priorities = transition_priorities();
        let fragile_audit =
            adjust_transition_priorities_with_response(&mut fragile_priorities, &fragile, None)
                .unwrap();
        let stable_npc = stable_audit
            .iter()
            .find(|row| row.state_id == "npc_like")
            .unwrap();
        let fragile_npc = fragile_audit
            .iter()
            .find(|row| row.state_id == "npc_like")
            .unwrap();
        assert!(fragile_npc.batch_followup_attention_milli > 0);
        assert!(fragile_npc.batch_sensitivity_milli > 0);
        assert!(
            fragile_npc.propagation_uncertainty_milli >= fragile_npc.batch_followup_attention_milli
        );
        assert!(fragile_npc.applied_priority_milli > stable_npc.applied_priority_milli);
    }

    #[test]
    fn unresolved_lineage_fit_is_refused_before_campaign_execution() {
        let request = propagation_request();
        let mut rows = propagation_snapshots();
        rows.retain(|row| !row.lineage_id.ends_with("-l1"));
        let mut unresolved_request = request.clone();
        unresolved_request.min_lineages_per_arm = 2;
        let unresolved = analyze_glioma_lineage_propagation(&unresolved_request, &rows).unwrap();
        assert_eq!(
            unresolved.disposition,
            LineagePropagationDisposition::Unresolved
        );
        let error = validate_lineage_propagation_for_request(
            &unresolved,
            &GliomaModelSystem::Organoid,
            "control",
            "perturbation",
            &request.state_order,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            GliomaStatePlasticityInstrumentError::InvalidInput(_)
        ));
    }

    #[test]
    fn apportionment_is_deterministic_and_keeps_nonzero_state_weights() {
        let analysis =
            analyze_glioma_lineage_propagation(&propagation_request(), &propagation_snapshots())
                .unwrap();
        let mut priorities = transition_priorities();
        let first_audit =
            adjust_transition_priorities_with_response(&mut priorities, &analysis, None).unwrap();
        let mut reversed = transition_priorities();
        reversed.reverse();
        let second_audit =
            adjust_transition_priorities_with_response(&mut reversed, &analysis, None).unwrap();
        let mut first = priorities
            .iter()
            .map(|row| (row.state_id.clone(), row.adjusted_priority_milli))
            .collect::<Vec<_>>();
        let mut second = reversed
            .iter()
            .map(|row| (row.state_id.clone(), row.adjusted_priority_milli))
            .collect::<Vec<_>>();
        first.sort();
        second.sort();
        assert_eq!(first, second);
        assert_eq!(first_audit, second_audit);
        assert!(priorities.iter().all(|row| row.adjusted_priority_milli > 0));
    }

    #[test]
    fn lineage_guided_weights_are_the_weights_passed_to_each_research_stratum() {
        let analysis =
            analyze_glioma_lineage_propagation(&propagation_request(), &propagation_snapshots())
                .unwrap();
        let mut priorities = transition_priorities();
        adjust_transition_priorities_with_response(&mut priorities, &analysis, None).unwrap();
        let strata = vec![
            GliomaResearchStratum {
                stratum_id: "npc_like".into(),
                label: "NPC-like state".into(),
                priority_weight_milli: 500,
                mechanism_priors: Vec::new(),
            },
            GliomaResearchStratum {
                stratum_id: "mes_like".into(),
                label: "MES-like state".into(),
                priority_weight_milli: 500,
                mechanism_priors: Vec::new(),
            },
        ];
        let reprioritized = reprioritize_strata(&strata, &priorities).unwrap();
        for stratum in reprioritized {
            let priority = priorities
                .iter()
                .find(|priority| priority.state_id == stratum.stratum_id)
                .unwrap();
            assert_eq!(
                stratum.priority_weight_milli,
                priority.adjusted_priority_milli
            );
        }
    }

    #[test]
    fn p10_response_focus_raises_the_matching_source_states_assay_weight() {
        let propagation = analyze_glioma_lineage_propagation(
            &propagation_request(),
            &propagation_snapshots_with_treatment_matrices([[[6, 2], [4, 8]]; 3]),
        )
        .unwrap();
        let response = analyze_glioma_lineage_response_decomposition(
            &LineageResponseDecompositionRequest {
                baseline_source_composition_ppm: vec![700_000, 300_000],
                minimum_component_ppm: 25_000,
            },
            &propagation,
        )
        .unwrap();
        let npc_to_mes = response
            .decompositions
            .iter()
            .find(|row| row.from_state == "npc_like" && row.to_state == "mes_like")
            .unwrap();
        assert_eq!(
            npc_to_mes.follow_up_focus,
            LineageResponseFollowUpFocus::ValidateStateComposition
        );

        let mut ordinary = transition_priorities();
        let ordinary_audit =
            adjust_transition_priorities_with_response(&mut ordinary, &propagation, None).unwrap();
        assert!(ordinary_audit.iter().all(|row| {
            row.response_followup_attention_milli.is_none()
                && row.unresolved_response_pair_count.is_none()
        }));
        let mut response_guided = transition_priorities();
        let response_audit = adjust_transition_priorities_with_response(
            &mut response_guided,
            &propagation,
            Some(&response),
        )
        .unwrap();
        let ordinary_npc = ordinary_audit
            .iter()
            .find(|row| row.state_id == "npc_like")
            .unwrap();
        let ordinary_mes = ordinary_audit
            .iter()
            .find(|row| row.state_id == "mes_like")
            .unwrap();
        let guided_npc = response_audit
            .iter()
            .find(|row| row.state_id == "npc_like")
            .unwrap();
        let guided_mes = response_audit
            .iter()
            .find(|row| row.state_id == "mes_like")
            .unwrap();
        assert_eq!(guided_npc.response_followup_attention_milli, Some(1_000));
        assert_eq!(guided_npc.unresolved_response_pair_count, Some(0));
        assert_eq!(guided_mes.response_followup_attention_milli, Some(0));
        assert!(
            u32::from(guided_npc.applied_priority_milli)
                * u32::from(ordinary_mes.applied_priority_milli)
                > u32::from(ordinary_npc.applied_priority_milli)
                    * u32::from(guided_mes.applied_priority_milli)
        );
    }
}
