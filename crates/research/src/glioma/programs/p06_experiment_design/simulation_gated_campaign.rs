//! Adaptive glioma assay campaigns connected to P07 protocol simulation and P08 execution.
//!
//! Candidate workflows are simulated before P06 can select them. A selected workflow then goes
//! through the existing P08 authorization, calibration, interlock, timing, and live-manifest
//! checks. Instrument artifacts become scientific outcomes only through an institution-local
//! outcome interpreter. This controller does not grant instrument authority or infer biology
//! from a successful device operation.

use super::adaptive_information_campaign::AdaptiveInformationObservation;
use super::information_design::DesignAction;
use super::lineage_acquisition_design::{
    LineagePropagationAcquisitionPlan, LineagePropagationAcquisitionPolicy,
    LineagePropagationObservedAssay, LineagePropagationResponseDependence,
};
use super::state_stratified_campaign::{
    execute_glioma_state_stratified_campaign, CandidateSpecificAcquisitionPriority,
    CandidateSpecificOutcomeUpdate, GliomaResearchStratum, GliomaStateStratifiedAssayExecutor,
    StateStratifiedCampaign, StateStratifiedCampaignError, StateStratifiedCampaignRequest,
    StateStratifiedExecutionFailure, StratifiedAssayCandidate, StratifiedAssayObservation,
};
use crate::glioma::programs::p07_protocol_simulation::{
    simulate_glioma_protocol, ProtocolDisposition, ProtocolSimulation,
};
use crate::glioma::programs::p08_instrument_robotics::{
    execute_glioma_simulation_gated_instrument_workflow, InstrumentProtocolGateway,
    SimulationGatedInstrumentWorkflowDisposition, SimulationGatedInstrumentWorkflowRequest,
    SimulationGatedInstrumentWorkflowRun,
};
use crate::glioma_engine::LocalArtifactRef;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

