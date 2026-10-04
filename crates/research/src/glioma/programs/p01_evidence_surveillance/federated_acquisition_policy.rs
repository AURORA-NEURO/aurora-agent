//! Consortium-aware acquisition policy for unresolved preclinical glioma evidence.
//!
//! This module is deliberately a policy compiler rather than a federation client.  It turns
//! typed evidence needs into bounded, site-local acquisition actions while preserving quorum,
//! independence, cost, privacy, and local-raw-data constraints.  It never moves specimen bytes,
//! contacts a site, or claims that a proposed acquisition happened.

use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P01-F12";
pub const OUTPUT_SCHEMA: &str = "GliomaFederatedEvidenceAcquisitionPolicy1@1";
pub const MAX_NEEDS: usize = 1_024;
pub const MAX_SITES: usize = 512;
pub const MAX_ACTIONS: usize = 1_024;

/// A typed acquisition that a site can perform locally.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedAcquisitionKind {
    LiteratureReview,
    DatasetReanalysis,
    AssayReplication,
    MultimodalValidation,
    ModelComparison,
}

/// Why an evidence need did or did not enter the bounded plan.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedAcquisitionDecision {
    Selected,
    Deferred,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedEvidenceNeed {
    pub need_id: String,
    pub claim_key: String,
    pub modality: GliomaModality,
    pub model_system: Option<GliomaModelSystem>,
    pub action_kind: FederatedAcquisitionKind,
    pub priority_milli: u16,
    pub expected_information_milli: u16,
    pub estimated_cost_units: u64,
    pub required_sites: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedAcquisitionSite {
    pub site_id: String,
    pub independence_group: String,
    pub supported_modalities: BTreeSet<GliomaModality>,
    pub supported_model_systems: BTreeSet<GliomaModelSystem>,
    pub capability_milli: u16,
    pub independence_milli: u16,
    pub privacy_risk_milli: u16,
    pub cost_multiplier_milli: u16,
    pub local_only: bool,
    pub revoked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedAcquisitionPolicyRequest {
    pub objective: String,
    pub needs: Vec<FederatedEvidenceNeed>,
    pub sites: Vec<FederatedAcquisitionSite>,
    pub budget_units: u64,
    pub max_actions: usize,
    pub max_site_privacy_risk_milli: u16,
    pub privacy_budget_milli: u32,
    pub min_site_capability_milli: u16,
    pub min_independence_milli: u16,
    pub require_local_raw_data: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedAcquisitionAction {
    pub action_id: String,
    pub need_id: String,
    pub site_id: String,
    pub independence_group: String,
    pub estimated_cost_units: u64,
    pub privacy_risk_milli: u16,
    pub score_milli: i32,
    pub rank: u16,
    pub local_raw_data_required: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedNeedDecision {
    pub need_id: String,
    pub decision: FederatedAcquisitionDecision,
    pub candidate_site_order: Vec<String>,
    pub selected_site_order: Vec<String>,
    pub independent_group_order: Vec<String>,
    pub selected_count: usize,
    pub required_sites: usize,
    pub score_milli: i32,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedEvidenceAcquisitionPolicy {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub need_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub deferred_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub actions: Vec<FederatedAcquisitionAction>,
    pub decisions: Vec<FederatedNeedDecision>,
    pub budget_spent_units: u64,
    pub budget_remaining_units: u64,
    pub privacy_spent_milli: u32,
    pub quorum_satisfied_count: usize,
    pub raw_data_local_only: bool,
    pub uncertainty: Vec<String>,
    pub next_steps: Vec<String>,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedAcquisitionPolicyError {
    #[error("federated acquisition request is invalid: {0}")]
    InvalidRequest(String),
    #[error("federated acquisition output is invalid: {0}")]
    InvalidOutput(String),
    #[error("federated acquisition digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &FederatedEvidenceAcquisitionPolicy) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "need_order": output.need_order,
        "selected_order": output.selected_order,
        "deferred_order": output.deferred_order,
        "blocked_order": output.blocked_order,
        "actions": output.actions,
        "decisions": output.decisions,
        "budget_spent_units": output.budget_spent_units,
        "budget_remaining_units": output.budget_remaining_units,
        "privacy_spent_milli": output.privacy_spent_milli,
        "quorum_satisfied_count": output.quorum_satisfied_count,
        "raw_data_local_only": output.raw_data_local_only,
        "uncertainty": output.uncertainty,
        "next_steps": output.next_steps,
    })
}

impl FederatedEvidenceAcquisitionPolicy {
    pub fn validate(&self) -> Result<(), FederatedAcquisitionPolicyError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !canonical(&self.need_order)
            || !canonical(&self.selected_order)
            || !canonical(&self.deferred_order)
            || !canonical(&self.blocked_order)
            || !canonical(&self.uncertainty)
            || !canonical(&self.next_steps)
            || !canonical(
                &self
                    .actions
                    .iter()
                    .map(|row| row.action_id.clone())
                    .collect::<Vec<_>>(),
            )
            || !canonical(
                &self
                    .decisions
                    .iter()
                    .map(|row| row.need_id.clone())
                    .collect::<Vec<_>>(),
            )
            || self.digest.as_str().len() != 64
            || self.actions.iter().any(|row| {
                row.action_id.trim().is_empty()
                    || row.need_id.trim().is_empty()
                    || row.site_id.trim().is_empty()
                    || row.independence_group.trim().is_empty()
                    || row.estimated_cost_units == 0
                    || row.score_milli < 0
                    || row.score_milli > 1_000
                    || row.rank == 0
                    || !row.local_raw_data_required
            })
            || self.decisions.iter().any(|row| {
                row.need_id.trim().is_empty()
                    || row.required_sites == 0
                    || row.selected_count != row.selected_site_order.len()
                    || !canonical(&row.candidate_site_order)
                    || !canonical(&row.selected_site_order)
                    || !canonical(&row.independent_group_order)
                    || row.score_milli < 0
                    || row.score_milli > 1_000
                    || row.reason.trim().is_empty()
            })
        {
            return Err(FederatedAcquisitionPolicyError::InvalidOutput(
                "identity, canonical ordering, scores, quorum, action, or digest fields are invalid".into(),
            ));
        }

        let need_set = self.need_order.iter().cloned().collect::<BTreeSet<_>>();
        let selected_set = self.selected_order.iter().cloned().collect::<BTreeSet<_>>();
        let deferred_set = self.deferred_order.iter().cloned().collect::<BTreeSet<_>>();
        let blocked_set = self.blocked_order.iter().cloned().collect::<BTreeSet<_>>();
        let action_set = self
            .actions
            .iter()
            .map(|row| row.action_id.clone())
            .collect::<BTreeSet<_>>();
        let decision_set = self
            .decisions
            .iter()
            .map(|row| row.need_id.clone())
            .collect::<BTreeSet<_>>();
        if need_set.len() != self.need_order.len()
            || decision_set != need_set
            || selected_set.len() != self.selected_order.len()
            || deferred_set.len() != self.deferred_order.len()
            || blocked_set.len() != self.blocked_order.len()
            || selected_set.intersection(&deferred_set).next().is_some()
            || selected_set.intersection(&blocked_set).next().is_some()
            || deferred_set.intersection(&blocked_set).next().is_some()
            || selected_set != action_set
            || self.budget_spent_units
                < self
                    .actions
                    .iter()
                    .map(|row| row.estimated_cost_units)
                    .sum::<u64>()
            || self.quorum_satisfied_count > self.decisions.len()
        {
            return Err(FederatedAcquisitionPolicyError::InvalidOutput(
                "need partitions, action identity, budget accounting, or decision coverage is invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| FederatedAcquisitionPolicyError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(FederatedAcquisitionPolicyError::Digest(
                "policy digest does not match canonical content".into(),
            ));
        }
        Ok(())
    }
}

fn score_site(need: &FederatedEvidenceNeed, site: &FederatedAcquisitionSite, cost: u64) -> i32 {
    let cost_efficiency =
        ((u64::from(need.expected_information_milli) * 1_000) / cost.max(1)).min(1_000) as i32;
    let cost_component = (1_000 - cost_efficiency).max(0);
    let privacy_component = 1_000 - i32::from(site.privacy_risk_milli);
    // Priority and information dominate; capability, privacy, and cost provide deterministic
    // tie-breaking pressure without pretending to estimate a biological effect.
    (i32::from(need.priority_milli) * 300
        + i32::from(need.expected_information_milli) * 300
        + i32::from(site.capability_milli) * 180
        + privacy_component * 120
        + cost_component * 100)
        / 1_000
}

fn valid_site(
    request: &FederatedAcquisitionPolicyRequest,
    need: &FederatedEvidenceNeed,
    site: &FederatedAcquisitionSite,
) -> bool {
    !site.revoked
        && site.capability_milli >= request.min_site_capability_milli
        && site.independence_milli >= request.min_independence_milli
        && site.privacy_risk_milli <= request.max_site_privacy_risk_milli
        // The glioma boundary is local-first even when a caller forgets to set the stricter
        // request flag: federation may exchange policy metadata, never raw experimental bytes.
        && site.local_only
        && site.supported_modalities.contains(&need.modality)
        && need
            .model_system
            .map(|model| site.supported_model_systems.contains(&model))
            .unwrap_or(true)
}

/// Compile a deterministic, quorum-aware acquisition portfolio for a federated preclinical
/// consortium.  All selected actions remain local-site requests; only bounded metadata leaves a
/// site, and this function itself performs no network or instrument operation.
pub fn plan_federated_glioma_evidence_acquisition(
    request: &FederatedAcquisitionPolicyRequest,
) -> Result<FederatedEvidenceAcquisitionPolicy, FederatedAcquisitionPolicyError> {
    if request.objective.trim().is_empty()
        || request.needs.is_empty()
        || request.needs.len() > MAX_NEEDS
        || request.sites.is_empty()
        || request.sites.len() > MAX_SITES
        || request.max_actions == 0
        || request.max_actions > MAX_ACTIONS
        || request.budget_units == 0
        || request.min_independence_milli > 1_000
        || request.max_site_privacy_risk_milli > 1_000
        || request.privacy_budget_milli > 1_000_000
        || request.min_site_capability_milli > 1_000
    {
        return Err(FederatedAcquisitionPolicyError::InvalidRequest(
            "objective, bounds, budget, privacy, capability, or action limits are invalid".into(),
        ));
    }
    let mut needs_by_id = BTreeMap::new();
    for need in &request.needs {
        if need.need_id.trim().is_empty()
            || need.claim_key.trim().is_empty()
            || need.priority_milli > 1_000
            || need.expected_information_milli > 1_000
            || need.estimated_cost_units == 0
            || need.required_sites == 0
            || need.required_sites > request.sites.len()
            || needs_by_id.insert(need.need_id.clone(), need).is_some()
        {
            return Err(FederatedAcquisitionPolicyError::InvalidRequest(
                "needs must have unique non-empty ids and bounded values".into(),
            ));
        }
    }
    let mut sites_by_id = BTreeMap::new();
    for site in &request.sites {
        if site.site_id.trim().is_empty()
            || site.independence_group.trim().is_empty()
            || site.capability_milli > 1_000
            || site.independence_milli > 1_000
            || site.privacy_risk_milli > 1_000
            || site.cost_multiplier_milli == 0
            || site.cost_multiplier_milli > 4_000
            || sites_by_id.insert(site.site_id.clone(), site).is_some()
        {
            return Err(FederatedAcquisitionPolicyError::InvalidRequest(
                "sites must have unique non-empty ids and bounded values".into(),
            ));
        }
    }

    let mut need_order = needs_by_id.keys().cloned().collect::<Vec<_>>();
    need_order.sort_by(|left, right| {
        needs_by_id[right]
            .priority_milli
            .cmp(&needs_by_id[left].priority_milli)
            .then_with(|| {
                needs_by_id[right]
                    .expected_information_milli
                    .cmp(&needs_by_id[left].expected_information_milli)
            })
            .then_with(|| left.cmp(right))
    });
    let canonical_need_order = {
        let mut ids = need_order.clone();
        ids.sort();
        ids
    };

    let mut actions = Vec::new();
    let mut decisions = Vec::new();
    let mut deferred_ids = BTreeSet::new();
    let mut blocked_ids = BTreeSet::new();
    let mut spent = 0_u64;
    let mut privacy_spent = 0_u32;
    let mut quorum_satisfied_count = 0_usize;
    let mut uncertainty = BTreeSet::new();

    for need_id in &need_order {
        let need = needs_by_id[need_id];
        let mut candidates = sites_by_id
            .values()
            .filter(|site| valid_site(request, need, site))
            .map(|site| {
                let cost = (need
                    .estimated_cost_units
                    .saturating_mul(u64::from(site.cost_multiplier_milli))
                    .saturating_add(999))
                    / 1_000;
                (site, cost, score_site(need, site, cost))
            })
            .collect::<Vec<_>>();
        candidates.sort_by(|left, right| {
            right
                .2
                .cmp(&left.2)
                .then_with(|| left.0.site_id.cmp(&right.0.site_id))
        });
        let mut candidate_site_order = candidates
            .iter()
            .map(|row| row.0.site_id.clone())
            .collect::<Vec<_>>();
        candidate_site_order.sort();
        let mut selected = Vec::new();
        let mut groups = BTreeSet::new();
        let mut local_spent = 0_u64;
        let mut local_privacy = 0_u32;
        for (site, cost, score) in &candidates {
            if groups.contains(&site.independence_group)
                || selected.len() >= need.required_sites
                || actions.len().saturating_add(selected.len()) >= request.max_actions
            {
                continue;
            }
            if spent.saturating_add(local_spent).saturating_add(*cost) > request.budget_units
                || privacy_spent
                    .saturating_add(local_privacy)
                    .saturating_add(u32::from(site.privacy_risk_milli))
                    > request.privacy_budget_milli
            {
                continue;
            }
            groups.insert(site.independence_group.clone());
            selected.push((site, *cost, *score));
            local_spent = local_spent.saturating_add(*cost);
            local_privacy = local_privacy.saturating_add(u32::from(site.privacy_risk_milli));
        }

        let (decision, reason) = if candidates.is_empty() {
            blocked_ids.insert(need_id.clone());
            uncertainty.insert(format!("{need_id}: no eligible local site met modality, capability, privacy, or revocation policy"));
            (
                FederatedAcquisitionDecision::Blocked,
                "no eligible local site".to_string(),
            )
        } else if selected.len() == need.required_sites {
            quorum_satisfied_count += 1;
            spent = spent.saturating_add(local_spent);
            privacy_spent = privacy_spent.saturating_add(local_privacy);
            (
                FederatedAcquisitionDecision::Selected,
                "independent quorum satisfied within budget".to_string(),
            )
        } else {
            deferred_ids.insert(need_id.clone());
            uncertainty.insert(format!("{need_id}: eligible sites exist but quorum, budget, privacy, or action capacity is unresolved"));
            (
                FederatedAcquisitionDecision::Deferred,
                "eligible but quorum was not affordable".to_string(),
            )
        };

        let mut selected_site_order = if decision == FederatedAcquisitionDecision::Selected {
            selected
                .iter()
                .map(|row| row.0.site_id.clone())
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        selected_site_order.sort();
        let mut independent_group_order = if decision == FederatedAcquisitionDecision::Selected {
            selected
                .iter()
                .map(|row| row.0.independence_group.clone())
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        independent_group_order.sort();
        let score_milli = selected
            .iter()
            .map(|row| row.2)
            .max()
            .unwrap_or_else(|| candidates.first().map(|row| row.2).unwrap_or(0));
        if decision == FederatedAcquisitionDecision::Selected {
            for (site, cost, score) in selected {
                let action_id = format!("{}::{}", need_id, site.site_id);
                actions.push(FederatedAcquisitionAction {
                    action_id,
                    need_id: need_id.clone(),
                    site_id: site.site_id.clone(),
                    independence_group: site.independence_group.clone(),
                    estimated_cost_units: cost,
                    privacy_risk_milli: site.privacy_risk_milli,
                    score_milli: score,
                    rank: 0,
                    local_raw_data_required: true,
                });
            }
        }
        decisions.push(FederatedNeedDecision {
            need_id: need_id.clone(),
            decision,
            candidate_site_order,
            selected_site_order,
            independent_group_order,
            selected_count: if decision == FederatedAcquisitionDecision::Selected {
                need.required_sites
            } else {
                0
            },
            required_sites: need.required_sites,
            score_milli,
            reason,
        });
    }

    actions.sort_by(|left, right| left.action_id.cmp(&right.action_id));
    for (index, action) in actions.iter_mut().enumerate() {
        action.rank = u16::try_from(index + 1).map_err(|_| {
            FederatedAcquisitionPolicyError::InvalidOutput("action rank exceeds u16".into())
        })?;
    }
    decisions.sort_by(|left, right| left.need_id.cmp(&right.need_id));
    let selected_order = actions
        .iter()
        .map(|row| row.action_id.clone())
        .collect::<Vec<_>>();
    let deferred_order = deferred_ids.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked_ids.into_iter().collect::<Vec<_>>();
    let raw_data_local_only = actions.iter().all(|row| row.local_raw_data_required);
    let next_steps = if blocked_order.is_empty() && deferred_order.is_empty() {
        vec![
            "dispatch selected actions to site-local adapters after institution approval"
                .to_string(),
        ]
    } else {
        vec![
            "resolve blocked or deferred site capability and quorum debt before dispatch"
                .to_string(),
        ]
    };
    let mut output = FederatedEvidenceAcquisitionPolicy {
        feature_id: FEATURE_ID.to_string(),
        output_schema: OUTPUT_SCHEMA.to_string(),
        objective: request.objective.clone(),
        need_order: canonical_need_order,
        selected_order,
        deferred_order,
        blocked_order,
        actions,
        decisions,
        budget_spent_units: spent,
        budget_remaining_units: request.budget_units.saturating_sub(spent),
        privacy_spent_milli: privacy_spent,
        quorum_satisfied_count,
        raw_data_local_only,
        uncertainty: uncertainty.into_iter().collect(),
        next_steps,
        digest: ContentHash::of_value(&serde_json::Value::Null)
            .map_err(|error| FederatedAcquisitionPolicyError::Digest(error.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| FederatedAcquisitionPolicyError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site(id: &str, group: &str, modality: GliomaModality) -> FederatedAcquisitionSite {
        FederatedAcquisitionSite {
            site_id: id.into(),
            independence_group: group.into(),
            supported_modalities: [modality].into_iter().collect(),
            supported_model_systems: [GliomaModelSystem::Organoid].into_iter().collect(),
            capability_milli: 900,
            independence_milli: 900,
            privacy_risk_milli: 50,
            cost_multiplier_milli: 1_000,
            local_only: true,
            revoked: false,
        }
    }

    fn request() -> FederatedAcquisitionPolicyRequest {
        FederatedAcquisitionPolicyRequest {
            objective: "resolve organoid resistance evidence".into(),
            needs: vec![FederatedEvidenceNeed {
                need_id: "need-1".into(),
                claim_key: "egfr-resistance".into(),
                modality: GliomaModality::FunctionalPerturbation,
                model_system: Some(GliomaModelSystem::Organoid),
                action_kind: FederatedAcquisitionKind::AssayReplication,
                priority_milli: 900,
                expected_information_milli: 850,
                estimated_cost_units: 10,
                required_sites: 2,
            }],
            sites: vec![
                site("site-a", "north", GliomaModality::FunctionalPerturbation),
                site("site-b", "south", GliomaModality::FunctionalPerturbation),
            ],
            budget_units: 30,
            max_actions: 8,
            max_site_privacy_risk_milli: 200,
            privacy_budget_milli: 500,
            min_site_capability_milli: 700,
            min_independence_milli: 500,
            require_local_raw_data: true,
        }
    }

    #[test]
    fn selects_independent_sites_and_preserves_locality() {
        let output = plan_federated_glioma_evidence_acquisition(&request()).unwrap();
        assert_eq!(output.quorum_satisfied_count, 1);
        assert_eq!(output.actions.len(), 2);
        assert!(output.raw_data_local_only);
        assert_eq!(output.budget_spent_units, 20);
        output.validate().unwrap();
    }

    #[test]
    fn blocks_revoked_or_unsupported_sites() {
        let mut input = request();
        input.sites[0].revoked = true;
        input.sites[1].supported_modalities.clear();
        let output = plan_federated_glioma_evidence_acquisition(&input).unwrap();
        assert_eq!(output.blocked_order, vec!["need-1"]);
        assert!(output.actions.is_empty());
    }

    #[test]
    fn defers_when_budget_cannot_buy_the_quorum() {
        let mut input = request();
        input.budget_units = 10;
        let output = plan_federated_glioma_evidence_acquisition(&input).unwrap();
        assert_eq!(output.deferred_order, vec!["need-1"]);
        assert!(output.actions.is_empty());
        assert_eq!(output.budget_spent_units, 0);
    }
}
