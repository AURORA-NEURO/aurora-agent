//! Multimodal QC incident root-cause attribution for preclinical glioma research.
//!
//! This feature turns typed QC signals into a ranked remediation queue. It is deliberately an
//! attribution of measurement-process failure, not a biological explanation: each cause retains
//! supporting and contradicting signals, missing modality coverage blocks overconfident output,
//! and competing causes remain explicit for a researcher or the adaptive campaign to resolve.

use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P03-F30";
pub const OUTPUT_SCHEMA: &str = "GliomaMultimodalQualityRootCause1@1";
pub const MAX_MODALITIES: usize = 64;
pub const MAX_SIGNALS: usize = 65_536;
pub const MAX_REPLICATES: u16 = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityRootCause {
    Instrument,
    Batch,
    SamplePreparation,
    Alignment,
    Transport,
    Connector,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualitySignalScope {
    Modality,
    Batch,
    Study,
    CrossStudy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityIncidentSignal {
    pub signal_id: String,
    pub modality: GliomaModality,
    pub cause: QualityRootCause,
    pub scope: QualitySignalScope,
    pub magnitude_milli: u16,
    pub reliability_milli: u16,
    pub replicate_count: u16,
    pub supports_cause: bool,
    pub observed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityRootCauseRequest {
    pub objective: String,
    pub incident_id: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub required_modalities: Vec<GliomaModality>,
    pub signals: Vec<QualityIncidentSignal>,
    pub min_signal_reliability_milli: u16,
    pub min_support_milli: u16,
    pub min_confidence_milli: u16,
    pub min_margin_milli: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityRootCauseDisposition {
    Qualified,
    Competing,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityRootCauseCampaignDisposition {
    Ready,
    Conditional,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityRootCauseAttribution {
    pub cause: QualityRootCause,
    pub support_milli: u16,
    pub contradiction_milli: u16,
    pub net_score_milli: i16,
    pub confidence_milli: u16,
    pub signal_count: u32,
    pub modality_order: Vec<GliomaModality>,
    pub disposition: QualityRootCauseDisposition,
    pub remediation_action: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityRootCauseAttributionResult {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub incident_id: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub modality_order: Vec<GliomaModality>,
    pub signal_order: Vec<String>,
    pub attributions: Vec<QualityRootCauseAttribution>,
    pub primary_cause: Option<QualityRootCause>,
    pub competing_cause_order: Vec<QualityRootCause>,
    pub remediation_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: QualityRootCauseCampaignDisposition,
    pub next_action: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum QualityRootCauseError {
    #[error("quality root-cause request is invalid: {0}")]
    InvalidRequest(String),
    #[error("quality root-cause signal is invalid: {0}")]
    InvalidSignal(String),
    #[error("quality root-cause output is invalid: {0}")]
    InvalidOutput(String),
    #[error("quality root-cause digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn scope_weight(scope: QualitySignalScope) -> u64 {
    match scope {
        QualitySignalScope::Modality => 400,
        QualitySignalScope::Batch => 650,
        QualitySignalScope::Study => 850,
        QualitySignalScope::CrossStudy => 1_000,
    }
}

fn cause_action(cause: QualityRootCause) -> &'static str {
    match cause {
        QualityRootCause::Instrument => {
            "inspect calibration, interlocks, firmware, and instrument drift"
        }
        QualityRootCause::Batch => "re-harmonize batch controls and run cross-batch QC replicates",
        QualityRootCause::SamplePreparation => {
            "audit preanalytic handling and reacquire independent preparation replicates"
        }
        QualityRootCause::Alignment => {
            "re-register modality coordinates and verify shared feature semantics"
        }
        QualityRootCause::Transport => {
            "hold cross-study policy transfer and run target-local calibration"
        }
        QualityRootCause::Connector => {
            "quarantine the connector and replay ingestion from a pinned artifact"
        }
        QualityRootCause::Unknown => {
            "collect orthogonal QC evidence before assigning a remediation cause"
        }
    }
}

fn digest_input(output: &QualityRootCauseAttributionResult) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "incident_id": output.incident_id,
        "study_id": output.study_id,
        "model_system": output.model_system,
        "modality_order": output.modality_order,
        "signal_order": output.signal_order,
        "attributions": output.attributions,
        "primary_cause": output.primary_cause,
        "competing_cause_order": output.competing_cause_order,
        "remediation_order": output.remediation_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
        "next_action": output.next_action,
    })
}

fn validate_request(request: &QualityRootCauseRequest) -> Result<(), QualityRootCauseError> {
    if request.objective.trim().is_empty()
        || request.incident_id.trim().is_empty()
        || request.study_id.trim().is_empty()
        || request.required_modalities.is_empty()
        || request.required_modalities.len() > MAX_MODALITIES
        || !canonical(&request.required_modalities)
        || request.signals.is_empty()
        || request.signals.len() > MAX_SIGNALS
        || request.min_signal_reliability_milli > 1_000
        || request.min_support_milli > 1_000
        || request.min_confidence_milli > 1_000
        || request.min_margin_milli > 1_000
    {
        return Err(QualityRootCauseError::InvalidRequest(
            "objective, incident/study binding, canonical modalities, bounded signals, and attribution gates are required".into(),
        ));
    }
    let modalities = request
        .required_modalities
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut signal_ids = BTreeSet::new();
    for signal in &request.signals {
        if signal.signal_id.trim().is_empty()
            || !modalities.contains(&signal.modality)
            || signal.magnitude_milli > 1_000
            || signal.reliability_milli > 1_000
            || signal.replicate_count == 0
            || signal.replicate_count > MAX_REPLICATES
            || !signal_ids.insert(signal.signal_id.clone())
        {
            return Err(QualityRootCauseError::InvalidSignal(
                "signals require unique IDs, declared modalities, bounded magnitudes/reliability, and positive replicate counts".into(),
            ));
        }
    }
    Ok(())
}

fn validate_output(
    output: &QualityRootCauseAttributionResult,
) -> Result<(), QualityRootCauseError> {
    if output.feature_id != FEATURE_ID
        || output.output_schema != OUTPUT_SCHEMA
        || output.objective.trim().is_empty()
        || output.incident_id.trim().is_empty()
        || output.study_id.trim().is_empty()
        || !canonical(&output.modality_order)
        || !canonical(&output.signal_order)
        || !canonical(&output.competing_cause_order)
        || !canonical(&output.remediation_order)
        || !canonical(&output.negative_evidence)
        || !canonical(&output.uncertainty)
        || output
            .attributions
            .windows(2)
            .any(|pair| pair[0].cause >= pair[1].cause)
        || output.attributions.iter().any(|attribution| {
            attribution.support_milli > 1_000
                || attribution.contradiction_milli > 1_000
                || attribution.confidence_milli > 1_000
                || attribution.signal_count == 0
                || !canonical(&attribution.modality_order)
                || attribution.remediation_action.trim().is_empty()
        })
    {
        return Err(QualityRootCauseError::InvalidOutput(
            "identity, canonical order, attribution bounds, signal counts, or remediation invariants are invalid".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(output))
        .map_err(|error| QualityRootCauseError::Digest(error.to_string()))?;
    if expected != output.digest {
        return Err(QualityRootCauseError::InvalidOutput(
            "quality root-cause digest is not content-addressed".into(),
        ));
    }
    Ok(())
}

impl QualityRootCauseAttributionResult {
    pub fn validate(&self) -> Result<(), QualityRootCauseError> {
        validate_output(self)
    }
}

#[derive(Default)]
struct CauseAccumulator {
    support: u64,
    contradiction: u64,
    signal_count: u32,
    modalities: BTreeSet<GliomaModality>,
}

/// Attribute a local QC incident to measurement-process causes and return a deterministic
/// remediation queue. A qualified cause is never a biological conclusion.
pub fn attribute_glioma_multimodal_quality_root_cause(
    request: &QualityRootCauseRequest,
) -> Result<QualityRootCauseAttributionResult, QualityRootCauseError> {
    validate_request(request)?;
    let required = request
        .required_modalities
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut accumulators = BTreeMap::<QualityRootCause, CauseAccumulator>::new();
    let mut observed_modalities = BTreeSet::new();
    let mut signal_order = Vec::new();
    let mut negative = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for signal in &request.signals {
        signal_order.push(signal.signal_id.clone());
        if !signal.observed {
            uncertainty.insert(format!("unobserved-signal:{}", signal.signal_id));
            continue;
        }
        if signal.reliability_milli < request.min_signal_reliability_milli {
            uncertainty.insert(format!("low-reliability-signal:{}", signal.signal_id));
            continue;
        }
        observed_modalities.insert(signal.modality);
        let replicate_weight =
            (500_u64 + u64::from(signal.replicate_count).saturating_mul(100)).min(1_000);
        let contribution = u64::from(signal.magnitude_milli)
            .saturating_mul(u64::from(signal.reliability_milli))
            .saturating_mul(scope_weight(signal.scope))
            .saturating_mul(replicate_weight)
            / 1_000_000_000;
        let accumulator = accumulators.entry(signal.cause).or_default();
        if signal.supports_cause {
            accumulator.support = accumulator.support.saturating_add(contribution);
        } else {
            accumulator.contradiction = accumulator.contradiction.saturating_add(contribution);
            negative.insert(format!("contradicting-signal:{}", signal.signal_id));
        }
        accumulator.signal_count = accumulator.signal_count.saturating_add(1);
        accumulator.modalities.insert(signal.modality);
    }
    signal_order.sort();
    for modality in &required {
        if !observed_modalities.contains(modality) {
            negative.insert(format!("missing-cause-evidence:{modality:?}"));
        }
    }
    let missing_required = required
        .iter()
        .any(|modality| !observed_modalities.contains(modality));
    let mut ranked = accumulators
        .into_iter()
        .map(|(cause, accumulator)| {
            let total = accumulator
                .support
                .saturating_add(accumulator.contradiction)
                .max(1);
            let net = accumulator
                .support
                .saturating_sub(accumulator.contradiction);
            let signed_net = if accumulator.support >= accumulator.contradiction {
                net.min(1_000) as i16
            } else {
                -(accumulator
                    .contradiction
                    .saturating_sub(accumulator.support)
                    .min(1_000) as i16)
            };
            let confidence = (net.saturating_mul(1_000) / total).min(1_000) as u16;
            (cause, accumulator, confidence, signed_net)
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        right
            .2
            .cmp(&left.2)
            .then_with(|| right.3.cmp(&left.3))
            .then_with(|| left.0.cmp(&right.0))
    });
    let top_confidence = ranked.first().map_or(0, |entry| entry.2);
    let second_confidence = ranked.get(1).map_or(0, |entry| entry.2);
    let margin = top_confidence.saturating_sub(second_confidence);
    let primary_ranked_cause = ranked.first().map(|entry| entry.0);
    let mut attributions = ranked
        .into_iter()
        .map(|(cause, accumulator, confidence, signed_net)| {
            let disposition = if confidence < request.min_confidence_milli
                || signed_net < request.min_support_milli as i16
            {
                QualityRootCauseDisposition::Unresolved
            } else if confidence == top_confidence && margin < request.min_margin_milli {
                QualityRootCauseDisposition::Competing
            } else {
                QualityRootCauseDisposition::Qualified
            };
            QualityRootCauseAttribution {
                cause,
                support_milli: accumulator.support.min(1_000) as u16,
                contradiction_milli: accumulator.contradiction.min(1_000) as u16,
                net_score_milli: signed_net,
                confidence_milli: confidence,
                signal_count: accumulator.signal_count,
                modality_order: accumulator.modalities.into_iter().collect(),
                disposition,
                remediation_action: cause_action(cause).into(),
            }
        })
        .collect::<Vec<_>>();
    attributions.sort_by_key(|attribution| attribution.cause);
    let ranked_causes = attributions
        .iter()
        .filter(|attribution| attribution.confidence_milli > 0)
        .collect::<Vec<_>>();
    let primary_cause = if !missing_required
        && margin >= request.min_margin_milli
        && top_confidence >= request.min_confidence_milli
    {
        primary_ranked_cause
    } else {
        None
    };
    let competing_cause_order = ranked_causes
        .iter()
        .filter(|attribution| attribution.confidence_milli >= request.min_confidence_milli)
        .map(|attribution| attribution.cause)
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let remediation_order = ranked_causes
        .iter()
        .map(|attribution| attribution.remediation_action.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let disposition = if missing_required {
        QualityRootCauseCampaignDisposition::Blocked
    } else if primary_cause.is_some() {
        QualityRootCauseCampaignDisposition::Ready
    } else if !ranked_causes.is_empty() {
        QualityRootCauseCampaignDisposition::Conditional
    } else {
        QualityRootCauseCampaignDisposition::Unresolved
    };
    let next_action = match disposition {
        QualityRootCauseCampaignDisposition::Ready => {
            "route the primary QC remediation action to the adaptive quality campaign"
        }
        QualityRootCauseCampaignDisposition::Conditional => {
            "collect orthogonal QC signals before selecting one remediation cause"
        }
        QualityRootCauseCampaignDisposition::Blocked => {
            "collect missing required-modality QC evidence before attribution"
        }
        QualityRootCauseCampaignDisposition::Unresolved => {
            "hold remediation and request higher-reliability QC evidence"
        }
    }
    .into();
    let modality_order = required.into_iter().collect::<Vec<_>>();
    let mut output = QualityRootCauseAttributionResult {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        incident_id: request.incident_id.clone(),
        study_id: request.study_id.clone(),
        model_system: request.model_system,
        modality_order,
        signal_order,
        attributions,
        primary_cause,
        competing_cause_order,
        remediation_order,
        negative_evidence: negative.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        next_action,
        digest: ContentHash::of_bytes(b"unsealed-glioma-quality-root-cause"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| QualityRootCauseError::Digest(error.to_string()))?;
    validate_output(&output)?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signal(
        signal_id: &str,
        modality: GliomaModality,
        cause: QualityRootCause,
        supports_cause: bool,
    ) -> QualityIncidentSignal {
        QualityIncidentSignal {
            signal_id: signal_id.into(),
            modality,
            cause,
            scope: QualitySignalScope::Batch,
            magnitude_milli: 900,
            reliability_milli: 900,
            replicate_count: 3,
            supports_cause,
            observed: true,
        }
    }

    fn request(signals: Vec<QualityIncidentSignal>) -> QualityRootCauseRequest {
        QualityRootCauseRequest {
            objective: "attribute a multimodal QC incident before reacquisition".into(),
            incident_id: "incident-1".into(),
            study_id: "root-cause-study".into(),
            model_system: GliomaModelSystem::Organoid,
            required_modalities: vec![GliomaModality::Genomics, GliomaModality::Imaging],
            signals,
            min_signal_reliability_milli: 700,
            min_support_milli: 400,
            min_confidence_milli: 600,
            min_margin_milli: 150,
        }
    }

    #[test]
    fn qualifies_replicated_batch_root_cause_and_routes_remediation() {
        let output = attribute_glioma_multimodal_quality_root_cause(&request(vec![
            signal(
                "batch-g",
                GliomaModality::Genomics,
                QualityRootCause::Batch,
                true,
            ),
            signal(
                "batch-i",
                GliomaModality::Imaging,
                QualityRootCause::Batch,
                true,
            ),
            signal(
                "instrument-i",
                GliomaModality::Imaging,
                QualityRootCause::Instrument,
                false,
            ),
        ]))
        .expect("root cause");
        assert_eq!(
            output.disposition,
            QualityRootCauseCampaignDisposition::Ready
        );
        assert_eq!(output.primary_cause, Some(QualityRootCause::Batch));
        assert!(output
            .remediation_order
            .iter()
            .any(|action| action.contains("batch")));
        output.validate().expect("digest and invariants");
    }

    #[test]
    fn blocks_when_required_modality_has_no_observed_cause_signal() {
        let output = attribute_glioma_multimodal_quality_root_cause(&request(vec![signal(
            "batch-g",
            GliomaModality::Genomics,
            QualityRootCause::Batch,
            true,
        )]))
        .expect("blocked root cause");
        assert_eq!(
            output.disposition,
            QualityRootCauseCampaignDisposition::Blocked
        );
        assert!(output.primary_cause.is_none());
        assert!(output
            .negative_evidence
            .iter()
            .any(|entry| entry.contains("missing-cause-evidence")));
    }

    #[test]
    fn preserves_competing_causes_when_margin_is_small() {
        let output = attribute_glioma_multimodal_quality_root_cause(&request(vec![
            signal(
                "batch-g",
                GliomaModality::Genomics,
                QualityRootCause::Batch,
                true,
            ),
            signal(
                "instrument-g",
                GliomaModality::Genomics,
                QualityRootCause::Instrument,
                true,
            ),
            signal(
                "batch-i",
                GliomaModality::Imaging,
                QualityRootCause::Batch,
                true,
            ),
            signal(
                "instrument-i",
                GliomaModality::Imaging,
                QualityRootCause::Instrument,
                true,
            ),
        ]))
        .expect("competing root cause");
        assert_eq!(
            output.disposition,
            QualityRootCauseCampaignDisposition::Conditional
        );
        assert!(output.primary_cause.is_none());
        assert!(output.competing_cause_order.len() >= 2);
    }
}
