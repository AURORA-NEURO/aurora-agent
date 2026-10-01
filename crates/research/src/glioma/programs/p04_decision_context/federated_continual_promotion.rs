//! Continual promotion gate over independently digested, site-local decision-context epochs.
//!
//! This layer consumes only the aggregate reports produced by P04-F04. It keeps the complete
//! epoch ledger so a promotion decision can be replayed after transport, requires a stable
//! qualified branch across consecutive epochs, and never lets repeated observation identifiers
//! manufacture temporal support. Negative, contradicted, failed, unknown, and omitted evidence
//! remains in the retained child reports and is reflected in explicit reason codes.

use super::federated_decision_context::{
    FederatedBranchDisposition, FederatedDecisionContextReport, FederatedDecisionDisposition,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = super::snapshot_store::FEATURE_ID;
pub const OUTPUT_SCHEMA: &str = "GliomaFederatedContinualContextPromotion1@1";
pub const MAX_EPOCHS: usize = 32;
pub const MIN_STABILITY_EPOCHS: usize = 2;
pub const MAX_STABILITY_EPOCHS: usize = 32;
pub const MAX_BRANCH_EPOCH_ROWS: usize = 8_192;
pub const MAX_RETAINED_OBSERVATIONS: usize = 65_536;
pub const MAX_REQUEST_BYTES: usize = 4_000_000;
pub const MAX_REPORT_BYTES: usize = 4_500_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedContinualPromotionDisposition {
    Promote,
    Continue,
    Hold,
    Reject,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContinualPromotionRequest {
    pub objective: String,
    pub minimum_consecutive_epochs: usize,
    pub minimum_sites: usize,
    pub minimum_independent_groups: usize,
    pub minimum_support_milli: u16,
    pub maximum_uncertainty_milli: u16,
    pub maximum_failure_risk_milli: u16,
    pub maximum_heterogeneity_milli: u16,
    pub maximum_influence_milli: u16,
    pub source_reports: Vec<FederatedDecisionContextReport>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContinualPromotionReport {
    pub feature_id: String,
    pub output_schema: String,
    pub request: FederatedContinualPromotionRequest,
    pub epoch_order: Vec<u32>,
    pub source_report_digest_order: Vec<String>,
    pub selected_branch_order: Vec<Option<String>>,
    pub stable_branch_id: Option<String>,
    pub stable_epoch_order: Vec<u32>,
    pub stable_epoch_count: usize,
    pub promoted_branch_id: Option<String>,
    pub negative_evidence_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub reason_codes: Vec<String>,
    pub disposition: FederatedContinualPromotionDisposition,
    pub next_route: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedContinualPromotionError {
    #[error("federated continual-context promotion request is invalid: {0}")]
    InvalidRequest(String),
    #[error("federated continual-context promotion output is invalid: {0}")]
    InvalidOutput(String),
    #[error("federated continual-context promotion digest failed: {0}")]
    Digest(String),
}

fn digest_input(report: &FederatedContinualPromotionReport) -> serde_json::Value {
    serde_json::json!({
        "feature_id": report.feature_id,
        "output_schema": report.output_schema,
        "request": report.request,
        "epoch_order": report.epoch_order,
        "source_report_digest_order": report.source_report_digest_order,
        "selected_branch_order": report.selected_branch_order,
        "stable_branch_id": report.stable_branch_id,
        "stable_epoch_order": report.stable_epoch_order,
        "stable_epoch_count": report.stable_epoch_count,
        "promoted_branch_id": report.promoted_branch_id,
        "negative_evidence_order": report.negative_evidence_order,
        "uncertainty_order": report.uncertainty_order,
        "reason_codes": report.reason_codes,
        "disposition": report.disposition,
        "next_route": report.next_route,
    })
}

fn fail_request(message: impl Into<String>) -> FederatedContinualPromotionError {
    FederatedContinualPromotionError::InvalidRequest(message.into())
}

fn validate_request(
    request: &FederatedContinualPromotionRequest,
) -> Result<(), FederatedContinualPromotionError> {
    if request.objective.trim().is_empty()
        || !(MIN_STABILITY_EPOCHS..=MAX_STABILITY_EPOCHS)
            .contains(&request.minimum_consecutive_epochs)
        || request.minimum_sites == 0
        || request.minimum_sites > super::federated_decision_context::MAX_SITES
        || request.minimum_independent_groups == 0
        || request.minimum_independent_groups > super::federated_decision_context::MAX_SITES
        || request.minimum_support_milli > 1_000
        || request.maximum_uncertainty_milli > 1_000
        || request.maximum_failure_risk_milli > 1_000
        || request.maximum_heterogeneity_milli > 1_000
        || request.maximum_influence_milli > 1_000
        || request.source_reports.is_empty()
        || request.source_reports.len() > MAX_EPOCHS
    {
        return Err(fail_request(
            "objective, temporal quorum, site quorum, thresholds, or epoch count is outside its bounds",
        ));
    }
    let request_bytes = serde_json::to_vec(request)
        .map_err(|error| fail_request(format!("request encoding failed: {error}")))?
        .len();
    if request_bytes > MAX_REQUEST_BYTES {
        return Err(fail_request(format!(
            "request is {request_bytes} bytes, above the {MAX_REQUEST_BYTES}-byte limit"
        )));
    }

    let mut previous_epoch = None;
    let mut observation_ids = BTreeSet::new();
    let mut branch_epoch_rows = 0usize;
    let mut retained_observations = 0usize;
    for report in &request.source_reports {
        report
            .validate()
            .map_err(|error| fail_request(format!("source report failed validation: {error}")))?;
        if report.objective != request.objective {
            return Err(fail_request(
                "every source report must bind the exact requested objective",
            ));
        }
        if let Some(previous) = previous_epoch {
            if report.epoch != previous + 1 {
                return Err(fail_request(
                    "source reports must be unique and in consecutive epoch order",
                ));
            }
        }
        previous_epoch = Some(report.epoch);
        branch_epoch_rows = branch_epoch_rows.saturating_add(report.branches.len());
        for branch in &report.branches {
            retained_observations =
                retained_observations.saturating_add(branch.observation_order.len());
            if branch
                .observation_order
                .iter()
                .any(|observation| !observation_ids.insert(observation.clone()))
            {
                return Err(fail_request(
                    "observation identities must be unique across all retained epochs",
                ));
            }
        }
    }
    if branch_epoch_rows > MAX_BRANCH_EPOCH_ROWS
        || retained_observations > MAX_RETAINED_OBSERVATIONS
    {
        return Err(fail_request(
            "retained branch or observation rows exceed their aggregate limits",
        ));
    }
    Ok(())
}

fn route_for(disposition: FederatedContinualPromotionDisposition) -> &'static str {
    match disposition {
        FederatedContinualPromotionDisposition::Promote => "glioma_decision_operating_cycle",
        FederatedContinualPromotionDisposition::Continue => {
            "glioma_federated_continual_context_promotion"
        }
        FederatedContinualPromotionDisposition::Hold => "glioma_researcher_workbench",
        FederatedContinualPromotionDisposition::Reject => "glioma_decision_branch_plan",
    }
}

fn report_route(report: &FederatedDecisionContextReport) -> bool {
    match report.disposition {
        FederatedDecisionDisposition::Promote => {
            report.selected_branch_id.is_some()
                && report.frontier_order.first() == report.selected_branch_id.as_ref()
        }
        FederatedDecisionDisposition::Continue
        | FederatedDecisionDisposition::Hold
        | FederatedDecisionDisposition::Reject => report.selected_branch_id.is_none(),
    }
}

fn adverse_branch_ids(request: &FederatedContinualPromotionRequest) -> BTreeSet<String> {
    request
        .source_reports
        .iter()
        .flat_map(|report| {
            report
                .branches
                .iter()
                .filter(|branch| {
                    !branch.negative_order.is_empty()
                        || !branch.contradicted_order.is_empty()
                        || !branch.failed_order.is_empty()
                })
                .map(|branch| branch.branch_id.clone())
        })
        .collect()
}

fn branch_qualifies(
    request: &FederatedContinualPromotionRequest,
    report: &FederatedDecisionContextReport,
    adverse_branches: &BTreeSet<String>,
) -> Option<String> {
    if report.disposition != FederatedDecisionDisposition::Promote || !report_route(report) {
        return None;
    }
    let branch_id = report.selected_branch_id.as_ref()?;
    if adverse_branches.contains(branch_id) {
        return None;
    }
    let branch = report
        .branches
        .iter()
        .find(|branch| &branch.branch_id == branch_id)?;
    (branch.disposition == FederatedBranchDisposition::Qualified
        && branch.eligible_site_order.len() >= request.minimum_sites
        && branch.independent_group_count >= request.minimum_independent_groups
        && branch.qualified_count >= request.minimum_sites
        && branch.support_milli >= request.minimum_support_milli
        && branch.uncertainty_milli <= request.maximum_uncertainty_milli
        && branch.failure_risk_milli <= request.maximum_failure_risk_milli
        && branch.heterogeneity_milli <= request.maximum_heterogeneity_milli
        && branch.maximum_influence_milli <= request.maximum_influence_milli
        && branch.negative_order.is_empty()
        && branch.contradicted_order.is_empty()
        && branch.unknown_order.is_empty()
        && branch.failed_order.is_empty())
    .then(|| branch_id.clone())
}

fn calculate(
    request: &FederatedContinualPromotionRequest,
) -> Result<FederatedContinualPromotionReport, FederatedContinualPromotionError> {
    let adverse_branches = adverse_branch_ids(request);
    let mut epoch_order = Vec::with_capacity(request.source_reports.len());
    let mut source_report_digest_order = Vec::with_capacity(request.source_reports.len());
    let mut selected_branch_order = Vec::with_capacity(request.source_reports.len());
    let mut negative_evidence_order = BTreeSet::new();
    let mut uncertainty_order = BTreeSet::new();
    let mut reason_codes = BTreeSet::new();
    let mut stable_branch_id: Option<String> = None;
    let mut stable_epoch_order = Vec::new();

    for report in &request.source_reports {
        epoch_order.push(report.epoch);
        source_report_digest_order.push(report.digest.as_str().to_string());
        selected_branch_order.push(report.selected_branch_id.clone());
        if !report_route(report) {
            return Err(FederatedContinualPromotionError::InvalidRequest(
                "promoted source report selection does not match its frontier rank".into(),
            ));
        }
        for item in &report.negative_evidence_order {
            negative_evidence_order.insert(format!("epoch-{}:{item}", report.epoch));
        }
        for item in &report.uncertainty_order {
            uncertainty_order.insert(format!("epoch-{}:{item}", report.epoch));
        }

        if let Some(branch_id) = branch_qualifies(request, report, &adverse_branches) {
            if stable_branch_id.as_ref() == Some(&branch_id) {
                stable_epoch_order.push(report.epoch);
            } else {
                stable_branch_id = Some(branch_id);
                stable_epoch_order.clear();
                stable_epoch_order.push(report.epoch);
            }
        } else {
            if let Some(branch_id) = report.selected_branch_id.as_ref() {
                let child_branch = report
                    .branches
                    .iter()
                    .find(|branch| &branch.branch_id == branch_id);
                if adverse_branches.contains(branch_id)
                    || child_branch.is_some_and(|branch| {
                        !branch.negative_order.is_empty()
                            || !branch.contradicted_order.is_empty()
                            || !branch.failed_order.is_empty()
                    })
                {
                    reason_codes.insert("selected_branch_has_adverse_history".to_string());
                } else if child_branch.is_some_and(|branch| !branch.unknown_order.is_empty()) {
                    reason_codes.insert("selected_branch_has_unknown_evidence".to_string());
                } else {
                    reason_codes.insert("selected_branch_failed_promotion_thresholds".to_string());
                }
            } else if report.disposition == FederatedDecisionDisposition::Reject {
                reason_codes.insert("latest_or_prior_epoch_rejected_all_branches".to_string());
            } else {
                reason_codes.insert("epoch_has_no_promotable_branch".to_string());
            }
            stable_branch_id = None;
            stable_epoch_order.clear();
        }
    }

    let stable_epoch_count = stable_epoch_order.len();
    let latest = request
        .source_reports
        .last()
        .expect("validated request has at least one source report");
    let (disposition, promoted_branch_id) =
        if latest.disposition == FederatedDecisionDisposition::Reject {
            reason_codes.insert("latest_epoch_rejected_all_branches".into());
            (FederatedContinualPromotionDisposition::Reject, None)
        } else if stable_epoch_count >= request.minimum_consecutive_epochs {
            (
                FederatedContinualPromotionDisposition::Promote,
                stable_branch_id.clone(),
            )
        } else if reason_codes.contains("selected_branch_has_adverse_history") {
            (FederatedContinualPromotionDisposition::Hold, None)
        } else if request.source_reports.len() < request.minimum_consecutive_epochs
            || latest.disposition == FederatedDecisionDisposition::Continue
            || stable_epoch_count > 0
        {
            reason_codes.insert("consecutive_epoch_stability_not_yet_met".into());
            (FederatedContinualPromotionDisposition::Continue, None)
        } else {
            (FederatedContinualPromotionDisposition::Hold, None)
        };
    if disposition == FederatedContinualPromotionDisposition::Promote {
        reason_codes.insert("stable_independent_epoch_window_qualified".into());
    }

    let mut report = FederatedContinualPromotionReport {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        request: request.clone(),
        epoch_order,
        source_report_digest_order,
        selected_branch_order,
        stable_branch_id,
        stable_epoch_order,
        stable_epoch_count,
        promoted_branch_id,
        negative_evidence_order: negative_evidence_order.into_iter().collect(),
        uncertainty_order: uncertainty_order.into_iter().collect(),
        reason_codes: reason_codes.into_iter().collect(),
        disposition,
        next_route: route_for(disposition).into(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-federated-continual-context-promotion"),
    };
    report.digest = ContentHash::of_value(&digest_input(&report))
        .map_err(|error| FederatedContinualPromotionError::Digest(error.to_string()))?;
    let report_bytes = serde_json::to_vec(&report)
        .map_err(|error| FederatedContinualPromotionError::Digest(error.to_string()))?
        .len();
    if report_bytes > MAX_REPORT_BYTES {
        return Err(FederatedContinualPromotionError::InvalidRequest(format!(
            "report is {report_bytes} bytes, above the {MAX_REPORT_BYTES}-byte limit"
        )));
    }
    Ok(report)
}

impl FederatedContinualPromotionReport {
    /// Revalidates all embedded child reports and replays the promotion decision from the request.
    pub fn validate(&self) -> Result<(), FederatedContinualPromotionError> {
        if self.feature_id != FEATURE_ID || self.output_schema != OUTPUT_SCHEMA {
            return Err(FederatedContinualPromotionError::InvalidOutput(
                "feature or output schema identity is invalid".into(),
            ));
        }
        validate_request(&self.request).map_err(|error| {
            FederatedContinualPromotionError::InvalidOutput(format!(
                "embedded request failed validation: {error}"
            ))
        })?;
        let expected = calculate(&self.request).map_err(|error| {
            FederatedContinualPromotionError::InvalidOutput(format!(
                "embedded request cannot be replayed: {error}"
            ))
        })?;
        if expected != *self {
            return Err(FederatedContinualPromotionError::InvalidOutput(
                "promotion report does not match deterministic replay of its retained epoch ledger"
                    .into(),
            ));
        }
        Ok(())
    }
}

/// Evaluates whether the same robust branch has remained qualified across independent epochs.
pub fn promote_glioma_federated_continual_context(
    request: &FederatedContinualPromotionRequest,
) -> Result<FederatedContinualPromotionReport, FederatedContinualPromotionError> {
    validate_request(request)?;
    let report = calculate(request)?;
    report.validate()?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p04_decision_context::federated_decision_context::{
        FederatedBranchOutcome, FederatedDecisionBranchObservation,
        FederatedDecisionContextRequest, FederatedDecisionSiteSummary,
        aggregate_glioma_federated_decision_context,
    };

    fn observation(site: &str, branch: &str, epoch: u32) -> FederatedDecisionBranchObservation {
        FederatedDecisionBranchObservation {
            observation_id: format!("{site}-{branch}-epoch-{epoch}"),
            branch_id: branch.into(),
            outcome: FederatedBranchOutcome::Qualified,
            expected_value_milli: 800,
            worst_case_value_milli: 700,
            uncertainty_milli: 100,
            failure_risk_milli: 100,
            support_milli: 900,
            cost_units: 2,
        }
    }

    fn source_report_with_observation_epoch(
        epoch: u32,
        branch: &str,
        observation_epoch: u32,
    ) -> FederatedDecisionContextReport {
        let sites = ["site-a", "site-b"]
            .into_iter()
            .enumerate()
            .map(|(index, site)| FederatedDecisionSiteSummary {
                site_id: site.into(),
                independent_group: format!("group-{index}"),
                plan_digest: ContentHash::of_bytes(format!("{site}-{epoch}").as_bytes()),
                quality_milli: 900,
                local_only: true,
                aggregate_only: true,
                policy_allowed: true,
                observations: vec![observation(site, branch, observation_epoch)],
            })
            .collect();
        aggregate_glioma_federated_decision_context(&FederatedDecisionContextRequest {
            objective: "prioritize invasion research".into(),
            epoch,
            minimum_sites: 2,
            minimum_independent_groups: 2,
            minimum_quality_milli: 700,
            minimum_branch_support_milli: 700,
            maximum_heterogeneity_milli: 250,
            maximum_influence_milli: 300,
            maximum_branches: 8,
            sites,
        })
        .unwrap()
    }

    fn source_report(epoch: u32, branch: &str) -> FederatedDecisionContextReport {
        source_report_with_observation_epoch(epoch, branch, epoch)
    }

    fn request(
        source_reports: Vec<FederatedDecisionContextReport>,
    ) -> FederatedContinualPromotionRequest {
        FederatedContinualPromotionRequest {
            objective: "prioritize invasion research".into(),
            minimum_consecutive_epochs: 3,
            minimum_sites: 2,
            minimum_independent_groups: 2,
            minimum_support_milli: 700,
            maximum_uncertainty_milli: 250,
            maximum_failure_risk_milli: 250,
            maximum_heterogeneity_milli: 250,
            maximum_influence_milli: 300,
            source_reports,
        }
    }

    #[test]
    fn stable_qualified_branch_promotes_only_after_configured_consecutive_epochs() {
        let source_reports = (1..=3)
            .map(|epoch| source_report(epoch, "branch-a"))
            .collect();
        let report = promote_glioma_federated_continual_context(&request(source_reports)).unwrap();
        assert_eq!(
            report.disposition,
            FederatedContinualPromotionDisposition::Promote
        );
        assert_eq!(report.promoted_branch_id.as_deref(), Some("branch-a"));
        assert_eq!(report.stable_epoch_order, vec![1, 2, 3]);
        assert_eq!(report.source_report_digest_order.len(), 3);
        report.validate().unwrap();
    }

    #[test]
    fn a_branch_change_restarts_the_temporal_stability_window() {
        let report = promote_glioma_federated_continual_context(&request(vec![
            source_report(1, "branch-a"),
            source_report(2, "branch-b"),
            source_report(3, "branch-b"),
        ]))
        .unwrap();
        assert_eq!(
            report.disposition,
            FederatedContinualPromotionDisposition::Continue
        );
        assert_eq!(report.promoted_branch_id, None);
        assert_eq!(report.stable_branch_id.as_deref(), Some("branch-b"));
        assert_eq!(report.stable_epoch_order, vec![2, 3]);
    }

    #[test]
    fn insufficient_epochs_remain_continue_without_claiming_stability() {
        let report = promote_glioma_federated_continual_context(&request(vec![
            source_report(1, "branch-a"),
            source_report(2, "branch-a"),
        ]))
        .unwrap();
        assert_eq!(
            report.disposition,
            FederatedContinualPromotionDisposition::Continue
        );
        assert_eq!(report.stable_epoch_count, 2);
        assert_eq!(report.promoted_branch_id, None);
    }

    #[test]
    fn replay_rejects_a_restamped_but_altered_promotion_decision() {
        let mut report = promote_glioma_federated_continual_context(&request(
            (1..=3)
                .map(|epoch| source_report(epoch, "branch-a"))
                .collect(),
        ))
        .unwrap();
        report.promoted_branch_id = Some("branch-b".into());
        report.digest = ContentHash::of_value(&digest_input(&report)).unwrap();
        assert!(matches!(
            report.validate(),
            Err(FederatedContinualPromotionError::InvalidOutput(_))
        ));
    }

    #[test]
    fn duplicate_observation_identity_cannot_create_an_extra_epoch() {
        let first = source_report(1, "branch-a");
        let second = source_report_with_observation_epoch(2, "branch-a", 1);
        assert!(matches!(
            promote_glioma_federated_continual_context(&request(vec![first, second])),
            Err(FederatedContinualPromotionError::InvalidRequest(_))
        ));
    }
}
