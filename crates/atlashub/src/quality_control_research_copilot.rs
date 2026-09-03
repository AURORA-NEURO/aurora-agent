//! Prospective high-throughput quality-control research copilot (`AFA-atlashub-P07-F11`).
//! It evaluates caller-supplied quality metrics and emits a bounded, read-only verdict.
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-atlashub-P07-F11";
pub const CONTRACT_VERSION: &str = "atlashub-prospective-quality-control-research-copilot/1.0";
pub const INPUT_SCHEMA: &str = "ResearchObject3@1";
pub const OUTPUT_SCHEMA: &str = "QualityVerdict3@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.atlashub-quality-verdict-3+json";
pub const PRECLINICAL_BOUNDARY:&str="preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityEvidenceState {
    Proven,
    Supported,
    Unknown,
    Unmeasured,
    Contradicted,
    Negative,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QualityMetric3 {
    pub metric_id: String,
    pub value: Option<f64>,
    pub threshold: Option<f64>,
    pub evidence_state: QualityEvidenceState,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_result: bool,
    pub policy_allowed: bool,
    pub local: bool,
    pub aggregate_only: bool,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResearchObject3 {
    pub object_id: String,
    pub semantic_profile: String,
    pub modality_order: Vec<String>,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub metrics: Vec<QualityMetric3>,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QualityControlRequest3 {
    pub schema_version: String,
    pub request_id: String,
    pub researcher: String,
    pub purpose: String,
    pub required_metric_order: Vec<String>,
    pub required_modality_order: Vec<String>,
    pub object: ResearchObject3,
    pub replay_identity: ContentHash,
    pub policy_allowed: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub tool_approval: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub adversarial_event_order: Vec<String>,
    pub boundary: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QualityVerdictArtifact3 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QualityVerdict3 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub object_id: String,
    pub semantic_profile: String,
    pub disposition: String,
    pub metric_order: Vec<String>,
    pub passed_order: Vec<String>,
    pub failed_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub unmeasured_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub missing_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub missing_modality_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub verdict_digest: ContentHash,
    pub artifact: QualityVerdictArtifact3,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum QualityControlError {
    #[error("invalid quality-control request: {0}")]
    Invalid(String),
    #[error("quality verdict failed validation: {0}")]
    Output(String),
}
fn ordered(v: &[String]) -> bool {
    v.windows(2).all(|p| p[0] < p[1])
}
fn hash(v: &ContentHash) -> bool {
    v.as_str().len() == 64 && v.as_str().bytes().all(|b| b.is_ascii_hexdigit())
}
fn nonempty(v: &str) -> bool {
    !v.trim().is_empty()
}
pub fn quality_control_research_copilot_manifest() -> Value {
    json!({"schema_version":"aurora-research-contract/1.0","capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"atlashub","consumers":["AURORA extension developer","quality engineer","research workbench operator"],"behavior":"qualify prospective high-throughput preclinical quality envelopes with witness-bearing metric verdicts","value":"makes pass, fail, unknown, unmeasured, omission, negative, and policy states explicit before bounded tool use","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["invoke:declared-tools","block:unsafe-release"],"permissions":["invoke:declared-tools"],"autonomy_tier":"A2","boundary":PRECLINICAL_BOUNDARY})
}
fn validate_request(r: &QualityControlRequest3) -> Result<(), QualityControlError> {
    let invalid = r.schema_version != INPUT_SCHEMA
        || [&r.request_id, &r.researcher, &r.purpose]
            .iter()
            .any(|value| value.trim().is_empty())
        || r.required_metric_order.is_empty()
        || !ordered(&r.required_metric_order)
        || !ordered(&r.required_modality_order)
        || !hash(&r.replay_identity)
        || r.boundary != PRECLINICAL_BOUNDARY
        || !r.raw_data_local
        || !r.aggregate_only
        || !ordered(&r.adversarial_event_order)
        || r.object.object_id.trim().is_empty()
        || r.object.semantic_profile.trim().is_empty()
        || !ordered(&r.object.modality_order)
        || !hash(&r.object.provenance_digest)
        || r.object.replay_identity != r.replay_identity;
    if invalid {
        return Err(QualityControlError::Invalid(
            "request identity, metric/modality closure, replay, provenance, locality, or boundary is invalid".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for metric in &r.object.metrics {
        if !nonempty(&metric.metric_id)
            || !ids.insert(metric.metric_id.clone())
            || !hash(&metric.provenance_digest)
            || !hash(&metric.replay_identity)
            || !ordered(&metric.omission_order)
            || !ordered(&metric.uncertainty_order)
        {
            return Err(QualityControlError::Invalid(
                "metric identity, digest, or ordering is invalid".into(),
            ));
        }
    }
    Ok(())
}
impl QualityVerdict3 {
    pub fn validate(&self) -> Result<(), QualityControlError> {
        if self.schema_version != "aurora-research-contract/1.0"
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.content_type != CONTENT_TYPE
            || !self.raw_data_local
            || !self.aggregate_only
            || !["qualified", "unresolved", "blocked"].contains(&self.disposition.as_str())
            || !nonempty(&self.request_id)
            || !nonempty(&self.object_id)
            || !nonempty(&self.semantic_profile)
            || self.metric_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(QualityControlError::Output(
                "quality identity, locality, metrics, disposition, or effects are incomplete"
                    .into(),
            ));
        }
        for v in [
            &self.metric_order,
            &self.passed_order,
            &self.failed_order,
            &self.unknown_order,
            &self.unmeasured_order,
            &self.blocked_order,
            &self.missing_order,
            &self.modality_order,
            &self.missing_modality_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !ordered(v) {
                return Err(QualityControlError::Output(
                    "quality ordering is not canonical".into(),
                ));
            }
        }
        let ids = self.metric_order.iter().cloned().collect::<BTreeSet<_>>();
        let parts = self
            .passed_order
            .iter()
            .chain(&self.failed_order)
            .chain(&self.unknown_order)
            .chain(&self.unmeasured_order)
            .chain(&self.blocked_order)
            .chain(&self.missing_order)
            .cloned()
            .collect::<Vec<_>>();
        if ids.len() != self.metric_order.len()
            || parts.len() != ids.len()
            || BTreeSet::from_iter(parts) != ids
        {
            return Err(QualityControlError::Output(
                "quality states do not partition metrics".into(),
            ));
        }
        if !hash(&self.replay_identity)
            || !hash(&self.verdict_digest)
            || self.artifact.content_hash != self.verdict_digest
            || self.artifact.provenance_digests.iter().any(|d| !hash(d))
        {
            return Err(QualityControlError::Output(
                "quality digest or artifact metadata is invalid".into(),
            ));
        }
        if self
            .effect_receipts
            .iter()
            .any(|e| e != "block:unsafe-release" && !e.starts_with("invoke:declared-tools:"))
        {
            return Err(QualityControlError::Output(
                "quality effect is outside bounded-tool gate".into(),
            ));
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<ContentHash, QualityControlError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self).map_err(|e| QualityControlError::Output(e.to_string()))?,
        )
        .map_err(|e| QualityControlError::Output(e.to_string()))
    }
}
pub fn qualify_quality_control(
    r: &QualityControlRequest3,
) -> Result<QualityVerdict3, QualityControlError> {
    validate_request(r)?;
    let mut rows = r.object.metrics.clone();
    rows.sort_by(|a, b| a.metric_id.cmp(&b.metric_id));
    let mut metric = BTreeSet::new();
    let mut passed = BTreeSet::new();
    let mut failed = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut unmeasured = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut missing = BTreeSet::new();
    let mut omission = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut prov = BTreeSet::new();
    for m in &rows {
        let id = m.metric_id.clone();
        metric.insert(id.clone());
        prov.insert(m.provenance_digest.clone());
        omission.extend(m.omission_order.iter().map(|x| format!("{id}:{x}")));
        uncertainty.extend(m.uncertainty_order.iter().map(|x| format!("{id}:{x}")));
        if m.negative_result || m.evidence_state == QualityEvidenceState::Negative {
            negative.insert(format!("{id}:negative-result"));
        }
        if !m.policy_allowed
            || !m.local
            || !m.aggregate_only
            || m.replay_identity != r.replay_identity
        {
            blocked.insert(id.clone());
            omission.insert(format!("{id}:policy-locality-or-replay"));
        } else if r.required_metric_order.contains(&id)
            && !r
                .required_modality_order
                .iter()
                .all(|x| r.object.modality_order.contains(x))
        {
            missing.insert(id.clone());
        } else if m.evidence_state == QualityEvidenceState::Contradicted {
            failed.insert(id.clone());
        } else if m.evidence_state == QualityEvidenceState::Unknown {
            unknown.insert(id.clone());
            uncertainty.insert(format!("{id}:unknown-evidence"));
        } else if m.evidence_state == QualityEvidenceState::Unmeasured || m.value.is_none() {
            unmeasured.insert(id.clone());
            omission.insert(format!("{id}:unmeasured"));
        } else if m.threshold.is_some_and(|t| m.value.unwrap_or(f64::NAN) < t) {
            failed.insert(id.clone());
        } else if !matches!(
            m.evidence_state,
            QualityEvidenceState::Proven | QualityEvidenceState::Supported
        ) {
            unknown.insert(id.clone());
        } else {
            passed.insert(id.clone());
        }
    }
    for id in &r.required_metric_order {
        if !metric.contains(id) {
            missing.insert(id.clone());
            omission.insert(format!("missing:{id}"));
        }
    }
    let global = !r.policy_allowed
        || !r.protected_closure
        || !r.signed_approval
        || !r.tool_approval
        || !r.raw_data_local
        || !r.aggregate_only
        || !r.adversarial_event_order.is_empty();
    if global {
        blocked.extend(metric.iter().cloned());
        passed.clear();
        failed.clear();
        unknown.clear();
        unmeasured.clear();
        missing.clear();
        omission.insert("request:governance-or-tool-approval-blocked".into());
    }
    uncertainty.extend(
        r.adversarial_event_order
            .iter()
            .map(|e| format!("adversarial:{e}")),
    );
    let mo = metric.iter().cloned().collect::<Vec<_>>();
    let po = passed.iter().cloned().collect::<Vec<_>>();
    let fo = failed.iter().cloned().collect::<Vec<_>>();
    let uo = unknown.iter().cloned().collect::<Vec<_>>();
    let um = unmeasured.iter().cloned().collect::<Vec<_>>();
    let bo = blocked.iter().cloned().collect::<Vec<_>>();
    let mi = missing.iter().cloned().collect::<Vec<_>>();
    let disposition = if global || (!bo.is_empty() && po.is_empty()) {
        "blocked"
    } else if !fo.is_empty() || !uo.is_empty() || !um.is_empty() || !bo.is_empty() || !mi.is_empty()
    {
        "unresolved"
    } else {
        "qualified"
    };
    if disposition != "qualified" {
        omission.insert("request:quality-closure-not-ready".into());
    }
    let mut payload = json!({"schema_version":"aurora-research-contract/1.0","contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":r.request_id,"object_id":r.object.object_id,"semantic_profile":r.object.semantic_profile,"disposition":disposition,"metric_order":mo,"passed_order":po,"failed_order":fo,"unknown_order":uo,"unmeasured_order":um,"blocked_order":bo,"missing_order":mi,"modality_order":r.object.modality_order,"missing_modality_order":if r.required_modality_order.iter().all(|x|r.object.modality_order.contains(x)){Vec::<String>::new()}else{r.required_modality_order.clone()},"omission_order":omission.iter().cloned().collect::<Vec<_>>(),"uncertainty_order":uncertainty.iter().cloned().collect::<Vec<_>>(),"negative_evidence_order":negative.iter().cloned().collect::<Vec<_>>(),"replay_identity":r.replay_identity,"raw_data_local":true,"aggregate_only":true,"boundary":PRECLINICAL_BOUNDARY});
    let digest =
        ContentHash::of_value(&payload).map_err(|e| QualityControlError::Output(e.to_string()))?;
    payload["verdict_digest"] = json!(digest);
    payload["artifact"] = json!({"artifact_id":format!("quality-verdict-3:{}",r.request_id),"content_type":CONTENT_TYPE,"content_hash":digest,"semantic_loss":payload["omission_order"],"provenance_digests":prov.iter().cloned().collect::<Vec<_>>(),"boundary":PRECLINICAL_BOUNDARY});
    payload["effect_receipts"] = json!(if disposition == "qualified" {
        vec![format!("invoke:declared-tools:{}", r.request_id)]
    } else {
        vec!["block:unsafe-release".to_string()]
    });
    let out: QualityVerdict3 =
        serde_json::from_value(payload).map_err(|e| QualityControlError::Output(e.to_string()))?;
    out.validate()?;
    Ok(out)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn h(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn metric(id: &str) -> QualityMetric3 {
        QualityMetric3 {
            metric_id: id.into(),
            value: Some(1.0),
            threshold: Some(0.5),
            evidence_state: QualityEvidenceState::Supported,
            provenance_digest: h("p"),
            replay_identity: h("r"),
            omission_order: vec![],
            uncertainty_order: vec![],
            negative_result: false,
            policy_allowed: true,
            local: true,
            aggregate_only: true,
        }
    }
    fn req(ms: Vec<QualityMetric3>) -> QualityControlRequest3 {
        QualityControlRequest3 {
            schema_version: INPUT_SCHEMA.into(),
            request_id: "q".into(),
            researcher: "dev".into(),
            purpose: "qc".into(),
            required_metric_order: vec!["m".into()],
            required_modality_order: vec!["imaging".into()],
            object: ResearchObject3 {
                object_id: "o".into(),
                semantic_profile: "ome".into(),
                modality_order: vec!["imaging".into()],
                provenance_digest: h("p"),
                replay_identity: h("r"),
                metrics: ms,
            },
            replay_identity: h("r"),
            policy_allowed: true,
            protected_closure: true,
            signed_approval: true,
            tool_approval: true,
            raw_data_local: true,
            aggregate_only: true,
            adversarial_event_order: vec![],
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn qualified() {
        assert_eq!(
            qualify_quality_control(&req(vec![metric("m")]))
                .unwrap()
                .disposition,
            "qualified"
        )
    }
    #[test]
    fn unknown_is_visible() {
        let mut m = metric("m");
        m.evidence_state = QualityEvidenceState::Unknown;
        assert!(!qualify_quality_control(&req(vec![m]))
            .unwrap()
            .unknown_order
            .is_empty())
    }
    #[test]
    fn tool_gate_blocks() {
        let mut r = req(vec![metric("m")]);
        r.tool_approval = false;
        assert_eq!(qualify_quality_control(&r).unwrap().disposition, "blocked")
    }
}
