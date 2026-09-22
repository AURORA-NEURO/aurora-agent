//! Deterministic Bayesian-style mechanism updating for preclinical glioma research.
//!
//! This feature consumes competing mechanism predictions and typed local observations, then
//! updates a bounded integer posterior. It is deliberately a model-updating aid rather than a
//! causal oracle: missing features, weak coverage, diffuse posteriors, and contradictory fits are
//! retained as explicit states for the next experiment-design or mechanism-policy stage.

use super::discrimination::{MechanismFeatureObservation, MechanismPrediction};
use crate::glioma_engine::GliomaModelSystem;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P05-F08";
pub const OUTPUT_SCHEMA: &str = "GliomaMechanismBayesianUpdate1@1";
pub const MAX_HYPOTHESES: usize = 256;
pub const MAX_OBSERVATIONS: usize = 16_384;
pub const MAX_PREDICTIONS: usize = 4_096;
pub const MAX_SCALE_MILLI: u64 = 1_000_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BayesianMechanismHypothesis {
    pub mechanism_id: String,
    pub statement: String,
    pub prior_milli: u16,
    pub predictions: Vec<MechanismPrediction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BayesianMechanismUpdateRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub min_shared_features: usize,
    pub max_hypotheses: usize,
    pub likelihood_scale_milli: u64,
    pub supported_posterior_floor_milli: u16,
    pub contradicted_posterior_ceiling_milli: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismPosteriorStatus {
    Supported,
    Contradicted,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismPosteriorRecord {
    pub mechanism_id: String,
    pub statement: String,
    pub prior_milli: u16,
    pub matched_feature_order: Vec<String>,
    pub missing_feature_order: Vec<String>,
    pub residual_loss_milli: u64,
    pub likelihood_milli: u64,
    pub posterior_milli: u16,
    pub status: MechanismPosteriorStatus,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BayesianUpdateDisposition {
    Qualified,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismBayesianUpdate {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub mechanism_order: Vec<String>,
    pub observed_feature_order: Vec<String>,
    pub records: Vec<MechanismPosteriorRecord>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: BayesianUpdateDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum BayesianMechanismUpdateError {
    #[error("Bayesian mechanism update request is invalid: {0}")]
    InvalidRequest(String),
    #[error("Bayesian mechanism update input is invalid: {0}")]
    InvalidInput(String),
    #[error("Bayesian mechanism update output is invalid: {0}")]
    InvalidOutput(String),
    #[error("Bayesian mechanism update digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &MechanismBayesianUpdate) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "model_system": output.model_system,
        "mechanism_order": output.mechanism_order,
        "observed_feature_order": output.observed_feature_order,
        "records": output.records,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

impl MechanismBayesianUpdate {
    pub fn validate(&self) -> Result<(), BayesianMechanismUpdateError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !canonical(&self.mechanism_order)
            || !canonical(&self.observed_feature_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.records.windows(2).any(|pair| {
                pair[0].posterior_milli < pair[1].posterior_milli
                    || (pair[0].posterior_milli == pair[1].posterior_milli
                        && pair[0].mechanism_id > pair[1].mechanism_id)
            })
            || self.records.iter().any(|record| {
                record.mechanism_id.trim().is_empty()
                    || record.statement.trim().is_empty()
                    || record.prior_milli == 0
                    || record.likelihood_milli == 0
                    || !canonical(&record.matched_feature_order)
                    || !canonical(&record.missing_feature_order)
                    || record
                        .matched_feature_order
                        .iter()
                        .any(|feature| record.missing_feature_order.binary_search(feature).is_ok())
            })
        {
            return Err(BayesianMechanismUpdateError::InvalidOutput(
                "identity, ordering, posterior records, or feature partitions are invalid".into(),
            ));
        }
        let mechanism_ids = self
            .mechanism_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let record_ids = self
            .records
            .iter()
            .map(|record| record.mechanism_id.clone())
            .collect::<BTreeSet<_>>();
        let posterior_sum = self
            .records
            .iter()
            .map(|record| u32::from(record.posterior_milli))
            .sum::<u32>();
        if mechanism_ids != record_ids || posterior_sum != 1_000 {
            return Err(BayesianMechanismUpdateError::InvalidOutput(
                "mechanism records must cover exactly the declared order and sum to 1000 milli"
                    .into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| BayesianMechanismUpdateError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(BayesianMechanismUpdateError::InvalidOutput(
                "digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &BayesianMechanismUpdateRequest,
) -> Result<(), BayesianMechanismUpdateError> {
    if request.objective.trim().is_empty()
        || request.min_shared_features == 0
        || request.max_hypotheses == 0
        || request.max_hypotheses > MAX_HYPOTHESES
        || request.likelihood_scale_milli == 0
        || request.likelihood_scale_milli > MAX_SCALE_MILLI
        || request.supported_posterior_floor_milli > 1_000
        || request.contradicted_posterior_ceiling_milli > 1_000
        || request.contradicted_posterior_ceiling_milli >= request.supported_posterior_floor_milli
    {
        return Err(BayesianMechanismUpdateError::InvalidRequest(
            "objective, positive bounds, ordered posterior thresholds, and a bounded likelihood scale are required".into(),
        ));
    }
    Ok(())
}

fn abs_diff(left: i64, right: i64) -> u64 {
    left.abs_diff(right)
}

fn allocate_posteriors(weights: &[(String, u128)]) -> BTreeMap<String, u16> {
    let total = weights.iter().map(|(_, weight)| *weight).sum::<u128>();
    if total == 0 {
        return weights
            .iter()
            .map(|(id, _)| (id.clone(), 0))
            .collect::<BTreeMap<_, _>>();
    }
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
        .collect::<BTreeMap<_, _>>()
}

fn update_record(
    hypothesis: &BayesianMechanismHypothesis,
    observations: &BTreeMap<String, &MechanismFeatureObservation>,
    request: &BayesianMechanismUpdateRequest,
) -> Result<(MechanismPosteriorRecord, u128), BayesianMechanismUpdateError> {
    let predictions = hypothesis
        .predictions
        .iter()
        .map(|prediction| (prediction.feature_id.clone(), prediction))
        .collect::<BTreeMap<_, _>>();
    if predictions.len() != hypothesis.predictions.len() {
        return Err(BayesianMechanismUpdateError::InvalidInput(format!(
            "mechanism {} repeats a prediction feature",
            hypothesis.mechanism_id
        )));
    }
    let mut matched = Vec::new();
    let mut missing = Vec::new();
    let mut residual_sum = 0_u64;
    let mut uncertainty_sum = 0_u64;
    for feature in observations.keys() {
        if let Some(prediction) = predictions.get(feature) {
            let observation = observations[feature];
            matched.push(feature.clone());
            residual_sum = residual_sum.saturating_add(abs_diff(
                prediction.predicted_milli,
                observation.observed_milli,
            ));
            uncertainty_sum = uncertainty_sum
                .saturating_add(prediction.uncertainty_milli)
                .saturating_add(observation.uncertainty_milli);
        } else {
            missing.push(feature.clone());
        }
    }
    matched.sort();
    missing.sort();
    let sufficient = matched.len() >= request.min_shared_features;
    let residual_loss = if matched.is_empty() {
        0
    } else {
        residual_sum / matched.len() as u64
    };
    let uncertainty_scale = if matched.is_empty() {
        0
    } else {
        uncertainty_sum / matched.len() as u64
    };
    let effective_scale = request
        .likelihood_scale_milli
        .saturating_add(uncertainty_scale)
        .max(1);
    let likelihood = if sufficient {
        1_000_000_u64
            .checked_div(1_u64.saturating_add(residual_loss / effective_scale))
            .unwrap_or(1)
            .max(1)
    } else {
        1
    };
    let status = if !sufficient {
        MechanismPosteriorStatus::Unresolved
    } else {
        MechanismPosteriorStatus::Unresolved
    };
    let record = MechanismPosteriorRecord {
        mechanism_id: hypothesis.mechanism_id.clone(),
        statement: hypothesis.statement.clone(),
        prior_milli: hypothesis.prior_milli,
        matched_feature_order: matched,
        missing_feature_order: missing,
        residual_loss_milli: residual_loss,
        likelihood_milli: likelihood,
        posterior_milli: 0,
        status,
    };
    Ok((
        record,
        u128::from(hypothesis.prior_milli) * u128::from(likelihood),
    ))
}

/// Update competing mechanism weights from typed local observations using deterministic integer
/// likelihoods. The output can be consumed by the adaptive mechanism policy or experiment
/// design stages; it does not dispatch an assay or claim observed causality.
pub fn update_glioma_mechanism_posterior(
    request: &BayesianMechanismUpdateRequest,
    hypotheses: &[BayesianMechanismHypothesis],
    observations: &[MechanismFeatureObservation],
) -> Result<MechanismBayesianUpdate, BayesianMechanismUpdateError> {
    validate_request(request)?;
    if hypotheses.is_empty()
        || hypotheses.len() > request.max_hypotheses
        || observations.len() > MAX_OBSERVATIONS
        || hypotheses
            .iter()
            .map(|item| item.predictions.len())
            .sum::<usize>()
            > MAX_PREDICTIONS
    {
        return Err(BayesianMechanismUpdateError::InvalidInput(
            "hypothesis, observation, or prediction bound exceeded".into(),
        ));
    }
    let mut hypothesis_ids = BTreeSet::new();
    let mut prior_sum = 0_u32;
    for hypothesis in hypotheses {
        if hypothesis.mechanism_id.trim().is_empty()
            || hypothesis.statement.trim().is_empty()
            || hypothesis.prior_milli == 0
            || !hypothesis_ids.insert(hypothesis.mechanism_id.clone())
        {
            return Err(BayesianMechanismUpdateError::InvalidInput(
                "mechanism ids/statements must be non-empty and unique with positive priors".into(),
            ));
        }
        prior_sum = prior_sum.saturating_add(u32::from(hypothesis.prior_milli));
    }
    if prior_sum != 1_000 {
        return Err(BayesianMechanismUpdateError::InvalidInput(
            "mechanism priors must sum to exactly 1000 milli".into(),
        ));
    }
    let mut observations_by_feature = BTreeMap::new();
    for observation in observations {
        if observation.feature_id.trim().is_empty()
            || observation.uncertainty_milli == 0
            || observations_by_feature
                .insert(observation.feature_id.clone(), observation)
                .is_some()
        {
            return Err(BayesianMechanismUpdateError::InvalidInput(
                "observation feature ids must be unique with positive uncertainty".into(),
            ));
        }
    }
    let mut mechanism_order = hypothesis_ids.iter().cloned().collect::<Vec<_>>();
    mechanism_order.sort();
    let observed_feature_order = observations_by_feature.keys().cloned().collect::<Vec<_>>();
    let mut rows = hypotheses
        .iter()
        .map(|hypothesis| update_record(hypothesis, &observations_by_feature, request))
        .collect::<Result<Vec<_>, _>>()?;
    let weights = rows
        .iter()
        .map(|(record, weight)| (record.mechanism_id.clone(), *weight))
        .collect::<Vec<_>>();
    let posteriors = allocate_posteriors(&weights);
    for (record, _) in &mut rows {
        record.posterior_milli = *posteriors.get(&record.mechanism_id).unwrap_or(&0);
        let sufficient = record.matched_feature_order.len() >= request.min_shared_features;
        record.status = if !sufficient {
            MechanismPosteriorStatus::Unresolved
        } else if record.posterior_milli >= request.supported_posterior_floor_milli {
            MechanismPosteriorStatus::Supported
        } else if record.posterior_milli <= request.contradicted_posterior_ceiling_milli {
            MechanismPosteriorStatus::Contradicted
        } else {
            MechanismPosteriorStatus::Unresolved
        };
    }
    rows.sort_by(|left, right| {
        right
            .0
            .posterior_milli
            .cmp(&left.0.posterior_milli)
            .then_with(|| left.0.mechanism_id.cmp(&right.0.mechanism_id))
    });
    let has_coverage = rows
        .iter()
        .any(|(record, _)| record.matched_feature_order.len() >= request.min_shared_features);
    let has_support = rows
        .iter()
        .any(|(record, _)| record.status == MechanismPosteriorStatus::Supported);
    let disposition = if !has_coverage {
        BayesianUpdateDisposition::Unresolved
    } else if has_support {
        BayesianUpdateDisposition::Qualified
    } else {
        BayesianUpdateDisposition::Partial
    };
    let mut negative_evidence = rows
        .iter()
        .filter(|(record, _)| record.status == MechanismPosteriorStatus::Contradicted)
        .map(|(record, _)| {
            format!(
                "mechanism:{}:posterior-below-contradiction-ceiling",
                record.mechanism_id
            )
        })
        .collect::<Vec<_>>();
    let mut uncertainty = rows
        .iter()
        .filter(|(record, _)| record.status == MechanismPosteriorStatus::Unresolved)
        .map(|(record, _)| {
            if record.matched_feature_order.len() < request.min_shared_features {
                format!(
                    "mechanism:{}:insufficient-shared-feature-coverage",
                    record.mechanism_id
                )
            } else {
                format!(
                    "mechanism:{}:posterior-remains-diffuse",
                    record.mechanism_id
                )
            }
        })
        .collect::<Vec<_>>();
    if observations.is_empty() {
        uncertainty.push("no-local-observations-prior-only-update".into());
    }
    negative_evidence.sort();
    negative_evidence.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    let mut output = MechanismBayesianUpdate {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        mechanism_order,
        observed_feature_order,
        records: rows.into_iter().map(|(record, _)| record).collect(),
        negative_evidence,
        uncertainty,
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-mechanism-bayesian-update"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| BayesianMechanismUpdateError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma_engine::LocalArtifactRef;
    use bioprism_ids::ContentHash;

    fn hypothesis(id: &str, prior_milli: u16, predicted: i64) -> BayesianMechanismHypothesis {
        BayesianMechanismHypothesis {
            mechanism_id: id.into(),
            statement: format!("{id} mechanism"),
            prior_milli,
            predictions: vec![super::super::discrimination::MechanismPrediction {
                feature_id: "growth".into(),
                predicted_milli: predicted,
                uncertainty_milli: 10,
            }],
        }
    }

    fn observation(value: i64) -> MechanismFeatureObservation {
        MechanismFeatureObservation {
            feature_id: "growth".into(),
            observed_milli: value,
            uncertainty_milli: 10,
            artifact: LocalArtifactRef {
                artifact_id: "local-growth".into(),
                content_hash: ContentHash::of_bytes(b"growth"),
                content_type: "tabular-feature".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
        }
    }

    fn request() -> BayesianMechanismUpdateRequest {
        BayesianMechanismUpdateRequest {
            objective: "separate growth mechanisms".into(),
            model_system: GliomaModelSystem::Organoid,
            min_shared_features: 1,
            max_hypotheses: 4,
            likelihood_scale_milli: 100,
            supported_posterior_floor_milli: 600,
            contradicted_posterior_ceiling_milli: 100,
        }
    }

    #[test]
    fn posterior_favors_the_mechanism_with_the_lower_residual() {
        let output = update_glioma_mechanism_posterior(
            &request(),
            &[hypothesis("near", 500, 100), hypothesis("far", 500, 900)],
            &[observation(110)],
        )
        .unwrap();
        assert_eq!(output.records[0].mechanism_id, "near");
        assert_eq!(
            output.records[0].status,
            MechanismPosteriorStatus::Supported
        );
        assert_eq!(output.disposition, BayesianUpdateDisposition::Qualified);
        output.validate().unwrap();
    }

    #[test]
    fn missing_shared_features_remain_unresolved() {
        let mut request = request();
        request.min_shared_features = 2;
        let output = update_glioma_mechanism_posterior(
            &request,
            &[hypothesis("near", 500, 100), hypothesis("far", 500, 900)],
            &[observation(110)],
        )
        .unwrap();
        assert_eq!(output.disposition, BayesianUpdateDisposition::Unresolved);
        assert!(output
            .uncertainty
            .iter()
            .any(|item| item.contains("insufficient-shared-feature")));
    }

    #[test]
    fn input_order_does_not_change_the_posterior_digest() {
        let request = request();
        let left = update_glioma_mechanism_posterior(
            &request,
            &[hypothesis("near", 500, 100), hypothesis("far", 500, 900)],
            &[observation(110)],
        )
        .unwrap();
        let right = update_glioma_mechanism_posterior(
            &request,
            &[hypothesis("far", 500, 900), hypothesis("near", 500, 100)],
            &[observation(110)],
        )
        .unwrap();
        assert_eq!(left.digest, right.digest);
        assert_eq!(left.records, right.records);
    }
}
