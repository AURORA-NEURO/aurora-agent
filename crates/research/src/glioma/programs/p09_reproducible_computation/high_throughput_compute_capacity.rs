//! High-throughput capacity planning for autonomous preclinical glioma computation.
//!
//! This is a scheduler-facing product surface, not a runtime worker. It turns admitted metadata
//! into a deterministic, fair capacity plan under CPU/memory/accelerator and budget limits. Runtime
//! observations produce explicit duration intervals; stale telemetry, exhausted budgets, and
//! unschedulable required jobs stop admission with visible negative evidence.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P09-F31";
pub const OUTPUT_SCHEMA: &str = "GliomaComputeCapacityPlan1@1";
pub const MAX_JOBS: usize = 4_096;
pub const MAX_OBSERVATIONS: usize = 16_384;
pub const MAX_TEXT_LEN: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapacityJob {
    pub job_id: String,
    pub workflow_class: String,
    pub fairness_group: String,
    pub priority_milli: u16,
    pub resource_units: u64,
    pub memory_mb: u32,
    pub accelerator_count: u16,
    pub estimated_duration_ticks: u64,
    pub budget_units: u64,
    pub required: bool,
    pub submitted_tick: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapacityRuntimeObservation {
    pub job_id: String,
    pub workflow_class: String,
    pub fairness_group: String,
    pub observed_duration_ticks: u64,
    pub success: bool,
    pub retry_count: u16,
    pub completed_tick: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapacityTelemetry {
    pub available_concurrency: usize,
    pub resource_capacity_units: u64,
    pub memory_capacity_mb: u32,
    pub accelerator_capacity_count: u16,
    pub sampled_tick: u64,
    pub max_age_ticks: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapacityPolicy {
    pub max_concurrency: usize,
    pub max_budget_units: u64,
    pub max_job_age_ticks: u64,
    pub max_duration_ticks: u64,
    pub fairness_weight_milli: u16,
    pub require_fresh_telemetry: bool,
    pub policy_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputeCapacityRequest {
    pub objective: String,
    pub jobs: Vec<CapacityJob>,
    pub observations: Vec<CapacityRuntimeObservation>,
    pub telemetry: CapacityTelemetry,
    pub policy: CapacityPolicy,
    pub current_tick: u64,
    pub horizon_ticks: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapacityAssignmentDisposition {
    Scheduled,
    Deferred,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapacityAssignment {
    pub job_id: String,
    pub fairness_group: String,
    pub disposition: CapacityAssignmentDisposition,
    pub predicted_duration_lower_ticks: u64,
    pub predicted_duration_upper_ticks: u64,
    pub reserved_resource_units: u64,
    pub reserved_memory_mb: u32,
    pub reserved_accelerator_count: u16,
    pub reserved_budget_units: u64,
    pub reason_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapacityThroughputForecast {
    pub horizon_ticks: u64,
    pub scheduled_job_count: u32,
    pub predicted_successful_jobs: u32,
    pub lower_successful_jobs: u32,
    pub upper_successful_jobs: u32,
    pub observed_success_rate_milli: Option<u16>,
    pub calibrated: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ComputeCapacityPlanDisposition {
    Ready,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComputeCapacityPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub current_tick: u64,
    pub horizon_ticks: u64,
    pub assignment_order: Vec<String>,
    pub assignments: Vec<CapacityAssignment>,
    pub scheduled_order: Vec<String>,
    pub deferred_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub reserved_concurrency: usize,
    pub reserved_resource_units: u64,
    pub reserved_memory_mb: u32,
    pub reserved_accelerator_count: u16,
    pub reserved_budget_units: u64,
    pub resource_utilization_milli: u16,
    pub memory_utilization_milli: u16,
    pub fairness_gap_milli: u16,
    pub forecast: CapacityThroughputForecast,
    pub saturation_alert_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: ComputeCapacityPlanDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ComputeCapacityError {
    #[error("compute-capacity request is invalid: {0}")]
    InvalidRequest(String),
    #[error("compute-capacity plan is invalid: {0}")]
    InvalidOutput(String),
    #[error("compute-capacity digest failed: {0}")]
    Digest(String),
}

fn valid_hash(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}

fn safe_text(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= MAX_TEXT_LEN
        && !value.chars().any(|character| character.is_control())
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_bounded(values: &[String], max: usize) -> bool {
    let mut seen = BTreeSet::new();
    values.len() <= max
        && values
            .iter()
            .all(|value| safe_text(value) && seen.insert(value))
}

fn digest_input(plan: &ComputeCapacityPlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": plan.feature_id,
        "output_schema": plan.output_schema,
        "objective": plan.objective,
        "current_tick": plan.current_tick,
        "horizon_ticks": plan.horizon_ticks,
        "assignment_order": plan.assignment_order,
        "assignments": plan.assignments,
        "scheduled_order": plan.scheduled_order,
        "deferred_order": plan.deferred_order,
        "blocked_order": plan.blocked_order,
        "reserved_concurrency": plan.reserved_concurrency,
        "reserved_resource_units": plan.reserved_resource_units,
        "reserved_memory_mb": plan.reserved_memory_mb,
        "reserved_accelerator_count": plan.reserved_accelerator_count,
        "reserved_budget_units": plan.reserved_budget_units,
        "resource_utilization_milli": plan.resource_utilization_milli,
        "memory_utilization_milli": plan.memory_utilization_milli,
        "fairness_gap_milli": plan.fairness_gap_milli,
        "forecast": plan.forecast,
        "saturation_alert_order": plan.saturation_alert_order,
        "negative_evidence": plan.negative_evidence,
        "uncertainty": plan.uncertainty,
        "disposition": plan.disposition,
    })
}

fn validate_request(request: &ComputeCapacityRequest) -> Result<(), ComputeCapacityError> {
    if !safe_text(&request.objective)
        || request.jobs.is_empty()
        || request.jobs.len() > MAX_JOBS
        || request.observations.len() > MAX_OBSERVATIONS
        || request.current_tick == 0
        || request.horizon_ticks == 0
        || request.telemetry.available_concurrency == 0
        || request.telemetry.resource_capacity_units == 0
        || request.telemetry.memory_capacity_mb == 0
        || request.telemetry.max_age_ticks == 0
        || request.telemetry.sampled_tick == 0
        || request.policy.max_concurrency == 0
        || request.policy.max_budget_units == 0
        || request.policy.max_job_age_ticks == 0
        || request.policy.max_duration_ticks == 0
        || request.policy.fairness_weight_milli == 0
        || !valid_hash(&request.policy.policy_digest)
    {
        return Err(ComputeCapacityError::InvalidRequest(
            "bounded objective, jobs, fresh-capacity dimensions, horizon, positive policy limits, and policy identity are required".into(),
        ));
    }
    let mut job_ids = BTreeSet::new();
    for job in &request.jobs {
        if !safe_text(&job.job_id)
            || !safe_text(&job.workflow_class)
            || !safe_text(&job.fairness_group)
            || job.priority_milli > 1_000
            || job.resource_units == 0
            || job.memory_mb == 0
            || job.estimated_duration_ticks == 0
            || job.estimated_duration_ticks > request.policy.max_duration_ticks
            || job.budget_units == 0
            || job.submitted_tick == 0
            || !job_ids.insert(job.job_id.clone())
        {
            return Err(ComputeCapacityError::InvalidRequest(
                "jobs require unique bounded IDs, fairness/workflow labels, positive resource/time/budget demand, and policy-compatible estimates".into(),
            ));
        }
    }
    let mut observation_ids = BTreeSet::new();
    for observation in &request.observations {
        if !safe_text(&observation.job_id)
            || !safe_text(&observation.workflow_class)
            || !safe_text(&observation.fairness_group)
            || observation.observed_duration_ticks == 0
            || observation.observed_duration_ticks > request.policy.max_duration_ticks
            || observation.completed_tick == 0
            || !observation_ids.insert(format!(
                "{}:{}",
                observation.job_id, observation.completed_tick
            ))
        {
            return Err(ComputeCapacityError::InvalidRequest(
                "runtime observations require bounded labels, positive durations/ticks, and unique job/tick identities".into(),
            ));
        }
    }
    Ok(())
}

fn prediction(
    job: &CapacityJob,
    observations: &[CapacityRuntimeObservation],
    max_duration: u64,
) -> (u64, u64, Option<u16>, bool) {
    let mut durations = observations
        .iter()
        .filter(|observation| observation.workflow_class == job.workflow_class)
        .map(|observation| observation.observed_duration_ticks)
        .collect::<Vec<_>>();
    let mut success_count = 0_u64;
    let mut observed_count = 0_u64;
    for observation in observations
        .iter()
        .filter(|observation| observation.workflow_class == job.workflow_class)
    {
        observed_count += 1;
        if observation.success {
            success_count += 1;
        }
    }
    durations.sort_unstable();
    if durations.is_empty() {
        let upper = job
            .estimated_duration_ticks
            .saturating_mul(2)
            .min(max_duration);
        (
            job.estimated_duration_ticks,
            upper.max(job.estimated_duration_ticks),
            None,
            false,
        )
    } else {
        let low = durations[durations.len() / 4].max(1);
        let high = durations[(durations.len().saturating_sub(1) * 3) / 4]
            .max(low)
            .min(max_duration);
        let success_rate = ((success_count.saturating_mul(1_000)) / observed_count.max(1)) as u16;
        (low, high, Some(success_rate), true)
    }
}

fn fairness_order(
    jobs: &[CapacityJob],
    observations: &[CapacityRuntimeObservation],
    fairness_weight_milli: u16,
) -> Vec<CapacityJob> {
    let mut groups = BTreeMap::<String, Vec<CapacityJob>>::new();
    for job in jobs {
        groups
            .entry(job.fairness_group.clone())
            .or_default()
            .push(job.clone());
    }
    for group in groups.values_mut() {
        group.sort_by(|left, right| {
            right
                .priority_milli
                .cmp(&left.priority_milli)
                .then(left.submitted_tick.cmp(&right.submitted_tick))
                .then(left.job_id.cmp(&right.job_id))
        });
    }
    let mut served =
        observations
            .iter()
            .fold(BTreeMap::<String, u64>::new(), |mut counts, observation| {
                if observation.success {
                    *counts
                        .entry(observation.fairness_group.clone())
                        .or_default() += 1;
                }
                counts
            });
    let mut order = Vec::with_capacity(jobs.len());
    while groups.values().any(|group| !group.is_empty()) {
        let selected_group = groups
            .iter()
            .filter(|(_, group)| !group.is_empty())
            .min_by(|(left_name, _), (right_name, _)| {
                let left_score = served
                    .get(*left_name)
                    .copied()
                    .unwrap_or(0)
                    .saturating_mul(1_000)
                    / u64::from(fairness_weight_milli.max(1));
                let right_score = served
                    .get(*right_name)
                    .copied()
                    .unwrap_or(0)
                    .saturating_mul(1_000)
                    / u64::from(fairness_weight_milli.max(1));
                left_score.cmp(&right_score).then(left_name.cmp(right_name))
            })
            .map(|(name, _)| name.clone());
        let Some(group_name) = selected_group else {
            break;
        };
        let job = groups
            .get_mut(&group_name)
            .expect("selected group exists")
            .remove(0);
        *served.entry(group_name).or_default() += 1;
        order.push(job);
    }
    order
}

impl ComputeCapacityPlan {
    pub fn validate(&self) -> Result<(), ComputeCapacityError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !safe_text(&self.objective)
            || self.current_tick == 0
            || self.horizon_ticks == 0
            || self.assignment_order.len() != self.assignments.len()
            || self.assignment_order
                != self
                    .assignments
                    .iter()
                    .map(|assignment| assignment.job_id.clone())
                    .collect::<Vec<_>>()
            || !unique_bounded(&self.scheduled_order, MAX_JOBS)
            || !canonical(&self.scheduled_order)
            || !unique_bounded(&self.deferred_order, MAX_JOBS)
            || !canonical(&self.deferred_order)
            || !unique_bounded(&self.blocked_order, MAX_JOBS)
            || !canonical(&self.blocked_order)
            || !unique_bounded(&self.saturation_alert_order, MAX_JOBS)
            || !canonical(&self.saturation_alert_order)
            || !unique_bounded(&self.negative_evidence, MAX_JOBS)
            || !canonical(&self.negative_evidence)
            || !unique_bounded(&self.uncertainty, MAX_JOBS)
            || !canonical(&self.uncertainty)
            || self.resource_utilization_milli > 1_000
            || self.memory_utilization_milli > 1_000
            || self.fairness_gap_milli > 1_000
            || !valid_hash(&self.digest)
        {
            return Err(ComputeCapacityError::InvalidOutput(
                "capacity plan identity, assignment binding, ordered outcomes, utilization bounds, or digest is invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ComputeCapacityError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ComputeCapacityError::InvalidOutput(
                "capacity plan digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Build a deterministic fair capacity plan from local queue metadata and runtime observations.
pub fn plan_glioma_compute_capacity(
    request: &ComputeCapacityRequest,
) -> Result<ComputeCapacityPlan, ComputeCapacityError> {
    validate_request(request)?;
    let telemetry_age = request
        .current_tick
        .saturating_sub(request.telemetry.sampled_tick);
    let telemetry_fresh = telemetry_age <= request.telemetry.max_age_ticks;
    let mut negative_evidence = Vec::new();
    let mut uncertainty = Vec::new();
    if !telemetry_fresh {
        negative_evidence.push("capacity-telemetry-is-stale".into());
    }
    let available_slots = request
        .telemetry
        .available_concurrency
        .min(request.policy.max_concurrency);
    let ordered_jobs = fairness_order(
        &request.jobs,
        &request.observations,
        request.policy.fairness_weight_milli,
    );
    let job_by_id = request
        .jobs
        .iter()
        .map(|job| (job.job_id.clone(), job))
        .collect::<BTreeMap<_, _>>();
    let mut assignments = Vec::new();
    let mut scheduled = Vec::new();
    let mut deferred = Vec::new();
    let mut blocked = Vec::new();
    let mut reserved_concurrency = 0_usize;
    let mut reserved_resource_units = 0_u64;
    let mut reserved_memory_mb = 0_u32;
    let mut reserved_accelerators = 0_u16;
    let mut reserved_budget_units = 0_u64;
    let mut group_schedule_count = BTreeMap::<String, u64>::new();
    for job in ordered_jobs {
        let (lower, upper, _, _) = prediction(
            &job,
            &request.observations,
            request.policy.max_duration_ticks,
        );
        let mut reasons = Vec::new();
        let age = request.current_tick.saturating_sub(job.submitted_tick);
        if age > request.policy.max_job_age_ticks {
            reasons.push("job-exceeded-admission-age".into());
        }
        if request.policy.require_fresh_telemetry && !telemetry_fresh {
            reasons.push("fresh-capacity-telemetry-required".into());
        }
        if reserved_concurrency + 1 > available_slots {
            reasons.push("concurrency-capacity-exhausted".into());
        }
        if reserved_resource_units.saturating_add(job.resource_units)
            > request.telemetry.resource_capacity_units
        {
            reasons.push("resource-capacity-exhausted".into());
        }
        if reserved_memory_mb.saturating_add(job.memory_mb) > request.telemetry.memory_capacity_mb {
            reasons.push("memory-capacity-exhausted".into());
        }
        if reserved_accelerators.saturating_add(job.accelerator_count)
            > request.telemetry.accelerator_capacity_count
        {
            reasons.push("accelerator-capacity-exhausted".into());
        }
        if reserved_budget_units.saturating_add(job.budget_units) > request.policy.max_budget_units
        {
            reasons.push("budget-exhausted".into());
        }
        let disposition = if reasons.is_empty() {
            reserved_concurrency += 1;
            reserved_resource_units = reserved_resource_units.saturating_add(job.resource_units);
            reserved_memory_mb = reserved_memory_mb.saturating_add(job.memory_mb);
            reserved_accelerators = reserved_accelerators.saturating_add(job.accelerator_count);
            reserved_budget_units = reserved_budget_units.saturating_add(job.budget_units);
            scheduled.push(job.job_id.clone());
            *group_schedule_count
                .entry(job.fairness_group.clone())
                .or_default() += 1;
            CapacityAssignmentDisposition::Scheduled
        } else if job.required {
            blocked.push(job.job_id.clone());
            negative_evidence.push(format!("required-job-blocked:{}", job.job_id));
            CapacityAssignmentDisposition::Blocked
        } else {
            deferred.push(job.job_id.clone());
            CapacityAssignmentDisposition::Deferred
        };
        reasons.sort();
        assignments.push(CapacityAssignment {
            job_id: job.job_id.clone(),
            fairness_group: job.fairness_group.clone(),
            disposition,
            predicted_duration_lower_ticks: lower,
            predicted_duration_upper_ticks: upper,
            reserved_resource_units: if reasons.is_empty() {
                job.resource_units
            } else {
                0
            },
            reserved_memory_mb: if reasons.is_empty() { job.memory_mb } else { 0 },
            reserved_accelerator_count: if reasons.is_empty() {
                job.accelerator_count
            } else {
                0
            },
            reserved_budget_units: if reasons.is_empty() {
                job.budget_units
            } else {
                0
            },
            reason_order: reasons,
        });
    }
    if !telemetry_fresh {
        uncertainty.push("capacity-plan-confidence-is-limited-by-telemetry-age".into());
    }
    if request.observations.is_empty() {
        uncertainty.push("no-held-out-runtime-observations-for-duration-calibration".into());
    }
    let mut group_counts = group_schedule_count.values().copied().collect::<Vec<_>>();
    group_counts.sort_unstable();
    let fairness_gap = if group_counts.is_empty() {
        0
    } else {
        ((group_counts.last().unwrap() - group_counts.first().unwrap()).min(1_000)) as u16
    };
    let resource_utilization = ((reserved_resource_units.saturating_mul(1_000)
        / request.telemetry.resource_capacity_units)
        .min(1_000)) as u16;
    let memory_utilization = ((u64::from(reserved_memory_mb).saturating_mul(1_000)
        / u64::from(request.telemetry.memory_capacity_mb))
    .min(1_000)) as u16;
    let mut saturation_alerts = Vec::new();
    if resource_utilization >= 900 {
        saturation_alerts.push("resource-capacity-near-saturation".into());
    }
    if memory_utilization >= 900 {
        saturation_alerts.push("memory-capacity-near-saturation".into());
    }
    if reserved_accelerators == request.telemetry.accelerator_capacity_count
        && reserved_accelerators > 0
    {
        saturation_alerts.push("accelerator-capacity-saturated".into());
    }
    let mut success_rates = Vec::new();
    for observation in &request.observations {
        success_rates.push(if observation.success { 1_000_u32 } else { 0 });
    }
    let observed_success_rate = if success_rates.is_empty() {
        None
    } else {
        Some((success_rates.iter().sum::<u32>() / success_rates.len() as u32) as u16)
    };
    let scheduled_count = scheduled.len() as u32;
    let predicted_successful = observed_success_rate
        .map(|rate| scheduled_count.saturating_mul(u32::from(rate)) / 1_000)
        .unwrap_or(0);
    let lower_successful = if observed_success_rate.is_some() {
        predicted_successful.saturating_sub(1)
    } else {
        0
    };
    let upper_successful = if observed_success_rate.is_some() {
        (predicted_successful + 1).min(scheduled_count)
    } else {
        scheduled_count
    };
    let forecast = CapacityThroughputForecast {
        horizon_ticks: request.horizon_ticks,
        scheduled_job_count: scheduled_count,
        predicted_successful_jobs: predicted_successful,
        lower_successful_jobs: lower_successful,
        upper_successful_jobs: upper_successful,
        observed_success_rate_milli: observed_success_rate,
        calibrated: observed_success_rate.is_some(),
    };
    let disposition = if !blocked.is_empty() {
        ComputeCapacityPlanDisposition::Blocked
    } else if !deferred.is_empty() || !telemetry_fresh {
        ComputeCapacityPlanDisposition::Partial
    } else {
        ComputeCapacityPlanDisposition::Ready
    };
    negative_evidence.sort();
    negative_evidence.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    saturation_alerts.sort();
    let mut plan = ComputeCapacityPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        current_tick: request.current_tick,
        horizon_ticks: request.horizon_ticks,
        assignment_order: assignments
            .iter()
            .map(|assignment| assignment.job_id.clone())
            .collect(),
        assignments,
        scheduled_order: {
            let mut values = scheduled;
            values.sort();
            values
        },
        deferred_order: {
            let mut values = deferred;
            values.sort();
            values
        },
        blocked_order: {
            let mut values = blocked;
            values.sort();
            values
        },
        reserved_concurrency,
        reserved_resource_units,
        reserved_memory_mb,
        reserved_accelerator_count: reserved_accelerators,
        reserved_budget_units,
        resource_utilization_milli: resource_utilization,
        memory_utilization_milli: memory_utilization,
        fairness_gap_milli: fairness_gap,
        forecast,
        saturation_alert_order: saturation_alerts,
        negative_evidence,
        uncertainty,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-compute-capacity-plan"),
    };
    plan.digest = ContentHash::of_value(&digest_input(&plan))
        .map_err(|error| ComputeCapacityError::Digest(error.to_string()))?;
    plan.validate()?;
    let _ = job_by_id;
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }

    fn job(id: &str, group: &str, required: bool) -> CapacityJob {
        CapacityJob {
            job_id: id.into(),
            workflow_class: "imaging".into(),
            fairness_group: group.into(),
            priority_milli: 500,
            resource_units: 2,
            memory_mb: 512,
            accelerator_count: 0,
            estimated_duration_ticks: 10,
            budget_units: 2,
            required,
            submitted_tick: 1,
        }
    }

    fn request(jobs: Vec<CapacityJob>) -> ComputeCapacityRequest {
        ComputeCapacityRequest {
            objective: "capacity-test".into(),
            jobs,
            observations: vec![CapacityRuntimeObservation {
                job_id: "history".into(),
                workflow_class: "imaging".into(),
                fairness_group: "group-a".into(),
                observed_duration_ticks: 8,
                success: true,
                retry_count: 0,
                completed_tick: 1,
            }],
            telemetry: CapacityTelemetry {
                available_concurrency: 2,
                resource_capacity_units: 10,
                memory_capacity_mb: 2_048,
                accelerator_capacity_count: 0,
                sampled_tick: 5,
                max_age_ticks: 10,
            },
            policy: CapacityPolicy {
                max_concurrency: 2,
                max_budget_units: 10,
                max_job_age_ticks: 100,
                max_duration_ticks: 100,
                fairness_weight_milli: 1_000,
                require_fresh_telemetry: true,
                policy_digest: hash("policy"),
            },
            current_tick: 5,
            horizon_ticks: 100,
        }
    }

    #[test]
    fn equal_groups_receive_fair_capacity_before_one_group_monopolizes_queue() {
        let plan = plan_glioma_compute_capacity(&request(vec![
            job("a", "group-a", false),
            job("b", "group-b", false),
            job("c", "group-a", false),
        ]))
        .expect("plan");
        let groups = plan
            .assignments
            .iter()
            .filter(|assignment| assignment.disposition == CapacityAssignmentDisposition::Scheduled)
            .map(|assignment| assignment.fairness_group.clone())
            .collect::<BTreeSet<_>>();
        assert_eq!(groups, BTreeSet::from(["group-a".into(), "group-b".into()]));
        assert_eq!(plan.deferred_order, vec!["c"]);
    }

    #[test]
    fn resource_exhaustion_defers_non_required_jobs() {
        let mut req = request(vec![job("a", "group-a", false), job("b", "group-b", false)]);
        req.telemetry.resource_capacity_units = 2;
        let plan = plan_glioma_compute_capacity(&req).expect("plan");
        assert_eq!(plan.disposition, ComputeCapacityPlanDisposition::Partial);
        assert_eq!(plan.scheduled_order.len(), 1);
        assert_eq!(plan.deferred_order.len(), 1);
    }

    #[test]
    fn required_job_block_is_explicit_and_terminal() {
        let mut req = request(vec![job("required", "group-a", true)]);
        req.telemetry.memory_capacity_mb = 128;
        let plan = plan_glioma_compute_capacity(&req).expect("plan");
        assert_eq!(plan.disposition, ComputeCapacityPlanDisposition::Blocked);
        assert_eq!(plan.blocked_order, vec!["required"]);
        assert!(plan
            .negative_evidence
            .iter()
            .any(|item| item.contains("required-job")));
    }

    #[test]
    fn stale_telemetry_prevents_full_ready_claim() {
        let mut req = request(vec![job("a", "group-a", false)]);
        req.telemetry.sampled_tick = 1;
        req.telemetry.max_age_ticks = 1;
        let plan = plan_glioma_compute_capacity(&req).expect("plan");
        assert_eq!(plan.disposition, ComputeCapacityPlanDisposition::Partial);
        assert!(plan
            .negative_evidence
            .iter()
            .any(|item| item.contains("stale")));
    }

    #[test]
    fn budget_exhaustion_stops_admission_transparently() {
        let mut req = request(vec![job("a", "group-a", false), job("b", "group-b", false)]);
        req.policy.max_budget_units = 2;
        let plan = plan_glioma_compute_capacity(&req).expect("plan");
        assert_eq!(plan.scheduled_order.len(), 1);
        assert_eq!(plan.deferred_order.len(), 1);
        assert!(plan.assignments.iter().any(|assignment| {
            assignment
                .reason_order
                .iter()
                .any(|reason| reason.contains("budget"))
        }));
    }
}
