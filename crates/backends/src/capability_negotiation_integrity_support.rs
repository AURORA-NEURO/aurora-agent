//! Backends P32: capability-negotiation integrity contracts.
use bioprism_foundation::{PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use thiserror::Error;

pub const BOUNDARY: &str = PRECLINICAL_BOUNDARY;
pub const CONTENT_TYPE: &str =
    "application/vnd.aurora.backends.capability-negotiation-integrity-card-1+json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackendCandidate4 {
    pub backend_id: String,
    pub runtime: String,
    pub version: String,
    pub capability_digest: String,
    pub determinism: String,
    pub evidence_state: String,
    pub local: bool,
    pub aggregate_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackendRequest4 {
    pub schema_version: String,
    pub request_id: String,
    pub purpose: String,
    pub candidates: Vec<BackendCandidate4>,
    pub required_backend_order: Vec<String>,
    pub semantic_profile_digest: String,
    pub replay_identity: String,
    pub policy_allowed: bool,
    pub protected_closure: bool,
    pub signed_manifest: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub adversarial_events: Vec<String>,
    pub backend_budget: usize,
    pub boundary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackendArtifact4 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: String,
    pub semantic_loss: Vec<String>,
    pub evidence_digests: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BackendCard7 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub purpose: String,
    pub disposition: String,
    pub backend_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub rejected_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub omitted_order: Vec<String>,
    pub runtime_order: Vec<String>,
    pub version_order: Vec<String>,
    pub capability_order: Vec<String>,
    pub determinism_order: Vec<String>,
    pub replay_identity: String,
    pub closure_digest: String,
    pub selected_backend: Option<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
    pub effect_receipts: Vec<String>,
    pub artifact: BackendArtifact4,
}

#[derive(Debug, Error)]
pub enum CapabilityNegotiationIntegrityError {
    #[error("capability-negotiation integrity input is invalid: {0}")]
    Invalid(String),
    #[error("capability-negotiation integrity digest failed: {0}")]
    Digest(String),
}

fn digest(v: &str) -> bool {
    v.len() == 64 && v.bytes().all(|b| b.is_ascii_hexdigit())
}
fn canonical(v: &[String]) -> bool {
    v.windows(2).all(|w| w[0] < w[1])
}
fn invalid(v: impl Into<String>) -> CapabilityNegotiationIntegrityError {
    CapabilityNegotiationIntegrityError::Invalid(v.into())
}

pub fn manifest(feature_id: &str, contract_version: &str, scale: &str, mode: &str) -> Value {
    json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "capability_id": feature_id,
        "version": contract_version,
        "owner_crate": "backends",
        "consumers": ["runtime scheduler", "backend portfolio", "execution compiler", "research workbench"],
        "behavior": format!("negotiate typed backend capabilities at {scale} ({mode})"),
        "value": "selects only evidenced, deterministic, policy-authorized engines while preserving refusals and replay identity",
        "input_schema": "BackendRequest4@1",
        "output_schema": "BackendCard7@1",
        "effects": ["emit:capability-card", "retain:backend-refusal", "block:unsafe-execution"],
        "permissions": ["read:local-backend-manifests"],
        "determinism": "byte_stable",
        "autonomy_tier": "A1",
        "boundary": BOUNDARY
    })
}

