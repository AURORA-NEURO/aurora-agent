//! Explainable, policy-bounded site selection for federated preclinical glioma workflows.
//!
//! The selector turns aggregate capability advertisements into a deterministic consortium plan.
//! It optimizes compatibility, representation, marginal independence, capacity, and freshness while
//! refusing revoked, stale, privacy-incompatible, human-data, or under-capacity sites. Selection
//! is a planning result only: it does not invite a site, move data, or start an experiment.

use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P12-F09";
pub const OUTPUT_SCHEMA: &str = "GliomaFederatedSiteSelectionPlan1@3";
pub const MAX_SITES: usize = 256;
pub const MAX_TAGS: usize = 128;
const SITE_BEAM_WIDTH: usize = 96;
const MAX_SITE_SEARCH_DEPTH: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GliomaSiteCapabilityEnvelope {
    pub site_id: String,
    pub institution_group: String,
    pub capability_ids: Vec<String>,
    pub model_systems: Vec<GliomaModelSystem>,
    pub representation_tags: Vec<String>,
    pub capacity_units: u64,
    pub estimated_cost_units: u64,
    pub freshness_age_hours: u64,
    pub privacy_approved: bool,
    pub revoked: bool,
    pub local_only: bool,
    pub contains_human_data: bool,
    pub contains_direct_identifiers: bool,
    pub manifest_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedSiteSelectionRequest {
    pub objective: String,
    pub capability_id: String,
    pub benchmark_world: String,
    pub model_system: GliomaModelSystem,
    pub minimum_sites: usize,
    pub maximum_sites: usize,
    pub minimum_capacity_units: u64,
    pub maximum_cost_units: u64,
    pub maximum_freshness_age_hours: u64,
    pub maximum_same_group_fraction_milli: u16,
    pub required_representation_tags: Vec<String>,
    pub envelopes: Vec<GliomaSiteCapabilityEnvelope>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiteSelectionScore {
    pub site_id: String,
    pub total_score_milli: u16,
    pub compatibility_milli: u16,
    pub representation_milli: u16,
    pub independence_milli: u16,
    /// The diversity/representation bonus actually applied when this site was selected.
    /// Keeping this separate from the static score makes the greedy decision auditable.
    pub marginal_bonus_milli: u16,
    pub capacity_milli: u16,
    pub freshness_milli: u16,
    pub selected: bool,
    pub reason: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SiteSelectionDisposition {
    Qualified,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedSiteSelectionPlan {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub capability_id: String,
    pub benchmark_world: String,
    pub selected_site_order: Vec<String>,
    pub deferred_site_order: Vec<String>,
    pub excluded_site_order: Vec<String>,
    pub unresolved_site_order: Vec<String>,
    pub representation_coverage_milli: u16,
    pub maximum_group_fraction_milli: u16,
    pub scores: Vec<SiteSelectionScore>,
    pub disposition: SiteSelectionDisposition,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedSiteSelectionError {
    #[error("site selection request is invalid: {0}")]
    InvalidRequest(String),
    #[error("site selection output is invalid: {0}")]
    InvalidOutput(String),
    #[error("site selection digest failed: {0}")]
    Digest(String),
}

fn safe_text(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 256
        && !value.chars().any(|character| character.is_control())
}

fn canonical(values: &[String]) -> bool {
    values.iter().all(|value| safe_text(value)) && values.windows(2).all(|pair| pair[0] != pair[1])
}

fn contains<T: PartialEq>(values: &[T], value: &T) -> bool {
    values.iter().any(|candidate| candidate == value)
}

fn output_body(plan: &FederatedSiteSelectionPlan) -> serde_json::Value {
    serde_json::json!({
        "feature_id": plan.feature_id,
        "output_schema": plan.output_schema,
        "objective": plan.objective,
        "capability_id": plan.capability_id,
        "benchmark_world": plan.benchmark_world,
        "selected_site_order": plan.selected_site_order,
        "deferred_site_order": plan.deferred_site_order,
        "excluded_site_order": plan.excluded_site_order,
        "unresolved_site_order": plan.unresolved_site_order,
        "representation_coverage_milli": plan.representation_coverage_milli,
        "maximum_group_fraction_milli": plan.maximum_group_fraction_milli,
        "scores": plan.scores,
        "disposition": plan.disposition,
        "negative_evidence": plan.negative_evidence,
        "uncertainty": plan.uncertainty,
    })
}

impl FederatedSiteSelectionPlan {
    pub fn validate(&self) -> Result<(), FederatedSiteSelectionError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || !safe_text(&self.objective)
            || !safe_text(&self.capability_id)
            || !safe_text(&self.benchmark_world)
            || !canonical(&self.selected_site_order)
            || !canonical(&self.deferred_site_order)
            || !canonical(&self.excluded_site_order)
            || !canonical(&self.unresolved_site_order)
            || self.representation_coverage_milli > 1_000
            || self.maximum_group_fraction_milli > 1_000
            || self.scores.iter().any(|score| {
                !safe_text(&score.site_id)
                    || score.total_score_milli > 1_000
                    || score.compatibility_milli > 1_000
                    || score.representation_milli > 1_000
                    || score.independence_milli > 1_000
                    || score.marginal_bonus_milli > 1_000
                    || score.capacity_milli > 1_000
                    || score.freshness_milli > 1_000
                    || !safe_text(&score.reason)
            })
        {
            return Err(FederatedSiteSelectionError::InvalidOutput(
                "selection identity, partitions, scores, or bounded coverage invariants are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&output_body(self))
            .map_err(|error| FederatedSiteSelectionError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(FederatedSiteSelectionError::InvalidOutput(
                "selection digest is not content-bound".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &FederatedSiteSelectionRequest,
) -> Result<(), FederatedSiteSelectionError> {
    if !safe_text(&request.objective)
        || !safe_text(&request.capability_id)
        || !safe_text(&request.benchmark_world)
        || request.minimum_sites == 0
        || request.minimum_sites > request.maximum_sites
        || request.maximum_sites > MAX_SITES
        || request.minimum_capacity_units == 0
        || request.maximum_cost_units == 0
        || request.maximum_freshness_age_hours == 0
        || request.maximum_same_group_fraction_milli == 0
        || request.maximum_same_group_fraction_milli > 1_000
        || request.required_representation_tags.len() > MAX_TAGS
        || request.envelopes.is_empty()
        || request.envelopes.len() > MAX_SITES
        || request
            .required_representation_tags
            .iter()
            .any(|tag| !safe_text(tag))
    {
        return Err(FederatedSiteSelectionError::InvalidRequest(
            "objective, binding, site bounds, capacity/cost/freshness/fairness gates, and bounded capability envelopes are required".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for envelope in &request.envelopes {
        if !safe_text(&envelope.site_id)
            || !ids.insert(envelope.site_id.clone())
            || !safe_text(&envelope.institution_group)
            || envelope.capability_ids.iter().any(|id| !safe_text(id))
            || envelope.representation_tags.len() > MAX_TAGS
            || envelope
                .representation_tags
                .iter()
                .any(|tag| !safe_text(tag))
            || envelope.capacity_units == 0
            || envelope.estimated_cost_units == 0
            || !envelope.local_only
            || envelope.contains_human_data
            || envelope.contains_direct_identifiers
        {
            return Err(FederatedSiteSelectionError::InvalidRequest(format!(
                "site {} has invalid capability, capacity, locality, or privacy metadata",
                envelope.site_id
            )));
        }
    }
    Ok(())
}

#[derive(Debug, Clone)]
struct SitePortfolioState {
    selected: Vec<String>,
    total_utility: u64,
    total_cost: u64,
    covered: BTreeSet<String>,
    groups: BTreeMap<String, usize>,
    marginal_scores: BTreeMap<String, (u16, u16)>,
}

fn site_coverage_complete(
    state: &SitePortfolioState,
    required_representation_tags: &[String],
) -> bool {
    required_representation_tags.is_empty()
        || required_representation_tags
            .iter()
            .all(|tag| state.covered.contains(tag))
}

fn site_state_complete(
    state: &SitePortfolioState,
    request: &FederatedSiteSelectionRequest,
) -> bool {
    state.selected.len() >= request.minimum_sites
        && site_coverage_complete(state, &request.required_representation_tags)
}

fn site_state_better(
    left: &SitePortfolioState,
    right: &SitePortfolioState,
    request: &FederatedSiteSelectionRequest,
) -> bool {
    let left_complete = site_state_complete(left, request);
    let right_complete = site_state_complete(right, request);
    if left_complete != right_complete {
        return left_complete;
    }
    if left_complete {
        left.selected.len() < right.selected.len()
            || (left.selected.len() == right.selected.len()
                && (left.total_utility > right.total_utility
                    || (left.total_utility == right.total_utility
                        && (left.total_cost < right.total_cost
                            || (left.total_cost == right.total_cost
                                && left.selected < right.selected)))))
    } else {
        let left_coverage = left.covered.len();
        let right_coverage = right.covered.len();
        left_coverage > right_coverage
            || (left_coverage == right_coverage
                && (left.total_utility > right.total_utility
                    || (left.total_utility == right.total_utility
                        && (left.selected.len() > right.selected.len()
                            || (left.selected.len() == right.selected.len()
                                && left.selected < right.selected)))))
    }
}

fn select_site_portfolio<'a>(
    request: &FederatedSiteSelectionRequest,
    eligible: &[&'a GliomaSiteCapabilityEnvelope],
    preliminary: &[SiteSelectionScore],
) -> (SitePortfolioState, bool) {
    let initial = SitePortfolioState {
        selected: Vec::new(),
        total_utility: 0,
        total_cost: 0,
        covered: BTreeSet::new(),
        groups: BTreeMap::new(),
        marginal_scores: BTreeMap::new(),
    };
    let mut beam = vec![initial.clone()];
    let mut best = initial;
    let search_depth = request.maximum_sites.min(MAX_SITE_SEARCH_DEPTH);
    for _ in 0..search_depth {
        let mut expanded = Vec::new();
        for state in &beam {
            for envelope in eligible {
                if state
                    .selected
                    .last()
                    .is_some_and(|last| envelope.site_id <= *last)
                {
                    continue;
                }
                let group_count = state
                    .groups
                    .get(&envelope.institution_group)
                    .copied()
                    .unwrap_or(0);
                let would_fraction = ((group_count + 1) * 1_000) / (state.selected.len() + 1);
                if !state.selected.is_empty()
                    && would_fraction > usize::from(request.maximum_same_group_fraction_milli)
                {
                    continue;
                }
                let new_tags = request
                    .required_representation_tags
                    .iter()
                    .filter(|tag| {
                        contains(&envelope.representation_tags, tag)
                            && !state.covered.contains(*tag)
                    })
                    .count();
                let base_score = preliminary
                    .iter()
                    .find(|score| score.site_id == envelope.site_id)
                    .map(|score| score.total_score_milli)
                    .unwrap_or(0);
                let independence_milli = 1_000_u16.saturating_sub(would_fraction.min(1_000) as u16);
                let representation_bonus = new_tags.saturating_mul(100).min(1_000) as u16;
                let marginal_bonus = representation_bonus
                    .saturating_add(independence_milli / 5)
                    .min(1_000);
                let mut selected = state.selected.clone();
                selected.push(envelope.site_id.clone());
                let mut covered = state.covered.clone();
                covered.extend(
                    envelope
                        .representation_tags
                        .iter()
                        .filter(|tag| request.required_representation_tags.contains(tag))
                        .cloned(),
                );
                let mut groups = state.groups.clone();
                *groups
                    .entry(envelope.institution_group.clone())
                    .or_default() += 1;
                let mut marginal_scores = state.marginal_scores.clone();
                marginal_scores.insert(
                    envelope.site_id.clone(),
                    (independence_milli, marginal_bonus),
                );
                expanded.push(SitePortfolioState {
                    selected,
                    total_utility: state
                        .total_utility
                        .saturating_add(u64::from(base_score))
                        .saturating_add(u64::from(marginal_bonus)),
                    total_cost: state
                        .total_cost
                        .saturating_add(envelope.estimated_cost_units),
                    covered,
                    groups,
                    marginal_scores,
                });
            }
        }
        if expanded.is_empty() {
            break;
        }
        expanded.sort_by(|left, right| {
            if site_state_better(left, right, request) {
                std::cmp::Ordering::Less
            } else if site_state_better(right, left, request) {
                std::cmp::Ordering::Greater
            } else {
                left.selected.cmp(&right.selected)
            }
        });
        expanded.truncate(SITE_BEAM_WIDTH);
        for candidate in &expanded {
            if site_state_better(candidate, &best, request) {
                best = candidate.clone();
            }
        }
        beam = expanded;
        if site_state_complete(&best, request) {
            // Once a complete plan is found, the comparator prefers the smallest complete
            // portfolio; later rounds can still replace it with an equally small, stronger plan.
            if best.selected.len() == request.minimum_sites
                && site_coverage_complete(&best, &request.required_representation_tags)
            {
                break;
            }
        }
    }
    (best, request.maximum_sites > MAX_SITE_SEARCH_DEPTH)
}

pub fn plan_federated_glioma_sites(
    request: &FederatedSiteSelectionRequest,
) -> Result<FederatedSiteSelectionPlan, FederatedSiteSelectionError> {
    validate_request(request)?;
    let mut eligible = Vec::new();
    let mut excluded = Vec::new();
    let mut unresolved = Vec::new();
    let mut preliminary = Vec::new();
    for envelope in &request.envelopes {
        let capability = contains(&envelope.capability_ids, &request.capability_id);
        let model = contains(&envelope.model_systems, &request.model_system);
        let fresh = envelope.freshness_age_hours <= request.maximum_freshness_age_hours;
        let capacity = envelope.capacity_units >= request.minimum_capacity_units;
        let cost = envelope.estimated_cost_units <= request.maximum_cost_units;
        let forbidden = envelope.revoked
            || !envelope.privacy_approved
            || !envelope.local_only
            || envelope.contains_human_data
            || envelope.contains_direct_identifiers;
        let representation_hits = request
            .required_representation_tags
            .iter()
            .filter(|tag| contains(&envelope.representation_tags, tag))
            .count();
        let representation = if request.required_representation_tags.is_empty() {
            1_000
        } else {
            ((representation_hits * 1_000) / request.required_representation_tags.len()) as u16
        };
        let compatibility = if capability && model { 1_000 } else { 0 };
        let capacity_score = ((envelope
            .capacity_units
            .min(request.minimum_capacity_units.saturating_mul(4))
            * 1_000)
            / request.minimum_capacity_units)
            .min(1_000) as u16;
        let freshness_score = (1_000_u64.saturating_sub(
            envelope
                .freshness_age_hours
                .saturating_mul(1_000)
                .checked_div(request.maximum_freshness_age_hours.max(1))
                .unwrap_or(1_000),
        )) as u16;
        let mut reason: Vec<String> = Vec::new();
        if !capability {
            reason.push("missing capability".into());
        }
        if !model {
            reason.push("missing model system".into());
        }
        if !fresh {
            reason.push("stale capability manifest".into());
        }
        if !capacity {
            reason.push("insufficient capacity".into());
        }
        if !cost {
            reason.push("cost ceiling exceeded".into());
        }
        if forbidden {
            reason.push("revoked, unapproved, non-local, or protected data boundary".into());
        }
        let hard_eligible = capability && model && fresh && capacity && cost && !forbidden;
        let score = SiteSelectionScore {
            site_id: envelope.site_id.clone(),
            total_score_milli: ((compatibility as u32 * 350
                + representation as u32 * 250
                + capacity_score as u32 * 150
                + freshness_score as u32 * 100
                + 150_000)
                / 1_000) as u16,
            compatibility_milli: compatibility,
            representation_milli: representation,
            independence_milli: 1_000,
            marginal_bonus_milli: 0,
            capacity_milli: capacity_score,
            freshness_milli: freshness_score,
            selected: false,
            reason: if reason.is_empty() {
                "eligible pending independence and representation balancing".into()
            } else {
                reason.join("; ")
            },
        };
        if hard_eligible {
            eligible.push(envelope);
            preliminary.push(score);
        } else if !capability || !model || !fresh || forbidden {
            excluded.push(envelope.site_id.clone());
            preliminary.push(score);
        } else {
            unresolved.push(envelope.site_id.clone());
            preliminary.push(score);
        }
    }
    eligible.sort_by(|left, right| left.site_id.cmp(&right.site_id));
    preliminary.sort_by(|left, right| left.site_id.cmp(&right.site_id));
    let (best_portfolio, search_depth_bounded) =
        select_site_portfolio(request, &eligible, &preliminary);
    let mut selected = best_portfolio.selected.clone();
    let mut deferred = Vec::new();
    for envelope in &eligible {
        if !selected.contains(&envelope.site_id) {
            deferred.push(envelope.site_id.clone());
        }
    }
    selected.sort();
    deferred.sort();
    excluded.sort();
    unresolved.sort();
    let mut scores = preliminary;
    for score in &mut scores {
        score.selected = selected.contains(&score.site_id);
        if score.selected {
            if let Some((independence_milli, marginal_bonus)) =
                best_portfolio.marginal_scores.get(&score.site_id)
            {
                score.independence_milli = *independence_milli;
                score.marginal_bonus_milli = *marginal_bonus;
            }
            score.reason = "selected under capability, representation, capacity, freshness, and marginal-independence gates".into();
        }
    }
    scores.sort_by(|left, right| left.site_id.cmp(&right.site_id));
    let coverage = if request.required_representation_tags.is_empty() {
        1_000
    } else {
        ((best_portfolio.covered.len() * 1_000) / request.required_representation_tags.len())
            .min(1_000) as u16
    };
    let maximum_group_fraction = best_portfolio
        .groups
        .values()
        .map(|count| ((count * 1_000) / selected.len().max(1)) as u16)
        .max()
        .unwrap_or(1_000);
    let mut negative = Vec::new();
    let mut uncertainty = Vec::new();
    if selected.len() < request.minimum_sites {
        negative.push("eligible site count is below the consortium floor".into());
    }
    if coverage < 1_000 {
        negative.push("required representation tags are not fully covered".into());
    }
    if !excluded.is_empty() {
        uncertainty.push(
            "capability, model, freshness, approval, or privacy-incompatible sites were excluded"
                .into(),
        );
    }
    if !unresolved.is_empty() {
        uncertainty.push("capacity or cost evidence is unresolved for one or more sites".into());
    }
    if search_depth_bounded {
        uncertainty
            .push("site-portfolio search depth was bounded for deterministic planning".into());
    }
    negative.sort();
    negative.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    let disposition = if selected.len() < request.minimum_sites {
        SiteSelectionDisposition::Blocked
    } else if coverage < 1_000 || !deferred.is_empty() {
        SiteSelectionDisposition::Partial
    } else {
        SiteSelectionDisposition::Qualified
    };
    let mut plan = FederatedSiteSelectionPlan {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        capability_id: request.capability_id.clone(),
        benchmark_world: request.benchmark_world.clone(),
        selected_site_order: selected,
        deferred_site_order: deferred,
        excluded_site_order: excluded,
        unresolved_site_order: unresolved,
        representation_coverage_milli: coverage,
        maximum_group_fraction_milli: maximum_group_fraction,
        scores,
        disposition,
        negative_evidence: negative,
        uncertainty,
        digest: ContentHash::of_bytes(b"unsealed-glioma-site-selection"),
    };
    plan.digest = ContentHash::of_value(&output_body(&plan))
        .map_err(|error| FederatedSiteSelectionError::Digest(error.to_string()))?;
    plan.validate()?;
    Ok(plan)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn envelope(site_id: &str, group: &str, tags: &[&str]) -> GliomaSiteCapabilityEnvelope {
        GliomaSiteCapabilityEnvelope {
            site_id: site_id.into(),
            institution_group: group.into(),
            capability_ids: vec!["segmentation".into()],
            model_systems: vec![GliomaModelSystem::Organoid],
            representation_tags: tags.iter().map(|tag| (*tag).into()).collect(),
            capacity_units: 100,
            estimated_cost_units: 10,
            freshness_age_hours: 2,
            privacy_approved: true,
            revoked: false,
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
            manifest_digest: ContentHash::of_bytes(site_id.as_bytes()),
        }
    }

    fn request(envelopes: Vec<GliomaSiteCapabilityEnvelope>) -> FederatedSiteSelectionRequest {
        FederatedSiteSelectionRequest {
            objective: "select a glioma benchmark consortium".into(),
            capability_id: "segmentation".into(),
            benchmark_world: "glioma-world-v1".into(),
            model_system: GliomaModelSystem::Organoid,
            minimum_sites: 2,
            maximum_sites: 3,
            minimum_capacity_units: 50,
            maximum_cost_units: 100,
            maximum_freshness_age_hours: 24,
            maximum_same_group_fraction_milli: 500,
            required_representation_tags: vec!["organoid".into(), "imaging".into()],
            envelopes,
        }
    }

    #[test]
    fn selector_balances_representation_and_independence() {
        let plan = plan_federated_glioma_sites(&request(vec![
            envelope("site-a", "group-a", &["organoid", "imaging"]),
            envelope("site-b", "group-b", &["organoid"]),
            envelope("site-c", "group-c", &["imaging"]),
        ]))
        .unwrap();
        assert_eq!(plan.selected_site_order.len(), 2);
        assert_eq!(plan.representation_coverage_milli, 1_000);
        assert!(plan.validate().is_ok());
    }

    #[test]
    fn revoked_and_stale_sites_are_excluded() {
        let mut revoked = envelope("site-b", "group-b", &["imaging"]);
        revoked.revoked = true;
        let mut stale = envelope("site-c", "group-c", &["imaging"]);
        stale.freshness_age_hours = 100;
        let plan = plan_federated_glioma_sites(&request(vec![
            envelope("site-a", "group-a", &["organoid"]),
            revoked,
            stale,
        ]))
        .unwrap();
        assert!(plan.excluded_site_order.contains(&"site-b".into()));
        assert!(plan.excluded_site_order.contains(&"site-c".into()));
        assert_eq!(plan.disposition, SiteSelectionDisposition::Blocked);
    }

    #[test]
    fn same_group_fraction_prevents_single_institution_dominance() {
        let plan = plan_federated_glioma_sites(&request(vec![
            envelope("site-a", "group-a", &["organoid", "imaging"]),
            envelope("site-b", "group-a", &["organoid", "imaging"]),
            envelope("site-c", "group-b", &["organoid", "imaging"]),
        ]))
        .unwrap();
        assert!(plan.maximum_group_fraction_milli <= 500);
    }

    #[test]
    fn marginal_independence_breaks_soft_ties_between_institution_groups() {
        let mut selection_request = request(vec![
            envelope("site-a", "group-a", &[]),
            envelope("site-b", "group-a", &[]),
            envelope("site-c", "group-b", &[]),
        ]);
        selection_request.required_representation_tags.clear();
        selection_request.maximum_sites = 2;
        selection_request.maximum_same_group_fraction_milli = 1_000;

        let plan = plan_federated_glioma_sites(&selection_request).unwrap();

        assert_eq!(plan.selected_site_order, vec!["site-a", "site-c"]);
        let diversified = plan
            .scores
            .iter()
            .find(|score| score.site_id == "site-c")
            .unwrap();
        assert_eq!(diversified.independence_milli, 500);
        assert_eq!(diversified.marginal_bonus_milli, 100);
    }

    #[test]
    fn protected_data_boundary_is_rejected() {
        let mut protected = envelope("site-b", "group-b", &["imaging"]);
        protected.contains_human_data = true;
        assert!(plan_federated_glioma_sites(&request(vec![
            envelope("site-a", "group-a", &["organoid"]),
            protected,
        ]))
        .is_err());
    }

    #[test]
    fn deterministic_plan_repeats() {
        let input = request(vec![
            envelope("site-a", "group-a", &["organoid", "imaging"]),
            envelope("site-b", "group-b", &["organoid"]),
            envelope("site-c", "group-c", &["imaging"]),
        ]);
        assert_eq!(
            plan_federated_glioma_sites(&input).unwrap(),
            plan_federated_glioma_sites(&input).unwrap()
        );
    }

    #[test]
    fn portfolio_beam_prefers_two_site_coverage_over_myopic_high_score_fill() {
        let mut low_freshness_bridge = envelope("site-c", "group-c", &["imaging", "invasion"]);
        low_freshness_bridge.freshness_age_hours = 23;
        let mut selection_request = request(vec![
            envelope("site-a", "group-a", &["organoid"]),
            envelope("site-b", "group-b", &["imaging"]),
            low_freshness_bridge,
        ]);
        selection_request.required_representation_tags =
            vec!["organoid".into(), "imaging".into(), "invasion".into()];
        selection_request.maximum_sites = 3;
        let plan = plan_federated_glioma_sites(&selection_request).unwrap();
        assert_eq!(plan.selected_site_order, vec!["site-a", "site-c"]);
        assert!(!plan.selected_site_order.contains(&"site-b".into()));
        assert_eq!(plan.representation_coverage_milli, 1_000);
        assert_eq!(plan.disposition, SiteSelectionDisposition::Partial);
    }
}
