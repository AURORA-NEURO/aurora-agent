//! Deterministic, omission-aware quality control for Worldgen P07 F01-F04.
use bioprism_foundation::{PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const CONTENT_TYPE: &str = "application/vnd.aurora.worldgen.quality-control-receipt+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityObservation {
    pub observation_id: String,
    pub metric: String,
    pub value_milli: Option<u32>,
    pub threshold_milli: Option<u32>,
    pub state: String,
    pub evidence_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub artifact_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub raw_data_local: bool,
    pub negative_result: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityControlRequest {
    pub batch_id: String,
    pub consumer: String,
    pub observation_order: Vec<String>,
    pub required_metric_order: Vec<String>,
    pub observations: Vec<QualityObservation>,
    pub min_pass_fraction_milli: u32,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub federation_approved: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityVerdict {
    pub observation_id: String,
    pub metric: String,
    pub value_milli: Option<u32>,
    pub state: String,
    pub artifact_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityControlReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub batch_id: String,
    pub consumer: String,
    pub disposition: String,
    pub observation_order: Vec<String>,
    pub passed_order: Vec<String>,
    pub failed_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub unmeasured_order: Vec<String>,
    pub contradicted_order: Vec<String>,
    pub stale_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub omitted_order: Vec<String>,
    pub required_metric_order: Vec<String>,
    pub verdicts: Vec<QualityVerdict>,
    pub pass_fraction_milli: u32,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub replay_identity: ContentHash,
    pub quality_digest: ContentHash,
    pub effect_receipts: Vec<String>,
    pub artifact: serde_json::Value,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum QualityControlError {
    #[error("invalid quality control request: {0}")]
    Invalid(String),
    #[error("quality control artifact failed: {0}")]
    Artifact(String),
    #[error("invalid quality control receipt: {0}")]
    Receipt(String),
}

fn valid_hash(v: &ContentHash) -> bool {
    v.as_str().len() == 64 && v.as_str().bytes().all(|b| b.is_ascii_hexdigit())
}
fn ordered(v: &[String]) -> bool {
    v.windows(2).all(|p| p[0] < p[1])
}
fn sorted(v: Vec<String>) -> Vec<String> {
    let mut o = v;
    o.sort();
    o.dedup();
    o
}
fn digest(v: &serde_json::Value) -> Result<ContentHash, QualityControlError> {
    ContentHash::of_value(v).map_err(|e| QualityControlError::Artifact(e.to_string()))
}

impl QualityControlReceipt {
    pub fn validate(&self) -> Result<(), QualityControlError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.get("boundary").and_then(|v| v.as_str()) != Some(PRECLINICAL_BOUNDARY)
            || self.artifact.get("content_type").and_then(|v| v.as_str()) != Some(CONTENT_TYPE)
            || !self.raw_data_local
            || !self.aggregate_only
            || self.batch_id.trim().is_empty()
            || self.consumer.trim().is_empty()
            || self.observation_order.is_empty()
            || self.effect_receipts.is_empty()
            || ![&self.replay_identity, &self.quality_digest]
                .into_iter()
                .all(valid_hash)
        {
            return Err(QualityControlError::Receipt(
                "quality identity, observations, locality, digests, or effects are incomplete"
                    .into(),
            ));
        }
        for v in [
            &self.observation_order,
            &self.passed_order,
            &self.failed_order,
            &self.unknown_order,
            &self.unmeasured_order,
            &self.contradicted_order,
            &self.stale_order,
            &self.blocked_order,
            &self.omitted_order,
            &self.required_metric_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if !ordered(v) {
                return Err(QualityControlError::Receipt(
                    "quality vectors are not canonical".into(),
                ));
            }
        }
        let ids = self
            .observation_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let parts = self
            .passed_order
            .iter()
            .chain(&self.failed_order)
            .chain(&self.unknown_order)
            .chain(&self.unmeasured_order)
            .chain(&self.contradicted_order)
            .chain(&self.stale_order)
            .chain(&self.blocked_order)
            .chain(&self.omitted_order)
            .cloned()
            .collect::<Vec<_>>();
        if ids.len() != self.observation_order.len()
            || parts.len() != ids.len()
            || parts.iter().cloned().collect::<BTreeSet<_>>() != ids
        {
            return Err(QualityControlError::Receipt(
                "quality observation states do not partition".into(),
            ));
        }
        if self
            .verdicts
            .iter()
            .map(|v| v.observation_id.clone())
            .collect::<BTreeSet<_>>()
            != self
                .passed_order
                .iter()
                .chain(&self.failed_order)
                .chain(&self.unknown_order)
                .chain(&self.unmeasured_order)
                .chain(&self.contradicted_order)
                .chain(&self.stale_order)
                .cloned()
                .collect::<BTreeSet<_>>()
        {
            return Err(QualityControlError::Receipt(
                "quality verdicts do not match observation states".into(),
            ));
        }
        if self.artifact.get("content_hash").and_then(|v| v.as_str())
            != Some(self.quality_digest.as_str())
        {
            return Err(QualityControlError::Receipt(
                "quality artifact digest is inconsistent".into(),
            ));
        }
        if self
            .effect_receipts
            .iter()
            .any(|e| e != "block:unsafe-release" && !e.starts_with("assess:worldgen-quality:"))
        {
            return Err(QualityControlError::Receipt(
                "quality effect is outside assessment gate".into(),
            ));
        }
        Ok(())
    }
}

pub fn manifest(
    feature_id: &str,
    version: &str,
    input_schema: &str,
    scale: &str,
    autonomy: &str,
) -> serde_json::Value {
    json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":version,"owner_crate":"worldgen","consumers":["imaging core scientist","benchmark curator","research program lead","preclinical neuroscientist"],"behavior":format!("assess omission-aware research quality for {scale}"),"value":"turns typed research observations into witness-bearing quality verdicts without treating unknown or unmeasured evidence as pass","input_schema":input_schema,"output_schema":"QualityVerdict1@1","effects":["assess:worldgen-quality","block:unsafe-release"],"permissions":["assess:local-research-quality"],"determinism":"byte_stable","autonomy_tier":autonomy,"boundary":PRECLINICAL_BOUNDARY,"contract_version":version})
}

pub fn assess(
    request: &QualityControlRequest,
    feature_id: &str,
    version: &str,
    scale: &str,
    require_federation: bool,
) -> Result<QualityControlReceipt, QualityControlError> {
    if request.batch_id.trim().is_empty()
        || request.consumer.trim().is_empty()
        || request.observation_order.is_empty()
        || request.required_metric_order.is_empty()
        || request.observation_order != sorted(request.observation_order.clone())
        || request.required_metric_order != sorted(request.required_metric_order.clone())
        || request
            .observation_order
            .iter()
            .collect::<BTreeSet<_>>()
            .len()
            != request.observation_order.len()
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
        || request.min_pass_fraction_milli > 1000
        || !valid_hash(&request.replay_identity)
    {
        return Err(QualityControlError::Invalid(
            "quality identity, ordering, locality, threshold, boundary, or replay is invalid"
                .into(),
        ));
    }
    if require_federation && !request.federation_approved {
        return Err(QualityControlError::Invalid(
            "quality federation approval is required".into(),
        ));
    }
    let ids = request
        .observation_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut by_id = BTreeMap::new();
    for o in &request.observations {
        if !ids.contains(&o.observation_id)
            || o.boundary != PRECLINICAL_BOUNDARY
            || !o.raw_data_local
            || o.replay_identity != request.replay_identity
            || !valid_hash(&o.evidence_digest)
            || !valid_hash(&o.provenance_digest)
            || !valid_hash(&o.artifact_digest)
            || !valid_hash(&o.replay_identity)
        {
            return Err(QualityControlError::Invalid("quality observation identity, provenance, locality, replay, or boundary is invalid".into()));
        }
        if by_id.insert(o.observation_id.clone(), o).is_some() {
            return Err(QualityControlError::Invalid(
                "duplicate quality observation".into(),
            ));
        }
    }
    let required = request
        .required_metric_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut passed = BTreeSet::new();
    let mut failed = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut unmeasured = BTreeSet::new();
    let mut contradicted = BTreeSet::new();
    let mut stale = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut omitted = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut verdicts = Vec::new();
    for id in &ids {
        match by_id.get(id) {
            None => {
                omitted.insert(id.clone());
                omissions.insert(format!("observation:{}:missing", id));
            }
            Some(o) if !request.policy_allow || !request.protected_closure => {
                blocked.insert(id.clone());
                omissions.insert(format!("observation:{}:policy-or-closure-blocked", id));
            }
            Some(o) if o.negative_result => {
                unknown.insert(id.clone());
                negative.insert(format!("observation:{}:negative-result-retained", id));
                verdicts.push(QualityVerdict {
                    observation_id: o.observation_id.clone(),
                    metric: o.metric.clone(),
                    value_milli: o.value_milli,
                    state: "unknown".into(),
                    artifact_digest: o.artifact_digest.clone(),
                });
            }
            Some(o) if o.state == "stale" => {
                stale.insert(id.clone());
                uncertainty.insert(format!("observation:{}:stale", id));
                verdicts.push(QualityVerdict {
                    observation_id: o.observation_id.clone(),
                    metric: o.metric.clone(),
                    value_milli: o.value_milli,
                    state: "stale".into(),
                    artifact_digest: o.artifact_digest.clone(),
                });
            }
            Some(o) if o.state == "unmeasured" => {
                unmeasured.insert(id.clone());
                uncertainty.insert(format!("observation:{}:unmeasured", id));
                verdicts.push(QualityVerdict {
                    observation_id: o.observation_id.clone(),
                    metric: o.metric.clone(),
                    value_milli: o.value_milli,
                    state: "unmeasured".into(),
                    artifact_digest: o.artifact_digest.clone(),
                });
            }
            Some(o) if o.state == "contradicted" => {
                contradicted.insert(id.clone());
                uncertainty.insert(format!("observation:{}:contradicted", id));
                verdicts.push(QualityVerdict {
                    observation_id: o.observation_id.clone(),
                    metric: o.metric.clone(),
                    value_milli: o.value_milli,
                    state: "contradicted".into(),
                    artifact_digest: o.artifact_digest.clone(),
                });
            }
            Some(o) if o.state == "unknown" => {
                unknown.insert(id.clone());
                uncertainty.insert(format!("observation:{}:unknown", id));
                verdicts.push(QualityVerdict {
                    observation_id: o.observation_id.clone(),
                    metric: o.metric.clone(),
                    value_milli: o.value_milli,
                    state: "unknown".into(),
                    artifact_digest: o.artifact_digest.clone(),
                });
            }
            Some(o) if o.metric.trim().is_empty() || !required.contains(&o.metric) => {
                unknown.insert(id.clone());
                uncertainty.insert(format!("observation:{}:metric-not-required", id));
                verdicts.push(QualityVerdict {
                    observation_id: o.observation_id.clone(),
                    metric: o.metric.clone(),
                    value_milli: o.value_milli,
                    state: "unknown".into(),
                    artifact_digest: o.artifact_digest.clone(),
                });
            }
            Some(o) if o.value_milli.is_none() || o.threshold_milli.is_none() => {
                unmeasured.insert(id.clone());
                uncertainty.insert(format!("observation:{}:value-or-threshold-unmeasured", id));
                verdicts.push(QualityVerdict {
                    observation_id: o.observation_id.clone(),
                    metric: o.metric.clone(),
                    value_milli: o.value_milli,
                    state: "unmeasured".into(),
                    artifact_digest: o.artifact_digest.clone(),
                });
            }
            Some(o) => {
                let pass = o.value_milli.unwrap() >= o.threshold_milli.unwrap();
                if pass {
                    passed.insert(id.clone());
                } else {
                    failed.insert(id.clone());
                }
                verdicts.push(QualityVerdict {
                    observation_id: o.observation_id.clone(),
                    metric: o.metric.clone(),
                    value_milli: o.value_milli,
                    state: if pass { "pass" } else { "fail" }.into(),
                    artifact_digest: o.artifact_digest.clone(),
                });
            }
        }
    }
    let pass_fraction_milli = ((passed.len() as u64 * 1000) / (ids.len().max(1) as u64)) as u32;
    let authority = request.policy_allow
        && request.protected_closure
        && (!require_federation || request.federation_approved);
    let disposition = if !authority {
        "blocked"
    } else if passed.is_empty() {
        "unknown"
    } else if passed.len() == ids.len()
        && pass_fraction_milli >= request.min_pass_fraction_milli
        && omissions.is_empty()
        && uncertainty.is_empty()
        && negative.is_empty()
    {
        "qualified"
    } else {
        "partial"
    };
    let effects = if disposition == "blocked" {
        vec!["block:unsafe-release".into()]
    } else {
        vec![format!("assess:worldgen-quality:{}", request.batch_id)]
    };
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":version,"feature_id":feature_id,"batch_id":request.batch_id,"consumer":request.consumer,"scale":scale,"disposition":disposition,"observation_order":ids,"passed_order":passed,"failed_order":failed,"unknown_order":unknown,"unmeasured_order":unmeasured,"contradicted_order":contradicted,"stale_order":stale,"blocked_order":blocked,"omitted_order":omitted,"required_metric_order":required,"verdicts":verdicts,"pass_fraction_milli":pass_fraction_milli,"omissions":omissions,"uncertainty":uncertainty,"negative_evidence":negative,"replay_identity":request.replay_identity,"effect_receipts":effects,"raw_data_local":true,"aggregate_only":true,"boundary":PRECLINICAL_BOUNDARY});
    let d = digest(&payload)?;
    let receipt = QualityControlReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: version.into(),
        feature_id: feature_id.into(),
        batch_id: request.batch_id.clone(),
        consumer: request.consumer.clone(),
        disposition: disposition.into(),
        observation_order: ids.iter().cloned().collect(),
        passed_order: passed.iter().cloned().collect(),
        failed_order: failed.iter().cloned().collect(),
        unknown_order: unknown.iter().cloned().collect(),
        unmeasured_order: unmeasured.iter().cloned().collect(),
        contradicted_order: contradicted.iter().cloned().collect(),
        stale_order: stale.iter().cloned().collect(),
        blocked_order: blocked.iter().cloned().collect(),
        omitted_order: omitted.iter().cloned().collect(),
        required_metric_order: required.iter().cloned().collect(),
        verdicts,
        pass_fraction_milli,
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        replay_identity: request.replay_identity.clone(),
        quality_digest: d.clone(),
        effect_receipts: effects,
        artifact: json!({"artifact_id":format!("worldgen-quality-verdict:{}",request.batch_id),"content_type":CONTENT_TYPE,"content_hash":d,"boundary":PRECLINICAL_BOUNDARY}),
        raw_data_local: true,
        aggregate_only: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn h(v: &str) -> ContentHash {
        ContentHash::of_bytes(v.as_bytes())
    }
    fn req() -> QualityControlRequest {
        let r = h("replay");
        let o = QualityObservation {
            observation_id: "obs:a".into(),
            metric: "signal".into(),
            value_milli: Some(900),
            threshold_milli: Some(700),
            state: "measured".into(),
            evidence_digest: h("e"),
            provenance_digest: h("p"),
            artifact_digest: h("a"),
            replay_identity: r.clone(),
            raw_data_local: true,
            negative_result: false,
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        QualityControlRequest {
            batch_id: "batch:a".into(),
            consumer: "scientist".into(),
            observation_order: vec!["obs:a".into()],
            required_metric_order: vec!["signal".into()],
            observations: vec![o],
            min_pass_fraction_milli: 800,
            replay_identity: r,
            policy_allow: true,
            protected_closure: true,
            federation_approved: true,
            raw_data_local: true,
            aggregate_only: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn qualified_quality_is_witnessed() {
        let r = assess(
            &req(),
            "AFA-worldgen-P07-F01",
            "worldgen-local-quality-control/1.0",
            "local single-study",
            false,
        )
        .unwrap();
        assert_eq!(r.disposition, "qualified");
        assert_eq!(r.passed_order, vec!["obs:a"]);
    }
    #[test]
    fn unknown_and_negative_are_retained() {
        let mut q = req();
        q.observations[0].negative_result = true;
        let r = assess(
            &q,
            "AFA-worldgen-P07-F02",
            "worldgen-multimodal-quality-control/1.0",
            "multimodal multi-study",
            false,
        )
        .unwrap();
        assert_eq!(r.disposition, "unknown");
        assert!(!r.negative_evidence.is_empty());
    }
    #[test]
    fn policy_blocks_release() {
        let mut q = req();
        q.policy_allow = false;
        let r = assess(
            &q,
            "AFA-worldgen-P07-F03",
            "worldgen-throughput-quality-control/1.0",
            "prospective high-throughput",
            false,
        )
        .unwrap();
        assert_eq!(r.disposition, "blocked");
        assert_eq!(r.effect_receipts, vec!["block:unsafe-release"]);
    }
}
