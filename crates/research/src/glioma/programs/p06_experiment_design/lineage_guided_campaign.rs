//! P10 lineage analysis driving P06 state-specific adaptive assay execution.
//!
//! This bounded research workflow analyzes replicate-level clonal/state counts, allocates follow-up
//! attention to states with support gaps, unresolved uncertainty, or large component shifts, then
//! executes investigator-declared assays through the local P06 executor. It never turns lineage
//! abundance into a genetic-selection claim or within-lineage redistribution into proof of cell
//! switching. Assays are selected from the supplied candidate set and every measured outcome,
//! including null and out-of-model results, remains in the campaign record.

use super::adaptive_information_campaign::FEATURE_ID;
use super::simulation_gated_campaign::{
    execute_glioma_simulation_gated_assay_campaign, GliomaInstrumentOutcomeInterpreter,
    SimulationGatedAssayCampaignError, SimulationGatedAssayCampaignInputs,
    SimulationGatedAssayCampaignRun,
};
use super::state_stratified_campaign::{
    execute_glioma_state_stratified_campaign, GliomaResearchStratum,
    GliomaStateStratifiedAssayExecutor, StateStratifiedCampaign, StateStratifiedCampaignError,
    StateStratifiedCampaignRequest, StratifiedAssayCandidate, StratifiedAssayObservation,
};
use crate::glioma::programs::p10_interpretation_replication::lineage_dynamics::{
    analyze_glioma_lineage_dynamics, LineageDynamicsAnalysis, LineageDynamicsError,
    LineageDynamicsRequest, LineageStateSnapshot, UnitEligibility,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const OUTPUT_SCHEMA: &str = "GliomaLineageGuidedAssayWorkflow1@1";
pub const SCORE_SCALE: u16 = 1_000;
pub const INSTRUMENT_CAMPAIGN_OUTPUT_SCHEMA: &str = "GliomaLineageGuidedInstrumentCampaign1@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineageGuidanceWeights {
    /// Prioritizes states whose independent-unit support is below the P10 analysis floor.
    pub support_gap_weight_milli: u16,
    /// Prioritizes wide component intervals or analysis states without a resolvable interval.
    pub uncertainty_weight_milli: u16,
    /// Prioritizes larger descriptive population, abundance, or redistribution contrasts.
    pub signal_weight_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineageGuidedStatePriority {
    pub state_id: String,
    pub control_unit_count: u32,
    pub comparison_unit_count: u32,
    pub support_gap_milli: u16,
    pub uncertainty_milli: u16,
    pub signal_milli: u16,
    pub raw_priority_milli: u16,
    /// Final positive share assigned to this state's adaptive-assay budget, summing to 1,000.
    pub priority_weight_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineageGuidedAssayWorkflowRequest {
    pub lineage_analysis: LineageDynamicsRequest,
    pub assay_campaign: StateStratifiedCampaignRequest,
    pub guidance_weights: LineageGuidanceWeights,
}

/// P10 snapshot data and local P06/P07/P08 campaign inputs for a lineage-guided instrument run.
#[derive(Debug, Clone, Copy)]
pub struct LineageGuidedInstrumentCampaignInputs<'a> {
    pub snapshots: &'a [LineageStateSnapshot],
    pub campaign: SimulationGatedAssayCampaignInputs<'a>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineageGuidedAssayWorkflowRun {
    pub feature_id: String,
    pub output_schema: String,
    pub lineage_analysis: LineageDynamicsAnalysis,
    pub guidance_weights: LineageGuidanceWeights,
    pub priorities: Vec<LineageGuidedStatePriority>,
    pub assay_campaign: StateStratifiedCampaign,
    pub digest: ContentHash,
}

/// P10 lineage support/uncertainty guides P06 assay choice; P07 screens the protocol and P08
/// executes the selected assay through its existing local safety and authorization gates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LineageGuidedInstrumentCampaignRun {
    pub feature_id: String,
    pub output_schema: String,
    pub lineage_analysis: LineageDynamicsAnalysis,
    pub guidance_weights: LineageGuidanceWeights,
    pub priorities: Vec<LineageGuidedStatePriority>,
    pub instrument_campaign: SimulationGatedAssayCampaignRun,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum LineageGuidedAssayWorkflowError {
    #[error("lineage-guided assay workflow request is invalid: {0}")]
    InvalidRequest(String),
    #[error("lineage-guided assay workflow input is invalid: {0}")]
    InvalidInput(String),
    #[error("lineage-guided assay workflow output is invalid: {0}")]
    InvalidOutput(String),
    #[error("lineage analysis failed: {0}")]
    Analysis(String),
    #[error("state-stratified assay campaign failed: {0}")]
    Campaign(String),
    #[error("lineage-guided assay workflow digest failed: {0}")]
    Digest(String),
}

fn instrument_campaign_digest_input(run: &LineageGuidedInstrumentCampaignRun) -> serde_json::Value {
    serde_json::json!({
        "feature_id": run.feature_id,
        "output_schema": run.output_schema,
        "lineage_analysis_digest": run.lineage_analysis.digest,
        "guidance_weights": run.guidance_weights,
        "priorities": run.priorities,
        "instrument_campaign_digest": run.instrument_campaign.digest,
    })
}

impl LineageGuidedInstrumentCampaignRun {
    pub fn validate(&self) -> Result<(), LineageGuidedAssayWorkflowError> {
        self.lineage_analysis
            .validate()
            .map_err(|error| LineageGuidedAssayWorkflowError::InvalidOutput(error.to_string()))?;
        self.instrument_campaign.validate().map_err(
            |error: SimulationGatedAssayCampaignError| {
                LineageGuidedAssayWorkflowError::InvalidOutput(error.to_string())
            },
        )?;
        let expected = compute_priorities(&self.lineage_analysis, &self.guidance_weights);
        let weights = expected
            .iter()
            .map(|item| (item.state_id.as_str(), item.priority_weight_milli))
            .collect::<BTreeMap<_, _>>();
        let posterior_weights_match =
            self.instrument_campaign
                .campaign
                .as_ref()
                .is_none_or(|campaign| {
                    campaign.final_posteriors.iter().all(|posterior| {
                        weights.get(posterior.stratum_id.as_str())
                            == Some(&posterior.priority_weight_milli)
                    })
                });
        let route_states = self
            .instrument_campaign
            .route_readiness
            .iter()
            .map(|route| route.stratum_id.as_str())
            .collect::<BTreeSet<_>>();
        let expected_states = weights.keys().copied().collect::<BTreeSet<_>>();
        let weight_sum = self
            .priorities
            .iter()
            .map(|priority| u32::from(priority.priority_weight_milli))
            .sum::<u32>();
        let digest = ContentHash::of_value(&instrument_campaign_digest_input(self))
            .map_err(|error| LineageGuidedAssayWorkflowError::Digest(error.to_string()))?;
        if self.feature_id != FEATURE_ID
            || self.output_schema != INSTRUMENT_CAMPAIGN_OUTPUT_SCHEMA
            || self.lineage_analysis.model_system
                != self
                    .instrument_campaign
                    .campaign
                    .as_ref()
                    .map(|campaign| campaign.model_system)
                    .unwrap_or(self.lineage_analysis.model_system)
            || self.priorities != expected
            || weight_sum != u32::from(SCORE_SCALE)
            || !posterior_weights_match
            || route_states != expected_states
            || self.digest != digest
        {
            return Err(LineageGuidedAssayWorkflowError::InvalidOutput(
                "lineage priority, state coverage, campaign model, or digest does not match the P10 analysis".into(),
            ));
        }
        Ok(())
    }
}

fn digest_input(run: &LineageGuidedAssayWorkflowRun) -> serde_json::Value {
    serde_json::json!({
        "feature_id": run.feature_id,
        "output_schema": run.output_schema,
        "lineage_analysis_digest": run.lineage_analysis.digest,
        "guidance_weights": run.guidance_weights,
        "priorities": run.priorities,
        "assay_campaign_digest": run.assay_campaign.digest,
    })
}

fn clamp_score(value: u64) -> u16 {
    value.min(u64::from(SCORE_SCALE)) as u16
}

fn interval_width(
    interval: Option<crate::glioma::programs::p10_interpretation_replication::lineage_dynamics::BootstrapInterval>,
) -> Option<u64> {
    interval.map(|value| u64::from(value.upper_ppm.saturating_sub(value.lower_ppm) as u32))
}

fn normalized_priorities(raw: &BTreeMap<String, u16>) -> BTreeMap<String, u16> {
    if raw.is_empty() {
        return BTreeMap::new();
    }
    let count = raw.len() as u64;
    let distributable = u64::from(SCORE_SCALE).saturating_sub(count);
    // A one-point pseudocount gives every declared state a defined allocation even when all
    // guidance scores are zero (e.g. supported, precise, null contrasts).
    let total = raw.values().map(|value| u64::from(*value) + 1).sum::<u64>();
    let mut output = BTreeMap::new();
    let mut remainders = Vec::with_capacity(raw.len());
    for (state, score) in raw {
        let scaled = (u64::from(*score) + 1) * distributable;
        output.insert(state.clone(), 1 + (scaled / total) as u16);
        remainders.push((state.clone(), scaled % total));
    }
    let assigned = output
        .values()
        .map(|weight| u64::from(*weight))
        .sum::<u64>();
    remainders.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    for (state, _) in remainders
        .into_iter()
        .take(SCORE_SCALE as usize - assigned as usize)
    {
        if let Some(weight) = output.get_mut(&state) {
            *weight += 1;
        }
    }
    output
}

/// Derive auditable assay-attention weights from P10 replicate-level support and uncertainty.
/// This is a sampling heuristic, not an efficacy score; all three investigator-visible weights
/// must sum to 1,000 and the resulting per-state budget weights are normalized exactly.
fn compute_priorities(
    analysis: &LineageDynamicsAnalysis,
    guidance_weights: &LineageGuidanceWeights,
) -> Vec<LineageGuidedStatePriority> {
    let mut scored = Vec::with_capacity(analysis.state_order.len());
    let mut raw = BTreeMap::new();
    for contrast in &analysis.contrasts {
        let state_index = analysis
            .state_order
            .iter()
            .position(|state| state == &contrast.state_id)
            .expect("validated lineage analysis has an ordered contrast for every state");
        let control_count = analysis
            .units
            .iter()
            .filter(|unit| {
                unit.arm_id == analysis.control_arm
                    && unit.eligibility == UnitEligibility::Included
                    && (unit.states[state_index].baseline_cell_count > 0
                        || unit.states[state_index].followup_cell_count > 0)
            })
            .count() as u32;
        let comparison_count = analysis
            .units
            .iter()
            .filter(|unit| {
                unit.arm_id == analysis.comparison_arm
                    && unit.eligibility == UnitEligibility::Included
                    && (unit.states[state_index].baseline_cell_count > 0
                        || unit.states[state_index].followup_cell_count > 0)
            })
            .count() as u32;
        let supported = control_count
            .min(comparison_count)
            .min(analysis.min_units_per_arm as u32);
        let support_gap = SCORE_SCALE.saturating_sub(
            (u32::from(SCORE_SCALE) * supported / analysis.min_units_per_arm as u32) as u16,
        );
        let uncertainty = [
            interval_width(contrast.population_shift_interval),
            interval_width(contrast.lineage_selection_interval),
            interval_width(contrast.within_lineage_redistribution_interval),
        ]
        .into_iter()
        .flatten()
        .max()
        .map_or(SCORE_SCALE, |width| {
            clamp_score(width.saturating_mul(u64::from(SCORE_SCALE)) / 4_000_000)
        });
        let signal = clamp_score(
            [
                contrast.population_shift_difference_ppm.unsigned_abs(),
                contrast.lineage_selection_difference_ppm.unsigned_abs(),
                contrast
                    .within_lineage_redistribution_difference_ppm
                    .unsigned_abs(),
            ]
            .into_iter()
            .max()
            .unwrap_or(0) as u64
                * u64::from(SCORE_SCALE)
                / 1_000_000,
        );
        let raw_score = clamp_score(
            (u64::from(support_gap) * u64::from(guidance_weights.support_gap_weight_milli)
                + u64::from(uncertainty) * u64::from(guidance_weights.uncertainty_weight_milli)
                + u64::from(signal) * u64::from(guidance_weights.signal_weight_milli)
                + 500)
                / u64::from(SCORE_SCALE),
        );
        raw.insert(contrast.state_id.clone(), raw_score);
        scored.push(LineageGuidedStatePriority {
            state_id: contrast.state_id.clone(),
            control_unit_count: control_count,
            comparison_unit_count: comparison_count,
            support_gap_milli: support_gap,
            uncertainty_milli: uncertainty,
            signal_milli: signal,
            raw_priority_milli: raw_score,
            priority_weight_milli: 1,
        });
    }
    let normalized = normalized_priorities(&raw);
    for row in &mut scored {
        row.priority_weight_milli = normalized[&row.state_id];
    }
    scored
}

pub fn plan_glioma_lineage_guided_state_priorities(
    analysis: &LineageDynamicsAnalysis,
    guidance_weights: &LineageGuidanceWeights,
    strata: &[GliomaResearchStratum],
) -> Result<Vec<LineageGuidedStatePriority>, LineageGuidedAssayWorkflowError> {
    analysis.validate().map_err(|error: LineageDynamicsError| {
        LineageGuidedAssayWorkflowError::InvalidInput(error.to_string())
    })?;
    let weight_sum = u32::from(guidance_weights.support_gap_weight_milli)
        + u32::from(guidance_weights.uncertainty_weight_milli)
        + u32::from(guidance_weights.signal_weight_milli);
    let stratum_ids = strata
        .iter()
        .map(|item| item.stratum_id.as_str())
        .collect::<BTreeSet<_>>();
    if weight_sum != u32::from(SCORE_SCALE)
        || strata.len() != analysis.state_order.len()
        || stratum_ids.len() != strata.len()
        || strata
            .iter()
            .any(|item| item.stratum_id.trim().is_empty() || item.label.trim().is_empty())
        || stratum_ids
            != analysis
                .state_order
                .iter()
                .map(String::as_str)
                .collect::<BTreeSet<_>>()
    {
        return Err(LineageGuidedAssayWorkflowError::InvalidRequest(
            "guidance weights must sum to 1,000 and assay strata must uniquely match the analyzed states".into(),
        ));
    }

    Ok(compute_priorities(analysis, guidance_weights))
}

impl LineageGuidedAssayWorkflowRun {
    pub fn validate(&self) -> Result<(), LineageGuidedAssayWorkflowError> {
        self.lineage_analysis
            .validate()
            .map_err(|error| LineageGuidedAssayWorkflowError::InvalidOutput(error.to_string()))?;
        self.assay_campaign
            .validate()
            .map_err(|error| LineageGuidedAssayWorkflowError::InvalidOutput(error.to_string()))?;
        let expected_priorities =
            compute_priorities(&self.lineage_analysis, &self.guidance_weights);
        let weight_sum = u32::from(self.guidance_weights.support_gap_weight_milli)
            + u32::from(self.guidance_weights.uncertainty_weight_milli)
            + u32::from(self.guidance_weights.signal_weight_milli);
        let expected_weights = expected_priorities
            .iter()
            .map(|priority| (priority.state_id.as_str(), priority.priority_weight_milli))
            .collect::<BTreeMap<_, _>>();
        let campaign_weights_match = self
            .assay_campaign
            .final_posteriors
            .iter()
            .all(|posterior| {
                expected_weights.get(posterior.stratum_id.as_str())
                    == Some(&posterior.priority_weight_milli)
            })
            && self.assay_campaign.final_posteriors.len() == expected_weights.len();
        let expected_digest = ContentHash::of_value(&digest_input(self))
            .map_err(|error| LineageGuidedAssayWorkflowError::Digest(error.to_string()))?;
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || weight_sum != u32::from(SCORE_SCALE)
            || self.lineage_analysis.model_system != self.assay_campaign.model_system
            || self.priorities != expected_priorities
            || !campaign_weights_match
            || self.digest != expected_digest
        {
            return Err(LineageGuidedAssayWorkflowError::InvalidOutput(
                "workflow identity, P10-to-P06 priority linkage, model system, or digest is invalid".into(),
            ));
        }
        Ok(())
    }
}

/// Analyze lineage-resolved glioma counts, convert support/uncertainty/signal into inspectable
/// state-specific assay weights, then run the caller's bounded P06 adaptive campaign. The workflow
/// never invents assays, synthesizes outcomes, exports raw data, or executes a physical instrument.
pub fn execute_glioma_lineage_guided_assay_workflow<E: GliomaStateStratifiedAssayExecutor>(
    request: &LineageGuidedAssayWorkflowRequest,
    snapshots: &[LineageStateSnapshot],
    strata: &[GliomaResearchStratum],
    assay_candidates: &[StratifiedAssayCandidate],
    initial_assay_observations: &[StratifiedAssayObservation],
    executor: &mut E,
) -> Result<LineageGuidedAssayWorkflowRun, LineageGuidedAssayWorkflowError> {
    if request.lineage_analysis.model_system != request.assay_campaign.campaign.model_system {
        return Err(LineageGuidedAssayWorkflowError::InvalidRequest(
            "P10 lineage analysis and P06 assay execution must target the same preclinical model system".into(),
        ));
    }
    let lineage_analysis = analyze_glioma_lineage_dynamics(&request.lineage_analysis, snapshots)
        .map_err(|error| LineageGuidedAssayWorkflowError::Analysis(error.to_string()))?;
    let priorities = plan_glioma_lineage_guided_state_priorities(
        &lineage_analysis,
        &request.guidance_weights,
        strata,
    )?;
    let weights = priorities
        .iter()
        .map(|priority| (priority.state_id.clone(), priority.priority_weight_milli))
        .collect::<BTreeMap<_, _>>();
    let guided_strata = strata
        .iter()
        .cloned()
        .map(|mut stratum| {
            stratum.priority_weight_milli = weights[&stratum.stratum_id];
            stratum
        })
        .collect::<Vec<_>>();
    let assay_campaign = execute_glioma_state_stratified_campaign(
        &request.assay_campaign,
        &guided_strata,
        assay_candidates,
        initial_assay_observations,
        executor,
    )
    .map_err(|error: StateStratifiedCampaignError| {
        LineageGuidedAssayWorkflowError::Campaign(error.to_string())
    })?;
    let mut run = LineageGuidedAssayWorkflowRun {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        lineage_analysis,
        guidance_weights: request.guidance_weights.clone(),
        priorities,
        assay_campaign,
        digest: ContentHash::of_bytes(b"unsealed-glioma-lineage-guided-assay-workflow"),
    };
    run.digest = ContentHash::of_value(&digest_input(&run))
        .map_err(|error| LineageGuidedAssayWorkflowError::Digest(error.to_string()))?;
    run.validate()?;
    Ok(run)
}

/// End-to-end lineage-guided preclinical assay loop: analyze replicate-level P10 data, assign
/// inspectable P06 state priorities, screen declared assays with P07, execute selected routes via
/// P08, and update only the measured state's posterior from an artifact-backed local interpretation.
pub fn execute_glioma_lineage_guided_instrument_campaign<G, I>(
    request: &LineageGuidedAssayWorkflowRequest,
    inputs: LineageGuidedInstrumentCampaignInputs<'_>,
    gateway: &mut G,
    interpreter: &mut I,
) -> Result<LineageGuidedInstrumentCampaignRun, LineageGuidedAssayWorkflowError>
where
    G: super::super::p08_instrument_robotics::InstrumentProtocolGateway,
    I: GliomaInstrumentOutcomeInterpreter,
{
    if request.lineage_analysis.model_system != request.assay_campaign.campaign.model_system {
        return Err(LineageGuidedAssayWorkflowError::InvalidRequest(
            "P10 lineage analysis and instrument-backed P06 campaign must target the same preclinical model system".into(),
        ));
    }
    let lineage_analysis =
        analyze_glioma_lineage_dynamics(&request.lineage_analysis, inputs.snapshots)
            .map_err(|error| LineageGuidedAssayWorkflowError::Analysis(error.to_string()))?;
    let priorities = plan_glioma_lineage_guided_state_priorities(
        &lineage_analysis,
        &request.guidance_weights,
        inputs.campaign.strata,
    )?;
    let weights = priorities
        .iter()
        .map(|priority| (priority.state_id.clone(), priority.priority_weight_milli))
        .collect::<BTreeMap<_, _>>();
    let guided_strata = inputs
        .campaign
        .strata
        .iter()
        .cloned()
        .map(|mut stratum| {
            stratum.priority_weight_milli = weights[&stratum.stratum_id];
            stratum
        })
        .collect::<Vec<_>>();
    let instrument_campaign = execute_glioma_simulation_gated_assay_campaign(
        &request.assay_campaign,
        SimulationGatedAssayCampaignInputs {
            strata: &guided_strata,
            ..inputs.campaign
        },
        gateway,
        interpreter,
    )
    .map_err(|error| LineageGuidedAssayWorkflowError::Campaign(error.to_string()))?;
    let mut run = LineageGuidedInstrumentCampaignRun {
        feature_id: FEATURE_ID.into(),
        output_schema: INSTRUMENT_CAMPAIGN_OUTPUT_SCHEMA.into(),
        lineage_analysis,
        guidance_weights: request.guidance_weights.clone(),
        priorities,
        instrument_campaign,
        digest: ContentHash::of_bytes(b"unsealed-lineage-guided-instrument-campaign"),
    };
    run.digest = ContentHash::of_value(&instrument_campaign_digest_input(&run))
        .map_err(|error| LineageGuidedAssayWorkflowError::Digest(error.to_string()))?;
    run.validate()?;
    Ok(run)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p06_experiment_design::adaptive_information_campaign::AdaptiveInformationObservation;
    use crate::glioma::programs::p06_experiment_design::information_design::{
        DesignAction, DesignMechanism, DesignOutcome,
    };
    use crate::glioma::programs::p10_interpretation_replication::lineage_dynamics::LineageStateCount;
    use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
    use std::collections::{BTreeMap, VecDeque};

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: id.into(),
            content_hash: ContentHash::of_bytes(id.as_bytes()),
            content_type: "application/vnd.aurora.preclinical-assay+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn analysis_request(states: &[&str]) -> LineageDynamicsRequest {
        LineageDynamicsRequest {
            objective: "identify glioma lineage-state response to a perturbation".into(),
            control_arm: "vehicle".into(),
            comparison_arm: "perturbation".into(),
            model_system: GliomaModelSystem::Organoid,
            state_order: states.iter().map(|state| (*state).into()).collect(),
            baseline_timepoint: 0,
            followup_timepoint: 1,
            min_units_per_arm: 3,
            min_lineages_per_unit: 2,
            min_cells_per_unit_timepoint: 50,
            minimum_difference_ppm: 10_000,
            bootstrap_replicates: 999,
            bootstrap_seed: ContentHash::of_bytes(b"lineage-guided-test"),
        }
    }

    fn append_unit(
        rows: &mut Vec<LineageStateSnapshot>,
        states: &[&str],
        arm: &str,
        unit: &str,
        baseline: &[Vec<u64>],
        followup: &[Vec<u64>],
    ) {
        for lineage in 0..baseline.len() {
            for (timepoint, counts) in [(0, &baseline[lineage]), (1, &followup[lineage])] {
                let observation_id = format!("{arm}-{unit}-lineage-{lineage}-{timepoint}");
                rows.push(LineageStateSnapshot {
                    observation_id: observation_id.clone(),
                    experimental_unit_id: unit.into(),
                    lineage_id: format!("lineage-{lineage}"),
                    arm_id: arm.into(),
                    batch_id: "batch-a".into(),
                    model_system: GliomaModelSystem::Organoid,
                    timepoint,
                    state_counts: states
                        .iter()
                        .zip(counts)
                        .map(|(state, count)| LineageStateCount {
                            state_id: (*state).into(),
                            cell_count: *count,
                        })
                        .collect(),
                    artifact: LocalArtifactRef {
                        artifact_id: observation_id,
                        content_hash: ContentHash::of_bytes(b"local-glioma-lineage-fixture"),
                        content_type: "application/vnd.aurora.glioma-lineage-counts+json".into(),
                        local_only: true,
                        contains_human_data: false,
                        contains_direct_identifiers: false,
                    },
                });
            }
        }
    }

    fn assay_stratum(id: &str) -> GliomaResearchStratum {
        GliomaResearchStratum {
            stratum_id: id.into(),
            label: id.into(),
            priority_weight_milli: 1,
            mechanism_priors: vec![
                DesignMechanism {
                    mechanism_id: "mechanism-a".into(),
                    prior_milli: 500,
                },
                DesignMechanism {
                    mechanism_id: "mechanism-b".into(),
                    prior_milli: 500,
                },
            ],
        }
    }

    fn campaign_request(model_system: GliomaModelSystem) -> StateStratifiedCampaignRequest {
        StateStratifiedCampaignRequest {
            campaign:
                super::super::adaptive_information_campaign::AdaptiveInformationCampaignRequest {
                    objective: "disambiguate state-specific glioma response mechanisms".into(),
                    model_system,
                    max_rounds: 1,
                    max_actions_per_round: 1,
                    budget_units: 5,
                    min_information_gain_milli: 1,
                    information_weight_milli: 1_000,
                    feasibility_weight_milli: 0,
                    risk_penalty_milli: 0,
                    cost_penalty_milli: 0,
                    risk_ceiling_milli: 1_000,
                    stop_concentration_milli: 1_000,
                },
            min_assays_per_stratum: 1,
        }
    }

    fn candidate(stratum_id: &str) -> StratifiedAssayCandidate {
        let outcome = |outcome_id: &str, a: u16, b: u16| DesignOutcome {
            outcome_id: outcome_id.into(),
            label: outcome_id.into(),
            probability_milli_by_mechanism: BTreeMap::from([
                ("mechanism-a".into(), a),
                ("mechanism-b".into(), b),
            ]),
        };
        StratifiedAssayCandidate {
            stratum_id: stratum_id.into(),
            action: DesignAction {
                action_id: format!("assay-{stratum_id}"),
                feature_id: FEATURE_ID.into(),
                label: format!("orthogonal {stratum_id} assay"),
                outcomes: vec![outcome("null", 900, 100), outcome("signal", 100, 900)],
                feasibility_milli: 1_000,
                risk_milli: 0,
                cost_units: 1,
                max_replicates: 1,
            },
            negative_outcome_order: vec!["null".into()],
        }
    }

    struct NullExecutor(VecDeque<String>);

    impl GliomaStateStratifiedAssayExecutor for NullExecutor {
        fn execute_action(
            &mut self,
            _: &str,
            action: &DesignAction,
            _: u16,
        ) -> Result<
            AdaptiveInformationObservation,
            super::super::state_stratified_campaign::StateStratifiedExecutionFailure,
        > {
            Ok(AdaptiveInformationObservation {
                action_id: action.action_id.clone(),
                outcome_id: self.0.pop_front().unwrap_or_else(|| "null".into()),
                replicate_index: 1,
                artifact: artifact(&action.action_id),
            })
        }
    }

    #[test]
    fn all_zero_guidance_scores_still_allocate_the_full_adaptive_budget() {
        let priorities = normalized_priorities(&BTreeMap::from([
            ("mes".to_owned(), 0),
            ("npc".to_owned(), 0),
            ("opc".to_owned(), 0),
        ]));
        assert_eq!(
            priorities
                .values()
                .map(|weight| u32::from(*weight))
                .sum::<u32>(),
            1_000
        );
        assert!(priorities.values().all(|weight| *weight > 0));
        assert_eq!(
            priorities.values().copied().collect::<BTreeSet<_>>(),
            BTreeSet::from([333, 334])
        );
    }

    #[test]
    fn support_gaps_shift_assay_budget_to_the_weakly_covered_glioma_state() {
        let states = ["npc", "opc", "mes"];
        let mut rows = Vec::new();
        for index in 0..3 {
            append_unit(
                &mut rows,
                &states,
                "vehicle",
                &format!("control-{index}"),
                &[vec![100, 0, 0], vec![0, 100, 0]],
                &[vec![100, 0, 0], vec![0, 100, 0]],
            );
            append_unit(
                &mut rows,
                &states,
                "perturbation",
                &format!("treated-{index}"),
                &[vec![100, 0, 0], vec![0, 100, 0]],
                &[vec![50, 0, 50], vec![0, 100, 0]],
            );
        }
        let analysis = analyze_glioma_lineage_dynamics(&analysis_request(&states), &rows).unwrap();
        let strata = states
            .iter()
            .map(|state| assay_stratum(state))
            .collect::<Vec<_>>();
        let priorities = plan_glioma_lineage_guided_state_priorities(
            &analysis,
            &LineageGuidanceWeights {
                support_gap_weight_milli: 1_000,
                uncertainty_weight_milli: 0,
                signal_weight_milli: 0,
            },
            &strata,
        )
        .unwrap();
        assert_eq!(
            priorities
                .iter()
                .map(|row| u32::from(row.priority_weight_milli))
                .sum::<u32>(),
            1_000
        );
        let mes = priorities.iter().find(|row| row.state_id == "mes").unwrap();
        assert_eq!(mes.support_gap_milli, 1_000);
        assert_eq!(mes.control_unit_count, 0);
        assert_eq!(mes.comparison_unit_count, 3);
        assert!(
            mes.priority_weight_milli
                > priorities
                    .iter()
                    .find(|row| row.state_id == "npc")
                    .unwrap()
                    .priority_weight_milli
        );
    }

    #[test]
    fn exact_rare_state_counts_survive_when_compositional_shares_round_to_zero() {
        let states = ["npc", "opc", "mes"];
        let mut rows = Vec::new();
        for index in 0..3 {
            let counts = [vec![1_000_000_000_000, 0, 1], vec![0, 1_000_000_000_000, 0]];
            append_unit(
                &mut rows,
                &states,
                "vehicle",
                &format!("control-{index}"),
                &counts,
                &counts,
            );
            append_unit(
                &mut rows,
                &states,
                "perturbation",
                &format!("treated-{index}"),
                &counts,
                &counts,
            );
        }
        let analysis = analyze_glioma_lineage_dynamics(&analysis_request(&states), &rows).unwrap();
        let mes_index = states.iter().position(|state| *state == "mes").unwrap();
        assert!(analysis.units.iter().all(|unit| {
            unit.states[mes_index].baseline_cell_count == 1
                && unit.states[mes_index].baseline_share_ppm == 0
        }));
        let strata = states
            .iter()
            .map(|state| assay_stratum(state))
            .collect::<Vec<_>>();
        let priorities = plan_glioma_lineage_guided_state_priorities(
            &analysis,
            &LineageGuidanceWeights {
                support_gap_weight_milli: 1_000,
                uncertainty_weight_milli: 0,
                signal_weight_milli: 0,
            },
            &strata,
        )
        .unwrap();
        let mes = priorities.iter().find(|row| row.state_id == "mes").unwrap();
        assert_eq!(mes.support_gap_milli, 0);
        assert_eq!(mes.control_unit_count, 3);
        assert_eq!(mes.comparison_unit_count, 3);
    }

    #[test]
    fn full_workflow_runs_local_assay_and_preserves_null_result_and_lineage_provenance() {
        let states = ["npc", "mes"];
        let mut rows = Vec::new();
        for index in 0..3 {
            append_unit(
                &mut rows,
                &states,
                "vehicle",
                &format!("control-{index}"),
                &[vec![100, 0], vec![0, 100]],
                &[vec![100, 0], vec![0, 100]],
            );
            append_unit(
                &mut rows,
                &states,
                "perturbation",
                &format!("treated-{index}"),
                &[vec![100, 0], vec![0, 100]],
                &[vec![100, 0], vec![0, 300]],
            );
        }
        let request = LineageGuidedAssayWorkflowRequest {
            lineage_analysis: analysis_request(&states),
            assay_campaign: campaign_request(GliomaModelSystem::Organoid),
            guidance_weights: LineageGuidanceWeights {
                support_gap_weight_milli: 400,
                uncertainty_weight_milli: 400,
                signal_weight_milli: 200,
            },
        };
        let strata = states
            .iter()
            .map(|state| assay_stratum(state))
            .collect::<Vec<_>>();
        let candidates = states
            .iter()
            .map(|state| candidate(state))
            .collect::<Vec<_>>();
        let mut executor = NullExecutor(VecDeque::from(["null".into()]));
        let run = execute_glioma_lineage_guided_assay_workflow(
            &request,
            &rows,
            &strata,
            &candidates,
            &[],
            &mut executor,
        )
        .unwrap();
        assert_eq!(run.lineage_analysis.units.len(), 6);
        assert_eq!(run.assay_campaign.rounds.len(), 1);
        assert_eq!(run.assay_campaign.negative_result_order.len(), 1);
        assert_eq!(run.assay_campaign.out_of_model_order.len(), 0);
        assert_eq!(run.assay_campaign.budget_remaining_units, 4);
        assert_eq!(
            run.priorities
                .iter()
                .map(|row| u32::from(row.priority_weight_milli))
                .sum::<u32>(),
            1_000
        );
        run.validate().unwrap();
    }
}
