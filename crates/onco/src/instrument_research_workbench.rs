//! OncoWorld-aware laboratory instrument research workbench.
//!
//! Atlas feature `AFA-onco-P11-F20`.  The workbench is a researcher-facing admission and
//! visualization boundary for preclinical instrument actions.  It joins actions to institution-
//! local tumour-worldline/specimen identifiers, exposes every ethical and provenance gate, and
//! emits a replayable receipt.  It never contacts hardware, dispatches a protocol, or makes a
//! clinical decision.

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, EvidenceReference, EvidenceState,
    LossSeverity, ProvenanceLink, ResearchSurface, SemanticLoss, TypedPort, TypedResearchArtifact,
    PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-onco-P11-F20";
pub const CONTRACT_VERSION: &str = "onco-federated-continual-laboratory-integration-research-workbench/1.0";
pub const INPUT_SCHEMA: &str = "OncoInstrumentActionRequest5@1";
pub const OUTPUT_SCHEMA: &str = "OncoInstrumentReceipt5@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.onco-instrument-receipt-5+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OncoInstrumentAction5 {
    pub action_id: String,
    pub endpoint_id: String,
    pub capability_id: String,
    pub worldline_id: String,
    pub specimen_id: String,
    pub modality: String,
    pub protocol_version: String,
    pub parameters_digest: ContentHash,
    pub evidence_digest: Option<ContentHash>,
    pub provenance_digest: Option<ContentHash>,
    pub replay_identity: ContentHash,
    pub evidence_state: EvidenceState,
    pub world_scope: bool,
    pub scope_compatible: bool,
    pub permitted: bool,
    pub instrument_ready: bool,
    pub consented_preclinical: bool,
    pub local_only: bool,
    pub omission_order: Vec<String>,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OncoInstrumentRequest6 {
    pub schema_version: String,
    pub request_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_action_order: Vec<String>,
    pub required_endpoint_order: Vec<String>,
    pub required_capability_order: Vec<String>,
    pub actions: Vec<OncoInstrumentAction5>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub researcher_authorized: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub adversarial_events: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OncoInstrumentDisposition {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OncoInstrumentReceipt5 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub disposition: OncoInstrumentDisposition,
    pub action_order: Vec<String>,
    pub selected_action_order: Vec<String>,
    pub unresolved_action_order: Vec<String>,
    pub blocked_action_order: Vec<String>,
    pub missing_action_order: Vec<String>,
    pub endpoint_order: Vec<String>,
    pub capability_order: Vec<String>,
    pub selected_endpoint_order: Vec<String>,
    pub selected_capability_order: Vec<String>,
    pub missing_endpoint_order: Vec<String>,
    pub missing_capability_order: Vec<String>,
    pub worldline_order: Vec<String>,
    pub specimen_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub selected_worldline_order: Vec<String>,
    pub selected_specimen_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub action_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum OncoInstrumentError {
    #[error("invalid OncoWorld instrument request: {0}")]
    Invalid(String),
    #[error("OncoWorld instrument artifact failed: {0}")]
    Artifact(String),
}

fn invalid(message: impl Into<String>) -> OncoInstrumentError { OncoInstrumentError::Invalid(message.into()) }
fn canonical(values: &[String]) -> bool { values.windows(2).all(|pair| pair[0] < pair[1]) }
fn valid_digest(value: &ContentHash) -> bool { value.as_str().len() == 64 && value.as_str().bytes().all(|b| b.is_ascii_hexdigit()) }

impl OncoInstrumentReceipt5 {
    pub fn validate(&self) -> Result<(), OncoInstrumentError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION || self.contract_version != CONTRACT_VERSION || self.feature_id != FEATURE_ID || self.boundary != PRECLINICAL_BOUNDARY || self.artifact.boundary != PRECLINICAL_BOUNDARY || !self.raw_data_local || !self.aggregate_only || self.request_id.trim().is_empty() || self.requester.trim().is_empty() || self.purpose.trim().is_empty() || self.semantic_profile.trim().is_empty() || self.action_order.is_empty() || self.endpoint_order.is_empty() || self.capability_order.is_empty() || self.worldline_order.is_empty() || self.specimen_order.is_empty() || self.effect_receipts.is_empty() { return Err(invalid("OncoWorld instrument identity, axes, locality, boundary, or effects are incomplete")); }
        for values in [&self.action_order, &self.selected_action_order, &self.unresolved_action_order, &self.blocked_action_order, &self.missing_action_order, &self.endpoint_order, &self.capability_order, &self.selected_endpoint_order, &self.selected_capability_order, &self.missing_endpoint_order, &self.missing_capability_order, &self.worldline_order, &self.specimen_order, &self.modality_order, &self.selected_worldline_order, &self.selected_specimen_order, &self.omission_order, &self.uncertainty_order, &self.negative_evidence_order, &self.effect_receipts] { if !canonical(values) { return Err(invalid("OncoWorld instrument ordering is not canonical")); } }
        let all = self.action_order.iter().cloned().collect::<BTreeSet<_>>();
        let parts = self.selected_action_order.iter().chain(self.unresolved_action_order.iter()).chain(self.blocked_action_order.iter()).chain(self.missing_action_order.iter()).cloned().collect::<BTreeSet<_>>();
        if all.len() != self.action_order.len() || parts != all { return Err(invalid("instrument action states do not form a complete partition")); }
        if !valid_digest(&self.replay_identity) || !valid_digest(&self.action_digest) || self.artifact.content_hash != self.action_digest { return Err(invalid("instrument replay or action digest is invalid")); }
        self.artifact.validate_metadata().map_err(|e| OncoInstrumentError::Artifact(e.to_string()))?;
        if self.artifact.content_type != CONTENT_TYPE { return Err(invalid("OncoWorld instrument artifact type is invalid")); }
        if self.effect_receipts.iter().any(|effect| !effect.starts_with("view:authorized-research-state:") && effect != "block:unsafe-release") { return Err(invalid("effect is outside the researcher interaction gate")); }
        if self.disposition == OncoInstrumentDisposition::Qualified && self.effect_receipts != [format!("view:authorized-research-state:{}", self.request_id)] { return Err(invalid("qualified instrument effect is invalid")); }
        if self.disposition != OncoInstrumentDisposition::Qualified && self.effect_receipts != ["block:unsafe-release"] { return Err(invalid("non-qualified instrument interaction must block")); }
        Ok(())
    }
    pub fn digest(&self) -> Result<ContentHash, OncoInstrumentError> { self.validate()?; ContentHash::of_value(&serde_json::to_value(self).map_err(|e| OncoInstrumentError::Artifact(e.to_string()))?).map_err(|e| OncoInstrumentError::Artifact(e.to_string())) }
}

pub fn instrument_research_workbench_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "onco".into(), consumers: ["benchmark curator".into(), "assay-domain scientist".into(), "researcher workbench".into()].into(), behavior: "presents typed OncoWorld instrument-action attestations with deterministic safety and provenance partitions without contacting hardware".into(), value: "lets researchers inspect which preclinical assay actions are authorized, unresolved, or blocked while keeping worldline/specimen context and omissions visible".into(), inputs: vec![TypedPort { name: "onco_instrument_action_request".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "onco_instrument_receipt".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: BTreeSet::new(), permissions: BTreeSet::from(["view:authorized-research-state".into()]), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "mcp-2025-06-18".into(), state: EvidenceState::Supported, locator: Some("https://modelcontextprotocol.io/specification/2025-06-18/basic/index".into()) }, EvidenceReference { source_id: "ga4gh-wes".into(), state: EvidenceState::Supported, locator: Some("https://ga4gh.github.io/workflow-execution-service-schemas/docs/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn qualify_instrument_actions(request: &OncoInstrumentRequest6) -> Result<OncoInstrumentReceipt5, OncoInstrumentError> {
    validate_request(request)?;
    let mut actions = request.actions.clone();
    actions.sort_by(|a, b| a.endpoint_id.cmp(&b.endpoint_id).then(a.capability_id.cmp(&b.capability_id)).then(a.action_id.cmp(&b.action_id)));
    let action_order = actions.iter().map(|a| a.action_id.clone()).collect::<Vec<_>>();
    let mut selected = BTreeSet::new(); let mut unresolved = BTreeSet::new(); let mut blocked = BTreeSet::new(); let mut omission = BTreeSet::new(); let mut uncertainty = BTreeSet::new(); let mut negative = BTreeSet::new(); let mut selected_worldlines = BTreeSet::new(); let mut selected_specimens = BTreeSet::new(); let mut selected_endpoints = BTreeSet::new(); let mut selected_capabilities = BTreeSet::new();
    for action in &actions {
        if !action.world_scope || !action.permitted || !action.instrument_ready || !action.consented_preclinical || !action.local_only { blocked.insert(action.action_id.clone()); omission.insert(format!("{}:scope-permission-readiness", action.action_id)); }
        else if action.replay_identity != request.replay_identity || !action.scope_compatible || !matches!(action.evidence_state, EvidenceState::Proven | EvidenceState::Supported) || action.evidence_digest.is_none() || action.provenance_digest.is_none() { unresolved.insert(action.action_id.clone()); if action.replay_identity != request.replay_identity { uncertainty.insert(format!("{}:replay-mismatch", action.action_id)); } if !action.scope_compatible { uncertainty.insert(format!("{}:scope-incompatible", action.action_id)); } if !matches!(action.evidence_state, EvidenceState::Proven | EvidenceState::Supported) { uncertainty.insert(format!("{}:evidence-state", action.action_id)); } if action.evidence_digest.is_none() || action.provenance_digest.is_none() { omission.insert(format!("{}:evidence-or-provenance-missing", action.action_id)); } }
        else { selected.insert(action.action_id.clone()); selected_worldlines.insert(action.worldline_id.clone()); selected_specimens.insert(action.specimen_id.clone()); selected_endpoints.insert(action.endpoint_id.clone()); selected_capabilities.insert(action.capability_id.clone()); }
        omission.extend(action.omission_order.iter().map(|item| format!("{}:{item}", action.action_id))); if action.negative_result { negative.insert(format!("{}:negative-result", action.action_id)); }
    }
    let endpoint_order = actions.iter().map(|a| a.endpoint_id.clone()).collect::<BTreeSet<_>>(); let capability_order = actions.iter().map(|a| a.capability_id.clone()).collect::<BTreeSet<_>>(); let worldline_order = actions.iter().map(|a| a.worldline_id.clone()).collect::<BTreeSet<_>>(); let specimen_order = actions.iter().map(|a| a.specimen_id.clone()).collect::<BTreeSet<_>>(); let modality_order = actions.iter().map(|a| a.modality.clone()).collect::<BTreeSet<_>>();
    let required_actions = request.required_action_order.iter().cloned().collect::<BTreeSet<_>>(); let missing_actions = required_actions.difference(&action_order.iter().cloned().collect()).cloned().collect::<BTreeSet<_>>(); let missing_endpoints = request.required_endpoint_order.iter().cloned().collect::<BTreeSet<_>>().difference(&endpoint_order).cloned().collect::<BTreeSet<_>>(); let missing_capabilities = request.required_capability_order.iter().cloned().collect::<BTreeSet<_>>().difference(&capability_order).cloned().collect::<BTreeSet<_>>(); omission.extend(missing_actions.iter().map(|id| format!("action:{id}:missing"))); omission.extend(missing_endpoints.iter().map(|id| format!("endpoint:{id}:missing"))); omission.extend(missing_capabilities.iter().map(|id| format!("capability:{id}:missing"))); uncertainty.extend(request.adversarial_events.iter().map(|event| format!("adversarial:{event}")));
    let global_block = !request.policy_allow || !request.protected_closure || !request.signed_approval || !request.researcher_authorized || !request.raw_data_local || !request.aggregate_only || !request.adversarial_events.is_empty(); if global_block { blocked.extend(action_order.iter().cloned()); selected.clear(); unresolved.clear(); omission.insert("request:instrument-release-gate-blocked".into()); }
    let disposition = if global_block { OncoInstrumentDisposition::Blocked } else if selected.is_empty() || !missing_actions.is_empty() || !missing_endpoints.is_empty() || !missing_capabilities.is_empty() { OncoInstrumentDisposition::Unresolved } else { OncoInstrumentDisposition::Qualified }; if disposition != OncoInstrumentDisposition::Qualified { omission.insert("request:instrument-interaction-not-release-ready".into()); }
    let selected_action_order = selected.into_iter().collect::<Vec<_>>(); let unresolved_action_order = unresolved.into_iter().collect::<Vec<_>>(); let blocked_action_order = blocked.into_iter().collect::<Vec<_>>(); let effect_receipts = if disposition == OncoInstrumentDisposition::Qualified { vec![format!("view:authorized-research-state:{}", request.request_id)] } else { vec!["block:unsafe-release".into()] };
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "requester": request.requester, "purpose": request.purpose, "semantic_profile": request.semantic_profile, "disposition": disposition, "action_order": action_order, "selected_action_order": selected_action_order, "unresolved_action_order": unresolved_action_order, "blocked_action_order": blocked_action_order, "missing_action_order": missing_actions, "endpoint_order": endpoint_order, "capability_order": capability_order, "worldline_order": worldline_order, "specimen_order": specimen_order, "modality_order": modality_order, "selected_endpoint_order": selected_endpoints, "selected_capability_order": selected_capabilities, "selected_worldline_order": selected_worldlines, "selected_specimen_order": selected_specimens, "missing_endpoint_order": missing_endpoints, "missing_capability_order": missing_capabilities, "omission_order": omission, "uncertainty_order": uncertainty, "negative_evidence_order": negative, "effect_receipts": effect_receipts, "raw_data_local": request.raw_data_local, "aggregate_only": request.aggregate_only, "boundary": PRECLINICAL_BOUNDARY});
    let action_digest = ContentHash::of_value(&payload).map_err(|e| OncoInstrumentError::Artifact(e.to_string()))?; let artifact = TypedResearchArtifact::from_payload(format!("onco-instrument-receipt:{}", request.request_id), CONTENT_TYPE, &payload, vec![SemanticLoss { field: "omission_order".into(), reason: "unmeasured and incomplete instrument evidence remains visible".into(), severity: LossSeverity::DecisionRelevant }], vec![ProvenanceLink { source_id: request.request_id.clone(), relation: "qualified-oncoworld-instrument-attestations".into(), digest: action_digest.clone() }]).map_err(|e| OncoInstrumentError::Artifact(e.to_string()))?;
    let receipt = OncoInstrumentReceipt5 { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), contract_version: CONTRACT_VERSION.into(), feature_id: FEATURE_ID.into(), request_id: request.request_id.clone(), requester: request.requester.clone(), purpose: request.purpose.clone(), semantic_profile: request.semantic_profile.clone(), disposition, action_order: payload["action_order"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().into()).collect(), selected_action_order: payload["selected_action_order"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().into()).collect(), unresolved_action_order: payload["unresolved_action_order"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().into()).collect(), blocked_action_order: payload["blocked_action_order"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().into()).collect(), missing_action_order: payload["missing_action_order"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().into()).collect(), endpoint_order: payload["endpoint_order"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().into()).collect(), capability_order: payload["capability_order"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().into()).collect(), selected_endpoint_order: payload["selected_endpoint_order"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().into()).collect(), selected_capability_order: payload["selected_capability_order"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().into()).collect(), missing_endpoint_order: payload["missing_endpoint_order"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().into()).collect(), missing_capability_order: payload["missing_capability_order"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().into()).collect(), worldline_order: payload["worldline_order"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().into()).collect(), specimen_order: payload["specimen_order"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().into()).collect(), modality_order: payload["modality_order"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().into()).collect(), selected_worldline_order: payload["selected_worldline_order"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().into()).collect(), selected_specimen_order: payload["selected_specimen_order"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().into()).collect(), omission_order: payload["omission_order"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().into()).collect(), uncertainty_order: payload["uncertainty_order"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().into()).collect(), negative_evidence_order: payload["negative_evidence_order"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().into()).collect(), replay_identity: request.replay_identity.clone(), action_digest, artifact, effect_receipts: payload["effect_receipts"].as_array().unwrap().iter().map(|v| v.as_str().unwrap().into()).collect(), raw_data_local: request.raw_data_local, aggregate_only: request.aggregate_only, boundary: PRECLINICAL_BOUNDARY.into() }; receipt.validate()?; Ok(receipt)
}

