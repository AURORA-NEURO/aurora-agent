//! Robust federated continual knowledge fusion for preclinical glioma research.
//!
//! This feature consumes aggregate-only, caller-typed observations from multiple institutions
//! over successive release epochs. It uses robust medians and trimmed influence diagnostics so a
//! single site cannot silently define a consortium trend, while retaining quorum, privacy,
//! negative, contradiction, and coverage gates. No source text or raw experimental data crosses
//! this boundary, and no causal or clinical conclusion is inferred.

use super::knowledge_graph::KnowledgeClaimDisposition;
use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P02-F08";
pub const OUTPUT_SCHEMA: &str = "GliomaFederatedContinualKnowledge1@1";
pub const MAX_OBSERVATIONS: usize = 200_000;
pub const MAX_CLAIMS: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContinualKnowledgeRequest {
    pub objective: String,
    pub observations: Vec<FederatedContinualObservation>,
    pub min_sites: usize,
    pub min_epochs: usize,
    pub min_consensus_support_milli: u16,
    pub drift_threshold_milli: u16,
    pub outlier_threshold_milli: u16,
    pub required_modalities: BTreeSet<GliomaModality>,
    pub required_model_systems: BTreeSet<GliomaModelSystem>,
    pub max_claims: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContinualObservation {
    pub claim_key: String,
    pub claim_digest: ContentHash,
    pub site_id: String,
    pub epoch: u32,
    pub support_milli: u16,
    pub confidence_milli: u16,
    pub contradiction_milli: u16,
    pub source_count: usize,
    pub independent_artifact_count: usize,
    pub modality_order: Vec<GliomaModality>,
    pub model_system_order: Vec<GliomaModelSystem>,
    pub disposition: KnowledgeClaimDisposition,
    pub exportable: bool,
    pub preclinical_only: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedEpochDisposition {
    Consensus,
    Contested,
    Negative,
    Insufficient,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedEpochConsensus {
    pub epoch: u32,
    pub site_order: Vec<String>,
    pub site_count: usize,
    pub quorum_milli: u16,
    pub robust_support_milli: u16,
    pub robust_confidence_milli: u16,
    pub support_iqr_milli: u16,
    pub outlier_site_order: Vec<String>,
    pub negative_site_order: Vec<String>,
    pub contradiction_site_order: Vec<String>,
    pub disposition: FederatedEpochDisposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedContinualTrend {
    Strengthening,
    Weakening,
    Stable,
    Contested,
    Negative,
    Insufficient,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedContinualClaimDisposition {
    Qualified,
    Partial,
    Unresolved,
    Quarantined,
    Insufficient,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContinualClaim {
    pub claim_key: String,
    pub claim_digest: ContentHash,
    pub epoch_order: Vec<u32>,
    pub epochs: Vec<FederatedEpochConsensus>,
    pub baseline_support_milli: u16,
    pub latest_support_milli: u16,
    pub support_delta_milli: i32,
    pub leave_one_site_out_influence_milli: u16,
    pub modality_order: Vec<GliomaModality>,
    pub model_system_order: Vec<GliomaModelSystem>,
    pub missing_modality_order: Vec<GliomaModality>,
    pub missing_model_system_order: Vec<GliomaModelSystem>,
    pub negative_observation_count: usize,
    pub contradiction_observation_count: usize,
    pub omission_order: Vec<String>,
    pub trend: FederatedContinualTrend,
    pub disposition: FederatedContinualClaimDisposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedContinualDisposition {
    Qualified,
    Partial,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContinualKnowledge {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub claim_order: Vec<String>,
    pub qualified_claim_order: Vec<String>,
    pub partial_claim_order: Vec<String>,
    pub unresolved_claim_order: Vec<String>,
    pub quarantined_claim_order: Vec<String>,
    pub insufficient_claim_order: Vec<String>,
    pub claims: Vec<FederatedContinualClaim>,
    pub omission_order: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub disposition: FederatedContinualDisposition,
    pub next_step: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedContinualKnowledgeError {
    #[error("federated continual request is invalid: {0}")]
    InvalidRequest(String),
    #[error("federated continual output is invalid: {0}")]
    InvalidOutput(String),
    #[error("federated continual digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &FederatedContinualKnowledge) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "claim_order": output.claim_order,
        "qualified_claim_order": output.qualified_claim_order,
        "partial_claim_order": output.partial_claim_order,
        "unresolved_claim_order": output.unresolved_claim_order,
        "quarantined_claim_order": output.quarantined_claim_order,
        "insufficient_claim_order": output.insufficient_claim_order,
        "claims": output.claims,
        "omission_order": output.omission_order,
        "uncertainty": output.uncertainty,
        "negative_evidence": output.negative_evidence,
        "disposition": output.disposition,
        "next_step": output.next_step,
    })
}

impl FederatedContinualKnowledge {
    pub fn validate(&self) -> Result<(), FederatedContinualKnowledgeError> {
        let claim_order = self
            .claims
            .iter()
            .map(|claim| claim.claim_key.clone())
            .collect::<Vec<_>>();
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || claim_order != self.claim_order
            || !canonical(&self.claim_order)
            || !canonical(&self.qualified_claim_order)
            || !canonical(&self.partial_claim_order)
            || !canonical(&self.unresolved_claim_order)
            || !canonical(&self.quarantined_claim_order)
            || !canonical(&self.insufficient_claim_order)
            || !canonical(&self.omission_order)
            || !canonical(&self.uncertainty)
            || !canonical(&self.negative_evidence)
            || self.claims.iter().any(|claim| {
                claim.claim_key.trim().is_empty()
                    || claim.epoch_order.len() != claim.epochs.len()
                    || claim
                        .epoch_order
                        .iter()
                        .copied()
                        .ne(claim.epochs.iter().map(|epoch| epoch.epoch))
                    || !canonical(&claim.epoch_order)
                    || !canonical(&claim.modality_order)
                    || !canonical(&claim.model_system_order)
                    || !canonical(&claim.missing_modality_order)
                    || !canonical(&claim.missing_model_system_order)
                    || !canonical(&claim.omission_order)
                    || claim.baseline_support_milli > 1_000
                    || claim.latest_support_milli > 1_000
                    || claim.leave_one_site_out_influence_milli > 1_000
                    || claim.epochs.iter().any(|epoch| {
                        !canonical(&epoch.site_order)
                            || !canonical(&epoch.outlier_site_order)
                            || !canonical(&epoch.negative_site_order)
                            || !canonical(&epoch.contradiction_site_order)
                            || epoch.quorum_milli > 1_000
                            || epoch.robust_support_milli > 1_000
                            || epoch.robust_confidence_milli > 1_000
                            || epoch.support_iqr_milli > 1_000
                    })
            })
        {
            return Err(FederatedContinualKnowledgeError::InvalidOutput(
                "identity, ordering, epoch cardinality, or score bounds are invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| FederatedContinualKnowledgeError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(FederatedContinualKnowledgeError::InvalidOutput(
                "federated continual digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &FederatedContinualKnowledgeRequest,
) -> Result<(), FederatedContinualKnowledgeError> {
    if request.objective.trim().is_empty()
        || request.observations.is_empty()
        || request.observations.len() > MAX_OBSERVATIONS
        || request.min_sites == 0
        || request.min_epochs == 0
        || request.min_consensus_support_milli > 1_000
        || request.drift_threshold_milli == 0
        || request.drift_threshold_milli > 1_000
        || request.outlier_threshold_milli == 0
        || request.outlier_threshold_milli > 1_000
        || request.max_claims == 0
        || request.max_claims > MAX_CLAIMS
    {
        return Err(FederatedContinualKnowledgeError::InvalidRequest(
            "objective, observation capacity, quorum/epoch floors, score thresholds, or claim bound is invalid".into(),
        ));
    }
    let mut observation_keys = BTreeSet::new();
    let mut claim_digests = BTreeMap::<String, ContentHash>::new();
    let mut claim_keys = BTreeSet::new();
    for observation in &request.observations {
        if observation.claim_key.trim().is_empty()
            || observation.site_id.trim().is_empty()
            || !observation.exportable
            || !observation.preclinical_only
            || observation.source_count == 0
            || observation.independent_artifact_count == 0
            || observation.support_milli > 1_000
            || observation.confidence_milli > 1_000
            || observation.contradiction_milli > 1_000
            || !canonical(&observation.modality_order)
            || !canonical(&observation.model_system_order)
            || !observation_keys.insert((
                observation.claim_key.clone(),
                observation.site_id.clone(),
                observation.epoch,
            ))
        {
            return Err(FederatedContinualKnowledgeError::InvalidRequest(
                "aggregate-only identity, export policy, source/artifact floors, score bounds, coverage ordering, or uniqueness is invalid".into(),
            ));
        }
        if let Some(existing) = claim_digests.get(&observation.claim_key) {
            if existing != &observation.claim_digest {
                return Err(FederatedContinualKnowledgeError::InvalidRequest(
                    "the same claim key has conflicting semantic digests across sites".into(),
                ));
            }
        } else {
            claim_digests.insert(
                observation.claim_key.clone(),
                observation.claim_digest.clone(),
            );
        }
        claim_keys.insert(observation.claim_key.clone());
    }
    if claim_keys.len() > request.max_claims {
        return Err(FederatedContinualKnowledgeError::InvalidRequest(
            "aggregate stream exceeds the requested claim capacity".into(),
        ));
    }
    Ok(())
}

fn median(values: &mut [u16]) -> u16 {
    values.sort_unstable();
    if values.is_empty() {
        0
    } else if values.len() % 2 == 1 {
        values[values.len() / 2]
    } else {
        ((u32::from(values[values.len() / 2 - 1]) + u32::from(values[values.len() / 2])) / 2) as u16
    }
}

fn iqr(values: &[u16]) -> u16 {
    if values.len() < 2 {
        return 0;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();
    let lower = sorted[(sorted.len() - 1) / 4];
    let upper = sorted[((sorted.len() - 1) * 3) / 4];
    upper.saturating_sub(lower)
}

fn robust_mean(values: impl Iterator<Item = (u16, u16)>) -> u16 {
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

fn epoch_disposition(
    observations: &[&FederatedContinualObservation],
    request: &FederatedContinualKnowledgeRequest,
    robust_support_milli: u16,
) -> FederatedEpochDisposition {
    if observations.len() < request.min_sites {
        FederatedEpochDisposition::Insufficient
    } else if observations
        .iter()
        .all(|observation| matches!(observation.disposition, KnowledgeClaimDisposition::Negative))
    {
        FederatedEpochDisposition::Negative
    } else if observations.iter().any(|observation| {
        matches!(
            observation.disposition,
            KnowledgeClaimDisposition::Negative | KnowledgeClaimDisposition::Contested
        ) || observation.contradiction_milli >= request.drift_threshold_milli
    }) {
        FederatedEpochDisposition::Contested
    } else if robust_support_milli >= request.min_consensus_support_milli {
        FederatedEpochDisposition::Consensus
    } else {
        FederatedEpochDisposition::Insufficient
    }
}

pub fn analyze_federated_continual_knowledge(
    request: &FederatedContinualKnowledgeRequest,
) -> Result<FederatedContinualKnowledge, FederatedContinualKnowledgeError> {
    validate_request(request)?;
    let mut grouped = BTreeMap::<String, BTreeMap<u32, Vec<FederatedContinualObservation>>>::new();
    for observation in &request.observations {
        grouped
            .entry(observation.claim_key.clone())
            .or_default()
            .entry(observation.epoch)
            .or_default()
            .push(observation.clone());
    }
    let mut claims = Vec::new();
    let mut qualified_claim_order = Vec::new();
    let mut partial_claim_order = Vec::new();
    let mut unresolved_claim_order = Vec::new();
    let mut quarantined_claim_order = Vec::new();
    let mut insufficient_claim_order = Vec::new();
    let mut omission_order = Vec::new();
    let mut uncertainty = Vec::new();
    let mut negative_evidence = Vec::new();
    for (claim_key, epochs) in grouped {
        let claim_digest = request
            .observations
            .iter()
            .find(|observation| observation.claim_key == claim_key)
            .expect("validated claim has observation")
            .claim_digest
            .clone();
        let mut epoch_summaries = Vec::new();
        let mut union_modalities = BTreeSet::new();
        let mut union_models = BTreeSet::new();
        let mut negative_observation_count = 0;
        let mut contradiction_observation_count = 0;
        for (epoch, observations) in epochs {
            let references = observations.iter().collect::<Vec<_>>();
            for observation in &references {
                union_modalities.extend(observation.modality_order.iter().copied());
                union_models.extend(observation.model_system_order.iter().copied());
                if matches!(observation.disposition, KnowledgeClaimDisposition::Negative) {
                    negative_observation_count += 1;
                }
                if matches!(
                    observation.disposition,
                    KnowledgeClaimDisposition::Contested
                ) || observation.contradiction_milli >= request.drift_threshold_milli
                {
                    contradiction_observation_count += 1;
                }
            }
            let mut supports = references
                .iter()
                .map(|observation| observation.support_milli)
                .collect::<Vec<_>>();
            let robust_support_milli = median(&mut supports);
            let robust_confidence_milli = robust_mean(references.iter().map(|observation| {
                (
                    observation.confidence_milli,
                    observation.independent_artifact_count.min(1_000) as u16,
                )
            }));
            let support_values = references
                .iter()
                .map(|observation| observation.support_milli)
                .collect::<Vec<_>>();
            let outlier_site_order = references
                .iter()
                .filter(|observation| {
                    observation.support_milli.abs_diff(robust_support_milli)
                        >= request.outlier_threshold_milli
                })
                .map(|observation| observation.site_id.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            let negative_site_order = references
                .iter()
                .filter(|observation| {
                    matches!(observation.disposition, KnowledgeClaimDisposition::Negative)
                })
                .map(|observation| observation.site_id.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            let contradiction_site_order = references
                .iter()
                .filter(|observation| {
                    matches!(
                        observation.disposition,
                        KnowledgeClaimDisposition::Contested
                    ) || observation.contradiction_milli >= request.drift_threshold_milli
                })
                .map(|observation| observation.site_id.clone())
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            let disposition = epoch_disposition(&references, request, robust_support_milli);
            epoch_summaries.push(FederatedEpochConsensus {
                epoch,
                site_order: references
                    .iter()
                    .map(|observation| observation.site_id.clone())
                    .collect::<BTreeSet<_>>()
                    .into_iter()
                    .collect(),
                site_count: references.len(),
                quorum_milli: ((references.len().min(request.min_sites) as u32 * 1_000)
                    / request.min_sites as u32) as u16,
                robust_support_milli,
                robust_confidence_milli,
                support_iqr_milli: iqr(&support_values),
                outlier_site_order,
                negative_site_order,
                contradiction_site_order,
                disposition,
            });
        }
        epoch_summaries.sort_by_key(|summary| summary.epoch);
        let epoch_order = epoch_summaries
            .iter()
            .map(|summary| summary.epoch)
            .collect::<Vec<_>>();
        let baseline_support_milli = epoch_summaries
            .first()
            .map(|summary| summary.robust_support_milli)
            .unwrap_or(0);
        let latest_support_milli = epoch_summaries
            .last()
            .map(|summary| summary.robust_support_milli)
            .unwrap_or(0);
        let support_delta_milli =
            i32::from(latest_support_milli) - i32::from(baseline_support_milli);
        let latest_raw = request
            .observations
            .iter()
            .filter(|observation| {
                observation.claim_key == claim_key
                    && Some(observation.epoch) == epoch_order.last().copied()
            })
            .collect::<Vec<_>>();
        let leave_one_site_out_influence_milli = latest_raw
            .iter()
            .map(|removed| {
                let mut remaining = latest_raw
                    .iter()
                    .filter(|observation| observation.site_id != removed.site_id)
                    .map(|observation| observation.support_milli)
                    .collect::<Vec<_>>();
                median(&mut remaining).abs_diff(latest_support_milli)
            })
            .max()
            .unwrap_or(0);
        let missing_modality_order = request
            .required_modalities
            .difference(&union_modalities)
            .copied()
            .collect::<Vec<_>>();
        let missing_model_system_order = request
            .required_model_systems
            .difference(&union_models)
            .copied()
            .collect::<Vec<_>>();
        let latest_disposition = epoch_summaries
            .last()
            .map(|summary| summary.disposition)
            .unwrap_or(FederatedEpochDisposition::Insufficient);
        let trend = if epoch_summaries.len() < request.min_epochs
            || !missing_modality_order.is_empty()
            || !missing_model_system_order.is_empty()
        {
            FederatedContinualTrend::Insufficient
        } else if latest_disposition == FederatedEpochDisposition::Negative {
            FederatedContinualTrend::Negative
        } else if latest_disposition == FederatedEpochDisposition::Contested
            || epoch_summaries
                .iter()
                .any(|summary| summary.disposition == FederatedEpochDisposition::Contested)
        {
            FederatedContinualTrend::Contested
        } else if support_delta_milli >= i32::from(request.drift_threshold_milli) {
            FederatedContinualTrend::Strengthening
        } else if support_delta_milli <= -i32::from(request.drift_threshold_milli) {
            FederatedContinualTrend::Weakening
        } else {
            FederatedContinualTrend::Stable
        };
        let mut row_omissions = Vec::new();
        if epoch_summaries.len() < request.min_epochs {
            row_omissions.push(format!("{claim_key}: epoch floor is unmet"));
        }
        if epoch_summaries
            .iter()
            .any(|summary| summary.site_count < request.min_sites)
        {
            row_omissions.push(format!(
                "{claim_key}: site quorum is unmet for one or more epochs"
            ));
        }
        row_omissions.extend(
            missing_modality_order
                .iter()
                .map(|modality| format!("{claim_key}: missing modality {modality:?}")),
        );
        row_omissions.extend(
            missing_model_system_order
                .iter()
                .map(|model| format!("{claim_key}: missing model {model:?}")),
        );
        if leave_one_site_out_influence_milli >= request.outlier_threshold_milli {
            row_omissions.push(format!(
                "{claim_key}: one-site influence exceeds the outlier threshold"
            ));
        }
        row_omissions.sort();
        row_omissions.dedup();
        let disposition = if epoch_summaries.len() < request.min_epochs
            || epoch_summaries
                .iter()
                .any(|summary| summary.site_count < request.min_sites)
        {
            FederatedContinualClaimDisposition::Insufficient
        } else if latest_disposition == FederatedEpochDisposition::Negative {
            FederatedContinualClaimDisposition::Quarantined
        } else if latest_disposition == FederatedEpochDisposition::Contested
            || !missing_modality_order.is_empty()
            || !missing_model_system_order.is_empty()
        {
            FederatedContinualClaimDisposition::Unresolved
        } else if row_omissions.is_empty()
            && latest_disposition == FederatedEpochDisposition::Consensus
        {
            FederatedContinualClaimDisposition::Qualified
        } else {
            FederatedContinualClaimDisposition::Partial
        };
        match disposition {
            FederatedContinualClaimDisposition::Qualified => {
                qualified_claim_order.push(claim_key.clone())
            }
            FederatedContinualClaimDisposition::Partial => {
                partial_claim_order.push(claim_key.clone())
            }
            FederatedContinualClaimDisposition::Unresolved => {
                unresolved_claim_order.push(claim_key.clone())
            }
            FederatedContinualClaimDisposition::Quarantined => {
                quarantined_claim_order.push(claim_key.clone())
            }
            FederatedContinualClaimDisposition::Insufficient => {
                insufficient_claim_order.push(claim_key.clone())
            }
        }
        if negative_observation_count > 0 {
            negative_evidence.push(format!(
                "{claim_key}: negative evidence is present in the federated stream"
            ));
        }
        if !row_omissions.is_empty() {
            uncertainty.push(format!(
                "{claim_key}: federated continual closure is incomplete"
            ));
        }
        omission_order.extend(row_omissions.iter().cloned());
        claims.push(FederatedContinualClaim {
            claim_key,
            claim_digest,
            epoch_order,
            epochs: epoch_summaries,
            baseline_support_milli,
            latest_support_milli,
            support_delta_milli,
            leave_one_site_out_influence_milli,
            modality_order: union_modalities.into_iter().collect(),
            model_system_order: union_models.into_iter().collect(),
            missing_modality_order,
            missing_model_system_order,
            negative_observation_count,
            contradiction_observation_count,
            omission_order: row_omissions,
            trend,
            disposition,
        });
    }
    claims.sort_by(|left, right| left.claim_key.cmp(&right.claim_key));
    qualified_claim_order.sort();
    partial_claim_order.sort();
    unresolved_claim_order.sort();
    quarantined_claim_order.sort();
    insufficient_claim_order.sort();
    omission_order.sort();
    omission_order.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    negative_evidence.sort();
    negative_evidence.dedup();
    let claim_order = claims
        .iter()
        .map(|claim| claim.claim_key.clone())
        .collect::<Vec<_>>();
    let disposition = if claims.is_empty() {
        FederatedContinualDisposition::Blocked
    } else if !quarantined_claim_order.is_empty() || !unresolved_claim_order.is_empty() {
        FederatedContinualDisposition::Unresolved
    } else if !partial_claim_order.is_empty() || !insufficient_claim_order.is_empty() {
        FederatedContinualDisposition::Partial
    } else {
        FederatedContinualDisposition::Qualified
    };
    let next_step = match disposition {
        FederatedContinualDisposition::Qualified => {
            "release robust consensus rows to bounded consistency and mechanism planning"
        }
        FederatedContinualDisposition::Partial => {
            "acquire missing quorum/epoch/coverage and review high-influence sites before promotion"
        }
        FederatedContinualDisposition::Unresolved => {
            "quarantine contested or negative federated rows and route local adjudication"
        }
        FederatedContinualDisposition::Blocked => {
            "provide aggregate-only preclinical observations for at least one canonical claim"
        }
    };
    let mut output = FederatedContinualKnowledge {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        claim_order,
        qualified_claim_order,
        partial_claim_order,
        unresolved_claim_order,
        quarantined_claim_order,
        insufficient_claim_order,
        claims,
        omission_order,
        uncertainty,
        negative_evidence,
        disposition,
        next_step: next_step.into(),
        digest: ContentHash::of_value(&serde_json::Value::Null)
            .map_err(|error| FederatedContinualKnowledgeError::Digest(error.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| FederatedContinualKnowledgeError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn digest(key: &str) -> ContentHash {
        ContentHash::of_value(&serde_json::json!({"claim": key})).unwrap()
    }

    fn observation(
        key: &str,
        site: &str,
        epoch: u32,
        support_milli: u16,
        disposition: KnowledgeClaimDisposition,
    ) -> FederatedContinualObservation {
        FederatedContinualObservation {
            claim_key: key.into(),
            claim_digest: digest(key),
            site_id: site.into(),
            epoch,
            support_milli,
            confidence_milli: 900,
            contradiction_milli: if matches!(disposition, KnowledgeClaimDisposition::Contested) {
                900
            } else {
                0
            },
            source_count: 3,
            independent_artifact_count: 2,
            modality_order: vec![GliomaModality::Transcriptomics],
            model_system_order: vec![GliomaModelSystem::Organoid],
            disposition,
            exportable: true,
            preclinical_only: true,
        }
    }

    fn request(
        observations: Vec<FederatedContinualObservation>,
    ) -> FederatedContinualKnowledgeRequest {
        FederatedContinualKnowledgeRequest {
            objective: "federated continual glioma knowledge".into(),
            observations,
            min_sites: 3,
            min_epochs: 2,
            min_consensus_support_milli: 700,
            drift_threshold_milli: 150,
            outlier_threshold_milli: 250,
            required_modalities: BTreeSet::from([GliomaModality::Transcriptomics]),
            required_model_systems: BTreeSet::from([GliomaModelSystem::Organoid]),
            max_claims: 8,
        }
    }

    #[test]
    fn robust_continual_consensus_qualifies_and_exposes_influence() {
        let mut observations = Vec::new();
        for epoch in 1..=2 {
            observations.extend([
                observation(
                    "egfr-invasion",
                    "site-a",
                    epoch,
                    820,
                    KnowledgeClaimDisposition::Supported,
                ),
                observation(
                    "egfr-invasion",
                    "site-b",
                    epoch,
                    800,
                    KnowledgeClaimDisposition::Supported,
                ),
                observation(
                    "egfr-invasion",
                    "site-c",
                    epoch,
                    810,
                    KnowledgeClaimDisposition::Supported,
                ),
            ]);
        }
        let output =
            analyze_federated_continual_knowledge(&request(observations)).expect("federation");
        assert_eq!(output.disposition, FederatedContinualDisposition::Qualified);
        assert_eq!(output.qualified_claim_order, vec!["egfr-invasion"]);
        assert_eq!(output.claims[0].leave_one_site_out_influence_milli, 5);
        output.validate().expect("digest validates");
    }

    #[test]
    fn outlier_and_negative_site_are_not_silently_pooled() {
        let mut observations = Vec::new();
        for epoch in 1..=2 {
            observations.extend([
                observation(
                    "egfr-invasion",
                    "site-a",
                    epoch,
                    850,
                    KnowledgeClaimDisposition::Supported,
                ),
                observation(
                    "egfr-invasion",
                    "site-b",
                    epoch,
                    840,
                    KnowledgeClaimDisposition::Supported,
                ),
                observation(
                    "egfr-invasion",
                    "site-c",
                    epoch,
                    100,
                    KnowledgeClaimDisposition::Negative,
                ),
            ]);
        }
        let output =
            analyze_federated_continual_knowledge(&request(observations)).expect("federation");
        assert_eq!(
            output.disposition,
            FederatedContinualDisposition::Unresolved
        );
        assert_eq!(
            output.claims[0].epochs[0].outlier_site_order,
            vec!["site-c"]
        );
        assert!(!output.negative_evidence.is_empty());
    }

    #[test]
    fn protected_export_policy_is_fail_closed() {
        let mut item = observation(
            "egfr-invasion",
            "site-a",
            1,
            800,
            KnowledgeClaimDisposition::Supported,
        );
        item.exportable = false;
        let error =
            analyze_federated_continual_knowledge(&request(vec![item])).expect_err("policy");
        assert!(matches!(
            error,
            FederatedContinualKnowledgeError::InvalidRequest(_)
        ));
    }
}
