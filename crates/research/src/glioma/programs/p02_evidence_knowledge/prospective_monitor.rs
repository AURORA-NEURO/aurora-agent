//! Prospective high-throughput monitoring of typed glioma knowledge.
//!
//! This feature turns an ordered stream of already-typed study observations into a deterministic
//! sequential-monitoring surface. It estimates a claim-specific baseline, computes a bounded
//! cumulative drift signal, detects change points, preserves negative/contradictory/unresolved
//! states, and applies a multiplicity adjustment across claims. It does not infer equivalence,
//! causality, treatment effect, or clinical meaning; callers must provide canonical claim keys
//! and typed dispositions before the monitor can run.

use super::knowledge_graph::KnowledgeClaimDisposition;
use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P02-F07";
pub const OUTPUT_SCHEMA: &str = "GliomaProspectiveKnowledgeMonitor1@1";
pub const MAX_EVENTS: usize = 200_000;
pub const MAX_CLAIMS: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProspectiveKnowledgeRequest {
    pub objective: String,
    pub events: Vec<ProspectiveKnowledgeEvent>,
    pub baseline_window: usize,
    pub min_events: usize,
    pub drift_threshold_milli: u16,
    pub alert_persistence: usize,
    pub required_modalities: BTreeSet<GliomaModality>,
    pub required_model_systems: BTreeSet<GliomaModelSystem>,
    pub max_claims: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProspectiveKnowledgeEvent {
    pub event_id: String,
    pub study_id: String,
    pub site_id: String,
    pub canonical_claim_key: String,
    pub release_epoch: u32,
    pub sequence: u64,
    pub support_milli: u16,
    pub confidence_milli: u16,
    pub contradiction_milli: u16,
    pub equivalence_milli: u16,
    pub semantic_loss_milli: u16,
    pub modality_order: Vec<GliomaModality>,
    pub model_system_order: Vec<GliomaModelSystem>,
    pub disposition: KnowledgeClaimDisposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProspectiveKnowledgeTrend {
    Strengthening,
    Weakening,
    Stable,
    Contradictory,
    Unresolved,
    Insufficient,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProspectiveKnowledgeAlert {
    None,
    Review,
    Acquire,
    Quarantine,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProspectiveKnowledgeRow {
    pub canonical_claim_key: String,
    pub event_order: Vec<String>,
    pub study_order: Vec<String>,
    pub baseline_event_count: usize,
    pub event_count: usize,
    pub baseline_support_milli: u16,
    pub latest_support_milli: u16,
    pub support_delta_milli: i32,
    pub baseline_confidence_milli: u16,
    pub latest_confidence_milli: u16,
    pub max_abs_cusum_milli: u16,
    pub max_alert_run: usize,
    pub change_point_sequence: Option<u64>,
    pub multiplicity_adjusted_signal_milli: u16,
    pub contradiction_count: usize,
    pub negative_count: usize,
    pub unresolved_count: usize,
    pub modality_order: Vec<GliomaModality>,
    pub model_system_order: Vec<GliomaModelSystem>,
    pub missing_modality_order: Vec<GliomaModality>,
    pub missing_model_system_order: Vec<GliomaModelSystem>,
    pub semantic_loss_milli: u16,
    pub trend: ProspectiveKnowledgeTrend,
    pub alert: ProspectiveKnowledgeAlert,
    pub omission_order: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProspectiveKnowledgeDisposition {
    Qualified,
    Partial,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProspectiveKnowledgeMonitor {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub claim_order: Vec<String>,
    pub review_claim_order: Vec<String>,
    pub acquire_claim_order: Vec<String>,
    pub quarantine_claim_order: Vec<String>,
    pub strengthening_claim_order: Vec<String>,
    pub weakening_claim_order: Vec<String>,
    pub stable_claim_order: Vec<String>,
    pub rows: Vec<ProspectiveKnowledgeRow>,
    pub omission_order: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: ProspectiveKnowledgeDisposition,
    pub next_step: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProspectiveKnowledgeError {
    #[error("prospective knowledge request is invalid: {0}")]
    InvalidRequest(String),
    #[error("prospective knowledge output is invalid: {0}")]
    InvalidOutput(String),
    #[error("prospective knowledge digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &ProspectiveKnowledgeMonitor) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "claim_order": output.claim_order,
        "review_claim_order": output.review_claim_order,
        "acquire_claim_order": output.acquire_claim_order,
        "quarantine_claim_order": output.quarantine_claim_order,
        "strengthening_claim_order": output.strengthening_claim_order,
        "weakening_claim_order": output.weakening_claim_order,
        "stable_claim_order": output.stable_claim_order,
        "rows": output.rows,
        "omission_order": output.omission_order,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
        "next_step": output.next_step,
    })
}

impl ProspectiveKnowledgeMonitor {
    pub fn validate(&self) -> Result<(), ProspectiveKnowledgeError> {
        let row_keys = self
            .rows
            .iter()
            .map(|row| row.canonical_claim_key.clone())
            .collect::<Vec<_>>();
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !canonical(&self.claim_order)
            || row_keys != self.claim_order
            || !canonical(&self.review_claim_order)
            || !canonical(&self.acquire_claim_order)
            || !canonical(&self.quarantine_claim_order)
            || !canonical(&self.strengthening_claim_order)
            || !canonical(&self.weakening_claim_order)
            || !canonical(&self.stable_claim_order)
            || !canonical(&self.omission_order)
            || !canonical(&self.uncertainty)
            || self.rows.iter().any(|row| {
                row.canonical_claim_key.trim().is_empty()
                    || !canonical(&row.event_order)
                    || !canonical(&row.study_order)
                    || !canonical(&row.modality_order)
                    || !canonical(&row.model_system_order)
                    || !canonical(&row.missing_modality_order)
                    || !canonical(&row.missing_model_system_order)
                    || !canonical(&row.omission_order)
                    || row.baseline_event_count > row.event_count
                    || row.baseline_support_milli > 1_000
                    || row.latest_support_milli > 1_000
                    || row.baseline_confidence_milli > 1_000
                    || row.latest_confidence_milli > 1_000
                    || row.max_abs_cusum_milli > 1_000
                    || row.multiplicity_adjusted_signal_milli > 1_000
                    || row.semantic_loss_milli > 1_000
            })
        {
            return Err(ProspectiveKnowledgeError::InvalidOutput(
                "identity, ordering, row bounds, or score limits are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ProspectiveKnowledgeError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ProspectiveKnowledgeError::InvalidOutput(
                "prospective knowledge digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &ProspectiveKnowledgeRequest,
) -> Result<(), ProspectiveKnowledgeError> {
    if request.objective.trim().is_empty()
        || request.events.is_empty()
        || request.events.len() > MAX_EVENTS
        || request.baseline_window == 0
        || request.min_events == 0
        || request.baseline_window > request.min_events
        || request.drift_threshold_milli == 0
        || request.drift_threshold_milli > 1_000
        || request.alert_persistence == 0
        || request.max_claims == 0
        || request.max_claims > MAX_CLAIMS
    {
        return Err(ProspectiveKnowledgeError::InvalidRequest(
            "objective, event capacity, baseline/minimum windows, drift threshold, persistence, or claim bound is invalid".into(),
        ));
    }
    let mut event_ids = BTreeSet::new();
    let mut claim_keys = BTreeSet::new();
    let mut claim_sequences = BTreeSet::new();
    for event in &request.events {
        if event.event_id.trim().is_empty()
            || event.study_id.trim().is_empty()
            || event.site_id.trim().is_empty()
            || event.canonical_claim_key.trim().is_empty()
            || !event_ids.insert(event.event_id.clone())
            || !claim_sequences.insert((event.canonical_claim_key.clone(), event.sequence))
            || event.support_milli > 1_000
            || event.confidence_milli > 1_000
            || event.contradiction_milli > 1_000
            || event.equivalence_milli > 1_000
            || event.semantic_loss_milli > 1_000
            || !canonical(&event.modality_order)
            || !canonical(&event.model_system_order)
        {
            return Err(ProspectiveKnowledgeError::InvalidRequest(
                "event identity, uniqueness, score bounds, chronology, or canonical coverage is invalid".into(),
            ));
        }
        claim_keys.insert(event.canonical_claim_key.clone());
    }
    if claim_keys.len() > request.max_claims {
        return Err(ProspectiveKnowledgeError::InvalidRequest(
            "event stream exceeds the requested claim capacity".into(),
        ));
    }
    Ok(())
}

fn weighted_mean(values: impl Iterator<Item = (u16, u16)>) -> u16 {
    let mut numerator = 0_u64;
    let mut denominator = 0_u64;
    for (value, weight) in values {
        numerator += u64::from(value) * u64::from(weight.max(1));
        denominator += u64::from(weight.max(1));
    }
    if denominator == 0 {
        0
    } else {
        (numerator / denominator).min(1_000) as u16
    }
}

fn abs_signal(value: i32) -> u16 {
    value.unsigned_abs().min(1_000) as u16
}

fn row_trend(
    events: &[ProspectiveKnowledgeEvent],
    support_delta_milli: i32,
    drift_threshold_milli: u16,
    baseline_ready: bool,
) -> ProspectiveKnowledgeTrend {
    if !baseline_ready {
        return ProspectiveKnowledgeTrend::Insufficient;
    }
    if events
        .iter()
        .any(|event| matches!(event.disposition, KnowledgeClaimDisposition::Unresolved))
    {
        ProspectiveKnowledgeTrend::Unresolved
    } else if events.iter().any(|event| {
        matches!(
            event.disposition,
            KnowledgeClaimDisposition::Negative | KnowledgeClaimDisposition::Contested
        ) || event.contradiction_milli >= drift_threshold_milli
    }) {
        ProspectiveKnowledgeTrend::Contradictory
    } else if support_delta_milli >= i32::from(drift_threshold_milli) {
        ProspectiveKnowledgeTrend::Strengthening
    } else if support_delta_milli <= -i32::from(drift_threshold_milli) {
        ProspectiveKnowledgeTrend::Weakening
    } else {
        ProspectiveKnowledgeTrend::Stable
    }
}

pub fn monitor_prospective_knowledge(
    request: &ProspectiveKnowledgeRequest,
) -> Result<ProspectiveKnowledgeMonitor, ProspectiveKnowledgeError> {
    validate_request(request)?;
    let mut grouped = BTreeMap::<String, Vec<ProspectiveKnowledgeEvent>>::new();
    for event in &request.events {
        grouped
            .entry(event.canonical_claim_key.clone())
            .or_default()
            .push(event.clone());
    }
    for events in grouped.values_mut() {
        events.sort_by(|left, right| {
            (left.release_epoch, left.sequence, &left.event_id).cmp(&(
                right.release_epoch,
                right.sequence,
                &right.event_id,
            ))
        });
    }
    let mut rows = Vec::new();
    for (canonical_claim_key, events) in grouped {
        let baseline_count = request.baseline_window.min(events.len());
        let baseline_ready = events.len() >= request.min_events;
        let baseline = &events[..baseline_count];
        let baseline_support_milli = weighted_mean(
            baseline
                .iter()
                .map(|event| (event.support_milli, event.confidence_milli)),
        );
        let baseline_confidence_milli = weighted_mean(
            baseline
                .iter()
                .map(|event| (event.confidence_milli, event.equivalence_milli)),
        );
        let latest = events.last().expect("validated event stream is non-empty");
        let support_delta_milli =
            i32::from(latest.support_milli) - i32::from(baseline_support_milli);
        let latest_confidence_milli = latest.confidence_milli;
        let mut cusum = 0_i32;
        let mut max_abs_cusum_milli = 0_u16;
        let mut alert_run = 0_usize;
        let mut max_alert_run = 0_usize;
        let mut change_point_sequence = None;
        for event in events.iter().skip(baseline_count) {
            let delta = i32::from(event.support_milli) - i32::from(baseline_support_milli);
            cusum = (cusum + delta).clamp(-2_000, 2_000);
            let signal = abs_signal(cusum);
            max_abs_cusum_milli = max_abs_cusum_milli.max(signal);
            if signal >= request.drift_threshold_milli {
                alert_run += 1;
                max_alert_run = max_alert_run.max(alert_run);
            } else {
                alert_run = 0;
            }
            if change_point_sequence.is_none() && alert_run >= request.alert_persistence {
                change_point_sequence = Some(event.sequence);
            }
        }
        let modality_order = events
            .iter()
            .flat_map(|event| event.modality_order.iter().copied())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let model_system_order = events
            .iter()
            .flat_map(|event| event.model_system_order.iter().copied())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let observed_modalities = modality_order.iter().copied().collect::<BTreeSet<_>>();
        let observed_models = model_system_order.iter().copied().collect::<BTreeSet<_>>();
        let missing_modality_order = request
            .required_modalities
            .difference(&observed_modalities)
            .copied()
            .collect::<Vec<_>>();
        let missing_model_system_order = request
            .required_model_systems
            .difference(&observed_models)
            .copied()
            .collect::<Vec<_>>();
        let contradiction_count = events
            .iter()
            .filter(|event| {
                event.contradiction_milli >= request.drift_threshold_milli
                    || matches!(event.disposition, KnowledgeClaimDisposition::Contested)
            })
            .count();
        let negative_count = events
            .iter()
            .filter(|event| matches!(event.disposition, KnowledgeClaimDisposition::Negative))
            .count();
        let unresolved_count = events
            .iter()
            .filter(|event| matches!(event.disposition, KnowledgeClaimDisposition::Unresolved))
            .count();
        let semantic_loss_milli = events
            .iter()
            .map(|event| event.semantic_loss_milli)
            .max()
            .unwrap_or(0);
        let trend = row_trend(
            &events,
            support_delta_milli,
            request.drift_threshold_milli,
            baseline_ready,
        );
        let alert = if unresolved_count > 0
            || !missing_modality_order.is_empty()
            || !missing_model_system_order.is_empty()
            || !baseline_ready
        {
            ProspectiveKnowledgeAlert::Acquire
        } else if contradiction_count > 0 || negative_count > 0 {
            ProspectiveKnowledgeAlert::Quarantine
        } else if max_abs_cusum_milli >= request.drift_threshold_milli
            && max_alert_run >= request.alert_persistence
        {
            ProspectiveKnowledgeAlert::Review
        } else {
            ProspectiveKnowledgeAlert::None
        };
        let mut omission_order = Vec::new();
        if !baseline_ready {
            omission_order.push(format!(
                "{canonical_claim_key}: minimum event window is unmet"
            ));
        }
        if !missing_modality_order.is_empty() {
            omission_order.extend(
                missing_modality_order.iter().map(|modality| {
                    format!("{canonical_claim_key}: missing modality {modality:?}")
                }),
            );
        }
        if !missing_model_system_order.is_empty() {
            omission_order.extend(
                missing_model_system_order
                    .iter()
                    .map(|model| format!("{canonical_claim_key}: missing model {model:?}")),
            );
        }
        if semantic_loss_milli > 0 {
            omission_order.push(format!(
                "{canonical_claim_key}: semantic-loss declaration is {semantic_loss_milli} milli"
            ));
        }
        if change_point_sequence.is_some() {
            omission_order.push(format!(
                "{canonical_claim_key}: sequential drift change point detected"
            ));
        }
        omission_order.sort();
        omission_order.dedup();
        rows.push(ProspectiveKnowledgeRow {
            canonical_claim_key,
            event_order: events.iter().map(|event| event.event_id.clone()).collect(),
            study_order: events
                .iter()
                .map(|event| event.study_id.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
            baseline_event_count: baseline_count,
            event_count: events.len(),
            baseline_support_milli,
            latest_support_milli: latest.support_milli,
            support_delta_milli,
            baseline_confidence_milli,
            latest_confidence_milli,
            max_abs_cusum_milli,
            max_alert_run,
            change_point_sequence,
            multiplicity_adjusted_signal_milli: 0,
            contradiction_count,
            negative_count,
            unresolved_count,
            modality_order,
            model_system_order,
            missing_modality_order,
            missing_model_system_order,
            semantic_loss_milli,
            trend,
            alert,
            omission_order,
        });
    }
    rows.sort_by(|left, right| left.canonical_claim_key.cmp(&right.canonical_claim_key));
    let claim_count = rows.len();
    let mut ranked = rows
        .iter()
        .enumerate()
        .map(|(index, row)| (index, row.max_abs_cusum_milli))
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    for (rank, (index, signal)) in ranked.into_iter().enumerate() {
        rows[index].multiplicity_adjusted_signal_milli =
            ((u32::from(signal) * u32::try_from(claim_count - rank).unwrap_or(0))
                / u32::try_from(claim_count.max(1)).unwrap_or(1)) as u16;
    }
    let mut claim_order = rows
        .iter()
        .map(|row| row.canonical_claim_key.clone())
        .collect::<Vec<_>>();
    claim_order.sort();
    let mut review_claim_order = Vec::new();
    let mut acquire_claim_order = Vec::new();
    let mut quarantine_claim_order = Vec::new();
    let mut strengthening_claim_order = Vec::new();
    let mut weakening_claim_order = Vec::new();
    let mut stable_claim_order = Vec::new();
    let mut omission_order = Vec::new();
    let mut uncertainty = Vec::new();
    for row in &rows {
        match row.alert {
            ProspectiveKnowledgeAlert::Review => {
                review_claim_order.push(row.canonical_claim_key.clone())
            }
            ProspectiveKnowledgeAlert::Acquire => {
                acquire_claim_order.push(row.canonical_claim_key.clone())
            }
            ProspectiveKnowledgeAlert::Quarantine => {
                quarantine_claim_order.push(row.canonical_claim_key.clone())
            }
            ProspectiveKnowledgeAlert::None => {}
        }
        match row.trend {
            ProspectiveKnowledgeTrend::Strengthening => {
                strengthening_claim_order.push(row.canonical_claim_key.clone())
            }
            ProspectiveKnowledgeTrend::Weakening => {
                weakening_claim_order.push(row.canonical_claim_key.clone())
            }
            ProspectiveKnowledgeTrend::Stable => {
                stable_claim_order.push(row.canonical_claim_key.clone())
            }
            ProspectiveKnowledgeTrend::Contradictory
            | ProspectiveKnowledgeTrend::Unresolved
            | ProspectiveKnowledgeTrend::Insufficient => {}
        }
        omission_order.extend(row.omission_order.iter().cloned());
        if row.semantic_loss_milli > 0 {
            uncertainty.push(format!(
                "{}: non-zero semantic loss is carried through the prospective stream",
                row.canonical_claim_key
            ));
        }
    }
    omission_order.sort();
    omission_order.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    review_claim_order.sort();
    acquire_claim_order.sort();
    quarantine_claim_order.sort();
    strengthening_claim_order.sort();
    weakening_claim_order.sort();
    stable_claim_order.sort();
    let disposition = if rows.is_empty() {
        ProspectiveKnowledgeDisposition::Blocked
    } else if !quarantine_claim_order.is_empty()
        || rows
            .iter()
            .any(|row| row.trend == ProspectiveKnowledgeTrend::Unresolved)
    {
        ProspectiveKnowledgeDisposition::Unresolved
    } else if review_claim_order.is_empty() && acquire_claim_order.is_empty() {
        ProspectiveKnowledgeDisposition::Qualified
    } else {
        ProspectiveKnowledgeDisposition::Partial
    };
    let next_step = match disposition {
        ProspectiveKnowledgeDisposition::Qualified => {
            "continue bounded prospective monitoring and release stable rows to downstream planning"
        }
        ProspectiveKnowledgeDisposition::Partial => {
            "review sequential drift alerts and acquire missing coverage before promotion"
        }
        ProspectiveKnowledgeDisposition::Unresolved => {
            "quarantine contradictory or unresolved rows and route them to evidence adjudication"
        }
        ProspectiveKnowledgeDisposition::Blocked => {
            "provide at least one typed prospective claim event"
        }
    };
    let mut output = ProspectiveKnowledgeMonitor {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        claim_order,
        review_claim_order,
        acquire_claim_order,
        quarantine_claim_order,
        strengthening_claim_order,
        weakening_claim_order,
        stable_claim_order,
        rows,
        omission_order,
        uncertainty,
        disposition,
        next_step: next_step.into(),
        digest: ContentHash::of_value(&serde_json::Value::Null)
            .map_err(|error| ProspectiveKnowledgeError::Digest(error.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ProspectiveKnowledgeError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn event(
        key: &str,
        sequence: u64,
        support_milli: u16,
        disposition: KnowledgeClaimDisposition,
    ) -> ProspectiveKnowledgeEvent {
        ProspectiveKnowledgeEvent {
            event_id: format!("{key}-{sequence}"),
            study_id: format!("study-{sequence}"),
            site_id: format!("site-{sequence}"),
            canonical_claim_key: key.into(),
            release_epoch: sequence as u32,
            sequence,
            support_milli,
            confidence_milli: 900,
            contradiction_milli: if matches!(disposition, KnowledgeClaimDisposition::Contested) {
                900
            } else {
                0
            },
            equivalence_milli: 950,
            semantic_loss_milli: 0,
            modality_order: vec![GliomaModality::Transcriptomics],
            model_system_order: vec![GliomaModelSystem::Organoid],
            disposition,
        }
    }

    fn request(events: Vec<ProspectiveKnowledgeEvent>) -> ProspectiveKnowledgeRequest {
        ProspectiveKnowledgeRequest {
            objective: "prospective glioma knowledge".into(),
            events,
            baseline_window: 2,
            min_events: 4,
            drift_threshold_milli: 150,
            alert_persistence: 2,
            required_modalities: BTreeSet::from([GliomaModality::Transcriptomics]),
            required_model_systems: BTreeSet::from([GliomaModelSystem::Organoid]),
            max_claims: 8,
        }
    }

    #[test]
    fn stable_stream_is_qualified() {
        let output = monitor_prospective_knowledge(&request(
            (1..=4)
                .map(|sequence| {
                    event(
                        "egfr-invasion",
                        sequence,
                        800,
                        KnowledgeClaimDisposition::Supported,
                    )
                })
                .collect(),
        ))
        .expect("monitor");
        assert_eq!(
            output.disposition,
            ProspectiveKnowledgeDisposition::Qualified
        );
        assert_eq!(output.stable_claim_order, vec!["egfr-invasion"]);
        assert_eq!(output.rows[0].max_abs_cusum_milli, 0);
        assert_eq!(output.rows[0].max_alert_run, 0);
        output.validate().expect("digest validates");
    }

    #[test]
    fn persistent_weakening_emits_review() {
        let output = monitor_prospective_knowledge(&request(vec![
            event(
                "egfr-invasion",
                1,
                900,
                KnowledgeClaimDisposition::Supported,
            ),
            event(
                "egfr-invasion",
                2,
                900,
                KnowledgeClaimDisposition::Supported,
            ),
            event(
                "egfr-invasion",
                3,
                500,
                KnowledgeClaimDisposition::Supported,
            ),
            event(
                "egfr-invasion",
                4,
                500,
                KnowledgeClaimDisposition::Supported,
            ),
        ]))
        .expect("monitor");
        assert_eq!(output.disposition, ProspectiveKnowledgeDisposition::Partial);
        assert_eq!(output.review_claim_order, vec!["egfr-invasion"]);
        assert_eq!(output.rows[0].trend, ProspectiveKnowledgeTrend::Weakening);
        assert_eq!(output.rows[0].change_point_sequence, Some(4));
    }

    #[test]
    fn insufficient_stream_requests_acquisition() {
        let output = monitor_prospective_knowledge(&request(vec![
            event(
                "egfr-invasion",
                1,
                900,
                KnowledgeClaimDisposition::Supported,
            ),
            event(
                "egfr-invasion",
                2,
                900,
                KnowledgeClaimDisposition::Supported,
            ),
        ]))
        .expect("monitor");
        assert_eq!(output.disposition, ProspectiveKnowledgeDisposition::Partial);
        assert_eq!(output.acquire_claim_order, vec!["egfr-invasion"]);
        assert!(!output.omission_order.is_empty());
    }
}
