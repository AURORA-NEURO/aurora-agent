//! Federated context-compilation control planes (P03 F29-F32).
//!
//! The control plane admits digest-only attestations from research sites, ranks them
//! deterministically, and emits a governance receipt.  It does not aggregate raw observations,
//! contact a site, or authorize physical/clinical actions.

use std::collections::BTreeSet;

use bioprism_foundation::{PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

pub const CONTENT_TYPE: &str =
    "application/vnd.aurora.worldgen.context-control-plane-receipt+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextControlAttestation {
    pub attestation_id: String,
    pub site_id: String,
    pub context_digest: ContentHash,
    pub support_milli: u16,
    pub freshness_milli: u16,
    pub evidence_state: String,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub permitted: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextControlPlaneRequest {
    pub program_id: String,
    pub attestations: Vec<ContextControlAttestation>,
    pub minimum_support_milli: u16,
    pub minimum_freshness_milli: u16,
    pub minimum_site_quorum: u16,
    pub requested_action_order: Vec<String>,
    pub action_budget: u64,
    pub signed_approval: bool,
    pub federation_approved: bool,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextControlPlaneReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub program_id: String,
    pub disposition: String,
    pub candidate_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub site_order: Vec<String>,
    pub action_order: Vec<String>,
    pub admitted_action_order: Vec<String>,
    pub denied_action_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub control_digest: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: serde_json::Value,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum ContextControlPlaneError {
    #[error("invalid context control-plane request: {0}")]
    Invalid(String),
    #[error("context control-plane artifact failed: {0}")]
    Artifact(String),
}

fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().bytes().all(|byte| byte.is_ascii_hexdigit())
}
fn ordered(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}
fn sorted(values: &[String]) -> Vec<String> {
    let mut output = values.to_vec(); output.sort(); output.dedup(); output
}

impl ContextControlPlaneReceipt {
    pub fn validate(&self) -> Result<(), ContextControlPlaneError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.get("boundary").and_then(|value| value.as_str()) != Some(PRECLINICAL_BOUNDARY)
            || self.artifact.get("content_type").and_then(|value| value.as_str()) != Some(CONTENT_TYPE)
            || self.artifact.get("raw_attestations").and_then(|value| value.as_bool()) != Some(false)
            || !self.raw_data_local || !self.aggregate_only || self.program_id.trim().is_empty()
            || self.candidate_order.is_empty() || self.site_order.is_empty() || self.action_order.is_empty()
            || self.effect_receipts.is_empty()
            || ![&self.replay_identity, &self.control_digest].into_iter().all(digest)
        {
            return Err(ContextControlPlaneError::Invalid("control-plane identity, candidates, actions, locality, digests, or effects are incomplete".into()));
        }
        for values in [&self.candidate_order, &self.admitted_order, &self.unresolved_order, &self.blocked_order, &self.site_order, &self.action_order, &self.admitted_action_order, &self.denied_action_order, &self.omissions, &self.uncertainty, &self.negative_evidence, &self.effect_receipts] {
            if !ordered(values) { return Err(ContextControlPlaneError::Invalid("control-plane ordering is not canonical".into())); }
        }
        let candidates = self.candidate_order.iter().cloned().collect::<BTreeSet<_>>();
        let states = self.admitted_order.iter().chain(&self.unresolved_order).chain(&self.blocked_order).cloned().collect::<Vec<_>>();
        if candidates.len() != self.candidate_order.len() || states.len() != candidates.len() || states.iter().cloned().collect::<BTreeSet<_>>() != candidates {
            return Err(ContextControlPlaneError::Invalid("candidate states do not partition".into()));
        }
        let actions = self.action_order.iter().cloned().collect::<BTreeSet<_>>();
        let action_parts = self.admitted_action_order.iter().chain(&self.denied_action_order).cloned().collect::<Vec<_>>();
        if actions.len() != self.action_order.len() || action_parts.len() != actions.len() || action_parts.iter().cloned().collect::<BTreeSet<_>>() != actions {
            return Err(ContextControlPlaneError::Invalid("action states do not partition".into()));
        }
        if self.effect_receipts.iter().any(|effect| effect != "block:unsafe-release" && !effect.starts_with("control:worldgen-context:")) {
            return Err(ContextControlPlaneError::Invalid("control-plane effect is outside governance gate".into()));
        }
        if self.artifact.get("content_hash").and_then(|value| value.as_str()) != Some(self.control_digest.as_str()) { return Err(ContextControlPlaneError::Invalid("control artifact digest is inconsistent".into())); }
        Ok(())
    }
}

