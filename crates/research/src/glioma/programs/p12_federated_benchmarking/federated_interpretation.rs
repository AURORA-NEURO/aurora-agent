//! Federated, aggregate-only interpretation gate for preclinical glioma research.
//!
//! P10 can produce an uncertainty-aware local interpretation, while P12 can estimate whether a
//! declared capability transports across independent aggregate site summaries.  This feature
//! joins those two signals without pooling raw observations.  Qualification requires alignment;
//! a qualified local interpretation with heterogeneous or negative consortium evidence remains
//! partial or negative rather than becoming a stronger claim.

use super::consensus::{
    analyze_federated_benchmark, FederatedBenchmarkConsensus, FederatedBenchmarkDisposition,
    FederatedBenchmarkError, FederatedBenchmarkRequest, FederatedBenchmarkSite,
};
use crate::glioma::programs::p10_interpretation_replication::{
    ClosureInterpretationDisposition, ClosureInterpretationRun,
};
use crate::glioma_engine::GliomaModelSystem;
use bioprism_foundation::PRECLINICAL_BOUNDARY;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P12-F30";
pub const OUTPUT_SCHEMA: &str = "GliomaFederatedInterpretation1@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedInterpretationRequest {
    pub interpretation: ClosureInterpretationRun,
    pub benchmark: FederatedBenchmarkRequest,
    pub sites: Vec<FederatedBenchmarkSite>,
    pub require_qualified_interpretation: bool,
    pub require_qualified_consensus: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedInterpretationDisposition {
    Qualified,
    Negative,
    Heterogeneous,
    Partial,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedInterpretationRun {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub interpretation_digest: ContentHash,
    pub consensus: FederatedBenchmarkConsensus,
    pub interpretation_disposition: ClosureInterpretationDisposition,
    pub consensus_disposition: FederatedBenchmarkDisposition,
    pub alignment: bool,
    pub disposition: FederatedInterpretationDisposition,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub next_action: String,
    pub boundary: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedInterpretationError {
    #[error("federated interpretation request is invalid: {0}")]
    InvalidRequest(String),
    #[error("local closure interpretation is invalid: {0}")]
    Interpretation(String),
    #[error("federated consensus failed: {0}")]
    Consensus(#[from] FederatedBenchmarkError),
    #[error("federated interpretation output is invalid: {0}")]
    InvalidOutput(String),
    #[error("federated interpretation digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn sorted_unique(values: impl IntoIterator<Item = String>) -> Vec<String> {
    values
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn digest_input(output: &FederatedInterpretationRun) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "model_system": output.model_system,
        "interpretation_digest": output.interpretation_digest,
        "consensus": output.consensus,
        "interpretation_disposition": output.interpretation_disposition,
        "consensus_disposition": output.consensus_disposition,
        "alignment": output.alignment,
        "disposition": output.disposition,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "next_action": output.next_action,
        "boundary": output.boundary,
    })
}

fn combined_disposition(
    interpretation: ClosureInterpretationDisposition,
    consensus: FederatedBenchmarkDisposition,
    require_interpretation: bool,
    require_consensus: bool,
) -> (FederatedInterpretationDisposition, bool, &'static str) {
    let interpretation_ok = matches!(interpretation, ClosureInterpretationDisposition::Qualified);
    let consensus_ok = matches!(consensus, FederatedBenchmarkDisposition::Qualified);
    let alignment = interpretation_ok && consensus_ok;
    if matches!(consensus, FederatedBenchmarkDisposition::Heterogeneous) {
        return (
            FederatedInterpretationDisposition::Heterogeneous,
            alignment,
            "reconcile cross-site heterogeneity before generalizing the mechanism",
        );
    }
    if matches!(consensus, FederatedBenchmarkDisposition::Negative)
        || matches!(interpretation, ClosureInterpretationDisposition::Negative)
    {
        return (
            FederatedInterpretationDisposition::Negative,
            alignment,
            "publish the negative or contradictory consortium result with its model boundary",
        );
    }
    if require_interpretation && !interpretation_ok {
        return (
            FederatedInterpretationDisposition::Partial,
            alignment,
            "resolve the local interpretation gates before making a federated claim",
        );
    }
    if require_consensus && !consensus_ok {
        return (
            FederatedInterpretationDisposition::Unresolved,
            alignment,
            "acquire enough independent aggregate sites to clear the federated consensus gate",
        );
    }
    if alignment {
        (
            FederatedInterpretationDisposition::Qualified,
            true,
            "hold the aligned interpretation for consortium methods review and governed release",
        )
    } else {
        (
            FederatedInterpretationDisposition::Partial,
            false,
            "review local/federated disagreement and open a bounded follow-up frontier",
        )
    }
}

impl FederatedInterpretationRun {
    pub fn validate(&self) -> Result<(), FederatedInterpretationError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.next_action.trim().is_empty()
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.consensus.objective != self.objective
            || self.alignment
                != (matches!(
                    self.interpretation_disposition,
                    ClosureInterpretationDisposition::Qualified
                ) && matches!(
                    self.consensus_disposition,
                    FederatedBenchmarkDisposition::Qualified
                ))
        {
            return Err(FederatedInterpretationError::InvalidOutput(
                "identity, objective/model binding, boundary, evidence ordering, or alignment invariant failed".into(),
            ));
        }
        self.consensus
            .validate()
            .map_err(FederatedInterpretationError::Consensus)?;
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| FederatedInterpretationError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(FederatedInterpretationError::InvalidOutput(
                "federated interpretation digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Combine P10 local interpretation with aggregate-only P12 site consensus.
pub fn interpret_glioma_federated_closure(
    request: &FederatedInterpretationRequest,
) -> Result<FederatedInterpretationRun, FederatedInterpretationError> {
    if request.interpretation.objective != request.benchmark.objective
        || request.interpretation.model_system != request.benchmark.model_system
        || request.benchmark.objective.trim().is_empty()
        || request.sites.is_empty()
    {
        return Err(FederatedInterpretationError::InvalidRequest(
            "local interpretation, benchmark, model system, objective, and aggregate sites must match".into(),
        ));
    }
    request
        .interpretation
        .validate()
        .map_err(|error| FederatedInterpretationError::Interpretation(error.to_string()))?;
    let consensus = analyze_federated_benchmark(&request.benchmark, &request.sites)?;
    let (disposition, alignment, next_action) = combined_disposition(
        request.interpretation.disposition,
        consensus.disposition,
        request.require_qualified_interpretation,
        request.require_qualified_consensus,
    );
    let mut negative_evidence = consensus.negative_evidence.clone();
    negative_evidence.extend(request.interpretation.negative_evidence.clone());
    let mut uncertainty = consensus.uncertainty.clone();
    uncertainty.extend(request.interpretation.uncertainty.clone());
    if !alignment {
        uncertainty
            .push("local-interpretation-and-federated-consensus-are-not-both-qualified".into());
    }
    let mut output = FederatedInterpretationRun {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.benchmark.objective.clone(),
        model_system: request.benchmark.model_system,
        interpretation_digest: request.interpretation.digest.clone(),
        consensus_disposition: consensus.disposition,
        consensus,
        interpretation_disposition: request.interpretation.disposition,
        alignment,
        disposition,
        negative_evidence: sorted_unique(negative_evidence),
        uncertainty: sorted_unique(uncertainty),
        next_action: next_action.into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
        digest: ContentHash::of_bytes(b"unsealed-glioma-federated-interpretation"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| FederatedInterpretationError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_local_and_federated_objective_drift_before_consensus() {
        let error = interpret_glioma_federated_closure(&FederatedInterpretationRequest {
            interpretation: ClosureInterpretationRun {
                feature_id: "GAF-GLIOMA-P10-F30".into(),
                output_schema: "invalid".into(),
                campaign_id: "campaign".into(),
                objective: "local objective".into(),
                model_system: GliomaModelSystem::Organoid,
                campaign_digest: ContentHash::of_bytes(b"campaign"),
                campaign_evidence_order: Vec::new(),
                synthesis: crate::glioma::programs::p10_interpretation_replication::InterpretationSynthesis {
                    feature_id: "GAF-GLIOMA-P10-F19".into(),
                    output_schema: "GliomaInterpretationSynthesis1@1".into(),
                    objective: "local objective".into(),
                    hypothesis: "hypothesis".into(),
                    model_system: GliomaModelSystem::Organoid,
                    replay_identity: ContentHash::of_bytes(b"synthesis"),
                    evidence_order: Vec::new(),
                    unresolved_evidence_order: Vec::new(),
                    family_order: Vec::new(),
                    independent_group_order: Vec::new(),
                    families: Vec::new(),
                    aggregate_effect_milli: 0,
                    interval_low_milli: 0,
                    interval_high_milli: 0,
                    leave_one_out_low_milli: 0,
                    leave_one_out_high_milli: 0,
                    support_milli: 0,
                    contradiction_milli: 0,
                    disagreement_milli: 0,
                    stability_milli: 0,
                    negative_evidence: Vec::new(),
                    uncertainty: Vec::new(),
                    disposition: crate::glioma::programs::p10_interpretation_replication::InterpretationSynthesisDisposition::Unresolved,
                    digest: ContentHash::of_bytes(b"invalid"),
                    evidence: Vec::new(),
                },
                disposition: ClosureInterpretationDisposition::Unresolved,
                negative_evidence: Vec::new(),
                uncertainty: Vec::new(),
                next_action: "resolve".into(),
                boundary: PRECLINICAL_BOUNDARY.into(),
                digest: ContentHash::of_bytes(b"invalid"),
            },
            benchmark: FederatedBenchmarkRequest {
                objective: "different objective".into(),
                capability_id: "capability".into(),
                benchmark_world: "world".into(),
                metric_name: "metric".into(),
                model_system: GliomaModelSystem::Organoid,
                minimum_sites: 1,
                minimum_replicates_per_site: 1,
                effect_threshold_milli: 1,
                max_i2_milli: 1_000,
                min_signal_to_noise_milli: 1,
                max_site_spread_milli: 1_000,
                max_leave_one_out_shift_milli: 1_000,
            },
            sites: Vec::new(),
            require_qualified_interpretation: true,
            require_qualified_consensus: true,
        })
        .unwrap_err();
        assert!(matches!(
            error,
            FederatedInterpretationError::InvalidRequest(_)
        ));
    }
}
