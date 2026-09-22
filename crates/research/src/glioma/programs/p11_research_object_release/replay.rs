//! Autonomous reproducibility replay campaigns for preclinical glioma research.
//!
//! A research-object manifest is not evidence that another site can reproduce the computation.
//! This module turns a release candidate into an executable, dependency-aware replay campaign.
//! Institution-local executors rerun declared program tasks and return only typed artifact hashes;
//! the controller compares those hashes, blocks dependants after a mismatch, and never upgrades a
//! partial replay into a reproducibility claim.

use crate::glioma::release::ReleaseStatus;
use crate::glioma::{
    build_research_object_manifest, ResearchObjectManifest, ResearchObjectRequest,
};
use crate::glioma_engine::LocalArtifactRef;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P11-F10";
pub const OUTPUT_SCHEMA: &str = "GliomaResearchObjectReplayCampaign1@1";
pub const MAX_ROUNDS: u16 = 32;
pub const MAX_TASKS: usize = 512;
pub const MAX_RETRIES: u8 = 6;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayTask {
    pub task_id: String,
    pub program_id: String,
    pub artifact_id: String,
    pub expected_content_hash: ContentHash,
    pub cost_units: u32,
    pub required: bool,
    pub deterministic: bool,
    pub depends_on: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayObservationStatus {
    Match,
    Mismatch,
    Unavailable,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayObservation {
    pub task_id: String,
    pub status: ReplayObservationStatus,
    pub observed_content_hash: Option<ContentHash>,
    pub artifact: Option<LocalArtifactRef>,
    pub runtime_ticks: u64,
    pub note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayCampaignRequest {
    pub release: ResearchObjectRequest,
    pub tasks: Vec<ReplayTask>,
    pub budget_units: u64,
    pub max_rounds: u16,
    pub max_retries: u8,
    pub min_coverage_milli: u16,
    pub require_exact_hash: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplayExecutionFailure {
    pub reason: String,
    pub retryable: bool,
}

/// A local worker replays one declared program task. It owns the code, data, compute provider,
/// and credentials; the campaign receives only a content-addressed outcome.
pub trait ReplayCampaignExecutor {
    fn replay_task(
        &mut self,
        task: &ReplayTask,
        request: &ReplayCampaignRequest,
        attempt: u8,
    ) -> Result<ReplayObservation, ReplayExecutionFailure>;
}

/// Deterministic sandbox executor. Deterministic tasks produce matching synthetic hashes; tasks
/// declared non-deterministic remain explicitly unavailable rather than being promoted.
#[derive(Debug, Default)]
pub struct DryRunReplayCampaignExecutor;

impl ReplayCampaignExecutor for DryRunReplayCampaignExecutor {
    fn replay_task(
        &mut self,
        task: &ReplayTask,
        _request: &ReplayCampaignRequest,
        _attempt: u8,
    ) -> Result<ReplayObservation, ReplayExecutionFailure> {
        if !task.deterministic {
            return Ok(ReplayObservation {
                task_id: task.task_id.clone(),
                status: ReplayObservationStatus::Unavailable,
                observed_content_hash: None,
                artifact: None,
                runtime_ticks: 0,
                note: "dry-run refuses to fabricate a non-deterministic replay".into(),
            });
        }
        Ok(ReplayObservation {
            task_id: task.task_id.clone(),
            status: ReplayObservationStatus::Match,
            observed_content_hash: Some(task.expected_content_hash.clone()),
            artifact: Some(LocalArtifactRef {
                artifact_id: task.artifact_id.clone(),
                content_hash: task.expected_content_hash.clone(),
                content_type: "application/vnd.aurora.glioma.replay-artifact+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            }),
            runtime_ticks: u64::from(task.cost_units).saturating_mul(10),
            note: "synthetic replay only; no biological evidence".into(),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayCampaignRound {
    pub round: u16,
    pub selected_order: Vec<String>,
    pub matched_order: Vec<String>,
    pub mismatched_order: Vec<String>,
    pub unavailable_order: Vec<String>,
    pub failed_order: Vec<String>,
    pub cost_units: u32,
    pub budget_before_units: u64,
    pub budget_after_units: u64,
    pub retry_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayCampaignDisposition {
    Reproducible,
    Partial,
    NonReproducible,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReplayCampaignStopReason {
    Reproducible,
    BudgetExhausted,
    NoRunnableTasks,
    MaxRounds,
    ExecutorFailed,
    NoProgress,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplayCampaign {
    pub feature_id: String,
    pub output_schema: String,
    pub research_id: String,
    pub objective: String,
    pub manifest: ResearchObjectManifest,
    pub rounds: Vec<ReplayCampaignRound>,
    pub observations: Vec<ReplayObservation>,
    pub matched_order: Vec<String>,
    pub mismatched_order: Vec<String>,
    pub unavailable_order: Vec<String>,
    pub failed_order: Vec<String>,
    pub coverage_milli: u16,
    pub required_coverage_milli: u16,
    pub exact_match: bool,
    pub retry_count: u32,
    pub budget_spent_units: u64,
    pub remaining_budget_units: u64,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: ReplayCampaignDisposition,
    pub stop_reason: ReplayCampaignStopReason,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReplayCampaignError {
    #[error("replay campaign request is invalid: {0}")]
    InvalidRequest(String),
    #[error("replay campaign planning failed: {0}")]
    Planning(String),
    #[error("replay campaign execution failed: {0}")]
    Execution(String),
    #[error("replay campaign output is invalid: {0}")]
    InvalidOutput(String),
    #[error("replay campaign digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(campaign: &ReplayCampaign) -> serde_json::Value {
    serde_json::json!({
        "feature_id": campaign.feature_id,
        "output_schema": campaign.output_schema,
        "research_id": campaign.research_id,
        "objective": campaign.objective,
        "manifest": campaign.manifest,
        "rounds": campaign.rounds,
        "observations": campaign.observations,
        "matched_order": campaign.matched_order,
        "mismatched_order": campaign.mismatched_order,
        "unavailable_order": campaign.unavailable_order,
        "failed_order": campaign.failed_order,
        "coverage_milli": campaign.coverage_milli,
        "required_coverage_milli": campaign.required_coverage_milli,
        "exact_match": campaign.exact_match,
        "retry_count": campaign.retry_count,
        "budget_spent_units": campaign.budget_spent_units,
        "remaining_budget_units": campaign.remaining_budget_units,
        "negative_evidence": campaign.negative_evidence,
        "uncertainty": campaign.uncertainty,
        "disposition": campaign.disposition,
        "stop_reason": campaign.stop_reason,
    })
}

fn task_map(tasks: &[ReplayTask]) -> BTreeMap<String, &ReplayTask> {
    tasks
        .iter()
        .map(|task| (task.task_id.clone(), task))
        .collect()
}

fn validate_tasks(request: &ReplayCampaignRequest) -> Result<(), ReplayCampaignError> {
    if request.max_rounds == 0
        || request.max_rounds > MAX_ROUNDS
        || request.max_retries > MAX_RETRIES
        || request.budget_units == 0
        || request.min_coverage_milli > 1_000
        || request.tasks.is_empty()
        || request.tasks.len() > MAX_TASKS
    {
        return Err(ReplayCampaignError::InvalidRequest(
            "bounded rounds, retries, budget, coverage, and replay tasks are required".into(),
        ));
    }
    let manifest = build_research_object_manifest(&request.release)
        .map_err(|error| ReplayCampaignError::InvalidRequest(error.to_string()))?;
    let mut ids = BTreeSet::new();
    let mut artifacts = BTreeSet::new();
    for task in &request.tasks {
        if task.task_id.trim().is_empty()
            || !ids.insert(task.task_id.clone())
            || task.program_id.trim().is_empty()
            || !manifest.program_order.contains(&task.program_id)
            || task.artifact_id.trim().is_empty()
            || !artifacts.insert(task.artifact_id.clone())
            || task.expected_content_hash.as_str().len() != 64
            || task.cost_units == 0
            || task
                .depends_on
                .iter()
                .any(|dependency| dependency == &task.task_id)
        {
            return Err(ReplayCampaignError::InvalidRequest(
                "task identity, release-program binding, artifact uniqueness, hash, cost, or dependency bounds are invalid".into(),
            ));
        }
    }
    let map = task_map(&request.tasks);
    for task in &request.tasks {
        let mut dependencies = BTreeSet::new();
        for dependency in &task.depends_on {
            if !dependencies.insert(dependency) || !map.contains_key(dependency) {
                return Err(ReplayCampaignError::InvalidRequest(
                    "every dependency must be unique and reference a declared task".into(),
                ));
            }
        }
    }
    // Kahn's algorithm catches cycles before an executor can be invoked.
    let mut remaining = request
        .tasks
        .iter()
        .map(|task| (task.task_id.clone(), task.depends_on.len()))
        .collect::<BTreeMap<_, _>>();
    let mut queue = remaining
        .iter()
        .filter(|(_, degree)| **degree == 0)
        .map(|(id, _)| id.clone())
        .collect::<BTreeSet<_>>();
    let mut visited = 0_usize;
    while let Some(id) = queue.pop_first() {
        visited += 1;
        for task in &request.tasks {
            if task.depends_on.iter().any(|dependency| dependency == &id) {
                let degree = remaining
                    .get_mut(&task.task_id)
                    .expect("task degree exists");
                *degree = degree.saturating_sub(1);
                if *degree == 0 {
                    queue.insert(task.task_id.clone());
                }
            }
        }
    }
    if visited != request.tasks.len() {
        return Err(ReplayCampaignError::InvalidRequest(
            "replay task dependencies must be acyclic".into(),
        ));
    }
    Ok(())
}

fn validate_observation(
    observation: &ReplayObservation,
    task: &ReplayTask,
) -> Result<(), ReplayCampaignError> {
    if observation.task_id != task.task_id || observation.note.trim().is_empty() {
        return Err(ReplayCampaignError::Execution(format!(
            "executor returned an invalid replay observation for task {}",
            task.task_id
        )));
    }
    match observation.status {
        ReplayObservationStatus::Match
            if observation.observed_content_hash.as_ref() != Some(&task.expected_content_hash)
                || observation.artifact.is_none() =>
        {
            return Err(ReplayCampaignError::Execution(format!(
                "matching replay observation for task {} is not bound to the expected hash",
                task.task_id
            )));
        }
        ReplayObservationStatus::Mismatch
            if observation.observed_content_hash.is_none()
                || observation.observed_content_hash.as_ref()
                    == Some(&task.expected_content_hash)
                || observation.artifact.is_none() =>
        {
            return Err(ReplayCampaignError::Execution(format!(
                "mismatching replay observation for task {} lacks a distinct observed hash",
                task.task_id
            )));
        }
        ReplayObservationStatus::Unavailable | ReplayObservationStatus::Blocked
            if observation.observed_content_hash.is_some() || observation.artifact.is_some() =>
        {
            return Err(ReplayCampaignError::Execution(format!(
                "unavailable replay observation for task {} carries an unverified artifact",
                task.task_id
            )));
        }
        _ => {}
    }
    if let Some(artifact) = &observation.artifact {
        artifact
            .validate()
            .map_err(|error| ReplayCampaignError::Execution(error.to_string()))?;
        if artifact.artifact_id != task.artifact_id
            || Some(&artifact.content_hash) != observation.observed_content_hash.as_ref()
        {
            return Err(ReplayCampaignError::Execution(
                "replay artifact does not match the declared task or observed hash".into(),
            ));
        }
    }
    Ok(())
}

fn observation_for<'a>(
    observations: &'a [ReplayObservation],
    task_id: &str,
) -> Option<&'a ReplayObservation> {
    observations
        .iter()
        .find(|observation| observation.task_id == task_id)
}

fn is_match(observation: Option<&ReplayObservation>) -> bool {
    observation.is_some_and(|value| value.status == ReplayObservationStatus::Match)
}

fn coverage(tasks: &[ReplayTask], observations: &[ReplayObservation]) -> (u16, u16, bool) {
    let matches = tasks
        .iter()
        .filter(|task| is_match(observation_for(observations, &task.task_id)))
        .count() as u64;
    let required = tasks.iter().filter(|task| task.required).count() as u64;
    let required_matches = tasks
        .iter()
        .filter(|task| task.required && is_match(observation_for(observations, &task.task_id)))
        .count() as u64;
    let score = if tasks.is_empty() {
        0
    } else {
        (matches.saturating_mul(1_000) / tasks.len() as u64) as u16
    };
    let required_score = if required == 0 {
        1_000
    } else {
        required_matches
            .saturating_mul(1_000)
            .checked_div(required)
            .unwrap_or(0) as u16
    };
    let exact = matches == tasks.len() as u64;
    (score, required_score, exact)
}

impl ReplayCampaign {
    pub fn validate(&self) -> Result<(), ReplayCampaignError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.research_id.trim().is_empty()
            || self.objective.trim().is_empty()
            || self.rounds.len() > MAX_ROUNDS as usize
            || !canonical(&self.matched_order)
            || !canonical(&self.mismatched_order)
            || !canonical(&self.unavailable_order)
            || !canonical(&self.failed_order)
            || self.required_coverage_milli > 1_000
            || self.coverage_milli > 1_000
            || self.matched_order.iter().any(|id| {
                self.mismatched_order.binary_search(id).is_ok()
                    || self.unavailable_order.binary_search(id).is_ok()
                    || self.failed_order.binary_search(id).is_ok()
            })
        {
            return Err(ReplayCampaignError::InvalidOutput(
                "identity, coverage, canonical partitions, or replay outcome fields are invalid"
                    .into(),
            ));
        }
        self.manifest
            .validate()
            .map_err(|error| ReplayCampaignError::InvalidOutput(error.to_string()))?;
        let mut task_ids = BTreeSet::new();
        let mut seen_rounds = BTreeSet::new();
        let mut spent = 0_u64;
        let mut retries = 0_u32;
        for round in &self.rounds {
            if round.round == 0
                || !seen_rounds.insert(round.round)
                || !canonical(&round.selected_order)
                || !canonical(&round.matched_order)
                || !canonical(&round.mismatched_order)
                || !canonical(&round.unavailable_order)
                || !canonical(&round.failed_order)
                || round.budget_after_units > round.budget_before_units
                || u64::from(round.cost_units)
                    != round
                        .budget_before_units
                        .saturating_sub(round.budget_after_units)
            {
                return Err(ReplayCampaignError::InvalidOutput(
                    "round ordering or budget invariants are invalid".into(),
                ));
            }
            for id in round
                .matched_order
                .iter()
                .chain(round.mismatched_order.iter())
                .chain(round.unavailable_order.iter())
                .chain(round.failed_order.iter())
            {
                if !task_ids.insert(id.clone()) {
                    return Err(ReplayCampaignError::InvalidOutput(
                        "a replay task has more than one terminal outcome".into(),
                    ));
                }
            }
            spent = spent.saturating_add(u64::from(round.cost_units));
            retries = retries.saturating_add(round.retry_count);
        }
        if spent != self.budget_spent_units || retries != self.retry_count {
            return Err(ReplayCampaignError::InvalidOutput(
                "budget or retry count does not reconcile with rounds".into(),
            ));
        }
        let mut observed_ids = BTreeSet::new();
        for observation in &self.observations {
            if !observed_ids.insert(observation.task_id.clone()) {
                return Err(ReplayCampaignError::InvalidOutput(
                    "duplicate replay observations are not allowed".into(),
                ));
            }
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ReplayCampaignError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ReplayCampaignError::InvalidOutput(
                "campaign digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Execute a dependency-aware reproducibility campaign. A matching hash is necessary but not
/// sufficient for release: the manifest must also be ready for accountable signing, required
/// coverage must clear the caller's gate, and no required task may be mismatched or unavailable.
pub fn execute_glioma_replay_campaign<E: ReplayCampaignExecutor>(
    request: &ReplayCampaignRequest,
    executor: &mut E,
) -> Result<ReplayCampaign, ReplayCampaignError> {
    validate_tasks(request)?;
    let manifest = build_research_object_manifest(&request.release)
        .map_err(|error| ReplayCampaignError::Planning(error.to_string()))?;
    let task_registry = task_map(&request.tasks);
    let mut observations = Vec::new();
    let mut rounds = Vec::new();
    let mut failed = BTreeSet::new();
    let mut spent = 0_u64;
    let mut retries = 0_u32;
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut stop_reason = ReplayCampaignStopReason::MaxRounds;

    for round_number in 1..=request.max_rounds {
        let (coverage_now, required_now, exact_now) = coverage(&request.tasks, &observations);
        if manifest.release_status == ReleaseStatus::ReadyForSigning
            && required_now >= request.min_coverage_milli
            && (!request.require_exact_hash || exact_now)
            && observations.iter().all(|observation| {
                observation.status == ReplayObservationStatus::Match
                    || !task_registry
                        .get(&observation.task_id)
                        .is_some_and(|task| task.required)
            })
        {
            stop_reason = ReplayCampaignStopReason::Reproducible;
            break;
        }
        let remaining = request.budget_units.saturating_sub(spent);
        if remaining == 0 {
            stop_reason = ReplayCampaignStopReason::BudgetExhausted;
            break;
        }
        let mut eligible = request
            .tasks
            .iter()
            .filter(|task| {
                observation_for(&observations, &task.task_id).is_none()
                    && !failed.contains(&task.task_id)
                    && u64::from(task.cost_units) <= remaining
                    && task
                        .depends_on
                        .iter()
                        .all(|dependency| is_match(observation_for(&observations, dependency)))
            })
            .collect::<Vec<_>>();
        eligible.sort_by(|left, right| {
            right
                .required
                .cmp(&left.required)
                .then_with(|| left.cost_units.cmp(&right.cost_units))
                .then_with(|| left.task_id.cmp(&right.task_id))
        });
        if eligible.is_empty() {
            let unresolved_dependencies = request.tasks.iter().any(|task| {
                observation_for(&observations, &task.task_id).is_none()
                    && !failed.contains(&task.task_id)
                    && task.depends_on.iter().any(|dependency| {
                        observation_for(&observations, dependency).is_some_and(|observation| {
                            observation.status != ReplayObservationStatus::Match
                        })
                    })
            });
            stop_reason = if unresolved_dependencies {
                negative_evidence.insert("replay-dependency-blocked".into());
                ReplayCampaignStopReason::NoRunnableTasks
            } else if request.tasks.iter().any(|task| {
                observation_for(&observations, &task.task_id).is_none()
                    && u64::from(task.cost_units) > remaining
            }) {
                ReplayCampaignStopReason::BudgetExhausted
            } else {
                ReplayCampaignStopReason::NoRunnableTasks
            };
            break;
        }
        let mut selected = Vec::new();
        let mut round_cost = 0_u64;
        for task in eligible {
            let cost = u64::from(task.cost_units);
            if selected.is_empty() || round_cost.saturating_add(cost) <= remaining {
                selected.push(task);
                round_cost = round_cost.saturating_add(cost);
            }
            if selected.len() >= 8 {
                break;
            }
        }
        if selected.is_empty() {
            stop_reason = ReplayCampaignStopReason::BudgetExhausted;
            break;
        }
        let before_budget = remaining;
        let mut matched = Vec::new();
        let mut mismatched = Vec::new();
        let mut unavailable = Vec::new();
        let mut failed_round = Vec::new();
        let mut retry_round = 0_u32;
        let mut progress = false;
        for task in selected.iter().copied() {
            let mut accepted = None;
            for attempt in 1..=request.max_retries.saturating_add(1) {
                match executor.replay_task(task, request, attempt) {
                    Ok(observation) => {
                        validate_observation(&observation, task)?;
                        if observation_for(&observations, &task.task_id).is_some() {
                            return Err(ReplayCampaignError::Execution(
                                "executor returned a duplicate replay task".into(),
                            ));
                        }
                        accepted = Some(observation);
                        break;
                    }
                    Err(error) => {
                        if error.reason.trim().is_empty() {
                            return Err(ReplayCampaignError::Execution(
                                "executor returned an empty replay failure reason".into(),
                            ));
                        }
                        if error.retryable && attempt <= request.max_retries {
                            retries = retries.saturating_add(1);
                            retry_round = retry_round.saturating_add(1);
                            continue;
                        }
                        failed_round.push(task.task_id.clone());
                        failed.insert(task.task_id.clone());
                        break;
                    }
                }
            }
            if let Some(observation) = accepted {
                match observation.status {
                    ReplayObservationStatus::Match => matched.push(task.task_id.clone()),
                    ReplayObservationStatus::Mismatch => {
                        mismatched.push(task.task_id.clone());
                        negative_evidence.insert(format!("replay-hash-mismatch:{}", task.task_id));
                    }
                    ReplayObservationStatus::Unavailable => {
                        unavailable.push(task.task_id.clone());
                        uncertainty.insert(format!("replay-unavailable:{}", task.task_id));
                    }
                    ReplayObservationStatus::Blocked => {
                        unavailable.push(task.task_id.clone());
                        negative_evidence.insert(format!("replay-blocked:{}", task.task_id));
                    }
                }
                observations.push(observation);
                progress = true;
            } else {
                break;
            }
        }
        matched.sort();
        mismatched.sort();
        unavailable.sort();
        failed_round.sort();
        spent = spent.saturating_add(round_cost);
        let after_budget = request.budget_units.saturating_sub(spent);
        let (coverage_after, _, _) = coverage(&request.tasks, &observations);
        if coverage_after < request.min_coverage_milli {
            uncertainty.insert("replay-coverage-gate-not-met".into());
        }
        rounds.push(ReplayCampaignRound {
            round: round_number,
            selected_order: selected.iter().map(|task| task.task_id.clone()).collect(),
            matched_order: matched,
            mismatched_order: mismatched,
            unavailable_order: unavailable,
            failed_order: failed_round.clone(),
            cost_units: round_cost.min(u64::from(u32::MAX)) as u32,
            budget_before_units: before_budget,
            budget_after_units: after_budget,
            retry_count: retry_round,
        });
        if !failed_round.is_empty() {
            stop_reason = ReplayCampaignStopReason::ExecutorFailed;
            break;
        }
        if !progress {
            stop_reason = ReplayCampaignStopReason::NoProgress;
            break;
        }
        if after_budget == 0 {
            stop_reason = ReplayCampaignStopReason::BudgetExhausted;
            break;
        }
        let _ = coverage_now;
    }

    let (coverage_milli, required_coverage_milli, exact_match) =
        coverage(&request.tasks, &observations);
    let mut matched_order = observations
        .iter()
        .filter(|observation| observation.status == ReplayObservationStatus::Match)
        .map(|observation| observation.task_id.clone())
        .collect::<Vec<_>>();
    let mut mismatched_order = observations
        .iter()
        .filter(|observation| observation.status == ReplayObservationStatus::Mismatch)
        .map(|observation| observation.task_id.clone())
        .collect::<Vec<_>>();
    let mut unavailable_order = observations
        .iter()
        .filter(|observation| {
            matches!(
                observation.status,
                ReplayObservationStatus::Unavailable | ReplayObservationStatus::Blocked
            )
        })
        .map(|observation| observation.task_id.clone())
        .collect::<Vec<_>>();
    matched_order.sort();
    mismatched_order.sort();
    unavailable_order.sort();
    let required_mismatch = request.tasks.iter().any(|task| {
        task.required
            && observation_for(&observations, &task.task_id)
                .is_some_and(|observation| observation.status == ReplayObservationStatus::Mismatch)
    });
    if manifest.release_status == ReleaseStatus::ReadyForSigning
        && required_coverage_milli >= request.min_coverage_milli
        && (!request.require_exact_hash || exact_match)
        && !required_mismatch
    {
        stop_reason = ReplayCampaignStopReason::Reproducible;
    }
    let disposition = if stop_reason == ReplayCampaignStopReason::Reproducible {
        ReplayCampaignDisposition::Reproducible
    } else if required_mismatch || !mismatched_order.is_empty() {
        ReplayCampaignDisposition::NonReproducible
    } else if !failed.is_empty() || matches!(stop_reason, ReplayCampaignStopReason::BudgetExhausted)
    {
        ReplayCampaignDisposition::Blocked
    } else if !unavailable_order.is_empty() || required_coverage_milli < request.min_coverage_milli
    {
        ReplayCampaignDisposition::Partial
    } else if observations.is_empty() {
        ReplayCampaignDisposition::Unresolved
    } else {
        ReplayCampaignDisposition::Partial
    };
    let mut output = ReplayCampaign {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        research_id: request.release.research_id.clone(),
        objective: request.release.objective.clone(),
        manifest,
        rounds,
        observations,
        matched_order,
        mismatched_order,
        unavailable_order,
        failed_order: failed.into_iter().collect(),
        coverage_milli,
        required_coverage_milli,
        exact_match,
        retry_count: retries,
        budget_spent_units: spent,
        remaining_budget_units: request.budget_units.saturating_sub(spent),
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        stop_reason,
        digest: ContentHash::of_bytes(b"unsealed-glioma-replay-campaign"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ReplayCampaignError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma_engine::LocalArtifactRef;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"label": label})).unwrap()
    }

    fn release() -> ResearchObjectRequest {
        ResearchObjectRequest {
            research_id: "replay-research".into(),
            study_id: "replay-study".into(),
            objective: "replay a preclinical glioma mechanism result".into(),
            plan_digest: hash("plan"),
            execution_digest: hash("execution"),
            replay_identity: hash("replay"),
            program_order: vec!["p05-mechanism".into(), "p10-analysis".into()],
            artifacts: vec![LocalArtifactRef {
                artifact_id: "artifact-main".into(),
                content_hash: hash("artifact-main"),
                content_type: "application/vnd.aurora.glioma.result+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            }],
            negative_evidence: vec!["null-result-preserved".into()],
            limitations: vec!["single-model-system".into()],
            raw_data_local: true,
            aggregate_only: true,
        }
    }

    fn request() -> ReplayCampaignRequest {
        ReplayCampaignRequest {
            release: release(),
            tasks: vec![
                ReplayTask {
                    task_id: "mechanism-replay".into(),
                    program_id: "p05-mechanism".into(),
                    artifact_id: "artifact-main".into(),
                    expected_content_hash: hash("artifact-main"),
                    cost_units: 1,
                    required: true,
                    deterministic: true,
                    depends_on: Vec::new(),
                },
                ReplayTask {
                    task_id: "analysis-replay".into(),
                    program_id: "p10-analysis".into(),
                    artifact_id: "artifact-analysis".into(),
                    expected_content_hash: hash("artifact-analysis"),
                    cost_units: 1,
                    required: true,
                    deterministic: true,
                    depends_on: vec!["mechanism-replay".into()],
                },
            ],
            budget_units: 2,
            max_rounds: 3,
            max_retries: 1,
            min_coverage_milli: 1_000,
            require_exact_hash: true,
        }
    }

    #[test]
    fn replay_campaign_runs_dependency_order_and_is_stable() {
        let request = request();
        let mut first_executor = DryRunReplayCampaignExecutor;
        let mut second_executor = DryRunReplayCampaignExecutor;
        let first = execute_glioma_replay_campaign(&request, &mut first_executor).unwrap();
        let second = execute_glioma_replay_campaign(&request, &mut second_executor).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.disposition, ReplayCampaignDisposition::Reproducible);
        assert_eq!(first.rounds.len(), 2);
        assert!(first.exact_match);
        first.validate().unwrap();
    }

    #[test]
    fn non_deterministic_task_is_unavailable_not_fabricated() {
        let mut request = request();
        request.tasks[0].deterministic = false;
        let mut executor = DryRunReplayCampaignExecutor;
        let output = execute_glioma_replay_campaign(&request, &mut executor).unwrap();
        assert_eq!(output.disposition, ReplayCampaignDisposition::Partial);
        assert_eq!(output.unavailable_order, vec!["mechanism-replay"]);
        assert!(output
            .uncertainty
            .iter()
            .any(|value| value.contains("coverage-gate")));
    }

    #[test]
    fn mismatching_hash_is_non_reproducible_and_blocks_dependants() {
        struct MismatchExecutor;
        impl ReplayCampaignExecutor for MismatchExecutor {
            fn replay_task(
                &mut self,
                task: &ReplayTask,
                _request: &ReplayCampaignRequest,
                _attempt: u8,
            ) -> Result<ReplayObservation, ReplayExecutionFailure> {
                let observed = hash("wrong-output");
                Ok(ReplayObservation {
                    task_id: task.task_id.clone(),
                    status: ReplayObservationStatus::Mismatch,
                    observed_content_hash: Some(observed.clone()),
                    artifact: Some(LocalArtifactRef {
                        artifact_id: task.artifact_id.clone(),
                        content_hash: observed,
                        content_type: "application/json".into(),
                        local_only: true,
                        contains_human_data: false,
                        contains_direct_identifiers: false,
                    }),
                    runtime_ticks: 1,
                    note: "independent replay hash differs".into(),
                })
            }
        }
        let mut executor = MismatchExecutor;
        let output = execute_glioma_replay_campaign(&request(), &mut executor).unwrap();
        assert_eq!(
            output.disposition,
            ReplayCampaignDisposition::NonReproducible
        );
        assert_eq!(output.mismatched_order, vec!["mechanism-replay"]);
        assert!(output.observations.len() < request().tasks.len());
    }

    #[test]
    fn dependency_cycle_is_rejected_before_executor() {
        let mut request = request();
        request.tasks[0].depends_on = vec!["analysis-replay".into()];
        let mut executor = DryRunReplayCampaignExecutor;
        let error = execute_glioma_replay_campaign(&request, &mut executor).unwrap_err();
        assert!(error.to_string().contains("acyclic"));
    }
}
