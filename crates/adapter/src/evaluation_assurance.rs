//! Local evaluation and observability assurance harness.
//!
//! Atlas feature: `AFA-adapter-P23-F25`.
//!
//! The harness turns capability-run metrics, baseline comparisons, replay identity, and
//! witness-bearing checks into a release receipt. It does not execute a benchmark or infer
//! biology. Missing protected closure, provenance, witnesses, or metric measurements remain
//! unknown/blocked states; negative results remain first-class evidence.

use bioprism_foundation::{
    ProvenanceLink, TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P23-F25";
pub const CONTRACT_VERSION: &str = "evaluation-assurance-harness/1.0";
const MAX_TEXT_BYTES: usize = 512;
const MAX_ITEMS: usize = 16384;
const EXPECTED_GATES: [&str; 6] = [
    "baseline_delta",
    "policy_allow",
    "protected_closure",
    "provenance_complete",
    "replay_identity",
    "witness_coverage",
];

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
    pub input: CapabilityRun,
    pub input_digest: ContentHash,
    pub run_id: String,
    pub capability_id: String,
    pub benchmark_id: String,
    pub baseline_id: String,
    pub metrics: Vec<MetricObservation>,
    pub required_witnesses: Vec<String>,
    pub witnesses: Vec<AssuranceWitness>,
    pub input_omissions: Vec<String>,
    pub input_uncertainty: Vec<String>,
    pub input_negative_evidence: Vec<String>,
    pub policy_allow: bool,
    pub provenance_complete: bool,
    pub protected_closure: bool,
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
        validate_text("run_id", &self.run_id)?;
        validate_text("capability_id", &self.capability_id)?;
        validate_text("benchmark_id", &self.benchmark_id)?;
        validate_text("baseline_id", &self.baseline_id)?;
        validate_text("boundary", &self.boundary)?;
        validate_sorted_strings("metric_order", &self.metric_order)?;
        validate_sorted_strings("gate_order", &self.gate_order)?;
        validate_sorted_strings("witness_order", &self.witness_order)?;
        validate_sorted_strings("counterexample_order", &self.counterexample_order)?;
        validate_sorted_strings("omissions", &self.omissions)?;
        validate_sorted_strings("uncertainty", &self.uncertainty)?;
        validate_sorted_strings("negative_evidence", &self.negative_evidence)?;
        validate_sorted_strings("reasons", &self.reasons)?;
        validate_sorted_strings("effect_receipts", &self.effect_receipts)?;
        if self
            .gate_order
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            != EXPECTED_GATES
        {
            return Err(EvaluationAssuranceError::InvalidRequest(
                "evaluation gate order does not match the contract".into(),
            ));
        }
        if self.replay_identity == ContentHash::of_bytes(b"") {
            return Err(EvaluationAssuranceError::InvalidRequest(
                "evaluation replay identity is required".into(),
            ));
        }
        let expected_effect = if self.verdict == AssuranceVerdict::Passed {
            "release-evidence:eligible-after-independent-review".to_string()
        } else {
            format!("block:unsafe-release:{:?}", self.verdict).to_lowercase()
        };
        if self.effect_receipts != [expected_effect] {
            return Err(EvaluationAssuranceError::InvalidRequest(
                "evaluation effect receipt does not match its verdict".into(),
            ));
        }
        if self.artifact.artifact_id != format!("evaluation-assurance:{}", self.run_id)
            || self.artifact.content_type != "application/vnd.aurora.evaluation-assurance+json"
            || !self.artifact.semantic_loss.is_empty()
            || self.artifact.provenance
                != evaluation_provenance(&self.run_id, &self.replay_identity)
        {
            return Err(EvaluationAssuranceError::Contract(
                "evaluation artifact is not bound to the receipt".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| EvaluationAssuranceError::Contract(error.to_string()))?;
        self.artifact
            .verify_payload(&evaluation_payload(self))
            .map_err(|error| EvaluationAssuranceError::Contract(error.to_string()))?;
        validate_run(&self.input)?;
        if self.input_digest != evaluation_input_digest(&self.input)? {
            return Err(EvaluationAssuranceError::Contract(
                "evaluation retained input digest does not match the run".into(),
            ));
        }
        let expected = assure_evaluation_run_internal(&self.input, false)?;
        if self != &expected {
            return Err(EvaluationAssuranceError::Contract(
                "evaluation receipt is not derived from its retained run inputs".into(),
            ));
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, EvaluationAssuranceError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| EvaluationAssuranceError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| EvaluationAssuranceError::Serialization(error.to_string()))
    }
}

fn validate_text(field: &str, value: &str) -> Result<(), EvaluationAssuranceError> {
    if value.is_empty() || value.trim() != value {
        return Err(EvaluationAssuranceError::InvalidRequest(format!(
            "{field} must be non-empty and trimmed"
        )));
    }
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(EvaluationAssuranceError::InvalidRequest(format!(
            "{field} is outside its bounded text contract"
        )));
    }
    Ok(())
}

