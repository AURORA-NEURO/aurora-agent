//! Cross-family synthesis for autonomous preclinical glioma interpretation.
//!
//! P10 exposes several rigorous analyses, but a research engine still needs a gate that asks
//! whether those analyses agree across independent evidence families. This feature combines
//! signed, de-identified analysis summaries with quality- and uncertainty-weighted aggregation,
//! contradiction scoring, and leave-one-family-out stability. It does not pool raw observations,
//! turn unknown evidence into support, or make a clinical decision.

use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P10-F19";
pub const OUTPUT_SCHEMA: &str = "GliomaInterpretationSynthesis1@1";
pub const MAX_EVIDENCE: usize = 4_096;
pub const MAX_FAMILIES: usize = 16;
pub const MAX_INDEPENDENT_GROUPS: usize = 4_096;
pub const MAX_UNCERTAINTY_MILLI: u64 = 1_000_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterpretationEvidenceFamily {
    CausalContrast,
    CausalAdjustment,
    Mediation,
    Trajectory,
    StateTransition,
    Sensitivity,
    MetaAnalysis,
    Transportability,
    Replication,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterpretationEvidenceDirection {
    Positive,
    Negative,
    Null,
    Mixed,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterpretationEvidence {
    pub evidence_id: String,
    pub family: InterpretationEvidenceFamily,
    pub independent_group: String,
    pub model_system: GliomaModelSystem,
    pub direction: InterpretationEvidenceDirection,
    pub effect_milli: u64,
    pub uncertainty_milli: u64,
    pub quality_milli: u16,
    pub sample_count: u32,
    pub artifact: LocalArtifactRef,
    pub negative_evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterpretationSynthesisRequest {
    pub objective: String,
    pub hypothesis: String,
    pub model_system: GliomaModelSystem,
    pub min_evidence: usize,
    pub min_independent_groups: usize,
    pub min_families: usize,
    pub min_quality_milli: u16,
    pub effect_threshold_milli: u64,
    pub max_disagreement_milli: u64,
    pub max_leave_one_out_shift_milli: u64,
    pub require_replication_family: bool,
    pub replay_identity: ContentHash,
    pub evidence: Vec<InterpretationEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterpretationFamilySummary {
    pub family: InterpretationEvidenceFamily,
    pub evidence_order: Vec<String>,
    pub independent_group_order: Vec<String>,
    pub eligible_count: usize,
    pub unresolved_count: usize,
    pub weighted_effect_milli: i64,
    pub interval_low_milli: i64,
    pub interval_high_milli: i64,
    pub direction: InterpretationEvidenceDirection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterpretationSynthesisDisposition {
    Qualified,
    Negative,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterpretationSynthesis {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub hypothesis: String,
    pub model_system: GliomaModelSystem,
    pub replay_identity: ContentHash,
    pub evidence: Vec<InterpretationEvidence>,
    pub evidence_order: Vec<String>,
    pub unresolved_evidence_order: Vec<String>,
    pub family_order: Vec<InterpretationEvidenceFamily>,
    pub independent_group_order: Vec<String>,
    pub families: Vec<InterpretationFamilySummary>,
    pub aggregate_effect_milli: i64,
    pub interval_low_milli: i64,
    pub interval_high_milli: i64,
    pub leave_one_out_low_milli: i64,
    pub leave_one_out_high_milli: i64,
    pub support_milli: u16,
    pub contradiction_milli: u16,
    pub disagreement_milli: u64,
    pub stability_milli: u16,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: InterpretationSynthesisDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum InterpretationSynthesisError {
    #[error("interpretation synthesis request is invalid: {0}")]
    InvalidRequest(String),
    #[error("interpretation evidence is invalid: {0}")]
    InvalidEvidence(String),
    #[error("interpretation synthesis output is invalid: {0}")]
    InvalidOutput(String),
    #[error("interpretation synthesis digest failed: {0}")]
    Digest(String),
}

fn valid_identifier(value: &str) -> bool {
    !value.trim().is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:-".contains(&byte))
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn signed_effect(evidence: &InterpretationEvidence) -> Option<i64> {
    let magnitude = i64::try_from(evidence.effect_milli).ok()?;
    match evidence.direction {
        InterpretationEvidenceDirection::Positive => Some(magnitude),
        InterpretationEvidenceDirection::Negative => Some(-magnitude),
        InterpretationEvidenceDirection::Null => Some(0),
        InterpretationEvidenceDirection::Mixed | InterpretationEvidenceDirection::Unresolved => {
            None
        }
    }
}

fn weight(evidence: &InterpretationEvidence) -> u128 {
    u128::from(evidence.quality_milli)
        .saturating_mul(u128::from(evidence.sample_count))
        .saturating_mul(1_000_000)
        .checked_div(u128::from(evidence.uncertainty_milli).saturating_add(1))
        .unwrap_or_default()
}

fn clamp_i128(value: i128) -> i64 {
    value.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
}

fn direction_for(effect: i64, threshold: u64) -> InterpretationEvidenceDirection {
    let magnitude = effect.unsigned_abs();
    if magnitude < threshold {
        InterpretationEvidenceDirection::Null
    } else if effect > 0 {
        InterpretationEvidenceDirection::Positive
    } else {
        InterpretationEvidenceDirection::Negative
    }
}

fn digest_input(output: &InterpretationSynthesis) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "hypothesis": output.hypothesis,
        "model_system": output.model_system,
        "replay_identity": output.replay_identity,
        "evidence": output.evidence,
        "evidence_order": output.evidence_order,
        "unresolved_evidence_order": output.unresolved_evidence_order,
        "family_order": output.family_order,
        "independent_group_order": output.independent_group_order,
        "families": output.families,
        "aggregate_effect_milli": output.aggregate_effect_milli,
        "interval_low_milli": output.interval_low_milli,
        "interval_high_milli": output.interval_high_milli,
        "leave_one_out_low_milli": output.leave_one_out_low_milli,
        "leave_one_out_high_milli": output.leave_one_out_high_milli,
        "support_milli": output.support_milli,
        "contradiction_milli": output.contradiction_milli,
        "disagreement_milli": output.disagreement_milli,
        "stability_milli": output.stability_milli,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn validate_request(
    request: &InterpretationSynthesisRequest,
) -> Result<(), InterpretationSynthesisError> {
    if request.objective.trim().is_empty()
        || request.hypothesis.trim().is_empty()
        || request.min_evidence == 0
        || request.min_evidence > MAX_EVIDENCE
        || request.min_independent_groups == 0
        || request.min_independent_groups > MAX_INDEPENDENT_GROUPS
        || request.min_families == 0
        || request.min_families > MAX_FAMILIES
        || request.min_quality_milli > 1_000
        || request.effect_threshold_milli > i64::MAX as u64
        || request.max_disagreement_milli > i64::MAX as u64
        || request.max_leave_one_out_shift_milli > i64::MAX as u64
        || request.replay_identity.as_str().len() != 64
        || request.evidence.is_empty()
        || request.evidence.len() > MAX_EVIDENCE
    {
        return Err(InterpretationSynthesisError::InvalidRequest(
            "objective, hypothesis, bounded floors, replay identity, and evidence are required"
                .into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for evidence in &request.evidence {
        if !ids.insert(evidence.evidence_id.clone())
            || !valid_identifier(&evidence.evidence_id)
            || !valid_identifier(&evidence.independent_group)
            || evidence.model_system != request.model_system
            || evidence.uncertainty_milli > MAX_UNCERTAINTY_MILLI
            || evidence.quality_milli > 1_000
            || evidence.sample_count == 0
            || matches!(
                evidence.direction,
                InterpretationEvidenceDirection::Positive
                    | InterpretationEvidenceDirection::Negative
                    | InterpretationEvidenceDirection::Null
            ) && evidence.effect_milli == 0
                && evidence.direction != InterpretationEvidenceDirection::Null
        {
            return Err(InterpretationSynthesisError::InvalidEvidence(
                "evidence identities, model binding, uncertainty, quality, and direction must be typed and unique".into(),
            ));
        }
        evidence
            .artifact
            .validate()
            .map_err(|error| InterpretationSynthesisError::InvalidEvidence(error.to_string()))?;
        if evidence
            .negative_evidence
            .iter()
            .any(|item| item.trim().is_empty())
        {
            return Err(InterpretationSynthesisError::InvalidEvidence(
                "negative evidence explanations cannot be empty".into(),
            ));
        }
    }
    Ok(())
}

impl InterpretationSynthesis {
    pub fn validate(&self) -> Result<(), InterpretationSynthesisError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.hypothesis.trim().is_empty()
            || self.replay_identity.as_str().len() != 64
            || self.evidence.is_empty()
            || self.evidence.len() > MAX_EVIDENCE
            || !canonical(&self.evidence_order)
            || !canonical(&self.unresolved_evidence_order)
            || !canonical(&self.family_order)
            || !canonical(&self.independent_group_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.support_milli > 1_000
            || self.contradiction_milli > 1_000
            || self.stability_milli > 1_000
            || self.interval_low_milli > self.interval_high_milli
            || self.leave_one_out_low_milli > self.leave_one_out_high_milli
            || self.digest.as_str().len() != 64
        {
            return Err(InterpretationSynthesisError::InvalidOutput(
                "identity, ordering, evidence, interval, score, or digest fields are invalid"
                    .into(),
            ));
        }
        let evidence_ids = self
            .evidence
            .iter()
            .map(|evidence| evidence.evidence_id.clone())
            .collect::<BTreeSet<_>>();
        if evidence_ids.len() != self.evidence.len()
            || self.evidence_order.iter().cloned().collect::<BTreeSet<_>>() != evidence_ids
            || self
                .unresolved_evidence_order
                .iter()
                .any(|id| !evidence_ids.contains(id))
        {
            return Err(InterpretationSynthesisError::InvalidOutput(
                "evidence identities do not reconcile with ordered partitions".into(),
            ));
        }
        for evidence in &self.evidence {
            evidence
                .artifact
                .validate()
                .map_err(|error| InterpretationSynthesisError::InvalidOutput(error.to_string()))?;
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| InterpretationSynthesisError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(InterpretationSynthesisError::InvalidOutput(
                "interpretation synthesis digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Synthesize independent P10 analysis families into a bounded research interpretation gate.
pub fn synthesize_glioma_interpretation(
    request: &InterpretationSynthesisRequest,
) -> Result<InterpretationSynthesis, InterpretationSynthesisError> {
    validate_request(request)?;
    let mut evidence = request.evidence.clone();
    evidence.sort_by(|left, right| left.evidence_id.cmp(&right.evidence_id));
    let evidence_order = evidence
        .iter()
        .map(|evidence| evidence.evidence_id.clone())
        .collect::<Vec<_>>();
    let unresolved_evidence_order = evidence
        .iter()
        .filter(|evidence| {
            matches!(
                evidence.direction,
                InterpretationEvidenceDirection::Mixed
                    | InterpretationEvidenceDirection::Unresolved
            ) || evidence.quality_milli < request.min_quality_milli
        })
        .map(|evidence| evidence.evidence_id.clone())
        .collect::<Vec<_>>();
    let mut family_map =
        BTreeMap::<InterpretationEvidenceFamily, Vec<&InterpretationEvidence>>::new();
    for evidence in &evidence {
        family_map
            .entry(evidence.family)
            .or_default()
            .push(evidence);
    }
    let family_order = family_map.keys().copied().collect::<Vec<_>>();
    let independent_group_order = evidence
        .iter()
        .map(|evidence| evidence.independent_group.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut families = Vec::new();
    let mut total_weight = 0_u128;
    let mut positive_weight = 0_u128;
    let mut negative_weight = 0_u128;
    let mut weighted_sum = 0_i128;
    let mut family_effects = Vec::new();
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for (family, rows) in &family_map {
        let mut family_weight = 0_u128;
        let mut family_sum = 0_i128;
        let mut family_uncertainty = 0_u128;
        let mut eligible_order = Vec::new();
        let mut unresolved_order = Vec::new();
        let mut groups = BTreeSet::new();
        for evidence in rows {
            negative_evidence.extend(evidence.negative_evidence.iter().cloned());
            if evidence.quality_milli < request.min_quality_milli {
                unresolved_order.push(evidence.evidence_id.clone());
                uncertainty.insert(format!("{}:below-quality-floor", evidence.evidence_id));
                continue;
            }
            let Some(effect) = signed_effect(evidence) else {
                unresolved_order.push(evidence.evidence_id.clone());
                uncertainty.insert(format!("{}:direction-unresolved", evidence.evidence_id));
                continue;
            };
            let current_weight = weight(evidence);
            if current_weight == 0 {
                unresolved_order.push(evidence.evidence_id.clone());
                uncertainty.insert(format!("{}:zero-evidence-weight", evidence.evidence_id));
                continue;
            }
            eligible_order.push(evidence.evidence_id.clone());
            groups.insert(evidence.independent_group.clone());
            family_weight = family_weight.saturating_add(current_weight);
            family_sum = family_sum
                .saturating_add(i128::from(effect).saturating_mul(current_weight as i128));
            family_uncertainty = family_uncertainty.saturating_add(
                current_weight.saturating_mul(u128::from(evidence.uncertainty_milli)),
            );
            total_weight = total_weight.saturating_add(current_weight);
            weighted_sum = weighted_sum
                .saturating_add(i128::from(effect).saturating_mul(current_weight as i128));
            if effect > 0 {
                positive_weight = positive_weight.saturating_add(current_weight);
            } else if effect < 0 {
                negative_weight = negative_weight.saturating_add(current_weight);
            }
        }
        let weighted_effect = if family_weight == 0 {
            0
        } else {
            clamp_i128(family_sum / family_weight as i128)
        };
        let mean_uncertainty = if family_weight == 0 {
            0
        } else {
            family_uncertainty
                .checked_div(family_weight)
                .unwrap_or_default()
                .min(i128::from(i64::MAX) as u128) as i64
        };
        let family_direction = if eligible_order.is_empty() {
            InterpretationEvidenceDirection::Unresolved
        } else {
            direction_for(weighted_effect, request.effect_threshold_milli)
        };
        let eligible_count = eligible_order.len();
        family_effects.push(weighted_effect);
        families.push(InterpretationFamilySummary {
            family: *family,
            evidence_order: eligible_order,
            independent_group_order: groups.into_iter().collect(),
            eligible_count,
            unresolved_count: unresolved_order.len(),
            weighted_effect_milli: weighted_effect,
            interval_low_milli: weighted_effect.saturating_sub(mean_uncertainty),
            interval_high_milli: weighted_effect.saturating_add(mean_uncertainty),
            direction: family_direction,
        });
        for evidence_id in unresolved_order {
            uncertainty.insert(format!("{family:?}:{evidence_id}:unresolved"));
        }
    }
    let aggregate_effect = if total_weight == 0 {
        0
    } else {
        clamp_i128(weighted_sum / total_weight as i128)
    };
    let total_uncertainty = evidence
        .iter()
        .filter(|evidence| evidence.quality_milli >= request.min_quality_milli)
        .filter_map(|evidence| {
            let current_weight = weight(evidence);
            (current_weight > 0)
                .then_some((current_weight, evidence.uncertainty_milli))
                .filter(|_| signed_effect(evidence).is_some())
        })
        .fold(0_u128, |sum, (current_weight, uncertainty)| {
            sum.saturating_add(current_weight.saturating_mul(u128::from(uncertainty)))
        });
    let mean_uncertainty = if total_weight == 0 {
        0
    } else {
        total_uncertainty
            .checked_div(total_weight)
            .unwrap_or_default()
            .min(i128::from(i64::MAX) as u128) as i64
    };
    let interval_low = aggregate_effect.saturating_sub(mean_uncertainty);
    let interval_high = aggregate_effect.saturating_add(mean_uncertainty);
    let disagreement = family_effects
        .iter()
        .min()
        .zip(family_effects.iter().max())
        .map(|(low, high)| high.abs_diff(*low))
        .unwrap_or_default();
    let mut leave_one_out = Vec::new();
    for family in &family_order {
        let (sum, weight_sum) = evidence
            .iter()
            .filter(|evidence| {
                evidence.family != *family && evidence.quality_milli >= request.min_quality_milli
            })
            .filter_map(|evidence| {
                let current_weight = weight(evidence);
                (current_weight > 0)
                    .then(|| signed_effect(evidence).map(|effect| (effect, current_weight)))
                    .flatten()
            })
            .fold(
                (0_i128, 0_u128),
                |(sum, weight_sum), (effect, current_weight)| {
                    (
                        sum.saturating_add(
                            i128::from(effect).saturating_mul(current_weight as i128),
                        ),
                        weight_sum.saturating_add(current_weight),
                    )
                },
            );
        if weight_sum > 0 {
            leave_one_out.push(clamp_i128(sum / weight_sum as i128));
        }
    }
    if leave_one_out.is_empty() {
        leave_one_out.push(aggregate_effect);
    }
    let leave_one_out_low = *leave_one_out.iter().min().unwrap_or(&aggregate_effect);
    let leave_one_out_high = *leave_one_out.iter().max().unwrap_or(&aggregate_effect);
    let leave_one_out_shift = leave_one_out
        .iter()
        .map(|effect| effect.abs_diff(aggregate_effect))
        .max()
        .unwrap_or_default();
    let stability = if aggregate_effect == 0 {
        if leave_one_out_shift == 0 {
            1_000
        } else {
            0
        }
    } else {
        1_000_u64.saturating_sub(
            leave_one_out_shift.saturating_mul(1_000) / aggregate_effect.unsigned_abs().max(1),
        ) as u16
    };
    let support = if total_weight == 0 {
        0
    } else {
        positive_weight
            .max(negative_weight)
            .saturating_mul(1_000)
            .checked_div(total_weight)
            .unwrap_or_default()
    };
    let contradiction = if total_weight == 0 {
        0
    } else {
        positive_weight
            .min(negative_weight)
            .saturating_mul(2_000)
            .checked_div(total_weight)
            .unwrap_or_default()
            .min(1_000) as u16
    };
    let enough_evidence = families
        .iter()
        .map(|family| family.eligible_count)
        .sum::<usize>()
        >= request.min_evidence;
    let enough_groups = independent_group_order.len() >= request.min_independent_groups;
    let enough_families = families
        .iter()
        .filter(|family| family.eligible_count > 0)
        .count()
        >= request.min_families;
    let has_replication = families.iter().any(|family| {
        family.family == InterpretationEvidenceFamily::Replication && family.eligible_count > 0
    });
    let stable = disagreement <= request.max_disagreement_milli
        && leave_one_out_shift <= request.max_leave_one_out_shift_milli;
    let direction = direction_for(aggregate_effect, request.effect_threshold_milli);
    let disposition = if total_weight == 0 {
        InterpretationSynthesisDisposition::Unresolved
    } else if request.require_replication_family && !has_replication {
        // A missing replication family is an actionable partial result even when
        // the remaining evidence also falls below one of the declared floors.
        // Keeping this state distinct from fully unresolved preserves the
        // evidence that is already useful while preventing qualification.
        InterpretationSynthesisDisposition::Partial
    } else if !enough_evidence || !enough_groups || !enough_families {
        InterpretationSynthesisDisposition::Unresolved
    } else if !unresolved_evidence_order.is_empty() || !stable {
        InterpretationSynthesisDisposition::Partial
    } else if direction == InterpretationEvidenceDirection::Null {
        InterpretationSynthesisDisposition::Negative
    } else {
        InterpretationSynthesisDisposition::Qualified
    };
    if request.require_replication_family && !has_replication {
        uncertainty.insert("replication-family-required-but-missing".into());
    }
    if !stable {
        uncertainty.insert("cross-family-disagreement-or-leave-one-out-instability".into());
    }
    if direction == InterpretationEvidenceDirection::Null {
        negative_evidence.insert("aggregate-effect-below-declared-threshold".into());
    }
    let mut output = InterpretationSynthesis {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        hypothesis: request.hypothesis.clone(),
        model_system: request.model_system,
        replay_identity: request.replay_identity.clone(),
        evidence,
        evidence_order,
        unresolved_evidence_order,
        family_order,
        independent_group_order,
        families,
        aggregate_effect_milli: aggregate_effect,
        interval_low_milli: interval_low,
        interval_high_milli: interval_high,
        leave_one_out_low_milli: leave_one_out_low,
        leave_one_out_high_milli: leave_one_out_high,
        support_milli: support.min(1_000) as u16,
        contradiction_milli: contradiction,
        disagreement_milli: disagreement,
        stability_milli: stability,
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-interpretation-synthesis"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| InterpretationSynthesisError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: id.into(),
            content_hash: ContentHash::of_bytes(id.as_bytes()),
            content_type: "application/json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn evidence(
        id: &str,
        family: InterpretationEvidenceFamily,
        group: &str,
        direction: InterpretationEvidenceDirection,
        effect_milli: u64,
    ) -> InterpretationEvidence {
        InterpretationEvidence {
            evidence_id: id.into(),
            family,
            independent_group: group.into(),
            model_system: GliomaModelSystem::Organoid,
            direction,
            effect_milli,
            uncertainty_milli: 50,
            quality_milli: 900,
            sample_count: 6,
            artifact: artifact(id),
            negative_evidence: Vec::new(),
        }
    }

    fn request() -> InterpretationSynthesisRequest {
        InterpretationSynthesisRequest {
            objective: "decide whether invasion signal is reproducibly supported".into(),
            hypothesis: "integrated invasion program is activated".into(),
            model_system: GliomaModelSystem::Organoid,
            min_evidence: 3,
            min_independent_groups: 2,
            min_families: 2,
            min_quality_milli: 700,
            effect_threshold_milli: 100,
            max_disagreement_milli: 150,
            max_leave_one_out_shift_milli: 200,
            require_replication_family: true,
            replay_identity: ContentHash::of_bytes(b"synthesis-replay"),
            evidence: vec![
                evidence(
                    "causal-a",
                    InterpretationEvidenceFamily::CausalContrast,
                    "site-a",
                    InterpretationEvidenceDirection::Positive,
                    300,
                ),
                evidence(
                    "meta-b",
                    InterpretationEvidenceFamily::MetaAnalysis,
                    "site-b",
                    InterpretationEvidenceDirection::Positive,
                    280,
                ),
                evidence(
                    "replication-c",
                    InterpretationEvidenceFamily::Replication,
                    "site-c",
                    InterpretationEvidenceDirection::Positive,
                    320,
                ),
            ],
        }
    }

    #[test]
    fn synthesis_is_permutation_stable_and_qualifies_concordant_families() {
        let first = synthesize_glioma_interpretation(&request()).unwrap();
        let mut permuted = request();
        permuted.evidence.reverse();
        let second = synthesize_glioma_interpretation(&permuted).unwrap();
        assert_eq!(first, second);
        assert_eq!(
            first.disposition,
            InterpretationSynthesisDisposition::Qualified
        );
        assert!(first.aggregate_effect_milli > 100);
        assert!(first.stability_milli > 0);
    }

    #[test]
    fn contradictory_family_remains_partial_and_visible() {
        let mut request = request();
        request.evidence.push(evidence(
            "negative-d",
            InterpretationEvidenceFamily::Sensitivity,
            "site-d",
            InterpretationEvidenceDirection::Negative,
            900,
        ));
        let output = synthesize_glioma_interpretation(&request).unwrap();
        assert_eq!(
            output.disposition,
            InterpretationSynthesisDisposition::Partial
        );
        assert!(output.contradiction_milli > 0);
        assert!(output
            .uncertainty
            .contains(&"cross-family-disagreement-or-leave-one-out-instability".to_string()));
    }

    #[test]
    fn missing_replication_is_not_promoted_to_qualification() {
        let mut request = request();
        request.require_replication_family = true;
        request
            .evidence
            .retain(|evidence| evidence.family != InterpretationEvidenceFamily::Replication);
        let output = synthesize_glioma_interpretation(&request).unwrap();
        assert_eq!(
            output.disposition,
            InterpretationSynthesisDisposition::Partial
        );
        assert!(output
            .uncertainty
            .contains(&"replication-family-required-but-missing".to_string()));
    }

    #[test]
    fn low_quality_replication_does_not_satisfy_replication_floor() {
        let mut request = request();
        request.evidence[2].quality_milli = 600;
        let output = synthesize_glioma_interpretation(&request).unwrap();
        assert_eq!(
            output.disposition,
            InterpretationSynthesisDisposition::Partial
        );
        assert_eq!(
            output.families[2].direction,
            InterpretationEvidenceDirection::Unresolved
        );
        assert!(output
            .uncertainty
            .contains(&"replication-family-required-but-missing".to_string()));
        assert!(output.negative_evidence.is_empty());
    }

    #[test]
    fn invalid_artifact_scope_is_refused() {
        let mut request = request();
        request.evidence[0].artifact.local_only = false;
        assert!(matches!(
            synthesize_glioma_interpretation(&request),
            Err(InterpretationSynthesisError::InvalidEvidence(_))
        ));
    }
}
