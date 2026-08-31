//! Sweep P32: release-audit and dependency-drift integrity contracts.
use bioprism_foundation::{PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;
use thiserror::Error;
pub const BOUNDARY: &str = PRECLINICAL_BOUNDARY;
pub const CONTENT_TYPE: &str = "application/vnd.aurora.sweep.audit-integrity-card-1+json";
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditSubject4 {
    pub subject_id: String,
    pub source_commit: String,
    pub observed_digest: String,
    pub expected_digest: String,
    pub status: String,
    pub evidence_state: String,
    pub local: bool,
    pub aggregate_only: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditRequest4 {
    pub schema_version: String,
    pub request_id: String,
    pub purpose: String,
    pub subjects: Vec<AuditSubject4>,
    pub required_subject_order: Vec<String>,
    pub replay_identity: String,
    pub policy_allowed: bool,
    pub protected_closure: bool,
    pub signed_manifest: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub adversarial_events: Vec<String>,
    pub subject_budget: usize,
    pub boundary: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditArtifact4 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: String,
    pub semantic_loss: Vec<String>,
    pub evidence_digests: Vec<String>,
    pub boundary: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuditCard7 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub purpose: String,
    pub disposition: String,
    pub subject_order: Vec<String>,
    pub clean_order: Vec<String>,
    pub drift_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub omitted_order: Vec<String>,
    pub source_order: Vec<String>,
    pub status_order: Vec<String>,
    pub evidence_order: Vec<String>,
    pub replay_identity: String,
    pub closure_digest: String,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
    pub effect_receipts: Vec<String>,
    pub artifact: AuditArtifact4,
}
#[derive(Debug, Error)]
pub enum AuditIntegrityError {
    #[error("audit integrity input is invalid: {0}")]
    Invalid(String),
    #[error("audit integrity digest failed: {0}")]
    Digest(String),
}
fn digest(v: &str) -> bool {
    v.len() == 64 && v.bytes().all(|b| b.is_ascii_hexdigit())
}
fn canonical(v: &[String]) -> bool {
    v.windows(2).all(|w| w[0] < w[1])
}
fn invalid(v: impl Into<String>) -> AuditIntegrityError {
    AuditIntegrityError::Invalid(v.into())
}
pub fn manifest(feature_id: &str, contract_version: &str, scale: &str, mode: &str) -> Value {
    json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"capability_id":feature_id,"version":contract_version,"owner_crate":"sweep","consumers":["portfolio auditor","release steward","dependency planner","research workbench"],"behavior":format!("classify release and dependency drift at {scale} ({mode})"),"value":"prevents unreviewed source drift, omitted checks, or poisoned artifacts from entering reproducible research releases","input_schema":"AuditRequest4@1","output_schema":"AuditCard7@1","effects":["emit:audit-card","retain:drift-evidence","block:unsafe-release"],"permissions":["read:local-audit-fixtures"],"determinism":"byte_stable","autonomy_tier":"A1","boundary":BOUNDARY})
}
fn validate_card(c: &AuditCard7) -> Result<(), AuditIntegrityError> {
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
            "audit identity, locality, artifact, digest, or boundary is incomplete",
        ));
    }
    for v in [
        &c.subject_order,
        &c.clean_order,
        &c.drift_order,
        &c.unknown_order,
        &c.omitted_order,
        &c.source_order,
        &c.status_order,
        &c.evidence_order,
        &c.effect_receipts,
    ] {
        if !canonical(v) {
            return Err(invalid("audit vectors are not canonical"));
        }
    }
    let ids = c.subject_order.iter().collect::<BTreeSet<_>>();
    let states = c
        .clean_order
        .iter()
        .chain(&c.drift_order)
        .chain(&c.unknown_order)
        .chain(&c.omitted_order)
        .collect::<Vec<_>>();
    if states.len() != ids.len() || states.into_iter().collect::<BTreeSet<_>>() != ids {
        return Err(invalid("subject states do not partition subjects"));
    }
    Ok(())
}
pub fn qualify(
    q: &AuditRequest4,
    feature_id: &str,
    contract_version: &str,
    scale: &str,
    mode: &str,
) -> Result<AuditCard7, AuditIntegrityError> {
    if q.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
        || q.request_id.trim().is_empty()
        || q.purpose.trim().is_empty()
        || q.subjects.is_empty()
        || q.required_subject_order.is_empty()
        || !canonical(&q.required_subject_order)
        || !digest(&q.replay_identity)
        || q.boundary != BOUNDARY
        || !q.raw_data_local
        || !q.aggregate_only
        || !canonical(&q.adversarial_events)
        || q.subject_budget == 0
    {
        return Err(invalid(
            "audit identity, ordering, digest, locality, boundary, or budget is invalid",
        ));
    }
    let mut rows = q.subjects.clone();
    rows.sort_by(|a, b| a.subject_id.cmp(&b.subject_id));
    let mut seen = BTreeSet::new();
    let mut clean = BTreeSet::new();
    let mut drift = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut omitted = BTreeSet::new();
    let mut sources = BTreeSet::new();
    let mut statuses = BTreeSet::new();
    let mut evidence = BTreeSet::new();
    for s in &rows {
        if s.subject_id.trim().is_empty()
            || !seen.insert(s.subject_id.clone())
            || s.source_commit.trim().is_empty()
            || !digest(&s.observed_digest)
            || !digest(&s.expected_digest)
            || s.status.trim().is_empty()
            || s.evidence_state.trim().is_empty()
            || !s.local
            || !s.aggregate_only
        {
            return Err(invalid(
                "subject identity, commits, digests, status, evidence, or locality is invalid",
            ));
        }
        sources.insert(format!("{}:{}", s.subject_id, s.source_commit));
        statuses.insert(format!("{}:{}", s.subject_id, s.status));
        evidence.insert(s.observed_digest.clone());
        if s.evidence_state == "unknown" || s.observed_digest == q.replay_identity {
            unknown.insert(s.subject_id.clone());
        } else if s.observed_digest != s.expected_digest || s.status == "drift" {
            drift.insert(s.subject_id.clone());
        } else if !q.required_subject_order.contains(&s.subject_id) {
            omitted.insert(s.subject_id.clone());
        } else {
            clean.insert(s.subject_id.clone());
        }
    }
    let missing = q
        .required_subject_order
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
        || rows.len() > q.subject_budget;
    if global {
        omitted.extend(seen.iter().cloned());
        clean.clear();
        drift.clear();
        unknown.clear();
    }
    let disposition = if global {
        "blocked"
    } else if !missing.is_empty() || !unknown.is_empty() {
        "unknown"
    } else if !drift.is_empty() || !omitted.is_empty() {
        "partial"
    } else {
        "qualified"
    };
    let body = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"request_id":q.request_id,"purpose":q.purpose,"disposition":disposition,"subject_order":seen.iter().cloned().collect::<Vec<_>>()});
    let closure_digest = ContentHash::of_value(&body)
        .map_err(|e| AuditIntegrityError::Digest(e.to_string()))?
        .to_string();
    let effect_receipts = if disposition == "qualified" {
        vec![format!("approve:audit:{}", q.request_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let clean_order = clean.into_iter().collect::<Vec<_>>();
    let drift_order = drift.into_iter().collect::<Vec<_>>();
    let unknown_order = unknown.into_iter().collect::<Vec<_>>();
    let omitted_order = omitted.into_iter().collect::<Vec<_>>();
    let artifact = AuditArtifact4 {
        artifact_id: format!("sweep-audit:{}", q.request_id),
        content_type: CONTENT_TYPE.into(),
        content_hash: closure_digest.clone(),
        semantic_loss: omitted_order.clone(),
        evidence_digests: evidence.into_iter().collect(),
        boundary: BOUNDARY.into(),
    };
    let c = AuditCard7 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: contract_version.into(),
        feature_id: feature_id.into(),
        request_id: q.request_id.clone(),
        purpose: q.purpose.clone(),
        disposition: disposition.into(),
        subject_order: body["subject_order"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        clean_order,
        drift_order,
        unknown_order,
        omitted_order,
        source_order: sources.into_iter().collect(),
        status_order: statuses.into_iter().collect(),
        evidence_order: evidence_order(&artifact),
        replay_identity: q.replay_identity.clone(),
        closure_digest,
        raw_data_local: true,
        aggregate_only: true,
        boundary: BOUNDARY.into(),
        effect_receipts,
        artifact,
    };
    validate_card(&c)?;
    let _ = (scale, mode, missing);
    Ok(c)
}
fn evidence_order(a: &AuditArtifact4) -> Vec<String> {
    let mut v = a.evidence_digests.clone();
    v.sort();
    v.dedup();
    v
}
