//! Closed-loop mechanism evidence assimilation across preclinical glioma study epochs.
//!
//! Bayesian updates are deliberately local snapshots.  This feature combines those snapshots
//! into a recency-weighted posterior ledger, retains mechanisms that disappear from coverage,
//! exposes posterior trend and contradiction counts, and emits a bounded frontier for the next
//! discriminating action.  It never promotes a posterior to an observed causal result.

use super::bayesian_update::{MechanismBayesianUpdate, MechanismPosteriorStatus};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P05-F05";
pub const OUTPUT_SCHEMA: &str = "GliomaMechanismEvidenceAssimilation1@1";
pub const MAX_SNAPSHOTS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismEvidenceSnapshot {
    pub epoch_id: String,
    pub update: MechanismBayesianUpdate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismEvidenceAssimilationRequest {
    pub objective: String,
    pub snapshots: Vec<MechanismEvidenceSnapshot>,
    pub recency_decay_milli: u16,
    pub max_snapshots: usize,
    pub supported_floor_milli: u16,
    pub contradicted_ceiling_milli: u16,
    pub preserve_negative_results: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssimilatedMechanismStatus {
    Supported,
    Contradicted,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssimilatedMechanismRecord {
    pub mechanism_id: String,
    pub statement: String,
    pub posterior_milli: u16,
    pub earliest_posterior_milli: u16,
    pub latest_posterior_milli: u16,
    pub trend_milli: i32,
    pub observed_snapshot_order: Vec<String>,
    pub supported_snapshot_count: u16,
    pub contradicted_snapshot_count: u16,
    pub unresolved_snapshot_count: u16,
    pub status: AssimilatedMechanismStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismEvidenceAssimilationDisposition {
    Ready,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismEvidenceAssimilation {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub snapshot_order: Vec<String>,
    pub snapshot_digest_order: Vec<ContentHash>,
    pub mechanism_order: Vec<String>,
    pub records: Vec<AssimilatedMechanismRecord>,
    pub frontier_order: Vec<String>,
    pub omitted_mechanism_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: MechanismEvidenceAssimilationDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MechanismEvidenceAssimilationError {
    #[error("mechanism evidence assimilation request is invalid: {0}")]
    InvalidRequest(String),
    #[error("mechanism evidence assimilation snapshot is invalid: {0}")]
    InvalidSnapshot(String),
    #[error("mechanism evidence assimilation output is invalid: {0}")]
    InvalidOutput(String),
    #[error("mechanism evidence assimilation digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_nonempty(values: &[String]) -> bool {
    values.iter().all(|value| !value.trim().is_empty())
        && values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

fn digest_input(output: &MechanismEvidenceAssimilation) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "snapshot_order": output.snapshot_order,
        "snapshot_digest_order": output.snapshot_digest_order,
        "mechanism_order": output.mechanism_order,
        "records": output.records,
        "frontier_order": output.frontier_order,
        "omitted_mechanism_order": output.omitted_mechanism_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

impl MechanismEvidenceAssimilation {
    pub fn validate(&self) -> Result<(), MechanismEvidenceAssimilationError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.snapshot_order.is_empty()
            || !unique_nonempty(&self.snapshot_order)
            || self.snapshot_digest_order.len() != self.snapshot_order.len()
            || self
                .snapshot_digest_order
                .iter()
                .any(|digest| digest.as_str().len() != 64)
            || self.mechanism_order.is_empty()
            || !canonical(&self.mechanism_order)
            || self.records.len() != self.mechanism_order.len()
            || self
                .records
                .windows(2)
                .any(|pair| pair[0].mechanism_id >= pair[1].mechanism_id)
            || !unique_nonempty(&self.frontier_order)
            || !canonical(&self.omitted_mechanism_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.records.iter().any(|record| {
                record.mechanism_id.trim().is_empty()
                    || record.statement.trim().is_empty()
                    || record.posterior_milli > 1_000
                    || record.earliest_posterior_milli > 1_000
                    || record.latest_posterior_milli > 1_000
                    || !canonical(&record.observed_snapshot_order)
                    || record.observed_snapshot_order.is_empty()
                    || record.supported_snapshot_count
                        + record.contradicted_snapshot_count
                        + record.unresolved_snapshot_count
                        != record.observed_snapshot_order.len() as u16
            })
            || self.digest.as_str().len() != 64
        {
            return Err(MechanismEvidenceAssimilationError::InvalidOutput(
                "identity, ordering, snapshot coverage, posterior bounds, or digest shape is invalid".into(),
            ));
        }
        let record_ids = self
            .records
            .iter()
            .map(|record| record.mechanism_id.clone())
            .collect::<BTreeSet<_>>();
        if record_ids
            != self
                .mechanism_order
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>()
            || self
                .frontier_order
                .iter()
                .any(|id| !record_ids.contains(id))
            || self
                .omitted_mechanism_order
                .iter()
                .any(|id| !record_ids.contains(id))
        {
            return Err(MechanismEvidenceAssimilationError::InvalidOutput(
                "mechanism partitions do not reconcile with records".into(),
            ));
        }
        let posterior_sum = self
            .records
            .iter()
            .map(|record| u32::from(record.posterior_milli))
            .sum::<u32>();
        if posterior_sum != 1_000 {
            return Err(MechanismEvidenceAssimilationError::InvalidOutput(
                "assimilated posterior must sum to 1000 milli".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| MechanismEvidenceAssimilationError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(MechanismEvidenceAssimilationError::Digest(
                "mechanism evidence assimilation digest does not match canonical content".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &MechanismEvidenceAssimilationRequest,
) -> Result<(), MechanismEvidenceAssimilationError> {
    if request.objective.trim().is_empty()
        || request.snapshots.is_empty()
        || request.snapshots.len() > MAX_SNAPSHOTS
        || request.max_snapshots == 0
        || request.max_snapshots > MAX_SNAPSHOTS
        || request.snapshots.len() > request.max_snapshots
        || request.recency_decay_milli == 0
        || request.recency_decay_milli > 1_000
        || request.supported_floor_milli > 1_000
        || request.contradicted_ceiling_milli > 1_000
        || request.contradicted_ceiling_milli >= request.supported_floor_milli
    {
        return Err(MechanismEvidenceAssimilationError::InvalidRequest(
            "objective, bounded snapshots, positive recency decay, and ordered posterior thresholds are required".into(),
        ));
    }
    let mut epoch_ids = BTreeSet::new();
    for snapshot in &request.snapshots {
        if snapshot.epoch_id.trim().is_empty() || !epoch_ids.insert(snapshot.epoch_id.clone()) {
            return Err(MechanismEvidenceAssimilationError::InvalidRequest(
                "snapshot epoch ids must be unique and non-empty".into(),
            ));
        }
        if snapshot.update.objective != request.objective {
            return Err(MechanismEvidenceAssimilationError::InvalidSnapshot(
                format!(
                    "snapshot {} objective does not match assimilation objective",
                    snapshot.epoch_id
                ),
            ));
        }
        snapshot.update.validate().map_err(|error| {
            MechanismEvidenceAssimilationError::InvalidSnapshot(error.to_string())
        })?;
    }
    Ok(())
}

fn recency_weights(count: usize, decay_milli: u16) -> Vec<u64> {
    let mut weights = vec![1_000_u64; count];
    for index in (0..count.saturating_sub(1)).rev() {
        weights[index] = weights[index + 1].saturating_mul(u64::from(decay_milli)) / 1_000;
    }
    weights
}

fn allocate_posteriors(weights: &BTreeMap<String, u128>) -> BTreeMap<String, u16> {
    let total = weights.values().copied().sum::<u128>();
    let mut allocated = weights
        .iter()
        .map(|(id, weight)| {
            let scaled = weight.saturating_mul(1_000);
            (id.clone(), (scaled / total) as u16, scaled % total)
        })
        .collect::<Vec<_>>();
    let used = allocated
        .iter()
        .map(|(_, value, _)| u32::from(*value))
        .sum::<u32>();
    let mut remainder = 1_000_u32.saturating_sub(used);
    allocated.sort_by(|left, right| right.2.cmp(&left.2).then_with(|| left.0.cmp(&right.0)));
    for (_, value, _) in &mut allocated {
        if remainder == 0 {
            break;
        }
        *value = value.saturating_add(1);
        remainder -= 1;
    }
    allocated
        .into_iter()
        .map(|(id, value, _)| (id, value))
        .collect()
}

/// Assimilate Bayesian mechanism snapshots into a recency-weighted, contradiction-preserving
/// ledger for the next local discriminating workflow.
pub fn assimilate_glioma_mechanism_evidence(
    request: &MechanismEvidenceAssimilationRequest,
) -> Result<MechanismEvidenceAssimilation, MechanismEvidenceAssimilationError> {
    validate_request(request)?;
    let recency = recency_weights(request.snapshots.len(), request.recency_decay_milli);
    let mut mechanism_ids = BTreeSet::new();
    let mut statements = BTreeMap::new();
    for snapshot in &request.snapshots {
        for record in &snapshot.update.records {
            mechanism_ids.insert(record.mechanism_id.clone());
            statements
                .entry(record.mechanism_id.clone())
                .or_insert_with(|| record.statement.clone());
        }
    }
    if mechanism_ids.is_empty() {
        return Err(MechanismEvidenceAssimilationError::InvalidSnapshot(
            "snapshots contain no mechanism records".into(),
        ));
    }
    let mut records = Vec::new();
    let mut posterior_weights = BTreeMap::new();
    let mut omitted = BTreeSet::new();
    for mechanism_id in &mechanism_ids {
        let mut observed_snapshot_order = Vec::new();
        let mut weighted_sum = 0_u128;
        let mut weight_total = 0_u128;
        let mut earliest = None;
        let mut latest = None;
        let mut supported = 0_u16;
        let mut contradicted = 0_u16;
        let mut unresolved = 0_u16;
        for (index, snapshot) in request.snapshots.iter().enumerate() {
            if let Some(record) = snapshot
                .update
                .records
                .iter()
                .find(|record| &record.mechanism_id == mechanism_id)
            {
                observed_snapshot_order.push(snapshot.epoch_id.clone());
                weighted_sum = weighted_sum.saturating_add(
                    u128::from(record.posterior_milli).saturating_mul(u128::from(recency[index])),
                );
                weight_total = weight_total.saturating_add(u128::from(recency[index]));
                earliest.get_or_insert(record.posterior_milli);
                latest = Some(record.posterior_milli);
                match record.status {
                    MechanismPosteriorStatus::Supported => supported += 1,
                    MechanismPosteriorStatus::Contradicted => contradicted += 1,
                    MechanismPosteriorStatus::Unresolved => unresolved += 1,
                }
            }
        }
        if observed_snapshot_order.len() < request.snapshots.len() {
            omitted.insert(mechanism_id.clone());
        }
        let earliest_posterior = earliest.unwrap_or_default();
        let latest_posterior = latest.unwrap_or_default();
        let posterior = if weight_total == 0 {
            0
        } else {
            (weighted_sum / weight_total) as u16
        };
        posterior_weights.insert(mechanism_id.clone(), u128::from(posterior));
        let status = if posterior >= request.supported_floor_milli && supported > contradicted {
            AssimilatedMechanismStatus::Supported
        } else if posterior <= request.contradicted_ceiling_milli && contradicted > supported {
            AssimilatedMechanismStatus::Contradicted
        } else {
            AssimilatedMechanismStatus::Unresolved
        };
        records.push(AssimilatedMechanismRecord {
            mechanism_id: mechanism_id.clone(),
            statement: statements.get(mechanism_id).cloned().unwrap_or_default(),
            posterior_milli: posterior,
            earliest_posterior_milli: earliest_posterior,
            latest_posterior_milli: latest_posterior,
            trend_milli: i32::from(latest_posterior) - i32::from(earliest_posterior),
            observed_snapshot_order,
            supported_snapshot_count: supported,
            contradicted_snapshot_count: contradicted,
            unresolved_snapshot_count: unresolved,
            status,
        });
    }
    let normalized = allocate_posteriors(&posterior_weights);
    for record in &mut records {
        record.posterior_milli = *normalized.get(&record.mechanism_id).unwrap_or(&0);
    }
    records.sort_by(|left, right| left.mechanism_id.cmp(&right.mechanism_id));
    let mechanism_order = records
        .iter()
        .map(|record| record.mechanism_id.clone())
        .collect::<Vec<_>>();
    let mut frontier = records.clone();
    frontier.sort_by(|left, right| {
        (left.status == AssimilatedMechanismStatus::Unresolved)
            .cmp(&(right.status == AssimilatedMechanismStatus::Unresolved))
            .then_with(|| right.posterior_milli.cmp(&left.posterior_milli))
            .then_with(|| left.mechanism_id.cmp(&right.mechanism_id))
    });
    let frontier_order = frontier
        .into_iter()
        .map(|record| record.mechanism_id)
        .collect::<Vec<_>>();
    let mut negative_evidence = Vec::new();
    let mut uncertainty = Vec::new();
    for snapshot in &request.snapshots {
        negative_evidence.extend(snapshot.update.negative_evidence.iter().cloned());
        uncertainty.extend(snapshot.update.uncertainty.iter().cloned());
    }
    if !request.preserve_negative_results {
        negative_evidence.clear();
    }
    negative_evidence.sort();
    negative_evidence.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    let unresolved_count = records
        .iter()
        .filter(|record| record.status == AssimilatedMechanismStatus::Unresolved)
        .count();
    let disposition = if unresolved_count == records.len() {
        MechanismEvidenceAssimilationDisposition::Blocked
    } else if unresolved_count > 0 || !omitted.is_empty() {
        MechanismEvidenceAssimilationDisposition::Partial
    } else {
        MechanismEvidenceAssimilationDisposition::Ready
    };
    let mut output = MechanismEvidenceAssimilation {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        snapshot_order: request
            .snapshots
            .iter()
            .map(|snapshot| snapshot.epoch_id.clone())
            .collect(),
        snapshot_digest_order: request
            .snapshots
            .iter()
            .map(|snapshot| snapshot.update.digest.clone())
            .collect(),
        mechanism_order,
        records,
        frontier_order,
        omitted_mechanism_order: omitted.into_iter().collect(),
        negative_evidence,
        uncertainty,
        disposition,
        digest: ContentHash::of_bytes(b"placeholder"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| MechanismEvidenceAssimilationError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::super::bayesian_update::{
        update_glioma_mechanism_posterior, BayesianMechanismHypothesis,
        BayesianMechanismUpdateRequest,
    };
    use super::super::discrimination::{MechanismFeatureObservation, MechanismPrediction};
    use super::*;
    use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};

    fn observation(value: i64, label: &str) -> MechanismFeatureObservation {
        MechanismFeatureObservation {
            feature_id: "growth".into(),
            observed_milli: value,
            uncertainty_milli: 10,
            artifact: LocalArtifactRef {
                artifact_id: format!("artifact-{label}"),
                content_hash: ContentHash::of_bytes(label.as_bytes()),
                content_type: "tabular-feature".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
        }
    }

    fn update(value: i64, label: &str) -> MechanismBayesianUpdate {
        update_glioma_mechanism_posterior(
            &BayesianMechanismUpdateRequest {
                objective: "assimilate mechanism evidence".into(),
                model_system: GliomaModelSystem::Organoid,
                min_shared_features: 1,
                max_hypotheses: 4,
                likelihood_scale_milli: 100,
                supported_posterior_floor_milli: 600,
                contradicted_posterior_ceiling_milli: 100,
            },
            &[
                BayesianMechanismHypothesis {
                    mechanism_id: "near".into(),
                    statement: "near mechanism".into(),
                    prior_milli: 500,
                    predictions: vec![MechanismPrediction {
                        feature_id: "growth".into(),
                        predicted_milli: 100,
                        uncertainty_milli: 10,
                    }],
                },
                BayesianMechanismHypothesis {
                    mechanism_id: "far".into(),
                    statement: "far mechanism".into(),
                    prior_milli: 500,
                    predictions: vec![MechanismPrediction {
                        feature_id: "growth".into(),
                        predicted_milli: 900,
                        uncertainty_milli: 10,
                    }],
                },
            ],
            &[observation(value, label)],
        )
        .unwrap()
    }

    fn request(snapshots: Vec<MechanismEvidenceSnapshot>) -> MechanismEvidenceAssimilationRequest {
        MechanismEvidenceAssimilationRequest {
            objective: "assimilate mechanism evidence".into(),
            snapshots,
            recency_decay_milli: 800,
            max_snapshots: 8,
            supported_floor_milli: 600,
            contradicted_ceiling_milli: 100,
            preserve_negative_results: true,
        }
    }

    #[test]
    fn assimilation_is_recency_weighted_and_digest_bound() {
        let output = assimilate_glioma_mechanism_evidence(&request(vec![
            MechanismEvidenceSnapshot {
                epoch_id: "epoch-1".into(),
                update: update(110, "first"),
            },
            MechanismEvidenceSnapshot {
                epoch_id: "epoch-2".into(),
                update: update(890, "second"),
            },
        ]))
        .unwrap();
        assert_eq!(output.snapshot_order, vec!["epoch-1", "epoch-2"]);
        assert_eq!(output.records.len(), 2);
        assert!(output.records.iter().any(|record| record.trend_milli != 0));
        assert!(output.frontier_order.contains(&"near".into()));
        output.validate().unwrap();
    }

    #[test]
    fn assimilation_rejects_mismatched_snapshot_objectives() {
        let mut snapshot = MechanismEvidenceSnapshot {
            epoch_id: "epoch-1".into(),
            update: update(110, "first"),
        };
        snapshot.update.objective = "other objective".into();
        let error = assimilate_glioma_mechanism_evidence(&request(vec![snapshot])).unwrap_err();
        assert!(error.to_string().contains("objective"));
    }
}
