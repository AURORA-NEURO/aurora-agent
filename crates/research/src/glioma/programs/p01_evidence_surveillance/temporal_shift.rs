//! Prospective temporal-shift detection for preclinical glioma evidence streams.
//!
//! A continuous research engine needs to distinguish a genuine change in a claim from a noisy
//! batch update. This module aggregates only caller-supplied, de-identified observation summaries
//! into baseline/recent windows, downweights uncertain observations, reports heterogeneity, and
//! emits bounded re-review actions for emergent or reversing evidence. It never fetches sources,
//! infers unmeasured biology, or makes a clinical decision.

use crate::glioma_engine::{GliomaModality, GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P01-F03";
pub const OUTPUT_SCHEMA: &str = "GliomaEvidenceTemporalShift1@1";
pub const MAX_OBSERVATIONS: usize = 16_384;
pub const MAX_GROUPS: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceTemporalObservation {
    pub evidence_id: String,
    pub study_id: String,
    pub release_tick: u64,
    pub signal_milli: i32,
    pub uncertainty_milli: u16,
    pub quality_milli: u16,
    pub replicate_count: u16,
    pub modality: GliomaModality,
    pub model_system: Option<GliomaModelSystem>,
    pub artifact: LocalArtifactRef,
    pub preclinical_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceTemporalShiftRequest {
    pub objective: String,
    pub snapshot_id: String,
    pub as_of_tick: u64,
    pub baseline_window_ticks: u64,
    pub recent_window_ticks: u64,
    pub min_observations_per_window: usize,
    pub min_change_milli: u64,
    pub min_priority_milli: u16,
    pub max_actions: usize,
    pub observations: Vec<EvidenceTemporalObservation>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceTemporalShiftKind {
    EmergentPositive,
    EmergentNegative,
    Reversal,
    Stable,
    Contradictory,
    Insufficient,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceTemporalShiftAction {
    pub action_id: String,
    pub evidence_id: String,
    pub kind: EvidenceTemporalShiftKind,
    pub baseline_mean_milli: i32,
    pub recent_mean_milli: i32,
    pub delta_milli: i32,
    pub baseline_uncertainty_milli: u64,
    pub recent_uncertainty_milli: u64,
    pub heterogeneity_milli: u16,
    pub baseline_count: usize,
    pub recent_count: usize,
    pub priority_milli: u16,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceTemporalShiftDisposition {
    Ready,
    Partial,
    NoMaterialShift,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceTemporalShift {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub snapshot_id: String,
    pub candidate_order: Vec<String>,
    pub action_order: Vec<String>,
    pub actions: Vec<EvidenceTemporalShiftAction>,
    pub stable_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub contradictory_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: EvidenceTemporalShiftDisposition,
    pub next_step: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EvidenceTemporalShiftError {
    #[error("evidence temporal shift request is invalid: {0}")]
    InvalidRequest(String),
    #[error("evidence temporal observation is invalid: {0}")]
    InvalidObservation(String),
    #[error("evidence temporal shift output is invalid: {0}")]
    InvalidOutput(String),
    #[error("evidence temporal shift digest failed: {0}")]
    Digest(String),
}

#[derive(Debug, Clone, Copy)]
struct WindowSummary {
    count: usize,
    mean_milli: i32,
    uncertainty_milli: u64,
    heterogeneity_milli: u16,
    quality_milli: u16,
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
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

fn observation_weight(observation: &EvidenceTemporalObservation) -> u128 {
    let variance = u128::from(observation.uncertainty_milli)
        .saturating_mul(u128::from(observation.uncertainty_milli))
        .max(1);
    u128::from(observation.replicate_count)
        .saturating_mul(u128::from(observation.quality_milli.max(1)))
        .saturating_mul(1_000_000)
        / variance
}

fn summarize(observations: &[&EvidenceTemporalObservation]) -> WindowSummary {
    let total_weight = observations
        .iter()
        .map(|observation| observation_weight(observation))
        .sum::<u128>();
    let mean_milli = if total_weight == 0 {
        0
    } else {
        observations
            .iter()
            .map(|observation| {
                i128::from(observation.signal_milli) * observation_weight(observation) as i128
            })
            .sum::<i128>()
            .checked_div(total_weight as i128)
            .unwrap_or(0) as i32
    };
    let uncertainty_milli = if total_weight == 0 {
        u64::MAX
    } else {
        (1_000_000_u128 / integer_sqrt(total_weight).max(1)).min(u128::from(u64::MAX)) as u64
    };
    let max_deviation = observations
        .iter()
        .map(|observation| observation.signal_milli.abs_diff(mean_milli))
        .max()
        .unwrap_or(0);
    let heterogeneity_milli = max_deviation.min(1_000) as u16;
    let quality_milli = if observations.is_empty() {
        0
    } else {
        (observations
            .iter()
            .map(|observation| u64::from(observation.quality_milli))
            .sum::<u64>()
            / observations.len() as u64) as u16
    };
    WindowSummary {
        count: observations.len(),
        mean_milli,
        uncertainty_milli,
        heterogeneity_milli,
        quality_milli,
    }
}

fn digest_input(output: &EvidenceTemporalShift) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "snapshot_id": output.snapshot_id,
        "candidate_order": output.candidate_order,
        "action_order": output.action_order,
        "actions": output.actions,
        "stable_order": output.stable_order,
        "unresolved_order": output.unresolved_order,
        "contradictory_order": output.contradictory_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
        "next_step": output.next_step,
    })
}

impl EvidenceTemporalShift {
    pub fn validate(&self) -> Result<(), EvidenceTemporalShiftError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.snapshot_id.trim().is_empty()
            || !canonical(&self.candidate_order)
            || !canonical(&self.stable_order)
            || !canonical(&self.unresolved_order)
            || !canonical(&self.contradictory_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.actions.len() > MAX_GROUPS
            || self.actions.iter().any(|action| {
                action.action_id != format!("temporal:{}", action.evidence_id)
                    || action.priority_milli > 1_000
                    || action.heterogeneity_milli > 1_000
                    || action.baseline_count == 0
                    || action.recent_count == 0
                    || action.rationale.trim().is_empty()
            })
            || self.actions.windows(2).any(|pair| {
                pair[0].priority_milli < pair[1].priority_milli
                    || (pair[0].priority_milli == pair[1].priority_milli
                        && pair[0].action_id > pair[1].action_id)
            })
        {
            return Err(EvidenceTemporalShiftError::InvalidOutput(
                "identity, ordering, bounds, or action state is invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| EvidenceTemporalShiftError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(EvidenceTemporalShiftError::InvalidOutput(
                "temporal-shift digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &EvidenceTemporalShiftRequest,
) -> Result<(), EvidenceTemporalShiftError> {
    if request.objective.trim().is_empty()
        || request.snapshot_id.trim().is_empty()
        || request.baseline_window_ticks == 0
        || request.recent_window_ticks == 0
        || request.min_observations_per_window == 0
        || request.min_observations_per_window > MAX_OBSERVATIONS
        || request.min_change_milli > 2_000
        || request.min_priority_milli > 1_000
        || request.max_actions == 0
        || request.max_actions > MAX_GROUPS
        || request.observations.is_empty()
        || request.observations.len() > MAX_OBSERVATIONS
    {
        return Err(EvidenceTemporalShiftError::InvalidRequest(
            "objective, snapshot, windows, observation floor, change threshold, and bounded action capacity are required".into(),
        ));
    }
    Ok(())
}

fn action_priority(delta_milli: u64, recent: WindowSummary, heterogeneity_milli: u16) -> u16 {
    let signal = delta_milli.min(2_000);
    let quality = u64::from(recent.quality_milli);
    let replication = u64::from(recent.count.min(u16::MAX as usize) as u16);
    let stability = 1_000_u64.saturating_sub(u64::from(heterogeneity_milli));
    (signal
        .saturating_mul(quality)
        .saturating_mul(replication.max(1))
        .saturating_mul(stability)
        / 2_000_000)
        .min(1_000) as u16
}

pub fn detect_glioma_evidence_temporal_shifts(
    request: &EvidenceTemporalShiftRequest,
) -> Result<EvidenceTemporalShift, EvidenceTemporalShiftError> {
    validate_request(request)?;
    let mut observation_keys = BTreeSet::new();
    let mut groups = BTreeMap::<String, Vec<&EvidenceTemporalObservation>>::new();
    for observation in &request.observations {
        observation
            .artifact
            .validate()
            .map_err(|error| EvidenceTemporalShiftError::InvalidObservation(error.to_string()))?;
        if observation.evidence_id.trim().is_empty()
            || observation.study_id.trim().is_empty()
            || observation.signal_milli.abs() > 1_000
            || observation.uncertainty_milli == 0
            || observation.quality_milli > 1_000
            || observation.replicate_count == 0
            || !observation.preclinical_only
            || observation.artifact.contains_human_data
            || observation.artifact.contains_direct_identifiers
            || observation.release_tick > request.as_of_tick
            || !observation_keys.insert((
                observation.evidence_id.clone(),
                observation.study_id.clone(),
                observation.release_tick,
            ))
        {
            return Err(EvidenceTemporalShiftError::InvalidObservation(
                "identity, temporal bounds, score/uncertainty, privacy, preclinical, or uniqueness declaration is invalid".into(),
            ));
        }
        groups
            .entry(observation.evidence_id.clone())
            .or_default()
            .push(observation);
    }
    if groups.len() > MAX_GROUPS {
        return Err(EvidenceTemporalShiftError::InvalidObservation(
            "evidence group count exceeds the supported bound".into(),
        ));
    }
    let recent_start = request
        .as_of_tick
        .saturating_sub(request.recent_window_ticks);
    let baseline_end = recent_start;
    let baseline_start = baseline_end.saturating_sub(request.baseline_window_ticks);
    let mut candidate_order = Vec::new();
    let mut stable_order = Vec::new();
    let mut unresolved_order = Vec::new();
    let mut contradictory_order = Vec::new();
    let mut actions = Vec::new();
    let mut negative_evidence = Vec::new();
    let mut uncertainty = Vec::new();
    for (evidence_id, observations) in groups {
        let mut baseline = observations
            .iter()
            .copied()
            .filter(|observation| {
                observation.release_tick >= baseline_start
                    && observation.release_tick < baseline_end
            })
            .collect::<Vec<_>>();
        let mut recent = observations
            .iter()
            .copied()
            .filter(|observation| {
                observation.release_tick >= recent_start
                    && observation.release_tick <= request.as_of_tick
            })
            .collect::<Vec<_>>();
        baseline
            .sort_by_key(|observation| (observation.release_tick, observation.study_id.clone()));
        recent.sort_by_key(|observation| (observation.release_tick, observation.study_id.clone()));
        if baseline.len() < request.min_observations_per_window
            || recent.len() < request.min_observations_per_window
        {
            unresolved_order.push(evidence_id.clone());
            uncertainty.push(format!(
                "{evidence_id}: baseline or recent window is under-observed"
            ));
            continue;
        }
        let baseline_summary = summarize(&baseline);
        let recent_summary = summarize(&recent);
        let delta = recent_summary
            .mean_milli
            .saturating_sub(baseline_summary.mean_milli);
        let delta_abs = u64::from(delta.unsigned_abs());
        let heterogeneity_milli = baseline_summary
            .heterogeneity_milli
            .max(recent_summary.heterogeneity_milli);
        let opposite_sign = baseline_summary.mean_milli != 0
            && recent_summary.mean_milli != 0
            && baseline_summary.mean_milli.signum() != recent_summary.mean_milli.signum();
        let kind = if opposite_sign && delta_abs >= request.min_change_milli {
            EvidenceTemporalShiftKind::Reversal
        } else if delta_abs < request.min_change_milli {
            EvidenceTemporalShiftKind::Stable
        } else if heterogeneity_milli >= 900 {
            EvidenceTemporalShiftKind::Contradictory
        } else if delta.is_positive() {
            EvidenceTemporalShiftKind::EmergentPositive
        } else {
            EvidenceTemporalShiftKind::EmergentNegative
        };
        candidate_order.push(evidence_id.clone());
        let priority = action_priority(delta_abs, recent_summary, heterogeneity_milli);
        match kind {
            EvidenceTemporalShiftKind::Stable => {
                stable_order.push(evidence_id.clone());
                negative_evidence.push(format!("{evidence_id}: no material temporal shift"));
            }
            EvidenceTemporalShiftKind::Contradictory => {
                contradictory_order.push(evidence_id.clone());
                uncertainty.push(format!(
                    "{evidence_id}: temporal observations are heterogeneous"
                ));
            }
            EvidenceTemporalShiftKind::EmergentPositive
            | EvidenceTemporalShiftKind::EmergentNegative
            | EvidenceTemporalShiftKind::Reversal
                if priority >= request.min_priority_milli
                    && actions.len() < request.max_actions =>
            {
                let rationale = match kind {
                    EvidenceTemporalShiftKind::Reversal => {
                        "recent evidence reverses the weighted baseline direction; re-review competing explanations before autonomous replanning"
                    }
                    EvidenceTemporalShiftKind::EmergentPositive => {
                        "weighted recent evidence rises beyond the prospective change gate; refresh the typed claim and test mechanism implications"
                    }
                    _ => {
                        "weighted recent evidence falls beyond the prospective change gate; preserve negative evidence and revalidate the claim"
                    }
                };
                actions.push(EvidenceTemporalShiftAction {
                    action_id: format!("temporal:{evidence_id}"),
                    evidence_id: evidence_id.clone(),
                    kind,
                    baseline_mean_milli: baseline_summary.mean_milli,
                    recent_mean_milli: recent_summary.mean_milli,
                    delta_milli: delta,
                    baseline_uncertainty_milli: baseline_summary.uncertainty_milli,
                    recent_uncertainty_milli: recent_summary.uncertainty_milli,
                    heterogeneity_milli,
                    baseline_count: baseline_summary.count,
                    recent_count: recent_summary.count,
                    priority_milli: priority,
                    rationale: rationale.into(),
                });
            }
            EvidenceTemporalShiftKind::EmergentPositive
            | EvidenceTemporalShiftKind::EmergentNegative
            | EvidenceTemporalShiftKind::Reversal => {
                negative_evidence.push(format!(
                    "{evidence_id}: material shift did not clear the action priority gate"
                ));
            }
            EvidenceTemporalShiftKind::Insufficient => unreachable!(),
        }
        if baseline_summary.uncertainty_milli > request.min_change_milli
            || recent_summary.uncertainty_milli > request.min_change_milli
        {
            uncertainty.push(format!(
                "{evidence_id}: weighted window uncertainty exceeds the change scale"
            ));
        }
    }
    candidate_order.sort();
    stable_order.sort();
    unresolved_order.sort();
    contradictory_order.sort();
    negative_evidence.sort();
    negative_evidence.dedup();
    uncertainty.sort();
    uncertainty.dedup();
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
    let disposition = if candidate_order.is_empty() {
        EvidenceTemporalShiftDisposition::Blocked
    } else if !actions.is_empty() {
        EvidenceTemporalShiftDisposition::Ready
    } else if candidate_order.len() == stable_order.len() {
        EvidenceTemporalShiftDisposition::NoMaterialShift
    } else {
        EvidenceTemporalShiftDisposition::Partial
    };
    let next_step = match disposition {
        EvidenceTemporalShiftDisposition::Ready => {
            "route temporal actions to evidence refresh, mechanism review, or bounded experiment replanning"
        }
        EvidenceTemporalShiftDisposition::Partial => {
            "acquire the missing windows or resolve contradictions before autonomous replanning"
        }
        EvidenceTemporalShiftDisposition::NoMaterialShift => {
            "retain the current claims and continue prospective surveillance"
        }
        EvidenceTemporalShiftDisposition::Blocked => {
            "do not infer a trend; acquire independent preclinical observations for both windows"
        }
    };
    let mut output = EvidenceTemporalShift {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        snapshot_id: request.snapshot_id.clone(),
        candidate_order,
        action_order,
        actions,
        stable_order,
        unresolved_order,
        contradictory_order,
        negative_evidence,
        uncertainty,
        disposition,
        next_step: next_step.into(),
        digest: ContentHash::of_value(&serde_json::Value::Null)
            .map_err(|error| EvidenceTemporalShiftError::Digest(error.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| EvidenceTemporalShiftError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma_engine::LocalArtifactRef;

    fn observation(id: &str, tick: u64, signal: i32, quality: u16) -> EvidenceTemporalObservation {
        EvidenceTemporalObservation {
            evidence_id: id.into(),
            study_id: format!("study-{id}-{tick}"),
            release_tick: tick,
            signal_milli: signal,
            uncertainty_milli: 20,
            quality_milli: quality,
            replicate_count: 4,
            modality: GliomaModality::Transcriptomics,
            model_system: Some(GliomaModelSystem::Organoid),
            artifact: LocalArtifactRef {
                artifact_id: format!("artifact-{id}-{tick}"),
                content_hash: ContentHash::of_value(&serde_json::json!({"id": id, "tick": tick}))
                    .unwrap(),
                content_type: "aggregate-evidence".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
            preclinical_only: true,
        }
    }

    fn request(observations: Vec<EvidenceTemporalObservation>) -> EvidenceTemporalShiftRequest {
        EvidenceTemporalShiftRequest {
            objective: "detect prospective glioma evidence shifts".into(),
            snapshot_id: "snapshot-1".into(),
            as_of_tick: 20,
            baseline_window_ticks: 10,
            recent_window_ticks: 5,
            min_observations_per_window: 2,
            min_change_milli: 100,
            min_priority_milli: 50,
            max_actions: 8,
            observations,
        }
    }

    #[test]
    fn detects_a_weighted_reversal_as_a_research_action() {
        let output = detect_glioma_evidence_temporal_shifts(&request(vec![
            observation("egfr", 6, 500, 900),
            observation("egfr", 8, 450, 900),
            observation("egfr", 16, -400, 900),
            observation("egfr", 18, -450, 900),
        ]))
        .expect("temporal shift");
        assert_eq!(output.disposition, EvidenceTemporalShiftDisposition::Ready);
        assert_eq!(output.actions[0].kind, EvidenceTemporalShiftKind::Reversal);
        assert!(output.actions[0].delta_milli < 0);
        output.validate().expect("digest validates");
    }

    #[test]
    fn preserves_under_observed_windows_as_uncertainty() {
        let output = detect_glioma_evidence_temporal_shifts(&request(vec![
            observation("pdgfra", 3, 300, 900),
            observation("pdgfra", 17, 320, 900),
        ]))
        .expect("temporal shift");
        assert_eq!(
            output.disposition,
            EvidenceTemporalShiftDisposition::Blocked
        );
        assert_eq!(output.unresolved_order, vec!["pdgfra"]);
        assert!(output.actions.is_empty());
        assert!(!output.uncertainty.is_empty());
    }
}
