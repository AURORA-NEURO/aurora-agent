//! Deterministic evaluation-card construction from capability-run telemetry.
//!
//! Atlas feature: `AFA-evalengine-P23-F01`.
//!
//! This is deliberately a measurement product, not a scientific truth oracle. It consumes typed
//! run observations, computes a cost-normalized auditable-discovery rate and a Wilson uncertainty
//! interval, keeps every declared baseline visible, and refuses to report a production pass when
//! a baseline is missing or under-sampled. Raw data stays outside the receipt; only content hashes
//! and aggregate telemetry cross the contract boundary.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect as ResearchEffect,
    EvaluationCard, EvaluationMetric, EvidenceReference, EvidenceState, ReleaseVerdict,
    ResearchContractError, ResearchSurface, TypedPort, TypedResearchArtifact, UncertaintyStatement,
    PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

/// Stable atlas identity for this implementation slice.
pub const FEATURE_ID: &str = "AFA-evalengine-P23-F01";
pub const FEATURE_CONTRACT_VERSION: &str = "0.1.0";

/// One replayable run observation. Counts are aggregate research telemetry, never raw data.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityRunObservation {
    pub run_id: String,
    pub baseline_id: String,
    pub reproducible_conclusions: u64,
    pub total_conclusions: u64,
    pub cost_units: f64,
    pub evidence_digest: ContentHash,
}

/// Input to the evaluation and observability plane.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvaluationCardRequest {
    pub capability_id: String,
    pub benchmark_world: String,
    pub baseline_ids: Vec<String>,
    pub observations: Vec<CapabilityRunObservation>,
    pub limitations: Vec<String>,
    pub target_success_fraction: f64,
    pub minimum_observations_per_baseline: usize,
}

/// EvaluationCard plus the aggregate provenance and omission evidence needed to audit how it was
/// produced. A `Pass` here means the declared measurement gates passed; it does not claim
/// biological validity or clinical utility.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvaluationCardReceipt {
    pub schema_version: String,
    pub feature_id: String,
    pub card: EvaluationCard,
    pub card_digest: ContentHash,
    pub observations_digest: ContentHash,
    pub baseline_counts: BTreeMap<String, usize>,
    pub omissions: Vec<String>,
    pub reasons: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub boundary: String,
}

impl EvaluationCardReceipt {
    pub fn digest(&self) -> Result<ContentHash, EvaluationObservabilityError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| EvaluationObservabilityError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| EvaluationObservabilityError::Serialization(error.to_string()))
    }

    pub fn validate(&self) -> Result<(), EvaluationObservabilityError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION {
            return Err(EvaluationObservabilityError::Contract(
                ResearchContractError::SchemaVersion {
                    expected: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
                    found: self.schema_version.clone(),
                },
            ));
        }
        if self.feature_id != FEATURE_ID {
            return Err(EvaluationObservabilityError::InvalidRequest(
                "evaluation observability feature id mismatch".into(),
            ));
        }
        if self.boundary != PRECLINICAL_BOUNDARY {
            return Err(EvaluationObservabilityError::Contract(
                ResearchContractError::BoundaryMismatch {
                    capability: self.card.capability_id.clone(),
                },
            ));
        }
        self.card.validate()?;
        if self.reasons.is_empty() || self.baseline_counts.is_empty() {
            return Err(EvaluationObservabilityError::InvalidRequest(
                "evaluation receipt needs baseline counts and reasons".into(),
            ));
        }
        self.artifact.validate_metadata()?;
        Ok(())
    }
}

