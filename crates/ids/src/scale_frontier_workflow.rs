//! Prospective high-throughput scale-frontier workflow (`AFA-ids-P29-F15`).
//!
//! Computes a deterministic, local capacity preview from caller-declared
//! workload cells. It does not schedule jobs or operate instruments.

use crate::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-ids-P29-F15";
pub const CONTRACT_VERSION: &str =
    "ids-prospective-high-throughput-scale-frontier-workflow-fabric/1.0";
pub const INPUT_SCHEMA: &str = "IdsScaleWorkload8@1";
pub const OUTPUT_SCHEMA: &str = "IdsCapacityReport9@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.ids-capacity-report-9+json";
pub const PRECLINICAL_BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";
pub const MAX_CELLS: usize = 16_384;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapacityEvidenceState {
    Proven,
    Supported,
    Unknown,
    Unmeasured,
    Contradicted,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdsScaleCell7 {
    pub cell_id: String,
    pub workload_id: String,
    pub concurrency: u32,
    pub batch_size: u32,
    pub expected_latency_milli: u64,
    pub expected_cost_milli: u64,
    pub capacity_limit: u32,
    pub evidence_state: CapacityEvidenceState,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub local: bool,
    pub aggregate_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdsScaleWorkload8 {
    pub workload_id: String,
    pub capability_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub budget_milli: u64,
    pub maximum_concurrency: u32,
    pub cells: Vec<IdsScaleCell7>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdsCapacityReport9Artifact {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IdsCapacityReport9 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub disposition: String,
    pub workload_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub capacity_exceeded_order: Vec<String>,
    pub budget_exhausted_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub latency_milli_order: Vec<u64>,
    pub cost_milli_order: Vec<u64>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub effect_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub capacity_digest: ContentHash,
    pub artifact: IdsCapacityReport9Artifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ScaleFrontierError {
    #[error("invalid IDS scale-frontier request: {0}")]
    Invalid(String),
    #[error("IDS capacity report failed validation: {0}")]
    Report(String),
}

pub fn scale_frontier_manifest() -> serde_json::Value {
    json!({"schema_version":"aurora-research-contract/1.0","capability_id":FEATURE_ID,"version":CONTRACT_VERSION,"owner_crate":"ids","consumers":["formal methods researcher","capacity operator","workflow engineer"],"behavior":"preview typed high-throughput capacity cells with deterministic concurrency, latency, cost, evidence, policy, replay, and locality gates","value":"makes scale limits and budget exhaustion visible before any work is scheduled","input_schema":INPUT_SCHEMA,"output_schema":OUTPUT_SCHEMA,"effects":["preview:ids-capacity-frontier","manage:local-capability"],"permissions":["read:local-capacity-summaries","request:scale-frontier-preview"],"autonomy_tier":"A2","boundary":PRECLINICAL_BOUNDARY})
}
fn valid_digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().bytes().all(|byte| byte.is_ascii_hexdigit())
}
fn ordered(values: &[String]) -> bool {
    values.windows(2).all(|window| window[0] < window[1])
}
impl IdsCapacityReport9 {
    pub fn validate(&self) -> Result<(), ScaleFrontierError> {
        if self.schema_version != "aurora-research-contract/1.0"
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.workload_order.is_empty()
            || self.effect_order.is_empty()
            || self.effect_receipts.is_empty()
            || !["qualified", "unresolved", "blocked"].contains(&self.disposition.as_str())
        {
            return Err(ScaleFrontierError::Report("capacity identity, workload cells, effects, locality, or disposition is incomplete".into()));
        }
        for values in [
            &self.workload_order,
            &self.selected_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.capacity_exceeded_order,
            &self.budget_exhausted_order,
            &self.unknown_order,
            &self.negative_evidence_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.effect_order,
            &self.effect_receipts,
        ] {
            if !ordered(values) {
                return Err(ScaleFrontierError::Report(
                    "capacity ordering is not canonical".into(),
                ));
            }
        }
        let ids = BTreeSet::from_iter(self.workload_order.iter().cloned());
        let parts = self
            .selected_order
            .iter()
            .chain(&self.unresolved_order)
            .chain(&self.blocked_order)
            .cloned()
            .collect::<Vec<_>>();
        if ids.len() != self.workload_order.len()
            || parts.len() != ids.len()
            || BTreeSet::from_iter(parts) != ids
            || self.latency_milli_order.len() != self.workload_order.len()
            || self.cost_milli_order.len() != self.workload_order.len()
        {
            return Err(ScaleFrontierError::Report(
                "capacity states or measurements do not partition".into(),
            ));
        }
        if !valid_digest(&self.replay_identity)
            || !valid_digest(&self.capacity_digest)
            || self.artifact.content_hash != self.capacity_digest
            || self.artifact.content_type != CONTENT_TYPE
            || self
                .artifact
                .provenance_digests
                .iter()
                .any(|digest| !valid_digest(digest))
        {
            return Err(ScaleFrontierError::Report(
                "capacity digest or artifact metadata is inconsistent".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("preview:ids-capacity-frontier:")
                && !effect.starts_with("manage:local-capability:")
                && effect != "block:unsafe-release"
        }) {
            return Err(ScaleFrontierError::Report(
                "effect is outside the governed scale-frontier gate".into(),
            ));
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<ContentHash, ScaleFrontierError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ScaleFrontierError::Report(error.to_string()))?;
        ContentHash::of_value(&value).map_err(|error| ScaleFrontierError::Report(error.to_string()))
    }
}
fn validate_request(request: &IdsScaleWorkload8) -> Result<(), ScaleFrontierError> {
    if request.workload_id.trim().is_empty()
        || request.capability_id.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.maximum_concurrency == 0
        || request.cells.is_empty()
        || request.cells.len() > MAX_CELLS
        || !valid_digest(&request.replay_identity)
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
    {
        return Err(ScaleFrontierError::Invalid(
            "workload identity, concurrency, cells, replay, locality, or boundary is invalid"
                .into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for cell in &request.cells {
        if cell.cell_id.trim().is_empty()
            || !ids.insert(cell.cell_id.clone())
            || cell.workload_id != request.workload_id
            || cell.concurrency == 0
            || cell.batch_size == 0
            || cell.capacity_limit == 0
            || !valid_digest(&cell.provenance_digest)
            || !valid_digest(&cell.replay_identity)
            || !cell.local
            || !cell.aggregate_only
        {
            return Err(ScaleFrontierError::Invalid(format!(
                "capacity cell {} is invalid, duplicated, non-local, or not digest-bound",
                cell.cell_id
            )));
        }
    }
    Ok(())
}
pub fn preview_ids_scale_frontier(
    request: &IdsScaleWorkload8,
) -> Result<IdsCapacityReport9, ScaleFrontierError> {
    validate_request(request)?;
    let mut cells = request.cells.clone();
    cells.sort_by(|left, right| left.cell_id.cmp(&right.cell_id));
    let workload_order = cells
        .iter()
        .map(|cell| cell.cell_id.clone())
        .collect::<Vec<_>>();
    let mut selected = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut exceeded = BTreeSet::new();
    let mut exhausted = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut latency = BTreeSet::new();
    let mut costs = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    for cell in &cells {
        latency.insert((cell.cell_id.clone(), cell.expected_latency_milli));
        costs.insert((cell.cell_id.clone(), cell.expected_cost_milli));
        provenance.insert(cell.provenance_digest.clone());
        if cell.concurrency > request.maximum_concurrency || cell.concurrency > cell.capacity_limit
        {
            unresolved.insert(cell.cell_id.clone());
            exceeded.insert(cell.cell_id.clone());
            omissions.insert(format!("{}:capacity-exceeded", cell.cell_id));
        } else if cell.expected_cost_milli > request.budget_milli {
            unresolved.insert(cell.cell_id.clone());
            exhausted.insert(cell.cell_id.clone());
            omissions.insert(format!("{}:budget-exhausted", cell.cell_id));
        } else if cell.replay_identity != request.replay_identity {
            unresolved.insert(cell.cell_id.clone());
            uncertainty.insert(format!("{}:replay-identity", cell.cell_id));
        } else if cell.evidence_state == CapacityEvidenceState::Contradicted {
            blocked.insert(cell.cell_id.clone());
            negative.insert(format!("{}:contradicted", cell.cell_id));
        } else if !matches!(
            cell.evidence_state,
            CapacityEvidenceState::Proven | CapacityEvidenceState::Supported
        ) {
            unresolved.insert(cell.cell_id.clone());
            unknown.insert(cell.cell_id.clone());
            uncertainty.insert(format!("{}:evidence-state", cell.cell_id));
        } else {
            selected.insert(cell.cell_id.clone());
        }
    }
    let global = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.raw_data_local
        || !request.aggregate_only;
    if global {
        blocked.extend(workload_order.iter().cloned());
        selected.clear();
        unresolved.clear();
        omissions.insert("request:governance-or-locality-denied".into());
    }
    let selected_order = selected.into_iter().collect::<Vec<_>>();
    let unresolved_order = unresolved.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let disposition = if global || selected_order.is_empty() && unresolved_order.is_empty() {
        "blocked"
    } else if !blocked_order.is_empty() || !unresolved_order.is_empty() {
        "unresolved"
    } else {
        "qualified"
    };
    if disposition != "qualified" {
        omissions.insert("request:scale-frontier-not-closed".into());
    }
    let capacity_exceeded_order = exceeded.into_iter().collect::<Vec<_>>();
    let budget_exhausted_order = exhausted.into_iter().collect::<Vec<_>>();
    let unknown_order = unknown.into_iter().collect::<Vec<_>>();
    let negative_evidence_order = negative.into_iter().collect::<Vec<_>>();
    let omission_order = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty_order = uncertainty.into_iter().collect::<Vec<_>>();
    let latency_order = workload_order
        .iter()
        .map(|id| {
            latency
                .iter()
                .find(|(cell, _)| cell == id)
                .map(|(_, value)| *value)
                .unwrap_or(0)
        })
        .collect::<Vec<_>>();
    let cost_order = workload_order
        .iter()
        .map(|id| {
            costs
                .iter()
                .find(|(cell, _)| cell == id)
                .map(|(_, value)| *value)
                .unwrap_or(0)
        })
        .collect::<Vec<_>>();
    let mut effect_order = if disposition == "qualified" {
        vec![
            "manage:local-capability".to_string(),
            "preview:ids-capacity-frontier".to_string(),
        ]
    } else {
        vec!["block:unsafe-release".to_string()]
    };
    effect_order.sort();
    let effect_receipts = effect_order
        .iter()
        .map(|effect| {
            if effect == "block:unsafe-release" {
                effect.clone()
            } else {
                format!("{effect}:{}", request.workload_id)
            }
        })
        .collect::<Vec<_>>();
    let payload = json!({"schema_version":"aurora-research-contract/1.0","contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.workload_id,"purpose":request.purpose,"semantic_profile":request.semantic_profile,"disposition":disposition,"workload_order":workload_order,"selected_order":selected_order,"unresolved_order":unresolved_order,"blocked_order":blocked_order,"capacity_exceeded_order":capacity_exceeded_order,"budget_exhausted_order":budget_exhausted_order,"unknown_order":unknown_order,"negative_evidence_order":negative_evidence_order,"latency_milli_order":latency_order,"cost_milli_order":cost_order,"omission_order":omission_order,"uncertainty_order":uncertainty_order,"effect_order":effect_order,"replay_identity":request.replay_identity,"raw_data_local":true,"aggregate_only":true,"boundary":PRECLINICAL_BOUNDARY});
    let digest = ContentHash::of_value(&payload)
        .map_err(|error| ScaleFrontierError::Report(error.to_string()))?;
    let receipt = IdsCapacityReport9 {
        schema_version: "aurora-research-contract/1.0".into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.workload_id.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition: disposition.into(),
        workload_order: payload["workload_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        selected_order: payload["selected_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        unresolved_order: payload["unresolved_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        blocked_order: payload["blocked_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        capacity_exceeded_order: payload["capacity_exceeded_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        budget_exhausted_order: payload["budget_exhausted_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        unknown_order: payload["unknown_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        negative_evidence_order: payload["negative_evidence_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        latency_milli_order: latency_order,
        cost_milli_order: cost_order,
        omission_order: payload["omission_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        uncertainty_order: payload["uncertainty_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        effect_order: payload["effect_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
        replay_identity: request.replay_identity.clone(),
        capacity_digest: digest.clone(),
        artifact: IdsCapacityReport9Artifact {
            artifact_id: format!("ids-capacity-report-9:{}", request.workload_id),
            content_type: CONTENT_TYPE.into(),
            content_hash: digest,
            semantic_loss: payload["omission_order"]
                .as_array()
                .unwrap()
                .iter()
                .filter_map(|v| v.as_str().map(str::to_owned))
                .collect(),
            provenance_digests: provenance.into_iter().collect(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        },
        effect_receipts,
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
    fn cell(id: &str) -> IdsScaleCell7 {
        IdsScaleCell7 {
            cell_id: id.into(),
            workload_id: "workload:scale".into(),
            concurrency: 2,
            batch_size: 16,
            expected_latency_milli: 100,
            expected_cost_milli: 10,
            capacity_limit: 4,
            evidence_state: CapacityEvidenceState::Supported,
            provenance_digest: h("p"),
            replay_identity: h("r"),
            local: true,
            aggregate_only: true,
        }
    }
    fn request() -> IdsScaleWorkload8 {
        IdsScaleWorkload8 {
            workload_id: "workload:scale".into(),
            capability_id: "ids.compute".into(),
            purpose: "preview".into(),
            semantic_profile: "ome-v1".into(),
            budget_milli: 100,
            maximum_concurrency: 4,
            cells: vec![cell("cell:a"), cell("cell:b")],
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
    fn manifest_is_a2() {
        assert_eq!(scale_frontier_manifest()["autonomy_tier"], "A2");
    }
    #[test]
    fn nominal_is_qualified() {
        assert_eq!(
            preview_ids_scale_frontier(&request()).unwrap().disposition,
            "qualified"
        );
    }
    #[test]
    fn capacity_overflow_is_unresolved() {
        let mut q = request();
        q.cells[0].concurrency = 9;
        assert_eq!(
            preview_ids_scale_frontier(&q).unwrap().disposition,
            "unresolved"
        );
    }
    #[test]
    fn budget_exhaustion_is_unresolved() {
        let mut q = request();
        q.cells[0].expected_cost_milli = 101;
        assert_eq!(
            preview_ids_scale_frontier(&q).unwrap().disposition,
            "unresolved"
        );
    }
    #[test]
    fn policy_denial_blocks() {
        let mut q = request();
        q.policy_allow = false;
        assert_eq!(
            preview_ids_scale_frontier(&q).unwrap().disposition,
            "blocked"
        );
    }
    #[test]
    fn digest_is_deterministic() {
        let a = preview_ids_scale_frontier(&request()).unwrap();
        let b = preview_ids_scale_frontier(&request()).unwrap();
        assert_eq!(a.digest().unwrap(), b.digest().unwrap());
    }
}
