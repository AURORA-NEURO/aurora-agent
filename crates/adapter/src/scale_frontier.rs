//! Prospective adapter scale-frontier workflow fabric.
//!
//! Atlas feature: `AFA-adapter-P29-F15`.
//!
//! Replays declared throughput scenarios and emits a bounded capacity frontier. The planner
//! never schedules work or treats a nominal capacity estimate as an authorization to execute.

use bioprism_foundation::{
    ProvenanceLink, TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P29-F15";
pub const CONTRACT_VERSION: &str = "adapter-scale-frontier/1.0";
const MAX_TEXT_BYTES: usize = 512;
const MAX_SCENARIOS: usize = 8192;
const MAX_NOTE_ITEMS: usize = 16384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScaleScenario {
    pub scenario_id: String,
    pub workload_count: u64,
    pub concurrency: u64,
    pub max_budget_units: u64,
    pub failure_rate_ppm: u32,
    pub policy_allow: bool,
    pub protected_closure: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScaleFrontierRequest {
    pub request_id: String,
    pub workflow_id: String,
    pub required_capacity: u64,
    pub scenarios: Vec<ScaleScenario>,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScaleDisposition {
    Ready,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScaleFrontierReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub required_capacity: u64,
    pub scenarios: Vec<ScaleScenario>,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub disposition: ScaleDisposition,
    pub scenario_order: Vec<String>,
    pub admissible_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub frontier_order: Vec<String>,
    pub max_admitted_concurrency: u64,
    pub frontier_digest: ContentHash,
    pub checks: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

impl ScaleFrontierReceipt {
    pub fn validate(&self) -> Result<(), ScaleFrontierError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.required_capacity == 0
            || self.frontier_digest == ContentHash::of_bytes(b"")
            || self.scenario_order.is_empty()
            || self.scenarios.is_empty()
            || self.checks.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(ScaleFrontierError::Invalid(
                "scale frontier identity, scenarios, capacity, checks, effects, locality, or boundary are incomplete".into(),
            ));
        }
        validate_text("request_id", &self.request_id)?;
        validate_text("workflow_id", &self.workflow_id)?;
        if self.scenarios.len() > MAX_SCENARIOS
            || self
                .scenarios
                .windows(2)
                .any(|pair| pair[0].scenario_id >= pair[1].scenario_id)
        {
            return Err(ScaleFrontierError::Invalid(
                "scale scenarios are not in canonical order".into(),
            ));
        }
        for (field, values) in [
            ("scenario_order", &self.scenario_order),
            ("admissible_order", &self.admissible_order),
            ("blocked_order", &self.blocked_order),
            ("frontier_order", &self.frontier_order),
            ("checks", &self.checks),
            ("omissions", &self.omissions),
            ("uncertainty", &self.uncertainty),
            ("negative_evidence", &self.negative_evidence),
            ("effect_receipts", &self.effect_receipts),
        ] {
            validate_sorted_strings(field, values)?;
        }
        let scenario_ids = self.scenario_order.iter().collect::<BTreeSet<_>>();
        let admissible_ids = self.admissible_order.iter().collect::<BTreeSet<_>>();
        let blocked_ids = self.blocked_order.iter().collect::<BTreeSet<_>>();
        let frontier_ids = self.frontier_order.iter().collect::<BTreeSet<_>>();
        if admissible_ids.intersection(&blocked_ids).next().is_some()
            || admissible_ids
                .union(&blocked_ids)
                .cloned()
                .collect::<BTreeSet<_>>()
                != scenario_ids
            || frontier_ids != scenario_ids
            || (self.admissible_order.is_empty() && self.max_admitted_concurrency != 0)
            || (!self.admissible_order.is_empty() && self.max_admitted_concurrency == 0)
        {
            return Err(ScaleFrontierError::Invalid(
                "scale frontier scenario partition or capacity summary is inconsistent".into(),
            ));
        }
        let expected_effect = match self.disposition {
            ScaleDisposition::Ready | ScaleDisposition::Partial => {
                "exchange:permitted-scale-frontier-digests-only"
            }
            ScaleDisposition::Blocked => "block:adapter-scale-frontier:blocked",
            ScaleDisposition::Unknown => "block:adapter-scale-frontier:unknown",
        };
        if self.effect_receipts != vec![expected_effect.to_string()] {
            return Err(ScaleFrontierError::Invalid(
                "scale frontier effect does not match disposition".into(),
            ));
        }
        let expected_frontier_digest = ContentHash::of_value(&frontier_digest_payload(self))
            .map_err(|error| ScaleFrontierError::Serialization(error.to_string()))?;
        if self.frontier_digest != expected_frontier_digest {
            return Err(ScaleFrontierError::Invalid(
                "scale frontier digest does not bind the receipt".into(),
            ));
        }
        let expected_provenance = scale_provenance(&self.scenarios)?;
        if self.artifact.artifact_id != format!("adapter-scale-frontier:{}", self.request_id)
            || self.artifact.content_type != "application/vnd.aurora.adapter-scale-frontier+json"
            || self.artifact.provenance != expected_provenance
        {
            return Err(ScaleFrontierError::Artifact(
                "scale frontier artifact is not bound to the receipt".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ScaleFrontierError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&scale_payload(self))
            .map_err(|error| ScaleFrontierError::Artifact(error.to_string()))?;
        let request = ScaleFrontierRequest {
            request_id: self.request_id.clone(),
            workflow_id: self.workflow_id.clone(),
            required_capacity: self.required_capacity,
            scenarios: self.scenarios.clone(),
            policy_allow: self.policy_allow,
            protected_closure: self.protected_closure,
            raw_data_local: self.raw_data_local,
            boundary: self.boundary.clone(),
        };
        let expected = plan_adapter_scale_frontier_internal(&request, false)?;
        if self != &expected {
            return Err(ScaleFrontierError::Artifact(
                "scale frontier is not derived from its retained scenarios and gates".into(),
            ));
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<ContentHash, ScaleFrontierError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|e| ScaleFrontierError::Serialization(e.to_string()))?;
        ContentHash::of_value(&value).map_err(|e| ScaleFrontierError::Serialization(e.to_string()))
    }
}

fn validate_text(field: &str, value: &str) -> Result<(), ScaleFrontierError> {
    if value.is_empty() || value.trim() != value {
        return Err(ScaleFrontierError::Invalid(format!(
            "{field} must be non-empty and trimmed"
        )));
    }
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(ScaleFrontierError::Invalid(format!(
            "{field} is outside its bounded text contract"
        )));
    }
    Ok(())
}

fn validate_sorted_strings(field: &str, values: &[String]) -> Result<(), ScaleFrontierError> {
    if values.len() > MAX_NOTE_ITEMS {
        return Err(ScaleFrontierError::Invalid(format!(
            "{field} exceeds its item bound"
        )));
    }
    let mut unique = BTreeSet::new();
    for value in values {
        validate_text(field, value)?;
        if !unique.insert(value) {
            return Err(ScaleFrontierError::Invalid(format!(
                "{field} contains duplicate values"
            )));
        }
    }
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ScaleFrontierError::Invalid(format!(
            "{field} ordering is not canonical"
        )));
    }
    Ok(())
}

fn scale_provenance(
    scenarios: &[ScaleScenario],
) -> Result<Vec<ProvenanceLink>, ScaleFrontierError> {
    scenarios
        .iter()
        .map(|scenario| {
            let value = serde_json::to_value(scenario)
                .map_err(|error| ScaleFrontierError::Serialization(error.to_string()))?;
            let digest = ContentHash::of_value(&value)
                .map_err(|error| ScaleFrontierError::Serialization(error.to_string()))?;
            Ok(ProvenanceLink {
                source_id: scenario.scenario_id.clone(),
                relation: "scale-frontier-scenario-input".into(),
                digest,
            })
        })
        .collect()
}

fn scale_payload(receipt: &ScaleFrontierReceipt) -> serde_json::Value {
    scale_payload_from_parts(
        &receipt.schema_version,
        &receipt.contract_version,
        &receipt.feature_id,
        &receipt.request_id,
        &receipt.workflow_id,
        receipt.required_capacity,
        &receipt.scenarios,
        receipt.policy_allow,
        receipt.protected_closure,
        receipt.disposition,
        &receipt.scenario_order,
        &receipt.admissible_order,
        &receipt.blocked_order,
        &receipt.frontier_order,
        receipt.max_admitted_concurrency,
        &receipt.frontier_digest,
        &receipt.checks,
        &receipt.omissions,
        &receipt.uncertainty,
        &receipt.negative_evidence,
        &receipt.effect_receipts,
        &receipt.artifact.provenance,
        receipt.raw_data_local,
        &receipt.boundary,
    )
}

#[allow(clippy::too_many_arguments)]
fn scale_payload_from_parts(
    schema_version: &str,
    contract_version: &str,
    feature_id: &str,
    request_id: &str,
    workflow_id: &str,
    required_capacity: u64,
    scenarios: &[ScaleScenario],
    policy_allow: bool,
    protected_closure: bool,
    disposition: ScaleDisposition,
    scenario_order: &[String],
    admissible_order: &[String],
    blocked_order: &[String],
    frontier_order: &[String],
    max_admitted_concurrency: u64,
    frontier_digest: &ContentHash,
    checks: &[String],
    omissions: &[String],
    uncertainty: &[String],
    negative_evidence: &[String],
    effect_receipts: &[String],
    provenance: &[ProvenanceLink],
    raw_data_local: bool,
    boundary: &str,
) -> serde_json::Value {
    json!({
        "schema_version": schema_version,
        "contract_version": contract_version,
        "feature_id": feature_id,
        "request_id": request_id,
        "workflow_id": workflow_id,
        "required_capacity": required_capacity,
        "scenarios": scenarios,
        "policy_allow": policy_allow,
        "protected_closure": protected_closure,
        "disposition": disposition,
        "scenario_order": scenario_order,
        "admissible_order": admissible_order,
        "blocked_order": blocked_order,
        "frontier_order": frontier_order,
        "max_admitted_concurrency": max_admitted_concurrency,
        "frontier_digest": frontier_digest,
        "checks": checks,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": negative_evidence,
        "effect_receipts": effect_receipts,
        "provenance": provenance,
        "raw_data_local": raw_data_local,
        "boundary": boundary,
    })
}

fn frontier_digest_payload(receipt: &ScaleFrontierReceipt) -> serde_json::Value {
    frontier_digest_payload_from_parts(
        &receipt.request_id,
        &receipt.workflow_id,
        receipt.required_capacity,
        receipt.disposition,
        &receipt.scenario_order,
        &receipt.admissible_order,
        &receipt.blocked_order,
        &receipt.frontier_order,
        receipt.max_admitted_concurrency,
        &receipt.checks,
        &receipt.omissions,
        &receipt.uncertainty,
        &receipt.negative_evidence,
        &receipt.effect_receipts,
        receipt.raw_data_local,
        &receipt.boundary,
    )
}

#[allow(clippy::too_many_arguments)]
fn frontier_digest_payload_from_parts(
    request_id: &str,
    workflow_id: &str,
    required_capacity: u64,
    disposition: ScaleDisposition,
    scenario_order: &[String],
    admissible_order: &[String],
    blocked_order: &[String],
    frontier_order: &[String],
    max_admitted_concurrency: u64,
    checks: &[String],
    omissions: &[String],
    uncertainty: &[String],
    negative_evidence: &[String],
    effect_receipts: &[String],
    raw_data_local: bool,
    boundary: &str,
) -> serde_json::Value {
    json!({
        "request_id": request_id,
        "workflow_id": workflow_id,
        "required_capacity": required_capacity,
        "disposition": disposition,
        "scenario_order": scenario_order,
        "admissible_order": admissible_order,
        "blocked_order": blocked_order,
        "frontier_order": frontier_order,
        "max_admitted_concurrency": max_admitted_concurrency,
        "checks": checks,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": negative_evidence,
        "effect_receipts": effect_receipts,
        "raw_data_local": raw_data_local,
        "boundary": boundary,
    })
}

#[derive(Debug, Error)]
pub enum ScaleFrontierError {
    #[error("invalid adapter scale frontier request: {0}")]
    Invalid(String),
    #[error("adapter scale frontier artifact error: {0}")]
    Artifact(String),
    #[error("adapter scale frontier serialization error: {0}")]
    Serialization(String),
}

pub fn plan_adapter_scale_frontier(
    request: &ScaleFrontierRequest,
) -> Result<ScaleFrontierReceipt, ScaleFrontierError> {
    plan_adapter_scale_frontier_internal(request, true)
}

fn plan_adapter_scale_frontier_internal(
    request: &ScaleFrontierRequest,
    validate_output: bool,
) -> Result<ScaleFrontierReceipt, ScaleFrontierError> {
    validate_request(request)?;
    let mut scenarios = request.scenarios.clone();
    scenarios.sort_by(|a, b| a.scenario_id.cmp(&b.scenario_id));
    let scenario_order = scenarios
        .iter()
        .map(|s| s.scenario_id.clone())
        .collect::<Vec<_>>();
    let mut admissible = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut max = 0;
    if !request.policy_allow {
        omissions.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    for s in &scenarios {
        let units = s.workload_count.checked_mul(s.concurrency);
        let capacity_ok = s.concurrency >= request.required_capacity;
        let budget_ok = units.is_some_and(|v| v <= s.max_budget_units);
        if !request.policy_allow
            || !request.protected_closure
            || !s.policy_allow
            || !s.protected_closure
            || !capacity_ok
            || !budget_ok
            || s.failure_rate_ppm > 100_000
        {
            blocked.insert(s.scenario_id.clone());
            if !capacity_ok {
                omissions.insert(format!(
                    "scenario:{}:capacity-below-required",
                    s.scenario_id
                ));
            }
            if !budget_ok {
                omissions.insert(format!("scenario:{}:budget-exceeded", s.scenario_id));
            }
            if s.failure_rate_ppm > 100_000 {
                negative.insert(format!("scenario:{}:failure-rate-invalid", s.scenario_id));
            }
            if !s.policy_allow {
                omissions.insert(format!("scenario:{}:policy-denied", s.scenario_id));
            }
            if !s.protected_closure {
                uncertainty.insert(format!(
                    "scenario:{}:protected-closure-incomplete",
                    s.scenario_id
                ));
            }
        } else {
            admissible.insert(s.scenario_id.clone());
            max = max.max(s.concurrency);
        }
    }
    let admissible_order = admissible.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let disposition = if !request.policy_allow {
        ScaleDisposition::Blocked
    } else if !request.protected_closure || admissible_order.is_empty() {
        ScaleDisposition::Unknown
    } else if blocked_order.is_empty() {
        ScaleDisposition::Ready
    } else {
        ScaleDisposition::Partial
    };
    let mut checks = vec![
        "scenarios are ordered by stable id".into(),
        "workload, concurrency, budget, failure, policy, and protected-closure gates are explicit"
            .into(),
        "the scale frontier is a planning artifact and does not schedule execution".into(),
    ];
    if matches!(disposition, ScaleDisposition::Blocked) {
        checks.push("request policy denied external scheduling effects".into());
    }
    checks.sort();
    let omissions = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence = negative.into_iter().collect::<Vec<_>>();
    let frontier_order = admissible_order
        .iter()
        .chain(blocked_order.iter())
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let effect_receipts = vec![match disposition {
        ScaleDisposition::Ready | ScaleDisposition::Partial => {
            "exchange:permitted-scale-frontier-digests-only"
        }
        ScaleDisposition::Blocked => "block:adapter-scale-frontier:blocked",
        ScaleDisposition::Unknown => "block:adapter-scale-frontier:unknown",
    }
    .into()];
    let frontier_payload = frontier_digest_payload_from_parts(
        &request.request_id,
        &request.workflow_id,
        request.required_capacity,
        disposition,
        &scenario_order,
        &admissible_order,
        &blocked_order,
        &frontier_order,
        max,
        &checks,
        &omissions,
        &uncertainty,
        &negative_evidence,
        &effect_receipts,
        true,
        PRECLINICAL_BOUNDARY,
    );
    let frontier_digest = ContentHash::of_value(&frontier_payload)
        .map_err(|error| ScaleFrontierError::Serialization(error.to_string()))?;
    let provenance = scale_provenance(&scenarios)?;
    let payload = scale_payload_from_parts(
        RESEARCH_CONTRACT_SCHEMA_VERSION,
        CONTRACT_VERSION,
        FEATURE_ID,
        &request.request_id,
        &request.workflow_id,
        request.required_capacity,
        &scenarios,
        request.policy_allow,
        request.protected_closure,
        disposition,
        &scenario_order,
        &admissible_order,
        &blocked_order,
        &frontier_order,
        max,
        &frontier_digest,
        &checks,
        &omissions,
        &uncertainty,
        &negative_evidence,
        &effect_receipts,
        &provenance,
        true,
        PRECLINICAL_BOUNDARY,
    );
    let artifact = TypedResearchArtifact::from_payload(
        format!("adapter-scale-frontier:{}", request.request_id),
        "application/vnd.aurora.adapter-scale-frontier+json",
        &payload,
        Vec::new(),
        provenance,
    )
    .map_err(|e| ScaleFrontierError::Artifact(e.to_string()))?;
    let receipt = ScaleFrontierReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        required_capacity: request.required_capacity,
        scenarios,
        policy_allow: request.policy_allow,
        protected_closure: request.protected_closure,
        disposition,
        scenario_order,
        admissible_order,
        blocked_order,
        frontier_order,
        max_admitted_concurrency: max,
        frontier_digest,
        checks,
        omissions,
        uncertainty,
        negative_evidence,
        effect_receipts,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    if validate_output {
        receipt.validate()?;
    }
    Ok(receipt)
}

fn validate_request(request: &ScaleFrontierRequest) -> Result<(), ScaleFrontierError> {
    if request.request_id.trim().is_empty()
        || request.workflow_id.trim().is_empty()
        || request.required_capacity == 0
        || request.scenarios.is_empty()
        || request.scenarios.len() > MAX_SCENARIOS
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(ScaleFrontierError::Invalid(
            "scale frontier identity, required capacity, scenarios, locality, and boundary are required".into(),
        ));
    }
    validate_text("request_id", &request.request_id)?;
    validate_text("workflow_id", &request.workflow_id)?;
    validate_text("boundary", &request.boundary)?;
    let mut ids = BTreeSet::new();
    for s in &request.scenarios {
        validate_text("scenario_id", &s.scenario_id)?;
        if !ids.insert(s.scenario_id.clone())
            || s.workload_count == 0
            || s.concurrency == 0
            || s.max_budget_units == 0
            || s.failure_rate_ppm > 1_000_000
        {
            return Err(ScaleFrontierError::Invalid(format!(
                "scenario {} is invalid or duplicated",
                s.scenario_id
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn s(id: &str, c: u64, b: u64) -> ScaleScenario {
        ScaleScenario {
            scenario_id: id.into(),
            workload_count: 10,
            concurrency: c,
            max_budget_units: b,
            failure_rate_ppm: 1000,
            policy_allow: true,
            protected_closure: true,
        }
    }
    fn q() -> ScaleFrontierRequest {
        ScaleFrontierRequest {
            request_id: "scale:adapter".into(),
            workflow_id: "workflow:high-throughput".into(),
            required_capacity: 4,
            scenarios: vec![s("scenario:b", 8, 100), s("scenario:a", 2, 100)],
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn partial_frontier_retains_blocked_capacity_cell() {
        let r = plan_adapter_scale_frontier(&q()).unwrap();
        assert_eq!(r.disposition, ScaleDisposition::Partial);
        assert!(!r.blocked_order.is_empty());
    }
    #[test]
    fn ready_frontier_has_max_concurrency() {
        let mut q = q();
        q.scenarios[1].concurrency = 4;
        let r = plan_adapter_scale_frontier(&q).unwrap();
        assert_eq!(r.disposition, ScaleDisposition::Ready);
        assert_eq!(r.max_admitted_concurrency, 8);
    }
    #[test]
    fn budget_exceedance_is_blocked() {
        let mut q = q();
        q.scenarios[0].max_budget_units = 1;
        let r = plan_adapter_scale_frontier(&q).unwrap();
        assert!(r.omissions.iter().any(|v| v.contains("budget-exceeded")));
    }
    #[test]
    fn protected_gap_is_unknown() {
        let mut q = q();
        q.protected_closure = false;
        assert_eq!(
            plan_adapter_scale_frontier(&q).unwrap().disposition,
            ScaleDisposition::Unknown
        );
    }
    #[test]
    fn policy_denial_blocks() {
        let mut q = q();
        q.policy_allow = false;
        let receipt = plan_adapter_scale_frontier(&q).unwrap();
        assert_eq!(receipt.disposition, ScaleDisposition::Blocked);
        assert!(receipt.admissible_order.is_empty());
        assert_eq!(
            receipt.effect_receipts,
            vec!["block:adapter-scale-frontier:blocked"]
        );
    }

    #[test]
    fn protected_closure_gate_removes_admissible_scenarios() {
        let mut q = q();
        q.protected_closure = false;
        let receipt = plan_adapter_scale_frontier(&q).unwrap();
        assert_eq!(receipt.disposition, ScaleDisposition::Unknown);
        assert!(receipt.admissible_order.is_empty());
        assert_eq!(receipt.max_admitted_concurrency, 0);
    }

    #[test]
    fn frontier_digest_rejects_capacity_tampering() {
        let mut receipt = plan_adapter_scale_frontier(&q()).unwrap();
        receipt.required_capacity = 99;
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn forged_frontier_effect_is_rejected() {
        let mut receipt = plan_adapter_scale_frontier(&q()).unwrap();
        receipt.effect_receipts = vec!["block:adapter-scale-frontier:unknown".into()];
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn retained_scenario_tampering_is_rejected() {
        let mut receipt = plan_adapter_scale_frontier(&q()).unwrap();
        receipt.scenarios[0].concurrency = 99;
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn scale_frontier_provenance_tampering_is_rejected() {
        let mut receipt = plan_adapter_scale_frontier(&q()).unwrap();
        receipt.artifact.provenance[0].digest = ContentHash::of_bytes(b"tampered");
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn scenario_input_order_is_canonicalized() {
        let mut reordered = q();
        reordered.scenarios.reverse();
        let canonical = plan_adapter_scale_frontier(&q()).unwrap();
        let reordered = plan_adapter_scale_frontier(&reordered).unwrap();
        assert_eq!(canonical.digest().unwrap(), reordered.digest().unwrap());
    }
}