fn validate_card(c: &BackendCard7) -> Result<(), CapabilityNegotiationIntegrityError> {
    if c.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
        || c.feature_id.is_empty()
        || c.request_id.is_empty()
        || c.purpose.is_empty()
        || c.boundary != BOUNDARY
        || c.artifact.boundary != BOUNDARY
        || !c.raw_data_local
        || !c.aggregate_only
        || !digest(&c.replay_identity)
        || !digest(&c.closure_digest)
        || c.artifact.content_type != CONTENT_TYPE
        || c.artifact.content_hash != c.closure_digest
    {
        return Err(invalid(
            "backend identity, locality, artifact, digest, or boundary is incomplete",
        ));
    }
    for v in [
        &c.backend_order,
        &c.selected_order,
        &c.rejected_order,
        &c.unknown_order,
        &c.omitted_order,
        &c.runtime_order,
        &c.version_order,
        &c.capability_order,
        &c.determinism_order,
        &c.effect_receipts,
    ] {
        if !canonical(v) {
            return Err(invalid("backend vectors are not canonical"));
        }
    }
    let ids = c.backend_order.iter().collect::<BTreeSet<_>>();
    let states = c
        .selected_order
        .iter()
        .chain(&c.rejected_order)
        .chain(&c.unknown_order)
        .chain(&c.omitted_order)
        .collect::<Vec<_>>();
    if states.len() != ids.len() || states.into_iter().collect::<BTreeSet<_>>() != ids {
        return Err(invalid("backend states do not partition candidates"));
    }
    if let Some(selected) = &c.selected_backend {
        if !c.selected_order.contains(selected) {
            return Err(invalid("selected backend is not in selected order"));
        }
    }
    Ok(())
}