#[derive(Debug, Error)]
pub enum EvaluationObservabilityError {
    #[error("research contract rejected evaluation card: {0}")]
    Contract(#[from] ResearchContractError),
    #[error("invalid evaluation observability request: {0}")]
    InvalidRequest(String),
    #[error("observation references unknown baseline {0}")]
    UnknownBaseline(String),
    #[error("duplicate baseline id {0}")]
    DuplicateBaseline(String),
    #[error("observation run id is missing")]
    MissingRunId,
    #[error("observation count is inconsistent for run {0}")]
    InconsistentCounts(String),
    #[error("observation cost is invalid for run {0}")]
    InvalidCost(String),
    #[error("cannot serialize evaluation observability receipt: {0}")]
    Serialization(String),
}

/// Capability manifest for the local, deterministic evaluation-card compiler.
pub fn evaluation_observability_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: FEATURE_CONTRACT_VERSION.into(),
        owner_crate: "evalengine".into(),
        consumers: ["benchmark curator".into(), "research program lead".into()].into(),
        behavior: "computes a cost-normalized EvaluationCard from typed capability-run telemetry with baseline coverage and uncertainty".into(),
        value: "makes evaluation telemetry auditable, comparable, and fail-closed without promoting a metric into biological validity".into(),
        inputs: vec![TypedPort {
            name: "capability_run_observations".into(),
            schema: "EvaluationCardRequest@1".into(),
            required: true,
        }],
        outputs: vec![TypedPort {
            name: "evaluation_card_receipt".into(),
            schema: "EvaluationCardReceipt@1".into(),
            required: true,
        }],
        effects: [
            ResearchEffect::ReadLocalData,
            ResearchEffect::WriteLocalArtifact,
            ResearchEffect::ExecuteLocalComputation,
        ]
        .into(),
        permissions: ["read:local-telemetry".into(), "write:local-research-artifact".into()]
            .into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference {
            source_id: "opentelemetry-spec".into(),
            state: EvidenceState::Supported,
            locator: Some("https://opentelemetry.io/docs/specs/".into()),
        }],
        authority_requirements: vec![AuthorityRequirement {
            role: "benchmark curator".into(),
            reason: "evaluation-card thresholds and baseline identities are accountable release inputs".into(),
        }],
        autonomy_tier: AutonomyTier::A1,
        surfaces: [
            ResearchSurface::Cli,
            ResearchSurface::Api,
            ResearchSurface::Sdk,
            ResearchSurface::McpTool,
            ResearchSurface::Operator,
        ]
        .into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

