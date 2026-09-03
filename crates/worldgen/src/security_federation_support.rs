//! Worldgen P20 security and federation admission kernel.
//!
//! This capability evaluates typed export metadata and emits a signed-envelope-ready receipt.
//! It never transfers raw experimental bytes, executes tools, or infers clinical conclusions.

use bioprism_foundation::{
    AutonomyTier, AuthorityRequirement, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, ResearchSurface, TypedPort, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-worldgen-P20-F01";
pub const CONTRACT_VERSION: &str = "worldgen-local-security-federation/1.0";
pub const INPUT_SCHEMA: &str = "SecurityFederationRequest1@1";
pub const OUTPUT_SCHEMA: &str = "FederationEnvelope1@1";
pub const CONTENT_TYPE: &str =
    "application/vnd.aurora.worldgen.security-federation-receipt-1+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityFederationAction {
    pub action_id: String,
    pub actor: String,
    pub source: String,
    pub destination: String,
    pub effect_order: Vec<String>,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub evidence_state: EvidenceState,
    pub authorized: bool,
    pub key_active: bool,
    pub revocation_epoch: String,
    pub export_requested: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityFederationRequest {
    pub schema_version: String,
    pub request_id: String,
    pub consumer: String,
    pub purpose: String,
    pub origin: String,
    pub destination: String,
    pub policy_epoch: String,
    pub key_id: String,
    pub replay_identity: ContentHash,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub federation_requested: bool,
    pub federation_authorized: bool,
    pub boundary: String,
    pub actions: Vec<SecurityFederationAction>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityFederationArtifact {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub provenance_digests: Vec<ContentHash>,
    pub semantic_loss: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SecurityFederationReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub consumer: String,
    pub purpose: String,
    pub origin: String,
    pub destination: String,
    pub policy_epoch: String,
    pub disposition: String,
    pub action_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub local_only_order: Vec<String>,
    pub denied_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub threat_order: Vec<String>,
    pub revocation_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub federation_digest: ContentHash,
    pub artifact: SecurityFederationArtifact,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum SecurityFederationError {
    #[error("invalid security/federation request or receipt: {0}")]
    Invalid(String),
    #[error("security/federation artifact failed: {0}")]
    Artifact(String),
}

fn digest(v: &ContentHash) -> bool {
    v.as_str().len() == 64 && v.as_str().bytes().all(|b| b.is_ascii_hexdigit())
}
fn ordered(v: &[String]) -> bool {
    v.windows(2).all(|w| w[0] < w[1])
}

pub fn manifest(feature_id: &str, contract_version: &str, scale: &str) -> serde_json::Value {
    json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "capability_id": feature_id,
        "version": contract_version,
        "owner_crate": "worldgen",
        "consumers": ["consortium security steward", "federation operator", "research program lead"],
        "behavior": format!("classify signed aggregate federation actions for {scale} with key, authorization, locality, and evidence gates"),
        "value": "prevents unauthorized or raw-data export while preserving replayable security evidence",
        "input_schema": INPUT_SCHEMA,
        "output_schema": OUTPUT_SCHEMA,
        "effects": ["emit:federation-receipt", "block:unauthorized-export"],
        "permissions": ["read:local-research-artifact-metadata"],
        "determinism": "byte_stable",
        "autonomy_tier": "A1",
        "boundary": PRECLINICAL_BOUNDARY
    })
}

fn validate_request(r: &SecurityFederationRequest) -> Result<(), SecurityFederationError> {
    if r.schema_version != INPUT_SCHEMA
        || [&r.request_id, &r.consumer, &r.purpose, &r.origin, &r.destination, &r.policy_epoch, &r.key_id]
            .iter()
            .any(|v| v.trim().is_empty())
        || !digest(&r.replay_identity)
        || r.boundary != PRECLINICAL_BOUNDARY
        || r.actions.is_empty()
    {
        return Err(SecurityFederationError::Invalid(
            "security request identity, replay, boundary, or action closure is invalid".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for a in &r.actions {
        if a.action_id.trim().is_empty()
            || !ids.insert(a.action_id.clone())
            || a.actor.trim().is_empty()
            || a.source.trim().is_empty()
            || a.destination.trim().is_empty()
            || !ordered(&a.effect_order)
            || !digest(&a.artifact_digest)
            || !digest(&a.provenance_digest)
            || a.replay_identity != r.replay_identity
            || a.revocation_epoch.trim().is_empty()
        {
            return Err(SecurityFederationError::Invalid(
                "action identity, effect ordering, digest, replay, or revocation epoch is invalid".into(),
            ));
        }
    }
    Ok(())
}

impl SecurityFederationReceipt {
    pub fn validate(&self) -> Result<(), SecurityFederationError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.content_type != CONTENT_TYPE
            || !self.raw_data_local
            || !self.aggregate_only
            || !matches!(self.disposition.as_str(), "admitted" | "local_only" | "blocked" | "unresolved")
            || self.action_order.is_empty()
            || !digest(&self.replay_identity)
            || !digest(&self.federation_digest)
            || self.artifact.content_hash != self.federation_digest
            || !self.artifact.provenance_digests.iter().all(digest)
        {
            return Err(SecurityFederationError::Invalid(
                "security receipt identity, locality, digest, or disposition is incomplete".into(),
            ));
        }
        for values in [
            &self.action_order,
            &self.admitted_order,
            &self.local_only_order,
            &self.denied_order,
            &self.unresolved_order,
            &self.omission_order,
            &self.threat_order,
            &self.revocation_order,
        ] {
            if !ordered(values) {
                return Err(SecurityFederationError::Invalid(
                    "security receipt vectors are not canonical".into(),
                ));
            }
        }
        let ids = self.action_order.iter().cloned().collect::<BTreeSet<_>>();
        let parts = self
            .admitted_order
            .iter()
            .chain(&self.local_only_order)
            .chain(&self.denied_order)
            .chain(&self.unresolved_order)
            .cloned()
            .collect::<Vec<_>>();
        if ids.len() != self.action_order.len()
            || parts.len() != ids.len()
            || parts.iter().cloned().collect::<BTreeSet<_>>() != ids
        {
            return Err(SecurityFederationError::Invalid(
                "security action states do not partition".into(),
            ));
        }
        Ok(())
    }
}

pub fn infer_security_receipt(
    r: &SecurityFederationRequest,
) -> Result<SecurityFederationReceipt, SecurityFederationError> {
    validate_request(r)?;
    let action_order = r
        .actions
        .iter()
        .map(|a| a.action_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let mut admitted = BTreeSet::new();
    let mut local_only = BTreeSet::new();
    let mut denied = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut threats = BTreeSet::new();
    let mut revocations = BTreeSet::new();
    let provenance = r
        .actions
        .iter()
        .map(|a| a.provenance_digest.clone())
        .collect::<BTreeSet<_>>();
    for a in &r.actions {
        if a.revocation_epoch != r.policy_epoch || !a.key_active {
            revocations.insert(format!("{}:key-revoked-or-stale", a.action_id));
            denied.insert(a.action_id.clone());
            continue;
        }
        if !a.authorized {
            threats.insert(format!("{}:authorization-missing", a.action_id));
            denied.insert(a.action_id.clone());
            continue;
        }
        if matches!(a.evidence_state, EvidenceState::Unknown | EvidenceState::Speculative | EvidenceState::Contradicted) {
            unresolved.insert(a.action_id.clone());
            omissions.insert(format!("{}:evidence-not-closed", a.action_id));
            continue;
        }
        if !a.export_requested || a.destination == r.origin {
            local_only.insert(a.action_id.clone());
        } else if a.source != r.origin || a.destination != r.destination {
            threats.insert(format!("{}:route-outside-declaration", a.action_id));
            denied.insert(a.action_id.clone());
        } else {
            admitted.insert(a.action_id.clone());
        }
    }
    if !r.protected_closure {
        omissions.insert("request:protected-closure-incomplete".into());
    }
    if !r.raw_data_local {
        threats.insert("request:raw-data-locality-false".into());
    }
    if !r.aggregate_only {
        threats.insert("request:aggregate-only-false".into());
    }
    if r.federation_requested && !r.federation_authorized {
        threats.insert("request:federation-authorization-missing".into());
    }
    let global_block = !r.protected_closure
        || !r.raw_data_local
        || !r.aggregate_only
        || (r.federation_requested && !r.federation_authorized);
    let disposition = if global_block || !denied.is_empty() {
        "blocked"
    } else if !unresolved.is_empty() {
        "unresolved"
    } else if !admitted.is_empty() {
        "admitted"
    } else {
        "local_only"
    };
    if global_block {
        denied.extend(action_order.iter().cloned());
        admitted.clear();
        local_only.clear();
        unresolved.clear();
        omissions.insert("request:export-closure-not-ready".into());
    }
    let payload = json!({
        "action_order": action_order,
        "admitted_order": admitted,
        "local_only_order": local_only,
        "denied_order": denied,
        "unresolved_order": unresolved,
        "omission_order": omissions,
        "threat_order": threats,
        "revocation_order": revocations,
        "replay_identity": r.replay_identity,
        "origin": r.origin,
        "destination": r.destination,
        "policy_epoch": r.policy_epoch,
    });
    let d = ContentHash::of_value(&payload)
        .map_err(|e| SecurityFederationError::Artifact(e.to_string()))?;
    let strings = |k: &str| {
        payload[k]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default()
    };
    let out = SecurityFederationReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: r.request_id.clone(),
        consumer: r.consumer.clone(),
        purpose: r.purpose.clone(),
        origin: r.origin.clone(),
        destination: r.destination.clone(),
        policy_epoch: r.policy_epoch.clone(),
        disposition: disposition.into(),
        action_order: strings("action_order"),
        admitted_order: strings("admitted_order"),
        local_only_order: strings("local_only_order"),
        denied_order: strings("denied_order"),
        unresolved_order: strings("unresolved_order"),
        omission_order: strings("omission_order"),
        threat_order: strings("threat_order"),
        revocation_order: strings("revocation_order"),
        replay_identity: r.replay_identity.clone(),
        federation_digest: d.clone(),
        artifact: SecurityFederationArtifact {
            artifact_id: format!("worldgen-security-federation:{}", r.request_id),
            content_type: CONTENT_TYPE.into(),
            content_hash: d,
            provenance_digests: provenance.into_iter().collect(),
            semantic_loss: if disposition == "admitted" { Vec::new() } else { vec!["export-not-executed".into()] },
            boundary: PRECLINICAL_BOUNDARY.into(),
        },
        raw_data_local: true,
        aggregate_only: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    out.validate()?;
    Ok(out)
}

pub fn qualify(
    request: &SecurityFederationRequest,
    feature_id: &str,
    contract_version: &str,
) -> Result<SecurityFederationReceipt, SecurityFederationError> {
    let mut out = infer_security_receipt(request)?;
    out.feature_id = feature_id.into();
    out.contract_version = contract_version.into();
    Ok(out)
}

pub type SecurityFederationRequest1 = SecurityFederationRequest;
pub type FederationEnvelope1 = SecurityFederationReceipt;
pub type SecurityFederationAction1 = SecurityFederationAction;
pub type SecurityFederationError1 = SecurityFederationError;
pub type SecurityFederationEvidenceState = EvidenceState;

#[allow(dead_code)]
pub fn capability_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "worldgen".into(),
        consumers: ["consortium security steward".into(), "federation operator".into()].into(),
        behavior: "admit signed aggregate research federation without raw-data movement".into(),
        value: "fail-closed security and locality evidence".into(),
        inputs: vec![TypedPort { name: "security_federation_request".into(), schema: INPUT_SCHEMA.into(), required: true }],
        outputs: vec![TypedPort { name: "federation_envelope".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: BTreeSet::from([Effect::FederationExport, Effect::ReadLocalData]),
        permissions: ["read:local-research-artifact-metadata".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }],
        authority_requirements: vec![AuthorityRequirement { role: "federation steward".into(), reason: "approve aggregate export".into() }],
        autonomy_tier: AutonomyTier::A1,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Protocol, ResearchSurface::Policy, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn h(v: &str) -> ContentHash { ContentHash::of_bytes(v.as_bytes()) }
    fn req() -> SecurityFederationRequest {
        SecurityFederationRequest {
            schema_version: INPUT_SCHEMA.into(), request_id: "security-1".into(), consumer: "steward".into(), purpose: "aggregate benchmark".into(), origin: "site-a".into(), destination: "site-b".into(), policy_epoch: "2026q3".into(), key_id: "key-1".into(), replay_identity: h("r"), protected_closure: true, raw_data_local: true, aggregate_only: true, federation_requested: true, federation_authorized: true, boundary: PRECLINICAL_BOUNDARY.into(), actions: vec![SecurityFederationAction { action_id: "a".into(), actor: "agent".into(), source: "site-a".into(), destination: "site-b".into(), effect_order: vec!["emit-aggregate".into()], artifact_digest: h("a"), provenance_digest: h("p"), replay_identity: h("r"), evidence_state: EvidenceState::Supported, authorized: true, key_active: true, revocation_epoch: "2026q3".into(), export_requested: true }],
        }
    }
    #[test] fn admitted_export_is_replayable() { assert_eq!(infer_security_receipt(&req()).unwrap().disposition, "admitted"); }
    #[test] fn revoked_key_blocks() { let mut r = req(); r.actions[0].key_active = false; assert_eq!(infer_security_receipt(&r).unwrap().disposition, "blocked"); }
    #[test] fn raw_data_flag_fails_closed() { let mut r = req(); r.raw_data_local = false; assert_eq!(infer_security_receipt(&r).unwrap().admitted_order, Vec::<String>::new()); }
}
