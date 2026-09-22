//! Aggregate-only multi-site outcome reconciliation for preclinical glioma research.
//!
//! This capability gives the autonomous engine a consortium-level scientific boundary without
//! moving raw measurements. Sites contribute typed, bounded summaries; the reconciler equalizes
//! site influence, estimates directional consensus and heterogeneity, runs leave-one-site-out
//! influence checks, and routes support, negative, contradiction, or coverage outcomes. A
//! consensus is never a clinical conclusion and an underpowered panel is never promoted.

use crate::glioma::evidence::EvidenceState;
use crate::glioma_engine::{GliomaModality, GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P01-F26";
pub const OUTPUT_SCHEMA: &str = "GliomaMultiSiteOutcomeReconciliation1@1";
pub const MAX_OBSERVATIONS: usize = 16_384;
pub const MAX_CLAIMS: usize = 4_096;
pub const MAX_SITES: usize = 1_024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiSiteOutcomeObservation {
    pub site_id: String,
    pub independence_group: String,
    pub observation_id: String,
    pub claim_key: String,
    pub scope_key: String,
    pub modality: GliomaModality,
    pub model_system: Option<GliomaModelSystem>,
    pub state: EvidenceState,
    pub effect_milli: i32,
    pub uncertainty_milli: u16,
    pub quality_milli: u16,
    pub reproducibility_milli: u16,
    pub sample_count: u32,
    pub release_epoch: u32,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiSiteOutcomeReconciliationRequest {
    pub objective: String,
    pub claim_terms: Vec<String>,
    pub scope_terms: Vec<String>,
    pub required_modalities: BTreeSet<GliomaModality>,
    pub required_model_systems: BTreeSet<GliomaModelSystem>,
    /// Minimum distinct modalities represented by each reconciled claim when no explicit
    /// `required_modalities` set is supplied.
    pub minimum_modalities: usize,
    /// Minimum distinct model systems represented by each reconciled claim when no explicit
    /// `required_model_systems` set is supplied.
    pub minimum_model_systems: usize,
    pub minimum_sites: usize,
    pub minimum_independent_sites: usize,
    pub minimum_consensus_milli: u16,
    pub minimum_effect_milli: u16,
    pub maximum_heterogeneity_milli: u16,
    pub maximum_leave_one_site_out_influence_milli: u16,
    pub minimum_quality_milli: u16,
    pub minimum_reproducibility_milli: u16,
    pub current_epoch: u32,
    pub maximum_age_epochs: u32,
    pub maximum_claims: usize,
    pub observations: Vec<MultiSiteOutcomeObservation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultiSiteOutcomeVerdict {
    ConsistentSupport,
    ConsistentNegative,
    Contradicted,
    Heterogeneous,
    Insufficient,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultiSiteOutcomeAction {
    PromoteToKnowledge,
    PreserveNegative,
    RouteContradiction,
    RouteReplication,
    AcquireCoverage,
    Hold,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiSiteOutcomeClaim {
    pub claim_id: String,
    pub rank: usize,
    pub claim_key: String,
    pub scope_key: String,
    pub observation_order: Vec<String>,
    pub site_order: Vec<String>,
    pub independence_group_order: Vec<String>,
    pub support_order: Vec<String>,
    pub negative_order: Vec<String>,
    pub contradicted_order: Vec<String>,
    pub uncertain_order: Vec<String>,
    pub site_count: usize,
    pub independent_site_count: usize,
    pub support_milli: u16,
    pub negative_milli: u16,
    pub contradiction_milli: u16,
    pub consensus_milli: u16,
    pub effect_milli: i32,
    pub uncertainty_milli: u16,
    pub heterogeneity_milli: u16,
    pub leave_one_site_out_influence_milli: u16,
    pub modality_coverage_milli: u16,
    pub model_coverage_milli: u16,
    pub coverage_milli: u16,
    pub quality_milli: u16,
    pub reproducibility_milli: u16,
    pub sample_count: u64,
    pub verdict: MultiSiteOutcomeVerdict,
    pub action: MultiSiteOutcomeAction,
    pub explanation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiSiteOutcomeSiteSummary {
    pub site_id: String,
    pub independence_group: String,
    pub observation_order: Vec<String>,
    pub claim_order: Vec<String>,
    pub modality_order: Vec<GliomaModality>,
    pub model_system_order: Vec<GliomaModelSystem>,
    pub state_counts: BTreeMap<String, usize>,
    pub mean_effect_milli: i32,
    pub mean_quality_milli: u16,
    pub mean_reproducibility_milli: u16,
    pub sample_count: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultiSiteOutcomeDisposition {
    Ready,
    Partial,
    NoClaims,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiSiteOutcomeOmission {
    pub observation_id: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiSiteOutcomeReconciliation {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub query_digest: ContentHash,
    pub observation_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub omitted_order: Vec<String>,
    pub claim_order: Vec<String>,
    pub claims: Vec<MultiSiteOutcomeClaim>,
    pub omissions: Vec<MultiSiteOutcomeOmission>,
    pub site_order: Vec<String>,
    pub site_summaries: Vec<MultiSiteOutcomeSiteSummary>,
    pub modality_counts: BTreeMap<String, usize>,
    pub model_system_counts: BTreeMap<String, usize>,
    pub state_counts: BTreeMap<String, usize>,
    pub negative_order: Vec<String>,
    pub contradicted_order: Vec<String>,
    pub uncertain_order: Vec<String>,
    pub disposition: MultiSiteOutcomeDisposition,
    pub next_route: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MultiSiteOutcomeReconciliationError {
    #[error("multi-site outcome reconciliation request is invalid: {0}")]
    InvalidRequest(String),
    #[error("multi-site outcome observation is invalid: {0}")]
    InvalidObservation(String),
    #[error("multi-site outcome reconciliation output is invalid: {0}")]
    InvalidOutput(String),
    #[error("multi-site outcome reconciliation digest failed: {0}")]
    Digest(String),
}

#[derive(Debug, Clone)]
struct SiteAggregate {
    site_id: String,
    independence_group: String,
    score_milli: i32,
    uncertainty_milli: u16,
    quality_milli: u16,
    reproducibility_milli: u16,
    sample_count: u64,
    weight: u128,
    support_fraction_milli: u16,
    negative_fraction_milli: u16,
    contradiction_fraction_milli: u16,
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_nonempty(values: &[String]) -> bool {
    values.iter().all(|value| !value.trim().is_empty())
        && values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

fn normalized_terms(values: &[String]) -> Vec<String> {
    values
        .iter()
        .flat_map(|value| {
            value
                .split(|character: char| !character.is_alphanumeric())
                .filter(|term| !term.is_empty())
                .map(|term| term.to_lowercase())
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn token_set(value: &str) -> BTreeSet<String> {
    value
        .split(|character: char| !character.is_alphanumeric())
        .filter(|term| !term.is_empty())
        .map(|term| term.to_lowercase())
        .collect()
}

fn label<T: Serialize>(value: T) -> String {
    serde_json::to_string(&value)
        .unwrap_or_default()
        .trim_matches('"')
        .to_string()
}

fn uncertain(state: EvidenceState) -> bool {
    matches!(
        state,
        EvidenceState::Unknown | EvidenceState::Stale | EvidenceState::Unmeasured
    )
}

fn state_signal(state: EvidenceState, effect_milli: i32) -> i32 {
    match state {
        EvidenceState::Supported => effect_milli.abs(),
        EvidenceState::Negative | EvidenceState::Contradicted => -effect_milli.abs(),
        EvidenceState::Unknown | EvidenceState::Stale | EvidenceState::Unmeasured => 0,
    }
}

fn observation_weight(observation: &MultiSiteOutcomeObservation) -> u128 {
    let uncertainty = u128::from(observation.uncertainty_milli.max(1));
    u128::from(observation.sample_count.min(10_000).max(1))
        .saturating_mul(u128::from(observation.quality_milli.max(1)))
        .saturating_mul(u128::from(observation.reproducibility_milli.max(1)))
        .saturating_mul(1_000)
        / uncertainty.saturating_mul(uncertainty)
}

fn mean_i32(values: impl Iterator<Item = i32>) -> i32 {
    let values = values.collect::<Vec<_>>();
    if values.is_empty() {
        0
    } else {
        (values.iter().map(|value| i128::from(*value)).sum::<i128>() / values.len() as i128) as i32
    }
}

fn mean_u16(values: impl Iterator<Item = u16>) -> u16 {
    let values = values.collect::<Vec<_>>();
    if values.is_empty() {
        0
    } else {
        (values.iter().map(|value| u32::from(*value)).sum::<u32>() / values.len() as u32) as u16
    }
}

fn weighted_score(aggregates: &[SiteAggregate]) -> i32 {
    let total = aggregates.iter().map(|site| site.weight).sum::<u128>();
    if total == 0 {
        0
    } else {
        (aggregates
            .iter()
            .map(|site| i128::from(site.score_milli) * site.weight as i128)
            .sum::<i128>()
            / total as i128) as i32
    }
}

fn weighted_fraction(
    aggregates: &[SiteAggregate],
    selector: impl Fn(&SiteAggregate) -> u16,
) -> u16 {
    let total = aggregates.iter().map(|site| site.weight).sum::<u128>();
    if total == 0 {
        0
    } else {
        (aggregates
            .iter()
            .map(|site| site.weight.saturating_mul(u128::from(selector(site))))
            .sum::<u128>()
            .saturating_div(total)
            .min(1_000)) as u16
    }
}

fn site_aggregates(observations: &[MultiSiteOutcomeObservation]) -> Vec<SiteAggregate> {
    let mut grouped = BTreeMap::<(String, String), Vec<&MultiSiteOutcomeObservation>>::new();
    for observation in observations {
        grouped
            .entry((
                observation.site_id.clone(),
                observation.independence_group.clone(),
            ))
            .or_default()
            .push(observation);
    }
    grouped
        .into_iter()
        .map(|((site_id, independence_group), observations)| {
            let total_weight = observations
                .iter()
                .map(|observation| observation_weight(observation))
                .sum::<u128>();
            let score_milli = if total_weight == 0 {
                0
            } else {
                (observations
                    .iter()
                    .map(|observation| {
                        i128::from(state_signal(observation.state, observation.effect_milli))
                            * observation_weight(observation) as i128
                    })
                    .sum::<i128>()
                    / total_weight as i128) as i32
            };
            let total_state_weight = observations
                .iter()
                .filter(|observation| !uncertain(observation.state))
                .map(|observation| observation_weight(observation))
                .sum::<u128>();
            let fraction = |state: EvidenceState| {
                if total_state_weight == 0 {
                    0
                } else {
                    (observations
                        .iter()
                        .filter(|observation| observation.state == state)
                        .map(|observation| observation_weight(observation))
                        .sum::<u128>()
                        .saturating_mul(1_000)
                        / total_state_weight) as u16
                }
            };
            SiteAggregate {
                site_id,
                independence_group,
                score_milli,
                uncertainty_milli: mean_u16(
                    observations
                        .iter()
                        .map(|observation| observation.uncertainty_milli),
                ),
                quality_milli: mean_u16(
                    observations
                        .iter()
                        .map(|observation| observation.quality_milli),
                ),
                reproducibility_milli: mean_u16(
                    observations
                        .iter()
                        .map(|observation| observation.reproducibility_milli),
                ),
                sample_count: observations
                    .iter()
                    .map(|observation| u64::from(observation.sample_count))
                    .sum(),
                weight: total_weight,
                support_fraction_milli: fraction(EvidenceState::Supported),
                negative_fraction_milli: fraction(EvidenceState::Negative),
                contradiction_fraction_milli: fraction(EvidenceState::Contradicted),
            }
        })
        .collect()
}

fn heterogeneity(aggregates: &[SiteAggregate], consensus: i32) -> u16 {
    let total = aggregates.iter().map(|site| site.weight).sum::<u128>();
    if total == 0 {
        1_000
    } else {
        (aggregates
            .iter()
            .map(|site| {
                site.weight
                    .saturating_mul(u128::from(site.score_milli.abs_diff(consensus)))
            })
            .sum::<u128>()
            .saturating_mul(1_000)
            .saturating_div(total)
            .saturating_div(2_000)
            .min(1_000)) as u16
    }
}

fn leave_one_out_influence(aggregates: &[SiteAggregate], consensus: i32) -> u16 {
    aggregates
        .iter()
        .enumerate()
        .map(|(index, _)| {
            let without = aggregates
                .iter()
                .enumerate()
                .filter_map(|(candidate_index, candidate)| {
                    (candidate_index != index).then_some(candidate.clone())
                })
                .collect::<Vec<_>>();
            weighted_score(&without).abs_diff(consensus)
        })
        .max()
        .unwrap_or(0)
        .saturating_mul(1_000)
        .checked_div(2_000)
        .unwrap_or(1_000)
        .min(1_000) as u16
}

fn digest_input(output: &MultiSiteOutcomeReconciliation) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "query_digest": output.query_digest,
        "observation_order": output.observation_order,
        "selected_order": output.selected_order,
        "omitted_order": output.omitted_order,
        "claim_order": output.claim_order,
        "claims": output.claims,
        "omissions": output.omissions,
        "site_order": output.site_order,
        "site_summaries": output.site_summaries,
        "modality_counts": output.modality_counts,
        "model_system_counts": output.model_system_counts,
        "state_counts": output.state_counts,
        "negative_order": output.negative_order,
        "contradicted_order": output.contradicted_order,
        "uncertain_order": output.uncertain_order,
        "disposition": output.disposition,
        "next_route": output.next_route,
    })
}

fn validate_request(
    request: &MultiSiteOutcomeReconciliationRequest,
) -> Result<(), MultiSiteOutcomeReconciliationError> {
    if request.objective.trim().is_empty()
        || request.minimum_sites == 0
        || request.minimum_sites > MAX_SITES
        || request.minimum_independent_sites == 0
        || request.minimum_independent_sites > request.minimum_sites
        || request.minimum_modalities == 0
        || request.minimum_modalities > 64
        || request.minimum_model_systems == 0
        || request.minimum_model_systems > 64
        || request.minimum_consensus_milli > 1_000
        || request.minimum_effect_milli > 1_000
        || request.maximum_heterogeneity_milli > 1_000
        || request.maximum_leave_one_site_out_influence_milli > 1_000
        || request.minimum_quality_milli > 1_000
        || request.minimum_reproducibility_milli > 1_000
        || request.current_epoch == 0
        || request.maximum_age_epochs == 0
        || request.maximum_claims == 0
        || request.maximum_claims > MAX_CLAIMS
        || request.observations.len() > MAX_OBSERVATIONS
        || request.claim_terms.len() + request.scope_terms.len() > 128
    {
        return Err(MultiSiteOutcomeReconciliationError::InvalidRequest(
            "objective, bounded site/claim/age/score thresholds, and observation capacity are required".into(),
        ));
    }
    let claim_terms = normalized_terms(&request.claim_terms);
    let scope_terms = normalized_terms(&request.scope_terms);
    if claim_terms.is_empty()
        && scope_terms.is_empty()
        && request.required_modalities.is_empty()
        && request.required_model_systems.is_empty()
    {
        return Err(MultiSiteOutcomeReconciliationError::InvalidRequest(
            "at least one claim/scope term or structured modality/model filter is required".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    let mut sites = BTreeSet::new();
    let mut site_groups = BTreeMap::<String, String>::new();
    for observation in &request.observations {
        if observation.site_id.trim().is_empty()
            || observation.independence_group.trim().is_empty()
            || observation.observation_id.trim().is_empty()
            || observation.claim_key.trim().is_empty()
            || observation.scope_key.trim().is_empty()
            || observation.effect_milli.abs() > 1_000
            || observation.uncertainty_milli == 0
            || observation.quality_milli > 1_000
            || observation.reproducibility_milli > 1_000
            || observation.sample_count == 0
            || observation.release_epoch > request.current_epoch
            || observation.artifact.validate().is_err()
            || !ids.insert(observation.observation_id.clone())
            || site_groups
                .get(&observation.site_id)
                .is_some_and(|group| group != &observation.independence_group)
        {
            return Err(MultiSiteOutcomeReconciliationError::InvalidObservation(
                "observations must be unique, bounded aggregate-only local artifacts with explicit site, independence, claim, scope, uncertainty, and sample declarations".into(),
            ));
        }
        sites.insert(observation.site_id.clone());
        site_groups
            .entry(observation.site_id.clone())
            .or_insert_with(|| observation.independence_group.clone());
    }
    if sites.len() > MAX_SITES {
        return Err(MultiSiteOutcomeReconciliationError::InvalidObservation(
            "site count exceeds the supported bound".into(),
        ));
    }
    Ok(())
}

fn claim_action(verdict: MultiSiteOutcomeVerdict) -> MultiSiteOutcomeAction {
    match verdict {
        MultiSiteOutcomeVerdict::ConsistentSupport => MultiSiteOutcomeAction::PromoteToKnowledge,
        MultiSiteOutcomeVerdict::ConsistentNegative => MultiSiteOutcomeAction::PreserveNegative,
        MultiSiteOutcomeVerdict::Contradicted => MultiSiteOutcomeAction::RouteContradiction,
        MultiSiteOutcomeVerdict::Heterogeneous => MultiSiteOutcomeAction::RouteReplication,
        MultiSiteOutcomeVerdict::Insufficient => MultiSiteOutcomeAction::AcquireCoverage,
        MultiSiteOutcomeVerdict::Unresolved => MultiSiteOutcomeAction::Hold,
    }
}

/// Reconcile aggregate-only preclinical outcomes across independent sites without moving raw data.
pub fn reconcile_glioma_multisite_outcomes(
    request: &MultiSiteOutcomeReconciliationRequest,
) -> Result<MultiSiteOutcomeReconciliation, MultiSiteOutcomeReconciliationError> {
    validate_request(request)?;
    let claim_terms = normalized_terms(&request.claim_terms);
    let scope_terms = normalized_terms(&request.scope_terms);
    let mut observation_order = request
        .observations
        .iter()
        .map(|observation| observation.observation_id.clone())
        .collect::<Vec<_>>();
    observation_order.sort();
    let mut modality_counts = BTreeMap::new();
    let mut model_system_counts = BTreeMap::new();
    let mut state_counts = BTreeMap::new();
    let mut negative_order = Vec::new();
    let mut contradicted_order = Vec::new();
    let mut uncertain_order = Vec::new();
    let mut site_records = BTreeMap::<String, Vec<&MultiSiteOutcomeObservation>>::new();
    let mut eligible = Vec::new();
    let mut omissions = Vec::new();
    for observation in &request.observations {
        *modality_counts
            .entry(label(observation.modality))
            .or_insert(0_usize) += 1;
        if let Some(model) = observation.model_system {
            *model_system_counts.entry(label(model)).or_insert(0_usize) += 1;
        }
        *state_counts
            .entry(label(observation.state))
            .or_insert(0_usize) += 1;
        match observation.state {
            EvidenceState::Negative => negative_order.push(observation.observation_id.clone()),
            EvidenceState::Contradicted => {
                contradicted_order.push(observation.observation_id.clone())
            }
            EvidenceState::Unknown | EvidenceState::Stale | EvidenceState::Unmeasured => {
                uncertain_order.push(observation.observation_id.clone())
            }
            EvidenceState::Supported => {}
        }
        let claim_tokens = token_set(&observation.claim_key);
        let scope_tokens = token_set(&observation.scope_key);
        if !claim_terms.is_empty() && !claim_terms.iter().any(|term| claim_tokens.contains(term)) {
            omissions.push(MultiSiteOutcomeOmission {
                observation_id: observation.observation_id.clone(),
                reason: "claim-term-mismatch".into(),
            });
            continue;
        }
        if !scope_terms.is_empty() && !scope_terms.iter().any(|term| scope_tokens.contains(term)) {
            omissions.push(MultiSiteOutcomeOmission {
                observation_id: observation.observation_id.clone(),
                reason: "scope-term-mismatch".into(),
            });
            continue;
        }
        if request
            .current_epoch
            .saturating_sub(observation.release_epoch)
            > request.maximum_age_epochs
        {
            omissions.push(MultiSiteOutcomeOmission {
                observation_id: observation.observation_id.clone(),
                reason: "maximum-age-filter".into(),
            });
            continue;
        }
        if observation.quality_milli < request.minimum_quality_milli {
            omissions.push(MultiSiteOutcomeOmission {
                observation_id: observation.observation_id.clone(),
                reason: "quality-floor".into(),
            });
            continue;
        }
        if observation.reproducibility_milli < request.minimum_reproducibility_milli {
            omissions.push(MultiSiteOutcomeOmission {
                observation_id: observation.observation_id.clone(),
                reason: "reproducibility-floor".into(),
            });
            continue;
        }
        if !request.required_modalities.is_empty()
            && !request.required_modalities.contains(&observation.modality)
        {
            omissions.push(MultiSiteOutcomeOmission {
                observation_id: observation.observation_id.clone(),
                reason: "modality-filter".into(),
            });
            continue;
        }
        if !request.required_model_systems.is_empty()
            && !observation
                .model_system
                .is_some_and(|model| request.required_model_systems.contains(&model))
        {
            omissions.push(MultiSiteOutcomeOmission {
                observation_id: observation.observation_id.clone(),
                reason: "model-system-filter".into(),
            });
            continue;
        }
        eligible.push(observation);
        site_records
            .entry(observation.site_id.clone())
            .or_default()
            .push(observation);
    }
    eligible.sort_by(|left, right| left.observation_id.cmp(&right.observation_id));
    let site_summaries = site_records
        .iter()
        .map(|(site_id, records)| {
            let first_group = records
                .iter()
                .map(|record| record.independence_group.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .next()
                .unwrap_or_default();
            MultiSiteOutcomeSiteSummary {
                site_id: site_id.clone(),
                independence_group: first_group,
                observation_order: records
                    .iter()
                    .map(|record| record.observation_id.clone())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect(),
                claim_order: records
                    .iter()
                    .map(|record| format!("{}::{}", record.claim_key, record.scope_key))
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect(),
                modality_order: records
                    .iter()
                    .map(|record| record.modality)
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect(),
                model_system_order: records
                    .iter()
                    .filter_map(|record| record.model_system)
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect(),
                state_counts: records.iter().fold(BTreeMap::new(), |mut counts, record| {
                    *counts.entry(label(record.state)).or_insert(0) += 1;
                    counts
                }),
                mean_effect_milli: mean_i32(records.iter().map(|record| record.effect_milli)),
                mean_quality_milli: mean_u16(records.iter().map(|record| record.quality_milli)),
                mean_reproducibility_milli: mean_u16(
                    records.iter().map(|record| record.reproducibility_milli),
                ),
                sample_count: records
                    .iter()
                    .map(|record| u64::from(record.sample_count))
                    .sum(),
            }
        })
        .collect::<Vec<_>>();
    let mut grouped = BTreeMap::<(String, String), Vec<MultiSiteOutcomeObservation>>::new();
    for observation in eligible.iter().copied() {
        grouped
            .entry((observation.claim_key.clone(), observation.scope_key.clone()))
            .or_default()
            .push(observation.clone());
    }
    let mut claims = Vec::new();
    for ((claim_key, scope_key), observations) in grouped {
        let aggregates = site_aggregates(&observations);
        let site_order = aggregates
            .iter()
            .map(|site| site.site_id.clone())
            .collect::<BTreeSet<_>>();
        let independence_group_order = aggregates
            .iter()
            .map(|site| site.independence_group.clone())
            .collect::<BTreeSet<_>>();
        let effect_milli = weighted_score(&aggregates);
        let support_milli = weighted_fraction(&aggregates, |site| site.support_fraction_milli);
        let negative_milli = weighted_fraction(&aggregates, |site| site.negative_fraction_milli);
        let contradiction_milli =
            weighted_fraction(&aggregates, |site| site.contradiction_fraction_milli);
        let consensus_milli = support_milli.max(negative_milli).max(contradiction_milli);
        let heterogeneity_milli = heterogeneity(&aggregates, effect_milli);
        let leave_one_site_out_influence_milli = leave_one_out_influence(&aggregates, effect_milli);
        let modality_set = observations
            .iter()
            .map(|observation| observation.modality)
            .collect::<BTreeSet<_>>();
        let model_set = observations
            .iter()
            .filter_map(|observation| observation.model_system)
            .collect::<BTreeSet<_>>();
        let modality_coverage_milli = if request.required_modalities.is_empty() {
            ((modality_set.len() as u32 * 1_000) / request.minimum_modalities as u32) as u16
        } else {
            ((request
                .required_modalities
                .iter()
                .filter(|modality| modality_set.contains(modality))
                .count() as u32
                * 1_000)
                / request.required_modalities.len() as u32) as u16
        }
        .min(1_000);
        let model_coverage_milli = if request.required_model_systems.is_empty() {
            ((model_set.len() as u32 * 1_000) / request.minimum_model_systems as u32) as u16
        } else {
            ((request
                .required_model_systems
                .iter()
                .filter(|model| model_set.contains(model))
                .count() as u32
                * 1_000)
                / request.required_model_systems.len() as u32) as u16
        }
        .min(1_000);
        let site_coverage_milli =
            ((site_order.len() as u32 * 1_000) / request.minimum_sites as u32).min(1_000) as u16;
        let independent_coverage_milli = ((independence_group_order.len() as u32 * 1_000)
            / request.minimum_independent_sites as u32)
            .min(1_000) as u16;
        let coverage_milli = ((u32::from(site_coverage_milli)
            + u32::from(independent_coverage_milli)
            + u32::from(modality_coverage_milli)
            + u32::from(model_coverage_milli))
            / 4) as u16;
        let quality_milli = mean_u16(aggregates.iter().map(|site| site.quality_milli));
        let reproducibility_milli =
            mean_u16(aggregates.iter().map(|site| site.reproducibility_milli));
        let sample_count = aggregates.iter().map(|site| site.sample_count).sum::<u64>();
        let support_order = observations
            .iter()
            .filter(|observation| observation.state == EvidenceState::Supported)
            .map(|observation| observation.observation_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let negative_order_claim = observations
            .iter()
            .filter(|observation| observation.state == EvidenceState::Negative)
            .map(|observation| observation.observation_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let contradicted_order_claim = observations
            .iter()
            .filter(|observation| observation.state == EvidenceState::Contradicted)
            .map(|observation| observation.observation_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let uncertain_order_claim = observations
            .iter()
            .filter(|observation| uncertain(observation.state))
            .map(|observation| observation.observation_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let verdict = if site_order.len() < request.minimum_sites
            || independence_group_order.len() < request.minimum_independent_sites
            || modality_coverage_milli < 1_000
            || model_coverage_milli < 1_000
        {
            MultiSiteOutcomeVerdict::Insufficient
        } else if contradiction_milli > 0 {
            // Any eligible contradictory site is a safety-relevant routing signal. It must not
            // be diluted by a larger supportive panel; the contradiction cut decides whether the
            // disagreement is a scope, quality, or biological explanation.
            MultiSiteOutcomeVerdict::Contradicted
        } else if heterogeneity_milli > request.maximum_heterogeneity_milli
            || leave_one_site_out_influence_milli
                > request.maximum_leave_one_site_out_influence_milli
            || (support_milli >= 300 && negative_milli >= 300)
        {
            MultiSiteOutcomeVerdict::Heterogeneous
        } else if support_milli >= request.minimum_consensus_milli
            && effect_milli.unsigned_abs() >= u32::from(request.minimum_effect_milli)
        {
            MultiSiteOutcomeVerdict::ConsistentSupport
        } else if negative_milli >= request.minimum_consensus_milli
            && effect_milli.unsigned_abs() >= u32::from(request.minimum_effect_milli)
        {
            MultiSiteOutcomeVerdict::ConsistentNegative
        } else {
            MultiSiteOutcomeVerdict::Unresolved
        };
        let action = claim_action(verdict);
        let explanation = format!(
            "sites={};independent-sites={};support={support_milli};negative={negative_milli};contradiction={contradiction_milli};effect={effect_milli};heterogeneity={heterogeneity_milli};leave-one-site-out={leave_one_site_out_influence_milli};coverage={coverage_milli}",
            site_order.len(),
            independence_group_order.len()
        );
        claims.push(MultiSiteOutcomeClaim {
            claim_id: format!("{claim_key}::{scope_key}"),
            rank: 0,
            claim_key,
            scope_key,
            observation_order: observations
                .iter()
                .map(|observation| observation.observation_id.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
            site_order: site_order.into_iter().collect(),
            independence_group_order: independence_group_order.into_iter().collect(),
            support_order,
            negative_order: negative_order_claim,
            contradicted_order: contradicted_order_claim,
            uncertain_order: uncertain_order_claim,
            site_count: aggregates.len(),
            independent_site_count: aggregates
                .iter()
                .map(|site| site.independence_group.clone())
                .collect::<BTreeSet<_>>()
                .len(),
            support_milli,
            negative_milli,
            contradiction_milli,
            consensus_milli,
            effect_milli,
            uncertainty_milli: mean_u16(aggregates.iter().map(|site| site.uncertainty_milli)),
            heterogeneity_milli,
            leave_one_site_out_influence_milli,
            modality_coverage_milli,
            model_coverage_milli,
            coverage_milli,
            quality_milli,
            reproducibility_milli,
            sample_count,
            verdict,
            action,
            explanation,
        });
    }
    claims.sort_by(|left, right| {
        right
            .consensus_milli
            .saturating_mul(left.coverage_milli)
            .cmp(&left.consensus_milli.saturating_mul(right.coverage_milli))
            .then_with(|| left.claim_id.cmp(&right.claim_id))
    });
    claims.truncate(request.maximum_claims);
    for (rank, claim) in claims.iter_mut().enumerate() {
        claim.rank = rank + 1;
    }
    let selected_order = claims
        .iter()
        .flat_map(|claim| claim.observation_order.iter().cloned())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let selected_ids = selected_order.iter().cloned().collect::<BTreeSet<_>>();
    for observation in &request.observations {
        if selected_ids.contains(&observation.observation_id)
            || omissions
                .iter()
                .any(|omission| omission.observation_id == observation.observation_id)
        {
            continue;
        }
        omissions.push(MultiSiteOutcomeOmission {
            observation_id: observation.observation_id.clone(),
            reason: "maximum-claims-truncation".into(),
        });
    }
    omissions.sort_by(|left, right| left.observation_id.cmp(&right.observation_id));
    omissions.dedup_by(|left, right| left.observation_id == right.observation_id);
    let omitted_order = omissions
        .iter()
        .map(|omission| omission.observation_id.clone())
        .collect::<Vec<_>>();
    let claim_order = claims
        .iter()
        .map(|claim| claim.claim_id.clone())
        .collect::<Vec<_>>();
    let site_order = site_summaries
        .iter()
        .map(|summary| summary.site_id.clone())
        .collect::<Vec<_>>();
    let disposition = if request.observations.is_empty() {
        MultiSiteOutcomeDisposition::Blocked
    } else if claims.is_empty() {
        MultiSiteOutcomeDisposition::NoClaims
    } else if claims.iter().all(|claim| {
        matches!(
            claim.verdict,
            MultiSiteOutcomeVerdict::ConsistentSupport
                | MultiSiteOutcomeVerdict::ConsistentNegative
        )
    }) && omissions.is_empty()
    {
        MultiSiteOutcomeDisposition::Ready
    } else {
        MultiSiteOutcomeDisposition::Partial
    };
    let next_route = if claims.is_empty() {
        "glioma_federated_evidence_acquisition_policy"
    } else if claims
        .iter()
        .any(|claim| claim.verdict == MultiSiteOutcomeVerdict::Contradicted)
    {
        "plan_glioma_evidence_contradiction_cut"
    } else if claims.iter().any(|claim| {
        matches!(
            claim.verdict,
            MultiSiteOutcomeVerdict::Insufficient
                | MultiSiteOutcomeVerdict::Heterogeneous
                | MultiSiteOutcomeVerdict::Unresolved
        )
    }) {
        "glioma_federated_evidence_acquisition_policy"
    } else {
        "glioma_knowledge_protocol_gateway"
    };
    let query_digest = ContentHash::of_value(&serde_json::json!({
        "objective": request.objective,
        "claim_terms": claim_terms,
        "scope_terms": scope_terms,
        "required_modalities": request.required_modalities,
        "required_model_systems": request.required_model_systems,
        "minimum_modalities": request.minimum_modalities,
        "minimum_model_systems": request.minimum_model_systems,
        "minimum_sites": request.minimum_sites,
        "minimum_independent_sites": request.minimum_independent_sites,
        "minimum_consensus_milli": request.minimum_consensus_milli,
        "minimum_effect_milli": request.minimum_effect_milli,
        "maximum_heterogeneity_milli": request.maximum_heterogeneity_milli,
        "maximum_leave_one_site_out_influence_milli": request.maximum_leave_one_site_out_influence_milli,
        "minimum_quality_milli": request.minimum_quality_milli,
        "minimum_reproducibility_milli": request.minimum_reproducibility_milli,
        "current_epoch": request.current_epoch,
        "maximum_age_epochs": request.maximum_age_epochs,
        "maximum_claims": request.maximum_claims,
        "observation_order": observation_order,
    }))
    .map_err(|error| MultiSiteOutcomeReconciliationError::Digest(error.to_string()))?;
    let mut output = MultiSiteOutcomeReconciliation {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        query_digest,
        observation_order,
        selected_order,
        omitted_order,
        claim_order,
        claims,
        omissions,
        site_order,
        site_summaries,
        modality_counts,
        model_system_counts,
        state_counts,
        negative_order,
        contradicted_order,
        uncertain_order,
        disposition,
        next_route: next_route.into(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-multisite-outcomes"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| MultiSiteOutcomeReconciliationError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

impl MultiSiteOutcomeReconciliation {
    pub fn validate(&self) -> Result<(), MultiSiteOutcomeReconciliationError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.query_digest.as_str().len() != 64
            || !canonical(&self.observation_order)
            || !unique_nonempty(&self.observation_order)
            || !unique_nonempty(&self.selected_order)
            || !canonical(&self.omitted_order)
            || !unique_nonempty(&self.omitted_order)
            || self.claim_order
                != self
                    .claims
                    .iter()
                    .map(|claim| claim.claim_id.clone())
                    .collect::<Vec<_>>()
            || !unique_nonempty(&self.claim_order)
            || self.claims.iter().enumerate().any(|(index, claim)| {
                claim.rank != index + 1
                    || claim.claim_id != format!("{}::{}", claim.claim_key, claim.scope_key)
                    || claim.observation_order.is_empty()
                    || !canonical(&claim.observation_order)
                    || !unique_nonempty(&claim.observation_order)
                    || !canonical(&claim.site_order)
                    || !unique_nonempty(&claim.site_order)
                    || !canonical(&claim.independence_group_order)
                    || !unique_nonempty(&claim.independence_group_order)
                    || !canonical(&claim.support_order)
                    || !canonical(&claim.negative_order)
                    || !canonical(&claim.contradicted_order)
                    || !canonical(&claim.uncertain_order)
                    || claim.site_count != claim.site_order.len()
                    || claim.independent_site_count != claim.independence_group_order.len()
                    || claim.support_milli > 1_000
                    || claim.negative_milli > 1_000
                    || claim.contradiction_milli > 1_000
                    || claim.consensus_milli > 1_000
                    || claim.effect_milli.abs() > 1_000
                    || claim.uncertainty_milli == 0
                    || claim.heterogeneity_milli > 1_000
                    || claim.leave_one_site_out_influence_milli > 1_000
                    || claim.modality_coverage_milli > 1_000
                    || claim.model_coverage_milli > 1_000
                    || claim.coverage_milli > 1_000
                    || claim.quality_milli > 1_000
                    || claim.reproducibility_milli > 1_000
                    || claim.sample_count == 0
                    || claim.explanation.trim().is_empty()
            })
            || !canonical(&self.site_order)
            || !unique_nonempty(&self.site_order)
            || self
                .site_summaries
                .iter()
                .map(|summary| summary.site_id.clone())
                .collect::<Vec<_>>()
                != self.site_order
            || self.site_summaries.iter().any(|summary| {
                summary.site_id.trim().is_empty()
                    || summary.independence_group.trim().is_empty()
                    || !canonical(&summary.observation_order)
                    || !unique_nonempty(&summary.observation_order)
                    || !canonical(&summary.claim_order)
                    || !canonical(&summary.modality_order)
                    || !canonical(&summary.model_system_order)
                    || !canonical(&summary.state_counts.keys().cloned().collect::<Vec<_>>())
                    || summary.mean_effect_milli.abs() > 1_000
                    || summary.mean_quality_milli > 1_000
                    || summary.mean_reproducibility_milli > 1_000
                    || summary.sample_count == 0
            })
            || !canonical(&self.modality_counts.keys().cloned().collect::<Vec<_>>())
            || !canonical(&self.model_system_counts.keys().cloned().collect::<Vec<_>>())
            || !canonical(&self.state_counts.keys().cloned().collect::<Vec<_>>())
            || !canonical(&self.negative_order)
            || !canonical(&self.contradicted_order)
            || !canonical(&self.uncertain_order)
            || self.next_route.trim().is_empty()
        {
            return Err(MultiSiteOutcomeReconciliationError::InvalidOutput(
                "identity, claim/site ordering, coverage bounds, or summary shape is invalid"
                    .into(),
            ));
        }
        let observation_ids = self
            .observation_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let selected_ids = self.selected_order.iter().cloned().collect::<BTreeSet<_>>();
        let omitted_ids = self.omitted_order.iter().cloned().collect::<BTreeSet<_>>();
        let claim_observation_ids = self
            .claims
            .iter()
            .flat_map(|claim| claim.observation_order.iter().cloned())
            .collect::<Vec<_>>();
        let omission_ids = self
            .omissions
            .iter()
            .map(|omission| omission.observation_id.clone())
            .collect::<BTreeSet<_>>();
        if selected_ids.len() + omitted_ids.len() != observation_ids.len()
            || selected_ids
                .union(&omitted_ids)
                .cloned()
                .collect::<BTreeSet<_>>()
                != observation_ids
            || selected_ids
                != claim_observation_ids
                    .iter()
                    .cloned()
                    .collect::<BTreeSet<_>>()
            || claim_observation_ids.len() != selected_ids.len()
            || omission_ids != omitted_ids
            || self.omissions.len() != omitted_ids.len()
            || self
                .omissions
                .iter()
                .map(|omission| omission.observation_id.clone())
                .collect::<Vec<_>>()
                != self.omitted_order
            || self.omissions.iter().any(|omission| {
                omission.observation_id.trim().is_empty() || omission.reason.trim().is_empty()
            })
            || self.modality_counts.values().any(|count| *count == 0)
            || self.model_system_counts.values().any(|count| *count == 0)
            || self.state_counts.values().any(|count| *count == 0)
        {
            return Err(MultiSiteOutcomeReconciliationError::InvalidOutput(
                "observation/claim/omission partition or facet counts do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| MultiSiteOutcomeReconciliationError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(MultiSiteOutcomeReconciliationError::Digest(
                "multi-site outcome reconciliation digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: id.into(),
            content_hash: ContentHash::of_bytes(id.as_bytes()),
            content_type: "application/vnd.aurora.glioma-site-summary+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn observation(
        id: &str,
        site_id: &str,
        group: &str,
        modality: GliomaModality,
        state: EvidenceState,
        effect_milli: i32,
    ) -> MultiSiteOutcomeObservation {
        MultiSiteOutcomeObservation {
            site_id: site_id.into(),
            independence_group: group.into(),
            observation_id: id.into(),
            claim_key: "egfr-invasion".into(),
            scope_key: "organoid".into(),
            modality,
            model_system: Some(GliomaModelSystem::Organoid),
            state,
            effect_milli,
            uncertainty_milli: 100,
            quality_milli: 900,
            reproducibility_milli: 850,
            sample_count: 20,
            release_epoch: 5,
            artifact: artifact(&format!("artifact-{id}")),
        }
    }

    fn request() -> MultiSiteOutcomeReconciliationRequest {
        MultiSiteOutcomeReconciliationRequest {
            objective: "reconcile EGFR invasion outcomes across independent glioma sites".into(),
            claim_terms: vec!["invasion".into()],
            scope_terms: vec!["organoid".into()],
            required_modalities: BTreeSet::new(),
            required_model_systems: [GliomaModelSystem::Organoid].into_iter().collect(),
            minimum_modalities: 1,
            minimum_model_systems: 1,
            minimum_sites: 3,
            minimum_independent_sites: 3,
            minimum_consensus_milli: 700,
            minimum_effect_milli: 100,
            maximum_heterogeneity_milli: 250,
            maximum_leave_one_site_out_influence_milli: 250,
            minimum_quality_milli: 500,
            minimum_reproducibility_milli: 500,
            current_epoch: 5,
            maximum_age_epochs: 10,
            maximum_claims: 8,
            observations: vec![
                observation(
                    "obs-a",
                    "site-a",
                    "consortium-a",
                    GliomaModality::Imaging,
                    EvidenceState::Supported,
                    700,
                ),
                observation(
                    "obs-b",
                    "site-b",
                    "consortium-b",
                    GliomaModality::Transcriptomics,
                    EvidenceState::Supported,
                    650,
                ),
                observation(
                    "obs-c",
                    "site-c",
                    "consortium-c",
                    GliomaModality::Proteomics,
                    EvidenceState::Supported,
                    680,
                ),
            ],
        }
    }

    #[test]
    fn reconciles_consistent_support_with_leave_one_site_out_control() {
        let report = reconcile_glioma_multisite_outcomes(&request()).unwrap();
        assert_eq!(report.disposition, MultiSiteOutcomeDisposition::Ready);
        assert_eq!(report.claims.len(), 1);
        assert_eq!(
            report.claims[0].verdict,
            MultiSiteOutcomeVerdict::ConsistentSupport
        );
        assert_eq!(report.claims[0].independent_site_count, 3);
        assert_eq!(
            report.claims[0].action,
            MultiSiteOutcomeAction::PromoteToKnowledge
        );
        report.validate().unwrap();
    }

    #[test]
    fn contradiction_routes_to_resolution_without_silent_pooling() {
        let mut request = request();
        request.observations.push(observation(
            "obs-d",
            "site-d",
            "consortium-d",
            GliomaModality::Spatial,
            EvidenceState::Contradicted,
            700,
        ));
        let report = reconcile_glioma_multisite_outcomes(&request).unwrap();
        assert_eq!(
            report.claims[0].verdict,
            MultiSiteOutcomeVerdict::Contradicted
        );
        assert_eq!(
            report.claims[0].action,
            MultiSiteOutcomeAction::RouteContradiction
        );
        assert_eq!(report.next_route, "plan_glioma_evidence_contradiction_cut");
        assert!(report.contradicted_order.contains(&"obs-d".to_string()));
        report.validate().unwrap();
    }

    #[test]
    fn underpowered_site_quorum_is_an_explicit_claim_state() {
        let mut request = request();
        request.minimum_sites = 4;
        request.minimum_independent_sites = 4;
        let report = reconcile_glioma_multisite_outcomes(&request).unwrap();
        assert_eq!(
            report.claims[0].verdict,
            MultiSiteOutcomeVerdict::Insufficient
        );
        assert_eq!(
            report.claims[0].action,
            MultiSiteOutcomeAction::AcquireCoverage
        );
        assert_eq!(
            report.next_route,
            "glioma_federated_evidence_acquisition_policy"
        );
        report.validate().unwrap();
    }

    #[test]
    fn repeated_observations_from_one_site_are_equalized_into_one_site_vote() {
        let mut request = request();
        request.observations.push(observation(
            "obs-a2",
            "site-a",
            "consortium-a",
            GliomaModality::Imaging,
            EvidenceState::Supported,
            600,
        ));
        let report = reconcile_glioma_multisite_outcomes(&request).unwrap();
        assert_eq!(report.claims[0].site_count, 3);
        assert_eq!(report.claims[0].independent_site_count, 3);
        assert_eq!(report.site_summaries[0].observation_order.len(), 2);
        report.validate().unwrap();
    }

    #[test]
    fn empty_surface_is_blocked_and_replays_deterministically() {
        let mut request = request();
        request.observations.clear();
        let first = reconcile_glioma_multisite_outcomes(&request).unwrap();
        let second = reconcile_glioma_multisite_outcomes(&request).unwrap();
        assert_eq!(first.disposition, MultiSiteOutcomeDisposition::Blocked);
        assert_eq!(first.digest, second.digest);
        assert_eq!(
            first.next_route,
            "glioma_federated_evidence_acquisition_policy"
        );
        first.validate().unwrap();
    }
}