fn evaluation_input_digest(run: &CapabilityRun) -> Result<ContentHash, EvaluationAssuranceError> {
    let value = serde_json::to_value(&canonical_evaluation_run(run))
        .map_err(|error| EvaluationAssuranceError::Serialization(error.to_string()))?;
    ContentHash::of_value(&value)
        .map_err(|error| EvaluationAssuranceError::Serialization(error.to_string()))
}

fn canonical_evaluation_run(run: &CapabilityRun) -> CapabilityRun {
    let mut canonical = run.clone();
    canonical
        .metrics
        .sort_by(|left, right| left.metric_id.cmp(&right.metric_id));
    canonical.required_witnesses.sort();
    canonical
        .witnesses
        .sort_by(|left, right| left.witness_id.cmp(&right.witness_id));
    canonical.omissions.sort();
    canonical.uncertainty.sort();
    canonical.negative_evidence.sort();
    canonical
}

fn validate_unique_strings(field: &str, values: &[String]) -> Result<(), EvaluationAssuranceError> {
    if values.len() > MAX_ITEMS {
        return Err(EvaluationAssuranceError::InvalidRequest(format!(
            "{field} exceeds its item bound"
        )));
    }
    let mut unique = BTreeSet::new();
    for value in values {
        validate_text(field, value)?;
        if !unique.insert(value) {
            return Err(EvaluationAssuranceError::InvalidRequest(format!(
                "{field} contains duplicate values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_strings(field: &str, values: &[String]) -> Result<(), EvaluationAssuranceError> {
    validate_unique_strings(field, values)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(EvaluationAssuranceError::InvalidRequest(format!(
            "{field} ordering is not canonical"
        )));
    }
    Ok(())
}

fn evaluation_provenance(run_id: &str, replay_identity: &ContentHash) -> Vec<ProvenanceLink> {
    vec![ProvenanceLink {
        source_id: run_id.into(),
        relation: "evaluation-run-replay-identity".into(),
        digest: replay_identity.clone(),
    }]
}

fn evaluation_payload(receipt: &EvaluationAssuranceReceipt) -> serde_json::Value {
    evaluation_payload_from_parts(
        &receipt.schema_version,
        &receipt.contract_version,
        &receipt.feature_id,
        &receipt.run_id,
        &receipt.capability_id,
        &receipt.benchmark_id,
        &receipt.baseline_id,
        &receipt.metrics,
        &receipt.required_witnesses,
        &receipt.witnesses,
        &receipt.input_omissions,
        &receipt.input_uncertainty,
        &receipt.input_negative_evidence,
        receipt.policy_allow,
        receipt.provenance_complete,
        receipt.protected_closure,
        receipt.verdict,
        &receipt.metric_order,
        &receipt.gate_order,
        &receipt.witness_order,
        &receipt.counterexample_order,
        &receipt.omissions,
        &receipt.uncertainty,
        &receipt.negative_evidence,
        &receipt.reasons,
        &receipt.effect_receipts,
        &receipt.replay_identity,
        &receipt.artifact.provenance,
        receipt.raw_data_local,
        &receipt.boundary,
    )
}

#[allow(clippy::too_many_arguments)]
fn evaluation_payload_from_parts(
    schema_version: &str,
    contract_version: &str,
    feature_id: &str,
    run_id: &str,
    capability_id: &str,
    benchmark_id: &str,
    baseline_id: &str,
    metrics: &[MetricObservation],
    required_witnesses: &[String],
    witnesses: &[AssuranceWitness],
    input_omissions: &[String],
    input_uncertainty: &[String],
    input_negative_evidence: &[String],
    policy_allow: bool,
    provenance_complete: bool,
    protected_closure: bool,
    verdict: AssuranceVerdict,
    metric_order: &[String],
    gate_order: &[String],
    witness_order: &[String],
    counterexample_order: &[String],
    omissions: &[String],
    uncertainty: &[String],
    negative_evidence: &[String],
    reasons: &[String],
    effect_receipts: &[String],
    replay_identity: &ContentHash,
    provenance: &[ProvenanceLink],
    raw_data_local: bool,
    boundary: &str,
) -> serde_json::Value {
    json!({
        "schema_version": schema_version,
        "contract_version": contract_version,
        "feature_id": feature_id,
        "run_id": run_id,
        "capability_id": capability_id,
        "benchmark_id": benchmark_id,
        "baseline_id": baseline_id,
        "metrics": metrics,
        "required_witnesses": required_witnesses,
        "witnesses": witnesses,
        "input_omissions": input_omissions,
        "input_uncertainty": input_uncertainty,
        "input_negative_evidence": input_negative_evidence,
        "policy_allow": policy_allow,
        "provenance_complete": provenance_complete,
        "protected_closure": protected_closure,
        "verdict": verdict,
        "metric_order": metric_order,
        "gate_order": gate_order,
        "witness_order": witness_order,
        "counterexample_order": counterexample_order,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": negative_evidence,
        "reasons": reasons,
        "effect_receipts": effect_receipts,
        "replay_identity": replay_identity,
        "provenance": provenance,
        "raw_data_local": raw_data_local,
        "boundary": boundary,
    })
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
    assure_evaluation_run_internal(run, true)
}

fn assure_evaluation_run_internal(
    run: &CapabilityRun,
    validate_output: bool,
) -> Result<EvaluationAssuranceReceipt, EvaluationAssuranceError> {
    validate_run(run)?;
    let mut metrics = run.metrics.clone();
    metrics.sort_by(|left, right| left.metric_id.cmp(&right.metric_id));
    let metric_order = metrics
        .iter()
        .map(|metric| metric.metric_id.clone())
        .collect::<Vec<_>>();
    let mut witness_records = run.witnesses.clone();
    witness_records.sort_by(|left, right| left.witness_id.cmp(&right.witness_id));
    let witnesses = witness_records
        .iter()
        .map(|witness| witness.witness_id.clone())
        .collect::<Vec<_>>();
    let mut required = run.required_witnesses.clone();
    required.sort();
    required.dedup();
    let mut input_omissions = run.omissions.clone();
    input_omissions.sort();
    input_omissions.dedup();
    let mut input_uncertainty = run.uncertainty.clone();
    input_uncertainty.sort();
    input_uncertainty.dedup();
    let mut input_negative_evidence = run.negative_evidence.clone();
    input_negative_evidence.sort();
    input_negative_evidence.dedup();
    let gate_order = EXPECTED_GATES
        .iter()
        .map(|gate| (*gate).to_string())
        .collect::<Vec<_>>();
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
        if let Some(witness_id) = &metric.witness_id {
            if !witness_set.contains(witness_id.as_str()) {
                counterexamples.insert(format!("missing-metric-witness:{witness_id}"));
            }
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
    omissions.sort();
    omissions.dedup();
    uncertainty.sort();
    uncertainty.dedup();
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
    negative_evidence.dedup();
    reasons.sort();
    reasons.dedup();
    let provenance = evaluation_provenance(&run.run_id, &run.replay_identity);
    let payload = evaluation_payload_from_parts(
        RESEARCH_CONTRACT_SCHEMA_VERSION,
        CONTRACT_VERSION,
        FEATURE_ID,
        &run.run_id,
        &run.capability_id,
        &run.benchmark_id,
        &run.baseline_id,
        &metrics,
        &required,
        &witness_records,
        &input_omissions,
        &input_uncertainty,
        &input_negative_evidence,
        run.policy_allow,
        run.provenance_complete,
        run.protected_closure,
        verdict,
        &metric_order,
        &gate_order,
        &witnesses,
        &counterexample_order,
        &omissions,
        &uncertainty,
        &negative_evidence,
        &reasons,
        &effect_receipts,
        &run.replay_identity,
        &provenance,
        true,
        PRECLINICAL_BOUNDARY,
    );
    let artifact = TypedResearchArtifact::from_payload(
        format!("evaluation-assurance:{}", run.run_id),
        "application/vnd.aurora.evaluation-assurance+json",
        &payload,
        Vec::new(),
        provenance,
    )
    .map_err(|error| EvaluationAssuranceError::Contract(error.to_string()))?;
    let receipt = EvaluationAssuranceReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        input: canonical_evaluation_run(run),
        input_digest: evaluation_input_digest(run)?,
        run_id: run.run_id.clone(),
        capability_id: run.capability_id.clone(),
        benchmark_id: run.benchmark_id.clone(),
        baseline_id: run.baseline_id.clone(),
        metrics,
        required_witnesses: required,
        witnesses: witness_records,
        input_omissions,
        input_uncertainty,
        input_negative_evidence,
        policy_allow: run.policy_allow,
        provenance_complete: run.provenance_complete,
        protected_closure: run.protected_closure,
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
    if validate_output {
        receipt.validate()?;
    }
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
    validate_text("run_id", &run.run_id)?;
    validate_text("capability_id", &run.capability_id)?;
    validate_text("benchmark_id", &run.benchmark_id)?;
    validate_text("baseline_id", &run.baseline_id)?;
    validate_text("boundary", &run.boundary)?;
    if run.metrics.len() > MAX_ITEMS
        || run.required_witnesses.len() > MAX_ITEMS
        || run.witnesses.len() > MAX_ITEMS
    {
        return Err(EvaluationAssuranceError::InvalidRequest(
            "evaluation metric or witness count exceeds its bound".into(),
        ));
    }
    validate_unique_strings("required_witnesses", &run.required_witnesses)?;
    validate_unique_strings("omissions", &run.omissions)?;
    validate_unique_strings("uncertainty", &run.uncertainty)?;
    validate_unique_strings("negative_evidence", &run.negative_evidence)?;
    if run.replay_identity == ContentHash::of_bytes(b"") {
        return Err(EvaluationAssuranceError::InvalidRequest(
            "evaluation replay identity is required".into(),
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
        validate_text("metric_id", &metric.metric_id)?;
        if let Some(witness_id) = &metric.witness_id {
            validate_text("metric.witness_id", witness_id)?;
        }
    }
    let mut witness_ids = BTreeSet::new();
    for witness in &run.witnesses {
        validate_text("witness_id", &witness.witness_id)?;
        validate_text("witness.kind", &witness.kind)?;
        validate_text("witness.detail", &witness.detail)?;
        if !witness_ids.insert(witness.witness_id.as_str()) {
            return Err(EvaluationAssuranceError::InvalidRequest(
                "witness identifiers must be unique".into(),
            ));
        }
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

    #[test]
    fn undeclared_metric_witness_is_rejected() {
        let mut run = run();
        run.metrics[0].witness_id = Some("w:missing".into());
        let receipt = assure_evaluation_run(&run).unwrap();
        assert_eq!(receipt.verdict, AssuranceVerdict::Blocked);
        assert!(receipt
            .counterexample_order
            .contains(&"missing-metric-witness:w:missing".into()));
    }

    #[test]
    fn duplicate_witness_ids_are_rejected() {
        let mut run = run();
        run.witnesses.push(run.witnesses[0].clone());
        let error = assure_evaluation_run(&run).unwrap_err();
        assert!(error
            .to_string()
            .contains("witness identifiers must be unique"));
    }

    #[test]
    fn receipt_rejects_tampered_artifact_payload_binding() {
        let mut receipt = assure_evaluation_run(&run()).unwrap();
        receipt.baseline_id = "tampered-baseline".into();
        let error = receipt.validate().unwrap_err();
        assert!(error.to_string().contains("digest mismatch"));
    }

    #[test]
    fn receipt_rejects_a_verdict_effect_mismatch() {
        let mut receipt = assure_evaluation_run(&run()).unwrap();
        receipt.effect_receipts = vec!["block:unsafe-release:passed".into()];
        let error = receipt.validate().unwrap_err();
        assert!(error.to_string().contains("effect receipt"));
    }

    #[test]
    fn retained_metric_tampering_is_rejected() {
        let mut receipt = assure_evaluation_run(&run()).unwrap();
        receipt.metrics[0].observed = Some(0.81);
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn evaluation_artifact_provenance_tampering_is_rejected() {
        let mut receipt = assure_evaluation_run(&run()).unwrap();
        receipt.artifact.provenance[0].digest = ContentHash::of_bytes(b"tampered");
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn retained_run_tampering_is_rejected() {
        let mut receipt = assure_evaluation_run(&run()).unwrap();
        receipt.input.capability_id = "capability:tampered".into();
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn evaluation_input_order_is_canonicalized() {
        let extra_metric = MetricObservation {
            metric_id: "z-score".into(),
            observed: Some(0.95),
            baseline: Some(0.80),
            minimum_delta: 0.05,
            uncertainty_width: Some(0.02),
            witness_id: Some("w:z-score".into()),
        };
        let extra_witness = AssuranceWitness {
            witness_id: "w:z-score".into(),
            kind: "replay".into(),
            detail: "secondary fixture replay".into(),
        };
        let mut canonical_input = run();
        canonical_input.metrics.push(extra_metric.clone());
        canonical_input.required_witnesses.push("w:z-score".into());
        canonical_input.witnesses.push(extra_witness.clone());
        let mut reordered = canonical_input.clone();
        reordered.metrics.reverse();
        reordered.required_witnesses.reverse();
        reordered.witnesses.reverse();
        let canonical = assure_evaluation_run(&canonical_input).unwrap();
        let reordered = assure_evaluation_run(&reordered).unwrap();
        assert_eq!(canonical.digest().unwrap(), reordered.digest().unwrap());
    }
}
