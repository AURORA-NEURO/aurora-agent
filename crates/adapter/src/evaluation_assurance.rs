//! Local evaluation and observability assurance harness.
//!
//! Atlas feature: `AFA-adapter-P23-F25`.
//!
//! The harness turns capability-run metrics, baseline comparisons, replay identity, and
//! witness-bearing checks into a release receipt. It does not execute a benchmark or infer
//! biology. Missing protected closure, provenance, witnesses, or metric measurements remain
//! unknown/blocked states; negative results remain first-class evidence.

use bioprism_foundation::{
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P23-F25";
pub const CONTRACT_VERSION: &str = "evaluation-assurance-harness/1.0";

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MetricObservation {
    pub metric_id: String,
    pub observed: Option<f64>,
    pub baseline: Option<f64>,
    pub minimum_delta: f64,
    pub uncertainty_width: Option<f64>,
    pub witness_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssuranceWitness {
    pub witness_id: String,
    pub kind: String,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityRun {
    pub run_id: String,
    pub capability_id: String,
    pub benchmark_id: String,
    pub baseline_id: String,
    pub metrics: Vec<MetricObservation>,
    pub required_witnesses: Vec<String>,
    pub witnesses: Vec<AssuranceWitness>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub policy_allow: bool,
    pub provenance_complete: bool,
    pub protected_closure: bool,
    pub replay_identity: ContentHash,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssuranceVerdict {
    Passed,
    Conditional,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvaluationAssuranceReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub run_id: String,
    pub capability_id: String,
    pub benchmark_id: String,
    pub baseline_id: String,
    pub verdict: AssuranceVerdict,
    pub metric_order: Vec<String>,
    pub gate_order: Vec<String>,
    pub witness_order: Vec<String>,
    pub counterexample_order: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub reasons: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub replay_identity: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

impl EvaluationAssuranceReceipt {
    pub fn validate(&self) -> Result<(), EvaluationAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
        {
            return Err(EvaluationAssuranceError::Contract(
                "evaluation assurance identity mismatch".into(),
            ));
        }
        if self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.run_id.trim().is_empty()
            || self.capability_id.trim().is_empty()
            || self.benchmark_id.trim().is_empty()
            || self.baseline_id.trim().is_empty()
            || self.metric_order.is_empty()
            || self.gate_order.is_empty()
            || self.reasons.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(EvaluationAssuranceError::InvalidRequest("evaluation identity, metrics, gates, reasons, effects, locality, and boundary are required".into()));
        }
        for values in [
            &self.metric_order,
            &self.gate_order,
            &self.witness_order,
            &self.counterexample_order,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(EvaluationAssuranceError::InvalidRequest(
                    "evaluation output ordering is not canonical".into(),
                ));
            }
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| EvaluationAssuranceError::Contract(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, EvaluationAssuranceError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| EvaluationAssuranceError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| EvaluationAssuranceError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum EvaluationAssuranceError {
    #[error("invalid evaluation assurance run: {0}")]
    InvalidRequest(String),
    #[error("evaluation assurance contract rejected: {0}")]
    Contract(String),
    #[error("evaluation assurance serialization failed: {0}")]
    Serialization(String),
}

pub fn assure_evaluation_run(
    run: &CapabilityRun,
) -> Result<EvaluationAssuranceReceipt, EvaluationAssuranceError> {
    validate_run(run)?;
    let mut metrics = run.metrics.clone();
    metrics.sort_by(|left, right| left.metric_id.cmp(&right.metric_id));
    let metric_order = metrics
        .iter()
        .map(|metric| metric.metric_id.clone())
        .collect::<Vec<_>>();
    let mut witnesses = run
        .witnesses
        .iter()
        .map(|witness| witness.witness_id.clone())
        .collect::<Vec<_>>();
    witnesses.sort();
    witnesses.dedup();
    let mut required = run.required_witnesses.clone();
    required.sort();
    required.dedup();
    let mut gate_order = vec![
        "policy_allow".into(),
        "provenance_complete".into(),
        "protected_closure".into(),
        "replay_identity".into(),
        "baseline_delta".into(),
        "witness_coverage".into(),
    ];
    gate_order.sort();
    let mut omissions = run.omissions.clone();
    let mut uncertainty = run.uncertainty.clone();
    let mut counterexamples = BTreeSet::new();
    let witness_set = run
        .witnesses
        .iter()
        .map(|witness| witness.witness_id.as_str())
        .collect::<BTreeSet<_>>();
    for witness in &required {
        if !witness_set.contains(witness.as_str()) {
            counterexamples.insert(format!("missing-witness:{witness}"));
        }
    }
    for metric in &metrics {
        match (metric.observed, metric.baseline) {
            (Some(observed), Some(baseline)) if observed < baseline + metric.minimum_delta => {
                counterexamples.insert(format!("metric-under-baseline:{}", metric.metric_id));
            }
            (Some(_), Some(_)) => {}
            _ => {
                omissions.push(format!("metric-unmeasured:{}", metric.metric_id));
            }
        }
        if metric.uncertainty_width.is_none() {
            uncertainty.push(format!(
                "metric-uncertainty-unreported:{}",
                metric.metric_id
            ));
        }
    }
    let mut reasons = Vec::new();
    let verdict = if !run.policy_allow || !run.provenance_complete {
        reasons.push("policy or provenance gate denied release evidence".into());
        AssuranceVerdict::Blocked
    } else if !run.protected_closure {
        uncertainty.push("protected closure is incomplete".into());
        reasons.push("protected closure is required before a release verdict".into());
        AssuranceVerdict::Unknown
    } else if !counterexamples.is_empty() {
        reasons.push("one or more baseline, witness, or measurement gates failed".into());
        AssuranceVerdict::Blocked
    } else if !omissions.is_empty() || !uncertainty.is_empty() {
        reasons.push(
            "accepted metrics retain omissions or uncertainty and require conditional review"
                .into(),
        );
        AssuranceVerdict::Conditional
    } else {
        reasons.push("all declared evaluation gates passed with witness coverage".into());
        AssuranceVerdict::Passed
    };
    let counterexample_order = counterexamples.into_iter().collect::<Vec<_>>();
    let effect_receipts = if matches!(verdict, AssuranceVerdict::Passed) {
        vec!["release-evidence:eligible-after-independent-review".into()]
    } else {
        vec![format!("block:unsafe-release:{:?}", verdict).to_lowercase()]
    };
    let mut negative_evidence = run.negative_evidence.clone();
    negative_evidence.sort();
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "run_id": run.run_id, "capability_id": run.capability_id, "benchmark_id": run.benchmark_id, "baseline_id": run.baseline_id, "verdict": verdict, "metric_order": metric_order, "gate_order": gate_order, "witness_order": witnesses, "counterexample_order": counterexample_order, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative_evidence, "reasons": reasons, "effect_receipts": effect_receipts, "replay_identity": run.replay_identity, "raw_data_local": true, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("evaluation-assurance:{}", run.run_id),
        "application/vnd.aurora.evaluation-assurance+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| EvaluationAssuranceError::Contract(error.to_string()))?;
    let receipt = EvaluationAssuranceReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        run_id: run.run_id.clone(),
        capability_id: run.capability_id.clone(),
        benchmark_id: run.benchmark_id.clone(),
        baseline_id: run.baseline_id.clone(),
        verdict,
        metric_order,
        gate_order,
        witness_order: witnesses,
        counterexample_order,
        omissions,
        uncertainty,
        negative_evidence,
        reasons,
        effect_receipts,
        replay_identity: run.replay_identity.clone(),
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_run(run: &CapabilityRun) -> Result<(), EvaluationAssuranceError> {
    if run.run_id.trim().is_empty()
        || run.capability_id.trim().is_empty()
        || run.benchmark_id.trim().is_empty()
        || run.baseline_id.trim().is_empty()
        || run.metrics.is_empty()
        || !run.raw_data_local
        || run.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(EvaluationAssuranceError::InvalidRequest(
            "run identity, metrics, locality, and boundary are required".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for metric in &run.metrics {
        if metric.metric_id.trim().is_empty()
            || !ids.insert(metric.metric_id.clone())
            || !metric.minimum_delta.is_finite()
            || metric.minimum_delta < 0.0
            || metric.observed.is_some_and(|value| !value.is_finite())
            || metric.baseline.is_some_and(|value| !value.is_finite())
            || metric
                .uncertainty_width
                .is_some_and(|value| !value.is_finite() || value < 0.0)
        {
            return Err(EvaluationAssuranceError::InvalidRequest(format!(
                "metric {} is invalid or duplicated",
                metric.metric_id
            )));
        }
    }
    if run
        .required_witnesses
        .iter()
        .any(|witness| witness.trim().is_empty())
        || run.witnesses.iter().any(|witness| {
            witness.witness_id.trim().is_empty()
                || witness.kind.trim().is_empty()
                || witness.detail.trim().is_empty()
        })
    {
        return Err(EvaluationAssuranceError::InvalidRequest(
            "witness identifiers and details cannot be empty".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn run() -> CapabilityRun {
        CapabilityRun {
            run_id: "run:qc".into(),
            capability_id: "capability:qc".into(),
            benchmark_id: "benchmark:heldout".into(),
            baseline_id: "baseline:v1".into(),
            metrics: vec![MetricObservation {
                metric_id: "adr".into(),
                observed: Some(0.92),
                baseline: Some(0.80),
                minimum_delta: 0.05,
                uncertainty_width: Some(0.03),
                witness_id: Some("w:adr".into()),
            }],
            required_witnesses: vec!["w:adr".into()],
            witnesses: vec![AssuranceWitness {
                witness_id: "w:adr".into(),
                kind: "replay".into(),
                detail: "held-out fixture replay".into(),
            }],
            omissions: Vec::new(),
            uncertainty: Vec::new(),
            negative_evidence: vec!["null secondary metric".into()],
            policy_allow: true,
            provenance_complete: true,
            protected_closure: true,
            replay_identity: ContentHash::of_bytes(b"replay"),
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn passed_run_requires_witness_and_baseline() {
        let receipt = assure_evaluation_run(&run()).unwrap();
        assert_eq!(receipt.verdict, AssuranceVerdict::Passed);
        assert!(!receipt.negative_evidence.is_empty());
    }
    #[test]
    fn missing_witness_blocks() {
        let mut run = run();
        run.witnesses.clear();
        let receipt = assure_evaluation_run(&run).unwrap();
        assert_eq!(receipt.verdict, AssuranceVerdict::Blocked);
    }
    #[test]
    fn under_baseline_is_counterexample() {
        let mut run = run();
        run.metrics[0].observed = Some(0.81);
        let receipt = assure_evaluation_run(&run).unwrap();
        assert_eq!(receipt.verdict, AssuranceVerdict::Blocked);
        assert!(!receipt.counterexample_order.is_empty());
    }
    #[test]
    fn incomplete_closure_is_unknown() {
        let mut run = run();
        run.protected_closure = false;
        let receipt = assure_evaluation_run(&run).unwrap();
        assert_eq!(receipt.verdict, AssuranceVerdict::Unknown);
    }
    #[test]
    fn output_digest_is_deterministic() {
        let first = assure_evaluation_run(&run()).unwrap();
        let second = assure_evaluation_run(&run()).unwrap();
        assert_eq!(first.digest().unwrap(), second.digest().unwrap());
    }
}
