//! High-throughput autonomous scheduling for preclinical glioma research programs.
//!
//! A single research intent is not enough for a productive laboratory: several studies compete
//! for the same organoid, imaging, sequencing, and compute capacity. This controller compiles a
//! dependency-safe frontier for every intent, scores scientific unlock against cost and fairness
//! debt, admits a non-conflicting batch, executes each batch through the existing local director,
//! and replans from returned artifacts. It never merges biological evidence across studies, never
//! moves raw data, and never treats a simulated action as a scientific observation.

use super::action_execution::{
    ActionExecutionDisposition, DryRunGliomaActionExecutor, GliomaActionExecutor,
};
use super::director::{
    execute_glioma_research_director, plan_glioma_research_director, GliomaDirectorCheckpoint,
    GliomaResearchDirectorRequest, GliomaResearchDirectorRun,
};
use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P07-F15";
pub const OUTPUT_SCHEMA: &str = "GliomaProgramScheduler1@1";
pub const MAX_JOBS: usize = 64;
pub const MAX_ROUNDS: u16 = 32;
pub const MAX_RESOURCES: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaProgramScheduleJob {
    pub job_id: String,
    pub director: GliomaResearchDirectorRequest,
    pub priority_milli: u16,
    pub fairness_weight_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaProgramResourceCapacity {
    pub model_system: GliomaModelSystem,
    pub modality: GliomaModality,
    pub capacity_units: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaProgramSchedulerRequest {
    pub objective: String,
    pub jobs: Vec<GliomaProgramScheduleJob>,
    pub global_budget_units: u64,
    pub max_rounds: u16,
    pub max_jobs_per_round: u16,
    pub resource_capacities: Vec<GliomaProgramResourceCapacity>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GliomaProgramScheduleJobDisposition {
    Active,
    Completed,
    Partial,
    Blocked,
    Failed,
    NoRunnableActions,
    BudgetExhausted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GliomaProgramSchedulerDisposition {
    Completed,
    Partial,
    Blocked,
    BudgetExhausted,
    NoRunnableJobs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GliomaProgramSchedulerStopReason {
    Completed,
    BudgetExhausted,
    NoRunnableJobs,
    AllJobsBlocked,
    MaxRounds,
    NoProgress,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaProgramScheduleHold {
    pub job_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GliomaProgramScheduleExecution {
    pub job_id: String,
    pub action_order: Vec<String>,
    pub cost_units: u64,
    pub director: Option<GliomaResearchDirectorRun>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaProgramResourceUsage {
    pub model_system: GliomaModelSystem,
    pub modality: GliomaModality,
    pub used_units: u16,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GliomaProgramSchedulerRound {
    pub round: u16,
    pub planned_job_order: Vec<String>,
    pub selected_job_order: Vec<String>,
    pub blocked_job_order: Vec<String>,
    pub holds: Vec<GliomaProgramScheduleHold>,
    pub resource_usage: Vec<GliomaProgramResourceUsage>,
    pub executions: Vec<GliomaProgramScheduleExecution>,
    pub budget_before_units: u64,
    pub budget_after_units: u64,
    pub cost_units: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaProgramScheduleJobState {
    pub job_id: String,
    pub priority_milli: u16,
    pub fairness_weight_milli: u16,
    pub rounds_run: u16,
    pub budget_limit_units: u32,
    pub budget_spent_units: u64,
    pub remaining_budget_units: u32,
    pub completed_stage_order: Vec<String>,
    pub pending_action_order: Vec<String>,
    pub hold_order: Vec<String>,
    pub approval_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub last_director_digest: Option<ContentHash>,
    pub last_error: Option<String>,
    pub disposition: GliomaProgramScheduleJobDisposition,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GliomaProgramSchedulerRun {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub budget_units: u64,
    pub rounds: Vec<GliomaProgramSchedulerRound>,
    pub jobs: Vec<GliomaProgramScheduleJobState>,
    pub budget_spent_units: u64,
    pub remaining_budget_units: u64,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: GliomaProgramSchedulerDisposition,
    pub stop_reason: GliomaProgramSchedulerStopReason,
    pub next_step: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GliomaProgramSchedulerError {
    #[error("glioma program scheduler request is invalid: {0}")]
    InvalidRequest(String),
    #[error("glioma program scheduler output is invalid: {0}")]
    InvalidOutput(String),
    #[error("glioma program scheduler digest failed: {0}")]
    Digest(String),
}

#[derive(Debug, Clone)]
struct MutableJob {
    spec: GliomaProgramScheduleJob,
    request: GliomaResearchDirectorRequest,
    age_rounds: u16,
    rounds_run: u16,
    budget_spent_units: u64,
    last_plan: Option<GliomaResearchDirectorRun>,
    last_error: Option<String>,
    disposition: GliomaProgramScheduleJobDisposition,
    negative_evidence: BTreeSet<String>,
    uncertainty: BTreeSet<String>,
}

type ResourceKey = (GliomaModelSystem, GliomaModality);

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_nonempty(values: &[String]) -> bool {
    !values.iter().any(|value| value.trim().is_empty())
        && values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

fn digest_input(run: &GliomaProgramSchedulerRun) -> serde_json::Value {
    serde_json::json!({
        "feature_id": run.feature_id,
        "output_schema": run.output_schema,
        "objective": run.objective,
        "budget_units": run.budget_units,
        "rounds": run.rounds,
        "jobs": run.jobs,
        "budget_spent_units": run.budget_spent_units,
        "remaining_budget_units": run.remaining_budget_units,
        "negative_evidence": run.negative_evidence,
        "uncertainty": run.uncertainty,
        "disposition": run.disposition,
        "stop_reason": run.stop_reason,
        "next_step": run.next_step,
    })
}

fn resource_key(model_system: GliomaModelSystem, modality: GliomaModality) -> ResourceKey {
    (model_system, modality)
}

fn capacity_map(request: &GliomaProgramSchedulerRequest) -> BTreeMap<ResourceKey, u16> {
    request
        .resource_capacities
        .iter()
        .map(|capacity| {
            (
                resource_key(capacity.model_system, capacity.modality),
                capacity.capacity_units,
            )
        })
        .collect()
}

fn action_map(
    plan: &GliomaResearchDirectorRun,
) -> BTreeMap<String, &crate::glioma_engine::GliomaActionCandidate> {
    plan.actions
        .iter()
        .map(|action| (action.candidate.action_id.clone(), &action.candidate))
        .collect()
}

fn plan_cost(plan: &GliomaResearchDirectorRun) -> u64 {
    let actions = action_map(plan);
    plan.next_stage_order
        .iter()
        .filter_map(|action_id| {
            actions
                .get(action_id)
                .map(|action| u64::from(action.cost_units))
        })
        .sum()
}

fn plan_usage(plan: &GliomaResearchDirectorRun) -> BTreeMap<ResourceKey, u16> {
    let actions = action_map(plan);
    let mut usage = BTreeMap::new();
    for action_id in &plan.next_stage_order {
        let Some(action) = actions.get(action_id) else {
            continue;
        };
        let key = resource_key(action.model_system, action.modality);
        let entry = usage.entry(key).or_insert(0_u16);
        *entry = entry.saturating_add(1);
    }
    usage
}

fn plan_score(job: &MutableJob, plan: &GliomaResearchDirectorRun) -> u128 {
    let actions = action_map(plan);
    let (utility, feasibility) = plan
        .next_stage_order
        .iter()
        .filter_map(|action_id| actions.get(action_id))
        .fold((0_u128, 1_u128), |(utility, feasibility), action| {
            let action_utility = u128::from(action.information_gain_milli)
                + u128::from(action.frontier_novelty_milli)
                + u128::from(action.workflow_leverage_milli)
                + u128::from(action.cross_stage_unlock_milli);
            (
                utility.saturating_add(action_utility),
                feasibility.min(u128::from(action.feasibility_milli.max(1))),
            )
        });
    let priority = u128::from(job.spec.priority_milli).saturating_mul(1_000);
    let fairness = u128::from(job.spec.fairness_weight_milli)
        .saturating_mul(u128::from(job.age_rounds).saturating_add(1))
        .saturating_mul(1_000);
    let cost = plan_cost(plan).max(1) as u128;
    priority
        .saturating_add(fairness)
        .saturating_add(utility)
        .saturating_mul(feasibility)
        .saturating_div(cost)
}

fn fits_resources(
    usage: &BTreeMap<ResourceKey, u16>,
    reserved: &BTreeMap<ResourceKey, u16>,
    capacities: &BTreeMap<ResourceKey, u16>,
) -> bool {
    if capacities.is_empty() {
        return true;
    }
    usage.iter().all(|(key, amount)| {
        capacities.get(key).is_some_and(|capacity| {
            reserved
                .get(key)
                .copied()
                .unwrap_or(0)
                .saturating_add(*amount)
                <= *capacity
        })
    })
}

fn merge_evidence(target: &mut BTreeSet<String>, values: &[String]) {
    target.extend(values.iter().cloned());
}

fn apply_director_result(job: &mut MutableJob, run: &GliomaResearchDirectorRun, cost: u64) {
    job.rounds_run = job.rounds_run.saturating_add(1);
    job.budget_spent_units = job.budget_spent_units.saturating_add(cost);
    job.last_plan = Some(run.clone());
    job.last_error = None;
    merge_evidence(&mut job.negative_evidence, &run.negative_evidence);
    merge_evidence(&mut job.uncertainty, &run.uncertainty);
    let mut stages = job
        .request
        .completed_checkpoints
        .iter()
        .map(|checkpoint| checkpoint.stage_kind)
        .collect::<BTreeSet<_>>();
    let stage_by_action = run
        .actions
        .iter()
        .map(|action| {
            (
                action.candidate.action_id.as_str(),
                action.candidate.stage_kind,
            )
        })
        .collect::<BTreeMap<_, _>>();
    if let Some(execution) = &run.execution {
        for result in &execution.results {
            if matches!(
                result.disposition,
                ActionExecutionDisposition::Failed | ActionExecutionDisposition::Skipped
            ) {
                continue;
            }
            let Some(artifact) = &result.artifact else {
                continue;
            };
            let Some(stage_kind) = stage_by_action.get(result.action_id.as_str()) else {
                continue;
            };
            if stages.insert(*stage_kind) {
                job.request
                    .completed_checkpoints
                    .push(GliomaDirectorCheckpoint {
                        stage_kind: *stage_kind,
                        artifact_id: artifact.artifact_id.clone(),
                        artifact: artifact.clone(),
                    });
            }
        }
    }
    job.request
        .completed_checkpoints
        .sort_by_key(|checkpoint| checkpoint.stage_kind);
    job.request.budget_units = job
        .request
        .budget_units
        .saturating_sub(cost.min(u64::from(u32::MAX)) as u32);
    job.age_rounds = 0;
    job.disposition = if run.next_stage_order.is_empty()
        && run.hold_order.is_empty()
        && run.approval_order.is_empty()
        && run.blocked_order.is_empty()
        && run.workflow_plan.disposition == crate::glioma_engine::GliomaPlanDisposition::Admitted
    {
        GliomaProgramScheduleJobDisposition::Completed
    } else if matches!(
        run.disposition,
        super::director::GliomaDirectorDisposition::Blocked
    ) {
        GliomaProgramScheduleJobDisposition::Blocked
    } else if run.next_stage_order.is_empty() {
        GliomaProgramScheduleJobDisposition::NoRunnableActions
    } else {
        GliomaProgramScheduleJobDisposition::Active
    };
}

fn state_from_job(job: &MutableJob) -> GliomaProgramScheduleJobState {
    let mut completed = job
        .request
        .completed_checkpoints
        .iter()
        .map(|checkpoint| checkpoint.stage_kind.stage_id().to_string())
        .collect::<Vec<_>>();
    completed.sort();
    let (mut pending, hold, approval, blocked, digest) = job
        .last_plan
        .as_ref()
        .map(|plan| {
            (
                plan.next_stage_order.clone(),
                plan.hold_order.clone(),
                plan.approval_order.clone(),
                plan.blocked_order.clone(),
                Some(plan.digest.clone()),
            )
        })
        .unwrap_or_default();
    if let Some(plan) = &job.last_plan {
        let completed = job
            .request
            .completed_checkpoints
            .iter()
            .map(|checkpoint| checkpoint.stage_kind)
            .collect::<BTreeSet<_>>();
        let stage_by_action = plan
            .actions
            .iter()
            .map(|action| {
                (
                    action.candidate.action_id.as_str(),
                    action.candidate.stage_kind,
                )
            })
            .collect::<BTreeMap<_, _>>();
        pending.retain(|action_id| {
            stage_by_action
                .get(action_id.as_str())
                .is_none_or(|stage| !completed.contains(stage))
        });
    }
    GliomaProgramScheduleJobState {
        job_id: job.spec.job_id.clone(),
        priority_milli: job.spec.priority_milli,
        fairness_weight_milli: job.spec.fairness_weight_milli,
        rounds_run: job.rounds_run,
        budget_limit_units: job.spec.director.budget_units,
        budget_spent_units: job.budget_spent_units,
        remaining_budget_units: job.request.budget_units,
        completed_stage_order: completed,
        pending_action_order: pending,
        hold_order: hold,
        approval_order: approval,
        blocked_order: blocked,
        negative_evidence: job.negative_evidence.iter().cloned().collect(),
        uncertainty: job.uncertainty.iter().cloned().collect(),
        last_director_digest: digest,
        last_error: job.last_error.clone(),
        disposition: job.disposition,
    }
}

fn validate_request(
    request: &GliomaProgramSchedulerRequest,
) -> Result<(), GliomaProgramSchedulerError> {
    if request.objective.trim().is_empty()
        || request.jobs.is_empty()
        || request.jobs.len() > MAX_JOBS
        || request.global_budget_units == 0
        || request.max_rounds == 0
        || request.max_rounds > MAX_ROUNDS
        || request.max_jobs_per_round == 0
        || usize::from(request.max_jobs_per_round) > request.jobs.len()
        || request.resource_capacities.len() > MAX_RESOURCES
    {
        return Err(GliomaProgramSchedulerError::InvalidRequest(
            "objective, bounded jobs/rounds, positive global budget, and a nonzero batch width are required".into(),
        ));
    }
    let mut job_ids = BTreeSet::new();
    for job in &request.jobs {
        if job.job_id.trim().is_empty()
            || !job_ids.insert(job.job_id.clone())
            || job.priority_milli > 1_000
            || job.fairness_weight_milli > 1_000
            || job.director.intent.objective.trim().is_empty()
            || job.director.budget_units == 0
        {
            return Err(GliomaProgramSchedulerError::InvalidRequest(
                "job identity, bounded priority/fairness, director objective, and job budget are required".into(),
            ));
        }
    }
    let mut resources = BTreeSet::new();
    for capacity in &request.resource_capacities {
        if capacity.capacity_units == 0
            || !resources.insert(resource_key(capacity.model_system, capacity.modality))
        {
            return Err(GliomaProgramSchedulerError::InvalidRequest(
                "resource capacities must be positive and unique by model/modality".into(),
            ));
        }
    }
    Ok(())
}

impl GliomaProgramSchedulerRun {
    pub fn validate(&self) -> Result<(), GliomaProgramSchedulerError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.budget_units == 0
            || self
                .budget_spent_units
                .saturating_add(self.remaining_budget_units)
                != self.budget_units
            || self.rounds.len() > usize::from(MAX_ROUNDS)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || !canonical(
                &self
                    .jobs
                    .iter()
                    .map(|job| job.job_id.clone())
                    .collect::<Vec<_>>(),
            )
            || self.next_step.trim().is_empty()
        {
            return Err(GliomaProgramSchedulerError::InvalidOutput(
                "identity, budget, bounds, ordering, jobs, or next-step invariants are invalid"
                    .into(),
            ));
        }
        let mut seen_rounds = BTreeSet::new();
        let mut spent = 0_u64;
        for round in &self.rounds {
            if round.round == 0
                || !seen_rounds.insert(round.round)
                || !unique_nonempty(&round.planned_job_order)
                || !unique_nonempty(&round.selected_job_order)
                || !canonical(&round.blocked_job_order)
                || round.budget_after_units > round.budget_before_units
                || round
                    .budget_before_units
                    .saturating_sub(round.budget_after_units)
                    != round.cost_units
                || round.selected_job_order.iter().any(|job_id| {
                    !round
                        .planned_job_order
                        .iter()
                        .any(|planned| planned == job_id)
                })
            {
                return Err(GliomaProgramSchedulerError::InvalidOutput(
                    "round ordering, selection, or budget invariants are invalid".into(),
                ));
            }
            if round.executions.len() != round.selected_job_order.len()
                || round
                    .executions
                    .iter()
                    .map(|execution| execution.job_id.clone())
                    .collect::<BTreeSet<_>>()
                    != round
                        .selected_job_order
                        .iter()
                        .cloned()
                        .collect::<BTreeSet<_>>()
            {
                return Err(GliomaProgramSchedulerError::InvalidOutput(
                    "round executions do not reconcile with selected jobs".into(),
                ));
            }
            for execution in &round.executions {
                if execution.job_id.trim().is_empty()
                    || !unique_nonempty(&execution.action_order)
                    || execution.director.is_some() == execution.error.is_some()
                {
                    return Err(GliomaProgramSchedulerError::InvalidOutput(
                        "execution identity, action order, and director/error partition are invalid".into(),
                    ));
                }
                if let Some(director) = &execution.director {
                    director.validate().map_err(|error| {
                        GliomaProgramSchedulerError::InvalidOutput(error.to_string())
                    })?;
                    if director.next_stage_order != execution.action_order {
                        return Err(GliomaProgramSchedulerError::InvalidOutput(
                            "director action order does not match the scheduled action order"
                                .into(),
                        ));
                    }
                }
            }
            spent = spent.saturating_add(round.cost_units);
        }
        if spent != self.budget_spent_units {
            return Err(GliomaProgramSchedulerError::InvalidOutput(
                "round costs do not reconcile with scheduler spend".into(),
            ));
        }
        let mut job_ids = BTreeSet::new();
        for job in &self.jobs {
            if !job_ids.insert(job.job_id.clone())
                || !canonical(&job.completed_stage_order)
                || !canonical(&job.pending_action_order)
                || !canonical(&job.hold_order)
                || !canonical(&job.approval_order)
                || !canonical(&job.blocked_order)
                || !canonical(&job.negative_evidence)
                || !canonical(&job.uncertainty)
                || job
                    .budget_spent_units
                    .saturating_add(u64::from(job.remaining_budget_units))
                    != u64::from(job.budget_limit_units)
            {
                return Err(GliomaProgramSchedulerError::InvalidOutput(
                    "job state ordering, identity, or budget reconciliation is invalid".into(),
                ));
            }
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| GliomaProgramSchedulerError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(GliomaProgramSchedulerError::InvalidOutput(
                "scheduler digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Execute a bounded high-throughput scheduler over independent local glioma research intents.
/// The scheduler shares only typed control metadata; each job retains its own checkpoints and
/// evidence lineage while competing for explicit model/modality capacity.
pub fn execute_glioma_program_scheduler<E: GliomaActionExecutor>(
    request: &GliomaProgramSchedulerRequest,
    executor: &mut E,
) -> Result<GliomaProgramSchedulerRun, GliomaProgramSchedulerError> {
    validate_request(request)?;
    let capacities = capacity_map(request);
    let mut jobs = request
        .jobs
        .iter()
        .cloned()
        .map(|spec| MutableJob {
            request: spec.director.clone(),
            spec,
            age_rounds: 0,
            rounds_run: 0,
            budget_spent_units: 0,
            last_plan: None,
            last_error: None,
            disposition: GliomaProgramScheduleJobDisposition::Active,
            negative_evidence: BTreeSet::new(),
            uncertainty: BTreeSet::new(),
        })
        .collect::<Vec<_>>();
    let mut rounds = Vec::new();
    let mut spent = 0_u64;
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut stop_reason = GliomaProgramSchedulerStopReason::MaxRounds;

    for round_number in 1..=request.max_rounds {
        let remaining = request.global_budget_units.saturating_sub(spent);
        if remaining == 0 {
            stop_reason = GliomaProgramSchedulerStopReason::BudgetExhausted;
            break;
        }
        let mut planned = Vec::new();
        let mut blocked = BTreeSet::new();
        for (index, job) in jobs.iter_mut().enumerate() {
            if !matches!(job.disposition, GliomaProgramScheduleJobDisposition::Active) {
                continue;
            }
            if job.request.budget_units == 0 {
                job.disposition = GliomaProgramScheduleJobDisposition::BudgetExhausted;
                continue;
            }
            match plan_glioma_research_director(&job.request) {
                Ok(plan) if plan.next_stage_order.is_empty() => {
                    job.last_plan = Some(plan.clone());
                    merge_evidence(&mut job.negative_evidence, &plan.negative_evidence);
                    merge_evidence(&mut job.uncertainty, &plan.uncertainty);
                    if plan.workflow_plan.disposition
                        == crate::glioma_engine::GliomaPlanDisposition::Admitted
                        && plan.hold_order.is_empty()
                        && plan.approval_order.is_empty()
                        && plan.blocked_order.is_empty()
                    {
                        job.disposition = GliomaProgramScheduleJobDisposition::Completed;
                    } else {
                        job.disposition = GliomaProgramScheduleJobDisposition::NoRunnableActions;
                    }
                }
                Ok(plan) => planned.push((index, plan)),
                Err(error) => {
                    job.last_error = Some(error.to_string());
                    job.disposition = GliomaProgramScheduleJobDisposition::Blocked;
                    blocked.insert(job.spec.job_id.clone());
                }
            }
        }
        if planned.is_empty() {
            let active = jobs
                .iter()
                .filter(|job| {
                    matches!(job.disposition, GliomaProgramScheduleJobDisposition::Active)
                })
                .count();
            stop_reason = if active == 0 {
                if jobs.iter().all(|job| {
                    matches!(
                        job.disposition,
                        GliomaProgramScheduleJobDisposition::Completed
                            | GliomaProgramScheduleJobDisposition::NoRunnableActions
                            | GliomaProgramScheduleJobDisposition::BudgetExhausted
                    )
                }) {
                    GliomaProgramSchedulerStopReason::Completed
                } else {
                    GliomaProgramSchedulerStopReason::AllJobsBlocked
                }
            } else {
                GliomaProgramSchedulerStopReason::NoRunnableJobs
            };
            break;
        }
        planned.sort_by(|left, right| {
            plan_score(&jobs[right.0], &right.1)
                .cmp(&plan_score(&jobs[left.0], &left.1))
                .then_with(|| jobs[left.0].spec.job_id.cmp(&jobs[right.0].spec.job_id))
        });
        let planned_job_order = planned
            .iter()
            .map(|(index, _)| jobs[*index].spec.job_id.clone())
            .collect::<Vec<_>>();
        let mut selected = Vec::new();
        let mut reserved = BTreeMap::new();
        let mut selected_cost = 0_u64;
        let mut holds = Vec::new();
        for (index, plan) in planned {
            let cost = plan_cost(&plan);
            let usage = plan_usage(&plan);
            let reason = if selected.len() >= usize::from(request.max_jobs_per_round) {
                Some("max_jobs_per_round".to_string())
            } else if cost > remaining.saturating_sub(selected_cost) {
                Some("global_budget".to_string())
            } else if !fits_resources(&usage, &reserved, &capacities) {
                Some("resource_capacity".to_string())
            } else {
                None
            };
            if let Some(reason) = reason {
                holds.push(GliomaProgramScheduleHold {
                    job_id: jobs[index].spec.job_id.clone(),
                    reason,
                });
                jobs[index].age_rounds = jobs[index].age_rounds.saturating_add(1);
                continue;
            }
            selected.push((index, plan));
            selected_cost = selected_cost.saturating_add(cost);
            for (key, amount) in usage {
                let entry = reserved.entry(key).or_insert(0_u16);
                *entry = entry.saturating_add(amount);
            }
        }
        if selected.is_empty() {
            holds.sort_by(|left, right| left.job_id.cmp(&right.job_id));
            rounds.push(GliomaProgramSchedulerRound {
                round: round_number,
                planned_job_order,
                selected_job_order: Vec::new(),
                blocked_job_order: blocked.into_iter().collect(),
                holds,
                resource_usage: Vec::new(),
                executions: Vec::new(),
                budget_before_units: remaining,
                budget_after_units: remaining,
                cost_units: 0,
            });
            stop_reason = if remaining == 0 {
                GliomaProgramSchedulerStopReason::BudgetExhausted
            } else {
                GliomaProgramSchedulerStopReason::NoProgress
            };
            break;
        }
        let budget_before = remaining;
        let selected_job_order = selected
            .iter()
            .map(|(index, _)| jobs[*index].spec.job_id.clone())
            .collect::<Vec<_>>();
        let resource_usage = reserved
            .iter()
            .map(
                |((model_system, modality), used_units)| GliomaProgramResourceUsage {
                    model_system: *model_system,
                    modality: *modality,
                    used_units: *used_units,
                },
            )
            .collect::<Vec<_>>();
        let mut executions = Vec::new();
        for (index, plan) in selected {
            let action_order = plan.next_stage_order.clone();
            let cost = plan_cost(&plan);
            let mut execution_request = jobs[index].request.clone();
            execution_request.budget_units = execution_request
                .budget_units
                .max(cost.min(u64::from(u32::MAX)) as u32);
            match execute_glioma_research_director(&execution_request, executor) {
                Ok(run) => {
                    merge_evidence(&mut negative_evidence, &run.negative_evidence);
                    merge_evidence(&mut uncertainty, &run.uncertainty);
                    apply_director_result(&mut jobs[index], &run, cost);
                    executions.push(GliomaProgramScheduleExecution {
                        job_id: jobs[index].spec.job_id.clone(),
                        action_order,
                        cost_units: cost,
                        director: Some(run),
                        error: None,
                    });
                }
                Err(error) => {
                    let message = error.to_string();
                    jobs[index].last_error = Some(message.clone());
                    jobs[index].disposition = GliomaProgramScheduleJobDisposition::Failed;
                    jobs[index].age_rounds = 0;
                    jobs[index].budget_spent_units =
                        jobs[index].budget_spent_units.saturating_add(cost);
                    jobs[index].request.budget_units = jobs[index]
                        .request
                        .budget_units
                        .saturating_sub(cost.min(u64::from(u32::MAX)) as u32);
                    jobs[index]
                        .uncertainty
                        .insert(format!("scheduler-execution-failed:{}", message));
                    uncertainty.insert(format!(
                        "{}:scheduler-execution-failed",
                        jobs[index].spec.job_id
                    ));
                    executions.push(GliomaProgramScheduleExecution {
                        job_id: jobs[index].spec.job_id.clone(),
                        action_order,
                        cost_units: cost,
                        director: None,
                        error: Some(message),
                    });
                }
            }
        }
        spent = spent.saturating_add(selected_cost);
        for job in &mut jobs {
            if matches!(job.disposition, GliomaProgramScheduleJobDisposition::Active)
                && !selected_job_order.iter().any(|id| id == &job.spec.job_id)
            {
                job.age_rounds = job.age_rounds.saturating_add(1);
            }
        }
        let budget_after = request.global_budget_units.saturating_sub(spent);
        let mut held_job_order = holds
            .iter()
            .map(|hold| hold.job_id.clone())
            .collect::<Vec<_>>();
        held_job_order.sort();
        holds.sort_by(|left, right| left.job_id.cmp(&right.job_id));
        let mut blocked_job_order = blocked.into_iter().collect::<Vec<_>>();
        blocked_job_order.sort();
        rounds.push(GliomaProgramSchedulerRound {
            round: round_number,
            planned_job_order,
            selected_job_order,
            blocked_job_order,
            holds,
            resource_usage,
            executions,
            budget_before_units: budget_before,
            budget_after_units: budget_after,
            cost_units: selected_cost,
        });
        if jobs.iter().all(|job| {
            matches!(
                job.disposition,
                GliomaProgramScheduleJobDisposition::Completed
                    | GliomaProgramScheduleJobDisposition::NoRunnableActions
                    | GliomaProgramScheduleJobDisposition::BudgetExhausted
                    | GliomaProgramScheduleJobDisposition::Blocked
                    | GliomaProgramScheduleJobDisposition::Failed
            )
        }) {
            stop_reason = if jobs.iter().all(|job| {
                matches!(
                    job.disposition,
                    GliomaProgramScheduleJobDisposition::Completed
                )
            }) {
                GliomaProgramSchedulerStopReason::Completed
            } else {
                GliomaProgramSchedulerStopReason::NoRunnableJobs
            };
            break;
        }
    }

    let jobs_output = jobs.iter().map(state_from_job).collect::<Vec<_>>();
    let disposition = if jobs_output
        .iter()
        .all(|job| job.disposition == GliomaProgramScheduleJobDisposition::Completed)
    {
        GliomaProgramSchedulerDisposition::Completed
    } else if stop_reason == GliomaProgramSchedulerStopReason::BudgetExhausted {
        GliomaProgramSchedulerDisposition::BudgetExhausted
    } else if jobs_output.iter().all(|job| {
        matches!(
            job.disposition,
            GliomaProgramScheduleJobDisposition::Blocked
                | GliomaProgramScheduleJobDisposition::Failed
        )
    }) {
        GliomaProgramSchedulerDisposition::Blocked
    } else if matches!(
        stop_reason,
        GliomaProgramSchedulerStopReason::NoRunnableJobs
    ) {
        GliomaProgramSchedulerDisposition::NoRunnableJobs
    } else {
        GliomaProgramSchedulerDisposition::Partial
    };
    let next_step = match stop_reason {
        GliomaProgramSchedulerStopReason::Completed => {
            "review all job-level checkpoints and release only independently validated preclinical artifacts"
        }
        GliomaProgramSchedulerStopReason::BudgetExhausted => {
            "resume the scheduler with an explicit additional global budget"
        }
        GliomaProgramSchedulerStopReason::AllJobsBlocked => {
            "resolve failed intents, missing inputs, or authority holds before rescheduling"
        }
        GliomaProgramSchedulerStopReason::NoRunnableJobs => {
            "inspect job frontiers and supply the missing typed artifacts or approvals"
        }
        GliomaProgramSchedulerStopReason::MaxRounds => {
            "resume from the returned job checkpoints with a bounded continuation window"
        }
        GliomaProgramSchedulerStopReason::NoProgress => {
            "increase capacity or revise the resource-constrained job portfolio"
        }
    }
    .to_string();
    let mut output = GliomaProgramSchedulerRun {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        budget_units: request.global_budget_units,
        rounds,
        jobs: jobs_output,
        budget_spent_units: spent,
        remaining_budget_units: request.global_budget_units.saturating_sub(spent),
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        stop_reason,
        next_step,
        digest: ContentHash::of_bytes(b"unsealed-glioma-program-scheduler"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| GliomaProgramSchedulerError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

pub fn execute_glioma_program_scheduler_dry_run(
    request: &GliomaProgramSchedulerRequest,
) -> Result<GliomaProgramSchedulerRun, GliomaProgramSchedulerError> {
    let mut executor = DryRunGliomaActionExecutor;
    execute_glioma_program_scheduler(request, &mut executor)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma_engine::{
        GliomaResearchIntent, GliomaSelectionWeights, GliomaStageKind, LocalArtifactRef,
    };
    use bioprism_foundation::{AutonomyTier, PRECLINICAL_BOUNDARY};
    use bioprism_onco::OutputUse;

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: id.into(),
            content_hash: ContentHash::of_bytes(id.as_bytes()),
            content_type: "application/json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn director(job_id: &str, objective: &str) -> GliomaResearchDirectorRequest {
        GliomaResearchDirectorRequest {
            intent: GliomaResearchIntent {
                research_id: format!("research-{job_id}"),
                study_id: format!("study-{job_id}"),
                objective: objective.into(),
                output_uses: BTreeSet::from([OutputUse::CohortAnalysis]),
                model_systems: BTreeSet::from([GliomaModelSystem::Organoid]),
                modalities: BTreeSet::from([
                    GliomaModality::Transcriptomics,
                    GliomaModality::Imaging,
                    GliomaModality::Spatial,
                ]),
                input_artifacts: vec![artifact(&format!("input-{job_id}"))],
                requested_autonomy: AutonomyTier::A1,
                approval_reference: None,
                budget_units: 160,
                max_retries: 1,
                allow_instrument_execution: false,
                allow_federation: false,
                raw_data_local: true,
                aggregate_only: true,
                replay_identity: ContentHash::of_bytes(job_id.as_bytes()),
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            focus: super::super::director::GliomaDirectorFocus::MechanismFirst,
            completed_checkpoints: Vec::new(),
            budget_units: 80,
            max_actions: 2,
            approval_granted: false,
            allow_instrument_execution: false,
            allow_federation: false,
            selection_weights: GliomaSelectionWeights::default(),
            max_retries: 1,
            require_artifacts: true,
        }
    }

    fn request() -> GliomaProgramSchedulerRequest {
        GliomaProgramSchedulerRequest {
            objective: "schedule independent glioma mechanism programs".into(),
            jobs: vec![
                GliomaProgramScheduleJob {
                    job_id: "job-a".into(),
                    director: director("job-a", "identify invasion mechanisms"),
                    priority_milli: 900,
                    fairness_weight_milli: 400,
                },
                GliomaProgramScheduleJob {
                    job_id: "job-b".into(),
                    director: director("job-b", "identify stemness mechanisms"),
                    priority_milli: 500,
                    fairness_weight_milli: 900,
                },
            ],
            global_budget_units: 80,
            max_rounds: 2,
            max_jobs_per_round: 2,
            resource_capacities: vec![
                GliomaProgramResourceCapacity {
                    model_system: GliomaModelSystem::Organoid,
                    modality: GliomaModality::Literature,
                    capacity_units: 4,
                },
                GliomaProgramResourceCapacity {
                    model_system: GliomaModelSystem::Organoid,
                    modality: GliomaModality::Spatial,
                    capacity_units: 4,
                },
            ],
        }
    }

    #[test]
    fn scheduler_fairly_batches_independent_frontiers_and_replays() {
        let output = execute_glioma_program_scheduler_dry_run(&request()).unwrap();
        assert_eq!(output.feature_id, FEATURE_ID);
        assert_eq!(output.rounds.len(), 2);
        assert_eq!(output.jobs.len(), 2);
        assert!(output.rounds[0]
            .selected_job_order
            .contains(&"job-a".into()));
        assert!(output.rounds[0]
            .selected_job_order
            .contains(&"job-b".into()));
        assert!(output
            .negative_evidence
            .iter()
            .any(|item| item.contains("synthetic-dry-run")));
        output.validate().unwrap();
    }

    #[test]
    fn scheduler_holds_when_resource_capacity_is_not_declared() {
        let mut request = request();
        request.resource_capacities = vec![GliomaProgramResourceCapacity {
            model_system: GliomaModelSystem::Organoid,
            modality: GliomaModality::Transcriptomics,
            capacity_units: 1,
        }];
        let output = execute_glioma_program_scheduler_dry_run(&request).unwrap();
        assert!(output.rounds.iter().any(|round| {
            round
                .holds
                .iter()
                .any(|hold| hold.reason == "resource_capacity")
        }));
        assert!(output
            .uncertainty
            .iter()
            .all(|item| !item.contains("clinical")));
    }

    #[test]
    fn scheduler_rejects_duplicate_jobs_and_unbounded_capacity() {
        let mut duplicate_request = request();
        duplicate_request.jobs[1].job_id = duplicate_request.jobs[0].job_id.clone();
        assert!(matches!(
            execute_glioma_program_scheduler_dry_run(&duplicate_request),
            Err(GliomaProgramSchedulerError::InvalidRequest(_))
        ));
        let mut invalid_request = request();
        invalid_request.resource_capacities[0].capacity_units = 0;
        assert!(matches!(
            execute_glioma_program_scheduler_dry_run(&invalid_request),
            Err(GliomaProgramSchedulerError::InvalidRequest(_))
        ));
    }

    #[test]
    fn scheduler_keeps_stage_identity_typed() {
        let mut request = request();
        request.jobs[0]
            .director
            .completed_checkpoints
            .push(GliomaDirectorCheckpoint {
                stage_kind: GliomaStageKind::IntentNormalization,
                artifact_id: "checkpoint".into(),
                artifact: artifact("checkpoint"),
            });
        request.jobs[0]
            .director
            .completed_checkpoints
            .sort_by_key(|checkpoint| checkpoint.stage_kind);
        let output = execute_glioma_program_scheduler_dry_run(&request).unwrap();
        assert!(output.jobs[0]
            .completed_stage_order
            .contains(&"intent-normalization".into()));
    }
}
