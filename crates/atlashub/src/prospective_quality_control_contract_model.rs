//! Prospective high-throughput quality-control contract model (`AFA-atlashub-P07-F07`).
//!
//! This module is a typed, deterministic boundary for quality envelopes. It validates the
//! shape, locality, replay identity, and evidence closure of a `ResearchObject3`, then emits a
//! content-addressed `QualityVerdict2` artifact. It deliberately does not calculate metrics,
//! execute tools, or move raw preclinical data.

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-atlashub-P07-F07";
pub const CONTRACT_VERSION: &str =
    "atlashub-prospective-high-throughput-quality-control-contract-model/1.0";
pub const INPUT_SCHEMA: &str = "ResearchObject3@1";
pub const OUTPUT_SCHEMA: &str = "QualityVerdict2@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.atlashub-quality-verdict-2+json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContractEvidenceState {
    Proven,
    Supported,
    Speculative,
    Unknown,
    Unmeasured,
    Contradicted,
    Negative,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContractQualityMetric {
    pub metric_id: String,
    pub value: Option<f64>,
    pub threshold: Option<f64>,
    pub evidence_state: ContractEvidenceState,
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
pub struct ContractResearchObject {
    pub object_id: String,
    pub semantic_profile: String,
    pub modality_order: Vec<String>,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub metrics: Vec<ContractQualityMetric>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QualityControlContractRequest {
    pub schema_version: String,
    pub request_id: String,
    pub consumer: String,
    pub purpose: String,
    pub required_metric_order: Vec<String>,
    pub required_modality_order: Vec<String>,
    pub object: ContractResearchObject,
    pub replay_identity: ContentHash,
    pub policy_allowed: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QualityVerdictArtifact2 {
    pub artifact: TypedResearchArtifact,
    pub semantic_loss_order: Vec<String>,
    pub provenance_order: Vec<ContentHash>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QualityVerdict2 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub consumer: String,
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
    pub artifact: QualityVerdictArtifact2,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum QualityControlContractError {
    #[error("invalid quality-control contract request: {0}")]
    Invalid(String),
    #[error("quality-control contract artifact failed: {0}")]
    Artifact(String),
    #[error("quality-control contract output failed: {0}")]
    Output(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_is_valid(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn nonempty(value: &str) -> bool {
    !value.trim().is_empty()
}

pub fn prospective_quality_control_contract_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "atlashub".into(),
        consumers: BTreeSet::from([
            "integration engineer".into(),
            "quality engineer".into(),
            "benchmark curator".into(),
        ]),
        behavior: "validate and canonicalize prospective high-throughput preclinical quality envelopes into witness-bearing verdict artifacts".into(),
        value: "gives independent producers a stable schema, serializer, and compatibility boundary without granting execution authority".into(),
        inputs: vec![TypedPort { name: "research_object".into(), schema: INPUT_SCHEMA.into(), required: true }],
        outputs: vec![TypedPort { name: "quality_verdict".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: BTreeSet::from([Effect::ReadLocalData]),
        permissions: BTreeSet::from(["read:local-research-artifacts".into()]),
        determinism: Determinism::ByteStable,
        evidence: vec![
            EvidenceReference { source_id: "ro-crate-1.3".into(), state: EvidenceState::Supported, locator: Some("https://www.researchobject.org/ro-crate/specification.html".into()) },
            EvidenceReference { source_id: "json-schema".into(), state: EvidenceState::Supported, locator: Some("https://json-schema.org/specification".into()) },
            EvidenceReference { source_id: "ome-ngff-0.5".into(), state: EvidenceState::Supported, locator: Some("https://ngff.openmicroscopy.org/rfc/5/".into()) },
        ],
        authority_requirements: Vec::new(),
        autonomy_tier: AutonomyTier::A1,
        surfaces: BTreeSet::from([
            ResearchSurface::Ui, ResearchSurface::Cli, ResearchSurface::Api,
            ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Protocol,
            ResearchSurface::Policy, ResearchSurface::Operator,
        ]),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

fn validate_request(request: &QualityControlContractRequest) -> Result<(), QualityControlContractError> {
    if request.schema_version != INPUT_SCHEMA
        || !nonempty(&request.request_id)
        || !nonempty(&request.consumer)
        || !nonempty(&request.purpose)
        || request.required_metric_order.is_empty()
        || !canonical(&request.required_metric_order)
        || !canonical(&request.required_modality_order)
        || !digest_is_valid(&request.replay_identity)
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(QualityControlContractError::Invalid(
            "identity, required closure, replay, policy, locality, aggregate, or boundary is invalid".into(),
        ));
    }
    let object = &request.object;
    if !nonempty(&object.object_id)
        || !nonempty(&object.semantic_profile)
        || !canonical(&object.modality_order)
        || !digest_is_valid(&object.provenance_digest)
        || object.replay_identity != request.replay_identity
    {
        return Err(QualityControlContractError::Invalid(
            "research-object identity, modality ordering, provenance, or replay is invalid".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for metric in &object.metrics {
        if !nonempty(&metric.metric_id)
            || !ids.insert(metric.metric_id.clone())
            || !digest_is_valid(&metric.provenance_digest)
            || metric.replay_identity != request.replay_identity
            || !canonical(&metric.omission_order)
            || !canonical(&metric.uncertainty_order)
        {
            return Err(QualityControlContractError::Invalid(
                "metric identity, uniqueness, replay, provenance, or ordering is invalid".into(),
            ));
        }
    }
    Ok(())
}

impl QualityVerdict2 {
    pub fn validate(&self) -> Result<(), QualityControlContractError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.artifact.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.artifact.content_type != CONTENT_TYPE
            || !self.raw_data_local
            || !self.aggregate_only
            || !["qualified", "unresolved", "blocked"].contains(&self.disposition.as_str())
            || !nonempty(&self.request_id)
            || !nonempty(&self.consumer)
            || !nonempty(&self.object_id)
        {
            return Err(QualityControlContractError::Output(
                "identity, locality, metric closure, disposition, or artifact metadata is incomplete".into(),
            ));
        }
        for values in [
            &self.metric_order, &self.passed_order, &self.failed_order, &self.unknown_order,
            &self.unmeasured_order, &self.blocked_order, &self.missing_order, &self.modality_order,
            &self.missing_modality_order, &self.omission_order, &self.uncertainty_order,
            &self.negative_evidence_order,
        ] {
            if !canonical(values) {
                return Err(QualityControlContractError::Output("output ordering is not canonical".into()));
            }
        }
        let ids = self.metric_order.iter().cloned().collect::<BTreeSet<_>>();
        let parts = self.passed_order.iter().chain(&self.failed_order).chain(&self.unknown_order)
            .chain(&self.unmeasured_order).chain(&self.blocked_order).chain(&self.missing_order)
            .cloned().collect::<Vec<_>>();
        if ids.len() != self.metric_order.len()
            || parts.len() != ids.len()
            || parts.iter().cloned().collect::<BTreeSet<_>>() != ids
        {
            return Err(QualityControlContractError::Output("metric states do not partition metrics".into()));
        }
        if !digest_is_valid(&self.replay_identity)
            || !digest_is_valid(&self.verdict_digest)
            || self.artifact.artifact.content_hash != self.verdict_digest
            || self.artifact.provenance_order.iter().any(|value| !digest_is_valid(value))
        {
            return Err(QualityControlContractError::Output("artifact or replay digest is invalid".into()));
        }
        self.artifact.artifact.validate_metadata().map_err(|error| QualityControlContractError::Output(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, QualityControlContractError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| QualityControlContractError::Output(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| QualityControlContractError::Output(error.to_string()))
    }
}

pub fn model_prospective_quality_control_contract(
    request: &QualityControlContractRequest,
) -> Result<QualityVerdict2, QualityControlContractError> {
    validate_request(request)?;
    let mut rows = request.object.metrics.clone();
    rows.sort_by(|left, right| left.metric_id.cmp(&right.metric_id));
    let mut metric_ids = rows.iter().map(|row| row.metric_id.clone()).collect::<BTreeSet<_>>();
    let required = request.required_metric_order.iter().cloned().collect::<BTreeSet<_>>();
    let mut passed = BTreeSet::new(); let mut failed = BTreeSet::new(); let mut unknown = BTreeSet::new();
    let mut unmeasured = BTreeSet::new(); let mut blocked = BTreeSet::new(); let mut missing = BTreeSet::new();
    let mut omission = BTreeSet::new(); let mut uncertainty = BTreeSet::new(); let mut negative = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    for row in &rows {
        let id = row.metric_id.clone(); provenance.insert(row.provenance_digest.clone());
        omission.extend(row.omission_order.iter().map(|value| format!("{id}:{value}")));
        uncertainty.extend(row.uncertainty_order.iter().map(|value| format!("{id}:{value}")));
        if row.negative_result || row.evidence_state == ContractEvidenceState::Negative {
            negative.insert(format!("{id}:negative-result"));
        }
        if !row.policy_allowed || !row.local || !row.aggregate_only || row.replay_identity != request.replay_identity {
            blocked.insert(id.clone()); omission.insert(format!("{id}:policy-locality-or-replay"));
        } else if row.evidence_state == ContractEvidenceState::Contradicted {
            failed.insert(id.clone());
        } else if matches!(row.evidence_state, ContractEvidenceState::Unknown | ContractEvidenceState::Speculative) {
            unknown.insert(id.clone()); uncertainty.insert(format!("{id}:unknown-evidence"));
        } else if row.evidence_state == ContractEvidenceState::Unmeasured || row.value.is_none() {
            unmeasured.insert(id.clone()); omission.insert(format!("{id}:unmeasured"));
        } else if row.threshold.is_some_and(|threshold| row.value.unwrap_or(f64::NAN) < threshold) {
            failed.insert(id.clone());
        } else if !matches!(row.evidence_state, ContractEvidenceState::Proven | ContractEvidenceState::Supported) {
            unknown.insert(id.clone());
        } else { passed.insert(id); }
    }
    for id in &required {
        if !metric_ids.contains(id) {
            missing.insert(id.clone());
            metric_ids.insert(id.clone());
            omission.insert(format!("missing:{id}"));
        }
    }
    let metric_order = metric_ids.iter().cloned().collect::<Vec<_>>();
    let missing_modality = if request.required_modality_order.iter().all(|id| request.object.modality_order.contains(id)) { Vec::new() } else { request.required_modality_order.clone() };
    if !missing_modality.is_empty() { omission.insert("request:required-modality-missing".into()); }
    let global_block = !request.policy_allowed || !request.protected_closure || !request.raw_data_local || !request.aggregate_only;
    if global_block { blocked.extend(metric_order.iter().cloned()); passed.clear(); failed.clear(); unknown.clear(); unmeasured.clear(); missing.clear(); omission.insert("request:quality-contract-gate-blocked".into()); }
    let disposition = if global_block || (!blocked.is_empty() && passed.is_empty()) { "blocked" } else if !failed.is_empty() || !unknown.is_empty() || !unmeasured.is_empty() || !blocked.is_empty() || !missing.is_empty() || !missing_modality.is_empty() { "unresolved" } else { "qualified" };
    if disposition != "qualified" { omission.insert("request:quality-closure-not-ready".into()); }
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID, "request_id": request.request_id, "consumer": request.consumer,
        "object_id": request.object.object_id, "semantic_profile": request.object.semantic_profile,
        "disposition": disposition, "metric_order": metric_order,
        "passed_order": passed.iter().cloned().collect::<Vec<_>>(), "failed_order": failed.iter().cloned().collect::<Vec<_>>(),
        "unknown_order": unknown.iter().cloned().collect::<Vec<_>>(), "unmeasured_order": unmeasured.iter().cloned().collect::<Vec<_>>(),
        "blocked_order": blocked.iter().cloned().collect::<Vec<_>>(), "missing_order": missing.iter().cloned().collect::<Vec<_>>(),
        "modality_order": request.object.modality_order, "missing_modality_order": missing_modality,
        "omission_order": omission.iter().cloned().collect::<Vec<_>>(), "uncertainty_order": uncertainty.iter().cloned().collect::<Vec<_>>(),
        "negative_evidence_order": negative.iter().cloned().collect::<Vec<_>>(), "replay_identity": request.replay_identity,
        "raw_data_local": true, "aggregate_only": true, "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("atlashub-quality-verdict-2:{}", request.request_id), CONTENT_TYPE, &payload, Vec::new(), Vec::new(),
    ).map_err(|error| QualityControlContractError::Artifact(error.to_string()))?;
    let verdict_digest = artifact.content_hash.clone();
    let out = QualityVerdict2 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), contract_version: CONTRACT_VERSION.into(), feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(), consumer: request.consumer.clone(), object_id: request.object.object_id.clone(), semantic_profile: request.object.semantic_profile.clone(),
        disposition: disposition.into(), metric_order: payload["metric_order"].as_array().unwrap().iter().map(|value| value.as_str().unwrap().into()).collect(),
        passed_order: passed.into_iter().collect(), failed_order: failed.into_iter().collect(), unknown_order: unknown.into_iter().collect(), unmeasured_order: unmeasured.into_iter().collect(), blocked_order: blocked.into_iter().collect(), missing_order: missing.into_iter().collect(), modality_order: request.object.modality_order.clone(), missing_modality_order: payload["missing_modality_order"].as_array().unwrap().iter().map(|value| value.as_str().unwrap().into()).collect(), omission_order: omission.into_iter().collect(), uncertainty_order: uncertainty.into_iter().collect(), negative_evidence_order: negative.into_iter().collect(), replay_identity: request.replay_identity.clone(), verdict_digest, artifact: QualityVerdictArtifact2 { artifact, semantic_loss_order: payload["omission_order"].as_array().unwrap().iter().map(|value| value.as_str().unwrap().into()).collect(), provenance_order: provenance.into_iter().collect() }, raw_data_local: true, aggregate_only: true, boundary: PRECLINICAL_BOUNDARY.into(),
    };
    out.validate()?;
    Ok(out)
}

pub fn model_prospective_quality_control_contract_json(value: &Value) -> Result<Value, String> {
    let request: QualityControlContractRequest = serde_json::from_value(value.clone()).map_err(|error| format!("invalid quality-control contract request: {error}"))?;
    serde_json::to_value(model_prospective_quality_control_contract(&request).map_err(|error| error.to_string())?).map_err(|error| error.to_string())
}

pub fn validate_prospective_quality_control_contract_json(value: &Value) -> Result<QualityVerdict2, String> {
    let verdict: QualityVerdict2 = serde_json::from_value(value.clone()).map_err(|error| format!("invalid quality-control contract verdict: {error}"))?;
    verdict.validate().map_err(|error| error.to_string())?;
    if verdict.feature_id != FEATURE_ID { return Err("quality-control contract feature id mismatch".into()); }
    Ok(verdict)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash { ContentHash::of_bytes(value.as_bytes()) }
    fn metric(id: &str) -> ContractQualityMetric { ContractQualityMetric { metric_id: id.into(), value: Some(1.0), threshold: Some(0.5), evidence_state: ContractEvidenceState::Supported, provenance_digest: hash("p"), replay_identity: hash("r"), omission_order: Vec::new(), uncertainty_order: Vec::new(), negative_result: false, policy_allowed: true, local: true, aggregate_only: true } }
    fn request(metrics: Vec<ContractQualityMetric>) -> QualityControlContractRequest { QualityControlContractRequest { schema_version: INPUT_SCHEMA.into(), request_id: "q".into(), consumer: "integration engineer".into(), purpose: "quality contract".into(), required_metric_order: vec!["m".into()], required_modality_order: vec!["imaging".into()], object: ContractResearchObject { object_id: "o".into(), semantic_profile: "ome-ngff".into(), modality_order: vec!["imaging".into()], provenance_digest: hash("p"), replay_identity: hash("r"), metrics }, replay_identity: hash("r"), policy_allowed: true, protected_closure: true, raw_data_local: true, aggregate_only: true, boundary: PRECLINICAL_BOUNDARY.into() } }
    #[test] fn qualified_contract_is_replayable() { let out = model_prospective_quality_control_contract(&request(vec![metric("m")])).unwrap(); assert_eq!(out.disposition, "qualified"); assert_eq!(out.digest().unwrap(), out.digest().unwrap()); }
    #[test] fn unknown_is_unresolved() { let mut row = metric("m"); row.evidence_state = ContractEvidenceState::Unknown; assert_eq!(model_prospective_quality_control_contract(&request(vec![row])).unwrap().disposition, "unresolved"); }
    #[test] fn missing_required_metric_is_explicit() { let mut req = request(Vec::new()); req.required_metric_order = vec!["missing".into()]; let out = model_prospective_quality_control_contract(&req).unwrap(); assert!(out.missing_order.contains(&"missing".into())); }
    #[test] fn policy_gate_blocks() { let mut req = request(vec![metric("m")]); req.protected_closure = false; assert_eq!(model_prospective_quality_control_contract(&req).unwrap().disposition, "blocked"); }
}
