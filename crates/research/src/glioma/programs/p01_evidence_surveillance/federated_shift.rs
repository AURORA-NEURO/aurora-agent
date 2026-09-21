//! Aggregate-only federated consensus over prospective preclinical glioma evidence shifts.
//!
//! Local temporal-shift detectors answer whether one institution's claim moved. This capability
//! answers the harder continual-surveillance question: did the same claim move across independent
//! institutions, or is the apparent change site-specific? Only typed summaries cross the boundary;
//! raw sources, specimen data, human data, and clinical decisions never do.

use crate::glioma_engine::LocalArtifactRef;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P01-F04";
pub const OUTPUT_SCHEMA: &str = "GliomaFederatedEvidenceShift1@1";
pub const MAX_SITES: usize = 256;
pub const MAX_CLAIMS: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedEvidenceShiftSite {
    pub site_id: String,
    pub study_id: String,
    pub evidence_id: String,
    pub baseline_mean_milli: i32,
    pub recent_mean_milli: i32,
    pub baseline_uncertainty_milli: u64,
    pub recent_uncertainty_milli: u64,
    pub baseline_count: u16,
    pub recent_count: u16,
    pub local_heterogeneity_milli: u16,
    pub quality_milli: u16,
    pub artifact: LocalArtifactRef,
    pub preclinical_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedEvidenceShiftRequest {
    pub objective: String,
    pub snapshot_id: String,
    pub minimum_sites: usize,
    pub minimum_consensus_milli: u16,
    pub minimum_change_milli: u64,
    pub max_heterogeneity_milli: u16,
    pub max_leave_one_out_shift_milli: u64,
    pub max_actions: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedEvidenceShiftKind {
    ConsensusEmergence,
    ConsensusReversal,
    SiteSpecific,
    Stable,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedEvidenceShiftAction {
    pub action_id: String,
    pub evidence_id: String,
    pub kind: FederatedEvidenceShiftKind,
    pub site_order: Vec<String>,
    pub pooled_delta_milli: i32,
    pub pooled_uncertainty_milli: u64,
    pub consensus_milli: u16,
    pub heterogeneity_milli: u16,
    pub max_leave_one_out_shift_milli: u64,
    pub priority_milli: u16,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedEvidenceShiftDisposition {
    Qualified,
    Underpowered,
    Heterogeneous,
    InfluenceBlocked,
    SiteSpecific,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedEvidenceShift {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub snapshot_id: String,
    pub claim_order: Vec<String>,
    pub action_order: Vec<String>,
    pub actions: Vec<FederatedEvidenceShiftAction>,
    pub unresolved_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: FederatedEvidenceShiftDisposition,
    pub next_step: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedEvidenceShiftError {
    #[error("federated evidence shift request is invalid: {0}")]
    InvalidRequest(String),
    #[error("federated evidence shift site is invalid: {0}")]
    InvalidSite(String),
    #[error("federated evidence shift output is invalid: {0}")]
    InvalidOutput(String),
    #[error("federated evidence shift digest failed: {0}")]
    Digest(String),
}

#[derive(Debug, Clone, Copy)]
struct PooledShift {
    delta_milli: i32,
    uncertainty_milli: u64,
    consensus_milli: u16,
    heterogeneity_milli: u16,
    max_leave_one_out_shift_milli: u64,
    quality_milli: u16,
    site_count: usize,
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn delta(site: &FederatedEvidenceShiftSite) -> i32 {
    site.recent_mean_milli
        .saturating_sub(site.baseline_mean_milli)
}

fn weight(site: &FederatedEvidenceShiftSite) -> u128 {
    let variance = u128::from(site.baseline_uncertainty_milli)
        .saturating_mul(u128::from(site.baseline_uncertainty_milli))
        .saturating_add(
            u128::from(site.recent_uncertainty_milli)
                .saturating_mul(u128::from(site.recent_uncertainty_milli)),
        )
        .max(1);
    u128::from(site.baseline_count.min(site.recent_count).max(1))
        .saturating_mul(u128::from(site.quality_milli.max(1)))
        .saturating_mul(1_000_000)
        / variance
}

fn integer_sqrt(value: u128) -> u128 {
    if value == 0 {
        return 0;
    }
    let mut low = 0_u128;
    let mut high = value.saturating_add(1);
    while low + 1 < high {
        let mid = low + (high - low) / 2;
        if mid <= value / mid {
            low = mid;
        } else {
            high = mid;
        }
    }
    low
}

fn pooled(sites: &[&FederatedEvidenceShiftSite]) -> PooledShift {
    if sites.is_empty() {
        return PooledShift {
            delta_milli: 0,
            uncertainty_milli: u64::MAX,
            consensus_milli: 0,
            heterogeneity_milli: 1_000,
            max_leave_one_out_shift_milli: 0,
            quality_milli: 0,
            site_count: 0,
        };
    }
    let total_weight = sites.iter().map(|site| weight(site)).sum::<u128>();
    let weighted_delta = if total_weight == 0 {
        0
    } else {
        sites
            .iter()
            .map(|site| i128::from(delta(site)) * weight(site) as i128)
            .sum::<i128>()
            .checked_div(total_weight as i128)
            .unwrap_or(0) as i32
    };
    let uncertainty_milli = if total_weight == 0 {
        u64::MAX
    } else {
        (1_000_000_u128 / integer_sqrt(total_weight).max(1)).min(u128::from(u64::MAX)) as u64
    };
    let mass = sites
        .iter()
        .filter(|site| delta(site).signum() == weighted_delta.signum())
        .map(|site| weight(site))
        .sum::<u128>();
    let consensus_milli = if total_weight == 0 {
        0
    } else {
        (mass.saturating_mul(1_000) / total_weight).min(1_000) as u16
    };
    let max_deviation = sites
        .iter()
        .map(|site| delta(site).abs_diff(weighted_delta))
        .max()
        .unwrap_or(0);
    let heterogeneity_milli = (max_deviation
        .saturating_mul(1_000)
        .checked_div(weighted_delta.unsigned_abs().saturating_add(1))
        .unwrap_or(1_000)
        .min(1_000)) as u16;
    let max_leave_one_out_shift_milli = sites
        .iter()
        .enumerate()
        .map(|(index, _)| {
            let without = sites
                .iter()
                .enumerate()
                .filter_map(|(candidate_index, site)| (candidate_index != index).then_some(*site))
                .collect::<Vec<_>>();
            pooled_without(&without).abs_diff(weighted_delta)
        })
        .max()
        .unwrap_or(0);
    let quality_milli = (sites
        .iter()
        .map(|site| u64::from(site.quality_milli))
        .sum::<u64>()
        / sites.len() as u64) as u16;
    PooledShift {
        delta_milli: weighted_delta,
        uncertainty_milli,
        consensus_milli,
        heterogeneity_milli,
        max_leave_one_out_shift_milli: u64::from(max_leave_one_out_shift_milli),
        quality_milli,
        site_count: sites.len(),
    }
}

fn pooled_without(sites: &[&FederatedEvidenceShiftSite]) -> i32 {
    let total_weight = sites.iter().map(|site| weight(site)).sum::<u128>();
    if total_weight == 0 {
        return 0;
    }
    sites
        .iter()
        .map(|site| i128::from(delta(site)) * weight(site) as i128)
        .sum::<i128>()
        .checked_div(total_weight as i128)
        .unwrap_or(0) as i32
}

fn priority(summary: PooledShift) -> u16 {
    (summary
        .delta_milli
        .unsigned_abs()
        .min(2_000)
        .saturating_mul(u32::from(summary.consensus_milli))
        .saturating_mul(u32::from(summary.quality_milli))
        .saturating_mul(summary.site_count.min(u16::MAX as usize) as u32)
        .saturating_mul(
            1_000_u32
                .saturating_sub(u32::from(summary.heterogeneity_milli))
                .max(1),
        )
        / 2_000_000_000)
        .min(1_000) as u16
}

fn digest_input(output: &FederatedEvidenceShift) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "snapshot_id": output.snapshot_id,
        "claim_order": output.claim_order,
        "action_order": output.action_order,
        "actions": output.actions,
        "unresolved_order": output.unresolved_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
        "next_step": output.next_step,
    })
}

impl FederatedEvidenceShift {
    pub fn validate(&self) -> Result<(), FederatedEvidenceShiftError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.snapshot_id.trim().is_empty()
            || !canonical(&self.claim_order)
            || !canonical(&self.unresolved_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.actions.len() > MAX_CLAIMS
            || self.actions.iter().any(|action| {
                action.action_id != format!("federated-temporal:{}", action.evidence_id)
                    || !canonical(&action.site_order)
                    || action.consensus_milli > 1_000
                    || action.heterogeneity_milli > 1_000
                    || action.priority_milli > 1_000
                    || action.site_order.is_empty()
                    || action.rationale.trim().is_empty()
            })
            || self.actions.windows(2).any(|pair| {
                pair[0].priority_milli < pair[1].priority_milli
                    || (pair[0].priority_milli == pair[1].priority_milli
                        && pair[0].action_id > pair[1].action_id)
            })
        {
            return Err(FederatedEvidenceShiftError::InvalidOutput(
                "identity, ordering, bounds, or action state is invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| FederatedEvidenceShiftError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(FederatedEvidenceShiftError::InvalidOutput(
                "federated temporal-shift digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

pub fn analyze_federated_evidence_shifts(
    request: &FederatedEvidenceShiftRequest,
    sites: &[FederatedEvidenceShiftSite],
) -> Result<FederatedEvidenceShift, FederatedEvidenceShiftError> {
    if request.objective.trim().is_empty()
        || request.snapshot_id.trim().is_empty()
        || request.minimum_sites == 0
        || request.minimum_sites > MAX_SITES
        || request.minimum_consensus_milli > 1_000
        || request.minimum_change_milli > 2_000
        || request.max_heterogeneity_milli > 1_000
        || request.max_actions == 0
        || request.max_actions > MAX_CLAIMS
    {
        return Err(FederatedEvidenceShiftError::InvalidRequest(
            "objective, snapshot, site/consensus/change gates, and bounded action capacity are required".into(),
        ));
    }
    if sites.is_empty() || sites.len() > MAX_SITES {
        return Err(FederatedEvidenceShiftError::InvalidSite(
            "site count is outside the supported bound".into(),
        ));
    }
    let mut site_keys = BTreeSet::new();
    let mut grouped = BTreeMap::<String, Vec<&FederatedEvidenceShiftSite>>::new();
    for site in sites {
        site.artifact
            .validate()
            .map_err(|error| FederatedEvidenceShiftError::InvalidSite(error.to_string()))?;
        if site.site_id.trim().is_empty()
            || site.study_id.trim().is_empty()
            || site.evidence_id.trim().is_empty()
            || site.baseline_mean_milli.abs() > 1_000
            || site.recent_mean_milli.abs() > 1_000
            || site.baseline_uncertainty_milli == 0
            || site.recent_uncertainty_milli == 0
            || site.baseline_count == 0
            || site.recent_count == 0
            || site.local_heterogeneity_milli > 1_000
            || site.quality_milli > 1_000
            || !site.preclinical_only
            || site.artifact.contains_human_data
            || site.artifact.contains_direct_identifiers
            || !site_keys.insert((site.site_id.clone(), site.evidence_id.clone()))
        {
            return Err(FederatedEvidenceShiftError::InvalidSite(
                "identity, score/uncertainty, replicate, privacy, preclinical, or uniqueness declaration is invalid".into(),
            ));
        }
        grouped
            .entry(site.evidence_id.clone())
            .or_default()
            .push(site);
    }
    if grouped.len() > MAX_CLAIMS {
        return Err(FederatedEvidenceShiftError::InvalidSite(
            "claim count exceeds the supported bound".into(),
        ));
    }
    let claim_order = grouped.keys().cloned().collect::<Vec<_>>();
    let mut actions = Vec::new();
    let mut unresolved_order = Vec::new();
    let mut negative_evidence = Vec::new();
    let mut uncertainty = Vec::new();
    for (evidence_id, claim_sites) in grouped {
        let summary = pooled(&claim_sites);
        let site_order = claim_sites
            .iter()
            .map(|site| site.site_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let priority_milli = priority(summary);
        if summary.site_count < request.minimum_sites {
            unresolved_order.push(evidence_id.clone());
            uncertainty.push(format!(
                "{evidence_id}: federated site quorum is incomplete"
            ));
            continue;
        }
        let sign_consensus = summary.consensus_milli >= request.minimum_consensus_milli;
        let change_clear =
            u64::from(summary.delta_milli.unsigned_abs()) >= request.minimum_change_milli;
        let kind = if !change_clear {
            FederatedEvidenceShiftKind::Stable
        } else if !sign_consensus {
            FederatedEvidenceShiftKind::SiteSpecific
        } else if claim_sites
            .iter()
            .any(|site| site.baseline_mean_milli.signum() != 0)
            && claim_sites
                .iter()
                .any(|site| site.recent_mean_milli.signum() != 0)
            && claim_sites
                .iter()
                .all(|site| site.baseline_mean_milli.signum() != site.recent_mean_milli.signum())
        {
            FederatedEvidenceShiftKind::ConsensusReversal
        } else {
            FederatedEvidenceShiftKind::ConsensusEmergence
        };
        match kind {
            FederatedEvidenceShiftKind::Stable => {
                negative_evidence.push(format!("{evidence_id}: no consortium-wide material shift"));
            }
            FederatedEvidenceShiftKind::SiteSpecific => {
                negative_evidence.push(format!("{evidence_id}: shift is not consensus-supported"));
                uncertainty.push(format!("{evidence_id}: institution-local effects disagree"));
            }
            FederatedEvidenceShiftKind::ConsensusEmergence
            | FederatedEvidenceShiftKind::ConsensusReversal
                if summary.heterogeneity_milli <= request.max_heterogeneity_milli
                    && summary.max_leave_one_out_shift_milli
                        <= request.max_leave_one_out_shift_milli
                    && priority_milli > 0
                    && actions.len() < request.max_actions =>
            {
                let rationale = if matches!(kind, FederatedEvidenceShiftKind::ConsensusReversal) {
                    "independent sites report the same direction reversal; route competing explanations and replication review"
                } else {
                    "independent sites report a concordant change; refresh the typed claim and replan downstream mechanism work"
                };
                actions.push(FederatedEvidenceShiftAction {
                    action_id: format!("federated-temporal:{evidence_id}"),
                    evidence_id: evidence_id.clone(),
                    kind,
                    site_order,
                    pooled_delta_milli: summary.delta_milli,
                    pooled_uncertainty_milli: summary.uncertainty_milli,
                    consensus_milli: summary.consensus_milli,
                    heterogeneity_milli: summary.heterogeneity_milli,
                    max_leave_one_out_shift_milli: summary.max_leave_one_out_shift_milli,
                    priority_milli,
                    rationale: rationale.into(),
                });
            }
            FederatedEvidenceShiftKind::ConsensusEmergence
            | FederatedEvidenceShiftKind::ConsensusReversal => {
                negative_evidence.push(format!(
                    "{evidence_id}: consortium shift failed heterogeneity, influence, or priority gates"
                ));
            }
            FederatedEvidenceShiftKind::Unresolved => unreachable!(),
        }
        if summary.uncertainty_milli > request.minimum_change_milli {
            uncertainty.push(format!(
                "{evidence_id}: pooled uncertainty exceeds the requested change scale"
            ));
        }
    }
    actions.sort_by(|left, right| {
        right
            .priority_milli
            .cmp(&left.priority_milli)
            .then_with(|| left.action_id.cmp(&right.action_id))
    });
    let action_order = actions
        .iter()
        .map(|action| action.action_id.clone())
        .collect::<Vec<_>>();
    unresolved_order.sort();
    negative_evidence.sort();
    negative_evidence.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    let disposition = if claim_order.is_empty() {
        FederatedEvidenceShiftDisposition::Unresolved
    } else if !actions.is_empty() {
        FederatedEvidenceShiftDisposition::Qualified
    } else if !unresolved_order.is_empty() {
        FederatedEvidenceShiftDisposition::Underpowered
    } else if negative_evidence
        .iter()
        .any(|entry| entry.contains("heterogeneity"))
    {
        FederatedEvidenceShiftDisposition::Heterogeneous
    } else if negative_evidence
        .iter()
        .any(|entry| entry.contains("influence"))
    {
        FederatedEvidenceShiftDisposition::InfluenceBlocked
    } else if negative_evidence
        .iter()
        .any(|entry| entry.contains("not consensus"))
    {
        FederatedEvidenceShiftDisposition::SiteSpecific
    } else {
        FederatedEvidenceShiftDisposition::Unresolved
    };
    let next_step = match disposition {
        FederatedEvidenceShiftDisposition::Qualified => {
            "route consensus shift actions to typed-knowledge refresh, mechanism review, or replication planning"
        }
        FederatedEvidenceShiftDisposition::Underpowered => {
            "recruit independent aggregate site summaries before promoting the shift"
        }
        FederatedEvidenceShiftDisposition::Heterogeneous => {
            "retain site disagreement and investigate protocol, model, or measurement differences"
        }
        FederatedEvidenceShiftDisposition::InfluenceBlocked => {
            "retain the result as influence-sensitive and obtain additional independent sites"
        }
        FederatedEvidenceShiftDisposition::SiteSpecific => {
            "keep the local signal scoped to its originating site; do not generalize it"
        }
        FederatedEvidenceShiftDisposition::Unresolved => {
            "do not promote a consortium trend until the evidence windows and gates resolve"
        }
    };
    let mut output = FederatedEvidenceShift {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        snapshot_id: request.snapshot_id.clone(),
        claim_order,
        action_order,
        actions,
        unresolved_order,
        negative_evidence,
        uncertainty,
        disposition,
        next_step: next_step.into(),
        digest: ContentHash::of_value(&serde_json::Value::Null)
            .map_err(|error| FederatedEvidenceShiftError::Digest(error.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| FederatedEvidenceShiftError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site(
        site_id: &str,
        evidence_id: &str,
        baseline: i32,
        recent: i32,
    ) -> FederatedEvidenceShiftSite {
        FederatedEvidenceShiftSite {
            site_id: site_id.into(),
            study_id: format!("study-{site_id}"),
            evidence_id: evidence_id.into(),
            baseline_mean_milli: baseline,
            recent_mean_milli: recent,
            baseline_uncertainty_milli: 20,
            recent_uncertainty_milli: 20,
            baseline_count: 4,
            recent_count: 4,
            local_heterogeneity_milli: 100,
            quality_milli: 900,
            artifact: LocalArtifactRef {
                artifact_id: format!("artifact-{site_id}"),
                content_hash: ContentHash::of_value(&serde_json::json!({"site": site_id})).unwrap(),
                content_type: "aggregate-evidence-shift".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
            preclinical_only: true,
        }
    }

    fn request() -> FederatedEvidenceShiftRequest {
        FederatedEvidenceShiftRequest {
            objective: "consensus shift in glioma evidence".into(),
            snapshot_id: "snapshot-1".into(),
            minimum_sites: 3,
            minimum_consensus_milli: 800,
            minimum_change_milli: 100,
            max_heterogeneity_milli: 500,
            max_leave_one_out_shift_milli: 100,
            max_actions: 8,
        }
    }

    #[test]
    fn qualifies_a_concordant_multi_site_shift() {
        let result = analyze_federated_evidence_shifts(
            &request(),
            &[
                site("site-a", "egfr", 100, 420),
                site("site-b", "egfr", 120, 440),
                site("site-c", "egfr", 110, 430),
            ],
        )
        .expect("federated shift");
        assert_eq!(
            result.disposition,
            FederatedEvidenceShiftDisposition::Qualified
        );
        assert_eq!(
            result.actions[0].kind,
            FederatedEvidenceShiftKind::ConsensusEmergence
        );
        assert!(result.actions[0].consensus_milli >= 800);
        result.validate().expect("digest validates");
    }

    #[test]
    fn retains_site_specific_shift_as_negative_evidence() {
        let result = analyze_federated_evidence_shifts(
            &request(),
            &[
                site("site-a", "egfr", 100, 600),
                site("site-b", "egfr", 100, 100),
                site("site-c", "egfr", 100, 100),
            ],
        )
        .expect("federated shift");
        assert_eq!(
            result.disposition,
            FederatedEvidenceShiftDisposition::SiteSpecific
        );
        assert!(result
            .negative_evidence
            .iter()
            .any(|entry| entry.contains("not consensus")));
        assert!(result.actions.is_empty());
    }
}
