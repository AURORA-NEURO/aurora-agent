//! Explicit multimodal contradiction adjudication for autonomous preclinical glioma research.
//!
//! Endpoint fusion can identify that measurements disagree, but an autonomous research engine
//! also needs a reproducible resolution queue. This feature classifies pairwise disagreement,
//! compares declared measurement trust without silently deleting a rival modality, and routes
//! sign reversals or unresolved quality asymmetries to orthogonal follow-up. It is a research
//! evidence adjudicator only: it performs no assay, instrument, federation, or clinical action.

use super::evidence_fusion::EndpointEvidence;
use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeSet, HashMap};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P03-F19";
pub const OUTPUT_SCHEMA: &str = "GliomaMultimodalContradictionAdjudication1@1";
pub const MAX_MODALITIES: usize = 64;
pub const MAX_PAIRS: usize = 2_048;
pub const MAX_ABS_VALUE_MILLI: u64 = 1_000_000_000;
pub const MAX_UNCERTAINTY_MILLI: u64 = 1_000_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContradictionAdjudicationRequest {
    pub objective: String,
    pub endpoint_id: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub required_modalities: Vec<GliomaModality>,
    pub observations: Vec<EndpointEvidence>,
    pub min_modalities: usize,
    pub min_reliability_milli: u16,
    pub max_uncertainty_milli: u64,
    pub max_pair_difference_milli: u64,
    pub min_trust_margin_milli: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContradictionKind {
    Agreement,
    SignReversal,
    MagnitudeConflict,
    QualityAsymmetry,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContradictionAdjudicationDisposition {
    Ready,
    Conditional,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModalityPairAdjudication {
    pub left_modality: GliomaModality,
    pub right_modality: GliomaModality,
    pub left_value_milli: i64,
    pub right_value_milli: i64,
    pub absolute_difference_milli: u64,
    pub left_trust_milli: u64,
    pub right_trust_milli: u64,
    pub kind: ContradictionKind,
    pub confidence_milli: u16,
    pub preferred_modality: Option<GliomaModality>,
    pub reason: String,
    pub resolution_action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContradictionAdjudication {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub endpoint_id: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub modality_order: Vec<GliomaModality>,
    pub eligible_modality_order: Vec<GliomaModality>,
    pub missing_modality_order: Vec<GliomaModality>,
    pub pair_order: Vec<(GliomaModality, GliomaModality)>,
    pub pairs: Vec<ModalityPairAdjudication>,
    pub trusted_modality_order: Vec<GliomaModality>,
    pub resolution_order: Vec<(GliomaModality, GliomaModality)>,
    pub unresolved_pair_order: Vec<(GliomaModality, GliomaModality)>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: ContradictionAdjudicationDisposition,
    pub next_action: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ContradictionAdjudicationError {
    #[error("multimodal contradiction request is invalid: {0}")]
    InvalidRequest(String),
    #[error("multimodal contradiction input is invalid: {0}")]
    InvalidInput(String),
    #[error("multimodal contradiction output is invalid: {0}")]
    InvalidOutput(String),
    #[error("multimodal contradiction digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn abs_difference(left: i64, right: i64) -> u64 {
    (i128::from(left) - i128::from(right)).unsigned_abs() as u64
}

fn trust(observation: &EndpointEvidence) -> u64 {
    (u64::from(observation.reliability_milli)
        * u64::from(observation.quality_milli)
        * u64::from(observation.replicate_count))
        / (observation.uncertainty_milli + 1)
}

fn confidence(left: u64, right: u64) -> u16 {
    let total = left.saturating_add(right);
    if total == 0 {
        0
    } else {
        ((left.min(right).saturating_mul(2_000) / total).min(1_000)) as u16
    }
}

fn pair_key(left: GliomaModality, right: GliomaModality) -> (GliomaModality, GliomaModality) {
    if left < right {
        (left, right)
    } else {
        (right, left)
    }
}

fn severity(kind: ContradictionKind) -> u8 {
    match kind {
        ContradictionKind::Agreement => 0,
        ContradictionKind::QualityAsymmetry => 1,
        ContradictionKind::MagnitudeConflict => 2,
        ContradictionKind::Unresolved => 3,
        ContradictionKind::SignReversal => 4,
    }
}

fn digest_input(output: &ContradictionAdjudication) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "endpoint_id": output.endpoint_id,
        "study_id": output.study_id,
        "model_system": output.model_system,
        "modality_order": output.modality_order,
        "eligible_modality_order": output.eligible_modality_order,
        "missing_modality_order": output.missing_modality_order,
        "pair_order": output.pair_order,
        "pairs": output.pairs,
        "trusted_modality_order": output.trusted_modality_order,
        "resolution_order": output.resolution_order,
        "unresolved_pair_order": output.unresolved_pair_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
        "next_action": output.next_action,
    })
}

fn validate_request(
    request: &ContradictionAdjudicationRequest,
) -> Result<(), ContradictionAdjudicationError> {
    if request.objective.trim().is_empty()
        || request.endpoint_id.trim().is_empty()
        || request.study_id.trim().is_empty()
        || request.required_modalities.len() > MAX_MODALITIES
        || request.observations.is_empty()
        || request.observations.len() > MAX_MODALITIES
        || request.min_modalities == 0
        || request.min_modalities > request.observations.len()
        || request.min_reliability_milli > 1_000
        || request.max_uncertainty_milli > MAX_UNCERTAINTY_MILLI
        || request.max_pair_difference_milli == 0
        || request.max_pair_difference_milli > MAX_ABS_VALUE_MILLI
        || request.min_trust_margin_milli > MAX_ABS_VALUE_MILLI
    {
        return Err(ContradictionAdjudicationError::InvalidRequest(
            "endpoint/study identity, bounded unique observations, modality floor, and positive conflict gates are required".into(),
        ));
    }
    if !canonical(&request.required_modalities) {
        return Err(ContradictionAdjudicationError::InvalidRequest(
            "required modalities must be unique and canonical".into(),
        ));
    }
    let required = request
        .required_modalities
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut seen = BTreeSet::new();
    for observation in &request.observations {
        if !seen.insert(observation.modality)
            || (!required.is_empty() && !required.contains(&observation.modality))
            || observation.value_milli.unsigned_abs() > MAX_ABS_VALUE_MILLI
            || observation.uncertainty_milli > MAX_UNCERTAINTY_MILLI
            || observation.reliability_milli > 1_000
            || observation.quality_milli > 1_000
            || observation.replicate_count == 0
        {
            return Err(ContradictionAdjudicationError::InvalidInput(
                "observations require unique declared modalities, bounded values/uncertainty/quality/reliability, and positive replicate counts".into(),
            ));
        }
    }
    let pair_count = request.observations.len() * request.observations.len().saturating_sub(1) / 2;
    if pair_count > MAX_PAIRS {
        return Err(ContradictionAdjudicationError::InvalidRequest(
            "observation pair count exceeds bounded adjudication capacity".into(),
        ));
    }
    Ok(())
}

fn validate_output(
    output: &ContradictionAdjudication,
) -> Result<(), ContradictionAdjudicationError> {
    if output.feature_id != FEATURE_ID
        || output.output_schema != OUTPUT_SCHEMA
        || output.objective.trim().is_empty()
        || output.endpoint_id.trim().is_empty()
        || output.study_id.trim().is_empty()
        || !canonical(&output.modality_order)
        || !canonical(&output.eligible_modality_order)
        || !canonical(&output.missing_modality_order)
        || output.pair_order.len() != output.pairs.len()
        || output.pairs.windows(2).any(|pair| {
            pair[0].left_modality >= pair[1].left_modality
                || (pair[0].left_modality == pair[1].left_modality
                    && pair[0].right_modality >= pair[1].right_modality)
        })
        || !canonical(&output.negative_evidence)
        || !canonical(&output.uncertainty)
        || output
            .pairs
            .iter()
            .any(|pair| pair.reason.trim().is_empty() || pair.resolution_action.trim().is_empty())
    {
        return Err(ContradictionAdjudicationError::InvalidOutput(
            "identity, canonical modality/pair ordering, pair cardinality, or reason invariants are invalid".into(),
        ));
    }
    let mut seen_pairs = BTreeSet::new();
    for pair in &output.pair_order {
        if pair.0 >= pair.1 || !seen_pairs.insert(*pair) {
            return Err(ContradictionAdjudicationError::InvalidOutput(
                "pair order must contain unique canonical pairs".into(),
            ));
        }
    }
    if output.pairs.iter().enumerate().any(|(index, pair)| {
        output.pair_order.get(index) != Some(&(pair.left_modality, pair.right_modality))
    }) {
        return Err(ContradictionAdjudicationError::InvalidOutput(
            "pair order is not bound to pair records".into(),
        ));
    }
    let mut seen_resolution = BTreeSet::new();
    if output
        .resolution_order
        .iter()
        .any(|pair| !seen_resolution.insert(*pair))
    {
        return Err(ContradictionAdjudicationError::InvalidOutput(
            "resolution order must contain unique pairs".into(),
        ));
    }
    let mut seen_unresolved = BTreeSet::new();
    if output
        .unresolved_pair_order
        .iter()
        .any(|pair| !seen_unresolved.insert(*pair))
    {
        return Err(ContradictionAdjudicationError::InvalidOutput(
            "unresolved pair order must contain unique pairs".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(output))
        .map_err(|error| ContradictionAdjudicationError::Digest(error.to_string()))?;
    if expected != output.digest {
        return Err(ContradictionAdjudicationError::InvalidOutput(
            "digest is not bound to contradiction adjudication output".into(),
        ));
    }
    Ok(())
}

impl ContradictionAdjudication {
    pub fn validate(&self) -> Result<(), ContradictionAdjudicationError> {
        validate_output(self)
    }
}

/// Classify every observed modality pair and produce a deterministic resolution queue.
pub fn adjudicate_glioma_multimodal_contradictions(
    request: &ContradictionAdjudicationRequest,
) -> Result<ContradictionAdjudication, ContradictionAdjudicationError> {
    validate_request(request)?;
    let required = request
        .required_modalities
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut by_modality = HashMap::new();
    for observation in &request.observations {
        by_modality.insert(observation.modality, observation);
    }
    let observed = by_modality.keys().copied().collect::<BTreeSet<_>>();
    let modality_order = required.union(&observed).copied().collect::<Vec<_>>();
    let eligible = request
        .observations
        .iter()
        .filter(|observation| {
            observation.reliability_milli >= request.min_reliability_milli
                && observation.quality_milli >= request.min_reliability_milli
                && observation.uncertainty_milli <= request.max_uncertainty_milli
                && trust(observation) > 0
        })
        .collect::<Vec<_>>();
    let eligible_set = eligible
        .iter()
        .map(|observation| observation.modality)
        .collect::<BTreeSet<_>>();
    let missing_modality_order = modality_order
        .iter()
        .copied()
        .filter(|modality| !by_modality.contains_key(modality))
        .collect::<Vec<_>>();
    let mut pairs = Vec::new();
    let mut negative = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut resolution_rank = Vec::new();
    let mut unresolved_pair_order = Vec::new();
    for left_index in 0..request.observations.len() {
        for right_index in (left_index + 1)..request.observations.len() {
            let left = &request.observations[left_index];
            let right = &request.observations[right_index];
            let (left, right) = if left.modality < right.modality {
                (left, right)
            } else {
                (right, left)
            };
            let key = pair_key(left.modality, right.modality);
            let left_trust = trust(left);
            let right_trust = trust(right);
            let difference = abs_difference(left.value_milli, right.value_milli);
            let both_eligible =
                eligible_set.contains(&left.modality) && eligible_set.contains(&right.modality);
            let sign_reversal = (left.value_milli < 0 && right.value_milli > 0)
                || (left.value_milli > 0 && right.value_milli < 0);
            let trust_difference = abs_difference(
                left_trust.min(i64::MAX as u64) as i64,
                right_trust.min(i64::MAX as u64) as i64,
            );
            let trust_preference = if trust_difference >= request.min_trust_margin_milli {
                Some(if left_trust > right_trust {
                    left.modality
                } else {
                    right.modality
                })
            } else {
                None
            };
            let (kind, reason, resolution_action) = if !both_eligible {
                (
                    ContradictionKind::Unresolved,
                    "one or both modalities failed the reliability/quality/uncertainty gate".into(),
                    "reacquire_or_recalibrate_before_adjudication".into(),
                )
            } else if sign_reversal {
                (
                    ContradictionKind::SignReversal,
                    "eligible modalities report opposite endpoint directions".into(),
                    "acquire_orthogonal_assay_and_review_mechanism_alignment".into(),
                )
            } else if difference > request.max_pair_difference_milli {
                (
                    if trust_preference.is_some() {
                        ContradictionKind::QualityAsymmetry
                    } else {
                        ContradictionKind::MagnitudeConflict
                    },
                    if trust_preference.is_some() {
                        "magnitude conflict has a declared trust asymmetry".into()
                    } else {
                        "eligible modalities exceed the declared pair-difference gate".into()
                    },
                    if trust_preference.is_some() {
                        "retain_rival_evidence_and_reacquire_lower_trust_modality".into()
                    } else {
                        "reacquire_both_modalities_with_orthogonal_measurement".into()
                    },
                )
            } else {
                (
                    ContradictionKind::Agreement,
                    "eligible modalities agree within the declared pair-difference gate".into(),
                    "retain_pair_for_endpoint_fusion".into(),
                )
            };
            if !matches!(kind, ContradictionKind::Agreement) {
                negative.insert(format!(
                    "{:?}:{:?}:{:?}",
                    kind, left.modality, right.modality
                ));
                uncertainty.insert(format!(
                    "pair-requires-resolution:{:?}:{:?}",
                    left.modality, right.modality
                ));
                resolution_rank.push((kind, difference, key));
            }
            if matches!(
                kind,
                ContradictionKind::Unresolved | ContradictionKind::SignReversal
            ) {
                unresolved_pair_order.push(key);
            }
            pairs.push(ModalityPairAdjudication {
                left_modality: left.modality,
                right_modality: right.modality,
                left_value_milli: left.value_milli,
                right_value_milli: right.value_milli,
                absolute_difference_milli: difference,
                left_trust_milli: left_trust,
                right_trust_milli: right_trust,
                kind,
                confidence_milli: confidence(left_trust, right_trust),
                preferred_modality: trust_preference,
                reason,
                resolution_action,
            });
        }
    }
    pairs.sort_by(|left, right| {
        left.left_modality
            .cmp(&right.left_modality)
            .then_with(|| left.right_modality.cmp(&right.right_modality))
    });
    let pair_order = pairs
        .iter()
        .map(|pair| (pair.left_modality, pair.right_modality))
        .collect::<Vec<_>>();
    resolution_rank.sort_by(|left, right| {
        severity(right.0)
            .cmp(&severity(left.0))
            .then_with(|| right.1.cmp(&left.1))
            .then_with(|| left.2.cmp(&right.2))
    });
    let resolution_order = resolution_rank
        .iter()
        .map(|(_, _, pair)| *pair)
        .collect::<Vec<_>>();
    unresolved_pair_order.sort();
    let mut trusted_rank = eligible
        .iter()
        .map(|observation| (observation.modality, trust(observation)))
        .collect::<Vec<_>>();
    trusted_rank.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    let trusted_modality_order = trusted_rank
        .iter()
        .map(|(modality, _)| *modality)
        .collect::<Vec<_>>();
    let missing_required = required
        .iter()
        .any(|modality| !by_modality.contains_key(modality));
    let eligible_floor_failed = eligible.len() < request.min_modalities;
    if missing_required {
        for modality in &missing_modality_order {
            negative.insert(format!("required-modality-missing:{modality:?}"));
        }
    }
    if eligible_floor_failed {
        negative.insert(format!(
            "eligible-modality-floor-not-met:{}",
            request.min_modalities
        ));
    }
    let disposition = if eligible_floor_failed || missing_required || eligible.is_empty() {
        ContradictionAdjudicationDisposition::Blocked
    } else if pairs.iter().any(|pair| {
        matches!(
            pair.kind,
            ContradictionKind::Unresolved | ContradictionKind::SignReversal
        )
    }) {
        ContradictionAdjudicationDisposition::Unresolved
    } else if pairs
        .iter()
        .any(|pair| !matches!(pair.kind, ContradictionKind::Agreement))
    {
        ContradictionAdjudicationDisposition::Conditional
    } else {
        ContradictionAdjudicationDisposition::Ready
    };
    let next_action = match disposition {
        ContradictionAdjudicationDisposition::Ready => {
            "advance concordant endpoint evidence to fusion or decision gating"
        }
        ContradictionAdjudicationDisposition::Conditional => {
            "retain rival measurements and reacquire the lower-trust or orthogonal modality"
        }
        ContradictionAdjudicationDisposition::Blocked => {
            "acquire missing or eligible modality evidence before adjudication"
        }
        ContradictionAdjudicationDisposition::Unresolved => {
            "hold mechanism interpretation and run orthogonal contradiction-resolution work"
        }
    }
    .into();
    let mut output = ContradictionAdjudication {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        endpoint_id: request.endpoint_id.clone(),
        study_id: request.study_id.clone(),
        model_system: request.model_system,
        modality_order,
        eligible_modality_order: eligible_set.iter().copied().collect(),
        missing_modality_order,
        pair_order,
        pairs,
        trusted_modality_order,
        resolution_order,
        unresolved_pair_order,
        negative_evidence: negative.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        next_action,
        digest: ContentHash::of_value(&serde_json::Value::Null)
            .map_err(|error| ContradictionAdjudicationError::Digest(error.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ContradictionAdjudicationError::Digest(error.to_string()))?;
    validate_output(&output)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation(
        modality: GliomaModality,
        value_milli: i64,
        quality_milli: u16,
    ) -> EndpointEvidence {
        EndpointEvidence {
            modality,
            value_milli,
            uncertainty_milli: 20,
            reliability_milli: quality_milli,
            quality_milli,
            replicate_count: 3,
        }
    }

    fn request(observations: Vec<EndpointEvidence>) -> ContradictionAdjudicationRequest {
        ContradictionAdjudicationRequest {
            objective: "adjudicate multimodal invasion contradictions before mechanism planning"
                .into(),
            endpoint_id: "invasion-score".into(),
            study_id: "contradiction-study".into(),
            model_system: GliomaModelSystem::Organoid,
            required_modalities: vec![GliomaModality::Genomics, GliomaModality::Imaging],
            observations,
            min_modalities: 2,
            min_reliability_milli: 700,
            max_uncertainty_milli: 100,
            max_pair_difference_milli: 150,
            min_trust_margin_milli: 200,
        }
    }

    #[test]
    fn sign_reversal_is_unresolved_and_digest_bound() {
        let output = adjudicate_glioma_multimodal_contradictions(&request(vec![
            observation(GliomaModality::Genomics, 700, 900),
            observation(GliomaModality::Imaging, -500, 900),
        ]))
        .expect("contradiction adjudication");
        assert_eq!(
            output.disposition,
            ContradictionAdjudicationDisposition::Unresolved
        );
        assert_eq!(output.pairs[0].kind, ContradictionKind::SignReversal);
        assert_eq!(output.unresolved_pair_order.len(), 1);
        output.validate().expect("digest and invariants");
    }

    #[test]
    fn trust_asymmetry_routes_lower_trust_reacquisition() {
        let mut request = request(vec![
            observation(GliomaModality::Genomics, 700, 950),
            observation(GliomaModality::Imaging, 100, 500),
        ]);
        request.min_reliability_milli = 400;
        let output =
            adjudicate_glioma_multimodal_contradictions(&request).expect("quality asymmetry");
        assert_eq!(
            output.disposition,
            ContradictionAdjudicationDisposition::Conditional
        );
        assert_eq!(output.pairs[0].kind, ContradictionKind::QualityAsymmetry);
        assert_eq!(
            output.pairs[0].preferred_modality,
            Some(GliomaModality::Genomics)
        );
        assert!(output.pairs[0].resolution_action.contains("lower_trust"));
    }

    #[test]
    fn missing_required_modality_blocks_without_pair_imputation() {
        let mut request = request(vec![observation(GliomaModality::Genomics, 700, 900)]);
        request.min_modalities = 1;
        let output = adjudicate_glioma_multimodal_contradictions(&request).expect("blocked output");
        assert_eq!(
            output.disposition,
            ContradictionAdjudicationDisposition::Blocked
        );
        assert!(output
            .missing_modality_order
            .contains(&GliomaModality::Imaging));
        assert!(output.pairs.is_empty());
    }
}