pub fn negotiate(
    q: &BackendRequest4,
    feature_id: &str,
    contract_version: &str,
    scale: &str,
    mode: &str,
) -> Result<BackendCard7, CapabilityNegotiationIntegrityError> {
    if q.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
        || q.request_id.trim().is_empty()
        || q.purpose.trim().is_empty()
        || q.candidates.is_empty()
        || q.required_backend_order.is_empty()
        || !canonical(&q.required_backend_order)
        || !digest(&q.semantic_profile_digest)
        || !digest(&q.replay_identity)
        || q.boundary != BOUNDARY
        || !q.raw_data_local
        || !q.aggregate_only
        || !canonical(&q.adversarial_events)
        || q.backend_budget == 0
    {
        return Err(invalid("backend identity, ordering, semantic profile, digest, locality, boundary, or budget is invalid"));
    }
    let mut rows = q.candidates.clone();
    rows.sort_by(|a, b| a.backend_id.cmp(&b.backend_id));
    let mut seen = BTreeSet::new();
    let mut selected = BTreeSet::new();
    let mut rejected = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut omitted = BTreeSet::new();
    let mut runtimes = BTreeSet::new();
    let mut versions = BTreeSet::new();
    let mut capabilities = BTreeSet::new();
    let mut determinism = BTreeSet::new();
    for c in &rows {
        if c.backend_id.trim().is_empty()
            || !seen.insert(c.backend_id.clone())
            || c.runtime.trim().is_empty()
            || c.version.trim().is_empty()
            || !digest(&c.capability_digest)
            || c.determinism.trim().is_empty()
            || c.evidence_state.trim().is_empty()
            || !c.local
            || !c.aggregate_only
        {
            return Err(invalid(
                "backend identity, capability digest, evidence, or locality is invalid",
            ));
        }
        runtimes.insert(format!("{}:{}", c.backend_id, c.runtime));
        versions.insert(format!("{}:{}", c.backend_id, c.version));
        capabilities.insert(format!("{}:{}", c.backend_id, c.capability_digest));
        determinism.insert(format!("{}:{}", c.backend_id, c.determinism));
        if c.evidence_state == "unknown" || c.capability_digest == q.replay_identity {
            unknown.insert(c.backend_id.clone());
        } else if c.capability_digest != q.semantic_profile_digest || c.determinism != "byte_stable"
        {
            rejected.insert(c.backend_id.clone());
        } else if !q.required_backend_order.contains(&c.backend_id) {
            omitted.insert(c.backend_id.clone());
        } else {
            selected.insert(c.backend_id.clone());
        }
    }
    let missing = q
        .required_backend_order
        .iter()
        .filter(|x| !seen.contains(*x))
        .cloned()
        .collect::<Vec<_>>();
    let global = !q.policy_allowed
        || !q.protected_closure
        || !q.signed_manifest
        || !q.raw_data_local
        || !q.aggregate_only
        || !q.adversarial_events.is_empty()
        || rows.len() > q.backend_budget;
    if global {
        omitted.extend(seen.iter().cloned());
        selected.clear();
        rejected.clear();
        unknown.clear();
    }
    let disposition = if global {
        "blocked"
    } else if !missing.is_empty() || !unknown.is_empty() {
        "unknown"
    } else if selected.is_empty() || !rejected.is_empty() || !omitted.is_empty() {
        "partial"
    } else {
        "selected"
    };
    let selected_backend = q
        .required_backend_order
        .iter()
        .find(|id| selected.contains(*id))
        .cloned();
    let body = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"request_id":q.request_id,"purpose":q.purpose,"disposition":disposition,"backend_order":seen.iter().cloned().collect::<Vec<_>>()});
    let closure_digest = ContentHash::of_value(&body)
        .map_err(|e| CapabilityNegotiationIntegrityError::Digest(e.to_string()))?
        .to_string();
    let selected_order = selected.into_iter().collect::<Vec<_>>();
    let rejected_order = rejected.into_iter().collect::<Vec<_>>();
    let unknown_order = unknown.into_iter().collect::<Vec<_>>();
    let omitted_order = omitted.into_iter().collect::<Vec<_>>();
    let artifact = BackendArtifact4 {
        artifact_id: format!("backends-capability-negotiation:{}", q.request_id),
        content_type: CONTENT_TYPE.into(),
        content_hash: closure_digest.clone(),
        semantic_loss: omitted_order.clone(),
        evidence_digests: capabilities.iter().cloned().collect(),
        boundary: BOUNDARY.into(),
    };
    let c = BackendCard7 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: contract_version.into(),
        feature_id: feature_id.into(),
        request_id: q.request_id.clone(),
        purpose: q.purpose.clone(),
        disposition: disposition.into(),
        backend_order: body["backend_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        selected_order: selected_order.clone(),
        rejected_order,
        unknown_order,
        omitted_order,
        runtime_order: runtimes.into_iter().collect(),
        version_order: versions.into_iter().collect(),
        capability_order: capabilities.into_iter().collect(),
        determinism_order: determinism.into_iter().collect(),
        replay_identity: q.replay_identity.clone(),
        closure_digest,
        selected_backend,
        raw_data_local: true,
        aggregate_only: true,
        boundary: BOUNDARY.into(),
        effect_receipts: if disposition == "selected" {
            vec![format!("select:backend:{}", q.request_id)]
        } else {
            vec!["block:unsafe-execution".into()]
        },
        artifact,
    };
    validate_card(&c)?;
    let _ = (scale, mode);
    Ok(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> BackendRequest4 {
        let capability = "a".repeat(64);
        BackendRequest4 {
            schema_version: "aurora-research-contract/1.0".into(),
            request_id: "req-1".into(),
            purpose: "choose deterministic engine".into(),
            candidates: vec![BackendCandidate4 {
                backend_id: "engine-a".into(),
                runtime: "rust".into(),
                version: "1".into(),
                capability_digest: capability.clone(),
                determinism: "byte_stable".into(),
                evidence_state: "supported".into(),
                local: true,
                aggregate_only: true,
            }],
            required_backend_order: vec!["engine-a".into()],
            semantic_profile_digest: capability,
            replay_identity: "b".repeat(64),
            policy_allowed: true,
            protected_closure: true,
            signed_manifest: true,
            raw_data_local: true,
            aggregate_only: true,
            adversarial_events: vec![],
            backend_budget: 2,
            boundary: BOUNDARY.into(),
        }
    }

    #[test]
    fn selects_evidenced_backend_and_blocks_policy_failure() {
        let mut q = request();
        let card = negotiate(&q, "AFA-backends-P32-F01", "v1", "local", "inference").unwrap();
        assert_eq!(card.disposition, "selected");
        assert_eq!(card.selected_backend.as_deref(), Some("engine-a"));
        q.policy_allowed = false;
        assert_eq!(
            negotiate(&q, "AFA-backends-P32-F01", "v1", "local", "inference")
                .unwrap()
                .disposition,
            "blocked"
        );
    }
}
