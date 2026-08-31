//! Shared operations/federation capability kernel for Worldgen P01 F29–F32.
//!
//! The kernel is deliberately side-effect free: it admits caller-supplied event summaries,
//! emits deterministic telemetry and recovery receipts, and never starts an instrument,
//! contacts a remote peer, or exports raw preclinical data.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const SCHEMA_VERSION: &str = "aurora-research-contract/1.0";
pub const BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationsEvent {
    pub event_id: String,
    pub evidence_state: String,
    pub provenance_digest: ContentHash,
    pub permitted: bool,
    pub retryable: bool,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationsRequest {
    pub request_id: String,
    pub operator: String,
    pub scope: String,
    pub scale: String,
    pub input_schema: String,
    pub output_schema: String,
    pub events: Vec<OperationsEvent>,
    pub capacity: u64,
    pub budget_units: u64,
    pub requested_units: u64,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_approved: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OperationsDisposition {
    Qualified,
    Partial,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OperationsReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub operator: String,
    pub scope: String,
    pub scale: String,
    pub input_schema: String,
    pub output_schema: String,
    pub disposition: OperationsDisposition,
    pub event_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub recovery_order: Vec<String>,
    pub telemetry_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub consumed_units: u64,
    pub capacity: u64,
    pub budget_units: u64,
    pub replay_identity: ContentHash,
    pub capability_digest: ContentHash,
    pub artifact_digest: ContentHash,
    pub effect_receipts: Vec<String>,
    pub artifact: serde_json::Value,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum OperationsError {
    #[error("invalid operations request: {0}")]
    Invalid(String),
    #[error("invalid operations receipt: {0}")]
    Receipt(String),
    #[error("operations artifact failed: {0}")]
    Artifact(String),
}

fn ordered(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}
fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().bytes().all(|b| b.is_ascii_hexdigit())
}

impl OperationsReceipt {
    pub fn validate(&self) -> Result<(), OperationsError> {
        if self.schema_version != SCHEMA_VERSION
            || self.boundary != BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.operator.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.scale.trim().is_empty()
            || self.input_schema.trim().is_empty()
            || self.output_schema.trim().is_empty()
            || self.event_order.is_empty()
            || self.effect_receipts.is_empty()
            || self.capacity == 0
            || self.budget_units == 0
        {
            return Err(OperationsError::Receipt("operations identity, locality, capacity, budget, events, or effects are incomplete".into()));
        }
        for values in [
            &self.event_order,
            &self.admitted_order,
            &self.blocked_order,
            &self.unknown_order,
            &self.recovery_order,
            &self.telemetry_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if !ordered(values) {
                return Err(OperationsError::Receipt(
                    "operations ordering is not canonical".into(),
                ));
            }
        }
        let ids = self.event_order.iter().cloned().collect::<BTreeSet<_>>();
        let parts = self
            .admitted_order
            .iter()
            .chain(&self.blocked_order)
            .chain(&self.unknown_order)
            .cloned()
            .collect::<Vec<_>>();
        if ids.len() != self.event_order.len()
            || parts.len() != ids.len()
            || parts.iter().cloned().collect::<BTreeSet<_>>() != ids
        {
            return Err(OperationsError::Receipt(
                "operations event states do not partition".into(),
            ));
        }
        if self.consumed_units > self.budget_units
            || self.consumed_units > self.capacity
            || !digest(&self.replay_identity)
            || !digest(&self.capability_digest)
            || !digest(&self.artifact_digest)
        {
            return Err(OperationsError::Receipt(
                "operations budget, capacity, replay, or capability digest is invalid".into(),
            ));
        }
        let artifact = self
            .artifact
            .get("content_hash")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if artifact != self.artifact_digest.as_str()
            || self.artifact.get("boundary").and_then(|v| v.as_str()) != Some(BOUNDARY)
        {
            return Err(OperationsError::Receipt(
                "operations artifact digest or boundary is inconsistent".into(),
            ));
        }
        if self.effect_receipts.iter().any(|e| {
            !e.starts_with("telemetry:operations:")
                && !e.starts_with("recover:operations:")
                && !e.starts_with("exchange:permitted-summaries:")
                && e != "block:unsafe-release"
        }) {
            return Err(OperationsError::Receipt(
                "effect is outside the operations/federation gate".into(),
            ));
        }
        Ok(())
    }
    pub fn digest(&self) -> Result<ContentHash, OperationsError> {
        self.validate()?;
        let value =
            serde_json::to_value(self).map_err(|e| OperationsError::Receipt(e.to_string()))?;
        ContentHash::of_value(&value).map_err(|e| OperationsError::Receipt(e.to_string()))
    }
}

pub fn manifest(
    feature_id: &str,
    contract_version: &str,
    input_schema: &str,
    output_schema: &str,
    scale: &str,
    autonomy_tier: &str,
) -> serde_json::Value {
    json!({"schema_version":SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"worldgen","consumers":["benchmark curator","research program lead","preclinical neuroscientist","bioinformatician"],"behavior":format!("operate a typed {scale} evidence stream with deterministic telemetry, recovery, capacity, policy, and federation receipts"),"value":"keeps institution-local research operations observable, replayable, and fail-closed","inputs":{"name":"evidence_feed","schema":input_schema},"outputs":{"name":"qualified_evidence_set","schema":output_schema},"effects":["telemetry:operations","recover:operations","exchange:permitted-summaries","block:unsafe-release"],"permissions":["operate:institution-node"],"determinism":"byte_stable","autonomy_tier":autonomy_tier,"boundary":BOUNDARY})
}

pub fn operate(
    request: &OperationsRequest,
    feature_id: &str,
    contract_version: &str,
) -> Result<OperationsReceipt, OperationsError> {
    if request.request_id.trim().is_empty()
        || request.operator.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.scale.trim().is_empty()
        || request.events.is_empty()
        || request.events.len() as u64 > request.capacity
        || request.capacity == 0
        || request.requested_units == 0
        || request.requested_units > request.budget_units
        || !digest(&request.replay_identity)
        || request.boundary != BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
    {
        return Err(OperationsError::Invalid("operations request identity, event bound, capacity, budget, replay, locality, or boundary is invalid".into()));
    }
    let mut events = request.events.clone();
    events.sort_by(|a, b| a.event_id.cmp(&b.event_id));
    if events.windows(2).any(|p| p[0].event_id == p[1].event_id)
        || events
            .iter()
            .any(|e| e.event_id.trim().is_empty() || !digest(&e.provenance_digest))
    {
        return Err(OperationsError::Invalid(
            "operations events must be uniquely identified with valid provenance".into(),
        ));
    }
    let event_order = events
        .iter()
        .map(|e| e.event_id.clone())
        .collect::<Vec<_>>();
    let mut admitted: BTreeSet<String> = BTreeSet::new();
    let mut blocked: BTreeSet<String> = BTreeSet::new();
    let mut unknown: BTreeSet<String> = BTreeSet::new();
    let mut recovery: BTreeSet<String> = BTreeSet::new();
    let mut omissions: BTreeSet<String> = BTreeSet::new();
    let mut uncertainty: BTreeSet<String> = BTreeSet::new();
    let mut negative: BTreeSet<String> = BTreeSet::new();
    let base_allow = request.policy_allow
        && request.protected_closure
        && request.signed_approval
        && (request.scale == "local single-study" || request.federation_approved)
        && request.raw_data_local
        && request.aggregate_only;
    for event in &events {
        let supported = matches!(event.evidence_state.as_str(), "proven" | "supported")
            && event.permitted
            && base_allow;
        if supported {
            admitted.insert(event.event_id.clone());
        } else {
            blocked.insert(event.event_id.clone());
            if matches!(
                event.evidence_state.as_str(),
                "unknown" | "unmeasured" | "speculative"
            ) {
                unknown.insert(event.event_id.clone());
                uncertainty.insert(format!("event:{}:evidence-unresolved", event.event_id));
            }
            if event.retryable {
                recovery.insert(format!("event:{}:retryable-recovery", event.event_id));
            }
            if event.negative_result {
                negative.insert(format!("event:{}:negative-result-retained", event.event_id));
            }
        }
    }
    if !request.policy_allow {
        omissions.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.signed_approval {
        omissions.insert("request:signed-approval-missing".into());
    }
    if request.scale != "local single-study" && !request.federation_approved {
        omissions.insert("request:federation-approval-missing".into());
    }
    if request.requested_units > request.capacity {
        omissions.insert("request:capacity-exceeded".into());
    }
    let disposition = if !base_allow || request.requested_units > request.capacity {
        OperationsDisposition::Blocked
    } else if blocked.is_empty()
        && unknown.is_empty()
        && omissions.is_empty()
        && uncertainty.is_empty()
        && negative.is_empty()
    {
        OperationsDisposition::Qualified
    } else {
        OperationsDisposition::Partial
    };
    let consumed_units = request
        .requested_units
        .min(request.capacity)
        .min(request.budget_units);
    let capability_digest = ContentHash::of_value(&json!({"feature_id":feature_id,"contract_version":contract_version,"scale":request.scale,"input_schema":request.input_schema,"output_schema":request.output_schema})).map_err(|e| OperationsError::Artifact(e.to_string()))?;
    let telemetry_order = vec![
        format!("telemetry:operations:events:{}", event_order.len()),
        format!("telemetry:operations:units:{}", consumed_units),
    ];
    let mut effect_receipts = if disposition == OperationsDisposition::Qualified {
        vec![
            format!("telemetry:operations:{}", request.request_id),
            format!("exchange:permitted-summaries:{}", request.request_id),
        ]
    } else if !recovery.is_empty() {
        vec![
            format!("recover:operations:{}", request.request_id),
            "block:unsafe-release".into(),
        ]
    } else {
        vec!["block:unsafe-release".into()]
    };
    effect_receipts.sort();
    let payload = json!({"schema_version":SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"request_id":request.request_id,"scale":request.scale,"event_order":event_order,"admitted_order":admitted,"blocked_order":blocked,"unknown_order":unknown,"recovery_order":recovery,"telemetry_order":telemetry_order,"omission_order":omissions,"uncertainty_order":uncertainty,"negative_evidence_order":negative,"consumed_units":consumed_units,"capacity":request.capacity,"budget_units":request.budget_units,"replay_identity":request.replay_identity,"capability_digest":capability_digest,"boundary":BOUNDARY});
    let artifact_digest =
        ContentHash::of_value(&payload).map_err(|e| OperationsError::Artifact(e.to_string()))?;
    let receipt = OperationsReceipt {
        schema_version: SCHEMA_VERSION.into(),
        contract_version: contract_version.into(),
        feature_id: feature_id.into(),
        request_id: request.request_id.clone(),
        operator: request.operator.clone(),
        scope: request.scope.clone(),
        scale: request.scale.clone(),
        input_schema: request.input_schema.clone(),
        output_schema: request.output_schema.clone(),
        disposition,
        event_order: payload["event_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        admitted_order: sorted(payload["admitted_order"].as_array().unwrap()),
        blocked_order: sorted(payload["blocked_order"].as_array().unwrap()),
        unknown_order: sorted(payload["unknown_order"].as_array().unwrap()),
        recovery_order: sorted(payload["recovery_order"].as_array().unwrap()),
        telemetry_order,
        omission_order: sorted(payload["omission_order"].as_array().unwrap()),
        uncertainty_order: sorted(payload["uncertainty_order"].as_array().unwrap()),
        negative_evidence_order: sorted(payload["negative_evidence_order"].as_array().unwrap()),
        consumed_units,
        capacity: request.capacity,
        budget_units: request.budget_units,
        replay_identity: request.replay_identity.clone(),
        capability_digest,
        artifact_digest: artifact_digest.clone(),
        effect_receipts,
        artifact: json!({"artifact_id":format!("operations:{}",request.request_id),"content_type":"application/vnd.aurora.worldgen.operations-receipt+json","content_hash":artifact_digest,"boundary":BOUNDARY}),
        raw_data_local: true,
        aggregate_only: true,
        boundary: BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn sorted(values: &[serde_json::Value]) -> Vec<String> {
    let mut out = values
        .iter()
        .filter_map(|v| v.as_str().map(str::to_string))
        .collect::<Vec<_>>();
    out.sort();
    out.dedup();
    out
}
