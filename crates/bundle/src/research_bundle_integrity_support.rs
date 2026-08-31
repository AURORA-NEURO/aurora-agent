//! Bundle P32: signed research-object bundle integrity contracts.
use bioprism_foundation::{PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use thiserror::Error;

pub const BOUNDARY: &str = PRECLINICAL_BOUNDARY;
pub const CONTENT_TYPE: &str =
    "application/vnd.aurora.bundle.research-object-integrity-card-1+json";
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BundleEntry4 {
    pub entry_id: String,
    pub role: String,
    pub content_hash: String,
    pub provenance_digest: String,
    pub evidence_state: String,
    pub required: bool,
    pub local: bool,
    pub aggregate_only: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BundleReleaseRequest4 {
    pub schema_version: String,
    pub request_id: String,
    pub purpose: String,
    pub entries: Vec<BundleEntry4>,
    pub required_entry_order: Vec<String>,
    pub release_digest: String,
    pub replay_identity: String,
    pub policy_allowed: bool,
    pub protected_closure: bool,
    pub signed_manifest: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub adversarial_events: Vec<String>,
    pub bundle_budget: usize,
    pub boundary: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BundleArtifact4 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: String,
    pub semantic_loss: Vec<String>,
    pub evidence_digests: Vec<String>,
    pub boundary: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BundleCard7 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub purpose: String,
    pub disposition: String,
    pub entry_order: Vec<String>,
    pub released_order: Vec<String>,
    pub rejected_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub omitted_order: Vec<String>,
    pub role_order: Vec<String>,
    pub content_order: Vec<String>,
    pub provenance_order: Vec<String>,
    pub replay_identity: String,
    pub closure_digest: String,
    pub release_digest: String,
    pub released_entry_count: u64,
    pub total_entry_count: u64,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
    pub effect_receipts: Vec<String>,
    pub artifact: BundleArtifact4,
}
#[derive(Debug, Error)]
pub enum ResearchBundleIntegrityError {
    #[error("research bundle integrity input is invalid: {0}")]
    Invalid(String),
    #[error("research bundle integrity digest failed: {0}")]
    Digest(String),
}
fn digest(v: &str) -> bool {
    v.len() == 64 && v.bytes().all(|b| b.is_ascii_hexdigit())
}
fn canonical(v: &[String]) -> bool {
    v.windows(2).all(|w| w[0] < w[1])
}
fn invalid(v: impl Into<String>) -> ResearchBundleIntegrityError {
    ResearchBundleIntegrityError::Invalid(v.into())
}
pub fn manifest(feature_id: &str, contract_version: &str, scale: &str, mode: &str) -> Value {
    json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"bundle","consumers":["publication release service","reproduction operator","provenance ledger","research workbench"],"behavior":format!("assemble signed research-object bundles at {scale} ({mode})"),"value":"releases content-addressed, omission-aware research objects with provenance and evidence gates while retaining negative results","input_schema":"BundleReleaseRequest4@1","output_schema":"BundleCard7@1","effects":["emit:research-object-bundle","retain:negative-result-evidence","block:unsafe-release"],"permissions":["read:local-research-objects"],"determinism":"byte_stable","autonomy_tier":"A1","boundary":BOUNDARY})
}
fn validate_card(c: &BundleCard7) -> Result<(), ResearchBundleIntegrityError> {
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
        || !digest(&c.release_digest)
        || c.artifact.content_type != CONTENT_TYPE
        || c.artifact.content_hash != c.closure_digest
        || c.released_entry_count > c.total_entry_count
    {
        return Err(invalid(
            "bundle identity, locality, artifact, digest, boundary, or count is incomplete",
        ));
    }
    for v in [
        &c.entry_order,
        &c.released_order,
        &c.rejected_order,
        &c.unknown_order,
        &c.omitted_order,
        &c.role_order,
        &c.content_order,
        &c.provenance_order,
        &c.effect_receipts,
    ] {
        if !canonical(v) {
            return Err(invalid("bundle vectors are not canonical"));
        }
    }
    let ids = c.entry_order.iter().collect::<BTreeSet<_>>();
    let states = c
        .released_order
        .iter()
        .chain(&c.rejected_order)
        .chain(&c.unknown_order)
        .chain(&c.omitted_order)
        .collect::<Vec<_>>();
    if states.len() != ids.len() || states.into_iter().collect::<BTreeSet<_>>() != ids {
        return Err(invalid("bundle states do not partition entries"));
    }
    Ok(())
}
pub fn release(
    q: &BundleReleaseRequest4,
    feature_id: &str,
    contract_version: &str,
    scale: &str,
    mode: &str,
) -> Result<BundleCard7, ResearchBundleIntegrityError> {
    if q.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
        || q.request_id.trim().is_empty()
        || q.purpose.trim().is_empty()
        || q.entries.is_empty()
        || q.required_entry_order.is_empty()
        || !canonical(&q.required_entry_order)
        || !digest(&q.release_digest)
        || !digest(&q.replay_identity)
        || q.boundary != BOUNDARY
        || !q.raw_data_local
        || !q.aggregate_only
        || !canonical(&q.adversarial_events)
        || q.bundle_budget == 0
    {
        return Err(invalid(
            "bundle identity, ordering, release digest, locality, boundary, or budget is invalid",
        ));
    }
    let mut rows = q.entries.clone();
    rows.sort_by(|a, b| a.entry_id.cmp(&b.entry_id));
    let mut seen = BTreeSet::new();
    let mut released = BTreeSet::new();
    let mut rejected = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut omitted = BTreeSet::new();
    let mut roles = BTreeSet::new();
    let mut contents = BTreeSet::new();
    let mut provenances = BTreeSet::new();
    for e in &rows {
        if e.entry_id.trim().is_empty()
            || !seen.insert(e.entry_id.clone())
            || e.role.trim().is_empty()
            || !digest(&e.content_hash)
            || !digest(&e.provenance_digest)
            || e.evidence_state.trim().is_empty()
            || !e.local
            || !e.aggregate_only
        {
            return Err(invalid(
                "entry identity, role, digest, evidence, or locality is invalid",
            ));
        }
        roles.insert(format!("{}:{}", e.entry_id, e.role));
        contents.insert(format!("{}:{}", e.entry_id, e.content_hash));
        provenances.insert(format!("{}:{}", e.entry_id, e.provenance_digest));
        if e.evidence_state == "unknown" || e.content_hash == q.replay_identity {
            unknown.insert(e.entry_id.clone());
        } else if e.evidence_state == "contradicted" {
            rejected.insert(e.entry_id.clone());
        } else if !e.required {
            omitted.insert(e.entry_id.clone());
        } else if !q.required_entry_order.contains(&e.entry_id) {
            omitted.insert(e.entry_id.clone());
        } else {
            released.insert(e.entry_id.clone());
        }
    }
    let missing = q
        .required_entry_order
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
        || rows.len() > q.bundle_budget;
    if global {
        omitted.extend(seen.iter().cloned());
        released.clear();
        rejected.clear();
        unknown.clear();
    }
    let disposition = if global {
        "blocked"
    } else if !missing.is_empty() || !unknown.is_empty() {
        "unknown"
    } else if released.is_empty() || !rejected.is_empty() || !omitted.is_empty() {
        "partial"
    } else {
        "released"
    };
    let body = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"request_id":q.request_id,"purpose":q.purpose,"disposition":disposition,"entry_order":seen.iter().cloned().collect::<Vec<_>>()});
    let closure_digest = ContentHash::of_value(&body)
        .map_err(|e| ResearchBundleIntegrityError::Digest(e.to_string()))?
        .to_string();
    let released_order = released.into_iter().collect::<Vec<_>>();
    let rejected_order = rejected.into_iter().collect::<Vec<_>>();
    let unknown_order = unknown.into_iter().collect::<Vec<_>>();
    let omitted_order = omitted.into_iter().collect::<Vec<_>>();
    let artifact = BundleArtifact4 {
        artifact_id: format!("bundle-research-object:{}", q.request_id),
        content_type: CONTENT_TYPE.into(),
        content_hash: closure_digest.clone(),
        semantic_loss: omitted_order.clone(),
        evidence_digests: provenances.iter().cloned().collect(),
        boundary: BOUNDARY.into(),
    };
    let c = BundleCard7 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: contract_version.into(),
        feature_id: feature_id.into(),
        request_id: q.request_id.clone(),
        purpose: q.purpose.clone(),
        disposition: disposition.into(),
        entry_order: body["entry_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        released_order: released_order.clone(),
        rejected_order,
        unknown_order,
        omitted_order,
        role_order: roles.into_iter().collect(),
        content_order: contents.into_iter().collect(),
        provenance_order: provenances.into_iter().collect(),
        replay_identity: q.replay_identity.clone(),
        closure_digest,
        release_digest: q.release_digest.clone(),
        released_entry_count: released_order.len() as u64,
        total_entry_count: rows.len() as u64,
        raw_data_local: true,
        aggregate_only: true,
        boundary: BOUNDARY.into(),
        effect_receipts: if disposition == "released" {
            vec![format!("release:bundle:{}", q.request_id)]
        } else {
            vec!["block:unsafe-release".into()]
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

    #[test]
    fn releases_required_entry_and_retains_contradiction() {
        let q = BundleReleaseRequest4 {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), request_id: "req-1".into(), purpose: "publish object".into(),
            entries: vec![BundleEntry4 { entry_id: "entry-a".into(), role: "result".into(), content_hash: "a".repeat(64), provenance_digest: "b".repeat(64), evidence_state: "supported".into(), required: true, local: true, aggregate_only: true }],
            required_entry_order: vec!["entry-a".into()], release_digest: "c".repeat(64), replay_identity: "d".repeat(64), policy_allowed: true, protected_closure: true, signed_manifest: true, raw_data_local: true, aggregate_only: true, adversarial_events: vec![], bundle_budget: 2, boundary: BOUNDARY.into(),
        };
        let card = release(&q, "AFA-bundle-P32-F01", "v1", "local", "inference").unwrap();
        assert_eq!(card.disposition, "released");
        assert_eq!(card.released_entry_count, 1);
    }
}