fn validate_request(request: &OncoInstrumentRequest6) -> Result<(), OncoInstrumentError> {
    if request.schema_version != INPUT_SCHEMA || request.request_id.trim().is_empty() || request.requester.trim().is_empty() || request.purpose.trim().is_empty() || request.semantic_profile.trim().is_empty() || request.required_action_order.is_empty() || request.required_endpoint_order.is_empty() || request.required_capability_order.is_empty() || request.actions.is_empty() || !canonical(&request.required_action_order) || !canonical(&request.required_endpoint_order) || !canonical(&request.required_capability_order) || !canonical(&request.adversarial_events) || !valid_digest(&request.replay_identity) || !request.raw_data_local || !request.aggregate_only || request.boundary != PRECLINICAL_BOUNDARY { return Err(invalid("OncoWorld instrument request identity, closure, replay, locality, or boundary is invalid")); }
    let mut ids = BTreeSet::new(); for action in &request.actions { if action.action_id.trim().is_empty() || action.endpoint_id.trim().is_empty() || action.capability_id.trim().is_empty() || action.worldline_id.trim().is_empty() || action.specimen_id.trim().is_empty() || action.modality.trim().is_empty() || action.protocol_version.trim().is_empty() || !ids.insert(action.action_id.clone()) || !valid_digest(&action.parameters_digest) || !valid_digest(&action.replay_identity) || !action.evidence_digest.as_ref().is_none_or(valid_digest) || !action.provenance_digest.as_ref().is_none_or(valid_digest) || !canonical(&action.omission_order) { return Err(invalid(format!("OncoWorld instrument action {} is malformed or duplicated", action.action_id))); } }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash { ContentHash::of_bytes(value.as_bytes()) }
    fn request() -> OncoInstrumentRequest6 {
        let action = |id: &str| OncoInstrumentAction5 { action_id: id.into(), endpoint_id: format!("endpoint:{id}"), capability_id: format!("assay:{id}"), worldline_id: format!("worldline:{id}"), specimen_id: format!("specimen:{id}"), modality: "methylation".into(), protocol_version: "v1".into(), parameters_digest: hash(id), evidence_digest: Some(hash("evidence")), provenance_digest: Some(hash("provenance")), replay_identity: hash("replay"), evidence_state: EvidenceState::Supported, world_scope: true, scope_compatible: true, permitted: true, instrument_ready: true, consented_preclinical: true, local_only: true, omission_order: Vec::new(), negative_result: false };
        OncoInstrumentRequest6 { schema_version: INPUT_SCHEMA.into(), request_id: "onco-instrument-request".into(), requester: "benchmark-curator".into(), purpose: "preclinical assay benchmark".into(), semantic_profile: "onco-instrument:v1".into(), required_action_order: vec!["action:a".into(), "action:b".into()], required_endpoint_order: vec!["endpoint:action:a".into(), "endpoint:action:b".into()], required_capability_order: vec!["assay:action:a".into(), "assay:action:b".into()], actions: vec![action("action:a"), action("action:b")], replay_identity: hash("replay"), policy_allow: true, protected_closure: true, signed_approval: true, researcher_authorized: true, raw_data_local: true, aggregate_only: true, adversarial_events: Vec::new(), boundary: PRECLINICAL_BOUNDARY.into() }
    }
    #[test] fn manifest_is_a1() { assert_eq!(instrument_research_workbench_manifest().autonomy_tier, AutonomyTier::A1); }
    #[test] fn qualified_actions_are_deterministic() { let receipt = qualify_instrument_actions(&request()).unwrap(); assert_eq!(receipt.disposition, OncoInstrumentDisposition::Qualified); assert_eq!(receipt.selected_action_order.len(), 2); assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap()); }
    #[test] fn missing_action_is_unresolved() { let mut req = request(); req.actions.pop(); let receipt = qualify_instrument_actions(&req).unwrap(); assert_eq!(receipt.disposition, OncoInstrumentDisposition::Unresolved); assert!(receipt.missing_action_order.contains(&"action:b".into())); }
    #[test] fn denied_action_is_blocked_but_batch_unresolved() { let mut req = request(); req.actions[0].permitted = false; let receipt = qualify_instrument_actions(&req).unwrap(); assert!(receipt.blocked_action_order.contains(&"action:a".into())); assert_eq!(receipt.disposition, OncoInstrumentDisposition::Unresolved); }
    #[test] fn adversarial_request_blocks_all() { let mut req = request(); req.adversarial_events = vec!["prompt-injection".into()]; let receipt = qualify_instrument_actions(&req).unwrap(); assert_eq!(receipt.disposition, OncoInstrumentDisposition::Blocked); assert!(receipt.selected_action_order.is_empty()); }
    #[test] fn negative_result_is_preserved() { let mut req = request(); req.actions[0].negative_result = true; let receipt = qualify_instrument_actions(&req).unwrap(); assert!(receipt.negative_evidence_order.contains(&"action:a:negative-result".into())); }
    #[test] fn replay_mismatch_is_unresolved() { let mut req = request(); req.actions[0].replay_identity = hash("other"); let receipt = qualify_instrument_actions(&req).unwrap(); assert!(receipt.unresolved_action_order.contains(&"action:a".into())); assert!(receipt.uncertainty_order.contains(&"action:a:replay-mismatch".into())); }
}
