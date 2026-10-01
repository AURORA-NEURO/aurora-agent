//! Prospective benchmark director snapshot and bounded reallocation proposals.
//!
//! This feature gives a consortium director a deterministic view over concurrent aggregate-only
//! glioma benchmarks. It separates operational completion from scientific success, makes quorum,
//! privacy, budget, anomaly, freshness, and uncertainty risk visible, and proposes only bounded
//! reallocations inside already-declared scope. It never dispatches a site or expands a study.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P12-F19";
pub const OUTPUT_SCHEMA: &str = "GliomaBenchmarkDirectorSnapshot1@1";
pub const MAX_RUNS: usize = 256;
pub const MAX_PROPOSALS: usize = 256;
pub const MAX_ALERTS: usize = 2_048;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkDirectorRunStatus {
    Proposed,
    Running,
    Completed,
    Negative,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkDirectorAlertKind {
    QuorumDeficit,
    BudgetRisk,
    PrivacyBudgetRisk,
    Anomaly,
    Uncertainty,
    CompletionWithoutScientificSuccess,
    ApprovalBlocked,
    Stale,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkDirectorProposalKind {
    NoAction,
    ReallocateDeclaredCapacity,
    RequestSiteApproval,
    RequestScientificReview,
    HoldForEvidence,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkDirectorDisposition {
    Healthy,
    AtRisk,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchmarkDirectorRunInput {
    pub run_id: String,
    pub objective: String,
    pub status: BenchmarkDirectorRunStatus,
    pub required_sites: u32,
    pub admitted_sites: u32,
    pub completed_sites: u32,
    pub requested_budget_units: u64,
    pub spent_budget_units: u64,
    pub privacy_budget_milli: u64,
    pub privacy_spent_milli: u64,
    pub workload_units: u64,
    pub anomaly_count: u32,
    pub uncertainty_milli: u16,
    pub scientific_success_observed: bool,
    pub release_ready: bool,
    pub signed_run: bool,
    pub approval_complete: bool,
    pub freshness_tick: u64,
    pub deadline_tick: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchmarkDirectorRequest {
    pub objective: String,
    pub current_tick: u64,
    pub total_budget_units: u64,
    pub total_privacy_budget_milli: u64,
    pub max_reallocation_units: u64,
    pub minimum_quorum_sites: u32,
    pub runs: Vec<BenchmarkDirectorRunInput>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchmarkDirectorAlert {
    pub alert_id: String,
    pub run_id: String,
    pub kind: BenchmarkDirectorAlertKind,
    pub severity_milli: u16,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchmarkDirectorRunView {
    pub run_id: String,
    pub status: BenchmarkDirectorRunStatus,
    pub quorum_gap: u32,
    pub budget_remaining_units: u64,
    pub privacy_remaining_milli: u64,
    pub workload_units: u64,
    pub risk_milli: u16,
    pub uncertainty_milli: u16,
    pub scientific_success_observed: bool,
    pub release_ready: bool,
    pub alert_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchmarkDirectorProposal {
    pub proposal_id: String,
    pub kind: BenchmarkDirectorProposalKind,
    pub source_run_id: Option<String>,
    pub target_run_id: Option<String>,
    pub units: u64,
    pub requires_approval: bool,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BenchmarkDirectorSnapshot {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub current_tick: u64,
    pub run_order: Vec<String>,
    pub runs: Vec<BenchmarkDirectorRunView>,
    pub alert_order: Vec<String>,
    pub alerts: Vec<BenchmarkDirectorAlert>,
    pub proposal_order: Vec<String>,
    pub proposals: Vec<BenchmarkDirectorProposal>,
    pub total_requested_budget_units: u64,
    pub total_spent_budget_units: u64,
    pub total_privacy_budget_milli: u64,
    pub total_privacy_spent_milli: u64,
    pub total_workload_units: u64,
    pub release_ready_order: Vec<String>,
    pub scientific_success_order: Vec<String>,
    pub completion_without_success_order: Vec<String>,
    pub dispatch_permitted: bool,
    pub disposition: BenchmarkDirectorDisposition,
    pub next_operator_action: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BenchmarkDirectorError {
    #[error("benchmark director request is invalid: {0}")]
    InvalidRequest(String),
    #[error("benchmark director output is invalid: {0}")]
    InvalidOutput(String),
    #[error("benchmark director digest failed: {0}")]
    Digest(String),
}

fn safe_text(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 256 && !value.chars().any(char::is_control)
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_body(snapshot: &BenchmarkDirectorSnapshot) -> serde_json::Value {
    serde_json::json!({
        "feature_id": snapshot.feature_id,
        "output_schema": snapshot.output_schema,
        "objective": snapshot.objective,
        "current_tick": snapshot.current_tick,
        "run_order": snapshot.run_order,
        "runs": snapshot.runs,
        "alert_order": snapshot.alert_order,
        "alerts": snapshot.alerts,
        "proposal_order": snapshot.proposal_order,
        "proposals": snapshot.proposals,
        "total_requested_budget_units": snapshot.total_requested_budget_units,
        "total_spent_budget_units": snapshot.total_spent_budget_units,
        "total_privacy_budget_milli": snapshot.total_privacy_budget_milli,
        "total_privacy_spent_milli": snapshot.total_privacy_spent_milli,
        "total_workload_units": snapshot.total_workload_units,
        "release_ready_order": snapshot.release_ready_order,
        "scientific_success_order": snapshot.scientific_success_order,
        "completion_without_success_order": snapshot.completion_without_success_order,
        "dispatch_permitted": snapshot.dispatch_permitted,
        "disposition": snapshot.disposition,
        "next_operator_action": snapshot.next_operator_action,
    })
}

fn validate_request(request: &BenchmarkDirectorRequest) -> Result<(), BenchmarkDirectorError> {
    if !safe_text(&request.objective)
        || request.runs.is_empty()
        || request.runs.len() > MAX_RUNS
        || request.minimum_quorum_sites == 0
    {
        return Err(BenchmarkDirectorError::InvalidRequest(
            "objective, bounded runs, and a positive quorum are required".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for run in &request.runs {
        if !safe_text(&run.run_id)
            || !ids.insert(run.run_id.clone())
            || !safe_text(&run.objective)
            || run.objective != request.objective
            || run.required_sites == 0
            || run.admitted_sites > run.required_sites
            || run.completed_sites > run.admitted_sites
            || run.spent_budget_units > run.requested_budget_units
            || run.privacy_spent_milli > run.privacy_budget_milli
            || run.uncertainty_milli > 1_000
            || run.freshness_tick > request.current_tick
        {
            return Err(BenchmarkDirectorError::InvalidRequest(format!(
                "run {} is malformed, out of scope, or not objective-bound",
                run.run_id
            )));
        }
    }
    Ok(())
}

impl BenchmarkDirectorSnapshot {
    pub fn validate(&self) -> Result<(), BenchmarkDirectorError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !safe_text(&self.objective)
            || !canonical(&self.run_order)
            || self.runs.len() != self.run_order.len()
            || !canonical(&self.alert_order)
            || !canonical(&self.proposal_order)
            || !canonical(&self.release_ready_order)
            || !canonical(&self.scientific_success_order)
            || !canonical(&self.completion_without_success_order)
            || self.alerts.len() > MAX_ALERTS
            || self.proposals.len() > MAX_PROPOSALS
            || self.total_spent_budget_units > self.total_requested_budget_units
            || self.total_privacy_spent_milli > self.total_privacy_budget_milli
            || self.dispatch_permitted
            || !safe_text(&self.next_operator_action)
            || self.digest.as_str().len() != 64
        {
            return Err(BenchmarkDirectorError::InvalidOutput(
                "director identity, ordering, bounds, or dispatch invariants are invalid".into(),
            ));
        }
        if self.alerts.iter().any(|alert| {
            !safe_text(&alert.alert_id)
                || !safe_text(&alert.run_id)
                || !safe_text(&alert.reason)
                || alert.severity_milli > 1_000
        }) || self.proposals.iter().any(|proposal| {
            !safe_text(&proposal.proposal_id)
                || !safe_text(&proposal.reason)
                || proposal.units == 0
                    && !matches!(proposal.kind, BenchmarkDirectorProposalKind::NoAction)
        }) {
            return Err(BenchmarkDirectorError::InvalidOutput(
                "alert or proposal contract is malformed".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_body(self))
            .map_err(|error| BenchmarkDirectorError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(BenchmarkDirectorError::InvalidOutput(
                "director snapshot digest is not content-bound".into(),
            ));
        }
        Ok(())
    }
}

/// Build a read-only director snapshot and bounded proposals for concurrent aggregate benchmarks.
pub fn build_glioma_benchmark_director_snapshot(
    request: &BenchmarkDirectorRequest,
) -> Result<BenchmarkDirectorSnapshot, BenchmarkDirectorError> {
    validate_request(request)?;
    let mut runs = request.runs.clone();
    runs.sort_by(|left, right| left.run_id.cmp(&right.run_id));
    let mut alerts = Vec::new();
    let mut views = Vec::with_capacity(runs.len());
    let mut release_ready_order = Vec::new();
    let mut scientific_success_order = Vec::new();
    let mut completion_without_success_order = Vec::new();
    let mut requested_total = 0u64;
    let mut spent_total = 0u64;
    let mut privacy_total = 0u64;
    let mut privacy_spent_total = 0u64;
    let mut workload_total = 0u64;
    for run in &runs {
        requested_total = requested_total.saturating_add(run.requested_budget_units);
        spent_total = spent_total.saturating_add(run.spent_budget_units);
        privacy_total = privacy_total.saturating_add(run.privacy_budget_milli);
        privacy_spent_total = privacy_spent_total.saturating_add(run.privacy_spent_milli);
        workload_total = workload_total.saturating_add(run.workload_units);
        let quorum_gap = request
            .minimum_quorum_sites
            .max(run.required_sites)
            .saturating_sub(run.admitted_sites);
        let mut run_alerts = Vec::new();
        let mut risk = 0u32;
        let mut add_alert = |kind: BenchmarkDirectorAlertKind, severity: u16, reason: &str| {
            let alert_id = format!("alert-{}-{:?}", run.run_id, kind).to_lowercase();
            run_alerts.push(alert_id.clone());
            alerts.push(BenchmarkDirectorAlert {
                alert_id,
                run_id: run.run_id.clone(),
                kind,
                severity_milli: severity,
                reason: reason.into(),
            });
            risk = risk.saturating_add(u32::from(severity) / 3);
        };
        if quorum_gap > 0 {
            add_alert(
                BenchmarkDirectorAlertKind::QuorumDeficit,
                (quorum_gap.saturating_mul(250)).min(1_000) as u16,
                "admitted sites do not satisfy the declared quorum",
            );
        }
        if run.spent_budget_units.saturating_mul(1_000)
            >= run.requested_budget_units.saturating_mul(850)
        {
            add_alert(
                BenchmarkDirectorAlertKind::BudgetRisk,
                if run.spent_budget_units >= run.requested_budget_units {
                    1_000
                } else {
                    700
                },
                "declared run budget is exhausted or within the risk band",
            );
        }
        if run.privacy_spent_milli.saturating_mul(1_000)
            >= run.privacy_budget_milli.saturating_mul(850)
        {
            add_alert(
                BenchmarkDirectorAlertKind::PrivacyBudgetRisk,
                if run.privacy_spent_milli >= run.privacy_budget_milli {
                    1_000
                } else {
                    700
                },
                "privacy budget is exhausted or within the risk band",
            );
        }
        if run.anomaly_count > 0 {
            add_alert(
                BenchmarkDirectorAlertKind::Anomaly,
                (run.anomaly_count.saturating_mul(200)).min(1_000) as u16,
                "aggregate anomalies require independent site-local review",
            );
        }
        if run.uncertainty_milli > 500 {
            add_alert(
                BenchmarkDirectorAlertKind::Uncertainty,
                run.uncertainty_milli,
                "uncertainty remains above the director review threshold",
            );
        }
        if run.status == BenchmarkDirectorRunStatus::Completed && !run.scientific_success_observed {
            completion_without_success_order.push(run.run_id.clone());
            add_alert(
                BenchmarkDirectorAlertKind::CompletionWithoutScientificSuccess,
                900,
                "operational completion is not evidence of scientific success",
            );
        }
        if !run.approval_complete {
            add_alert(
                BenchmarkDirectorAlertKind::ApprovalBlocked,
                1_000,
                "local approval is incomplete; no scope expansion or query dispatch is allowed",
            );
        }
        if request.current_tick.saturating_sub(run.freshness_tick) >= 100
            || run.deadline_tick < request.current_tick
        {
            add_alert(
                BenchmarkDirectorAlertKind::Stale,
                800,
                "run telemetry or deadline is stale at the snapshot tick",
            );
        }
        if run.scientific_success_observed {
            scientific_success_order.push(run.run_id.clone());
        }
        let release_ready = run.status == BenchmarkDirectorRunStatus::Completed
            && run.release_ready
            && run.signed_run
            && run.approval_complete
            && quorum_gap == 0
            && run.anomaly_count == 0
            && run.uncertainty_milli <= 500;
        if release_ready {
            release_ready_order.push(run.run_id.clone());
        }
        views.push(BenchmarkDirectorRunView {
            run_id: run.run_id.clone(),
            status: run.status,
            quorum_gap,
            budget_remaining_units: run
                .requested_budget_units
                .saturating_sub(run.spent_budget_units),
            privacy_remaining_milli: run
                .privacy_budget_milli
                .saturating_sub(run.privacy_spent_milli),
            workload_units: run.workload_units,
            risk_milli: risk.min(1_000) as u16,
            uncertainty_milli: run.uncertainty_milli,
            scientific_success_observed: run.scientific_success_observed,
            release_ready,
            alert_order: run_alerts,
        });
    }
    alerts.sort_by(|left, right| left.alert_id.cmp(&right.alert_id));
    let alert_order = alerts
        .iter()
        .map(|alert| alert.alert_id.clone())
        .collect::<Vec<_>>();
    let mut proposals = Vec::new();
    let mut donor_capacity = request
        .total_budget_units
        .saturating_sub(spent_total)
        .min(request.max_reallocation_units);
    for target in &runs {
        if donor_capacity == 0 || proposals.len() >= MAX_PROPOSALS {
            break;
        }
        let demand = target
            .requested_budget_units
            .saturating_sub(target.spent_budget_units);
        if demand == 0
            || !matches!(
                target.status,
                BenchmarkDirectorRunStatus::Proposed | BenchmarkDirectorRunStatus::Running
            )
            || target.approval_complete
        {
            continue;
        }
        let units = demand.min(donor_capacity);
        proposals.push(BenchmarkDirectorProposal {
            proposal_id: format!("proposal-reallocate-{}", target.run_id),
            kind: BenchmarkDirectorProposalKind::ReallocateDeclaredCapacity,
            source_run_id: None,
            target_run_id: Some(target.run_id.clone()),
            units,
            requires_approval: true,
            reason:
                "bounded proposal only; local approval and unchanged declared scope are required"
                    .into(),
        });
        donor_capacity -= units;
    }
    for run in &runs {
        if proposals.len() >= MAX_PROPOSALS {
            break;
        }
        if run.status == BenchmarkDirectorRunStatus::Blocked && !run.approval_complete {
            proposals.push(BenchmarkDirectorProposal {
                proposal_id: format!("proposal-approval-{}", run.run_id),
                kind: BenchmarkDirectorProposalKind::RequestSiteApproval,
                source_run_id: None,
                target_run_id: Some(run.run_id.clone()),
                units: 0,
                requires_approval: true,
                reason: "request local approval review; do not dispatch or widen the benchmark"
                    .into(),
            });
        }
    }
    proposals.sort_by(|left, right| left.proposal_id.cmp(&right.proposal_id));
    let proposal_order = proposals
        .iter()
        .map(|proposal| proposal.proposal_id.clone())
        .collect::<Vec<_>>();
    let disposition = if runs
        .iter()
        .any(|run| run.status == BenchmarkDirectorRunStatus::Blocked)
        || spent_total > request.total_budget_units
        || privacy_spent_total > request.total_privacy_budget_milli
    {
        BenchmarkDirectorDisposition::Blocked
    } else if runs
        .iter()
        .any(|run| run.status == BenchmarkDirectorRunStatus::Unresolved)
    {
        BenchmarkDirectorDisposition::Unresolved
    } else if !alerts.is_empty() {
        BenchmarkDirectorDisposition::AtRisk
    } else {
        BenchmarkDirectorDisposition::Healthy
    };
    let next_operator_action = match disposition {
        BenchmarkDirectorDisposition::Healthy => {
            "continue bounded benchmark execution and review scientific outcomes separately".into()
        }
        BenchmarkDirectorDisposition::AtRisk => {
            "review alerts, quorum, privacy, and uncertainty before accepting any proposal".into()
        }
        BenchmarkDirectorDisposition::Blocked => {
            "hold dispatch, resolve blockers locally, and preserve the blocked evidence".into()
        }
        BenchmarkDirectorDisposition::Unresolved => {
            "obtain missing evidence or site decisions; unresolved state is not approval".into()
        }
    };
    let mut snapshot = BenchmarkDirectorSnapshot {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        current_tick: request.current_tick,
        run_order: runs.iter().map(|run| run.run_id.clone()).collect(),
        runs: views,
        alert_order,
        alerts,
        proposal_order,
        proposals,
        total_requested_budget_units: requested_total,
        total_spent_budget_units: spent_total,
        total_privacy_budget_milli: privacy_total,
        total_privacy_spent_milli: privacy_spent_total,
        total_workload_units: workload_total,
        release_ready_order,
        scientific_success_order,
        completion_without_success_order,
        dispatch_permitted: false,
        disposition,
        next_operator_action,
        digest: ContentHash::of_bytes(b"unsealed-glioma-benchmark-director"),
    };
    snapshot.digest = ContentHash::of_value(&digest_body(&snapshot))
        .map_err(|error| BenchmarkDirectorError::Digest(error.to_string()))?;
    snapshot.validate()?;
    Ok(snapshot)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(id: &str, status: BenchmarkDirectorRunStatus) -> BenchmarkDirectorRunInput {
        BenchmarkDirectorRunInput {
            run_id: id.into(),
            objective: "compare invasion phenotypes".into(),
            status,
            required_sites: 3,
            admitted_sites: 3,
            completed_sites: 2,
            requested_budget_units: 100,
            spent_budget_units: 20,
            privacy_budget_milli: 1_000,
            privacy_spent_milli: 100,
            workload_units: 10,
            anomaly_count: 0,
            uncertainty_milli: 300,
            scientific_success_observed: false,
            release_ready: false,
            signed_run: false,
            approval_complete: true,
            freshness_tick: 95,
            deadline_tick: 200,
        }
    }

    fn request(runs: Vec<BenchmarkDirectorRunInput>) -> BenchmarkDirectorRequest {
        BenchmarkDirectorRequest {
            objective: "compare invasion phenotypes".into(),
            current_tick: 100,
            total_budget_units: 1_000,
            total_privacy_budget_milli: 10_000,
            max_reallocation_units: 200,
            minimum_quorum_sites: 2,
            runs,
        }
    }

    #[test]
    fn clean_snapshot_is_deterministic_and_non_dispatching() {
        let snapshot = build_glioma_benchmark_director_snapshot(&request(vec![run(
            "run-a",
            BenchmarkDirectorRunStatus::Running,
        )]))
        .unwrap();
        assert_eq!(snapshot.disposition, BenchmarkDirectorDisposition::Healthy);
        assert!(!snapshot.dispatch_permitted);
        assert!(snapshot.validate().is_ok());
    }

    #[test]
    fn completion_does_not_imply_scientific_success_or_release() {
        let mut completed = run("run-a", BenchmarkDirectorRunStatus::Completed);
        completed.completed_sites = 3;
        completed.approval_complete = true;
        let snapshot = build_glioma_benchmark_director_snapshot(&request(vec![completed])).unwrap();
        assert!(snapshot
            .completion_without_success_order
            .contains(&"run-a".into()));
        assert!(snapshot.release_ready_order.is_empty());
        assert!(snapshot
            .alerts
            .iter()
            .any(|alert| alert.kind
                == BenchmarkDirectorAlertKind::CompletionWithoutScientificSuccess));
    }

    #[test]
    fn quorum_privacy_and_stale_risks_remain_separate() {
        let mut risky = run("run-a", BenchmarkDirectorRunStatus::Running);
        risky.admitted_sites = 1;
        risky.completed_sites = 1;
        risky.privacy_spent_milli = 900;
        risky.freshness_tick = 0;
        let snapshot = build_glioma_benchmark_director_snapshot(&request(vec![risky])).unwrap();
        let kinds = snapshot
            .alerts
            .iter()
            .map(|alert| alert.kind)
            .collect::<BTreeSet<_>>();
        assert!(kinds.contains(&BenchmarkDirectorAlertKind::QuorumDeficit));
        assert!(kinds.contains(&BenchmarkDirectorAlertKind::PrivacyBudgetRisk));
        assert!(kinds.contains(&BenchmarkDirectorAlertKind::Stale));
    }

    #[test]
    fn proposals_require_approval_and_do_not_expand_scope() {
        let mut proposed = run("run-a", BenchmarkDirectorRunStatus::Proposed);
        proposed.approval_complete = false;
        let snapshot = build_glioma_benchmark_director_snapshot(&request(vec![proposed])).unwrap();
        assert!(snapshot.proposals.iter().any(|proposal| {
            proposal.kind == BenchmarkDirectorProposalKind::ReallocateDeclaredCapacity
                && proposal.requires_approval
        }));
        assert!(!snapshot.dispatch_permitted);
        assert!(snapshot.validate().is_ok());
    }

    #[test]
    fn malformed_objective_or_freshness_is_rejected() {
        let mut invalid = run("run-a", BenchmarkDirectorRunStatus::Running);
        invalid.objective = "different".into();
        assert!(build_glioma_benchmark_director_snapshot(&request(vec![invalid])).is_err());
        let mut stale = run("run-b", BenchmarkDirectorRunStatus::Running);
        stale.freshness_tick = 101;
        assert!(build_glioma_benchmark_director_snapshot(&request(vec![stale])).is_err());
    }
}
