//! Prospective adapter scale-frontier workflow fabric.
//!
//! Atlas feature: `AFA-adapter-P29-F15`.
//!
//! Replays declared throughput scenarios and emits a bounded capacity frontier. The planner
//! never schedules work or treats a nominal capacity estimate as an authorization to execute.

use bioprism_foundation::{
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P29-F15";
pub const CONTRACT_VERSION: &str = "adapter-scale-frontier/1.0";

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
    pub disposition: ScaleDisposition,
    pub scenario_order: Vec<String>,
    pub admissible_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub frontier_order: Vec<String>,
    pub max_admitted_concurrency: u64,
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
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.scenario_order.is_empty()
            || self.checks.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(ScaleFrontierError::Invalid("scale frontier identity, scenarios, checks, effects, locality, or boundary are incomplete".into()));
        }
        for values in [
            &self.scenario_order,
            &self.admissible_order,
            &self.blocked_order,
            &self.frontier_order,
            &self.checks,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(ScaleFrontierError::Invalid(
                    "scale frontier ordering is not canonical".into(),
                ));
            }
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ScaleFrontierError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, ScaleFrontierError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|e| ScaleFrontierError::Serialization(e.to_string()))?;
        ContentHash::of_value(&value).map_err(|e| ScaleFrontierError::Serialization(e.to_string()))
    }
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
    for s in &scenarios {
        let units = s.workload_count.checked_mul(s.concurrency);
        let capacity_ok = s.concurrency >= request.required_capacity;
        let budget_ok = units.is_some_and(|v| v <= s.max_budget_units);
        if !s.policy_allow
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
    } else if !request.protected_closure {
        ScaleDisposition::Unknown
    } else if admissible_order.is_empty() {
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
    let effect_receipts = if matches!(
        disposition,
        ScaleDisposition::Ready | ScaleDisposition::Partial
    ) {
        vec!["exchange:permitted-scale-frontier-digests-only".into()]
    } else {
        vec![format!("block:adapter-scale-frontier:{disposition:?}").to_lowercase()]
    };
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"workflow_id":request.workflow_id,"disposition":disposition,"scenario_order":scenario_order,"admissible_order":admissible_order,"blocked_order":blocked_order,"frontier_order":frontier_order,"max_admitted_concurrency":max,"checks":checks,"omissions":omissions,"uncertainty":uncertainty,"negative_evidence":negative_evidence,"effect_receipts":effect_receipts,"raw_data_local":true,"boundary":PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("adapter-scale-frontier:{}", request.request_id),
        "application/vnd.aurora.adapter-scale-frontier+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|e| ScaleFrontierError::Artifact(e.to_string()))?;
    let receipt = ScaleFrontierReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        disposition,
        scenario_order,
        admissible_order,
        blocked_order,
        frontier_order,
        max_admitted_concurrency: max,
        checks,
        omissions,
        uncertainty,
        negative_evidence,
        effect_receipts,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &ScaleFrontierRequest) -> Result<(), ScaleFrontierError> {
    if request.request_id.trim().is_empty()
        || request.workflow_id.trim().is_empty()
        || request.required_capacity == 0
        || request.scenarios.is_empty()
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(ScaleFrontierError::Invalid("scale frontier identity, required capacity, scenarios, locality, and boundary are required".into()));
    }
    let mut ids = BTreeSet::new();
    for s in &request.scenarios {
        if s.scenario_id.trim().is_empty()
            || !ids.insert(s.scenario_id.clone())
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
        assert_eq!(
            plan_adapter_scale_frontier(&q).unwrap().disposition,
            ScaleDisposition::Blocked
        );
    }
}
