//! Mechanism identifiability frontier for preclinical glioma research.
//!
//! A mechanism ranking can look decisive even when several mechanisms make nearly identical
//! predictions. This feature exposes that ambiguity explicitly: it computes pairwise separation
//! from typed local feature predictions, then searches a bounded portfolio of useful affordable features
//! for unresolved pairs. It is a planning/analysis artifact and never turns model predictions
//! into observed biology or a clinical recommendation.

use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P05-F02";
pub const OUTPUT_SCHEMA: &str = "GliomaMechanismIdentifiability1@2";
pub const MAX_MECHANISMS: usize = 256;
pub const MAX_FEATURES: usize = 4_096;
pub const SCORE_SCALE: u64 = 1_000_000;
const IDENTIFIABILITY_BEAM_WIDTH: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentifiabilityMechanism {
    pub mechanism_id: String,
    pub label: String,
    pub prior_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentifiabilityFeature {
    pub feature_id: String,
    pub label: String,
    pub value_milli_by_mechanism: BTreeMap<String, i32>,
    pub quality_milli: u16,
    pub cost_units: u32,
    pub risk_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismIdentifiabilityRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub mechanisms: Vec<IdentifiabilityMechanism>,
    pub features: Vec<IdentifiabilityFeature>,
    pub identifiability_floor_milli: u64,
    pub min_feature_quality_milli: u16,
    pub risk_ceiling_milli: u16,
    pub budget_units: u64,
    pub max_selected_features: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismPairIdentifiability {
    pub pair_id: String,
    pub left_mechanism_id: String,
    pub right_mechanism_id: String,
    pub separation_milli: u64,
    pub discriminating_feature_order: Vec<String>,
    pub status: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdentifiabilityFeatureUtility {
    pub feature_id: String,
    pub label: String,
    pub marginal_separation_milli: u64,
    pub covered_pair_order: Vec<String>,
    pub projected_cost_units: u64,
    pub action: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismIdentifiabilityDisposition {
    Qualified,
    Partial,
    NoEligibleFeatures,
    BudgetBlocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismIdentifiability {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub mechanism_order: Vec<String>,
    pub feature_order: Vec<String>,
    pub selected_feature_order: Vec<String>,
    pub quality_blocked_order: Vec<String>,
    pub risk_blocked_order: Vec<String>,
    pub budget_blocked_order: Vec<String>,
    pub pairs: Vec<MechanismPairIdentifiability>,
    pub feature_utilities: Vec<IdentifiabilityFeatureUtility>,
    pub unresolved_pair_order: Vec<String>,
    pub budget_remaining_units: u64,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: MechanismIdentifiabilityDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MechanismIdentifiabilityError {
    #[error("mechanism identifiability request is invalid: {0}")]
    InvalidRequest(String),
    #[error("mechanism identifiability input is invalid: {0}")]
    InvalidInput(String),
    #[error("mechanism identifiability output is invalid: {0}")]
    InvalidOutput(String),
    #[error("mechanism identifiability digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &MechanismIdentifiability) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "model_system": output.model_system,
        "mechanism_order": output.mechanism_order,
        "feature_order": output.feature_order,
        "selected_feature_order": output.selected_feature_order,
        "quality_blocked_order": output.quality_blocked_order,
        "risk_blocked_order": output.risk_blocked_order,
        "budget_blocked_order": output.budget_blocked_order,
        "pairs": output.pairs,
        "feature_utilities": output.feature_utilities,
        "unresolved_pair_order": output.unresolved_pair_order,
        "budget_remaining_units": output.budget_remaining_units,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn validate_request(
    request: &MechanismIdentifiabilityRequest,
) -> Result<(), MechanismIdentifiabilityError> {
    if request.objective.trim().is_empty()
        || request.mechanisms.len() < 2
        || request.mechanisms.len() > MAX_MECHANISMS
        || request.features.is_empty()
        || request.features.len() > MAX_FEATURES
        || request.identifiability_floor_milli == 0
        || request.identifiability_floor_milli > SCORE_SCALE
        || request.min_feature_quality_milli > 1_000
        || request.risk_ceiling_milli > 1_000
        || request.budget_units == 0
        || request.max_selected_features == 0
        || request.max_selected_features > request.features.len()
    {
        return Err(MechanismIdentifiabilityError::InvalidRequest(
            "objective, bounded mechanisms/features, positive identifiability/budget/selection limits, and finite quality/risk gates are required".into(),
        ));
    }
    let mut mechanism_ids = BTreeSet::new();
    let mut prior_sum = 0_u32;
    for mechanism in &request.mechanisms {
        if mechanism.mechanism_id.trim().is_empty()
            || mechanism.label.trim().is_empty()
            || mechanism.prior_milli == 0
            || !mechanism_ids.insert(mechanism.mechanism_id.clone())
        {
            return Err(MechanismIdentifiabilityError::InvalidInput(
                "mechanism identities, labels, positive priors, and uniqueness are required".into(),
            ));
        }
        prior_sum = prior_sum.saturating_add(u32::from(mechanism.prior_milli));
    }
    if prior_sum != 1_000 {
        return Err(MechanismIdentifiabilityError::InvalidInput(
            "mechanism priors must sum to exactly 1000 milli-units".into(),
        ));
    }
    let mut feature_ids = BTreeSet::new();
    for feature in &request.features {
        if feature.feature_id.trim().is_empty()
            || feature.label.trim().is_empty()
            || feature.cost_units == 0
            || feature.quality_milli > 1_000
            || feature.risk_milli > 1_000
            || !feature_ids.insert(feature.feature_id.clone())
            || feature.value_milli_by_mechanism.len() != mechanism_ids.len()
            || feature
                .value_milli_by_mechanism
                .keys()
                .any(|id| !mechanism_ids.contains(id))
        {
            return Err(MechanismIdentifiabilityError::InvalidInput(
                "feature identity, cost, quality/risk bounds, complete mechanism values, and uniqueness are required".into(),
            ));
        }
    }
    Ok(())
}

fn validate_output(output: &MechanismIdentifiability) -> Result<(), MechanismIdentifiabilityError> {
    if output.feature_id != FEATURE_ID
        || output.output_schema != OUTPUT_SCHEMA
        || output.objective.trim().is_empty()
        || !canonical(&output.mechanism_order)
        || !canonical(&output.feature_order)
        || !canonical(&output.selected_feature_order)
        || !canonical(&output.quality_blocked_order)
        || !canonical(&output.risk_blocked_order)
        || !canonical(&output.budget_blocked_order)
        || !canonical(&output.unresolved_pair_order)
        || !canonical(&output.negative_evidence)
        || !canonical(&output.uncertainty)
        || output
            .pairs
            .windows(2)
            .any(|pair| pair[0].pair_id >= pair[1].pair_id)
        || output
            .feature_utilities
            .windows(2)
            .any(|pair| pair[0].feature_id >= pair[1].feature_id)
        || output.pairs.iter().any(|pair| {
            pair.pair_id.trim().is_empty()
                || pair.left_mechanism_id.trim().is_empty()
                || pair.right_mechanism_id.trim().is_empty()
                || pair
                    .discriminating_feature_order
                    .windows(2)
                    .any(|window| window[0] >= window[1])
                || pair.status.trim().is_empty()
        })
    {
        return Err(MechanismIdentifiabilityError::InvalidOutput(
            "identity, canonical ordering, pair, feature, or status invariants are invalid".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(output))
        .map_err(|error| MechanismIdentifiabilityError::Digest(error.to_string()))?;
    if expected != output.digest {
        return Err(MechanismIdentifiabilityError::InvalidOutput(
            "digest is not bound to the mechanism identifiability frontier".into(),
        ));
    }
    Ok(())
}

impl MechanismIdentifiability {
    pub fn validate(&self) -> Result<(), MechanismIdentifiabilityError> {
        validate_output(self)
    }
}

fn pair_id(left: &str, right: &str) -> String {
    format!("{left}__{right}")
}

fn separation(feature: &IdentifiabilityFeature, left: &str, right: &str) -> u64 {
    let left_value = feature
        .value_milli_by_mechanism
        .get(left)
        .copied()
        .unwrap_or(0);
    let right_value = feature
        .value_milli_by_mechanism
        .get(right)
        .copied()
        .unwrap_or(0);
    u64::from(left_value.abs_diff(right_value)).saturating_mul(u64::from(feature.quality_milli))
        / 1_000
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct IdentifiabilityState {
    selected: Vec<String>,
    pair_scores: BTreeMap<String, u64>,
    resolved_pairs: BTreeSet<String>,
    spent_units: u64,
}

fn state_score(state: &IdentifiabilityState) -> u128 {
    let separation = state
        .pair_scores
        .values()
        .copied()
        .map(u128::from)
        .sum::<u128>();
    (state.resolved_pairs.len() as u128)
        .saturating_mul(1_000_000_000_000)
        .saturating_add(separation.saturating_mul(1_000_000))
        .saturating_add((state.selected.len() as u128).saturating_mul(1_000))
        .saturating_sub(u128::from(state.spent_units))
}

fn marginal_gain(
    feature_id: &str,
    state: &IdentifiabilityState,
    feature_contributions: &BTreeMap<String, Vec<(String, u64)>>,
    floor: u64,
) -> (u64, Vec<String>) {
    let mut gain = 0_u64;
    let mut covered = Vec::new();
    for (pair_id, contribution) in feature_contributions.get(feature_id).into_iter().flatten() {
        let current = state.pair_scores.get(pair_id).copied().unwrap_or(0);
        if current >= floor {
            continue;
        }
        let marginal = (*contribution).min(floor.saturating_sub(current));
        gain = gain.saturating_add(marginal);
        if marginal > 0 {
            covered.push(pair_id.clone());
        }
    }
    (gain, covered)
}

/// Compute unresolved mechanism pairs and select a bounded, budget-aware discriminating portfolio.
pub fn analyze_glioma_mechanism_identifiability(
    request: &MechanismIdentifiabilityRequest,
) -> Result<MechanismIdentifiability, MechanismIdentifiabilityError> {
    validate_request(request)?;
    let mechanism_order = request
        .mechanisms
        .iter()
        .map(|mechanism| mechanism.mechanism_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let feature_order = request
        .features
        .iter()
        .map(|feature| feature.feature_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut quality_blocked_order = request
        .features
        .iter()
        .filter(|feature| feature.quality_milli < request.min_feature_quality_milli)
        .map(|feature| feature.feature_id.clone())
        .collect::<Vec<_>>();
    let mut risk_blocked_order = request
        .features
        .iter()
        .filter(|feature| feature.risk_milli > request.risk_ceiling_milli)
        .map(|feature| feature.feature_id.clone())
        .collect::<Vec<_>>();
    quality_blocked_order.sort();
    risk_blocked_order.sort();
    let eligible = request
        .features
        .iter()
        .filter(|feature| {
            feature.quality_milli >= request.min_feature_quality_milli
                && feature.risk_milli <= request.risk_ceiling_milli
        })
        .collect::<Vec<_>>();
    let mut pair_records = Vec::new();
    for (left_index, left) in mechanism_order.iter().enumerate() {
        for right in mechanism_order.iter().skip(left_index + 1) {
            let mut contributions = eligible
                .iter()
                .map(|feature| (feature.feature_id.clone(), separation(feature, left, right)))
                .filter(|(_, value)| *value > 0)
                .collect::<Vec<_>>();
            contributions.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(&b.0)));
            pair_records.push((
                pair_id(left, right),
                left.clone(),
                right.clone(),
                0_u64,
                contributions,
            ));
        }
    }
    let mut feature_contributions = BTreeMap::<String, Vec<(String, u64)>>::new();
    for (pair_id, _, _, _, contributions) in &pair_records {
        for (feature_id, contribution) in contributions {
            feature_contributions
                .entry(feature_id.clone())
                .or_default()
                .push((pair_id.clone(), *contribution));
        }
    }
    for contributions in feature_contributions.values_mut() {
        contributions.sort_by(|left, right| left.0.cmp(&right.0));
    }
    let mut candidates = eligible.clone();
    candidates.sort_by(|left, right| left.feature_id.cmp(&right.feature_id));
    let mut states = vec![IdentifiabilityState {
        selected: Vec::new(),
        pair_scores: BTreeMap::new(),
        resolved_pairs: BTreeSet::new(),
        spent_units: 0,
    }];
    for feature in candidates {
        let mut next = states.clone();
        for state in &states {
            if state.selected.len() >= request.max_selected_features
                || state
                    .spent_units
                    .saturating_add(u64::from(feature.cost_units))
                    > request.budget_units
            {
                continue;
            }
            let (gain, _) = marginal_gain(
                &feature.feature_id,
                state,
                &feature_contributions,
                request.identifiability_floor_milli,
            );
            if gain == 0 {
                continue;
            }
            let mut selected = state.selected.clone();
            selected.push(feature.feature_id.clone());
            let mut pair_scores = state.pair_scores.clone();
            let mut resolved_pairs = state.resolved_pairs.clone();
            for (pair_id, contribution) in feature_contributions
                .get(&feature.feature_id)
                .into_iter()
                .flatten()
            {
                let current = pair_scores.get(pair_id).copied().unwrap_or(0);
                let next_score = current
                    .saturating_add(*contribution)
                    .min(request.identifiability_floor_milli);
                pair_scores.insert(pair_id.clone(), next_score);
                if next_score >= request.identifiability_floor_milli {
                    resolved_pairs.insert(pair_id.clone());
                }
            }
            next.push(IdentifiabilityState {
                selected,
                pair_scores,
                resolved_pairs,
                spent_units: state
                    .spent_units
                    .saturating_add(u64::from(feature.cost_units)),
            });
        }
        next.sort_by(|left, right| {
            state_score(right)
                .cmp(&state_score(left))
                .then_with(|| left.spent_units.cmp(&right.spent_units))
                .then_with(|| left.selected.cmp(&right.selected))
        });
        next.dedup_by(|left, right| left.selected == right.selected);
        next.truncate(IDENTIFIABILITY_BEAM_WIDTH);
        states = next;
    }
    let chosen = states
        .into_iter()
        .max_by(|left, right| {
            state_score(left)
                .cmp(&state_score(right))
                .then_with(|| right.spent_units.cmp(&left.spent_units))
                .then_with(|| right.selected.cmp(&left.selected))
        })
        .expect("identifiability beam always retains an empty state");
    let selected = chosen.selected.into_iter().collect::<BTreeSet<_>>();
    let spent = chosen.spent_units;
    let mut feature_utilities = Vec::new();
    let mut replay_state = IdentifiabilityState {
        selected: Vec::new(),
        pair_scores: BTreeMap::new(),
        resolved_pairs: BTreeSet::new(),
        spent_units: 0,
    };
    let mut replay_candidates = eligible.clone();
    replay_candidates.sort_by(|left, right| left.feature_id.cmp(&right.feature_id));
    for feature in replay_candidates {
        if !selected.contains(&feature.feature_id) {
            continue;
        }
        let (gain, covered) = marginal_gain(
            &feature.feature_id,
            &replay_state,
            &feature_contributions,
            request.identifiability_floor_milli,
        );
        replay_state.selected.push(feature.feature_id.clone());
        replay_state.spent_units = replay_state
            .spent_units
            .saturating_add(u64::from(feature.cost_units));
        for (pair_id, contribution) in feature_contributions
            .get(&feature.feature_id)
            .into_iter()
            .flatten()
        {
            let current = replay_state.pair_scores.get(pair_id).copied().unwrap_or(0);
            let next_score = current
                .saturating_add(*contribution)
                .min(request.identifiability_floor_milli);
            replay_state.pair_scores.insert(pair_id.clone(), next_score);
            if next_score >= request.identifiability_floor_milli {
                replay_state.resolved_pairs.insert(pair_id.clone());
            }
        }
        feature_utilities.push(IdentifiabilityFeatureUtility {
            feature_id: feature.feature_id.clone(),
            label: feature.label.clone(),
            marginal_separation_milli: gain,
            covered_pair_order: covered,
            projected_cost_units: replay_state.spent_units,
            action: "select-for-unresolved-pairs".into(),
        });
    }
    let mut budget_blocked_order = eligible
        .iter()
        .filter(|feature| {
            !selected.contains(&feature.feature_id)
                && u64::from(feature.cost_units) > request.budget_units.saturating_sub(spent)
        })
        .map(|feature| feature.feature_id.clone())
        .collect::<Vec<_>>();
    budget_blocked_order.sort();
    budget_blocked_order.dedup();
    feature_utilities.sort_by(|left, right| left.feature_id.cmp(&right.feature_id));
    let selected_feature_order = selected.iter().cloned().collect::<Vec<_>>();
    let mut pairs = Vec::new();
    let mut unresolved_pair_order = Vec::new();
    for (id, left, right, _, contributions) in pair_records {
        let total = contributions
            .iter()
            .filter(|(feature_id, _)| selected.contains(feature_id))
            .fold(0_u64, |sum, (_, value)| {
                sum.saturating_add(*value).min(SCORE_SCALE)
            });
        let discriminating_feature_order = contributions
            .iter()
            .filter(|(feature_id, _)| selected.contains(feature_id))
            .map(|(feature_id, _)| feature_id.clone())
            .collect::<Vec<_>>();
        let status = if total >= request.identifiability_floor_milli {
            "identified"
        } else {
            unresolved_pair_order.push(id.clone());
            "unresolved"
        };
        pairs.push(MechanismPairIdentifiability {
            pair_id: id,
            left_mechanism_id: left,
            right_mechanism_id: right,
            separation_milli: total,
            discriminating_feature_order,
            status: status.into(),
        });
    }
    pairs.sort_by(|left, right| left.pair_id.cmp(&right.pair_id));
    unresolved_pair_order.sort();
    let mut negative_evidence = quality_blocked_order
        .iter()
        .map(|id| format!("quality-gate-blocked:{id}"))
        .chain(
            risk_blocked_order
                .iter()
                .map(|id| format!("risk-gate-blocked:{id}")),
        )
        .chain(
            unresolved_pair_order
                .iter()
                .map(|id| format!("unresolved-pair:{id}")),
        )
        .collect::<Vec<_>>();
    negative_evidence.sort();
    let mut uncertainty = Vec::new();
    if !unresolved_pair_order.is_empty() {
        uncertainty.push("mechanism-pairs-remain-observationally-indistinguishable".into());
    }
    if !budget_blocked_order.is_empty() {
        uncertainty.push("some-discriminating-features-exceed-budget".into());
    }
    uncertainty.sort();
    let disposition = if eligible.is_empty() {
        MechanismIdentifiabilityDisposition::NoEligibleFeatures
    } else if unresolved_pair_order.is_empty() {
        MechanismIdentifiabilityDisposition::Qualified
    } else if selected_feature_order.is_empty() && !budget_blocked_order.is_empty() {
        MechanismIdentifiabilityDisposition::BudgetBlocked
    } else {
        MechanismIdentifiabilityDisposition::Partial
    };
    let mut output = MechanismIdentifiability {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        mechanism_order,
        feature_order,
        selected_feature_order,
        quality_blocked_order,
        risk_blocked_order,
        budget_blocked_order,
        pairs,
        feature_utilities,
        unresolved_pair_order,
        budget_remaining_units: request.budget_units.saturating_sub(spent),
        negative_evidence,
        uncertainty,
        disposition,
        digest: ContentHash::of_bytes(b"pending"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| MechanismIdentifiabilityError::Digest(error.to_string()))?;
    validate_output(&output)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> MechanismIdentifiabilityRequest {
        MechanismIdentifiabilityRequest {
            objective: "separate invasion mechanisms".into(),
            model_system: GliomaModelSystem::Organoid,
            mechanisms: vec![
                IdentifiabilityMechanism {
                    mechanism_id: "m-a".into(),
                    label: "matrix remodeling".into(),
                    prior_milli: 500,
                },
                IdentifiabilityMechanism {
                    mechanism_id: "m-b".into(),
                    label: "immune mimicry".into(),
                    prior_milli: 500,
                },
            ],
            features: vec![
                IdentifiabilityFeature {
                    feature_id: "f-low".into(),
                    label: "shared marker".into(),
                    value_milli_by_mechanism: BTreeMap::from([
                        ("m-a".into(), 100),
                        ("m-b".into(), 120),
                    ]),
                    quality_milli: 900,
                    cost_units: 2,
                    risk_milli: 100,
                },
                IdentifiabilityFeature {
                    feature_id: "f-high".into(),
                    label: "invasion edge".into(),
                    value_milli_by_mechanism: BTreeMap::from([
                        ("m-a".into(), 900),
                        ("m-b".into(), 100),
                    ]),
                    quality_milli: 900,
                    cost_units: 3,
                    risk_milli: 100,
                },
            ],
            identifiability_floor_milli: 500,
            min_feature_quality_milli: 700,
            risk_ceiling_milli: 500,
            budget_units: 10,
            max_selected_features: 2,
        }
    }

    #[test]
    fn selects_discriminating_feature_deterministically() {
        let first = analyze_glioma_mechanism_identifiability(&request()).expect("frontier");
        let second = analyze_glioma_mechanism_identifiability(&request()).expect("frontier");
        assert_eq!(first, second);
        assert_eq!(first.selected_feature_order, vec!["f-high"]);
        assert!(first.unresolved_pair_order.is_empty());
        first.validate().expect("valid output");
    }

    #[test]
    fn retains_unresolved_pair_as_negative_evidence() {
        let mut input = request();
        input.features[1].risk_milli = 900;
        let output = analyze_glioma_mechanism_identifiability(&input).expect("frontier");
        assert_eq!(output.risk_blocked_order, vec!["f-high"]);
        assert_eq!(output.unresolved_pair_order, vec!["m-a__m-b"]);
        assert!(output
            .negative_evidence
            .iter()
            .any(|entry| entry == "unresolved-pair:m-a__m-b"));
    }

    #[test]
    fn portfolio_beam_prefers_two_complementary_discriminators_over_expensive_hub() {
        let mut input = request();
        input.mechanisms = vec![
            IdentifiabilityMechanism {
                mechanism_id: "m-a".into(),
                label: "matrix remodeling".into(),
                prior_milli: 334,
            },
            IdentifiabilityMechanism {
                mechanism_id: "m-b".into(),
                label: "immune mimicry".into(),
                prior_milli: 333,
            },
            IdentifiabilityMechanism {
                mechanism_id: "m-c".into(),
                label: "vascular co-option".into(),
                prior_milli: 333,
            },
        ];
        input.features = vec![
            IdentifiabilityFeature {
                feature_id: "f-hub".into(),
                label: "expensive hub assay".into(),
                value_milli_by_mechanism: BTreeMap::from([
                    ("m-a".into(), 1_000),
                    ("m-b".into(), 0),
                    ("m-c".into(), 1_000),
                ]),
                quality_milli: 1_000,
                cost_units: 5,
                risk_milli: 100,
            },
            IdentifiabilityFeature {
                feature_id: "f-left".into(),
                label: "left discriminator".into(),
                value_milli_by_mechanism: BTreeMap::from([
                    ("m-a".into(), 1_000),
                    ("m-b".into(), 0),
                    ("m-c".into(), 0),
                ]),
                quality_milli: 1_000,
                cost_units: 3,
                risk_milli: 100,
            },
            IdentifiabilityFeature {
                feature_id: "f-right".into(),
                label: "right discriminator".into(),
                value_milli_by_mechanism: BTreeMap::from([
                    ("m-a".into(), 500),
                    ("m-b".into(), 500),
                    ("m-c".into(), 0),
                ]),
                quality_milli: 1_000,
                cost_units: 3,
                risk_milli: 100,
            },
        ];
        input.budget_units = 6;
        input.max_selected_features = 2;
        let output = analyze_glioma_mechanism_identifiability(&input).expect("frontier");
        assert_eq!(output.selected_feature_order, vec!["f-left", "f-right"]);
        assert!(output.unresolved_pair_order.is_empty());
    }
}
