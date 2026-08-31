//! Weave P32: capability-manifest admission integrity contracts.
//!
//! Capability manifests are the kernel's typed boundary with untrusted agents and
//! downstream crates.  This contract admits only manifests with an observable
//! consumer, typed ports, explicit effects, evidence, authority posture, and a
//! content-addressed replay identity.  It never grants authority or executes an
//! effect; it produces an auditable admission card for the policy and workflow layers.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use thiserror::Error;

pub const SCHEMA_VERSION: &str = "aurora-research-contract/1.0";
pub const BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";
pub const CONTENT_TYPE: &str =
    "application/vnd.aurora.weave.capability-manifest-integrity-card-1+json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityCandidate4 {
    pub capability_id: String,
    pub version: String,
    pub owner_crate: String,
    pub consumer: String,
    pub behavior: String,
    pub input_schema: String,
    pub output_schema: String,
    pub effect: String,
    pub determinism: String,
    pub evidence_state: String,
    pub evidence_digest: String,
    pub authority_required: bool,
    pub autonomy_tier: String,
    pub local: bool,
    pub aggregate_only: bool,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityManifestRequest4 {
    pub schema_version: String,
    pub request_id: String,
    pub purpose: String,
    pub candidates: Vec<CapabilityCandidate4>,
    pub required_capability_order: Vec<String>,
    pub replay_identity: String,
    pub policy_allowed: bool,
    pub protected_closure: bool,
    pub signed_manifest: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub adversarial_events: Vec<String>,
    pub capability_budget: usize,
    pub boundary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityArtifact4 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: String,
    pub semantic_loss: Vec<String>,
    pub evidence_digests: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityManifestCard7 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub purpose: String,
    pub disposition: String,
    pub capability_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub rejected_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub omitted_order: Vec<String>,
    pub consumer_order: Vec<String>,
    pub owner_order: Vec<String>,
    pub schema_order: Vec<String>,
    pub effect_order: Vec<String>,
    pub replay_identity: String,
    pub closure_digest: String,
    pub admitted_capability_count: u64,
    pub total_capability_count: u64,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
    pub effect_receipts: Vec<String>,
    pub artifact: CapabilityArtifact4,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum CapabilityManifestIntegrityError {
    #[error("capability manifest request is invalid: {0}")]
    Invalid(String),
    #[error("capability manifest digest could not be computed: {0}")]
    Digest(String),
}

fn digest(v: &str) -> bool {
    v.len() == 64 && v.bytes().all(|b| b.is_ascii_hexdigit())
}

fn canonical(v: &[String]) -> bool {
    v.windows(2).all(|w| w[0] < w[1])
}

fn invalid(v: impl Into<String>) -> CapabilityManifestIntegrityError {
    CapabilityManifestIntegrityError::Invalid(v.into())
}

pub fn manifest(feature_id: &str, contract_version: &str, scale: &str, mode: &str) -> Value {
    json!({
        "feature_id": feature_id,
        "contract_version": contract_version,
        "schema_version": SCHEMA_VERSION,
        "content_type": CONTENT_TYPE,
        "boundary": BOUNDARY,
        "scale": scale,
        "mode": mode,
        "consumer": "weave kernel, policy gate, and workflow compiler",
        "effects": ["emit typed admission card", "retain rejected and unresolved manifests"],
        "determinism": "canonical vectors and content-addressed closure",
        "autonomy": "A1 admission preparation; no authority grant or execution",
    })
}

fn validate_card(c: &CapabilityManifestCard7) -> Result<(), CapabilityManifestIntegrityError> {
    if c.schema_version != SCHEMA_VERSION
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
        || c.admitted_capability_count > c.total_capability_count
    {
        return Err(invalid(
            "capability identity, locality, artifact, digest, boundary, or count is incomplete",
        ));
    }
    for v in [
        &c.capability_order,
        &c.admitted_order,
        &c.rejected_order,
        &c.unknown_order,
        &c.omitted_order,
        &c.consumer_order,
        &c.owner_order,
        &c.schema_order,
        &c.effect_order,
        &c.effect_receipts,
    ] {
        if !canonical(v) {
            return Err(invalid("capability vectors are not canonical"));
        }
    }
    let ids = c.capability_order.iter().collect::<BTreeSet<_>>();
    let states = c
        .admitted_order
        .iter()
        .chain(&c.rejected_order)
        .chain(&c.unknown_order)
        .chain(&c.omitted_order)
        .collect::<Vec<_>>();
    if states.len() != ids.len() || states.into_iter().collect::<BTreeSet<_>>() != ids {
        return Err(invalid("capability states do not partition manifests"));
    }
    if c.admitted_capability_count != c.admitted_order.len() as u64 {
        return Err(invalid(
            "admitted capability count does not match admitted order",
        ));
    }
    Ok(())
}

pub fn admit(
    q: &CapabilityManifestRequest4,
    feature_id: &str,
    contract_version: &str,
    scale: &str,
    mode: &str,
) -> Result<CapabilityManifestCard7, CapabilityManifestIntegrityError> {
    if q.schema_version != SCHEMA_VERSION
        || q.request_id.is_empty()
        || q.purpose.is_empty()
        || q.candidates.is_empty()
        || q.capability_budget == 0
        || !digest(&q.replay_identity)
        || q.boundary != BOUNDARY
        || !q.raw_data_local
        || !q.aggregate_only
        || !canonical(&q.required_capability_order)
        || !canonical(&q.adversarial_events)
    {
        return Err(invalid(
            "capability identity, ordering, replay, locality, boundary, or budget is invalid",
        ));
    }
    let rows = q.candidates.iter().collect::<Vec<_>>();
    let mut seen = BTreeSet::new();
    let mut admitted = BTreeSet::new();
    let mut rejected = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut omitted = BTreeSet::new();
    let mut consumers = BTreeSet::new();
    let mut owners = BTreeSet::new();
    let mut schemas = BTreeSet::new();
    let mut effects = BTreeSet::new();
    let mut evidence = BTreeSet::new();
    let mut semantic_loss = Vec::new();
    let mut global_block = !q.policy_allowed
        || !q.protected_closure
        || !q.signed_manifest
        || !q.raw_data_local
        || !q.aggregate_only
        || !q.adversarial_events.is_empty()
        || rows.len() > q.capability_budget;
    for candidate in rows {
        if candidate.capability_id.is_empty()
            || candidate.version.is_empty()
            || candidate.owner_crate.is_empty()
            || candidate.consumer.is_empty()
            || candidate.behavior.is_empty()
            || candidate.input_schema.is_empty()
            || candidate.output_schema.is_empty()
            || candidate.effect.is_empty()
            || candidate.determinism.is_empty()
            || !digest(&candidate.evidence_digest)
        {
            return Err(invalid("capability identity, consumer, typed ports, effect, determinism, or evidence is incomplete"));
        }
        if !seen.insert(candidate.capability_id.clone()) {
            return Err(invalid(format!(
                "duplicate capability {}",
                candidate.capability_id
            )));
        }
        consumers.insert(candidate.consumer.clone());
        owners.insert(candidate.owner_crate.clone());
        schemas.insert(format!(
            "{}→{}",
            candidate.input_schema, candidate.output_schema
        ));
        effects.insert(candidate.effect.clone());
        evidence.insert(candidate.evidence_digest.clone());
        if !candidate.local || !candidate.aggregate_only {
            global_block = true;
        }
        match candidate.evidence_state.as_str() {
            "supported" | "proven"
                if candidate.required && candidate.determinism == "byte_stable" =>
            {
                admitted.insert(candidate.capability_id.clone());
            }
            "contradicted" | "rejected" => {
                rejected.insert(candidate.capability_id.clone());
                semantic_loss.push(candidate.capability_id.clone());
            }
            "unknown" | "speculative" | "unmeasured" => {
                unknown.insert(candidate.capability_id.clone());
                semantic_loss.push(candidate.capability_id.clone());
            }
            _ => {
                omitted.insert(candidate.capability_id.clone());
                semantic_loss.push(candidate.capability_id.clone());
            }
        }
    }
    let required = q
        .required_capability_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if required != seen {
        return Err(invalid(
            "required capability order is not the canonical capability set",
        ));
    }
    if global_block {
        omitted.extend(seen.clone());
        admitted.clear();
        rejected.clear();
        unknown.clear();
    }
    let disposition = if global_block {
        "blocked"
    } else if !unknown.is_empty() {
        "unknown"
    } else if !rejected.is_empty() || !omitted.is_empty() {
        "partial"
    } else {
        "admitted"
    };
    let body = json!({
        "schema_version": SCHEMA_VERSION,
        "contract_version": contract_version,
        "feature_id": feature_id,
        "request_id": q.request_id,
        "purpose": q.purpose,
        "disposition": disposition,
        "capability_order": seen.iter().cloned().collect::<Vec<_>>(),
    });
    let closure_digest = ContentHash::of_value(&body)
        .map_err(|e| CapabilityManifestIntegrityError::Digest(e.to_string()))?
        .to_string();
    let admitted_order = admitted.into_iter().collect::<Vec<_>>();
    let rejected_order = rejected.into_iter().collect::<Vec<_>>();
    let unknown_order = unknown.into_iter().collect::<Vec<_>>();
    let omitted_order = omitted.into_iter().collect::<Vec<_>>();
    let capability_order = body["capability_order"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_owned)
        .collect::<Vec<_>>();
    let artifact = CapabilityArtifact4 {
        artifact_id: format!("weave-capability-manifest:{}", q.request_id),
        content_type: CONTENT_TYPE.into(),
        content_hash: closure_digest.clone(),
        semantic_loss,
        evidence_digests: evidence.into_iter().collect(),
        boundary: BOUNDARY.into(),
    };
    let c = CapabilityManifestCard7 {
        schema_version: SCHEMA_VERSION.into(),
        contract_version: contract_version.into(),
        feature_id: feature_id.into(),
        request_id: q.request_id.clone(),
        purpose: q.purpose.clone(),
        disposition: disposition.into(),
        capability_order,
        admitted_order: admitted_order.clone(),
        rejected_order,
        unknown_order,
        omitted_order,
        consumer_order: consumers.into_iter().collect(),
        owner_order: owners.into_iter().collect(),
        schema_order: schemas.into_iter().collect(),
        effect_order: effects.into_iter().collect(),
        replay_identity: q.replay_identity.clone(),
        closure_digest,
        admitted_capability_count: admitted_order.len() as u64,
        total_capability_count: q.candidates.len() as u64,
        raw_data_local: true,
        aggregate_only: true,
        boundary: BOUNDARY.into(),
        effect_receipts: if disposition == "admitted" {
            vec![format!("prepare:capability-admission:{}", q.request_id)]
        } else {
            vec!["block:unsafe-capability-effect".into()]
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

    fn request() -> CapabilityManifestRequest4 {
        CapabilityManifestRequest4 {
            schema_version: SCHEMA_VERSION.into(),
            request_id: "req-1".into(),
            purpose: "admit typed capability".into(),
            candidates: vec![CapabilityCandidate4 {
                capability_id: "capability-a".into(),
                version: "1.0.0".into(),
                owner_crate: "weave".into(),
                consumer: "workflow compiler".into(),
                behavior: "emit deterministic execution receipt".into(),
                input_schema: "ResearchWorkflowSpec@1".into(),
                output_schema: "ExecutionRun@1".into(),
                effect: "emit:typed-receipt".into(),
                determinism: "byte_stable".into(),
                evidence_state: "supported".into(),
                evidence_digest: "a".repeat(64),
                authority_required: false,
                autonomy_tier: "A1".into(),
                local: true,
                aggregate_only: true,
                required: true,
            }],
            required_capability_order: vec!["capability-a".into()],
            replay_identity: "b".repeat(64),
            policy_allowed: true,
            protected_closure: true,
            signed_manifest: true,
            raw_data_local: true,
            aggregate_only: true,
            adversarial_events: vec![],
            capability_budget: 2,
            boundary: BOUNDARY.into(),
        }
    }

    #[test]
    fn admits_typed_capability_without_granting_authority() {
        let card = admit(&request(), "AFA-weave-P32-F01", "v1", "local", "inference").unwrap();
        assert_eq!(card.disposition, "admitted");
        assert_eq!(card.admitted_capability_count, 1);
        assert_eq!(
            card.effect_receipts,
            vec!["prepare:capability-admission:req-1"]
        );
    }

    #[test]
    fn policy_failure_blocks_all_capabilities() {
        let mut q = request();
        q.policy_allowed = false;
        let card = admit(&q, "AFA-weave-P32-F02", "v1", "local", "inference").unwrap();
        assert_eq!(card.disposition, "blocked");
        assert!(card.admitted_order.is_empty());
        assert_eq!(card.omitted_order, vec!["capability-a"]);
    }
}
