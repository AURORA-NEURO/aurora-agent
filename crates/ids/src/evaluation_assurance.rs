//! Local evaluation and observability assurance (`AFA-ids-P23-F25`).
//!
//! The harness evaluates typed metric summaries and emits an auditable card;
//! it does not inspect raw studies, fit models, or make clinical decisions.

use crate::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-ids-P23-F25";
pub const CONTRACT_VERSION: &str = "ids-local-evaluation-observability-assurance-harness/1.0";
pub const INPUT_SCHEMA: &str = "CapabilityRun7@1";
pub const OUTPUT_SCHEMA: &str = "EvaluationCard9@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.evaluation-card-9+json";
pub const PRECLINICAL_BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";
pub const MAX_OBSERVATIONS: usize = 16_384;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvaluationEvidenceState {
    Proven,
    Supported,
    Unknown,
    Unmeasured,
    Contradicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityObservation8 {
    pub observation_id: String,
    pub metric_id: String,
    pub benchmark_id: String,
    pub value_milli: i64,
    pub threshold_milli: i64,
    pub baseline_milli: i64,
    pub evidence_state: EvaluationEvidenceState,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub local: bool,
    pub aggregate_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityRun7 {
    pub request_id: String,
    pub study_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub benchmark_id: String,
    pub required_metrics: Vec<String>,
    pub observations: Vec<CapabilityObservation8>,
    pub minimum_pass_fraction_milli: u16,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvaluationCard9Artifact {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvaluationCard9 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub study_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub benchmark_id: String,
    pub disposition: String,
    pub metric_order: Vec<String>,
    pub passed_order: Vec<String>,
    pub failed_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub unmeasured_order: Vec<String>,
    pub contradicted_order: Vec<String>,
    pub omitted_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub baseline_delta_milli: Vec<i64>,
    pub pass_fraction_milli: u16,
    pub replay_identity: ContentHash,
    pub card_digest: ContentHash,
    pub artifact: EvaluationCard9Artifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EvaluationAssuranceError {
    #[error("invalid evaluation run: {0}")]
    Invalid(String),
    #[error("evaluation card failed validation: {0}")]
    Card(String),
}

pub fn evaluation_assurance_manifest() -> serde_json::Value {
    json!({
        "schema_version":"aurora-research-contract/1.0", "capability_id":FEATURE_ID, "version":CONTRACT_VERSION, "owner_crate":"ids",
        "consumers":["evaluation engineer", "research lead", "release auditor", "observability operator"],
        "behavior":"verify typed local metric summaries against a benchmark and emit an omission-aware evaluation card",
        "value":"prevents missing, contradictory, unmeasured, or below-baseline evidence from being reported as a passing capability",
        "input_schema":INPUT_SCHEMA, "output_schema":OUTPUT_SCHEMA, "effects":["measure:evaluation-card","manage:local-capability"],
        "permissions":["read:local-metric-summaries","request:evaluation-assurance"], "autonomy_tier":"A1", "boundary":PRECLINICAL_BOUNDARY
    })
}

fn valid_digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().bytes().all(|b| b.is_ascii_hexdigit())
}
fn ordered(values: &[String]) -> bool {
    values.windows(2).all(|w| w[0] < w[1])
}

impl EvaluationCard9 {
    pub fn validate(&self) -> Result<(), EvaluationAssuranceError> {
        if self.schema_version != "aurora-research-contract/1.0"
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.study_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.benchmark_id.trim().is_empty()
            || self.metric_order.is_empty()
            || self.effect_receipts.is_empty()
            || !["qualified", "unresolved", "blocked"].contains(&self.disposition.as_str())
        {
            return Err(EvaluationAssuranceError::Card(
                "evaluation identity, locality, metrics, disposition, or effects are incomplete"
                    .into(),
            ));
        }
        for values in [
            &self.metric_order,
            &self.passed_order,
            &self.failed_order,
            &self.unknown_order,
            &self.unmeasured_order,
            &self.contradicted_order,
            &self.omitted_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !ordered(values) {
                return Err(EvaluationAssuranceError::Card(
                    "evaluation ordering is not canonical".into(),
                ));
            }
        }
        let ids = BTreeSet::from_iter(self.metric_order.iter().cloned());
        let parts = self
            .passed_order
            .iter()
            .chain(&self.failed_order)
            .chain(&self.unknown_order)
            .chain(&self.unmeasured_order)
            .chain(&self.contradicted_order)
            .chain(&self.omitted_order)
            .cloned()
            .collect::<Vec<_>>();
        if ids.len() != self.metric_order.len()
            || parts.len() != ids.len()
            || BTreeSet::from_iter(parts) != ids
            || self.baseline_delta_milli.len() != self.metric_order.len()
        {
            return Err(EvaluationAssuranceError::Card(
                "metric states or baseline deltas do not partition".into(),
            ));
        }
        if !valid_digest(&self.replay_identity)
            || !valid_digest(&self.card_digest)
            || self.artifact.content_hash != self.card_digest
            || self.artifact.content_type != CONTENT_TYPE
            || self
                .artifact
                .provenance_digests
                .iter()
                .any(|d| !valid_digest(d))
        {
            return Err(EvaluationAssuranceError::Card(
                "evaluation digest or artifact metadata is inconsistent".into(),
            ));
        }
        if self.effect_receipts.iter().any(|e| {
            !e.starts_with("measure:evaluation-card:")
                && !e.starts_with("manage:local-capability:")
                && e != "block:unsafe-release"
        }) {
            return Err(EvaluationAssuranceError::Card(
                "effect is outside governed evaluation gate".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(request: &CapabilityRun7) -> Result<(), EvaluationAssuranceError> {
    if request.request_id.trim().is_empty()
        || request.study_id.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.benchmark_id.trim().is_empty()
        || request.required_metrics.is_empty()
        || request.observations.is_empty()
        || request.observations.len() > MAX_OBSERVATIONS
        || request.minimum_pass_fraction_milli > 1000
        || !valid_digest(&request.replay_identity)
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
    {
        return Err(EvaluationAssuranceError::Invalid("evaluation identity, benchmark, metrics, bound, threshold, replay, or locality is invalid".into()));
    }
    let required = request
        .required_metrics
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if required.len() != request.required_metrics.len() {
        return Err(EvaluationAssuranceError::Invalid(
            "required metrics are not unique".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for o in &request.observations {
        if o.observation_id.trim().is_empty()
            || o.metric_id.trim().is_empty()
            || o.benchmark_id.trim().is_empty()
            || !valid_digest(&o.provenance_digest)
            || !valid_digest(&o.replay_identity)
            || !ids.insert(o.observation_id.clone())
        {
            return Err(EvaluationAssuranceError::Invalid(
                "observation identity, benchmark, digest, or uniqueness is invalid".into(),
            ));
        }
    }
    Ok(())
}

pub fn assure_evaluation(
    request: &CapabilityRun7,
) -> Result<EvaluationCard9, EvaluationAssuranceError> {
    validate_request(request)?;
    let mut observations = request.observations.clone();
    observations.sort_by(|a, b| {
        a.metric_id
            .cmp(&b.metric_id)
            .then(a.observation_id.cmp(&b.observation_id))
    });
    let metric_order = request
        .required_metrics
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let metric_order = metric_order.into_iter().collect::<Vec<_>>();
    let mut passed = BTreeSet::new();
    let mut failed = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut unmeasured = BTreeSet::new();
    let mut contradicted = BTreeSet::new();
    let mut omitted = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut loss = BTreeSet::new();
    let mut deltas = Vec::new();
    let mut prov = BTreeSet::new();
    for metric in &metric_order {
        let rows = observations
            .iter()
            .filter(|o| o.metric_id == *metric)
            .collect::<Vec<_>>();
        if rows.is_empty() {
            omitted.insert(metric.clone());
            loss.insert(format!("{metric}:missing-observation"));
            deltas.push(0);
            continue;
        }
        let o = rows[0];
        prov.insert(o.provenance_digest.clone());
        deltas.push(o.value_milli - o.baseline_milli);
        if o.benchmark_id != request.benchmark_id {
            omitted.insert(metric.clone());
            loss.insert(format!("{metric}:benchmark-mismatch"));
        } else if o.replay_identity != request.replay_identity {
            unknown.insert(metric.clone());
            loss.insert(format!("{metric}:replay-identity"));
        } else {
            match o.evidence_state {
                EvaluationEvidenceState::Contradicted => {
                    contradicted.insert(metric.clone());
                    negative.insert(format!("{metric}:contradicted"));
                }
                EvaluationEvidenceState::Unknown => {
                    unknown.insert(metric.clone());
                    loss.insert(format!("{metric}:unknown"));
                }
                EvaluationEvidenceState::Unmeasured => {
                    unmeasured.insert(metric.clone());
                    loss.insert(format!("{metric}:unmeasured"));
                }
                EvaluationEvidenceState::Proven | EvaluationEvidenceState::Supported => {
                    if o.value_milli >= o.threshold_milli {
                        passed.insert(metric.clone());
                    } else {
                        failed.insert(metric.clone());
                        negative.insert(format!("{metric}:below-threshold"));
                    }
                }
            }
        }
    }
    let fraction = if metric_order.is_empty() {
        0
    } else {
        ((passed.len() * 1000) / metric_order.len()) as u16
    };
    if fraction < request.minimum_pass_fraction_milli {
        loss.insert(format!("request:pass-fraction:{fraction}"));
    }
    let global = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.raw_data_local
        || !request.aggregate_only;
    if global {
        passed.clear();
        failed.clear();
        unknown.clear();
        unmeasured.clear();
        contradicted.clear();
        omitted.extend(metric_order.iter().cloned());
        loss.insert("request:governance-or-locality-denied".into());
    }
    let po = passed.iter().cloned().collect::<Vec<_>>();
    let fo = failed.iter().cloned().collect::<Vec<_>>();
    let uo = unknown.iter().cloned().collect::<Vec<_>>();
    let umo = unmeasured.iter().cloned().collect::<Vec<_>>();
    let co = contradicted.iter().cloned().collect::<Vec<_>>();
    let oo = omitted.iter().cloned().collect::<Vec<_>>();
    let disposition = if global
        || po.is_empty()
            && fo.is_empty()
            && uo.is_empty()
            && umo.is_empty()
            && co.is_empty()
            && oo.is_empty()
    {
        "blocked"
    } else if !fo.is_empty()
        || !uo.is_empty()
        || !umo.is_empty()
        || !co.is_empty()
        || !oo.is_empty()
        || fraction < request.minimum_pass_fraction_milli
    {
        "unresolved"
    } else {
        "qualified"
    };
    if disposition != "qualified" {
        loss.insert("request:evaluation-not-closed".into());
    }
    let mut payload = json!({"schema_version":"aurora-research-contract/1.0","contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"study_id":request.study_id,"purpose":request.purpose,"semantic_profile":request.semantic_profile,"benchmark_id":request.benchmark_id,"disposition":disposition,"metric_order":metric_order,"passed_order":po,"failed_order":fo,"unknown_order":uo,"unmeasured_order":umo,"contradicted_order":co,"omitted_order":oo,"negative_evidence_order":negative.iter().cloned().collect::<Vec<_>>(),"baseline_delta_milli":deltas,"pass_fraction_milli":fraction,"replay_identity":request.replay_identity,"raw_data_local":true,"aggregate_only":true,"boundary":PRECLINICAL_BOUNDARY});
    let digest = ContentHash::of_value(&payload)
        .map_err(|e| EvaluationAssuranceError::Card(e.to_string()))?;
    payload["card_digest"] = json!(digest);
    payload["artifact"] = json!({"artifact_id":format!("evaluation-card-9:{}",request.study_id),"content_type":CONTENT_TYPE,"content_hash":digest,"semantic_loss":loss.iter().cloned().collect::<Vec<_>>(),"provenance_digests":prov.into_iter().collect::<Vec<_>>(),"boundary":PRECLINICAL_BOUNDARY});
    payload["effect_receipts"] = json!(if disposition == "qualified" {
        vec![
            format!("measure:evaluation-card:{}", request.request_id),
            format!("manage:local-capability:{}", request.request_id),
        ]
    } else {
        vec!["block:unsafe-release".to_string()]
    });
    let card: EvaluationCard9 = serde_json::from_value(payload)
        .map_err(|e| EvaluationAssuranceError::Card(e.to_string()))?;
    card.validate()?;
    Ok(card)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn h(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn obs(id: &str) -> CapabilityObservation8 {
        CapabilityObservation8 {
            observation_id: id.into(),
            metric_id: id.into(),
            benchmark_id: "bench".into(),
            value_milli: 900,
            threshold_milli: 800,
            baseline_milli: 700,
            evidence_state: EvaluationEvidenceState::Supported,
            provenance_digest: h("p"),
            replay_identity: h("r"),
            local: true,
            aggregate_only: true,
        }
    }
    fn req(os: Vec<CapabilityObservation8>) -> CapabilityRun7 {
        CapabilityRun7 {
            request_id: "eval:req".into(),
            study_id: "study:1".into(),
            purpose: "research".into(),
            semantic_profile: "ome".into(),
            benchmark_id: "bench".into(),
            required_metrics: os.iter().map(|o| o.metric_id.clone()).collect(),
            observations: os,
            minimum_pass_fraction_milli: 1000,
            replay_identity: h("r"),
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            aggregate_only: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(evaluation_assurance_manifest()["autonomy_tier"], "A1")
    }
    #[test]
    fn nominal_is_qualified() {
        assert_eq!(
            assure_evaluation(&req(vec![obs("m")])).unwrap().disposition,
            "qualified"
        )
    }
    #[test]
    fn below_threshold_is_unresolved() {
        let mut o = obs("m");
        o.value_milli = 100;
        assert_eq!(
            assure_evaluation(&req(vec![o])).unwrap().disposition,
            "unresolved"
        )
    }
    #[test]
    fn unknown_is_unresolved() {
        let mut o = obs("m");
        o.evidence_state = EvaluationEvidenceState::Unknown;
        assert_eq!(
            assure_evaluation(&req(vec![o])).unwrap().disposition,
            "unresolved"
        )
    }
    #[test]
    fn missing_metric_is_blocked() {
        let mut q = req(vec![obs("m")]);
        q.required_metrics.push("missing".into());
        assert_eq!(assure_evaluation(&q).unwrap().disposition, "unresolved")
    }
    #[test]
    fn policy_denial_blocks() {
        let mut q = req(vec![obs("m")]);
        q.policy_allow = false;
        assert_eq!(
            assure_evaluation(&q).unwrap().effect_receipts,
            vec!["block:unsafe-release"]
        )
    }
    #[test]
    fn metric_order_is_canonical() {
        let mut a = obs("z");
        let b = obs("a");
        a.metric_id = "z".into();
        let r = assure_evaluation(&req(vec![a, b])).unwrap();
        assert_eq!(r.metric_order, vec!["a", "z"])
    }
}
