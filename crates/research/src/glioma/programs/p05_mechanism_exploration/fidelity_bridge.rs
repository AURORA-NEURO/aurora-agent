//! Cross-model fidelity bridging for preclinical glioma mechanism research.
//!
//! A mechanism that fits one model system can fail when transported to another.  This feature
//! compares typed predictions with local observations across model systems, scores residual
//! transportability with integer arithmetic, and exposes mechanisms that need more discriminating
//! evidence.  It is an analysis product only: it never treats transportability as causality.

use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P05-F06";
pub const OUTPUT_SCHEMA: &str = "GliomaMechanismFidelityBridge1@1";
pub const MAX_OBSERVATIONS: usize = 16_384;
pub const MAX_MECHANISMS: usize = 512;
pub const MAX_ABS_VALUE_MILLI: i64 = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismFidelityObservation {
    pub observation_id: String,
    pub mechanism_id: String,
    pub model_system: GliomaModelSystem,
    pub predicted_milli: i64,
    pub observed_milli: i64,
    pub uncertainty_milli: u32,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismFidelityBridgeRequest {
    pub objective: String,
    pub target_model_system: GliomaModelSystem,
    pub observations: Vec<MechanismFidelityObservation>,
    pub min_model_systems: usize,
    pub min_transport_milli: u16,
    pub max_observations: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismFidelityResidual {
    pub model_system: GliomaModelSystem,
    pub predicted_milli: i64,
    pub observed_milli: i64,
    pub residual_milli: i64,
    pub compatibility_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismFidelityRecord {
    pub mechanism_id: String,
    pub model_system_order: Vec<GliomaModelSystem>,
    pub residuals: Vec<MechanismFidelityResidual>,
    pub target_observed: bool,
    pub transport_milli: u16,
    pub coverage_milli: u16,
    pub confidence_milli: u16,
    pub negative_evidence_order: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismFidelityBridgeDisposition {
    Ready,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismFidelityBridge {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub target_model_system: GliomaModelSystem,
    pub mechanism_order: Vec<String>,
    pub records: Vec<MechanismFidelityRecord>,
    pub frontier_order: Vec<String>,
    pub missing_target_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: MechanismFidelityBridgeDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MechanismFidelityBridgeError {
    #[error("mechanism fidelity bridge request is invalid: {0}")]
    InvalidRequest(String),
    #[error("mechanism fidelity bridge observation is invalid: {0}")]
    InvalidObservation(String),
    #[error("mechanism fidelity bridge output is invalid: {0}")]
    InvalidOutput(String),
    #[error("mechanism fidelity bridge digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_nonempty(values: &[String]) -> bool {
    values.iter().all(|value| !value.trim().is_empty())
        && values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

fn digest_input(output: &MechanismFidelityBridge) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "target_model_system": output.target_model_system,
        "mechanism_order": output.mechanism_order,
        "records": output.records,
        "frontier_order": output.frontier_order,
        "missing_target_order": output.missing_target_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn compatibility(residual: i64, uncertainty: u32) -> u16 {
    let scale = u64::from(uncertainty).saturating_add(500);
    let penalty = residual.unsigned_abs().saturating_mul(1_000) / scale;
    (1_000_u64.saturating_sub(penalty.min(1_000))) as u16
}

impl MechanismFidelityBridge {
    pub fn validate(&self) -> Result<(), MechanismFidelityBridgeError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.mechanism_order.is_empty()
            || !canonical(&self.mechanism_order)
            || self.records.len() != self.mechanism_order.len()
            || self
                .records
                .windows(2)
                .any(|pair| pair[0].mechanism_id >= pair[1].mechanism_id)
            || !unique_nonempty(&self.frontier_order)
            || !canonical(&self.missing_target_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.records.iter().any(|record| {
                record.mechanism_id.trim().is_empty()
                    || !canonical(&record.model_system_order)
                    || record.model_system_order.is_empty()
                    || record.residuals.len() != record.model_system_order.len()
                    || record
                        .residuals
                        .windows(2)
                        .any(|pair| pair[0].model_system >= pair[1].model_system)
                    || record.transport_milli > 1_000
                    || record.coverage_milli > 1_000
                    || record.confidence_milli > 1_000
                    || !canonical(&record.negative_evidence_order)
            })
            || self.digest.as_str().len() != 64
        {
            return Err(MechanismFidelityBridgeError::InvalidOutput(
                "identity, canonical ordering, coverage, residual, or digest shape is invalid"
                    .into(),
            ));
        }
        let ids = self
            .records
            .iter()
            .map(|record| record.mechanism_id.clone())
            .collect::<BTreeSet<_>>();
        if ids
            != self
                .mechanism_order
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>()
            || self.frontier_order.iter().any(|id| !ids.contains(id))
            || self.missing_target_order.iter().any(|id| !ids.contains(id))
        {
            return Err(MechanismFidelityBridgeError::InvalidOutput(
                "mechanism partitions do not reconcile with records".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| MechanismFidelityBridgeError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(MechanismFidelityBridgeError::Digest(
                "mechanism fidelity bridge digest does not match canonical content".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &MechanismFidelityBridgeRequest,
) -> Result<(), MechanismFidelityBridgeError> {
    if request.objective.trim().is_empty()
        || request.observations.is_empty()
        || request.observations.len() > MAX_OBSERVATIONS
        || request.max_observations == 0
        || request.max_observations > MAX_OBSERVATIONS
        || request.observations.len() > request.max_observations
        || request.min_model_systems == 0
        || request.min_model_systems > 6
        || request.min_transport_milli > 1_000
    {
        return Err(MechanismFidelityBridgeError::InvalidRequest(
            "objective, bounded observations, positive model-system coverage, and transport threshold are required".into(),
        ));
    }
    let mut observation_ids = BTreeSet::new();
    let mut mechanism_model_pairs = BTreeSet::new();
    for observation in &request.observations {
        if observation.observation_id.trim().is_empty()
            || observation.mechanism_id.trim().is_empty()
            || observation.uncertainty_milli == 0
            || observation.predicted_milli.unsigned_abs() > MAX_ABS_VALUE_MILLI as u64
            || observation.observed_milli.unsigned_abs() > MAX_ABS_VALUE_MILLI as u64
            || !observation_ids.insert(observation.observation_id.clone())
            || !mechanism_model_pairs
                .insert((observation.mechanism_id.clone(), observation.model_system))
            || observation.artifact.validate().is_err()
        {
            return Err(MechanismFidelityBridgeError::InvalidObservation(
                "observation ids and mechanism/model pairs must be unique with bounded values, uncertainty, and valid local artifacts".into(),
            ));
        }
    }
    let mechanisms = request
        .observations
        .iter()
        .map(|observation| observation.mechanism_id.as_str())
        .collect::<BTreeSet<_>>();
    if mechanisms.len() > MAX_MECHANISMS {
        return Err(MechanismFidelityBridgeError::InvalidRequest(
            "mechanism bound exceeded".into(),
        ));
    }
    Ok(())
}

/// Compare mechanism fit across model systems and rank the next fidelity-bridging work.
pub fn bridge_glioma_mechanism_fidelity(
    request: &MechanismFidelityBridgeRequest,
) -> Result<MechanismFidelityBridge, MechanismFidelityBridgeError> {
    validate_request(request)?;
    let mut grouped = BTreeMap::<String, Vec<&MechanismFidelityObservation>>::new();
    for observation in &request.observations {
        grouped
            .entry(observation.mechanism_id.clone())
            .or_default()
            .push(observation);
    }
    let mechanism_order = grouped.keys().cloned().collect::<Vec<_>>();
    let mut records = Vec::new();
    let mut missing_target = BTreeSet::new();
    let mut negative_evidence = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for (mechanism_id, observations) in grouped {
        let mut ordered = observations;
        ordered.sort_by(|left, right| left.model_system.cmp(&right.model_system));
        let model_system_order = ordered
            .iter()
            .map(|observation| observation.model_system)
            .collect::<Vec<_>>();
        let residuals = ordered
            .iter()
            .map(|observation| {
                let residual = observation.observed_milli - observation.predicted_milli;
                MechanismFidelityResidual {
                    model_system: observation.model_system,
                    predicted_milli: observation.predicted_milli,
                    observed_milli: observation.observed_milli,
                    residual_milli: residual,
                    compatibility_milli: compatibility(residual, observation.uncertainty_milli),
                }
            })
            .collect::<Vec<_>>();
        let transport = if residuals.is_empty() {
            0
        } else {
            (residuals
                .iter()
                .map(|residual| u32::from(residual.compatibility_milli))
                .sum::<u32>()
                / residuals.len() as u32) as u16
        };
        let coverage = ((model_system_order.len().min(6) * 1_000) / 6) as u16;
        let target_observed = model_system_order.contains(&request.target_model_system);
        if !target_observed {
            missing_target.insert(mechanism_id.clone());
            uncertainty.insert(format!(
                "mechanism:{mechanism_id}:target-model-system-missing"
            ));
        }
        if model_system_order.len() < request.min_model_systems {
            uncertainty.insert(format!(
                "mechanism:{mechanism_id}:insufficient-model-system-coverage"
            ));
        }
        let mut negative = Vec::new();
        if transport < request.min_transport_milli {
            let evidence = format!("mechanism:{mechanism_id}:low-fidelity-transport");
            negative_evidence.insert(evidence.clone());
            negative.push(evidence);
        }
        records.push(MechanismFidelityRecord {
            mechanism_id,
            model_system_order,
            residuals,
            target_observed,
            transport_milli: transport,
            coverage_milli: coverage,
            confidence_milli: transport.min(coverage),
            negative_evidence_order: negative,
        });
    }
    let all_ready = records.iter().all(|record| {
        record.target_observed
            && record.model_system_order.len() >= request.min_model_systems
            && record.transport_milli >= request.min_transport_milli
    });
    let disposition = if records.iter().all(|record| !record.target_observed) {
        MechanismFidelityBridgeDisposition::Blocked
    } else if !all_ready {
        MechanismFidelityBridgeDisposition::Partial
    } else {
        MechanismFidelityBridgeDisposition::Ready
    };
    let mut frontier = records.clone();
    frontier.sort_by(|left, right| {
        left.transport_milli
            .cmp(&right.transport_milli)
            .then_with(|| left.coverage_milli.cmp(&right.coverage_milli))
            .then_with(|| left.mechanism_id.cmp(&right.mechanism_id))
    });
    let frontier_order = frontier
        .into_iter()
        .map(|record| record.mechanism_id)
        .collect::<Vec<_>>();
    let mut output = MechanismFidelityBridge {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        target_model_system: request.target_model_system,
        mechanism_order,
        records,
        frontier_order,
        missing_target_order: missing_target.into_iter().collect(),
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"placeholder"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| MechanismFidelityBridgeError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn observation(
        id: &str,
        mechanism: &str,
        model_system: GliomaModelSystem,
        predicted: i64,
        observed: i64,
    ) -> MechanismFidelityObservation {
        MechanismFidelityObservation {
            observation_id: id.into(),
            mechanism_id: mechanism.into(),
            model_system,
            predicted_milli: predicted,
            observed_milli: observed,
            uncertainty_milli: 100,
            artifact: LocalArtifactRef {
                artifact_id: format!("artifact-{id}"),
                content_hash: ContentHash::of_bytes(id.as_bytes()),
                content_type: "application/vnd.aurora.glioma-feature+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
        }
    }

    fn request(observations: Vec<MechanismFidelityObservation>) -> MechanismFidelityBridgeRequest {
        MechanismFidelityBridgeRequest {
            objective: "transport glioma invasion mechanisms".into(),
            target_model_system: GliomaModelSystem::Organoid,
            observations,
            min_model_systems: 2,
            min_transport_milli: 700,
            max_observations: 16,
        }
    }

    #[test]
    fn bridge_ranks_low_transport_and_preserves_target_gaps() {
        let output = bridge_glioma_mechanism_fidelity(&request(vec![
            observation("near-cell", "near", GliomaModelSystem::CellLine, 100, 110),
            observation(
                "near-organoid",
                "near",
                GliomaModelSystem::Organoid,
                100,
                120,
            ),
            observation("far-cell", "far", GliomaModelSystem::CellLine, 100, 900),
            observation(
                "far-in_silico",
                "far",
                GliomaModelSystem::InSilico,
                100,
                850,
            ),
        ]))
        .unwrap();
        assert_eq!(
            output.disposition,
            MechanismFidelityBridgeDisposition::Partial
        );
        assert_eq!(output.frontier_order[0], "far");
        assert!(output
            .negative_evidence
            .iter()
            .any(|item| item.contains("far")));
        output.validate().unwrap();
    }

    #[test]
    fn bridge_rejects_duplicate_mechanism_model_pairs() {
        let error = bridge_glioma_mechanism_fidelity(&request(vec![
            observation("a", "near", GliomaModelSystem::CellLine, 100, 110),
            observation("b", "near", GliomaModelSystem::CellLine, 100, 120),
        ]))
        .unwrap_err();
        assert!(error.to_string().contains("unique"));
    }
}
