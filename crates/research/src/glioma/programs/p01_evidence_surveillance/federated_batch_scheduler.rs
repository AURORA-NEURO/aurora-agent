//! Prospective high-throughput scheduling for federated glioma evidence cycles.
//!
//! P01-F30 compiles one bounded next-action portfolio. This capability coordinates many such
//! portfolios without turning throughput into authority: each cycle remains digest-bound, action
//! prefixes are dependency-safe, route quotas prevent one connector from starving the program,
//! and blocked/held cycles are rejected rather than silently scheduled. The output is a schedule
//! for institution-local executors, not a network client or a scientific conclusion.

use super::federated_operating_cycle::{
    FederatedCycleAction, FederatedCycleDisposition, FederatedEvidenceOperatingCycle,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P01-F31";
pub const OUTPUT_SCHEMA: &str = "GliomaFederatedBatchScheduler1@1";
pub const MAX_CYCLES: usize = 1_024;
pub const MAX_ACTIONS: usize = 8_192;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedBatchDecision {
    Selected,
    Deferred,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedBatchDisposition {
    Ready,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedBatchSchedulerRequest {
    pub objective: String,
    pub schedule_epoch: u32,
    pub max_cycles: usize,
    pub max_selected_actions: usize,
    pub max_actions_per_cycle: usize,
    pub max_actions_per_route: usize,
    pub cycles: Vec<FederatedEvidenceOperatingCycle>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedBatchActionDecision {
    pub candidate_id: String,
    pub cycle_key: String,
    pub cycle_epoch: u32,
    pub action_id: String,
    pub rank: usize,
    pub route: String,
    pub priority_milli: u16,
    pub decision: FederatedBatchDecision,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedBatchSchedule {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub schedule_epoch: u32,
    pub input_cycle_digest_order: Vec<ContentHash>,
    pub cycle_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub deferred_order: Vec<String>,
    pub rejected_order: Vec<String>,
    pub decisions: Vec<FederatedBatchActionDecision>,
    pub selected_by_cycle: BTreeMap<String, usize>,
    pub selected_by_route: BTreeMap<String, usize>,
    pub selected_cycle_order: Vec<String>,
    pub selected_route_order: Vec<String>,
    pub eligible_cycle_count: usize,
    pub selected_action_count: usize,
    pub capacity_utilization_milli: u16,
    pub fairness_milli: u16,
    pub disposition: FederatedBatchDisposition,
    pub next_routes: Vec<String>,
    pub uncertainty: Vec<String>,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedBatchSchedulerError {
    #[error("federated batch scheduler request is invalid: {0}")]
    InvalidRequest(String),
    #[error("federated batch scheduler input is invalid: {0}")]
    InvalidInput(String),
    #[error("federated batch scheduler output is invalid: {0}")]
    InvalidOutput(String),
    #[error("federated batch scheduler digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn cycle_key(cycle: &FederatedEvidenceOperatingCycle) -> String {
    format!("cycle-{}-{:08}", cycle.digest.as_str(), cycle.cycle_epoch)
}

fn candidate_id(cycle_key: &str, action: &FederatedCycleAction) -> String {
    format!("{cycle_key}::{}", action.action_id)
}

pub(crate) fn digest_input(output: &FederatedBatchSchedule) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "schedule_epoch": output.schedule_epoch,
        "input_cycle_digest_order": output.input_cycle_digest_order,
        "cycle_order": output.cycle_order,
        "candidate_order": output.candidate_order,
        "selected_order": output.selected_order,
        "deferred_order": output.deferred_order,
        "rejected_order": output.rejected_order,
        "decisions": output.decisions,
        "selected_by_cycle": output.selected_by_cycle,
        "selected_by_route": output.selected_by_route,
        "selected_cycle_order": output.selected_cycle_order,
        "selected_route_order": output.selected_route_order,
        "eligible_cycle_count": output.eligible_cycle_count,
        "selected_action_count": output.selected_action_count,
        "capacity_utilization_milli": output.capacity_utilization_milli,
        "fairness_milli": output.fairness_milli,
        "disposition": output.disposition,
        "next_routes": output.next_routes,
        "uncertainty": output.uncertainty,
    })
}

impl FederatedBatchSchedule {
    pub fn validate(&self) -> Result<(), FederatedBatchSchedulerError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.schedule_epoch == 0
            || !canonical(&self.input_cycle_digest_order)
            || !canonical(&self.cycle_order)
            || !canonical(&self.candidate_order)
            || !canonical(&self.selected_order)
            || !canonical(&self.deferred_order)
            || !canonical(&self.rejected_order)
            || !canonical(&self.selected_cycle_order)
            || !canonical(&self.selected_route_order)
            || !canonical(&self.next_routes)
            || !canonical(&self.uncertainty)
            || !canonical(
                &self
                    .decisions
                    .iter()
                    .map(|decision| decision.candidate_id.clone())
                    .collect::<Vec<_>>(),
            )
            || self.candidate_order.len() != self.decisions.len()
            || self.selected_action_count != self.selected_order.len()
            || self.capacity_utilization_milli > 1_000
            || self.fairness_milli > 1_000
            || self
                .input_cycle_digest_order
                .iter()
                .any(|digest| digest.as_str().len() != 64)
            || self.decisions.iter().any(|decision| {
                decision.candidate_id.trim().is_empty()
                    || decision.cycle_key.trim().is_empty()
                    || decision.cycle_epoch == 0
                    || decision.action_id.trim().is_empty()
                    || decision.rank == 0
                    || decision.route.trim().is_empty()
                    || decision.reason.trim().is_empty()
            })
        {
            return Err(FederatedBatchSchedulerError::InvalidOutput(
                "identity, ordering, capacity metrics, input hashes, or decision fields are invalid".into(),
            ));
        }
        let candidates = self
            .candidate_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let decision_ids = self
            .decisions
            .iter()
            .map(|decision| decision.candidate_id.clone())
            .collect::<BTreeSet<_>>();
        let selected = self.selected_order.iter().cloned().collect::<BTreeSet<_>>();
        let deferred = self.deferred_order.iter().cloned().collect::<BTreeSet<_>>();
        let rejected = self.rejected_order.iter().cloned().collect::<BTreeSet<_>>();
        let classified = selected
            .union(&deferred)
            .chain(rejected.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        if candidates.len() != self.candidate_order.len()
            || candidates != decision_ids
            || classified != candidates
            || selected.len() != self.selected_order.len()
            || deferred.len() != self.deferred_order.len()
            || rejected.len() != self.rejected_order.len()
            || selected.intersection(&deferred).next().is_some()
            || selected.intersection(&rejected).next().is_some()
            || deferred.intersection(&rejected).next().is_some()
            || self
                .decisions
                .iter()
                .any(|decision| match decision.decision {
                    FederatedBatchDecision::Selected => !selected.contains(&decision.candidate_id),
                    FederatedBatchDecision::Deferred => !deferred.contains(&decision.candidate_id),
                    FederatedBatchDecision::Rejected => !rejected.contains(&decision.candidate_id),
                })
        {
            return Err(FederatedBatchSchedulerError::InvalidOutput(
                "candidate partitions or decision classifications are inconsistent".into(),
            ));
        }
        let expected_selected_cycles = self
            .decisions
            .iter()
            .filter(|decision| decision.decision == FederatedBatchDecision::Selected)
            .map(|decision| decision.cycle_key.clone())
            .collect::<BTreeSet<_>>();
        let actual_selected_cycles = self
            .selected_cycle_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        if expected_selected_cycles != actual_selected_cycles
            || actual_selected_cycles.len() != self.selected_cycle_order.len()
            || self.selected_by_cycle.values().sum::<usize>() != self.selected_action_count
            || self.selected_by_route.values().sum::<usize>() != self.selected_action_count
        {
            return Err(FederatedBatchSchedulerError::InvalidOutput(
                "selected cycle/route accounting is inconsistent".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| FederatedBatchSchedulerError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(FederatedBatchSchedulerError::Digest(
                "batch schedule digest does not match canonical content".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct Candidate {
    candidate_id: String,
    cycle_key: String,
    cycle_epoch: u32,
    action: FederatedCycleAction,
    blocked: bool,
}

/// Schedule many advisory federation cycles with a hard global budget, per-cycle fairness,
/// per-route quotas, and dependency-prefix admission. No action is executed by this function.
pub fn schedule_glioma_federated_evidence_batch(
    request: &FederatedBatchSchedulerRequest,
) -> Result<FederatedBatchSchedule, FederatedBatchSchedulerError> {
    if request.objective.trim().is_empty()
        || request.schedule_epoch == 0
        || request.max_cycles == 0
        || request.max_cycles > MAX_CYCLES
        || request.max_selected_actions == 0
        || request.max_selected_actions > MAX_ACTIONS
        || request.max_actions_per_cycle == 0
        || request.max_actions_per_route == 0
        || request.cycles.len() > request.max_cycles
        || request.cycles.len() > MAX_CYCLES
    {
        return Err(FederatedBatchSchedulerError::InvalidRequest(
            "objective, schedule epoch, cycle bound, action bound, or quota is invalid".into(),
        ));
    }
    let mut cycles = request.cycles.clone();
    cycles.sort_by_key(cycle_key);
    if cycles
        .windows(2)
        .any(|pair| cycle_key(&pair[0]) == cycle_key(&pair[1]))
        || cycles
            .iter()
            .map(|cycle| cycle.digest.clone())
            .collect::<BTreeSet<_>>()
            .len()
            != cycles.len()
    {
        return Err(FederatedBatchSchedulerError::InvalidRequest(
            "cycle digests and epochs must uniquely identify each input cycle".into(),
        ));
    }
    for cycle in &cycles {
        cycle
            .validate()
            .map_err(|error| FederatedBatchSchedulerError::InvalidInput(error.to_string()))?;
    }
    let eligible_cycles = cycles
        .iter()
        .filter(|cycle| {
            !matches!(
                cycle.disposition,
                FederatedCycleDisposition::Blocked | FederatedCycleDisposition::Hold
            )
        })
        .collect::<Vec<_>>();
    let mut candidates = Vec::new();
    for cycle in &cycles {
        let key = cycle_key(cycle);
        let blocked = matches!(
            cycle.disposition,
            FederatedCycleDisposition::Blocked | FederatedCycleDisposition::Hold
        );
        for action in &cycle.actions {
            candidates.push(Candidate {
                candidate_id: candidate_id(&key, action),
                cycle_key: key.clone(),
                cycle_epoch: cycle.cycle_epoch,
                action: action.clone(),
                blocked,
            });
        }
    }
    if candidates.len() > MAX_ACTIONS {
        return Err(FederatedBatchSchedulerError::InvalidRequest(
            "input action portfolio exceeds scheduler bound".into(),
        ));
    }
    // Heads receive a fixed fairness bonus so every eligible cycle gets a chance before a
    // high-priority cycle consumes the whole global budget. The final order remains deterministic.
    candidates.sort_by(|left, right| {
        let left_score =
            left.action.priority_milli as u32 + if left.action.rank == 1 { 10_000 } else { 0 };
        let right_score =
            right.action.priority_milli as u32 + if right.action.rank == 1 { 10_000 } else { 0 };
        right_score
            .cmp(&left_score)
            .then_with(|| left.cycle_key.cmp(&right.cycle_key))
            .then_with(|| left.action.rank.cmp(&right.action.rank))
            .then_with(|| left.action.action_id.cmp(&right.action.action_id))
    });
    let mut selected_by_cycle = BTreeMap::<String, usize>::new();
    let mut selected_by_route = BTreeMap::<String, usize>::new();
    let mut decisions = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        let (decision, reason) = if candidate.blocked {
            (
                FederatedBatchDecision::Rejected,
                "source cycle is blocked or held for researcher review".into(),
            )
        } else if selected_by_cycle
            .get(&candidate.cycle_key)
            .copied()
            .unwrap_or_default()
            >= request.max_actions_per_cycle
        {
            (
                FederatedBatchDecision::Deferred,
                "per-cycle action quota is exhausted".into(),
            )
        } else if selected_by_route
            .get(&candidate.action.route)
            .copied()
            .unwrap_or_default()
            >= request.max_actions_per_route
        {
            (
                FederatedBatchDecision::Deferred,
                "per-route action quota is exhausted".into(),
            )
        } else if selected_by_cycle
            .get(&candidate.cycle_key)
            .copied()
            .unwrap_or_default()
            >= request.max_selected_actions
        {
            (
                FederatedBatchDecision::Deferred,
                "global action budget is exhausted".into(),
            )
        } else if selected_by_cycle
            .get(&candidate.cycle_key)
            .copied()
            .unwrap_or_default()
            + 1
            != candidate.action.rank
        {
            (
                FederatedBatchDecision::Deferred,
                "dependency-safe action prefix is not selected".into(),
            )
        } else if selected_by_cycle.values().sum::<usize>() >= request.max_selected_actions {
            (
                FederatedBatchDecision::Deferred,
                "global action budget is exhausted".into(),
            )
        } else {
            selected_by_cycle
                .entry(candidate.cycle_key.clone())
                .and_modify(|count| *count += 1)
                .or_insert(1);
            selected_by_route
                .entry(candidate.action.route.clone())
                .and_modify(|count| *count += 1)
                .or_insert(1);
            (
                FederatedBatchDecision::Selected,
                "fairness, route quota, budget, and dependency-prefix gates passed".into(),
            )
        };
        decisions.push(FederatedBatchActionDecision {
            candidate_id: candidate.candidate_id,
            cycle_key: candidate.cycle_key,
            cycle_epoch: candidate.cycle_epoch,
            action_id: candidate.action.action_id,
            rank: candidate.action.rank,
            route: candidate.action.route,
            priority_milli: candidate.action.priority_milli,
            decision,
            reason,
        });
    }
    decisions.sort_by(|left, right| left.candidate_id.cmp(&right.candidate_id));
    let candidate_order = decisions
        .iter()
        .map(|decision| decision.candidate_id.clone())
        .collect::<Vec<_>>();
    let selected_order = decisions
        .iter()
        .filter(|decision| decision.decision == FederatedBatchDecision::Selected)
        .map(|decision| decision.candidate_id.clone())
        .collect::<Vec<_>>();
    let deferred_order = decisions
        .iter()
        .filter(|decision| decision.decision == FederatedBatchDecision::Deferred)
        .map(|decision| decision.candidate_id.clone())
        .collect::<Vec<_>>();
    let rejected_order = decisions
        .iter()
        .filter(|decision| decision.decision == FederatedBatchDecision::Rejected)
        .map(|decision| decision.candidate_id.clone())
        .collect::<Vec<_>>();
    let selected_cycle_order = selected_by_cycle.keys().cloned().collect::<Vec<_>>();
    let selected_route_order = selected_by_route.keys().cloned().collect::<Vec<_>>();
    let selected_action_count = selected_order.len();
    let capacity_utilization_milli = ((selected_action_count as u64 * 1_000)
        / request.max_selected_actions as u64)
        .min(1_000) as u16;
    let fairness_milli = if eligible_cycles.is_empty() {
        0
    } else {
        ((selected_cycle_order.len() as u64 * 1_000) / eligible_cycles.len() as u64).min(1_000)
            as u16
    };
    let disposition = if selected_action_count == 0 {
        FederatedBatchDisposition::Blocked
    } else if !deferred_order.is_empty() || !rejected_order.is_empty() {
        FederatedBatchDisposition::Partial
    } else {
        FederatedBatchDisposition::Ready
    };
    let mut next_routes = selected_route_order.iter().cloned().collect::<Vec<_>>();
    if next_routes.is_empty() {
        next_routes.push("glioma_federated_evidence_operating_cycle".into());
    }
    let mut uncertainty = Vec::new();
    if fairness_milli < 1_000 {
        uncertainty.push("not_every_eligible_cycle_received_a_selected_action".into());
    }
    if !deferred_order.is_empty() {
        uncertainty.push("route_or_global_capacity_deferred_actions".into());
    }
    if !rejected_order.is_empty() {
        uncertainty.push("blocked_or_held_cycles_were_rejected_from_execution".into());
    }
    uncertainty.sort();
    let mut output = FederatedBatchSchedule {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        schedule_epoch: request.schedule_epoch,
        input_cycle_digest_order: cycles.iter().map(|cycle| cycle.digest.clone()).collect(),
        cycle_order: cycles.iter().map(cycle_key).collect(),
        candidate_order,
        selected_order,
        deferred_order,
        rejected_order,
        decisions,
        selected_by_cycle,
        selected_by_route,
        selected_cycle_order,
        selected_route_order,
        eligible_cycle_count: eligible_cycles.len(),
        selected_action_count,
        capacity_utilization_milli,
        fairness_milli,
        disposition,
        next_routes,
        uncertainty,
        digest: ContentHash::of_bytes(b"unsealed-glioma-federated-batch"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| FederatedBatchSchedulerError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p01_evidence_surveillance::federated_operating_cycle::{
        FederatedCycleAction, FederatedCycleActionKind,
    };

    fn hash(seed: &str) -> ContentHash {
        ContentHash::of_bytes(seed.as_bytes())
    }

    fn cycle(
        seed: &str,
        epoch: u32,
        disposition: FederatedCycleDisposition,
    ) -> FederatedEvidenceOperatingCycle {
        let action = FederatedCycleAction {
            action_id: "action-001".into(),
            rank: 1,
            kind: FederatedCycleActionKind::AcquireMissingCoverage,
            priority_milli: 900,
            rationale: "acquire missing coverage".into(),
            prerequisite_order: Vec::new(),
            route: "glioma_federated_evidence_acquisition_policy".into(),
            autonomy_tier: "A0_advisory_local_planner".into(),
        };
        let mut value = FederatedEvidenceOperatingCycle {
            feature_id: super::super::federated_operating_cycle::FEATURE_ID.into(),
            output_schema: super::super::federated_operating_cycle::OUTPUT_SCHEMA.into(),
            objective: format!("batch {seed}"),
            cycle_epoch: epoch,
            next_cycle_epoch: epoch + 1,
            transport_digest: hash(&format!("transport-{seed}")),
            calibration_digest: None,
            reconciliation_digest: None,
            accepted_bundle_order: Vec::new(),
            deferred_bundle_order: Vec::new(),
            denied_bundle_order: Vec::new(),
            negative_bundle_order: Vec::new(),
            contradicted_bundle_order: Vec::new(),
            unknown_bundle_order: Vec::new(),
            action_order: vec!["action-001".into()],
            actions: vec![action],
            omitted_action_order: Vec::new(),
            uncertainty: Vec::new(),
            disposition,
            next_routes: vec!["glioma_federated_evidence_acquisition_policy".into()],
            operator_summary: "bounded advisory cycle".into(),
            digest: hash("pending"),
        };
        value.digest = ContentHash::of_value(
            &super::super::federated_operating_cycle::digest_input(&value),
        )
        .unwrap();
        value
    }

    fn request(cycles: Vec<FederatedEvidenceOperatingCycle>) -> FederatedBatchSchedulerRequest {
        FederatedBatchSchedulerRequest {
            objective: "schedule federated cycles".into(),
            schedule_epoch: 8,
            max_cycles: 16,
            max_selected_actions: 2,
            max_actions_per_cycle: 1,
            max_actions_per_route: 8,
            cycles,
        }
    }

    #[test]
    fn fair_scheduler_selects_one_action_from_each_cycle() {
        let first = cycle("one", 8, FederatedCycleDisposition::Ready);
        let second = cycle("two", 8, FederatedCycleDisposition::Ready);
        let request = request(vec![first, second]);
        let output = schedule_glioma_federated_evidence_batch(&request).unwrap();
        assert_eq!(output.selected_action_count, 2);
        assert_eq!(output.fairness_milli, 1_000);
        assert_eq!(output.disposition, FederatedBatchDisposition::Ready);
        output.validate().unwrap();
    }

    #[test]
    fn blocked_cycle_is_rejected_and_visible() {
        let request = request(vec![
            cycle("blocked", 8, FederatedCycleDisposition::Blocked),
            cycle("ready", 8, FederatedCycleDisposition::Ready),
        ]);
        let output = schedule_glioma_federated_evidence_batch(&request).unwrap();
        assert_eq!(output.rejected_order.len(), 1);
        assert_eq!(output.selected_action_count, 1);
        assert_eq!(output.disposition, FederatedBatchDisposition::Partial);
    }

    #[test]
    fn global_budget_defers_lower_priority_work_deterministically() {
        let mut request = request(vec![
            cycle("one", 8, FederatedCycleDisposition::Ready),
            cycle("two", 8, FederatedCycleDisposition::Ready),
        ]);
        request.max_selected_actions = 1;
        let first = schedule_glioma_federated_evidence_batch(&request).unwrap();
        let second = schedule_glioma_federated_evidence_batch(&request).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.selected_action_count, 1);
        assert_eq!(first.deferred_order.len(), 1);
    }

    #[test]
    fn duplicate_cycle_identity_is_rejected() {
        let first = cycle("same", 8, FederatedCycleDisposition::Ready);
        let second = first.clone();
        let request = request(vec![first, second]);
        let error = schedule_glioma_federated_evidence_batch(&request).unwrap_err();
        assert!(matches!(
            error,
            FederatedBatchSchedulerError::InvalidRequest(_)
        ));
    }
}