/// Compile a deterministic evaluation card and its aggregate provenance receipt.
pub fn compile_evaluation_card(
    request: &EvaluationCardRequest,
) -> Result<EvaluationCardReceipt, EvaluationObservabilityError> {
    validate_request(request)?;
    let mut observations = request.observations.clone();
    observations.sort_by(|left, right| {
        left.baseline_id
            .cmp(&right.baseline_id)
            .then(left.run_id.cmp(&right.run_id))
    });

    let mut baseline_counts = BTreeMap::new();
    let mut metrics = Vec::new();
    let mut uncertainty = Vec::new();
    let mut omissions = Vec::new();
    let mut total_successes = 0u64;
    let mut total_conclusions = 0u64;
    let mut total_cost = 0.0f64;
    for baseline in &request.baseline_ids {
        let rows: Vec<&CapabilityRunObservation> = observations
            .iter()
            .filter(|observation| &observation.baseline_id == baseline)
            .collect();
        baseline_counts.insert(baseline.clone(), rows.len());
        if rows.len() < request.minimum_observations_per_baseline {
            omissions.push(format!(
                "baseline {baseline} has {} observations; required {}",
                rows.len(),
                request.minimum_observations_per_baseline
            ));
        }
        let successes: u64 = rows.iter().map(|row| row.reproducible_conclusions).sum();
        let conclusions: u64 = rows.iter().map(|row| row.total_conclusions).sum();
        let cost: f64 = rows.iter().map(|row| row.cost_units).sum();
        let fraction = ratio(successes, conclusions);
        let (lower, upper) = wilson_interval(successes, conclusions);
        let rate = ratio_f64(successes as f64, cost);
        metrics.push(EvaluationMetric {
            name: format!("auditable_discovery_rate::{baseline}"),
            value: format!("{rate:.6} reproducible_conclusions/cost_unit"),
            uncertainty: format!(
                "success_fraction={fraction:.6}; Wilson95=[{lower:.6},{upper:.6}]"
            ),
        });
        uncertainty.push(UncertaintyStatement {
            kind: format!("baseline::{baseline}"),
            statement: format!(
                "{} runs, {} reproducible of {} conclusions, cost {:.6}; interval [{lower:.6},{upper:.6}]",
                rows.len(), successes, conclusions, cost
            ),
        });
        total_successes += successes;
        total_conclusions += conclusions;
        total_cost += cost;
    }
    let global_fraction = ratio(total_successes, total_conclusions);
    let global_rate = ratio_f64(total_successes as f64, total_cost);
    let (global_lower, global_upper) = wilson_interval(total_successes, total_conclusions);
    metrics.push(EvaluationMetric {
        name: "auditable_discovery_rate::all_baselines".into(),
        value: format!("{global_rate:.6} reproducible_conclusions/cost_unit"),
        uncertainty: format!(
            "success_fraction={global_fraction:.6}; Wilson95=[{global_lower:.6},{global_upper:.6}]"
        ),
    });
    let pass = omissions.is_empty() && global_lower >= request.target_success_fraction;
    let release_verdict = if !omissions.is_empty() {
        ReleaseVerdict::Blocked
    } else if pass {
        ReleaseVerdict::Pass
    } else {
        ReleaseVerdict::Conditional
    };
    let mut limitations = request.limitations.clone();
    limitations.push("evaluation metrics measure reproducible research workflow performance; they do not establish biological validity or clinical utility".into());
    let card = EvaluationCard {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: request.capability_id.clone(),
        benchmark_world: request.benchmark_world.clone(),
        baselines: request.baseline_ids.clone(),
        metrics,
        uncertainty,
        limitations,
        release_verdict,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    card.validate()?;
    let card_value = serde_json::to_value(&card)
        .map_err(|error| EvaluationObservabilityError::Serialization(error.to_string()))?;
    let card_digest = ContentHash::of_value(&card_value)
        .map_err(|error| EvaluationObservabilityError::Serialization(error.to_string()))?;
    let observations_value = serde_json::to_value(&observations)
        .map_err(|error| EvaluationObservabilityError::Serialization(error.to_string()))?;
    let observations_digest = ContentHash::of_value(&observations_value)
        .map_err(|error| EvaluationObservabilityError::Serialization(error.to_string()))?;
    let reasons = if omissions.is_empty() {
        vec![format!(
            "all baselines met the observation floor; global Wilson lower bound {global_lower:.6} against target {:.6}",
            request.target_success_fraction
        )]
    } else {
        omissions.clone()
    };
    let artifact_payload = json!({
        "feature_id": FEATURE_ID,
        "capability_id": request.capability_id,
        "card_digest": card_digest,
        "observations_digest": observations_digest,
        "baseline_counts": baseline_counts,
        "omissions": omissions,
        "release_verdict": card.release_verdict,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("{}:{}", FEATURE_ID, request.capability_id),
        "application/vnd.aurora.evaluation-card-receipt+json",
        &artifact_payload,
        Vec::new(),
        vec![
            bioprism_foundation::ProvenanceLink {
                source_id: "evaluation-card".into(),
                relation: "derived-from".into(),
                digest: card_digest.clone(),
            },
            bioprism_foundation::ProvenanceLink {
                source_id: "capability-run-observations".into(),
                relation: "derived-from".into(),
                digest: observations_digest.clone(),
            },
        ],
    )?;
    let receipt = EvaluationCardReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        card,
        card_digest,
        observations_digest,
        baseline_counts,
        omissions,
        reasons,
        artifact,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &EvaluationCardRequest) -> Result<(), EvaluationObservabilityError> {
    if request.capability_id.trim().is_empty() || request.benchmark_world.trim().is_empty() {
        return Err(EvaluationObservabilityError::InvalidRequest(
            "capability id and benchmark world are required".into(),
        ));
    }
    if request.baseline_ids.is_empty() {
        return Err(EvaluationObservabilityError::InvalidRequest(
            "at least one baseline is required".into(),
        ));
    }
    let mut seen = BTreeSet::new();
    for baseline in &request.baseline_ids {
        if baseline.trim().is_empty() {
            return Err(EvaluationObservabilityError::InvalidRequest(
                "baseline ids cannot be empty".into(),
            ));
        }
        if !seen.insert(baseline.clone()) {
            return Err(EvaluationObservabilityError::DuplicateBaseline(
                baseline.clone(),
            ));
        }
    }
    if request.limitations.is_empty() || request.minimum_observations_per_baseline == 0 {
        return Err(EvaluationObservabilityError::InvalidRequest(
            "limitations and a positive observation floor are required".into(),
        ));
    }
    if !request.target_success_fraction.is_finite()
        || !(0.0..=1.0).contains(&request.target_success_fraction)
    {
        return Err(EvaluationObservabilityError::InvalidRequest(
            "target success fraction must be finite and within [0,1]".into(),
        ));
    }
    for observation in &request.observations {
        if observation.run_id.trim().is_empty() {
            return Err(EvaluationObservabilityError::MissingRunId);
        }
        if !seen.contains(&observation.baseline_id) {
            return Err(EvaluationObservabilityError::UnknownBaseline(
                observation.baseline_id.clone(),
            ));
        }
        if observation.reproducible_conclusions > observation.total_conclusions
            || observation.total_conclusions == 0
        {
            return Err(EvaluationObservabilityError::InconsistentCounts(
                observation.run_id.clone(),
            ));
        }
        if !observation.cost_units.is_finite() || observation.cost_units <= 0.0 {
            return Err(EvaluationObservabilityError::InvalidCost(
                observation.run_id.clone(),
            ));
        }
    }
    Ok(())
}

fn ratio(successes: u64, total: u64) -> f64 {
    ratio_f64(successes as f64, total as f64)
}

fn ratio_f64(numerator: f64, denominator: f64) -> f64 {
    if denominator == 0.0 {
        0.0
    } else {
        numerator / denominator
    }
}

fn wilson_interval(successes: u64, total: u64) -> (f64, f64) {
    if total == 0 {
        return (0.0, 0.0);
    }
    let n = total as f64;
    let p = successes as f64 / n;
    let z = 1.959_963_984_540_054_f64;
    let z2 = z * z;
    let denominator = 1.0 + z2 / n;
    let center = (p + z2 / (2.0 * n)) / denominator;
    let half = z * ((p * (1.0 - p) / n) + z2 / (4.0 * n * n)).sqrt() / denominator;
    ((center - half).max(0.0), (center + half).min(1.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> EvaluationCardRequest {
        EvaluationCardRequest {
            capability_id: "capability:workflow-executor".into(),
            benchmark_world: "preclinical-synthetic-world-v1".into(),
            baseline_ids: vec!["fixed-search".into(), "candidate".into()],
            observations: vec![
                CapabilityRunObservation {
                    run_id: "candidate-run-2".into(),
                    baseline_id: "candidate".into(),
                    reproducible_conclusions: 8,
                    total_conclusions: 10,
                    cost_units: 4.0,
                    evidence_digest: ContentHash::of_bytes(b"candidate-2"),
                },
                CapabilityRunObservation {
                    run_id: "fixed-run-1".into(),
                    baseline_id: "fixed-search".into(),
                    reproducible_conclusions: 5,
                    total_conclusions: 10,
                    cost_units: 5.0,
                    evidence_digest: ContentHash::of_bytes(b"fixed-1"),
                },
                CapabilityRunObservation {
                    run_id: "candidate-run-1".into(),
                    baseline_id: "candidate".into(),
                    reproducible_conclusions: 7,
                    total_conclusions: 10,
                    cost_units: 4.0,
                    evidence_digest: ContentHash::of_bytes(b"candidate-1"),
                },
                CapabilityRunObservation {
                    run_id: "fixed-run-2".into(),
                    baseline_id: "fixed-search".into(),
                    reproducible_conclusions: 6,
                    total_conclusions: 10,
                    cost_units: 5.0,
                    evidence_digest: ContentHash::of_bytes(b"fixed-2"),
                },
            ],
            limitations: vec!["synthetic benchmark only".into()],
            target_success_fraction: 0.2,
            minimum_observations_per_baseline: 2,
        }
    }

    #[test]
    fn card_is_deterministic_under_observation_reordering() {
        let left = compile_evaluation_card(&request()).unwrap();
        let mut reordered = request();
        reordered.observations.reverse();
        let right = compile_evaluation_card(&reordered).unwrap();
        assert_eq!(left.observations_digest, right.observations_digest);
        assert_eq!(left.digest().unwrap(), right.digest().unwrap());
        assert_eq!(left.card.release_verdict, ReleaseVerdict::Pass);
    }

    #[test]
    fn missing_baseline_is_blocked_and_explicit() {
        let mut request = request();
        request
            .observations
            .retain(|row| row.baseline_id == "candidate");
        let receipt = compile_evaluation_card(&request).unwrap();
        assert_eq!(receipt.card.release_verdict, ReleaseVerdict::Blocked);
        assert_eq!(receipt.baseline_counts["fixed-search"], 0);
        assert!(!receipt.omissions.is_empty());
    }

    #[test]
    fn malformed_counts_and_costs_are_rejected() {
        let mut counts = request();
        counts.observations[0].reproducible_conclusions = 11;
        assert!(matches!(
            compile_evaluation_card(&counts).unwrap_err(),
            EvaluationObservabilityError::InconsistentCounts(_)
        ));

        let mut cost = request();
        cost.observations[0].cost_units = 0.0;
        assert!(matches!(
            compile_evaluation_card(&cost).unwrap_err(),
            EvaluationObservabilityError::InvalidCost(_)
        ));
    }

    #[test]
    fn incomplete_baseline_coverage_cannot_pass() {
        let mut request = request();
        request.minimum_observations_per_baseline = 3;
        let receipt = compile_evaluation_card(&request).unwrap();
        assert_eq!(receipt.card.release_verdict, ReleaseVerdict::Blocked);
    }
}
