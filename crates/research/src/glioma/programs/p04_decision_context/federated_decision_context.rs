//! Federated continual decision-context aggregation for preclinical glioma research.
//!
//! Each institution computes its branch plan locally. This feature combines only typed,
//! aggregate branch summaries: raw observations, specimen data, and institution-local payloads
//! never cross the boundary. The algorithm applies quorum, independent-group, support, quality,
//! heterogeneity, and leave-one-site-out influence gates before a branch can be promoted. Negative,
//! contradicted, failed, and unknown site outcomes remain visible and cannot be converted into a
//! confident plan by absence of evidence.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P04-F04";
pub const OUTPUT_SCHEMA: &str = "GliomaFederatedDecisionContext1@1";
pub const MAX_SITES: usize = 256;
pub const MAX_BRANCHES: usize = 256;
pub const MAX_OBSERVATIONS: usize = 32_768;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedBranchOutcome {
    Qualified,
    Negative,
    Contradicted,
    Unknown,
    Failed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedBranchDisposition {
    Qualified,
    Underpowered,
    Heterogeneous,
    Rejected,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedDecisionDisposition {
    Promote,
    Continue,
    Hold,
    Reject,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedDecisionBranchObservation {
    pub observation_id: String,
    pub branch_id: String,
    pub outcome: FederatedBranchOutcome,
    pub expected_value_milli: i32,
    pub worst_case_value_milli: i32,
    pub uncertainty_milli: u16,
    pub failure_risk_milli: u16,
    pub support_milli: u16,
    pub cost_units: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedDecisionSiteSummary {
    pub site_id: String,
    pub independent_group: String,
    pub plan_digest: ContentHash,
    pub quality_milli: u16,
    pub local_only: bool,
    pub aggregate_only: bool,
    pub policy_allowed: bool,
    pub observations: Vec<FederatedDecisionBranchObservation>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedDecisionContextRequest {
    pub objective: String,
    pub epoch: u32,
    pub minimum_sites: usize,
    pub minimum_independent_groups: usize,
    pub minimum_quality_milli: u16,
    pub minimum_branch_support_milli: u16,
    pub maximum_heterogeneity_milli: u16,
    pub maximum_influence_milli: u16,
    pub maximum_branches: usize,
    pub sites: Vec<FederatedDecisionSiteSummary>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedDecisionBranchSummary {
    pub branch_id: String,
    pub observation_order: Vec<String>,
    pub eligible_site_order: Vec<String>,
    pub independent_group_count: usize,
    pub qualified_count: usize,
    pub support_milli: u16,
    pub expected_value_milli: i32,
    pub worst_case_value_milli: i32,
    pub uncertainty_milli: u16,
    pub failure_risk_milli: u16,
    pub heterogeneity_milli: u16,
    pub maximum_influence_milli: u16,
    pub negative_order: Vec<String>,
    pub contradicted_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub failed_order: Vec<String>,
    pub disposition: FederatedBranchDisposition,
    pub robust_score_milli: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedDecisionContextReport {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub epoch: u32,
    pub site_order: Vec<String>,
    pub eligible_site_order: Vec<String>,
    pub omitted_site_order: Vec<String>,
    pub branch_order: Vec<String>,
    pub frontier_order: Vec<String>,
    pub selected_branch_id: Option<String>,
    pub branches: Vec<FederatedDecisionBranchSummary>,
    pub omissions: BTreeMap<String, String>,
    pub negative_evidence_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub disposition: FederatedDecisionDisposition,
    pub next_route: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedDecisionContextError {
    #[error("federated decision-context request is invalid: {0}")]
    InvalidRequest(String),
    #[error("federated decision-context output is invalid: {0}")]
    InvalidOutput(String),
    #[error("federated decision-context digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn ranked_unique(values: &[String]) -> bool {
    let mut seen = BTreeSet::new();
    values
        .iter()
        .all(|value| !value.trim().is_empty() && seen.insert(value))
}

fn digest_input(output: &FederatedDecisionContextReport) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "epoch": output.epoch,
        "site_order": output.site_order,
        "eligible_site_order": output.eligible_site_order,
        "omitted_site_order": output.omitted_site_order,
        "branch_order": output.branch_order,
        "frontier_order": output.frontier_order,
        "selected_branch_id": output.selected_branch_id,
        "branches": output.branches,
        "omissions": output.omissions,
        "negative_evidence_order": output.negative_evidence_order,
        "uncertainty_order": output.uncertainty_order,
        "disposition": output.disposition,
        "next_route": output.next_route,
    })
}

impl FederatedDecisionContextReport {
    pub fn validate(&self) -> Result<(), FederatedDecisionContextError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.epoch == 0
            || !canonical(&self.site_order)
            || !canonical(&self.eligible_site_order)
            || !canonical(&self.omitted_site_order)
            || !canonical(&self.branch_order)
            || !ranked_unique(&self.frontier_order)
            || !canonical(&self.negative_evidence_order)
            || !canonical(&self.uncertainty_order)
            || self.branches.len() != self.branch_order.len()
            || self
                .branches
                .windows(2)
                .any(|pair| pair[0].branch_id >= pair[1].branch_id)
            || self.next_route.trim().is_empty()
            || self.digest.as_str().len() != 64
            || self
                .selected_branch_id
                .as_ref()
                .is_some_and(|branch| !self.branch_order.binary_search(branch).is_ok())
        {
            return Err(FederatedDecisionContextError::InvalidOutput(
                "identity, ordering, branch partition, route, or digest fields are invalid".into(),
            ));
        }
        let site_set = self.site_order.iter().cloned().collect::<BTreeSet<_>>();
        let eligible_set = self
            .eligible_site_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let omitted_set = self
            .omitted_site_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        if eligible_set.intersection(&omitted_set).next().is_some()
            || eligible_set
                .union(&omitted_set)
                .cloned()
                .collect::<BTreeSet<_>>()
                != site_set
            || self.branches.iter().any(|branch| {
                branch.branch_id.trim().is_empty()
                    || !canonical(&branch.observation_order)
                    || !canonical(&branch.eligible_site_order)
                    || !canonical(&branch.negative_order)
                    || !canonical(&branch.contradicted_order)
                    || !canonical(&branch.unknown_order)
                    || !canonical(&branch.failed_order)
                    || branch.qualified_count > branch.observation_order.len()
                    || branch.support_milli > 1_000
                    || branch.uncertainty_milli > 1_000
                    || branch.failure_risk_milli > 1_000
                    || branch.heterogeneity_milli > 1_000
                    || branch.maximum_influence_milli > 1_000
            })
        {
            return Err(FederatedDecisionContextError::InvalidOutput(
                "site partition or branch metric invariants are inconsistent".into(),
            ));
        }
        let branch_set = self.branch_order.iter().cloned().collect::<BTreeSet<_>>();
        if self
            .frontier_order
            .iter()
            .any(|branch| !branch_set.contains(branch))
        {
            return Err(FederatedDecisionContextError::InvalidOutput(
                "frontier references an unknown branch".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| FederatedDecisionContextError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(FederatedDecisionContextError::Digest(
                "federated decision-context digest does not match canonical content".into(),
            ));
        }
        Ok(())
    }
}

fn weighted_mean(values: &[(i32, u64)]) -> i32 {
    if values.is_empty() {
        return 0;
    }
    let weight = values
        .iter()
        .map(|(_, weight)| *weight as u128)
        .sum::<u128>();
    if weight == 0 {
        return values
            .iter()
            .map(|(value, _)| i64::from(*value))
            .sum::<i64>()
            .saturating_div(values.len() as i64) as i32;
    }
    let total = values
        .iter()
        .map(|(value, weight)| i128::from(*value) * i128::from(*weight as i64))
        .sum::<i128>();
    (total / i128::from(weight as i64)) as i32
}

fn scaled_deviation(value: i32, mean: i32) -> u16 {
    let scale = i64::from(mean.unsigned_abs().max(1));
    (i64::from(value.saturating_sub(mean)).unsigned_abs() as u64 * 1_000 / scale as u64).min(1_000)
        as u16
}

fn route_for(disposition: FederatedDecisionDisposition) -> &'static str {
    match disposition {
        FederatedDecisionDisposition::Promote => "glioma_decision_operating_cycle",
        FederatedDecisionDisposition::Continue => "glioma_federated_decision_context",
        FederatedDecisionDisposition::Hold => "glioma_researcher_workbench",
        FederatedDecisionDisposition::Reject => "glioma_decision_branch_plan",
    }
}

/// Aggregate site-local branch plans into a robust, executable decision frontier.
pub fn aggregate_glioma_federated_decision_context(
    request: &FederatedDecisionContextRequest,
) -> Result<FederatedDecisionContextReport, FederatedDecisionContextError> {
    if request.objective.trim().is_empty()
        || request.epoch == 0
        || request.minimum_sites == 0
        || request.minimum_independent_groups == 0
        || request.minimum_quality_milli > 1_000
        || request.minimum_branch_support_milli > 1_000
        || request.maximum_heterogeneity_milli > 1_000
        || request.maximum_influence_milli > 1_000
        || request.maximum_branches == 0
        || request.maximum_branches > MAX_BRANCHES
        || request.sites.is_empty()
        || request.sites.len() > MAX_SITES
    {
        return Err(FederatedDecisionContextError::InvalidRequest(
            "objective, quorum, quality, robustness, branch, and site bounds are invalid".into(),
        ));
    }
    let mut sites = request.sites.clone();
    sites.sort_by(|left, right| left.site_id.cmp(&right.site_id));
    if sites
        .windows(2)
        .any(|pair| pair[0].site_id == pair[1].site_id)
    {
        return Err(FederatedDecisionContextError::InvalidRequest(
            "site identifiers must be unique".into(),
        ));
    }
    let total_observations = sites
        .iter()
        .map(|site| site.observations.len())
        .sum::<usize>();
    if total_observations > MAX_OBSERVATIONS
        || sites.iter().any(|site| {
            site.site_id.trim().is_empty()
                || site.independent_group.trim().is_empty()
                || site.plan_digest.as_str().len() != 64
                || site.quality_milli > 1_000
                || site.observations.iter().any(|observation| {
                    observation.observation_id.trim().is_empty()
                        || observation.branch_id.trim().is_empty()
                        || observation.uncertainty_milli > 1_000
                        || observation.failure_risk_milli > 1_000
                        || observation.support_milli > 1_000
                        || observation.cost_units == 0
                })
                || {
                    let mut branch_ids = BTreeSet::new();
                    site.observations
                        .iter()
                        .any(|observation| !branch_ids.insert(observation.branch_id.clone()))
                }
        })
    {
        return Err(FederatedDecisionContextError::InvalidRequest(
            "site, plan digest, observation, quality, and cost fields are invalid".into(),
        ));
    }
    let mut observations_seen = BTreeSet::new();
    if sites.iter().any(|site| {
        site.observations
            .iter()
            .any(|observation| !observations_seen.insert(observation.observation_id.clone()))
    }) {
        return Err(FederatedDecisionContextError::InvalidRequest(
            "observation identifiers must be globally unique".into(),
        ));
    }

    let site_order = sites
        .iter()
        .map(|site| site.site_id.clone())
        .collect::<Vec<_>>();
    let mut omissions = BTreeMap::new();
    let mut eligible_sites = Vec::new();
    for site in &sites {
        let reason = if !site.local_only {
            Some("raw_data_not_local")
        } else if !site.aggregate_only {
            Some("site_export_is_not_aggregate_only")
        } else if !site.policy_allowed {
            Some("site_policy_denied")
        } else if site.quality_milli < request.minimum_quality_milli {
            Some("site_quality_below_threshold")
        } else if site.observations.is_empty() {
            Some("site_has_no_branch_observations")
        } else {
            None
        };
        if let Some(reason) = reason {
            omissions.insert(site.site_id.clone(), reason.into());
        } else {
            eligible_sites.push(site);
        }
    }
    let eligible_site_order = eligible_sites
        .iter()
        .map(|site| site.site_id.clone())
        .collect::<Vec<_>>();
    let omitted_site_order = omissions.keys().cloned().collect::<Vec<_>>();

    let mut branch_ids = eligible_sites
        .iter()
        .flat_map(|site| {
            site.observations
                .iter()
                .map(|observation| observation.branch_id.clone())
        })
        .collect::<BTreeSet<_>>();
    if branch_ids.len() > request.maximum_branches {
        let mut retained = branch_ids.into_iter().collect::<Vec<_>>();
        retained.truncate(request.maximum_branches);
        branch_ids = retained.into_iter().collect();
        for site in &eligible_sites {
            for observation in &site.observations {
                if !branch_ids.contains(&observation.branch_id) {
                    omissions.insert(
                        format!("{}::{}", site.site_id, observation.observation_id),
                        "branch_capacity_exceeded".into(),
                    );
                }
            }
        }
    }
    let branch_order = branch_ids.iter().cloned().collect::<Vec<_>>();
    let mut branches = Vec::with_capacity(branch_order.len());
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for branch_id in &branch_order {
        let mut branch_observations = Vec::new();
        let mut negative_order = Vec::new();
        let mut contradicted_order = Vec::new();
        let mut unknown_order = Vec::new();
        let mut failed_order = Vec::new();
        let mut eligible_branch_sites = BTreeSet::new();
        let mut groups = BTreeSet::new();
        for site in &eligible_sites {
            if let Some(observation) = site
                .observations
                .iter()
                .find(|observation| &observation.branch_id == branch_id)
            {
                eligible_branch_sites.insert(site.site_id.clone());
                groups.insert(site.independent_group.clone());
                let weight = u64::from(site.quality_milli.max(1))
                    .saturating_mul(u64::from(observation.support_milli.max(1)));
                branch_observations.push((site, observation, weight));
                match observation.outcome {
                    FederatedBranchOutcome::Negative => {
                        negative_order.push(observation.observation_id.clone())
                    }
                    FederatedBranchOutcome::Contradicted => {
                        contradicted_order.push(observation.observation_id.clone())
                    }
                    FederatedBranchOutcome::Unknown => {
                        unknown_order.push(observation.observation_id.clone())
                    }
                    FederatedBranchOutcome::Failed => {
                        failed_order.push(observation.observation_id.clone())
                    }
                    FederatedBranchOutcome::Qualified => {}
                }
            }
        }
        branch_observations
            .sort_by(|left, right| left.1.observation_id.cmp(&right.1.observation_id));
        negative_order.sort();
        contradicted_order.sort();
        unknown_order.sort();
        failed_order.sort();
        let qualified = branch_observations
            .iter()
            .filter(|(_, observation, _)| observation.outcome == FederatedBranchOutcome::Qualified)
            .collect::<Vec<_>>();
        let qualified_count = qualified.len();
        let support_milli = if branch_observations.is_empty() {
            0
        } else {
            ((qualified_count as u64 * 1_000) / branch_observations.len() as u64).min(1_000) as u16
        };
        let expected_value_milli = weighted_mean(
            &qualified
                .iter()
                .map(|(_, observation, weight)| (observation.expected_value_milli, *weight))
                .collect::<Vec<_>>(),
        );
        let worst_case_value_milli = weighted_mean(
            &qualified
                .iter()
                .map(|(_, observation, weight)| (observation.worst_case_value_milli, *weight))
                .collect::<Vec<_>>(),
        );
        let uncertainty_milli = if qualified.is_empty() {
            1_000
        } else {
            weighted_mean(
                &qualified
                    .iter()
                    .map(|(_, observation, weight)| {
                        (i32::from(observation.uncertainty_milli), *weight)
                    })
                    .collect::<Vec<_>>(),
            )
            .clamp(0, 1_000) as u16
        };
        let failure_risk_milli = if qualified.is_empty() {
            1_000
        } else {
            weighted_mean(
                &qualified
                    .iter()
                    .map(|(_, observation, weight)| {
                        (i32::from(observation.failure_risk_milli), *weight)
                    })
                    .collect::<Vec<_>>(),
            )
            .clamp(0, 1_000) as u16
        };
        let heterogeneity_milli = qualified
            .iter()
            .map(|(_, observation, _)| {
                scaled_deviation(observation.expected_value_milli, expected_value_milli)
            })
            .max()
            .unwrap_or(1_000);
        let maximum_influence_milli = if qualified.len() < 2 {
            0
        } else {
            let full_values = qualified
                .iter()
                .map(|(_, observation, weight)| (observation.expected_value_milli, *weight))
                .collect::<Vec<_>>();
            qualified
                .iter()
                .enumerate()
                .map(|(index, _)| {
                    let leave_one_out = full_values
                        .iter()
                        .enumerate()
                        .filter_map(|(candidate_index, value)| {
                            (candidate_index != index).then_some(*value)
                        })
                        .collect::<Vec<_>>();
                    scaled_deviation(weighted_mean(&leave_one_out), expected_value_milli)
                })
                .max()
                .unwrap_or(0)
        };
        let disposition = if !negative_order.is_empty()
            || !contradicted_order.is_empty()
            || !failed_order.is_empty()
        {
            FederatedBranchDisposition::Rejected
        } else if qualified_count < request.minimum_sites
            || groups.len() < request.minimum_independent_groups
            || support_milli < request.minimum_branch_support_milli
        {
            FederatedBranchDisposition::Underpowered
        } else if heterogeneity_milli > request.maximum_heterogeneity_milli
            || maximum_influence_milli > request.maximum_influence_milli
        {
            FederatedBranchDisposition::Heterogeneous
        } else {
            FederatedBranchDisposition::Qualified
        };
        let robust_score_milli = i64::from(worst_case_value_milli)
            .saturating_sub(i64::from(uncertainty_milli))
            .saturating_sub(i64::from(failure_risk_milli))
            .saturating_sub(i64::from(heterogeneity_milli))
            .saturating_sub(i64::from(maximum_influence_milli));
        if !negative_order.is_empty() || !contradicted_order.is_empty() || !failed_order.is_empty()
        {
            negative_evidence.extend(
                negative_order
                    .iter()
                    .map(|id| format!("{branch_id}:{id}:negative")),
            );
            negative_evidence.extend(
                contradicted_order
                    .iter()
                    .map(|id| format!("{branch_id}:{id}:contradicted")),
            );
            negative_evidence.extend(
                failed_order
                    .iter()
                    .map(|id| format!("{branch_id}:{id}:failed")),
            );
        }
        if !unknown_order.is_empty() || disposition != FederatedBranchDisposition::Qualified {
            uncertainty.extend(
                unknown_order
                    .iter()
                    .map(|id| format!("{branch_id}:{id}:unknown")),
            );
            uncertainty.insert(format!("{branch_id}:branch-gate-unresolved"));
        }
        branches.push(FederatedDecisionBranchSummary {
            branch_id: branch_id.clone(),
            observation_order: branch_observations
                .iter()
                .map(|(_, observation, _)| observation.observation_id.clone())
                .collect(),
            eligible_site_order: eligible_branch_sites.into_iter().collect(),
            independent_group_count: groups.len(),
            qualified_count,
            support_milli,
            expected_value_milli,
            worst_case_value_milli,
            uncertainty_milli,
            failure_risk_milli,
            heterogeneity_milli,
            maximum_influence_milli,
            negative_order,
            contradicted_order,
            unknown_order,
            failed_order,
            disposition,
            robust_score_milli,
        });
    }
    branches.sort_by(|left, right| left.branch_id.cmp(&right.branch_id));
    let mut frontier = branches
        .iter()
        .filter(|branch| branch.disposition == FederatedBranchDisposition::Qualified)
        .map(|branch| (branch.branch_id.clone(), branch.robust_score_milli))
        .collect::<Vec<_>>();
    frontier.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    let frontier_order = frontier
        .iter()
        .map(|(branch, _)| branch.clone())
        .collect::<Vec<_>>();
    let selected_branch_id = frontier.first().map(|(branch, _)| branch.clone());
    let disposition = if selected_branch_id.is_some() {
        FederatedDecisionDisposition::Promote
    } else if branches
        .iter()
        .all(|branch| branch.disposition == FederatedBranchDisposition::Rejected)
        && !branches.is_empty()
    {
        FederatedDecisionDisposition::Reject
    } else if branches
        .iter()
        .any(|branch| branch.disposition == FederatedBranchDisposition::Underpowered)
        || !uncertainty.is_empty()
    {
        FederatedDecisionDisposition::Continue
    } else {
        FederatedDecisionDisposition::Hold
    };
    let mut negative_evidence_order = negative_evidence.into_iter().collect::<Vec<_>>();
    negative_evidence_order.sort();
    let mut uncertainty_order = uncertainty.into_iter().collect::<Vec<_>>();
    uncertainty_order.sort();
    let next_route = route_for(disposition).into();
    let mut report = FederatedDecisionContextReport {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        epoch: request.epoch,
        site_order,
        eligible_site_order,
        omitted_site_order,
        branch_order,
        frontier_order,
        selected_branch_id,
        branches,
        omissions,
        negative_evidence_order,
        uncertainty_order,
        disposition,
        next_route,
        digest: ContentHash::of_bytes(b"unsealed-glioma-federated-decision-context"),
    };
    report.digest = ContentHash::of_value(&digest_input(&report))
        .map_err(|error| FederatedDecisionContextError::Digest(error.to_string()))?;
    report.validate()?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(seed: &str) -> ContentHash {
        ContentHash::of_bytes(seed.as_bytes())
    }

    fn observation(
        site: &str,
        branch: &str,
        outcome: FederatedBranchOutcome,
    ) -> FederatedDecisionBranchObservation {
        FederatedDecisionBranchObservation {
            observation_id: format!("{site}-{branch}"),
            branch_id: branch.into(),
            outcome,
            expected_value_milli: 800,
            worst_case_value_milli: 600,
            uncertainty_milli: 100,
            failure_risk_milli: 100,
            support_milli: 900,
            cost_units: 2,
        }
    }

    fn site(
        id: &str,
        group: &str,
        observations: Vec<FederatedDecisionBranchObservation>,
    ) -> FederatedDecisionSiteSummary {
        FederatedDecisionSiteSummary {
            site_id: id.into(),
            independent_group: group.into(),
            plan_digest: hash(id),
            quality_milli: 900,
            local_only: true,
            aggregate_only: true,
            policy_allowed: true,
            observations,
        }
    }

    fn request(sites: Vec<FederatedDecisionSiteSummary>) -> FederatedDecisionContextRequest {
        FederatedDecisionContextRequest {
            objective: "select invasion research branch".into(),
            epoch: 4,
            minimum_sites: 2,
            minimum_independent_groups: 2,
            minimum_quality_milli: 700,
            minimum_branch_support_milli: 700,
            maximum_heterogeneity_milli: 250,
            maximum_influence_milli: 300,
            maximum_branches: 8,
            sites,
        }
    }

    #[test]
    fn independent_qualified_sites_promote_high_score_branch() {
        let report = aggregate_glioma_federated_decision_context(&request(vec![
            site(
                "site-a",
                "group-a",
                vec![observation(
                    "site-a",
                    "branch-1",
                    FederatedBranchOutcome::Qualified,
                )],
            ),
            site(
                "site-b",
                "group-b",
                vec![observation(
                    "site-b",
                    "branch-1",
                    FederatedBranchOutcome::Qualified,
                )],
            ),
        ]))
        .unwrap();
        assert_eq!(report.disposition, FederatedDecisionDisposition::Promote);
        assert_eq!(report.selected_branch_id.as_deref(), Some("branch-1"));
        report.validate().unwrap();
    }

    #[test]
    fn denied_site_is_omitted_and_quorum_continues() {
        let mut denied = site(
            "site-b",
            "group-b",
            vec![observation(
                "site-b",
                "branch-1",
                FederatedBranchOutcome::Qualified,
            )],
        );
        denied.policy_allowed = false;
        let report = aggregate_glioma_federated_decision_context(&request(vec![
            site(
                "site-a",
                "group-a",
                vec![observation(
                    "site-a",
                    "branch-1",
                    FederatedBranchOutcome::Qualified,
                )],
            ),
            denied,
        ]))
        .unwrap();
        assert_eq!(report.disposition, FederatedDecisionDisposition::Continue);
        assert_eq!(report.omissions["site-b"], "site_policy_denied");
    }

    #[test]
    fn contradiction_is_retained_and_rejects_branch() {
        let report = aggregate_glioma_federated_decision_context(&request(vec![
            site(
                "site-a",
                "group-a",
                vec![observation(
                    "site-a",
                    "branch-1",
                    FederatedBranchOutcome::Contradicted,
                )],
            ),
            site(
                "site-b",
                "group-b",
                vec![observation(
                    "site-b",
                    "branch-1",
                    FederatedBranchOutcome::Contradicted,
                )],
            ),
        ]))
        .unwrap();
        assert_eq!(report.disposition, FederatedDecisionDisposition::Reject);
        assert_eq!(
            report.branches[0].disposition,
            FederatedBranchDisposition::Rejected
        );
        assert!(report.branches[0]
            .contradicted_order
            .contains(&"site-a-branch-1".into()));
        assert!(report
            .negative_evidence_order
            .iter()
            .any(|value| value.contains("contradicted")));
    }

    #[test]
    fn frontier_order_is_score_ranked_and_replayable() {
        let mut high = observation("site-a", "branch-a", FederatedBranchOutcome::Qualified);
        high.expected_value_milli = 900;
        high.worst_case_value_milli = 800;
        let mut low = observation("site-a", "branch-b", FederatedBranchOutcome::Qualified);
        low.expected_value_milli = 500;
        low.worst_case_value_milli = 300;
        let report = aggregate_glioma_federated_decision_context(&request(vec![
            site("site-a", "group-a", vec![high.clone(), low.clone()]),
            site(
                "site-b",
                "group-b",
                vec![
                    observation("site-b", "branch-a", FederatedBranchOutcome::Qualified),
                    observation("site-b", "branch-b", FederatedBranchOutcome::Qualified),
                ],
            ),
        ]))
        .unwrap();
        assert_eq!(
            report.frontier_order.first().map(String::as_str),
            Some("branch-a")
        );
        let first = report.clone();
        let second = aggregate_glioma_federated_decision_context(&request(vec![
            site(
                "site-b",
                "group-b",
                vec![
                    observation("site-b", "branch-b", FederatedBranchOutcome::Qualified),
                    observation("site-b", "branch-a", FederatedBranchOutcome::Qualified),
                ],
            ),
            site("site-a", "group-a", vec![low, high]),
        ]))
        .unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn duplicate_branch_observation_is_rejected_instead_of_silently_dropped() {
        let duplicate = observation("site-a", "branch-1", FederatedBranchOutcome::Qualified);
        let error = aggregate_glioma_federated_decision_context(&request(vec![
            site("site-a", "group-a", vec![duplicate.clone(), duplicate]),
            site(
                "site-b",
                "group-b",
                vec![observation(
                    "site-b",
                    "branch-1",
                    FederatedBranchOutcome::Qualified,
                )],
            ),
        ]))
        .unwrap_err();
        assert!(matches!(
            error,
            FederatedDecisionContextError::InvalidRequest(_)
        ));
    }
}
