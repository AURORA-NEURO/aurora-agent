//! Dry-run and replay-bounded reliability copilot (`AFA-ids-P21-F12`).
//!
//! This module preflights idempotent capability work without executing jobs,
//! contacting instruments, moving raw data, or making clinical decisions.

use crate::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-ids-P21-F12";
pub const CONTRACT_VERSION: &str = "ids-dry-run-replay-bounded-reliability-copilot/1.0";
pub const INPUT_SCHEMA: &str = "CapabilityWorkload7@1";
pub const OUTPUT_SCHEMA: &str = "ReliableCapabilityResult9@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.reliable-capability-result-9+json";
pub const PRECLINICAL_BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";
pub const MAX_UNITS: usize = 16_384;
pub const MAX_RETRIES: u8 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReliabilityEvidenceState {
    Proven,
    Supported,
    Unknown,
    Unmeasured,
    Contradicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityWorkUnit8 {
    pub unit_id: String,
    pub capability_id: String,
    pub input_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub idempotency_key: String,
    pub estimated_units: u64,
    pub retry_budget: u8,
    pub evidence_state: ReliabilityEvidenceState,
    pub local: bool,
    pub aggregate_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityWorkload7 {
    pub request_id: String,
    pub workload_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub units: Vec<CapabilityWorkUnit8>,
    pub max_budget_units: u64,
    pub max_retries: u8,
    pub dry_run: bool,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReliableCapabilityResult9Artifact {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReliableCapabilityResult9 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workload_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub disposition: String,
    pub unit_order: Vec<String>,
    pub dry_run_order: Vec<String>,
    pub ready_order: Vec<String>,
    pub retry_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub duplicate_order: Vec<String>,
    pub replay_mismatch_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub retry_count: u64,
    pub total_units: u64,
    pub budget_remaining: u64,
    pub replay_identity: ContentHash,
    pub result_digest: ContentHash,
    pub artifact: ReliableCapabilityResult9Artifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReliabilityCopilotError {
    #[error("invalid reliability workload: {0}")]
    Invalid(String),
    #[error("reliability result failed validation: {0}")]
    Result(String),
}

pub fn reliability_copilot_manifest() -> serde_json::Value {
    json!({
        "schema_version": "aurora-research-contract/1.0",
        "capability_id": FEATURE_ID,
        "version": CONTRACT_VERSION,
        "owner_crate": "ids",
        "consumers": ["capability operator", "agent SDK", "replay auditor", "federation administrator"],
        "behavior": "preflight idempotent capability workloads with dry-run, replay, retry, evidence, and locality gates",
        "value": "prevents duplicate, unreplayable, over-budget, under-evidenced, or unauthorized capability effects from being treated as reliable",
        "input_schema": INPUT_SCHEMA,
        "output_schema": OUTPUT_SCHEMA,
        "effects": ["observe:reliability-plan", "manage:local-capability"],
        "permissions": ["read:local-capability-manifests", "request:reliability-preflight"],
        "autonomy_tier": "A1",
        "boundary": PRECLINICAL_BOUNDARY
    })
}

fn valid_digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().bytes().all(|b| b.is_ascii_hexdigit())
}

fn ordered(values: &[String]) -> bool {
    values.windows(2).all(|window| window[0] < window[1])
}

impl ReliableCapabilityResult9 {
    pub fn validate(&self) -> Result<(), ReliabilityCopilotError> {
        if self.schema_version != "aurora-research-contract/1.0"
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.workload_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.unit_order.is_empty()
            || self.effect_receipts.is_empty()
            || !["qualified", "unresolved", "blocked"].contains(&self.disposition.as_str())
        {
            return Err(ReliabilityCopilotError::Result(
                "reliability identity, locality, units, disposition, or effects are incomplete"
                    .into(),
            ));
        }
        for values in [
            &self.unit_order,
            &self.dry_run_order,
            &self.ready_order,
            &self.retry_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.duplicate_order,
            &self.replay_mismatch_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !ordered(values) {
                return Err(ReliabilityCopilotError::Result(
                    "reliability ordering is not canonical".into(),
                ));
            }
        }
        let ids = BTreeSet::from_iter(self.unit_order.iter().cloned());
        let parts = self
            .dry_run_order
            .iter()
            .chain(&self.ready_order)
            .chain(&self.retry_order)
            .chain(&self.unresolved_order)
            .chain(&self.blocked_order)
            .chain(&self.duplicate_order)
            .chain(&self.replay_mismatch_order)
            .cloned()
            .collect::<Vec<_>>();
        if ids.len() != self.unit_order.len()
            || parts.len() != ids.len()
            || BTreeSet::from_iter(parts) != ids
        {
            return Err(ReliabilityCopilotError::Result(
                "reliability unit states do not partition".into(),
            ));
        }
        if !valid_digest(&self.replay_identity)
            || !valid_digest(&self.result_digest)
            || self.artifact.content_hash != self.result_digest
            || self.artifact.content_type != CONTENT_TYPE
            || self
                .artifact
                .provenance_digests
                .iter()
                .any(|digest| !valid_digest(digest))
        {
            return Err(ReliabilityCopilotError::Result(
                "reliability digest or artifact metadata is inconsistent".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("observe:reliability-plan:")
                && !effect.starts_with("manage:local-capability:")
                && effect != "block:unsafe-release"
        }) {
            return Err(ReliabilityCopilotError::Result(
                "effect is outside the governed reliability gate".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(request: &CapabilityWorkload7) -> Result<(), ReliabilityCopilotError> {
    if request.request_id.trim().is_empty()
        || request.workload_id.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.units.is_empty()
        || request.units.len() > MAX_UNITS
        || request.max_budget_units == 0
        || request.max_retries > MAX_RETRIES
        || !valid_digest(&request.replay_identity)
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
    {
        return Err(ReliabilityCopilotError::Invalid(
            "reliability identity, unit bound, retry bound, budget, replay, or locality is invalid"
                .into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for unit in &request.units {
        if unit.unit_id.trim().is_empty()
            || unit.capability_id.trim().is_empty()
            || unit.idempotency_key.trim().is_empty()
            || !valid_digest(&unit.input_digest)
            || !valid_digest(&unit.replay_identity)
            || unit.estimated_units == 0
            || unit.retry_budget > MAX_RETRIES
            || !ids.insert(unit.unit_id.clone())
        {
            return Err(ReliabilityCopilotError::Invalid(
                "unit identity, capability, digest, budget, retry, or uniqueness is invalid".into(),
            ));
        }
    }
    Ok(())
}

pub fn preflight_reliability(
    request: &CapabilityWorkload7,
) -> Result<ReliableCapabilityResult9, ReliabilityCopilotError> {
    validate_request(request)?;
    let mut units = request.units.clone();
    units.sort_by(|left, right| left.unit_id.cmp(&right.unit_id));
    let unit_order = units
        .iter()
        .map(|unit| unit.unit_id.clone())
        .collect::<Vec<_>>();
    let mut dry_run = BTreeSet::new();
    let mut ready = BTreeSet::new();
    let mut retry = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut duplicate = BTreeSet::new();
    let mut replay_mismatch = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut idempotency = BTreeMap::new();
    let mut total = 0_u64;
    let mut retry_count = 0_u64;
    for unit in &units {
        total = total.saturating_add(unit.estimated_units);
        let id = unit.unit_id.clone();
        if idempotency
            .insert(unit.idempotency_key.clone(), id.clone())
            .is_some()
        {
            duplicate.insert(id);
        } else if !unit.local || !unit.aggregate_only {
            blocked.insert(id.clone());
            omissions.insert(format!("{id}:raw-data-locality"));
        } else if unit.replay_identity != request.replay_identity {
            replay_mismatch.insert(id.clone());
            omissions.insert(format!("{id}:replay-identity"));
        } else if unit.evidence_state == ReliabilityEvidenceState::Contradicted {
            blocked.insert(id.clone());
            negative.insert(format!("{id}:contradicted"));
        } else if !matches!(
            unit.evidence_state,
            ReliabilityEvidenceState::Proven | ReliabilityEvidenceState::Supported
        ) {
            unresolved.insert(id.clone());
            uncertainty.insert(format!("{id}:evidence-state"));
        } else if unit.retry_budget > request.max_retries {
            retry.insert(id.clone());
            retry_count = retry_count.saturating_add(u64::from(unit.retry_budget));
            omissions.insert(format!("{id}:retry-budget"));
        } else if request.dry_run {
            dry_run.insert(id);
        } else {
            ready.insert(id);
        }
    }
    if total > request.max_budget_units {
        omissions.insert(format!("request:budget-exceeded:{total}"));
    }
    let global = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.raw_data_local
        || !request.aggregate_only;
    if global {
        blocked.extend(unit_order.iter().cloned());
        dry_run.clear();
        ready.clear();
        retry.clear();
        unresolved.clear();
        duplicate.clear();
        replay_mismatch.clear();
        omissions.insert("request:governance-or-locality-denied".into());
    }
    let dry_order = dry_run.iter().cloned().collect::<Vec<_>>();
    let ready_order = ready.iter().cloned().collect::<Vec<_>>();
    let retry_order = retry.iter().cloned().collect::<Vec<_>>();
    let unresolved_order = unresolved.iter().cloned().collect::<Vec<_>>();
    let blocked_order = blocked.iter().cloned().collect::<Vec<_>>();
    let duplicate_order = duplicate.iter().cloned().collect::<Vec<_>>();
    let mismatch_order = replay_mismatch.iter().cloned().collect::<Vec<_>>();
    let disposition = if global
        || (dry_order.is_empty() && ready_order.is_empty() && unresolved_order.is_empty())
    {
        "blocked"
    } else if !unresolved_order.is_empty()
        || !blocked_order.is_empty()
        || !retry_order.is_empty()
        || !duplicate_order.is_empty()
        || !mismatch_order.is_empty()
        || total > request.max_budget_units
    {
        "unresolved"
    } else {
        "qualified"
    };
    if disposition != "qualified" {
        omissions.insert("request:reliability-plan-not-closed".into());
    }
    let mut payload = json!({
        "schema_version":"aurora-research-contract/1.0", "contract_version":CONTRACT_VERSION, "feature_id":FEATURE_ID,
        "request_id":request.request_id, "workload_id":request.workload_id, "purpose":request.purpose, "semantic_profile":request.semantic_profile,
        "disposition":disposition, "unit_order":unit_order, "dry_run_order":dry_order, "ready_order":ready_order, "retry_order":retry_order,
        "unresolved_order":unresolved_order, "blocked_order":blocked_order, "duplicate_order":duplicate_order, "replay_mismatch_order":mismatch_order,
        "omission_order":omissions.iter().cloned().collect::<Vec<_>>(), "uncertainty_order":uncertainty.iter().cloned().collect::<Vec<_>>(),
        "negative_evidence_order":negative.iter().cloned().collect::<Vec<_>>(), "retry_count":retry_count, "total_units":total,
        "budget_remaining":request.max_budget_units.saturating_sub(total), "replay_identity":request.replay_identity, "raw_data_local":true,
        "aggregate_only":true, "boundary":PRECLINICAL_BOUNDARY
    });
    let digest = ContentHash::of_value(&payload)
        .map_err(|error| ReliabilityCopilotError::Result(error.to_string()))?;
    payload["result_digest"] = json!(digest);
    payload["artifact"] = json!({
        "artifact_id":format!("reliable-capability-result-9:{}",request.workload_id), "content_type":CONTENT_TYPE,
        "content_hash":digest, "semantic_loss":omissions.iter().cloned().collect::<Vec<_>>(),
        "provenance_digests":units.iter().map(|unit| unit.input_digest.clone()).collect::<BTreeSet<_>>().into_iter().collect::<Vec<_>>(),
        "boundary":PRECLINICAL_BOUNDARY
    });
    payload["effect_receipts"] = json!(if disposition == "qualified" {
        vec![
            format!("manage:local-capability:{}", request.request_id),
            format!("observe:reliability-plan:{}", request.request_id),
        ]
    } else {
        vec!["block:unsafe-release".to_string()]
    });
    let result: ReliableCapabilityResult9 = serde_json::from_value(payload)
        .map_err(|error| ReliabilityCopilotError::Result(error.to_string()))?;
    result.validate()?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn h(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn unit(id: &str) -> CapabilityWorkUnit8 {
        CapabilityWorkUnit8 {
            unit_id: id.into(),
            capability_id: "capability".into(),
            input_digest: h(id),
            replay_identity: h("replay"),
            idempotency_key: format!("key-{id}"),
            estimated_units: 1,
            retry_budget: 1,
            evidence_state: ReliabilityEvidenceState::Supported,
            local: true,
            aggregate_only: true,
        }
    }
    fn request(units: Vec<CapabilityWorkUnit8>) -> CapabilityWorkload7 {
        CapabilityWorkload7 {
            request_id: "reliability:req".into(),
            workload_id: "workload:1".into(),
            purpose: "research".into(),
            semantic_profile: "ome".into(),
            units,
            max_budget_units: 10,
            max_retries: 2,
            dry_run: false,
            replay_identity: h("replay"),
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
        assert_eq!(reliability_copilot_manifest()["autonomy_tier"], "A1");
    }
    #[test]
    fn nominal_is_qualified() {
        assert_eq!(
            preflight_reliability(&request(vec![unit("a")]))
                .unwrap()
                .disposition,
            "qualified"
        );
    }
    #[test]
    fn dry_run_selects_observation() {
        let mut q = request(vec![unit("a")]);
        q.dry_run = true;
        let r = preflight_reliability(&q).unwrap();
        assert_eq!(r.dry_run_order, vec!["a"]);
    }
    #[test]
    fn replay_mismatch_is_unresolved() {
        let mut u = unit("a");
        u.replay_identity = h("other");
        let r = preflight_reliability(&request(vec![u])).unwrap();
        assert_eq!(r.disposition, "blocked");
        assert!(!r.replay_mismatch_order.is_empty());
    }
    #[test]
    fn duplicate_idempotency_is_unresolved() {
        let mut b = unit("b");
        b.idempotency_key = "key-a".into();
        let r = preflight_reliability(&request(vec![unit("a"), b])).unwrap();
        assert_eq!(r.disposition, "unresolved");
        assert_eq!(r.duplicate_order, vec!["b"]);
    }
    #[test]
    fn unknown_evidence_is_unresolved() {
        let mut u = unit("a");
        u.evidence_state = ReliabilityEvidenceState::Unknown;
        let r = preflight_reliability(&request(vec![u])).unwrap();
        assert_eq!(r.disposition, "unresolved");
        assert_eq!(r.uncertainty_order, vec!["a:evidence-state"]);
    }
    #[test]
    fn policy_denial_blocks() {
        let mut q = request(vec![unit("a")]);
        q.policy_allow = false;
        let r = preflight_reliability(&q).unwrap();
        assert_eq!(r.effect_receipts, vec!["block:unsafe-release"]);
    }
}
