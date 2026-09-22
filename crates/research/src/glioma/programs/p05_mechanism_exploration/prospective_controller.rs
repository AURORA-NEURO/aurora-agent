//! Prospective high-throughput mechanism workflow control for preclinical glioma research.
//!
//! The multi-study compiler produces a bounded portfolio once. A production research engine
//! also needs a rolling controller: it admits a fresh batch, accounts for occupied capacity,
//! consumes typed local outcomes, detects drift and low-quality returns, and requeues unresolved
//! work without treating silence as success. This feature performs that control transition while
//! keeping every effect behind the institution-local executor seam.

use super::multi_study_workflow::{MechanismMultiStudyTaskStatus, MechanismMultiStudyWorkflowPlan};
use crate::glioma_engine::{GliomaModality, GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P05-F15";
pub const OUTPUT_SCHEMA: &str = "GliomaMechanismProspectiveController1@1";
pub const MAX_TASKS: usize = 4_096;
pub const MAX_OBSERVATIONS: usize = 4_096;
pub const MAX_RESOURCES: usize = 512;
pub const MAX_ATTEMPTS: u16 = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismProspectiveOutcome {
    Completed,
    Contradicted,
    Unresolved,
    Failed,
    Missing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismProspectiveTaskStatus {
    Queued,
    Inflight,
    Requeue,
    Retired,
    Blocked,
    Hold,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismProspectiveTaskPolicy {
    pub task_id: String,
    pub attempts_used: u16,
    pub max_attempts: u16,
    pub priority_boost_milli: u16,
    pub stale_after_epochs: u64,
    pub allow_requeue: bool,
    pub requires_approval: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismProspectiveObservation {
    pub task_id: String,
    pub epoch: u64,
    pub outcome: MechanismProspectiveOutcome,
    pub quality_milli: u16,
    pub drift_milli: u16,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismProspectiveResourceCapacity {
    pub model_system: GliomaModelSystem,
    pub modality: GliomaModality,
    pub capacity_units: u16,
    pub occupied_units: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismProspectiveControllerRequest {
    pub objective: String,
    pub portfolio: MechanismMultiStudyWorkflowPlan,
    pub policies: Vec<MechanismProspectiveTaskPolicy>,
    pub observations: Vec<MechanismProspectiveObservation>,
    pub resources: Vec<MechanismProspectiveResourceCapacity>,
    pub current_epoch: u64,
    pub budget_units: u64,
    pub max_inflight: usize,
    pub max_queue_depth: usize,
    pub minimum_quality_milli: u16,
    pub maximum_drift_milli: u16,
    pub allow_approval_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismProspectiveTaskDecision {
    pub task_id: String,
    pub status: MechanismProspectiveTaskStatus,
    pub priority_milli: u16,
    pub attempts_used: u16,
    pub next_attempt: u16,
    pub observed_epoch: Option<u64>,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismProspectiveResourceUsage {
    pub model_system: GliomaModelSystem,
    pub modality: GliomaModality,
    pub capacity_units: u16,
    pub occupied_units: u16,
    pub reserved_units: u16,
    pub remaining_units: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismProspectiveDisposition {
    Ready,
    Partial,
    QueueBlocked,
    DriftHalt,
    Exhausted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismProspectiveControllerPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub portfolio_digest: ContentHash,
    pub epoch: u64,
    pub next_epoch: u64,
    pub task_order: Vec<String>,
    pub decisions: Vec<MechanismProspectiveTaskDecision>,
    pub queue_order: Vec<String>,
    pub inflight_order: Vec<String>,
    pub requeue_order: Vec<String>,
    pub retired_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub hold_order: Vec<String>,
    pub resource_usage: Vec<MechanismProspectiveResourceUsage>,
    pub total_cost_units: u64,
    pub budget_units: u64,
    pub budget_remaining_units: u64,
    pub drift_alerts: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: MechanismProspectiveDisposition,
    pub next_route: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MechanismProspectiveControllerError {
    #[error("prospective mechanism controller request is invalid: {0}")]
    InvalidRequest(String),
    #[error("prospective mechanism controller input is invalid: {0}")]
    InvalidInput(String),
    #[error("prospective mechanism controller output is invalid: {0}")]
    InvalidOutput(String),
    #[error("prospective mechanism controller digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_nonempty(values: &[String]) -> bool {
    values.iter().all(|value| !value.trim().is_empty())
        && values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

fn resource_key(
    model_system: GliomaModelSystem,
    modality: GliomaModality,
) -> (GliomaModelSystem, GliomaModality) {
    (model_system, modality)
}

pub(crate) fn digest_input(output: &MechanismProspectiveControllerPlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "portfolio_digest": output.portfolio_digest,
        "epoch": output.epoch,
        "next_epoch": output.next_epoch,
        "task_order": output.task_order,
        "decisions": output.decisions,
        "queue_order": output.queue_order,
        "inflight_order": output.inflight_order,
        "requeue_order": output.requeue_order,
        "retired_order": output.retired_order,
        "blocked_order": output.blocked_order,
        "hold_order": output.hold_order,
        "resource_usage": output.resource_usage,
        "total_cost_units": output.total_cost_units,
        "budget_units": output.budget_units,
        "budget_remaining_units": output.budget_remaining_units,
        "drift_alerts": output.drift_alerts,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
        "next_route": output.next_route,
    })
}

fn validate_request(
    request: &MechanismProspectiveControllerRequest,
) -> Result<(), MechanismProspectiveControllerError> {
    if request.objective.trim().is_empty()
        || request.objective != request.portfolio.objective
        || request.portfolio.task_order.is_empty()
        || request.portfolio.task_order.len() > MAX_TASKS
        || request.current_epoch == 0
        || request.budget_units == 0
        || request.max_inflight == 0
        || request.max_inflight > MAX_TASKS
        || request.max_queue_depth == 0
        || request.max_queue_depth > MAX_TASKS
        || request.minimum_quality_milli > 1_000
        || request.maximum_drift_milli > 1_000
        || request.observations.len() > MAX_OBSERVATIONS
        || request.resources.len() > MAX_RESOURCES
    {
        return Err(MechanismProspectiveControllerError::InvalidRequest(
            "objective/portfolio binding, positive epoch/budget, bounded queue/inflight/observations/resources, and quality/drift bounds are required".into(),
        ));
    }
    request
        .portfolio
        .validate()
        .map_err(|error| MechanismProspectiveControllerError::InvalidInput(error.to_string()))?;
    let task_ids = request
        .portfolio
        .task_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if request.policies.len() != task_ids.len()
        || request
            .policies
            .iter()
            .map(|policy| policy.task_id.clone())
            .collect::<BTreeSet<_>>()
            != task_ids
    {
        return Err(MechanismProspectiveControllerError::InvalidInput(
            "task policies must exactly cover the portfolio task order".into(),
        ));
    }
    for policy in &request.policies {
        if policy.task_id.trim().is_empty()
            || policy.max_attempts == 0
            || policy.max_attempts > MAX_ATTEMPTS
            || policy.attempts_used > policy.max_attempts
            || policy.stale_after_epochs == 0
        {
            return Err(MechanismProspectiveControllerError::InvalidInput(
                "task policy identity, attempt, and staleness bounds are invalid".into(),
            ));
        }
    }
    let mut observed_tasks = BTreeSet::new();
    for observation in &request.observations {
        if !task_ids.contains(&observation.task_id)
            || observation.epoch == 0
            || observation.epoch > request.current_epoch
            || observation.quality_milli > 1_000
            || observation.drift_milli > 1_000
            || !observed_tasks.insert(observation.task_id.clone())
            || observation.artifact.validate().is_err()
        {
            return Err(MechanismProspectiveControllerError::InvalidInput(
                "observations must be unique, epoch-bounded, quality/drift-bounded, and local de-identified artifacts".into(),
            ));
        }
    }
    let mut resources = BTreeSet::new();
    for resource in &request.resources {
        if resource.capacity_units == 0
            || resource.occupied_units > resource.capacity_units
            || !resources.insert(resource_key(resource.model_system, resource.modality))
        {
            return Err(MechanismProspectiveControllerError::InvalidInput(
                "resource capacities must be positive, non-overcommitted, and unique by model/modality".into(),
            ));
        }
    }
    Ok(())
}

fn clamp_priority(value: i32) -> u16 {
    value.clamp(0, 1_000) as u16
}

/// Admit one rolling high-throughput batch from a multi-study mechanism portfolio.
pub fn control_glioma_mechanism_prospective_batch(
    request: &MechanismProspectiveControllerRequest,
) -> Result<MechanismProspectiveControllerPlan, MechanismProspectiveControllerError> {
    validate_request(request)?;
    let task_map = request
        .portfolio
        .tasks
        .iter()
        .map(|task| (task.task_id.clone(), task))
        .collect::<BTreeMap<_, _>>();
    let policy_map = request
        .policies
        .iter()
        .map(|policy| (policy.task_id.clone(), policy))
        .collect::<BTreeMap<_, _>>();
    let observation_map = request
        .observations
        .iter()
        .map(|observation| (observation.task_id.clone(), observation))
        .collect::<BTreeMap<_, _>>();
    let resource_map = request
        .resources
        .iter()
        .map(|resource| {
            (
                resource_key(resource.model_system, resource.modality),
                resource,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut decisions = Vec::with_capacity(task_map.len());
    let mut candidates = Vec::<(String, u16, bool)>::new();
    let mut drift_alerts = Vec::new();
    let mut negative_evidence = request.portfolio.negative_evidence.clone();
    let mut uncertainty = request.portfolio.uncertainty.clone();
    for task_id in &request.portfolio.task_order {
        let task = task_map.get(task_id).expect("validated portfolio task");
        let policy = policy_map.get(task_id).expect("validated task policy");
        let observation = observation_map.get(task_id).copied();
        let mut priority =
            i32::from(task.priority_milli).saturating_add(i32::from(policy.priority_boost_milli));
        let mut status = MechanismProspectiveTaskStatus::Queued;
        let mut reason = "awaiting first observation".to_string();
        let mut observed_epoch = None;
        let mut retry_candidate = false;
        if request
            .portfolio
            .blocked_task_order
            .iter()
            .any(|id| id == task_id)
            || task.status == MechanismMultiStudyTaskStatus::Blocked
        {
            status = MechanismProspectiveTaskStatus::Blocked;
            reason = "portfolio task is blocked before prospective admission".into();
        } else if let Some(observation) = observation {
            observed_epoch = Some(observation.epoch);
            let age = request.current_epoch.saturating_sub(observation.epoch);
            if observation.drift_milli > request.maximum_drift_milli {
                drift_alerts.push(format!(
                    "{}:epoch-{}:drift-{}",
                    task_id, observation.epoch, observation.drift_milli
                ));
                priority = priority.saturating_add(140);
                retry_candidate = true;
            }
            if age > policy.stale_after_epochs {
                retry_candidate = true;
                priority = priority.saturating_add(80);
                uncertainty.push(format!("{task_id}:stale-observation-age={age}"));
            }
            match observation.outcome {
                MechanismProspectiveOutcome::Completed
                    if observation.quality_milli >= request.minimum_quality_milli
                        && observation.drift_milli <= request.maximum_drift_milli
                        && age <= policy.stale_after_epochs =>
                {
                    status = MechanismProspectiveTaskStatus::Retired;
                    reason = "quality-qualified completion retained".into();
                    priority = priority.saturating_sub(1_000);
                }
                MechanismProspectiveOutcome::Completed => {
                    retry_candidate = true;
                    priority = priority.saturating_add(80);
                    uncertainty.push(format!(
                        "{task_id}:completion-below-quality-or-freshness-gate"
                    ));
                    reason =
                        "completion returned but quality, drift, or freshness gate failed".into();
                }
                MechanismProspectiveOutcome::Contradicted => {
                    retry_candidate = true;
                    priority = priority.saturating_add(180);
                    negative_evidence.push(format!("{task_id}:contradicted"));
                    reason = "contradiction retained and routed for discrimination".into();
                }
                MechanismProspectiveOutcome::Unresolved => {
                    retry_candidate = true;
                    priority = priority.saturating_add(120);
                    uncertainty.push(format!("{task_id}:unresolved"));
                    reason = "unresolved outcome retained for bounded requeue".into();
                }
                MechanismProspectiveOutcome::Failed => {
                    retry_candidate = true;
                    priority = priority.saturating_add(100);
                    uncertainty.push(format!("{task_id}:failed-attempt"));
                    reason = "failed local attempt requires bounded retry or review".into();
                }
                MechanismProspectiveOutcome::Missing => {
                    retry_candidate = true;
                    priority = priority.saturating_add(60);
                    uncertainty.push(format!("{task_id}:missing-outcome"));
                    reason = "missing outcome remains an explicit omission".into();
                }
            }
        }
        let attempts_exhausted = policy.attempts_used >= policy.max_attempts;
        if status != MechanismProspectiveTaskStatus::Retired
            && status != MechanismProspectiveTaskStatus::Blocked
        {
            if retry_candidate && (!policy.allow_requeue || attempts_exhausted) {
                status = if attempts_exhausted {
                    MechanismProspectiveTaskStatus::Hold
                } else {
                    MechanismProspectiveTaskStatus::Blocked
                };
                reason = if attempts_exhausted {
                    "retry budget exhausted; independent review required".into()
                } else {
                    "policy forbids automatic requeue".into()
                };
            } else if retry_candidate {
                status = MechanismProspectiveTaskStatus::Requeue;
            }
        }
        if policy.requires_approval && !request.allow_approval_required {
            status = MechanismProspectiveTaskStatus::Blocked;
            reason = "approval is required by task policy".into();
        }
        let priority = clamp_priority(priority);
        let next_attempt = policy.attempts_used.saturating_add(1);
        if matches!(
            status,
            MechanismProspectiveTaskStatus::Queued | MechanismProspectiveTaskStatus::Requeue
        ) {
            candidates.push((
                task_id.clone(),
                priority,
                status == MechanismProspectiveTaskStatus::Requeue,
            ));
        }
        decisions.push(MechanismProspectiveTaskDecision {
            task_id: task_id.clone(),
            status,
            priority_milli: priority,
            attempts_used: policy.attempts_used,
            next_attempt,
            observed_epoch,
            reason,
        });
    }
    drift_alerts.sort();
    drift_alerts.dedup();
    candidates.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    let mut resource_reserved = BTreeMap::<(GliomaModelSystem, GliomaModality), u16>::new();
    let mut inflight = Vec::new();
    let mut queue = Vec::new();
    let mut requeue = Vec::new();
    let mut retired = Vec::new();
    let mut blocked = Vec::new();
    let mut hold = Vec::new();
    let mut spent = 0_u64;
    let mut selected_count = 0_usize;
    let mut decision_map = decisions
        .into_iter()
        .map(|decision| (decision.task_id.clone(), decision))
        .collect::<BTreeMap<_, _>>();
    for decision in decision_map.values() {
        match decision.status {
            MechanismProspectiveTaskStatus::Retired => retired.push(decision.task_id.clone()),
            MechanismProspectiveTaskStatus::Blocked => blocked.push(decision.task_id.clone()),
            MechanismProspectiveTaskStatus::Hold => hold.push(decision.task_id.clone()),
            _ => {}
        }
    }
    for (task_id, priority, is_requeue) in candidates {
        if selected_count >= request.max_inflight {
            let decision = decision_map.get_mut(&task_id).expect("decision exists");
            decision.status = if is_requeue {
                MechanismProspectiveTaskStatus::Requeue
            } else {
                MechanismProspectiveTaskStatus::Queued
            };
            decision.reason = "concurrency capacity is full".into();
            if is_requeue {
                requeue.push(task_id);
            } else {
                queue.push(task_id);
            }
            continue;
        }
        let task = task_map.get(&task_id).expect("portfolio task");
        let key = resource_key(task.model_system, task.modality);
        let Some(resource) = resource_map.get(&key) else {
            let decision = decision_map.get_mut(&task_id).expect("decision exists");
            decision.status = MechanismProspectiveTaskStatus::Hold;
            decision.reason = "resource capacity is undeclared".into();
            hold.push(task_id);
            continue;
        };
        let reserved = resource_reserved.get(&key).copied().unwrap_or(0);
        if reserved.saturating_add(1)
            > resource
                .capacity_units
                .saturating_sub(resource.occupied_units)
        {
            let decision = decision_map.get_mut(&task_id).expect("decision exists");
            decision.status = if is_requeue {
                MechanismProspectiveTaskStatus::Requeue
            } else {
                MechanismProspectiveTaskStatus::Queued
            };
            decision.reason = "resource capacity is full".into();
            if is_requeue {
                requeue.push(task_id);
            } else {
                queue.push(task_id);
            }
            continue;
        }
        if spent.saturating_add(task.cost_units) > request.budget_units {
            let decision = decision_map.get_mut(&task_id).expect("decision exists");
            decision.status = if is_requeue {
                MechanismProspectiveTaskStatus::Requeue
            } else {
                MechanismProspectiveTaskStatus::Queued
            };
            decision.reason = "rolling budget is exhausted".into();
            if is_requeue {
                requeue.push(task_id);
            } else {
                queue.push(task_id);
            }
            continue;
        }
        selected_count += 1;
        spent = spent.saturating_add(task.cost_units);
        *resource_reserved.entry(key).or_default() += 1;
        inflight.push(task_id);
        let decision = decision_map
            .get_mut(inflight.last().expect("selected task"))
            .expect("decision exists");
        decision.status = MechanismProspectiveTaskStatus::Inflight;
        decision.priority_milli = priority;
        decision.reason = if is_requeue {
            "admitted as a bounded retry in the next local batch".into()
        } else {
            "admitted into the next local batch".into()
        };
    }
    let pending_count = queue.len() + requeue.len();
    if pending_count > request.max_queue_depth {
        let mut pressure = queue
            .iter()
            .chain(requeue.iter())
            .cloned()
            .collect::<Vec<_>>();
        pressure.sort_by(|left, right| {
            decision_map[right]
                .priority_milli
                .cmp(&decision_map[left].priority_milli)
                .then_with(|| left.cmp(right))
        });
        let retained = pressure
            .iter()
            .take(request.max_queue_depth)
            .cloned()
            .collect::<BTreeSet<_>>();
        let excess = pressure
            .into_iter()
            .filter(|task_id| !retained.contains(task_id))
            .collect::<Vec<_>>();
        for task_id in excess {
            let decision = decision_map.get_mut(&task_id).expect("decision exists");
            decision.status = MechanismProspectiveTaskStatus::Hold;
            decision.reason = "queue depth backpressure requires review".into();
            hold.push(task_id.clone());
            queue.retain(|id| id != &task_id);
            requeue.retain(|id| id != &task_id);
            uncertainty.push(format!("{task_id}:queue-depth-backpressure"));
        }
    }
    queue.sort();
    requeue.sort();
    retired.sort();
    blocked.sort();
    hold.sort();
    inflight.sort();
    let mut resource_usage = Vec::new();
    for resource in &request.resources {
        let key = resource_key(resource.model_system, resource.modality);
        let reserved = resource_reserved.get(&key).copied().unwrap_or(0);
        resource_usage.push(MechanismProspectiveResourceUsage {
            model_system: resource.model_system,
            modality: resource.modality,
            capacity_units: resource.capacity_units,
            occupied_units: resource.occupied_units,
            reserved_units: reserved,
            remaining_units: resource
                .capacity_units
                .saturating_sub(resource.occupied_units)
                .saturating_sub(reserved),
        });
    }
    resource_usage.sort_by_key(|usage| (usage.model_system, usage.modality));
    negative_evidence.sort();
    negative_evidence.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    let decisions = request
        .portfolio
        .task_order
        .iter()
        .map(|task_id| decision_map.remove(task_id).expect("decision coverage"))
        .collect::<Vec<_>>();
    let disposition = if !inflight.is_empty() {
        if queue.is_empty() && requeue.is_empty() && hold.is_empty() {
            MechanismProspectiveDisposition::Ready
        } else {
            MechanismProspectiveDisposition::Partial
        }
    } else if !drift_alerts.is_empty() {
        MechanismProspectiveDisposition::DriftHalt
    } else if retired.len() == request.portfolio.task_order.len() {
        MechanismProspectiveDisposition::Exhausted
    } else {
        MechanismProspectiveDisposition::QueueBlocked
    };
    let next_route = if !drift_alerts.is_empty() {
        "glioma_mechanism_feedback_replan"
    } else if !inflight.is_empty() {
        "glioma_mechanism_operating_cycle"
    } else {
        "glioma_mechanism_workflow_compile"
    };
    let mut output = MechanismProspectiveControllerPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        portfolio_digest: request.portfolio.digest.clone(),
        epoch: request.current_epoch,
        next_epoch: request.current_epoch.saturating_add(1),
        task_order: request.portfolio.task_order.clone(),
        decisions,
        queue_order: queue,
        inflight_order: inflight,
        requeue_order: requeue,
        retired_order: retired,
        blocked_order: blocked,
        hold_order: hold,
        resource_usage,
        total_cost_units: spent,
        budget_units: request.budget_units,
        budget_remaining_units: request.budget_units.saturating_sub(spent),
        drift_alerts,
        negative_evidence,
        uncertainty,
        disposition,
        next_route: next_route.into(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-prospective-controller"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| MechanismProspectiveControllerError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

impl MechanismProspectiveControllerPlan {
    pub fn validate(&self) -> Result<(), MechanismProspectiveControllerError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.portfolio_digest.as_str().len() != 64
            || self.epoch == 0
            || self.next_epoch <= self.epoch
            || !canonical(&self.task_order)
            || !unique_nonempty(&self.task_order)
            || self.decisions.len() != self.task_order.len()
            || self
                .decisions
                .iter()
                .map(|decision| decision.task_id.clone())
                .collect::<Vec<_>>()
                != self.task_order
            || !canonical(&self.queue_order)
            || !canonical(&self.requeue_order)
            || !canonical(&self.retired_order)
            || !canonical(&self.blocked_order)
            || !canonical(&self.hold_order)
            || !unique_nonempty(&self.queue_order)
            || !unique_nonempty(&self.requeue_order)
            || !unique_nonempty(&self.retired_order)
            || !unique_nonempty(&self.blocked_order)
            || !unique_nonempty(&self.hold_order)
            || !unique_nonempty(&self.inflight_order)
            || !canonical(&self.drift_alerts)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.next_route.trim().is_empty()
        {
            return Err(MechanismProspectiveControllerError::InvalidOutput(
                "identity, epoch, decision, status ordering, or digest shape is invalid".into(),
            ));
        }
        let task_ids = self.task_order.iter().cloned().collect::<BTreeSet<_>>();
        let inflight_ids = self.inflight_order.iter().cloned().collect::<BTreeSet<_>>();
        let queue_ids = self.queue_order.iter().cloned().collect::<BTreeSet<_>>();
        let requeue_ids = self.requeue_order.iter().cloned().collect::<BTreeSet<_>>();
        let retired_ids = self.retired_order.iter().cloned().collect::<BTreeSet<_>>();
        let blocked_ids = self.blocked_order.iter().cloned().collect::<BTreeSet<_>>();
        let hold_ids = self.hold_order.iter().cloned().collect::<BTreeSet<_>>();
        let partition = inflight_ids
            .union(&queue_ids)
            .cloned()
            .collect::<BTreeSet<_>>()
            .union(&requeue_ids)
            .cloned()
            .collect::<BTreeSet<_>>()
            .union(&retired_ids)
            .cloned()
            .collect::<BTreeSet<_>>()
            .union(&blocked_ids)
            .cloned()
            .collect::<BTreeSet<_>>()
            .union(&hold_ids)
            .cloned()
            .collect::<BTreeSet<_>>();
        let status_ids = |status: MechanismProspectiveTaskStatus| {
            self.decisions
                .iter()
                .filter(|decision| decision.status == status)
                .map(|decision| decision.task_id.clone())
                .collect::<BTreeSet<_>>()
        };
        if partition != task_ids
            || inflight_ids.len()
                + queue_ids.len()
                + requeue_ids.len()
                + retired_ids.len()
                + blocked_ids.len()
                + hold_ids.len()
                != partition.len()
            || status_ids(MechanismProspectiveTaskStatus::Inflight) != inflight_ids
            || status_ids(MechanismProspectiveTaskStatus::Queued) != queue_ids
            || status_ids(MechanismProspectiveTaskStatus::Requeue) != requeue_ids
            || status_ids(MechanismProspectiveTaskStatus::Retired) != retired_ids
            || status_ids(MechanismProspectiveTaskStatus::Blocked) != blocked_ids
            || status_ids(MechanismProspectiveTaskStatus::Hold) != hold_ids
            || self.decisions.iter().any(|decision| {
                decision.task_id.trim().is_empty()
                    || decision.priority_milli > 1_000
                    || decision.attempts_used > decision.next_attempt
                    || decision.next_attempt == 0
                    || decision.reason.trim().is_empty()
            })
        {
            return Err(MechanismProspectiveControllerError::InvalidOutput(
                "status partition, decision bounds, or task coverage does not reconcile".into(),
            ));
        }
        let reserved = self
            .resource_usage
            .iter()
            .map(|usage| {
                (
                    resource_key(usage.model_system, usage.modality),
                    usage.reserved_units,
                )
            })
            .collect::<BTreeMap<_, _>>();
        if self.resource_usage.iter().any(|usage| {
            usage.capacity_units == 0
                || usage.occupied_units > usage.capacity_units
                || usage.reserved_units > usage.capacity_units.saturating_sub(usage.occupied_units)
                || usage.remaining_units
                    != usage
                        .capacity_units
                        .saturating_sub(usage.occupied_units)
                        .saturating_sub(usage.reserved_units)
        }) || reserved.len() != self.resource_usage.len()
        {
            return Err(MechanismProspectiveControllerError::InvalidOutput(
                "resource usage is overcommitted or duplicated".into(),
            ));
        }
        if self.total_cost_units > self.budget_units
            || self
                .total_cost_units
                .saturating_add(self.budget_remaining_units)
                != self.budget_units
        {
            return Err(MechanismProspectiveControllerError::InvalidOutput(
                "rolling budget accounting does not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| MechanismProspectiveControllerError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(MechanismProspectiveControllerError::Digest(
                "prospective controller digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::super::multi_study_workflow::{
        MechanismMultiStudyActionGroup, MechanismMultiStudyWorkflowDisposition,
    };
    use super::*;

    fn hash(seed: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"seed": seed})).unwrap()
    }

    fn portfolio() -> MechanismMultiStudyWorkflowPlan {
        let task_id = "lane-a::measure".to_string();
        let task = super::super::multi_study_workflow::MechanismMultiStudyTask {
            task_id: task_id.clone(),
            lane_id: "lane-a".into(),
            study_id: "study-a".into(),
            site_id: "site-a".into(),
            action_id: "measure".into(),
            mechanism_id: "m1".into(),
            model_system: GliomaModelSystem::Organoid,
            modality: GliomaModality::Imaging,
            dependency_order: vec![],
            cost_units: 1,
            priority_milli: 800,
            transport_score_milli: 900,
            status: MechanismMultiStudyTaskStatus::Scheduled,
            reason: None,
        };
        let mut output = MechanismMultiStudyWorkflowPlan {
            feature_id: super::super::multi_study_workflow::FEATURE_ID.into(),
            output_schema: super::super::multi_study_workflow::OUTPUT_SCHEMA.into(),
            objective: "prospective mechanism control".into(),
            lane_order: vec!["lane-a".into()],
            group_order: vec!["measure".into()],
            task_order: vec![task_id.clone()],
            topological_task_order: vec![task_id.clone()],
            execution_waves: vec![vec![task_id.clone()]],
            tasks: vec![task],
            groups: vec![MechanismMultiStudyActionGroup {
                action_id: "measure".into(),
                mechanism_id: "m1".into(),
                task_order: vec![task_id.clone()],
                study_order: vec!["study-a".into()],
                site_order: vec!["site-a".into()],
                modality_order: vec![GliomaModality::Imaging],
                supporting_study_count: 1,
                independent_site_count: 1,
                transport_score_milli: 900,
                priority_milli: 800,
                required_modality_complete: true,
                replication_complete: true,
                selected: true,
                block_reason: None,
            }],
            scheduled_task_order: vec![task_id],
            deferred_task_order: vec![],
            blocked_task_order: vec![],
            federation_mode: super::super::multi_study_workflow::MechanismFederationMode::LocalOnly,
            total_cost_units: 1,
            budget_units: 1,
            budget_remaining_units: 0,
            critical_path_units: 1,
            negative_evidence: vec![],
            uncertainty: vec![],
            disposition: MechanismMultiStudyWorkflowDisposition::Ready,
            next_route: "glioma_mechanism_operating_cycle".into(),
            digest: hash("unsealed"),
        };
        output.digest =
            ContentHash::of_value(&super::super::multi_study_workflow::digest_input(&output))
                .unwrap();
        output.validate().unwrap();
        output
    }

    fn request() -> MechanismProspectiveControllerRequest {
        MechanismProspectiveControllerRequest {
            objective: "prospective mechanism control".into(),
            portfolio: portfolio(),
            policies: vec![MechanismProspectiveTaskPolicy {
                task_id: "lane-a::measure".into(),
                attempts_used: 0,
                max_attempts: 3,
                priority_boost_milli: 0,
                stale_after_epochs: 2,
                allow_requeue: true,
                requires_approval: false,
            }],
            observations: vec![],
            resources: vec![MechanismProspectiveResourceCapacity {
                model_system: GliomaModelSystem::Organoid,
                modality: GliomaModality::Imaging,
                capacity_units: 1,
                occupied_units: 0,
            }],
            current_epoch: 1,
            budget_units: 1,
            max_inflight: 1,
            max_queue_depth: 4,
            minimum_quality_milli: 700,
            maximum_drift_milli: 200,
            allow_approval_required: false,
        }
    }

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: id.into(),
            content_hash: ContentHash::of_bytes(id.as_bytes()),
            content_type: "application/vnd.aurora.glioma-observation+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    #[test]
    fn admits_a_fresh_batch_and_accounts_for_resource_capacity() {
        let plan = control_glioma_mechanism_prospective_batch(&request()).unwrap();
        assert_eq!(plan.inflight_order, vec!["lane-a::measure"]);
        assert_eq!(plan.total_cost_units, 1);
        assert_eq!(plan.resource_usage[0].reserved_units, 1);
        assert_eq!(plan.disposition, MechanismProspectiveDisposition::Ready);
        plan.validate().unwrap();
    }

    #[test]
    fn contradiction_and_drift_are_retained_and_requeued() {
        let mut request = request();
        request.observations = vec![MechanismProspectiveObservation {
            task_id: "lane-a::measure".into(),
            epoch: 1,
            outcome: MechanismProspectiveOutcome::Contradicted,
            quality_milli: 500,
            drift_milli: 800,
            artifact: artifact("observation-1"),
        }];
        let plan = control_glioma_mechanism_prospective_batch(&request).unwrap();
        assert_eq!(plan.inflight_order, vec!["lane-a::measure"]);
        assert!(plan
            .negative_evidence
            .iter()
            .any(|item| item.contains("contradicted")));
        assert!(!plan.drift_alerts.is_empty());
        assert_eq!(plan.next_route, "glioma_mechanism_feedback_replan");
        plan.validate().unwrap();
    }
}