const FEATURE_ID: &str = super::adaptive_information_campaign::FEATURE_ID;
pub const OUTPUT_SCHEMA: &str = "GliomaSimulationGatedAssayCampaign1@9";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimulationGatedAssayRoute {
    pub stratum_id: String,
    pub action_id: String,
    /// This exact local plan is screened before selection and revalidated by P08 at dispatch.
    pub workflow: SimulationGatedInstrumentWorkflowRequest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssayRouteDisposition {
    Ready,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssayRouteReadiness {
    pub stratum_id: String,
    pub action_id: String,
    pub simulation: Option<ProtocolSimulation>,
    pub disposition: AssayRouteDisposition,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutedGliomaAssayRoute {
    pub stratum_id: String,
    pub action_id: String,
    pub workflow: SimulationGatedInstrumentWorkflowRun,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SimulationGatedAssayCampaignDisposition {
    Completed,
    Partial,
    Blocked,
}

/// Closed-loop result: P07 readiness, selected P08 executions, and the P06 scientific campaign.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SimulationGatedAssayCampaignRun {
    pub feature_id: String,
    pub output_schema: String,
    pub route_readiness: Vec<AssayRouteReadiness>,
    pub executions: Vec<ExecutedGliomaAssayRoute>,
    /// Absent when a stratum has no P07-feasible physical assay route; nothing was dispatched.
    pub campaign: Option<StateStratifiedCampaign>,
    /// Final sequential ranking and posterior particle weights; pass these weights to resume.
    pub lineage_acquisition_plan: Option<LineagePropagationAcquisitionPlan>,
    pub disposition: SimulationGatedAssayCampaignDisposition,
    pub next_step: String,
    pub digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InterpretedGliomaAssayOutcome {
    pub action_id: String,
    pub outcome_id: String,
    pub replicate_index: u16,
    pub artifact: LocalArtifactRef,
}

/// Local, domain-specific interpretation of real instrument output. Implementations own assay
/// calibration, QC, classifier versions, and outcome-bin mapping. They must not turn missing or
/// failed measurements into a negative result or accept synthetic dry-run artifacts as evidence.
pub trait GliomaInstrumentOutcomeInterpreter {
    fn interpret(
        &mut self,
        stratum_id: &str,
        action: &DesignAction,
        workflow: &SimulationGatedInstrumentWorkflowRun,
    ) -> Result<InterpretedGliomaAssayOutcome, String>;
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SimulationGatedAssayCampaignError {
    #[error("simulation-gated assay campaign request is invalid: {0}")]
    InvalidRequest(String),
    #[error("simulation-gated assay campaign failed: {0}")]
    Campaign(String),
    #[error("simulation-gated assay campaign output is invalid: {0}")]
    InvalidOutput(String),
    #[error("simulation-gated assay campaign digest failed: {0}")]
    Digest(String),
}

fn digest_input(run: &SimulationGatedAssayCampaignRun) -> serde_json::Value {
    serde_json::json!({
        "feature_id": run.feature_id,
        "output_schema": run.output_schema,
        "route_readiness": run.route_readiness,
        "executions": run.executions,
        "campaign": run.campaign,
        "lineage_acquisition_plan": run.lineage_acquisition_plan,
        "disposition": run.disposition,
        "next_step": run.next_step,
    })
}

impl SimulationGatedAssayCampaignRun {
    pub fn validate(&self) -> Result<(), SimulationGatedAssayCampaignError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.route_readiness.is_empty()
            || self.route_readiness.windows(2).any(|pair| {
                (&pair[0].stratum_id, &pair[0].action_id)
                    >= (&pair[1].stratum_id, &pair[1].action_id)
            })
            || self.route_readiness.iter().any(|route| {
                route.stratum_id.trim().is_empty()
                    || route.action_id.trim().is_empty()
                    || (route.disposition == AssayRouteDisposition::Ready
                        && (route.simulation.as_ref().is_none_or(|simulation| {
                            simulation.disposition != ProtocolDisposition::Feasible
                        }) || route.reason.is_some()))
                    || (route.disposition == AssayRouteDisposition::Blocked
                        && route
                            .reason
                            .as_ref()
                            .is_none_or(|reason| reason.trim().is_empty()))
            })
            || self.executions.iter().any(|item| {
                item.stratum_id.trim().is_empty()
                    || item.action_id.trim().is_empty()
                    || item.workflow.validate().is_err()
                    || item.workflow.disposition
                        != SimulationGatedInstrumentWorkflowDisposition::Completed
                        && item.workflow.disposition
                            != SimulationGatedInstrumentWorkflowDisposition::Negative
            })
            || self.next_step.trim().is_empty()
        {
            return Err(SimulationGatedAssayCampaignError::InvalidOutput(
                "identity, canonical readiness, execution disposition, or next step is invalid"
                    .into(),
            ));
        }
        if let Some(campaign) = &self.campaign {
            campaign.validate().map_err(|error| {
                SimulationGatedAssayCampaignError::InvalidOutput(error.to_string())
            })?;
            let selected = campaign
                .rounds
                .iter()
                .map(|round| (round.stratum_id.as_str(), round.action_id.as_str()))
                .collect::<Vec<_>>();
            let executed = self
                .executions
                .iter()
                .map(|item| (item.stratum_id.as_str(), item.action_id.as_str()))
                .collect::<Vec<_>>();
            let executions_match_readiness = self.executions.iter().all(|item| {
                self.route_readiness.iter().any(|ready| {
                    ready.stratum_id == item.stratum_id
                        && ready.action_id == item.action_id
                        && ready.disposition == AssayRouteDisposition::Ready
                        && ready
                            .simulation
                            .as_ref()
                            .map(|simulation| &simulation.digest)
                            == Some(&item.workflow.simulation.digest)
                })
            });
            if selected != executed || !executions_match_readiness {
                return Err(SimulationGatedAssayCampaignError::InvalidOutput(
                    "selected P06 assays, P08 executions, and P07 readiness simulations do not match in order".into(),
                ));
            }
        } else if !self.executions.is_empty()
            || self.disposition != SimulationGatedAssayCampaignDisposition::Blocked
        {
            return Err(SimulationGatedAssayCampaignError::InvalidOutput(
                "a campaign-less result must be blocked and have no physical executions".into(),
            ));
        }
        if let Some(plan) = &self.lineage_acquisition_plan {
            plan.validate().map_err(|error| {
                SimulationGatedAssayCampaignError::InvalidOutput(error.to_string())
            })?;
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| SimulationGatedAssayCampaignError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(SimulationGatedAssayCampaignError::InvalidOutput(
                "campaign digest mismatch".into(),
            ));
        }
        Ok(())
    }
}

fn seal_run(
    route_readiness: Vec<AssayRouteReadiness>,
    executions: Vec<ExecutedGliomaAssayRoute>,
    campaign: Option<StateStratifiedCampaign>,
    lineage_acquisition_plan: Option<LineagePropagationAcquisitionPlan>,
    disposition: SimulationGatedAssayCampaignDisposition,
    next_step: String,
) -> Result<SimulationGatedAssayCampaignRun, SimulationGatedAssayCampaignError> {
    let mut run = SimulationGatedAssayCampaignRun {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        route_readiness,
        executions,
        campaign,
        lineage_acquisition_plan,
        disposition,
        next_step,
        digest: ContentHash::of_bytes(b"pending"),
    };
    run.digest = ContentHash::of_value(&digest_input(&run))
        .map_err(|error| SimulationGatedAssayCampaignError::Digest(error.to_string()))?;
    run.validate()?;
    Ok(run)
}

struct InstrumentCampaignExecutor<'a, G, I> {
    gateway: &'a mut G,
    interpreter: &'a mut I,
    routes: BTreeMap<(String, String), SimulationGatedInstrumentWorkflowRequest>,
    executions: Vec<ExecutedGliomaAssayRoute>,
    acquisition_policy: Option<LineagePropagationAcquisitionPolicy>,
}

impl<G, I> GliomaStateStratifiedAssayExecutor for InstrumentCampaignExecutor<'_, G, I>
where
    G: InstrumentProtocolGateway,
    I: GliomaInstrumentOutcomeInterpreter,
{
    fn execute_action(
        &mut self,
        stratum_id: &str,
        action: &DesignAction,
        _round: u16,
    ) -> Result<AdaptiveInformationObservation, StateStratifiedExecutionFailure> {
        let key = (stratum_id.to_owned(), action.action_id.clone());
        let request = self
            .routes
            .get(&key)
            .ok_or_else(|| StateStratifiedExecutionFailure {
                reason: "selected assay has no admitted P07/P08 route".into(),
                retryable: false,
            })?;
        if self.gateway.simulation_only() {
            return Err(StateStratifiedExecutionFailure {
                reason: "simulation-only gateway output cannot update a scientific posterior"
                    .into(),
                retryable: false,
            });
        }
        let workflow =
            execute_glioma_simulation_gated_instrument_workflow(request, &mut *self.gateway)
                .map_err(|error| StateStratifiedExecutionFailure {
                    reason: error.to_string(),
                    retryable: false,
                })?;
        if !matches!(
            workflow.disposition,
            SimulationGatedInstrumentWorkflowDisposition::Completed
                | SimulationGatedInstrumentWorkflowDisposition::Negative
        ) {
            return Err(StateStratifiedExecutionFailure {
                reason: workflow.stop_detail.clone().unwrap_or_else(|| {
                    format!("P08 workflow stopped as {:?}", workflow.disposition)
                }),
                retryable: false,
            });
        }
        let interpreted = self
            .interpreter
            .interpret(stratum_id, action, &workflow)
            .map_err(|reason| StateStratifiedExecutionFailure {
                reason: format!("local assay interpretation failed: {reason}"),
                retryable: false,
            })?;
        let declared_outcome = action
            .outcomes
            .iter()
            .any(|outcome| outcome.outcome_id == interpreted.outcome_id);
        let produced_artifact = workflow.execution.as_ref().is_some_and(|execution| {
            execution
                .results
                .iter()
                .any(|result| result.artifact.as_ref() == Some(&interpreted.artifact))
        });
        interpreted
            .artifact
            .validate()
            .map_err(|error| StateStratifiedExecutionFailure {
                reason: format!("interpreted artifact is invalid: {error}"),
                retryable: false,
            })?;
        if interpreted.action_id != action.action_id
            || interpreted.replicate_index == 0
            || !declared_outcome
            || !produced_artifact
            || !interpreted.artifact.local_only
            || interpreted.artifact.contains_human_data
            || interpreted.artifact.contains_direct_identifiers
        {
            return Err(StateStratifiedExecutionFailure {
                reason: "interpreter output must map to a declared assay outcome and an exact local P08 artifact".into(),
                retryable: false,
            });
        }
        let observation = AdaptiveInformationObservation {
            action_id: interpreted.action_id,
            outcome_id: interpreted.outcome_id,
            replicate_index: interpreted.replicate_index,
            artifact: interpreted.artifact,
        };
        self.executions.push(ExecutedGliomaAssayRoute {
            stratum_id: stratum_id.into(),
            action_id: action.action_id.clone(),
            workflow,
        });
        Ok(observation)
    }

    fn uses_candidate_specific_acquisition(&self) -> bool {
        self.acquisition_policy.is_some()
    }

    fn candidate_specific_acquisition_priority(
        &self,
        stratum_id: &str,
        action: &DesignAction,
    ) -> Option<CandidateSpecificAcquisitionPriority> {
        let candidate = StratifiedAssayCandidate {
            stratum_id: stratum_id.into(),
            action: action.clone(),
            negative_outcome_order: Vec::new(),
        };
        let score = self.acquisition_policy.as_ref()?.score(&candidate)?;
        Some(CandidateSpecificAcquisitionPriority {
            horizon_assays: 1,
            expected_variance_reduction_milli: score.expected_variance_reduction_milli,
            reduction_per_cost_million: score.expected_realized_reduction_per_cost_million,
            minimum_predicted_effective_sample_fraction_milli: score
                .minimum_predicted_effective_sample_fraction_milli,
        })
    }

    fn candidate_specific_acquisition_priorities(
        &self,
        candidates: &[StratifiedAssayCandidate],
        remaining_budget_units: u64,
        remaining_rounds: u16,
    ) -> Result<BTreeMap<(String, String), CandidateSpecificAcquisitionPriority>, String> {
        self.acquisition_policy
            .as_ref()
            .ok_or_else(|| "lineage acquisition policy is not configured".to_string())?
            .candidate_specific_priorities_with_lookahead(
                candidates,
                remaining_budget_units,
                remaining_rounds,
            )
            .map_err(|error| error.to_string())
    }

    fn assimilate_candidate_specific_outcome(
        &mut self,
        _stratum_id: &str,
        action: &DesignAction,
        outcome_id: &str,
    ) -> Result<CandidateSpecificOutcomeUpdate, String> {
        let Some(policy) = self.acquisition_policy.as_mut() else {
            return Ok(CandidateSpecificOutcomeUpdate::Updated);
        };
        match policy.assimilate_outcome(&action.action_id, outcome_id) {
            Ok(()) => Ok(CandidateSpecificOutcomeUpdate::Updated),
            Err(error) => Ok(CandidateSpecificOutcomeUpdate::Unresolved {
                reason: error.to_string(),
            }),
        }
    }
}

/// Screen each declared physical assay route, then run an adaptive P06 campaign over only routes
/// that can meet P07's schedule/risk/capacity gate. Selected routes are revalidated and executed by
/// P08; an outcome interpreter must bind each posterior update to one of the exact local artifacts
/// returned by that P08 execution. Each physical route is single-use (`max_replicates == 1`) so a
/// repeated campaign call cannot silently reuse an instrument action identity.
pub fn execute_glioma_simulation_gated_assay_campaign<G, I>(
    request: &StateStratifiedCampaignRequest,
    strata: &[GliomaResearchStratum],
    candidates: &[StratifiedAssayCandidate],
    initial_observations: &[StratifiedAssayObservation],
    routes: &[SimulationGatedAssayRoute],
    gateway: &mut G,
    interpreter: &mut I,
) -> Result<SimulationGatedAssayCampaignRun, SimulationGatedAssayCampaignError>
where
    G: InstrumentProtocolGateway,
    I: GliomaInstrumentOutcomeInterpreter,
{
    execute_glioma_simulation_gated_assay_campaign_inner(
        request,
        strata,
        candidates,
        initial_observations,
        routes,
        None,
        gateway,
        interpreter,
    )
}

/// Run the same P06/P07/P08 closed loop while ranking each feasible candidate by calibrated
/// expected reduction in P10 lineage-operator uncertainty per local assay cost. Candidates whose
/// plausible outcomes would leave too little effective bootstrap support are withheld.
pub fn execute_glioma_simulation_gated_assay_campaign_with_lineage_acquisition<G, I>(
    request: &StateStratifiedCampaignRequest,
    strata: &[GliomaResearchStratum],
    candidates: &[StratifiedAssayCandidate],
    initial_observations: &[StratifiedAssayObservation],
    routes: &[SimulationGatedAssayRoute],
    acquisition_policy: LineagePropagationAcquisitionPolicy,
    gateway: &mut G,
    interpreter: &mut I,
) -> Result<SimulationGatedAssayCampaignRun, SimulationGatedAssayCampaignError>
where
    G: InstrumentProtocolGateway,
    I: GliomaInstrumentOutcomeInterpreter,
{
    execute_glioma_simulation_gated_assay_campaign_inner(
        request,
        strata,
        candidates,
        initial_observations,
        routes,
        Some(acquisition_policy),
        gateway,
        interpreter,
    )
}

fn execute_glioma_simulation_gated_assay_campaign_inner<G, I>(
    request: &StateStratifiedCampaignRequest,
    strata: &[GliomaResearchStratum],
    candidates: &[StratifiedAssayCandidate],
    initial_observations: &[StratifiedAssayObservation],
    routes: &[SimulationGatedAssayRoute],
    acquisition_policy: Option<LineagePropagationAcquisitionPolicy>,
    gateway: &mut G,
    interpreter: &mut I,
) -> Result<SimulationGatedAssayCampaignRun, SimulationGatedAssayCampaignError>
where
    G: InstrumentProtocolGateway,
    I: GliomaInstrumentOutcomeInterpreter,
{
    if candidates.is_empty() || routes.len() != candidates.len() {
        return Err(SimulationGatedAssayCampaignError::InvalidRequest(
            "every candidate must have exactly one declared P07/P08 route".into(),
        ));
    }
    if let Some(policy) = acquisition_policy.as_ref() {
        if policy.response_dependence()
            == LineagePropagationResponseDependence::FirstOrderPairCalibration
        {
            let supplied_history = initial_observations
                .iter()
                .map(|observation| LineagePropagationObservedAssay {
                    action_id: observation.action_id.clone(),
                    outcome_id: observation.outcome_id.clone(),
                })
                .collect::<Vec<_>>();
            if supplied_history != policy.response_history() {
                return Err(SimulationGatedAssayCampaignError::InvalidRequest(
                    "joint-response policy history must exactly match initial observations in execution order".into(),
                ));
            }
        }
    }
    let candidates_by_key = candidates
        .iter()
        .map(|candidate| {
            (
                (
                    candidate.stratum_id.as_str(),
                    candidate.action.action_id.as_str(),
                ),
                candidate,
            )
        })
        .collect::<BTreeMap<_, _>>();
    if candidates_by_key.len() != candidates.len()
        || candidates
            .iter()
            .any(|candidate| candidate.action.max_replicates != 1)
    {
        return Err(SimulationGatedAssayCampaignError::InvalidRequest(
            "candidate routes must be unique and physical assays must use a single-use action identity".into(),
        ));
    }
    let mut routes_by_key = BTreeMap::new();
    for route in routes {
        let key = (route.stratum_id.as_str(), route.action_id.as_str());
        let candidate = candidates_by_key.get(&key).ok_or_else(|| {
            SimulationGatedAssayCampaignError::InvalidRequest(
                "a P07/P08 route has no matching P06 assay candidate".into(),
            )
        })?;
        if routes_by_key
            .insert((route.stratum_id.clone(), route.action_id.clone()), route)
            .is_some()
            || route.workflow.protocol.objective != request.campaign.objective
            || route.workflow.protocol.model_system != request.campaign.model_system
            || route.workflow.protocol_manifest.instrument_id
                != route.workflow.instrument_preflight.instrument_id
            || route.workflow.protocol.tasks.is_empty()
            || route
                .workflow
                .protocol
                .tasks
                .iter()
                .any(|task| task.model_system != request.campaign.model_system)
            || candidate.action.outcomes.is_empty()
        {
            return Err(SimulationGatedAssayCampaignError::InvalidRequest(
                "route identity, objective, model system, instrument scope, or outcome model is inconsistent".into(),
            ));
        }
    }

    let mut readiness = Vec::with_capacity(routes.len());
    let mut executable_routes = BTreeMap::new();
    for route in routes {
        let protocol = &route.workflow.protocol;
        let (simulation, reason) = match simulate_glioma_protocol(protocol) {
            Ok(simulation) if simulation.disposition == ProtocolDisposition::Feasible => {
                let has_instrument_task =
                    protocol.tasks.iter().any(|task| task.requires_instrument);
                if has_instrument_task {
                    (Some(simulation), None)
                } else {
                    (
                        Some(simulation),
                        Some("P07 plan has no instrument-backed assay task".into()),
                    )
                }
            }
            Ok(simulation) => {
                let reason = format!("P07 disposition is {:?}", simulation.disposition);
                (Some(simulation), Some(reason))
            }
            Err(error) => (
                None,
                Some(format!("P07 simulation rejected the route: {error}")),
            ),
        };
        let disposition = if reason.is_none() {
            executable_routes.insert(
                (route.stratum_id.clone(), route.action_id.clone()),
                route.workflow.clone(),
            );
            AssayRouteDisposition::Ready
        } else {
            AssayRouteDisposition::Blocked
        };
        readiness.push(AssayRouteReadiness {
            stratum_id: route.stratum_id.clone(),
            action_id: route.action_id.clone(),
            simulation,
            disposition,
            reason,
        });
    }
    readiness.sort_by(|left, right| {
        (&left.stratum_id, &left.action_id).cmp(&(&right.stratum_id, &right.action_id))
    });

    let strata_with_ready_routes = executable_routes
        .keys()
        .map(|(stratum_id, _)| stratum_id.as_str())
        .collect::<BTreeSet<_>>();
    if strata
        .iter()
        .any(|stratum| !strata_with_ready_routes.contains(stratum.stratum_id.as_str()))
    {
        return seal_run(
            readiness,
            Vec::new(),
            None,
            None,
            SimulationGatedAssayCampaignDisposition::Blocked,
            "At least one declared biological stratum has no P07-feasible instrument route; repair its protocol or scope before any campaign execution.".into(),
        );
    }

    // Keep historical candidates needed by supplied observations, but never schedule a blocked
    // candidate for new work. P06 rejects invalid history and enforces replicate/budget limits.
    let observed_ids = initial_observations
        .iter()
        .map(|observation| observation.action_id.as_str())
        .collect::<BTreeSet<_>>();
    let eligible_candidates = candidates
        .iter()
        .filter(|candidate| {
            executable_routes.contains_key(&(
                candidate.stratum_id.clone(),
                candidate.action.action_id.clone(),
            )) || observed_ids.contains(candidate.action.action_id.as_str())
        })
        .cloned()
        .collect::<Vec<_>>();
    let mut executor = InstrumentCampaignExecutor {
        gateway,
        interpreter,
        routes: executable_routes,
        executions: Vec::new(),
        acquisition_policy,
    };
    let campaign = execute_glioma_state_stratified_campaign(
        request,
        strata,
        &eligible_candidates,
        initial_observations,
        &mut executor,
    )
    .map_err(|error: StateStratifiedCampaignError| {
        SimulationGatedAssayCampaignError::Campaign(error.to_string())
    })?;
    let disposition = match campaign.disposition {
        super::state_stratified_campaign::StateStratifiedDisposition::Completed => {
            SimulationGatedAssayCampaignDisposition::Completed
        }
        super::state_stratified_campaign::StateStratifiedDisposition::Partial
        | super::state_stratified_campaign::StateStratifiedDisposition::BudgetExhausted => {
            SimulationGatedAssayCampaignDisposition::Partial
        }
        super::state_stratified_campaign::StateStratifiedDisposition::EvidenceBlocked
        | super::state_stratified_campaign::StateStratifiedDisposition::Unresolved => {
            SimulationGatedAssayCampaignDisposition::Blocked
        }
    };
    let next_step = campaign.next_step.clone();
    let lineage_acquisition_plan = executor
        .acquisition_policy
        .as_ref()
        .map(|policy| policy.plan(candidates))
        .transpose()
        .map_err(|error| SimulationGatedAssayCampaignError::InvalidOutput(error.to_string()))?;
    seal_run(
        readiness,
        executor.executions,
        Some(campaign),
        lineage_acquisition_plan,
        disposition,
        next_step,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p06_experiment_design::adaptive_information_campaign::AdaptiveInformationCampaignRequest;
    use crate::glioma::programs::p06_experiment_design::information_design::{
        DesignMechanism, DesignOutcome,
    };
    use crate::glioma::programs::p06_experiment_design::lineage_acquisition_design::{
        LineagePropagationAcquisitionPolicy, LineagePropagationAcquisitionTarget,
        LineagePropagationResponseDependence,
    };
    use crate::glioma::programs::p06_experiment_design::lineage_response_calibration::{
        calibrate_glioma_lineage_assay_response_model,
        calibrate_glioma_lineage_joint_assay_response_model, LineageAssayCalibrationObservation,
        LineageAssayJointCalibrationObservation, LineageAssayJointResponseCalibrationDisposition,
        LineageAssayJointResponseCalibrationRequest, LineageAssayResponseCalibrationDisposition,
        LineageAssayResponseCalibrationRequest,
    };
    use crate::glioma::programs::p06_experiment_design::state_plasticity_instrument_campaign::{
        execute_glioma_lineage_response_guided_state_plasticity_campaign,
        GliomaStatePlasticityInstrumentInputs, GliomaStatePlasticityInstrumentRequest,
    };
    use crate::glioma::programs::p06_experiment_design::transition_guided_campaign::{
        TransitionGuidanceRequest, TransitionGuidedStateCampaignRequest,
    };
    use crate::glioma::programs::p07_protocol_simulation::{
        ProtocolResource, ProtocolResourceKind, ProtocolSimulationRequest, ProtocolTask,
    };
    use crate::glioma::programs::p08_instrument_robotics::calibration::{
        analyze_instrument_calibration, CalibrationDisposition, CalibrationRequest, CalibrationRun,
        InstrumentCalibration,
    };
    use crate::glioma::programs::p08_instrument_robotics::execution::{
        InstrumentExecutionDisposition, InstrumentExecutionFailure, InstrumentExecutionResult,
    };
    use crate::glioma::programs::p08_instrument_robotics::preflight::{
        InstrumentAction, InstrumentOperation, InstrumentPreflightRequest,
    };
    use crate::glioma::programs::p08_instrument_robotics::preflight::{
        InstrumentAuthorization, InstrumentInterlockSnapshot,
    };
    use crate::glioma::programs::p08_instrument_robotics::protocol_binding::{
        BoundInstrumentCommand, InstrumentProtocolGateway,
    };
    use crate::glioma::programs::p08_instrument_robotics::protocol_binding::{
        InstrumentCommandCapability, InstrumentControlProtocol, InstrumentProtocolManifest,
    };
    use crate::glioma::programs::p10_interpretation_replication::lineage_propagation::{
        analyze_glioma_lineage_propagation, LineagePropagationAnalysis,
        LineagePropagationDisposition, LineagePropagationRequest, LineagePropagationSnapshot,
    };
    use crate::glioma::programs::p10_interpretation_replication::lineage_response_decomposition::LineageResponseDecompositionRequest;
    use crate::glioma::programs::p10_interpretation_replication::state_transition::{
        StateTransitionObservation, StateTransitionRequest,
    };
    use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
    use bioprism_ids::ContentHash;

    fn campaign_request() -> StateStratifiedCampaignRequest {
        StateStratifiedCampaignRequest {
            campaign: AdaptiveInformationCampaignRequest {
                objective: "measure preclinical glioma organoid response".into(),
                model_system: GliomaModelSystem::Organoid,
                max_rounds: 1,
                max_actions_per_round: 1,
                budget_units: 10,
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

    fn candidate() -> StratifiedAssayCandidate {
        StratifiedAssayCandidate {
            stratum_id: "mesenchymal".into(),
            action: DesignAction {
                action_id: "assay-mesenchymal".into(),
                feature_id: "glioma-state-assay".into(),
                label: "measure a declared organoid response".into(),
                outcomes: vec![
                    DesignOutcome {
                        outcome_id: "null".into(),
                        label: "no measured response".into(),
                        probability_milli_by_mechanism: [("m1".into(), 100), ("m2".into(), 900)]
                            .into(),
                    },
                    DesignOutcome {
                        outcome_id: "signal".into(),
                        label: "declared response".into(),
                        probability_milli_by_mechanism: [("m1".into(), 900), ("m2".into(), 100)]
                            .into(),
                    },
                ],
                feasibility_milli: 1_000,
                risk_milli: 0,
                cost_units: 1,
                max_replicates: 1,
            },
            negative_outcome_order: vec!["null".into()],
        }
    }

    fn blocked_route() -> SimulationGatedAssayRoute {
        let objective = "measure preclinical glioma organoid response";
        let instrument_id = "local-microscope";
        let protocol = ProtocolSimulationRequest {
            objective: objective.into(),
            model_system: GliomaModelSystem::Organoid,
            tasks: vec![ProtocolTask {
                task_id: "image-capture".into(),
                label: "capture organoid assay image".into(),
                resource_kind: ProtocolResourceKind::Imaging,
                resource_units: 1,
                duration_ticks: 2,
                depends_on: Vec::new(),
                model_system: GliomaModelSystem::Organoid,
                output_schema: "GliomaAssayImage1@1".into(),
                risk_milli: 10,
                requires_instrument: true,
            }],
            resources: vec![ProtocolResource {
                resource_id: "microscope-slot".into(),
                kind: ProtocolResourceKind::Imaging,
                capacity_units: 1,
            }],
            max_ticks: 10,
            max_risk_milli: 0,
            allow_instrument_execution: true,
            approval_reference: Some("approval-1".into()),
            randomization_seed: ContentHash::of_bytes(b"simulation-gated-test"),
        };
        let calibration = InstrumentCalibration {
            feature_id: "GAF-GLIOMA-P08-F02".into(),
            output_schema: "GliomaInstrumentCalibration1@1".into(),
            objective: objective.into(),
            instrument_id: instrument_id.into(),
            model_system: GliomaModelSystem::Organoid,
            metric_name: "image-focus".into(),
            run_order: Vec::new(),
            reference_order: Vec::new(),
            points: Vec::new(),
            reference_residual_median_milli: 0,
            reference_mad_milli: 0,
            final_drift_milli: 0,
            max_abs_drift_milli: 0,
            slope_milli_per_tick: 0,
            negative_evidence: Vec::new(),
            uncertainty: Vec::new(),
            disposition: CalibrationDisposition::Unresolved,
            digest: ContentHash::of_bytes(b"unused-calibration"),
        };
        SimulationGatedAssayRoute {
            stratum_id: "mesenchymal".into(),
            action_id: "assay-mesenchymal".into(),
            workflow: SimulationGatedInstrumentWorkflowRequest {
                protocol,
                simulation_epoch_tick: 0,
                instrument_preflight: InstrumentPreflightRequest {
                    objective: objective.into(),
                    instrument_id: instrument_id.into(),
                    model_system: GliomaModelSystem::Organoid,
                    actions: vec![InstrumentAction {
                        action_id: "image-capture".into(),
                        instrument_id: instrument_id.into(),
                        operation: InstrumentOperation::AcquireImage,
                        model_system: GliomaModelSystem::Organoid,
                        requested_start_tick: 0,
                        duration_ticks: 2,
                        risk_milli: 10,
                        requires_operator: true,
                        output_schema: "GliomaAssayImage1@1".into(),
                        parameters: Vec::new(),
                    }],
                    calibration,
                    interlocks: InstrumentInterlockSnapshot {
                        observed_tick: 0,
                        emergency_stop_clear: true,
                        guard_closed: true,
                        deck_clear: true,
                        consumables_available: true,
                        waste_capacity_milli: 1_000,
                        temperature_milli: Some(37_000),
                        minimum_temperature_milli: Some(36_000),
                        maximum_temperature_milli: Some(38_000),
                        calibration_valid_until_tick: 100,
                        calibration_sequence_index: 3,
                    },
                    authorization: InstrumentAuthorization {
                        authorization_id: "approval-1".into(),
                        operator_id: "local-operator".into(),
                        instrument_scope: instrument_id.into(),
                        approval_digest: ContentHash::of_bytes(b"approval"),
                        issued_tick: 0,
                        expires_tick: 100,
                        revoked: false,
                    },
                    current_tick: 0,
                    maximum_total_risk_milli: 1_000,
                    maximum_duration_ticks: 10,
                    minimum_waste_capacity_milli: 0,
                },
                completed_prerequisite_artifacts: Vec::new(),
                protocol_manifest: InstrumentProtocolManifest {
                    instrument_id: instrument_id.into(),
                    protocol: InstrumentControlProtocol::Sila2,
                    protocol_version: "1.0".into(),
                    manifest_id: "manifest-1".into(),
                    revision: 1,
                    commands: Vec::new(),
                },
                max_retries: 0,
                require_artifacts: true,
            },
        }
    }

    #[derive(Default)]
    struct NoDispatchGateway {
        calls: usize,
    }

    impl InstrumentProtocolGateway for NoDispatchGateway {
        fn observe_interlocks(
            &mut self,
        ) -> Result<InstrumentInterlockSnapshot, InstrumentExecutionFailure> {
            self.calls += 1;
            unreachable!("blocked P07 protocol must not reach P08")
        }

        fn verify_authorization(
            &mut self,
            _authorization: &InstrumentAuthorization,
        ) -> Result<(), InstrumentExecutionFailure> {
            self.calls += 1;
            unreachable!("blocked P07 protocol must not reach P08")
        }

        fn current_manifest(
            &mut self,
            _instrument_id: &str,
        ) -> Result<InstrumentProtocolManifest, InstrumentExecutionFailure> {
            self.calls += 1;
            unreachable!("blocked P07 protocol must not reach P08")
        }

        fn execute_bound_command(
            &mut self,
            _command: &BoundInstrumentCommand,
            _attempt: u8,
        ) -> Result<InstrumentExecutionResult, InstrumentExecutionFailure> {
            self.calls += 1;
            unreachable!("blocked P07 protocol must not reach P08")
        }

        fn emergency_stop(&mut self) -> Result<(), InstrumentExecutionFailure> {
            self.calls += 1;
            unreachable!("blocked P07 protocol must not reach P08")
        }
    }

    struct UnusedInterpreter;

    impl GliomaInstrumentOutcomeInterpreter for UnusedInterpreter {
        fn interpret(
            &mut self,
            _stratum_id: &str,
            _action: &DesignAction,
            _workflow: &SimulationGatedInstrumentWorkflowRun,
        ) -> Result<InterpretedGliomaAssayOutcome, String> {
            Err("no physical assay was run".into())
        }
    }

    #[test]
    fn risk_blocked_protocol_prevents_all_dispatch_before_campaign_selection() {
        let mut gateway = NoDispatchGateway::default();
        let mut interpreter = UnusedInterpreter;
        let result = execute_glioma_simulation_gated_assay_campaign(
            &campaign_request(),
            &[GliomaResearchStratum {
                stratum_id: "mesenchymal".into(),
                label: "mesenchymal-like glioma organoids".into(),
                priority_weight_milli: 1_000,
                mechanism_priors: vec![
                    DesignMechanism {
                        mechanism_id: "m1".into(),
                        prior_milli: 500,
                    },
                    DesignMechanism {
                        mechanism_id: "m2".into(),
                        prior_milli: 500,
                    },
                ],
            }],
            &[candidate()],
            &[],
            &[blocked_route()],
            &mut gateway,
            &mut interpreter,
        )
        .unwrap();

        assert_eq!(
            result.disposition,
            SimulationGatedAssayCampaignDisposition::Blocked
        );
        assert!(result.campaign.is_none());
        assert!(result.executions.is_empty());
        assert_eq!(gateway.calls, 0);
        assert_eq!(result.route_readiness.len(), 1);
        assert_eq!(
            result.route_readiness[0].disposition,
            AssayRouteDisposition::Blocked
        );
        assert_eq!(
            result.route_readiness[0]
                .simulation
                .as_ref()
                .unwrap()
                .disposition,
            ProtocolDisposition::RiskBlocked
        );
        result.validate().unwrap();
    }

    fn local_artifact(id: &str, content_type: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: id.into(),
            content_hash: ContentHash::of_bytes(id.as_bytes()),
            content_type: content_type.into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn qualified_calibration(instrument_id: &str) -> InstrumentCalibration {
        let runs = (1..=3)
            .map(|index| CalibrationRun {
                run_id: format!("cal-{index}"),
                sequence_index: index,
                batch_id: format!("reference-{index}"),
                instrument_id: instrument_id.into(),
                metric_name: "image-reference".into(),
                model_system: GliomaModelSystem::Organoid,
                observed_milli: 1_000,
                expected_milli: 1_000,
                artifact: local_artifact(
                    &format!("calibration-{index}"),
                    "application/vnd.aurora.glioma.calibration+json",
                ),
            })
            .collect::<Vec<_>>();
        analyze_instrument_calibration(
            &CalibrationRequest {
                objective: "qualify local glioma imager".into(),
                instrument_id: instrument_id.into(),
                model_system: GliomaModelSystem::Organoid,
                metric_name: "image-reference".into(),
                minimum_runs: 3,
                reference_run_count: 2,
                max_reference_mad_milli: 100,
                max_drift_milli: 100,
                max_slope_milli_per_tick: 100,
            },
            &runs,
        )
        .unwrap()
    }

    fn ready_route() -> SimulationGatedAssayRoute {
        let objective = "measure preclinical glioma organoid response";
        let instrument_id = "local-microscope";
        let protocol = ProtocolSimulationRequest {
            objective: objective.into(),
            model_system: GliomaModelSystem::Organoid,
            tasks: vec![ProtocolTask {
                task_id: "image-assay".into(),
                label: "capture a preclinical organoid assay image".into(),
                resource_kind: ProtocolResourceKind::Imaging,
                resource_units: 1,
                duration_ticks: 2,
                depends_on: Vec::new(),
                model_system: GliomaModelSystem::Organoid,
                output_schema: "GliomaAssayImage1@1".into(),
                risk_milli: 10,
                requires_instrument: true,
            }],
            resources: vec![ProtocolResource {
                resource_id: "imager-slot".into(),
                kind: ProtocolResourceKind::Imaging,
                capacity_units: 1,
            }],
            max_ticks: 10,
            max_risk_milli: 100,
            allow_instrument_execution: true,
            approval_reference: Some("operator-approval-1".into()),
            randomization_seed: ContentHash::of_bytes(b"glioma-state-assay"),
        };
        let simulation = simulate_glioma_protocol(&protocol).unwrap();
        let schedule = &simulation.schedule[0];
        SimulationGatedAssayRoute {
            stratum_id: "mesenchymal".into(),
            action_id: "assay-mesenchymal".into(),
            workflow: SimulationGatedInstrumentWorkflowRequest {
                protocol,
                simulation_epoch_tick: 100,
                instrument_preflight: InstrumentPreflightRequest {
                    objective: objective.into(),
                    instrument_id: instrument_id.into(),
                    model_system: GliomaModelSystem::Organoid,
                    actions: vec![InstrumentAction {
                        action_id: "image-assay".into(),
                        instrument_id: instrument_id.into(),
                        operation: InstrumentOperation::AcquireImage,
                        model_system: GliomaModelSystem::Organoid,
                        requested_start_tick: 100 + u64::from(schedule.start_tick),
                        duration_ticks: u64::from(schedule.finish_tick - schedule.start_tick),
                        risk_milli: 10,
                        requires_operator: false,
                        output_schema: "GliomaAssayImage1@1".into(),
                        parameters: Vec::new(),
                    }],
                    calibration: qualified_calibration(instrument_id),
                    interlocks: InstrumentInterlockSnapshot {
                        observed_tick: 100,
                        emergency_stop_clear: true,
                        guard_closed: true,
                        deck_clear: true,
                        consumables_available: true,
                        waste_capacity_milli: 1_000,
                        temperature_milli: Some(37_000),
                        minimum_temperature_milli: Some(36_000),
                        maximum_temperature_milli: Some(38_000),
                        calibration_valid_until_tick: 1_000,
                        calibration_sequence_index: 3,
                    },
                    authorization: InstrumentAuthorization {
                        authorization_id: "operator-approval-1".into(),
                        operator_id: "local-operator".into(),
                        instrument_scope: instrument_id.into(),
                        approval_digest: ContentHash::of_bytes(b"approval"),
                        issued_tick: 0,
                        expires_tick: 1_000,
                        revoked: false,
                    },
                    current_tick: 100,
                    maximum_total_risk_milli: 100,
                    maximum_duration_ticks: 10,
                    minimum_waste_capacity_milli: 0,
                },
                completed_prerequisite_artifacts: Vec::new(),
                protocol_manifest: InstrumentProtocolManifest {
                    instrument_id: instrument_id.into(),
                    protocol: InstrumentControlProtocol::Sila2,
                    protocol_version: "1.1".into(),
                    manifest_id: "local-imager-manifest".into(),
                    revision: 1,
                    commands: vec![InstrumentCommandCapability {
                        operation: InstrumentOperation::AcquireImage,
                        feature_path: "org.aurora.GliomaImaging".into(),
                        command_id: "AcquireFrame".into(),
                        output_schema: "GliomaAssayImage1@1".into(),
                        retry_idempotent: false,
                        parameters: Vec::new(),
                    }],
                },
                max_retries: 0,
                require_artifacts: true,
            },
        }
    }

    struct MeasurementGateway {
        manifest: InstrumentProtocolManifest,
        interlocks: InstrumentInterlockSnapshot,
        dispatches: Vec<String>,
    }

    impl InstrumentProtocolGateway for MeasurementGateway {
        fn observe_interlocks(
            &mut self,
        ) -> Result<InstrumentInterlockSnapshot, InstrumentExecutionFailure> {
            Ok(self.interlocks.clone())
        }

        fn verify_authorization(
            &mut self,
            authorization: &InstrumentAuthorization,
        ) -> Result<(), InstrumentExecutionFailure> {
            if authorization.authorization_id == "operator-approval-1" {
                Ok(())
            } else {
                Err(InstrumentExecutionFailure {
                    reason: "unexpected authorization".into(),
                    retryable: false,
                })
            }
        }

        fn current_manifest(
            &mut self,
            instrument_id: &str,
        ) -> Result<InstrumentProtocolManifest, InstrumentExecutionFailure> {
            if instrument_id == self.manifest.instrument_id {
                Ok(self.manifest.clone())
            } else {
                Err(InstrumentExecutionFailure {
                    reason: "unexpected instrument".into(),
                    retryable: false,
                })
            }
        }

        fn execute_bound_command(
            &mut self,
            command: &BoundInstrumentCommand,
            attempt: u8,
        ) -> Result<InstrumentExecutionResult, InstrumentExecutionFailure> {
            let artifact_id = format!("measured-image-{}", self.dispatches.len() + 1);
            self.dispatches.push(command.action_id.clone());
            Ok(InstrumentExecutionResult {
                action_id: command.action_id.clone(),
                disposition: InstrumentExecutionDisposition::Completed,
                attempt_count: attempt,
                started_tick: Some(100),
                completed_tick: Some(102),
                artifact: Some(local_artifact(&artifact_id, &command.output_schema)),
                note: "local measurement completed".into(),
                uncertainty: Vec::new(),
                negative_evidence: Vec::new(),
            })
        }

        fn emergency_stop(&mut self) -> Result<(), InstrumentExecutionFailure> {
            Ok(())
        }
    }

    struct SignalInterpreter;

    impl GliomaInstrumentOutcomeInterpreter for SignalInterpreter {
        fn interpret(
            &mut self,
            _stratum_id: &str,
            action: &DesignAction,
            workflow: &SimulationGatedInstrumentWorkflowRun,
        ) -> Result<InterpretedGliomaAssayOutcome, String> {
            let artifact = workflow
                .execution
                .as_ref()
                .and_then(|execution| execution.results.first())
                .and_then(|result| result.artifact.clone())
                .ok_or_else(|| "P08 did not return a measurement artifact".to_string())?;
            Ok(InterpretedGliomaAssayOutcome {
                action_id: action.action_id.clone(),
                outcome_id: "signal".into(),
                replicate_index: 1,
                artifact,
            })
        }
    }

    struct UnboundArtifactInterpreter;

    impl GliomaInstrumentOutcomeInterpreter for UnboundArtifactInterpreter {
        fn interpret(
            &mut self,
            _stratum_id: &str,
            action: &DesignAction,
            _workflow: &SimulationGatedInstrumentWorkflowRun,
        ) -> Result<InterpretedGliomaAssayOutcome, String> {
            Ok(InterpretedGliomaAssayOutcome {
                action_id: action.action_id.clone(),
                outcome_id: "signal".into(),
                replicate_index: 1,
                artifact: local_artifact("unbound-result", "application/json"),
            })
        }
    }

    #[test]
    fn selected_assay_runs_through_p08_and_updates_only_its_state_posterior() {
        let route = ready_route();
        let mut gateway = MeasurementGateway {
            manifest: route.workflow.protocol_manifest.clone(),
            interlocks: route.workflow.instrument_preflight.interlocks.clone(),
            dispatches: Vec::new(),
        };
        let mut interpreter = SignalInterpreter;
        let result = execute_glioma_simulation_gated_assay_campaign(
            &campaign_request(),
            &[GliomaResearchStratum {
                stratum_id: "mesenchymal".into(),
                label: "mesenchymal-like glioma organoids".into(),
                priority_weight_milli: 1_000,
                mechanism_priors: vec![
                    DesignMechanism {
                        mechanism_id: "m1".into(),
                        prior_milli: 500,
                    },
                    DesignMechanism {
                        mechanism_id: "m2".into(),
                        prior_milli: 500,
                    },
                ],
            }],
            &[candidate()],
            &[],
            &[route],
            &mut gateway,
            &mut interpreter,
        )
        .unwrap();

        let campaign = result.campaign.as_ref().unwrap();
        assert_eq!(campaign.selected_order, vec!["assay-mesenchymal"]);
        assert_eq!(result.executions.len(), 1);
        assert_eq!(gateway.dispatches, vec!["image-assay"]);
        assert_eq!(campaign.final_posteriors.len(), 1);
        let posterior = &campaign.final_posteriors[0].posterior;
        assert!(posterior[0].posterior_milli > posterior[1].posterior_milli);
        assert_eq!(posterior[0].observations_used, 1);
        result.validate().unwrap();
    }

    fn acquisition_analysis() -> LineagePropagationAnalysis {
        let request = LineagePropagationRequest {
            objective: "estimate finite-interval glioma lineage-state perturbation contrasts"
                .into(),
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
            bootstrap_seed: ContentHash::of_bytes(b"simulation-gated-acquisition-test"),
            confidence_level_milli: 900,
            minimum_effect_ppm: 25_000,
        };
        let unit_matrices = [
            ("control", [[8, 1], [2, 9]]),
            ("control", [[9, 2], [1, 8]]),
            ("control", [[7, 3], [3, 7]]),
            ("perturbation", [[6, 3], [4, 7]]),
            ("perturbation", [[7, 4], [3, 6]]),
            ("perturbation", [[5, 2], [5, 8]]),
        ];
        let mut snapshots = Vec::new();
        for (unit_index, (arm, matrix)) in unit_matrices.into_iter().enumerate() {
            for source in 0..2 {
                let lineage = format!("{arm}-u{unit_index}-l{source}");
                let mut counts = vec![0_u32; 2];
                counts[source] = 100;
                for (step, day) in [0_u32, 7, 14].into_iter().enumerate() {
                    snapshots.push(LineagePropagationSnapshot {
                        observation_id: format!("{lineage}-t{day}"),
                        experimental_unit_id: format!("{arm}-u{unit_index}"),
                        lineage_id: lineage.clone(),
                        arm_id: arm.into(),
                        model_system: GliomaModelSystem::Organoid,
                        assay_batch_id: format!("assay-batch-{unit_index}"),
                        timepoint_day: day,
                        state_counts: counts.clone(),
                        capture_fraction_ppm: 1_000_000,
                        artifact: local_artifact(
                            &format!("{lineage}-{day}"),
                            "application/octet-stream",
                        ),
                    });
                    if step < 2 {
                        counts = (0..2)
                            .map(|to| {
                                (0..2)
                                    .map(|from| matrix[to][from] * counts[from])
                                    .sum::<u32>()
                                    / 10
                            })
                            .collect();
                    }
                }
            }
        }
        analyze_glioma_lineage_propagation(&request, &snapshots).unwrap()
    }

    fn lineage_candidate(action_id: &str) -> StratifiedAssayCandidate {
        let mut candidate = candidate();
        candidate.stratum_id = "mes_like".into();
        candidate.action.action_id = action_id.into();
        candidate.action.label = format!("{action_id} lineage-resolved organoid assay");
        candidate
    }

    #[derive(Clone, Copy)]
    enum AcquisitionResponsePattern {
        Uninformative,
        MedianSplit,
        UpperQuartile,
    }

    fn calibration_observations(
        analysis: &LineagePropagationAnalysis,
        candidate: &StratifiedAssayCandidate,
        pattern: AcquisitionResponsePattern,
    ) -> Vec<LineageAssayCalibrationObservation> {
        let source = analysis
            .state_order
            .iter()
            .position(|state| state == &candidate.stratum_id)
            .unwrap();
        let destination = analysis
            .state_order
            .iter()
            .position(|state| state == "mes_like")
            .unwrap();
        let states = analysis.state_order.len();
        let contrasts = analysis
            .bootstrap_draws
            .iter()
            .map(|draw| {
                let flat_index = destination * states + source;
                draw.treatment_coefficients_ppm[flat_index] as i64
                    - draw.control_coefficients_ppm[flat_index] as i64
            })
            .collect::<Vec<_>>();
        let mut sorted = contrasts.clone();
        sorted.sort_unstable();
        let median = sorted[sorted.len() / 2];
        let upper_quartile = sorted[sorted.len() * 3 / 4];
        let minimum = *sorted.first().unwrap();
        let maximum = *sorted.last().unwrap();
        let mut observations = Vec::new();
        match pattern {
            AcquisitionResponsePattern::Uninformative => {
                for index in 0..32 {
                    let value = minimum + ((i128::from(maximum - minimum) * index) / 31) as i64;
                    observations.push(LineageAssayCalibrationObservation {
                        experimental_unit_id: format!("unit-{index:03}"),
                        treatment_minus_control_coefficient_ppm: value,
                        outcome_id: if index % 2 == 0 { "signal" } else { "null" }.into(),
                    });
                }
            }
            AcquisitionResponsePattern::MedianSplit | AcquisitionResponsePattern::UpperQuartile => {
                let threshold = match pattern {
                    AcquisitionResponsePattern::MedianSplit => median,
                    AcquisitionResponsePattern::UpperQuartile => upper_quartile,
                    AcquisitionResponsePattern::Uninformative => unreachable!(),
                };
                for index in 0..32 {
                    let value = minimum + ((i128::from(maximum - minimum) * index) / 31) as i64;
                    observations.push(LineageAssayCalibrationObservation {
                        experimental_unit_id: format!("unit-{index:03}"),
                        treatment_minus_control_coefficient_ppm: value,
                        outcome_id: if value >= threshold { "signal" } else { "null" }.into(),
                    });
                }
            }
        }
        observations
    }

    fn calibrated_model(
        analysis: &LineagePropagationAnalysis,
        candidate: &StratifiedAssayCandidate,
        pattern: AcquisitionResponsePattern,
    ) -> super::super::lineage_response_calibration::LineageAssayResponseCalibrationRun {
        let request = LineageAssayResponseCalibrationRequest {
            destination_state_id: "mes_like".into(),
            model_version: "empirical-organoid-response-v1".into(),
            maximum_quantile_bins: 6,
            minimum_brier_skill_ppm: 50_000,
        };
        calibrate_glioma_lineage_assay_response_model(
            analysis,
            candidate,
            &request,
            &calibration_observations(analysis, candidate, pattern),
        )
        .unwrap()
    }

    fn paired_calibration_observations(
        analysis: &LineagePropagationAnalysis,
        first: &StratifiedAssayCandidate,
        first_pattern: AcquisitionResponsePattern,
        second: &StratifiedAssayCandidate,
        second_pattern: AcquisitionResponsePattern,
    ) -> Vec<LineageAssayJointCalibrationObservation> {
        let first_rows = calibration_observations(analysis, first, first_pattern);
        let second_rows = calibration_observations(analysis, second, second_pattern);
        assert_eq!(first_rows.len(), second_rows.len());
        first_rows
            .into_iter()
            .zip(second_rows)
            .map(|(first, second)| {
                assert_eq!(first.experimental_unit_id, second.experimental_unit_id);
                assert_eq!(
                    first.treatment_minus_control_coefficient_ppm,
                    second.treatment_minus_control_coefficient_ppm
                );
                LineageAssayJointCalibrationObservation {
                    experimental_unit_id: first.experimental_unit_id,
                    treatment_minus_control_coefficient_ppm: first
                        .treatment_minus_control_coefficient_ppm,
                    first_outcome_id: first.outcome_id,
                    second_outcome_id: second.outcome_id,
                }
            })
            .collect()
    }

    fn calibrated_joint_model(
        analysis: &LineagePropagationAnalysis,
        first: &StratifiedAssayCandidate,
        first_pattern: AcquisitionResponsePattern,
        second: &StratifiedAssayCandidate,
        second_pattern: AcquisitionResponsePattern,
    ) -> super::super::lineage_response_calibration::LineageAssayJointResponseCalibrationRun {
        calibrate_glioma_lineage_joint_assay_response_model(
            analysis,
            first,
            second,
            &LineageAssayJointResponseCalibrationRequest {
                destination_state_id: "mes_like".into(),
                model_version: "empirical-organoid-response-v1".into(),
                maximum_quantile_bins: 6,
                minimum_marginal_brier_skill_ppm: 50_000,
                minimum_dependence_brier_skill_ppm: 50_000,
            },
            &paired_calibration_observations(
                analysis,
                first,
                first_pattern,
                second,
                second_pattern,
            ),
        )
        .unwrap()
    }

    #[test]
    fn lineage_acquisition_replans_across_sequential_p07_p08_assays_and_updates_p10_posterior() {
        let analysis = acquisition_analysis();
        let mut informative = lineage_candidate("assay-informative");
        informative.action.feasibility_milli = 100;
        let uninformative = lineage_candidate("assay-uninformative");
        let complementary = lineage_candidate("assay-complementary");
        let candidates = vec![
            uninformative.clone(),
            informative.clone(),
            complementary.clone(),
        ];
        let calibration_runs = vec![
            calibrated_model(
                &analysis,
                &uninformative,
                AcquisitionResponsePattern::Uninformative,
            ),
            calibrated_model(
                &analysis,
                &informative,
                AcquisitionResponsePattern::MedianSplit,
            ),
            calibrated_model(
                &analysis,
                &complementary,
                AcquisitionResponsePattern::UpperQuartile,
            ),
        ];
        assert_eq!(
            calibration_runs[0].disposition,
            LineageAssayResponseCalibrationDisposition::BaselineOnly
        );
        assert_eq!(
            calibration_runs[1].disposition,
            LineageAssayResponseCalibrationDisposition::Predictive
        );
        assert_eq!(
            calibration_runs[2].disposition,
            LineageAssayResponseCalibrationDisposition::Predictive
        );
        let models = calibration_runs
            .iter()
            .map(|run| run.response_model.clone())
            .collect::<Vec<_>>();
        let target = LineagePropagationAcquisitionTarget {
            destination_state_order: analysis.state_order.clone(),
            destination_state_weights_milli: vec![0, 1_000],
        };
        let patterns = [
            AcquisitionResponsePattern::Uninformative,
            AcquisitionResponsePattern::MedianSplit,
            AcquisitionResponsePattern::UpperQuartile,
        ];
        let mut joint_models = Vec::new();
        for first_index in 0..candidates.len() {
            for second_index in first_index + 1..candidates.len() {
                let run = calibrated_joint_model(
                    &analysis,
                    &candidates[first_index],
                    patterns[first_index],
                    &candidates[second_index],
                    patterns[second_index],
                );
                assert_eq!(
                    run.first_response_model,
                    *models
                        .iter()
                        .find(|model| model.action_id == run.first_action_id)
                        .unwrap()
                );
                assert_eq!(
                    run.second_response_model,
                    *models
                        .iter()
                        .find(|model| model.action_id == run.second_action_id)
                        .unwrap()
                );
                joint_models.push(run.joint_response_model);
            }
        }
        let policy =
            LineagePropagationAcquisitionPolicy::new_with_target_and_joint_response_models(
                analysis.clone(),
                &candidates,
                &models,
                &joint_models,
                target,
                &[],
                &[],
                3,
            )
            .unwrap();
        let initial_particle_weights = policy.particle_weights_million().to_vec();
        let initial_plan = policy.plan(&candidates).unwrap();
        assert_eq!(initial_plan.scores[0].action_id, "assay-complementary");
        let informative_score = initial_plan
            .scores
            .iter()
            .find(|score| score.action_id == "assay-informative")
            .unwrap();
        let complementary_score = initial_plan
            .scores
            .iter()
            .find(|score| score.action_id == "assay-complementary")
            .unwrap();
        assert!(
            informative_score.reduction_per_cost_million
                > complementary_score.reduction_per_cost_million
        );
        assert!(
            informative_score.expected_realized_reduction_per_cost_million
                < complementary_score.expected_realized_reduction_per_cost_million
        );
        let stratum = GliomaResearchStratum {
            stratum_id: "mes_like".into(),
            label: "mesenchymal-like glioma organoids".into(),
            priority_weight_milli: 1_000,
            mechanism_priors: vec![
                DesignMechanism {
                    mechanism_id: "m1".into(),
                    prior_milli: 500,
                },
                DesignMechanism {
                    mechanism_id: "m2".into(),
                    prior_milli: 500,
                },
            ],
        };
        let mut uninformative_route = ready_route();
        uninformative_route.stratum_id = uninformative.stratum_id.clone();
        uninformative_route.action_id = uninformative.action.action_id.clone();
        let mut informative_route = uninformative_route.clone();
        informative_route.action_id = informative.action.action_id.clone();
        let mut complementary_route = uninformative_route.clone();
        complementary_route.action_id = complementary.action.action_id.clone();
        let mut gateway = MeasurementGateway {
            manifest: informative_route.workflow.protocol_manifest.clone(),
            interlocks: informative_route
                .workflow
                .instrument_preflight
                .interlocks
                .clone(),
            dispatches: Vec::new(),
        };
        let mut interpreter = SignalInterpreter;

        let mut request = campaign_request();
        request.campaign.max_rounds = 3;
        let mut mismatch_gateway = MeasurementGateway {
            manifest: informative_route.workflow.protocol_manifest.clone(),
            interlocks: informative_route
                .workflow
                .instrument_preflight
                .interlocks
                .clone(),
            dispatches: Vec::new(),
        };
        let mut mismatch_interpreter = SignalInterpreter;
        let mismatch = execute_glioma_simulation_gated_assay_campaign_with_lineage_acquisition(
            &request,
            &[stratum.clone()],
            &candidates,
            &[StratifiedAssayObservation {
                stratum_id: "mes_like".into(),
                action_id: "assay-informative".into(),
                outcome_id: "signal".into(),
                replicate_index: 1,
                artifact: local_artifact("prior-assay", "application/json"),
            }],
            &[
                uninformative_route.clone(),
                informative_route.clone(),
                complementary_route.clone(),
            ],
            policy.clone(),
            &mut mismatch_gateway,
            &mut mismatch_interpreter,
        );
        assert!(matches!(
            mismatch,
            Err(SimulationGatedAssayCampaignError::InvalidRequest(message)) if message.contains("history")
        ));
        assert!(mismatch_gateway.dispatches.is_empty());

        let result = execute_glioma_simulation_gated_assay_campaign_with_lineage_acquisition(
            &request,
            &[stratum],
            &candidates,
            &[],
            &[uninformative_route, informative_route, complementary_route],
            policy,
            &mut gateway,
            &mut interpreter,
        )
        .unwrap();

        let campaign = result.campaign.as_ref().unwrap();
        assert!((2..=3).contains(&campaign.selected_order.len()));
        assert_eq!(campaign.selected_order[0], "assay-complementary");
        assert_eq!(campaign.rounds[1].acquisition_horizon_assays, Some(2));
        assert_eq!(result.executions.len(), campaign.selected_order.len());
        assert_eq!(
            campaign.selected_order,
            result
                .executions
                .iter()
                .map(|execution| execution.action_id.clone())
                .collect::<Vec<_>>()
        );
        assert_eq!(result.executions[0].action_id, "assay-complementary");
        assert_eq!(gateway.dispatches.len(), result.executions.len());
        assert!(gateway
            .dispatches
            .iter()
            .all(|dispatch| dispatch == "image-assay"));
        let final_plan = result.lineage_acquisition_plan.as_ref().unwrap();
        final_plan.validate().unwrap();
        assert_eq!(
            final_plan.response_dependence,
            LineagePropagationResponseDependence::FirstOrderPairCalibration
        );
        assert_eq!(
            final_plan.conditioned_on.as_ref().unwrap().action_id,
            result.executions.last().unwrap().action_id
        );
        assert_ne!(
            final_plan.particle_weights_million,
            initial_particle_weights
        );
        result.validate().unwrap();
    }

    #[test]
    fn empirical_lineage_assay_calibration_is_unit_held_out_and_refuses_uninformative_skill() {
        let analysis = acquisition_analysis();
        let candidate = lineage_candidate("assay-calibrated");
        let request = LineageAssayResponseCalibrationRequest {
            destination_state_id: "mes_like".into(),
            model_version: "empirical-organoid-response-v1".into(),
            maximum_quantile_bins: 6,
            minimum_brier_skill_ppm: 50_000,
        };
        let observations = calibration_observations(
            &analysis,
            &candidate,
            AcquisitionResponsePattern::MedianSplit,
        );
        let calibrated = calibrate_glioma_lineage_assay_response_model(
            &analysis,
            &candidate,
            &request,
            &observations,
        )
        .unwrap();
        assert_eq!(
            calibrated.disposition,
            LineageAssayResponseCalibrationDisposition::Predictive
        );
        assert!(calibrated.diagnostics.brier_skill_ppm >= 50_000);
        assert!(
            calibrated.diagnostics.leave_one_unit_out_brier_million
                < calibrated.diagnostics.prevalence_baseline_brier_million
        );
        calibrated.validate(&analysis, &candidate).unwrap();

        let mut reversed = observations.clone();
        reversed.reverse();
        let permutation_replay = calibrate_glioma_lineage_assay_response_model(
            &analysis, &candidate, &request, &reversed,
        )
        .unwrap();
        assert_eq!(calibrated.digest, permutation_replay.digest);

        let uninformative = lineage_candidate("assay-null-skill");
        let null_rows = calibration_observations(
            &analysis,
            &uninformative,
            AcquisitionResponsePattern::Uninformative,
        );
        let null_run = calibrate_glioma_lineage_assay_response_model(
            &analysis,
            &uninformative,
            &request,
            &null_rows,
        )
        .unwrap();
        assert_eq!(
            null_run.disposition,
            LineageAssayResponseCalibrationDisposition::BaselineOnly
        );
        for outcome in &null_run.response_model.outcomes {
            assert!(outcome
                .likelihood_milli_by_bootstrap_draw
                .iter()
                .all(|probability| *probability == outcome.likelihood_milli_by_bootstrap_draw[0]));
        }

        let mut duplicated_unit = observations;
        duplicated_unit[1].experimental_unit_id = duplicated_unit[0].experimental_unit_id.clone();
        assert!(calibrate_glioma_lineage_assay_response_model(
            &analysis,
            &candidate,
            &request,
            &duplicated_unit,
        )
        .is_err());
    }

    #[test]
    fn paired_response_calibration_beats_independence_and_replays_permutations() {
        let analysis = acquisition_analysis();
        let first = lineage_candidate("assay-joint-first");
        let second = lineage_candidate("assay-joint-second");
        let observations = paired_calibration_observations(
            &analysis,
            &first,
            AcquisitionResponsePattern::MedianSplit,
            &second,
            AcquisitionResponsePattern::UpperQuartile,
        );
        let request = LineageAssayJointResponseCalibrationRequest {
            destination_state_id: "mes_like".into(),
            model_version: "paired-organoid-assay-response-v1".into(),
            maximum_quantile_bins: 6,
            minimum_marginal_brier_skill_ppm: 50_000,
            minimum_dependence_brier_skill_ppm: 50_000,
        };
        let run = calibrate_glioma_lineage_joint_assay_response_model(
            &analysis,
            &first,
            &second,
            &request,
            &observations,
        )
        .unwrap();
        assert_eq!(
            run.disposition,
            LineageAssayJointResponseCalibrationDisposition::ConditionalDependenceSupported
        );
        assert!(run.diagnostics.brier_skill_ppm >= 50_000);
        assert!(
            run.diagnostics.leave_one_unit_out_brier_million
                < run.diagnostics.independence_baseline_brier_million
        );
        run.validate(&analysis, &first, &second).unwrap();

        let mut duplicated_unit = observations.clone();
        duplicated_unit[1].experimental_unit_id = duplicated_unit[0].experimental_unit_id.clone();
        assert!(calibrate_glioma_lineage_joint_assay_response_model(
            &analysis,
            &first,
            &second,
            &request,
            &duplicated_unit,
        )
        .is_err());

        let mut reversed_observations = observations.clone();
        reversed_observations.reverse();
        for observation in &mut reversed_observations {
            std::mem::swap(
                &mut observation.first_outcome_id,
                &mut observation.second_outcome_id,
            );
        }
        let permutation_replay = calibrate_glioma_lineage_joint_assay_response_model(
            &analysis,
            &second,
            &first,
            &request,
            &reversed_observations,
        )
        .unwrap();
        assert_eq!(run.digest, permutation_replay.digest);
        assert_eq!(
            run.joint_response_model,
            permutation_replay.joint_response_model
        );
    }

    #[test]
    fn paired_response_calibration_falls_back_below_dependence_skill_gate() {
        let analysis = acquisition_analysis();
        let first = lineage_candidate("assay-pair-fallback-first");
        let second = lineage_candidate("assay-pair-fallback-second");
        let observations = paired_calibration_observations(
            &analysis,
            &first,
            AcquisitionResponsePattern::MedianSplit,
            &second,
            AcquisitionResponsePattern::UpperQuartile,
        );
        let run = calibrate_glioma_lineage_joint_assay_response_model(
            &analysis,
            &first,
            &second,
            &LineageAssayJointResponseCalibrationRequest {
                destination_state_id: "mes_like".into(),
                model_version: "paired-organoid-assay-response-strict-gate-v1".into(),
                maximum_quantile_bins: 6,
                minimum_marginal_brier_skill_ppm: 50_000,
                minimum_dependence_brier_skill_ppm: 1_000_000,
            },
            &observations,
        )
        .unwrap();
        assert_eq!(
            run.disposition,
            LineageAssayJointResponseCalibrationDisposition::ConditionalIndependenceBaseline
        );
        run.validate(&analysis, &first, &second).unwrap();
    }

    #[test]
    fn interpreter_cannot_update_the_posterior_from_an_unbound_artifact() {
        let route = ready_route();
        let mut gateway = MeasurementGateway {
            manifest: route.workflow.protocol_manifest.clone(),
            interlocks: route.workflow.instrument_preflight.interlocks.clone(),
            dispatches: Vec::new(),
        };
        let mut interpreter = UnboundArtifactInterpreter;
        let result = execute_glioma_simulation_gated_assay_campaign(
            &campaign_request(),
            &[GliomaResearchStratum {
                stratum_id: "mesenchymal".into(),
                label: "mesenchymal-like glioma organoids".into(),
                priority_weight_milli: 1_000,
                mechanism_priors: vec![
                    DesignMechanism {
                        mechanism_id: "m1".into(),
                        prior_milli: 500,
                    },
                    DesignMechanism {
                        mechanism_id: "m2".into(),
                        prior_milli: 500,
                    },
                ],
            }],
            &[candidate()],
            &[],
            &[route],
            &mut gateway,
            &mut interpreter,
        );

        assert!(result.is_err());
        assert_eq!(gateway.dispatches, vec!["image-assay"]);
    }

    #[test]
    fn longitudinal_state_guidance_drives_adaptive_assays_through_p07_and_p08() {
        let assay = campaign_request();
        let request = GliomaStatePlasticityInstrumentRequest {
            transition_analysis: StateTransitionRequest {
                objective: "measure preclinical glioma organoid state transitions".into(),
                control_arm: "control".into(),
                treatment_arm: "perturbation".into(),
                model_system: GliomaModelSystem::Organoid,
                state_order: vec!["mesenchymal".into(), "npc-like".into()],
                min_units_per_arm: 3,
                min_transitions_per_arm: 3,
                max_timepoint_gap: 1,
                min_contrast_milli: 0,
            },
            assay_campaign: TransitionGuidedStateCampaignRequest {
                campaign: StateStratifiedCampaignRequest {
                    campaign: AdaptiveInformationCampaignRequest {
                        max_rounds: 2,
                        ..assay.campaign
                    },
                    ..assay
                },
                transition_guidance: TransitionGuidanceRequest {
                    min_source_units_per_arm: 3,
                },
            },
        };

        let strata = [
            GliomaResearchStratum {
                stratum_id: "mesenchymal".into(),
                label: "mesenchymal-like preclinical organoids".into(),
                priority_weight_milli: 500,
                mechanism_priors: vec![
                    DesignMechanism {
                        mechanism_id: "m1".into(),
                        prior_milli: 500,
                    },
                    DesignMechanism {
                        mechanism_id: "m2".into(),
                        prior_milli: 500,
                    },
                ],
            },
            GliomaResearchStratum {
                stratum_id: "npc-like".into(),
                label: "neural-progenitor-like preclinical organoids".into(),
                priority_weight_milli: 500,
                mechanism_priors: vec![
                    DesignMechanism {
                        mechanism_id: "m1".into(),
                        prior_milli: 500,
                    },
                    DesignMechanism {
                        mechanism_id: "m2".into(),
                        prior_milli: 500,
                    },
                ],
            },
        ];
        let mut npc_candidate = candidate();
        npc_candidate.stratum_id = "npc-like".into();
        npc_candidate.action.action_id = "assay-npc-like".into();
        let candidates = [candidate(), npc_candidate];
        let mut npc_route = ready_route();
        npc_route.stratum_id = "npc-like".into();
        npc_route.action_id = "assay-npc-like".into();
        let routes = [ready_route(), npc_route];
        let observations = ["control", "perturbation"]
            .into_iter()
            .flat_map(|arm| {
                (0..3).flat_map(move |replicate| {
                    ["mesenchymal", "npc-like", "mesenchymal"]
                        .into_iter()
                        .enumerate()
                        .map(move |(timepoint, state_id)| {
                            let id = format!("{arm}-{replicate}-{timepoint}");
                            StateTransitionObservation {
                                observation_id: id.clone(),
                                unit_id: format!("{arm}-unit-{replicate}"),
                                arm_id: arm.into(),
                                model_system: GliomaModelSystem::Organoid,
                                batch_id: format!("batch-{replicate}"),
                                timepoint: timepoint as u32,
                                state_id: state_id.into(),
                                state_score_milli: if state_id == "mesenchymal" {
                                    300
                                } else {
                                    700
                                },
                                artifact: local_artifact(
                                    &format!("transition-{id}"),
                                    "application/vnd.aurora.preclinical-state-observation+json",
                                ),
                            }
                        })
                })
            })
            .collect::<Vec<_>>();
        let first_route = &routes[0];
        let mut gateway = MeasurementGateway {
            manifest: first_route.workflow.protocol_manifest.clone(),
            interlocks: first_route.workflow.instrument_preflight.interlocks.clone(),
            dispatches: Vec::new(),
        };
        let mut interpreter = SignalInterpreter;

        let inputs = GliomaStatePlasticityInstrumentInputs {
            longitudinal_observations: &observations,
            strata: &strata,
            candidates: &candidates,
            initial_observations: &[],
            routes: &routes,
        };
        let propagation_request = LineagePropagationRequest {
            objective: "estimate finite-interval glioma lineage propagation before selecting follow-up assays".into(),
            model_system: GliomaModelSystem::Organoid,
            control_arm: "control".into(),
            treatment_arm: "perturbation".into(),
            state_order: vec!["mesenchymal".into(), "npc-like".into()],
            min_units_per_arm: 2,
            min_lineages_per_arm: 4,
            ridge_penalty_ppm: 1,
            max_coefficient_ppm: 5_000_000,
            max_prediction_error_ppm: 300_000,
            bootstrap_replicates: 99,
            bootstrap_seed: ContentHash::of_bytes(b"p06-f04-guided-campaign"),
            confidence_level_milli: 900,
            minimum_effect_ppm: 25_000,
        };
        let mut propagation_snapshots = Vec::new();
        for arm in ["control", "perturbation"] {
            for unit_index in 0..3 {
                for source in 0..2 {
                    let lineage = format!("{arm}-unit-{unit_index}-lineage-{source}");
                    let matrix = if arm == "control" {
                        [[8_u32, 2_u32], [2_u32, 8_u32]]
                    } else {
                        [[6_u32, 2_u32], [4_u32, 8_u32]]
                    };
                    let mut counts = [0_u32; 2];
                    counts[source] = 100;
                    for (step, timepoint_day) in [0_u32, 7, 14].into_iter().enumerate() {
                        propagation_snapshots.push(LineagePropagationSnapshot {
                            observation_id: format!("{lineage}-{timepoint_day}"),
                            experimental_unit_id: format!("{arm}-unit-{unit_index}"),
                            lineage_id: lineage.clone(),
                            arm_id: arm.into(),
                            model_system: GliomaModelSystem::Organoid,
                            assay_batch_id: format!("{arm}-batch-{unit_index}"),
                            timepoint_day,
                            state_counts: counts.to_vec(),
                            capture_fraction_ppm: 1_000_000,
                            artifact: local_artifact(
                                &format!("{lineage}-{timepoint_day}"),
                                "application/vnd.aurora.glioma-lineage-propagation+json",
                            ),
                        });
                        if step < 2 {
                            counts = [
                                (matrix[0][0] * counts[0] + matrix[0][1] * counts[1]) / 10,
                                (matrix[1][0] * counts[0] + matrix[1][1] * counts[1]) / 10,
                            ];
                        }
                    }
                }
            }
        }
        let propagation =
            analyze_glioma_lineage_propagation(&propagation_request, &propagation_snapshots)
                .unwrap();
        assert_eq!(
            propagation.disposition,
            LineagePropagationDisposition::Qualified
        );
        let response_request = LineageResponseDecompositionRequest {
            baseline_source_composition_ppm: vec![700_000, 300_000],
            minimum_component_ppm: 25_000,
        };
        let run = execute_glioma_lineage_response_guided_state_plasticity_campaign(
            &request,
            &inputs,
            &propagation,
            &response_request,
            &mut gateway,
            &mut interpreter,
        )
        .unwrap();

        assert_eq!(run.transition_analysis.unit_order.len(), 6);
        assert!(run.lineage_response_decomposition.is_some());
        assert!(run.lineage_propagation_priorities.iter().any(|priority| {
            priority.state_id == "mesenchymal"
                && priority
                    .response_followup_attention_milli
                    .is_some_and(|value| value > 0)
        }));
        let campaign = run.instrument_campaign.campaign.as_ref().unwrap();
        assert_eq!(campaign.selected_order.len(), 2);
        assert_eq!(run.instrument_campaign.executions.len(), 2);
        assert_eq!(gateway.dispatches, vec!["image-assay", "image-assay"]);
        assert_eq!(campaign.final_posteriors.len(), 2);
        for priority in &run.priorities {
            let posterior = campaign
                .final_posteriors
                .iter()
                .find(|row| row.stratum_id == priority.state_id)
                .unwrap();
            assert_eq!(
                posterior.priority_weight_milli,
                priority.adjusted_priority_milli
            );
            assert_eq!(posterior.valid_assay_count, 1);
        }
        run.validate().unwrap();

        let mut blocked_routes = routes.clone();
        for route in &mut blocked_routes {
            route.workflow.protocol.max_risk_milli = 0;
        }
        let blocked_inputs = GliomaStatePlasticityInstrumentInputs {
            longitudinal_observations: &observations,
            strata: &strata,
            candidates: &candidates,
            initial_observations: &[],
            routes: &blocked_routes,
        };
        let mut blocked_gateway = MeasurementGateway {
            manifest: blocked_routes[0].workflow.protocol_manifest.clone(),
            interlocks: blocked_routes[0]
                .workflow
                .instrument_preflight
                .interlocks
                .clone(),
            dispatches: Vec::new(),
        };
        let mut blocked_interpreter = SignalInterpreter;
        let blocked_run = execute_glioma_lineage_response_guided_state_plasticity_campaign(
            &request,
            &blocked_inputs,
            &propagation,
            &response_request,
            &mut blocked_gateway,
            &mut blocked_interpreter,
        )
        .unwrap();
        assert_eq!(
            blocked_run.instrument_campaign.disposition,
            SimulationGatedAssayCampaignDisposition::Blocked
        );
        assert!(blocked_run.instrument_campaign.campaign.is_none());
        assert!(blocked_gateway.dispatches.is_empty());
        blocked_run.validate().unwrap();
    }
}
