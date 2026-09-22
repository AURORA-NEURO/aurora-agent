//! Conservative verification of multimodal QC recovery after a remediation action.
//!
//! The verifier closes the P03 quality loop: it compares declared baseline and post-remediation
//! observations, requires measurable improvement and target floors, and emits an explicit
//! proceed/iterate/escalate disposition. It is a measurement-process gate, never a biological or
//! clinical conclusion.

use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P03-F32";
pub const OUTPUT_SCHEMA: &str = "GliomaMultimodalQualityRecovery1@1";
pub const MAX_MODALITIES: usize = 64;
pub const MAX_OBSERVATIONS: usize = 65_536;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityRecoveryPhase {
    Baseline,
    PostRemediation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityRecoveryObservation {
    pub observation_id: String,
    pub modality: GliomaModality,
    pub phase: QualityRecoveryPhase,
    pub quality_milli: u16,
    pub coverage_milli: u16,
    pub alignment_milli: u16,
    pub drift_milli: u16,
    pub reliability_milli: u16,
    pub sample_count: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityRecoveryRequest {
    pub objective: String,
    pub incident_id: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub remediation_action_id: String,
    pub required_modalities: Vec<GliomaModality>,
    pub observations: Vec<QualityRecoveryObservation>,
    pub min_samples_per_phase: u16,
    pub min_reliability_milli: u16,
    pub min_quality_milli: u16,
    pub min_coverage_milli: u16,
    pub min_alignment_milli: u16,
    pub max_drift_milli: u16,
    pub min_improvement_milli: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityRecoveryModalityDisposition {
    Recovered,
    Partial,
    Failed,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityRecoveryModalityResult {
    pub modality: GliomaModality,
    pub baseline_quality_milli: Option<u16>,
    pub post_quality_milli: Option<u16>,
    pub baseline_coverage_milli: Option<u16>,
    pub post_coverage_milli: Option<u16>,
    pub baseline_alignment_milli: Option<u16>,
    pub post_alignment_milli: Option<u16>,
    pub baseline_drift_milli: Option<u16>,
    pub post_drift_milli: Option<u16>,
    pub quality_delta_milli: Option<i16>,
    pub coverage_delta_milli: Option<i16>,
    pub alignment_delta_milli: Option<i16>,
    pub drift_delta_milli: Option<i16>,
    pub confidence_milli: u16,
    pub baseline_observation_count: u16,
    pub post_observation_count: u16,
    pub disposition: QualityRecoveryModalityDisposition,
    pub evidence: Vec<String>,
    pub next_action: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityRecoveryCampaignDisposition {
    Recovered,
    Partial,
    Failed,
    Blocked,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityRecoveryResult {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub incident_id: String,
    pub study_id: String,
    pub model_system: GliomaModelSystem,
    pub remediation_action_id: String,
    pub modality_order: Vec<GliomaModality>,
    pub observation_order: Vec<String>,
    pub modalities: Vec<QualityRecoveryModalityResult>,
    pub proceed_modalities: Vec<GliomaModality>,
    pub iterate_modalities: Vec<GliomaModality>,
    pub escalate_modalities: Vec<GliomaModality>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: QualityRecoveryCampaignDisposition,
    pub next_action: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum QualityRecoveryError {
    #[error("quality recovery request is invalid: {0}")]
    InvalidRequest(String),
    #[error("quality recovery observation is invalid: {0}")]
    InvalidObservation(String),
    #[error("quality recovery output is invalid: {0}")]
    InvalidOutput(String),
    #[error("quality recovery digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn median_lower(values: &mut [u16]) -> u16 {
    values.sort_unstable();
    values[(values.len() - 1) / 2]
}

fn signed_delta(post: u16, baseline: u16) -> i16 {
    (post as i16).saturating_sub(baseline as i16)
}

fn digest_input(result: &QualityRecoveryResult) -> serde_json::Value {
    serde_json::json!({
        "feature_id": result.feature_id,
        "output_schema": result.output_schema,
        "objective": result.objective,
        "incident_id": result.incident_id,
        "study_id": result.study_id,
        "model_system": result.model_system,
        "remediation_action_id": result.remediation_action_id,
        "modality_order": result.modality_order,
        "observation_order": result.observation_order,
        "modalities": result.modalities,
        "proceed_modalities": result.proceed_modalities,
        "iterate_modalities": result.iterate_modalities,
        "escalate_modalities": result.escalate_modalities,
        "negative_evidence": result.negative_evidence,
        "uncertainty": result.uncertainty,
        "disposition": result.disposition,
        "next_action": result.next_action,
    })
}

fn validate_request(request: &QualityRecoveryRequest) -> Result<(), QualityRecoveryError> {
    if request.objective.trim().is_empty()
        || request.incident_id.trim().is_empty()
        || request.study_id.trim().is_empty()
        || request.remediation_action_id.trim().is_empty()
        || request.required_modalities.is_empty()
        || request.required_modalities.len() > MAX_MODALITIES
        || !canonical(&request.required_modalities)
        || request.observations.is_empty()
        || request.observations.len() > MAX_OBSERVATIONS
        || request.min_samples_per_phase == 0
        || request.min_reliability_milli > 1_000
        || request.min_quality_milli > 1_000
        || request.min_coverage_milli > 1_000
        || request.min_alignment_milli > 1_000
        || request.max_drift_milli > 1_000
        || request.min_improvement_milli > 1_000
    {
        return Err(QualityRecoveryError::InvalidRequest(
            "incident/action binding, canonical modalities, bounded observations, and valid recovery gates are required".into(),
        ));
    }
    let modalities = request
        .required_modalities
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut ids = BTreeSet::new();
    for observation in &request.observations {
        if observation.observation_id.trim().is_empty()
            || !ids.insert(observation.observation_id.clone())
            || !modalities.contains(&observation.modality)
            || observation.quality_milli > 1_000
            || observation.coverage_milli > 1_000
            || observation.alignment_milli > 1_000
            || observation.drift_milli > 1_000
            || observation.reliability_milli > 1_000
            || observation.sample_count == 0
        {
            return Err(QualityRecoveryError::InvalidObservation(
                "observations require unique IDs, declared modalities, bounded metrics, and positive sample counts".into(),
            ));
        }
    }
    Ok(())
}

fn validate_output(result: &QualityRecoveryResult) -> Result<(), QualityRecoveryError> {
    if result.feature_id != FEATURE_ID
        || result.output_schema != OUTPUT_SCHEMA
        || result.objective.trim().is_empty()
        || result.incident_id.trim().is_empty()
        || result.study_id.trim().is_empty()
        || result.remediation_action_id.trim().is_empty()
        || !canonical(&result.modality_order)
        || !canonical(&result.observation_order)
        || !canonical(&result.proceed_modalities)
        || !canonical(&result.iterate_modalities)
        || !canonical(&result.escalate_modalities)
        || !canonical(&result.negative_evidence)
        || !canonical(&result.uncertainty)
        || result
            .modalities
            .windows(2)
            .any(|pair| pair[0].modality >= pair[1].modality)
        || result.modalities.iter().any(|modality| {
            modality.confidence_milli > 1_000
                || modality.evidence.is_empty()
                || !canonical(&modality.evidence)
                || modality.next_action.trim().is_empty()
        })
    {
        return Err(QualityRecoveryError::InvalidOutput(
            "identity, canonical ordering, evidence, confidence, or modality invariants are invalid".into(),
        ));
    }
    let expected = ContentHash::of_value(&digest_input(result))
        .map_err(|error| QualityRecoveryError::Digest(error.to_string()))?;
    if expected != result.digest {
        return Err(QualityRecoveryError::InvalidOutput(
            "quality recovery digest is not content-addressed".into(),
        ));
    }
    Ok(())
}

impl QualityRecoveryResult {
    pub fn validate(&self) -> Result<(), QualityRecoveryError> {
        validate_output(self)
    }
}

#[derive(Default)]
struct PhaseAccumulator {
    quality: Vec<u16>,
    coverage: Vec<u16>,
    alignment: Vec<u16>,
    drift: Vec<u16>,
    reliability: Vec<u16>,
    sample_count: u16,
}

fn summarize(accumulator: &PhaseAccumulator) -> (u16, u16, u16, u16, u16, u16) {
    let mut quality = accumulator.quality.clone();
    let mut coverage = accumulator.coverage.clone();
    let mut alignment = accumulator.alignment.clone();
    let mut drift = accumulator.drift.clone();
    let mut reliability = accumulator.reliability.clone();
    (
        median_lower(&mut quality),
        median_lower(&mut coverage),
        median_lower(&mut alignment),
        median_lower(&mut drift),
        median_lower(&mut reliability),
        accumulator.sample_count,
    )
}

/// Verify whether a declared QC remediation action recovered each required modality.
pub fn verify_glioma_multimodal_quality_recovery(
    request: &QualityRecoveryRequest,
) -> Result<QualityRecoveryResult, QualityRecoveryError> {
    validate_request(request)?;
    let required = request
        .required_modalities
        .iter()
        .copied()
        .collect::<BTreeSet<_>>();
    let mut phases = BTreeMap::<GliomaModality, (PhaseAccumulator, PhaseAccumulator)>::new();
    let mut observation_order = request
        .observations
        .iter()
        .map(|observation| observation.observation_id.clone())
        .collect::<Vec<_>>();
    observation_order.sort();
    for observation in &request.observations {
        let (baseline, post) = phases.entry(observation.modality).or_default();
        let accumulator = match observation.phase {
            QualityRecoveryPhase::Baseline => baseline,
            QualityRecoveryPhase::PostRemediation => post,
        };
        accumulator.quality.push(observation.quality_milli);
        accumulator.coverage.push(observation.coverage_milli);
        accumulator.alignment.push(observation.alignment_milli);
        accumulator.drift.push(observation.drift_milli);
        accumulator.reliability.push(observation.reliability_milli);
        accumulator.sample_count = accumulator
            .sample_count
            .saturating_add(observation.sample_count);
    }
    let mut modalities = Vec::new();
    let mut proceed = BTreeSet::new();
    let mut iterate = BTreeSet::new();
    let mut escalate = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for modality in &required {
        let (baseline, post) = phases.remove(modality).unwrap_or_default();
        let mut evidence = BTreeSet::new();
        let (baseline_summary, post_summary) = if baseline.quality.is_empty() {
            evidence.insert("missing-baseline-observation".into());
            (None, None)
        } else if post.quality.is_empty() {
            evidence.insert("missing-post-remediation-observation".into());
            (Some(summarize(&baseline)), None)
        } else {
            (Some(summarize(&baseline)), Some(summarize(&post)))
        };
        let mut quality_delta = None;
        let mut coverage_delta = None;
        let mut alignment_delta = None;
        let mut drift_delta = None;
        let (disposition, confidence, next_action) = match (baseline_summary, post_summary) {
            (Some((bq, bc, ba, bd, br, bs)), Some((pq, pc, pa, pd, pr, ps)))
                if bs >= request.min_samples_per_phase
                    && ps >= request.min_samples_per_phase
                    && br >= request.min_reliability_milli
                    && pr >= request.min_reliability_milli =>
            {
                let qd = signed_delta(pq, bq);
                let cd = signed_delta(pc, bc);
                let ad = signed_delta(pa, ba);
                let dd = signed_delta(bd, pd);
                quality_delta = Some(qd);
                coverage_delta = Some(cd);
                alignment_delta = Some(ad);
                drift_delta = Some(dd);
                let floors_met = pq >= request.min_quality_milli
                    && pc >= request.min_coverage_milli
                    && pa >= request.min_alignment_milli
                    && pd <= request.max_drift_milli;
                let improvement_met = qd >= request.min_improvement_milli as i16
                    && cd >= request.min_improvement_milli as i16
                    && ad >= request.min_improvement_milli as i16
                    && dd >= 0;
                let confidence = u16::from(br.min(pr));
                if floors_met && improvement_met {
                    evidence.insert("post-remediation-floors-met".into());
                    evidence.insert("all-required-improvements-met".into());
                    (
                        QualityRecoveryModalityDisposition::Recovered,
                        confidence,
                        "permit downstream multimodal analysis for this modality after campaign approval".into(),
                    )
                } else if floors_met {
                    evidence.insert("post-remediation-floors-met".into());
                    evidence.insert("improvement-floor-not-met".into());
                    (
                        QualityRecoveryModalityDisposition::Partial,
                        confidence,
                        "iterate bounded QC remediation and collect another paired observation"
                            .into(),
                    )
                } else {
                    evidence.insert("post-remediation-floor-failed".into());
                    negative.insert(format!("quality-floor-failed:{modality:?}"));
                    (
                        QualityRecoveryModalityDisposition::Failed,
                        confidence,
                        "escalate the remediation cause and prevent downstream analysis admission"
                            .into(),
                    )
                }
            }
            (Some(_), Some(_)) => {
                evidence.insert("sample-or-reliability-floor-failed".into());
                uncertainty.insert(format!("insufficient-recovery-evidence:{modality:?}"));
                (
                    QualityRecoveryModalityDisposition::Blocked,
                    0,
                    "collect reliable paired observations before judging recovery".into(),
                )
            }
            _ => (
                QualityRecoveryModalityDisposition::Blocked,
                0,
                "collect both baseline and post-remediation observations".into(),
            ),
        };
        match disposition {
            QualityRecoveryModalityDisposition::Recovered => {
                proceed.insert(*modality);
            }
            QualityRecoveryModalityDisposition::Partial => {
                iterate.insert(*modality);
            }
            QualityRecoveryModalityDisposition::Failed => {
                escalate.insert(*modality);
            }
            QualityRecoveryModalityDisposition::Blocked => {
                uncertainty.insert(format!("blocked-modality:{modality:?}"));
            }
        }
        modalities.push(QualityRecoveryModalityResult {
            modality: *modality,
            baseline_quality_milli: baseline_summary.map(|summary| summary.0),
            post_quality_milli: post_summary.map(|summary| summary.0),
            baseline_coverage_milli: baseline_summary.map(|summary| summary.1),
            post_coverage_milli: post_summary.map(|summary| summary.1),
            baseline_alignment_milli: baseline_summary.map(|summary| summary.2),
            post_alignment_milli: post_summary.map(|summary| summary.2),
            baseline_drift_milli: baseline_summary.map(|summary| summary.3),
            post_drift_milli: post_summary.map(|summary| summary.3),
            quality_delta_milli: quality_delta,
            coverage_delta_milli: coverage_delta,
            alignment_delta_milli: alignment_delta,
            drift_delta_milli: drift_delta,
            confidence_milli: confidence,
            baseline_observation_count: baseline.quality.len() as u16,
            post_observation_count: post.quality.len() as u16,
            disposition,
            evidence: evidence.into_iter().collect(),
            next_action,
        });
    }
    let disposition = if !escalate.is_empty() {
        QualityRecoveryCampaignDisposition::Failed
    } else if modalities
        .iter()
        .any(|modality| modality.disposition == QualityRecoveryModalityDisposition::Blocked)
    {
        QualityRecoveryCampaignDisposition::Blocked
    } else if !iterate.is_empty() {
        QualityRecoveryCampaignDisposition::Partial
    } else if proceed.len() == required.len() {
        QualityRecoveryCampaignDisposition::Recovered
    } else {
        QualityRecoveryCampaignDisposition::Unresolved
    };
    let next_action = match disposition {
        QualityRecoveryCampaignDisposition::Recovered => {
            "permit the downstream research workflow to proceed under its existing policy gates"
        }
        QualityRecoveryCampaignDisposition::Partial => {
            "iterate bounded remediation for modalities below the improvement gate"
        }
        QualityRecoveryCampaignDisposition::Failed => {
            "escalate failed modalities and keep them out of downstream analysis"
        }
        QualityRecoveryCampaignDisposition::Blocked => {
            "collect missing or reliable paired QC observations before deciding recovery"
        }
        QualityRecoveryCampaignDisposition::Unresolved => {
            "hold downstream analysis and request additional independent QC evidence"
        }
    }
    .into();
    let mut result = QualityRecoveryResult {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        incident_id: request.incident_id.clone(),
        study_id: request.study_id.clone(),
        model_system: request.model_system,
        remediation_action_id: request.remediation_action_id.clone(),
        modality_order: required.iter().copied().collect(),
        observation_order,
        modalities,
        proceed_modalities: proceed.into_iter().collect(),
        iterate_modalities: iterate.into_iter().collect(),
        escalate_modalities: escalate.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        next_action,
        digest: ContentHash::of_bytes(b"unsealed-glioma-quality-recovery"),
    };
    result.digest = ContentHash::of_value(&digest_input(&result))
        .map_err(|error| QualityRecoveryError::Digest(error.to_string()))?;
    validate_output(&result)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation(
        id: &str,
        modality: GliomaModality,
        phase: QualityRecoveryPhase,
        quality: u16,
        coverage: u16,
        alignment: u16,
        drift: u16,
    ) -> QualityRecoveryObservation {
        QualityRecoveryObservation {
            observation_id: id.into(),
            modality,
            phase,
            quality_milli: quality,
            coverage_milli: coverage,
            alignment_milli: alignment,
            drift_milli: drift,
            reliability_milli: 900,
            sample_count: 4,
        }
    }

    fn request(observations: Vec<QualityRecoveryObservation>) -> QualityRecoveryRequest {
        QualityRecoveryRequest {
            objective: "verify QC recovery before glioma analysis admission".into(),
            incident_id: "incident-1".into(),
            study_id: "study-1".into(),
            model_system: GliomaModelSystem::Organoid,
            remediation_action_id: "reharmonize-batch".into(),
            required_modalities: vec![GliomaModality::Genomics],
            observations,
            min_samples_per_phase: 2,
            min_reliability_milli: 700,
            min_quality_milli: 800,
            min_coverage_milli: 800,
            min_alignment_milli: 800,
            max_drift_milli: 100,
            min_improvement_milli: 50,
        }
    }

    #[test]
    fn recognizes_recovered_modality_after_paired_improvement() {
        let result = verify_glioma_multimodal_quality_recovery(&request(vec![
            observation(
                "b",
                GliomaModality::Genomics,
                QualityRecoveryPhase::Baseline,
                600,
                650,
                700,
                300,
            ),
            observation(
                "p",
                GliomaModality::Genomics,
                QualityRecoveryPhase::PostRemediation,
                900,
                900,
                900,
                50,
            ),
        ]))
        .expect("recovery");
        assert_eq!(
            result.disposition,
            QualityRecoveryCampaignDisposition::Recovered
        );
        assert_eq!(result.proceed_modalities, vec![GliomaModality::Genomics]);
        result.validate().expect("digest and invariants");
    }

    #[test]
    fn blocks_missing_post_remediation_evidence() {
        let result = verify_glioma_multimodal_quality_recovery(&request(vec![observation(
            "b",
            GliomaModality::Genomics,
            QualityRecoveryPhase::Baseline,
            600,
            650,
            700,
            300,
        )]))
        .expect("blocked recovery");
        assert_eq!(
            result.disposition,
            QualityRecoveryCampaignDisposition::Blocked
        );
        assert!(result
            .uncertainty
            .iter()
            .any(|entry| entry.contains("blocked-modality")));
    }

    #[test]
    fn fails_when_post_remediation_floor_remains_below_gate() {
        let result = verify_glioma_multimodal_quality_recovery(&request(vec![
            observation(
                "b",
                GliomaModality::Genomics,
                QualityRecoveryPhase::Baseline,
                600,
                650,
                700,
                300,
            ),
            observation(
                "p",
                GliomaModality::Genomics,
                QualityRecoveryPhase::PostRemediation,
                700,
                700,
                700,
                250,
            ),
        ]))
        .expect("failed recovery");
        assert_eq!(
            result.disposition,
            QualityRecoveryCampaignDisposition::Failed
        );
        assert_eq!(result.escalate_modalities, vec![GliomaModality::Genomics]);
    }
}