pub fn manifest(feature_id: &str, version: &str, input_schema: &str, scale: &str, autonomy: &str) -> serde_json::Value {
    json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "capability_id": feature_id,
        "version": version,
        "owner_crate": "worldgen",
        "consumers": ["consortium operator", "research program lead", "federated benchmark steward"],
        "behavior": format!("rank and govern digest-only context attestations for {scale}"),
        "value": "makes federated context admission, quorum, freshness, and action governance deterministic and auditable",
        "input_schema": input_schema,
        "output_schema": "FederatedContextControlReceipt1@1",
        "effects": ["control:worldgen-context", "block:unsafe-release"],
        "permissions": ["control:aggregate-context-attestations"],
        "determinism": "byte_stable",
        "autonomy_tier": autonomy,
        "boundary": PRECLINICAL_BOUNDARY,
        "contract_version": version
    })
}

pub fn control(
    request: &ContextControlPlaneRequest,
    feature_id: &str,
    contract_version: &str,
    scale: &str,
    require_approval: bool,
    require_federation: bool,
) -> Result<ContextControlPlaneReceipt, ContextControlPlaneError> {
    if request.program_id.trim().is_empty() || request.attestations.is_empty() || request.requested_action_order.is_empty() || request.action_budget == 0 || request.minimum_site_quorum == 0 || request.boundary != PRECLINICAL_BOUNDARY || sorted(&request.requested_action_order) != request.requested_action_order || !digest(&request.replay_identity) {
        return Err(ContextControlPlaneError::Invalid("control-plane program, attestations, actions, budget, boundary, ordering, or replay is invalid".into()));
    }
    let mut candidates = request.attestations.clone();
    candidates.sort_by(|left, right| right.support_milli.cmp(&left.support_milli).then_with(|| right.freshness_milli.cmp(&left.freshness_milli)).then_with(|| left.attestation_id.cmp(&right.attestation_id)));
    if candidates.iter().any(|attestation| attestation.attestation_id.trim().is_empty() || attestation.site_id.trim().is_empty() || !digest(&attestation.context_digest) || !digest(&attestation.provenance_digest) || !digest(&attestation.replay_identity) || !attestation.raw_data_local || !attestation.aggregate_only || attestation.boundary != PRECLINICAL_BOUNDARY || attestation.replay_identity != request.replay_identity) {
        return Err(ContextControlPlaneError::Invalid("attestation identity, replay, provenance, locality, or boundary is invalid".into()));
    }
    let candidate_order = sorted(&candidates.iter().map(|attestation| attestation.attestation_id.clone()).collect::<Vec<_>>());
    let mut admitted = BTreeSet::new(); let mut unresolved = BTreeSet::new(); let mut blocked = BTreeSet::new();
    for attestation in &candidates {
        if !attestation.permitted { blocked.insert(attestation.attestation_id.clone()); }
        else if attestation.evidence_state != "supported" || attestation.support_milli < request.minimum_support_milli || attestation.freshness_milli < request.minimum_freshness_milli { unresolved.insert(attestation.attestation_id.clone()); }
        else { admitted.insert(attestation.attestation_id.clone()); }
    }
    let sites = admitted.iter().filter_map(|id| candidates.iter().find(|attestation| &attestation.attestation_id == id).map(|attestation| attestation.site_id.clone())).collect::<BTreeSet<_>>();
    let approvals_ok = (!require_approval || request.signed_approval) && (!require_federation || request.federation_approved);
    let quorum_ok = sites.len() >= request.minimum_site_quorum as usize;
    let actions_ok = request.requested_action_order.len() as u64 <= request.action_budget;
    let safe = approvals_ok && quorum_ok && actions_ok && !admitted.is_empty() && unresolved.is_empty() && blocked.is_empty();
    let disposition = if !approvals_ok || !quorum_ok { "blocked" } else if safe { "qualified" } else { "partial" };
    let mut omissions = Vec::new(); if !approvals_ok { omissions.push("control:approval-missing".into()); } if !quorum_ok { omissions.push("control:site-quorum-missing".into()); } if !actions_ok { omissions.push("control:action-budget-exceeded".into()); } if !unresolved.is_empty() { omissions.push("control:unsupported-or-stale-attestation".into()); } if !blocked.is_empty() { omissions.push("control:permitted-attestation-missing".into()); } omissions.sort();
    let mut denied_actions = if safe { Vec::new() } else { request.requested_action_order.clone() }; denied_actions.sort();
    let admitted_actions = if safe { request.requested_action_order.clone() } else { Vec::new() };
    let sites_sorted = sites.into_iter().collect::<Vec<_>>();
    let effect_receipts = if disposition == "qualified" { vec![format!("control:worldgen-context:{}", request.program_id)] } else { vec!["block:unsafe-release".into()] };
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"program_id":request.program_id,"scale":scale,"disposition":disposition,"candidate_order":candidate_order,"admitted_order":admitted,"unresolved_order":unresolved,"blocked_order":blocked,"site_order":sites_sorted,"action_order":request.requested_action_order,"admitted_action_order":admitted_actions,"denied_action_order":denied_actions,"replay_identity":request.replay_identity,"omissions":omissions,"uncertainty":if safe {Vec::<String>::new()} else {vec!["control:qualification-requires-complete-attestations".to_string()]},"negative_evidence":if !actions_ok {vec!["control:action-budget-negative".to_string()]} else {Vec::<String>::new()},"effect_receipts":effect_receipts,"raw_attestations":false,"boundary":PRECLINICAL_BOUNDARY});
    let control_digest = ContentHash::of_value(&payload).map_err(|error| ContextControlPlaneError::Artifact(error.to_string()))?;
    let admitted_order = sorted(&admitted.into_iter().collect::<Vec<_>>()); let unresolved_order = sorted(&unresolved.into_iter().collect::<Vec<_>>()); let blocked_order = sorted(&blocked.into_iter().collect::<Vec<_>>());
    let receipt = ContextControlPlaneReceipt { schema_version:RESEARCH_CONTRACT_SCHEMA_VERSION.into(), contract_version:contract_version.into(), feature_id:feature_id.into(), program_id:request.program_id.clone(), disposition:disposition.into(), candidate_order, admitted_order, unresolved_order, blocked_order, site_order:sites_sorted, action_order:request.requested_action_order.clone(), admitted_action_order:admitted_actions, denied_action_order:denied_actions, replay_identity:request.replay_identity.clone(), control_digest: control_digest.clone(), omissions, uncertainty:if safe {Vec::new()} else {vec!["control:qualification-requires-complete-attestations".into()]}, negative_evidence:if !actions_ok {vec!["control:action-budget-negative".into()]} else {Vec::new()}, effect_receipts:sorted(&effect_receipts), artifact:json!({"artifact_id":format!("worldgen-context-control:{}",request.program_id),"content_type":CONTENT_TYPE,"content_hash":control_digest,"raw_attestations":false,"aggregate_only":true,"boundary":PRECLINICAL_BOUNDARY}), raw_data_local:true, aggregate_only:true, boundary:PRECLINICAL_BOUNDARY.into()}; receipt.validate()?; Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(seed: &str) -> ContentHash { ContentHash::of_bytes(seed.as_bytes()) }
    fn request() -> ContextControlPlaneRequest { let replay=hash("replay"); let attestation=ContextControlAttestation{attestation_id:"attestation:a".into(),site_id:"site:a".into(),context_digest:hash("context"),support_milli:900,freshness_milli:900,evidence_state:"supported".into(),provenance_digest:hash("provenance"),replay_identity:replay.clone(),permitted:true,raw_data_local:true,aggregate_only:true,boundary:PRECLINICAL_BOUNDARY.into()}; ContextControlPlaneRequest{program_id:"program:context".into(),attestations:vec![attestation],minimum_support_milli:500,minimum_freshness_milli:500,minimum_site_quorum:1,requested_action_order:vec!["retain:digest".into()],action_budget:2,signed_approval:true,federation_approved:true,replay_identity:replay,boundary:PRECLINICAL_BOUNDARY.into()} }
    #[test] fn qualified_digest_control_is_deterministic(){let r=control(&request(),"AFA-worldgen-P03-F29","worldgen-local-context-control-plane/1.0","local single-study",false,false).unwrap();assert_eq!(r.disposition,"qualified");assert!(r.effect_receipts[0].starts_with("control:worldgen-context:"));assert!(r.validate().is_ok());}
    #[test] fn unsupported_attestation_is_retained(){let mut q=request();q.attestations[0].evidence_state="unknown".into();let r=control(&q,"AFA-worldgen-P03-F30","worldgen-multimodal-context-control-plane/1.0","multimodal multi-study",false,false).unwrap();assert_eq!(r.disposition,"partial");assert_eq!(r.unresolved_order,vec!["attestation:a"]);}
    #[test] fn federation_gate_blocks(){let mut q=request();q.federation_approved=false;let r=control(&q,"AFA-worldgen-P03-F32","worldgen-federated-continual-context-control-plane/1.0","federated continual autonomous",true,true).unwrap();assert_eq!(r.disposition,"blocked");}
}
