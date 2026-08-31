//! Portable evidence and bounded indexing for workflow execution receipts.
//!
//! The interweave execution route intentionally returns a receipt instead of pretending that a
//! local simulator is an external provider.  A receipt is still useful after the request ends,
//! however, if its workflow binding, adaptive receipt, provenance counts, subject, and domain
//! labels can be independently rechecked.  This module is that bridge.  It creates a portable
//! evidence record from an already-produced workflow receipt and supplies a small digest-keyed
//! registry for later query, replay preparation, and cross-transport handoff.
//!
//! The record is evidence of a bounded receipt contract only.  Simulated and replayed rows stay
//! distinguishable from observed rows; registry presence is not execution authority; and no
//! record here authorizes a release, clinical action, publication, rollback, or other external
//! effect.

use bioprism_ids::ContentHash;
use bioprism_interweave::workflow_execution::{WorkflowExecutionBinding, WorkflowExecutionReceipt};
use serde_json::{json, Map, Value};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const WORKFLOW_EXECUTION_EVIDENCE_SCHEMA_VERSION: &str =
    "bioprism-devplat-workflow-execution-evidence/0.1";
pub const WORKFLOW_EXECUTION_EVIDENCE_WORKFLOW: &str = "interweave_workflow_execution_evidence";
pub const WORKFLOW_EXECUTION_EVIDENCE_IMPORT_SCHEMA_VERSION: &str =
    "bioprism-devplat-workflow-execution-evidence-import/0.1";
pub const WORKFLOW_EXECUTION_EVIDENCE_QUERY_SCHEMA_VERSION: &str =
    "bioprism-devplat-workflow-execution-evidence-query/0.1";
pub const WORKFLOW_EXECUTION_EVIDENCE_GET_SCHEMA_VERSION: &str =
    "bioprism-devplat-workflow-execution-evidence-get/0.1";
pub const WORKFLOW_EXECUTION_EVIDENCE_REGISTRY_SCHEMA_VERSION: &str =
    "bioprism-devplat-workflow-execution-evidence-registry/0.1";
pub const MAX_WORKFLOW_EXECUTION_EVIDENCE_BYTES: usize = 8 * 1024 * 1024;
pub const MAX_WORKFLOW_EXECUTION_EVIDENCE_DOMAINS: usize = 64;
pub const MAX_WORKFLOW_EXECUTION_EVIDENCE_PARENTS: usize = 128;
pub const MAX_WORKFLOW_EXECUTION_EVIDENCE_CAPABILITIES: usize = 128;
pub const MAX_WORKFLOW_EXECUTION_EVIDENCE_RECORDS: usize = 512;
pub const MAX_WORKFLOW_EXECUTION_EVIDENCE_QUERY_ITEMS: usize = 256;
pub const MAX_WORKFLOW_EXECUTION_EVIDENCE_TEXT_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WorkflowExecutionEvidenceError {
    #[error("workflow execution evidence must be a JSON object")]
    NotObject,
    #[error("workflow execution evidence field {0} is missing or invalid")]
    InvalidField(String),
    #[error("workflow execution evidence field {field} exceeds the {maximum}-byte bound")]
    TextTooLarge { field: String, maximum: usize },
    #[error("workflow execution evidence field {field} exceeds the {maximum}-item bound")]
    TooManyItems { field: String, maximum: usize },
    #[error("workflow execution evidence is {actual} bytes, above the {maximum}-byte bound")]
    TooLarge { actual: usize, maximum: usize },
    #[error("workflow execution receipt or binding is invalid: {0}")]
    InvalidReceipt(String),
    #[error("workflow execution evidence digest mismatch for {0}")]
    DigestMismatch(String),
    #[error("workflow execution evidence could not be canonicalised: {0}")]
    Canonicalisation(String),
    #[error("workflow execution evidence registry has reached its {maximum}-record limit")]
    Full { maximum: usize },
    #[error("workflow execution evidence registry has a conflicting record for {digest}")]
    Conflict { digest: String },
    #[error("workflow execution evidence {digest} was not found")]
    NotFound { digest: String },
    #[error("workflow execution evidence snapshot is invalid: {0}")]
    InvalidSnapshot(String),
    #[error(
        "workflow execution evidence snapshot is {actual} bytes, above the {maximum}-byte bound"
    )]
    SnapshotTooLarge { actual: usize, maximum: usize },
    #[error("workflow execution evidence generation counter is exhausted")]
    GenerationExhausted,
}

/// Bounded, digest-ordered filters for workflow execution evidence registry queries.
///
/// Borrowed filter values keep query construction allocation-free while grouping the wire
/// contract into one extensible input instead of growing a positional argument list.
#[derive(Debug, Clone, Copy)]
pub struct WorkflowExecutionEvidenceQuery<'a> {
    pub workflow_id: Option<&'a str>,
    pub subject_id: Option<&'a str>,
    pub domain: Option<&'a str>,
    pub plan_digest: Option<&'a str>,
    pub binding_digest: Option<&'a str>,
    pub receipt_status: Option<&'a str>,
    pub provenance_mode: Option<&'a str>,
    pub after: Option<&'a str>,
    pub max_items: usize,
    pub include_records: bool,
}

fn required_text(
    object: &Map<String, Value>,
    field: &str,
) -> Result<String, WorkflowExecutionEvidenceError> {
    let value = object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| valid_text(value))
        .ok_or_else(|| WorkflowExecutionEvidenceError::InvalidField(field.into()))?;
    if value.len() > MAX_WORKFLOW_EXECUTION_EVIDENCE_TEXT_BYTES {
        return Err(WorkflowExecutionEvidenceError::TextTooLarge {
            field: field.into(),
            maximum: MAX_WORKFLOW_EXECUTION_EVIDENCE_TEXT_BYTES,
        });
    }
    Ok(value.to_string())
}

fn exact_text(
    object: &Map<String, Value>,
    field: &str,
    expected: &str,
) -> Result<(), WorkflowExecutionEvidenceError> {
    if object.get(field).and_then(Value::as_str) != Some(expected) {
        return Err(WorkflowExecutionEvidenceError::InvalidField(field.into()));
    }
    Ok(())
}

fn bounded_text_set(
    object: &Map<String, Value>,
    field: &str,
    maximum: usize,
    required: bool,
) -> Result<Vec<String>, WorkflowExecutionEvidenceError> {
    let Some(value) = object.get(field) else {
        if required {
            return Err(WorkflowExecutionEvidenceError::InvalidField(field.into()));
        }
        return Ok(Vec::new());
    };
    let values = value
        .as_array()
        .ok_or_else(|| WorkflowExecutionEvidenceError::InvalidField(field.into()))?;
    if values.len() > maximum {
        return Err(WorkflowExecutionEvidenceError::TooManyItems {
            field: field.into(),
            maximum,
        });
    }
    let mut result = BTreeSet::new();
    let mut identity_keys = BTreeSet::new();
    for (index, value) in values.iter().enumerate() {
        let item = value
            .as_str()
            .filter(|item| valid_text(item))
            .ok_or_else(|| {
                WorkflowExecutionEvidenceError::InvalidField(format!("{field}[{index}]"))
            })?;
        if item.len() > MAX_WORKFLOW_EXECUTION_EVIDENCE_TEXT_BYTES {
            return Err(WorkflowExecutionEvidenceError::TextTooLarge {
                field: format!("{field}[{index}]"),
                maximum: MAX_WORKFLOW_EXECUTION_EVIDENCE_TEXT_BYTES,
            });
        }
        if !identity_keys.insert(item.to_ascii_lowercase()) || !result.insert(item.to_string()) {
            return Err(WorkflowExecutionEvidenceError::InvalidField(format!(
                "{field}[{index}]"
            )));
        }
    }
    if required && result.is_empty() {
        return Err(WorkflowExecutionEvidenceError::InvalidField(field.into()));
    }
    Ok(result.into_iter().collect())
}

fn digest_value(value: &Value) -> Result<String, WorkflowExecutionEvidenceError> {
    ContentHash::of_value(value)
        .map(|digest| digest.to_string())
        .map_err(|error| WorkflowExecutionEvidenceError::Canonicalisation(error.to_string()))
}

fn valid_text(value: &str) -> bool {
    !value.trim().is_empty()
        && value == value.trim()
        && value.len() <= MAX_WORKFLOW_EXECUTION_EVIDENCE_TEXT_BYTES
        && !value.chars().any(char::is_control)
}

fn valid_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        && ContentHash::parse(value.to_owned()).is_ok()
}

fn required_digest(
    object: &Map<String, Value>,
    field: &str,
) -> Result<String, WorkflowExecutionEvidenceError> {
    let value = required_text(object, field)?;
    if !valid_digest(&value) {
        return Err(WorkflowExecutionEvidenceError::DigestMismatch(field.into()));
    }
    Ok(value)
}

fn digest_set(
    object: &Map<String, Value>,
    field: &str,
    maximum: usize,
) -> Result<Vec<String>, WorkflowExecutionEvidenceError> {
    let Some(value) = object.get(field) else {
        return Ok(Vec::new());
    };
    let values = value
        .as_array()
        .ok_or_else(|| WorkflowExecutionEvidenceError::InvalidField(field.into()))?;
    if values.len() > maximum {
        return Err(WorkflowExecutionEvidenceError::TooManyItems {
            field: field.into(),
            maximum,
        });
    }
    let mut result = BTreeSet::new();
    for (index, value) in values.iter().enumerate() {
        let digest = value
            .as_str()
            .filter(|digest| valid_text(digest))
            .ok_or_else(|| {
                WorkflowExecutionEvidenceError::InvalidField(format!("{field}[{index}]"))
            })?
            .to_string();
        if !valid_digest(&digest) {
            return Err(WorkflowExecutionEvidenceError::DigestMismatch(format!(
                "{field}[{index}]"
            )));
        }
        if !result.insert(digest) {
            return Err(WorkflowExecutionEvidenceError::InvalidField(format!(
                "{field}[{index}]"
            )));
        }
    }
    Ok(result.into_iter().collect())
}

fn without_digest(value: &Value, field: &str) -> Result<Value, WorkflowExecutionEvidenceError> {
    let mut object = value
        .as_object()
        .cloned()
        .ok_or(WorkflowExecutionEvidenceError::NotObject)?;
    object.remove(field);
    Ok(Value::Object(object))
}

fn encoded_size(value: &Value) -> Result<usize, WorkflowExecutionEvidenceError> {
    serde_json::to_vec(value)
        .map(|bytes| bytes.len())
        .map_err(|error| WorkflowExecutionEvidenceError::Canonicalisation(error.to_string()))
}

fn ensure_size(value: &Value) -> Result<(), WorkflowExecutionEvidenceError> {
    let actual = encoded_size(value)?;
    if actual > MAX_WORKFLOW_EXECUTION_EVIDENCE_BYTES {
        return Err(WorkflowExecutionEvidenceError::TooLarge {
            actual,
            maximum: MAX_WORKFLOW_EXECUTION_EVIDENCE_BYTES,
        });
    }
    Ok(())
}

fn parse_receipt_pair(
    binding_value: &Value,
    receipt_value: &Value,
) -> Result<(WorkflowExecutionBinding, WorkflowExecutionReceipt), WorkflowExecutionEvidenceError> {
    let binding: WorkflowExecutionBinding =
        serde_json::from_value(binding_value.clone()).map_err(|error| {
            WorkflowExecutionEvidenceError::InvalidReceipt(format!("binding: {error}"))
        })?;
    binding
        .validate_identity()
        .map_err(|error| WorkflowExecutionEvidenceError::InvalidReceipt(error.to_string()))?;
    if binding.required_capabilities.len() > MAX_WORKFLOW_EXECUTION_EVIDENCE_CAPABILITIES {
        return Err(WorkflowExecutionEvidenceError::TooManyItems {
            field: "binding.required_capabilities".into(),
            maximum: MAX_WORKFLOW_EXECUTION_EVIDENCE_CAPABILITIES,
        });
    }
    let receipt: WorkflowExecutionReceipt =
        serde_json::from_value(receipt_value.clone()).map_err(|error| {
            WorkflowExecutionEvidenceError::InvalidReceipt(format!("receipt: {error}"))
        })?;
    receipt
        .validate_against(&binding)
        .map_err(|error| WorkflowExecutionEvidenceError::InvalidReceipt(error.to_string()))?;
    Ok((binding, receipt))
}

fn workflow_id_value(binding: &WorkflowExecutionBinding) -> Value {
    serde_json::to_value(binding.workflow).unwrap_or_else(|_| Value::String("unknown".into()))
}

fn status_value(receipt: &WorkflowExecutionReceipt) -> Value {
    serde_json::to_value(receipt.adaptive.status)
        .unwrap_or_else(|_| Value::String("unknown".into()))
}

fn provenance_mode(observed: usize, simulated: usize, replayed: usize) -> &'static str {
    match (observed > 0, simulated > 0, replayed > 0) {
        (false, false, false) => "none",
        (true, false, false) => "observed_declared",
        (false, true, false) => "simulated",
        (false, false, true) => "replayed",
        _ => "mixed",
    }
}

/// Build a canonical, digest-addressed evidence record from an already-produced workflow receipt.
///
/// This function performs no provider call and does not require the adaptive plan body. The
/// binding's self-contained identity and receipt shape are still checked, so a caller cannot
/// index an arbitrary JSON object under a workflow label.
pub fn build_workflow_execution_evidence(
    binding_value: &Value,
    receipt_value: &Value,
    subject_id: &str,
    domains: &[String],
    parent_digests: &[String],
) -> Result<Value, WorkflowExecutionEvidenceError> {
    if !valid_text(subject_id) {
        return Err(WorkflowExecutionEvidenceError::InvalidField(
            "subject_id".into(),
        ));
    }
    if subject_id.len() > MAX_WORKFLOW_EXECUTION_EVIDENCE_TEXT_BYTES {
        return Err(WorkflowExecutionEvidenceError::TextTooLarge {
            field: "subject_id".into(),
            maximum: MAX_WORKFLOW_EXECUTION_EVIDENCE_TEXT_BYTES,
        });
    }
    if domains.is_empty() {
        return Err(WorkflowExecutionEvidenceError::InvalidField(
            "domains".into(),
        ));
    }
    if domains.len() > MAX_WORKFLOW_EXECUTION_EVIDENCE_DOMAINS {
        return Err(WorkflowExecutionEvidenceError::TooManyItems {
            field: "domains".into(),
            maximum: MAX_WORKFLOW_EXECUTION_EVIDENCE_DOMAINS,
        });
    }
    let mut domain_set = BTreeSet::new();
    let mut domain_identity_keys = BTreeSet::new();
    for domain in domains {
        if !valid_text(domain) {
            return Err(WorkflowExecutionEvidenceError::InvalidField(
                "domains".into(),
            ));
        }
        if domain.len() > MAX_WORKFLOW_EXECUTION_EVIDENCE_TEXT_BYTES {
            return Err(WorkflowExecutionEvidenceError::TextTooLarge {
                field: "domains".into(),
                maximum: MAX_WORKFLOW_EXECUTION_EVIDENCE_TEXT_BYTES,
            });
        }
        if !domain_identity_keys.insert(domain.to_ascii_lowercase())
            || !domain_set.insert(domain.clone())
        {
            return Err(WorkflowExecutionEvidenceError::InvalidField(
                "domains".into(),
            ));
        }
    }
    if parent_digests.len() > MAX_WORKFLOW_EXECUTION_EVIDENCE_PARENTS {
        return Err(WorkflowExecutionEvidenceError::TooManyItems {
            field: "parent_digests".into(),
            maximum: MAX_WORKFLOW_EXECUTION_EVIDENCE_PARENTS,
        });
    }
    let mut parents = BTreeSet::new();
    for digest in parent_digests {
        if !valid_digest(digest) {
            return Err(WorkflowExecutionEvidenceError::DigestMismatch(
                "parent_digests".into(),
            ));
        }
        if !parents.insert(digest.clone()) {
            return Err(WorkflowExecutionEvidenceError::InvalidField(
                "parent_digests".into(),
            ));
        }
    }
    let (binding, receipt) = parse_receipt_pair(binding_value, receipt_value)?;
    let receipt_digest = digest_value(receipt_value)?;
    let (observed, simulated, replayed) = receipt.provenance_counts();
    let status = status_value(&receipt);
    let mode = provenance_mode(observed, simulated, replayed);
    let workflow_id = workflow_id_value(&binding);
    let mut evidence = json!({
        "schema": WORKFLOW_EXECUTION_EVIDENCE_SCHEMA_VERSION,
        "workflow": WORKFLOW_EXECUTION_EVIDENCE_WORKFLOW,
        "workflow_id": workflow_id,
        "subject_id": subject_id,
        "domains": domain_set.into_iter().collect::<Vec<_>>(),
        "binding_digest": binding.binding_digest,
        "plan_digest": binding.adaptive_plan_digest,
        "workflow_spec_digest": binding.workflow_spec_digest,
        "provider_id": binding.provider_id,
        "receipt_digest": receipt_digest,
        "receipt_status": status,
        "completed": receipt.is_completed(),
        "provenance": {
            "mode": mode,
            "observed": observed,
            "simulated": simulated,
            "replayed": replayed,
            "observation_count": receipt.adaptive.observations.len()
        },
        "binding": binding,
        "receipt": receipt,
        "parent_digests": parents.into_iter().collect::<Vec<_>>(),
        "claim_posture": {
            "status": "review_required",
            "does_not_claim": [
                "completion of any forbidden workflow effect",
                "provider authentication or consent",
                "scientific, clinical, causal, operational, publication, or release validity",
            ],
            "limitations": [
                "provenance labels are preserved declarations and are not independently authenticated",
                "simulated and replayed observations are not observed-world measurements",
                "the receipt does not contain the adaptive plan body"
            ]
        },
        "readiness_claimed": false,
        "execution": "not_started",
        "guarantees": [
            "binding identity, workflow specification, provider, plan, and effect prohibitions were checked before indexing",
            "the evidence digest covers the canonical record independently of the transport envelope",
            "the receipt was validated against the binding and its provenance counts are retained without collapse"
        ],
        "does_not_claim": [
            "a valid receipt proves that a provider actually performed an external operation",
            "an observed label proves authenticity, consent, chain of custody, or domain truth",
            "registry presence authorizes release, publication, rollback, patient-level action, or any external effect"
        ]
    });
    let evidence_digest = digest_value(&without_digest(&evidence, "evidence_digest")?)?;
    evidence["evidence_digest"] = Value::String(evidence_digest);
    ensure_size(&evidence)?;
    validate_workflow_execution_evidence(&evidence)?;
    Ok(evidence)
}

/// Validate a portable workflow execution evidence record before indexing or handoff.
pub fn validate_workflow_execution_evidence(
    evidence: &Value,
) -> Result<(), WorkflowExecutionEvidenceError> {
    ensure_size(evidence)?;
    let object = evidence
        .as_object()
        .ok_or(WorkflowExecutionEvidenceError::NotObject)?;
    exact_text(object, "schema", WORKFLOW_EXECUTION_EVIDENCE_SCHEMA_VERSION)?;
    exact_text(object, "workflow", WORKFLOW_EXECUTION_EVIDENCE_WORKFLOW)?;
    required_text(object, "subject_id")?;
    let domains = bounded_text_set(
        object,
        "domains",
        MAX_WORKFLOW_EXECUTION_EVIDENCE_DOMAINS,
        true,
    )?;
    let _ = required_digest(object, "binding_digest")?;
    let _ = required_digest(object, "plan_digest")?;
    let _ = required_digest(object, "workflow_spec_digest")?;
    let receipt_digest = required_digest(object, "receipt_digest")?;
    let parent_digests = digest_set(
        object,
        "parent_digests",
        MAX_WORKFLOW_EXECUTION_EVIDENCE_PARENTS,
    )?;
    let binding_value = object
        .get("binding")
        .ok_or_else(|| WorkflowExecutionEvidenceError::InvalidField("binding".into()))?;
    let receipt_value = object
        .get("receipt")
        .ok_or_else(|| WorkflowExecutionEvidenceError::InvalidField("receipt".into()))?;
    let (binding, receipt) = parse_receipt_pair(binding_value, receipt_value)?;
    let workflow_id = workflow_id_value(&binding);
    if object.get("workflow_id") != Some(&workflow_id)
        || object.get("binding_digest").and_then(Value::as_str)
            != Some(binding.binding_digest.as_str())
        || object.get("plan_digest").and_then(Value::as_str)
            != Some(binding.adaptive_plan_digest.as_str())
        || object.get("workflow_spec_digest").and_then(Value::as_str)
            != Some(binding.workflow_spec_digest.as_str())
        || object.get("provider_id").and_then(Value::as_str) != Some(binding.provider_id.as_str())
    {
        return Err(WorkflowExecutionEvidenceError::DigestMismatch(
            "binding identity".into(),
        ));
    }
    if digest_value(receipt_value)? != receipt_digest {
        return Err(WorkflowExecutionEvidenceError::DigestMismatch(
            "receipt_digest".into(),
        ));
    }
    if object.get("domains") != Some(&json!(domains))
        || object.get("parent_digests") != Some(&json!(parent_digests))
    {
        return Err(WorkflowExecutionEvidenceError::InvalidField(
            "canonical label sets".into(),
        ));
    }
    let expected_status = status_value(&receipt);
    if object.get("receipt_status") != Some(&expected_status)
        || object.get("completed") != Some(&Value::Bool(receipt.is_completed()))
    {
        return Err(WorkflowExecutionEvidenceError::InvalidField(
            "receipt status".into(),
        ));
    }
    let (observed, simulated, replayed) = receipt.provenance_counts();
    let provenance = object
        .get("provenance")
        .and_then(Value::as_object)
        .ok_or_else(|| WorkflowExecutionEvidenceError::InvalidField("provenance".into()))?;
    if provenance.get("mode").and_then(Value::as_str)
        != Some(provenance_mode(observed, simulated, replayed))
        || provenance.get("observed").and_then(Value::as_u64) != Some(observed as u64)
        || provenance.get("simulated").and_then(Value::as_u64) != Some(simulated as u64)
        || provenance.get("replayed").and_then(Value::as_u64) != Some(replayed as u64)
        || provenance.get("observation_count").and_then(Value::as_u64)
            != Some(receipt.adaptive.observations.len() as u64)
    {
        return Err(WorkflowExecutionEvidenceError::InvalidField(
            "provenance".into(),
        ));
    }
    exact_text(object, "execution", "not_started")?;
    if object.get("readiness_claimed") != Some(&Value::Bool(false)) {
        return Err(WorkflowExecutionEvidenceError::InvalidField(
            "readiness_claimed".into(),
        ));
    }
    let claim_posture = object
        .get("claim_posture")
        .and_then(Value::as_object)
        .ok_or_else(|| WorkflowExecutionEvidenceError::InvalidField("claim_posture".into()))?;
    exact_text(claim_posture, "status", "review_required")?;
    let claim_non_claims = bounded_text_set(claim_posture, "does_not_claim", 16, true)?;
    let claim_limitations = bounded_text_set(claim_posture, "limitations", 16, true)?;
    if claim_posture.get("does_not_claim") != Some(&json!(claim_non_claims))
        || claim_posture.get("limitations") != Some(&json!(claim_limitations))
    {
        return Err(WorkflowExecutionEvidenceError::InvalidField(
            "claim_posture canonical label sets".into(),
        ));
    }
    let guarantees = bounded_text_set(object, "guarantees", 16, true)?;
    let does_not_claim = bounded_text_set(object, "does_not_claim", 16, true)?;
    if object.get("guarantees") != Some(&json!(guarantees))
        || object.get("does_not_claim") != Some(&json!(does_not_claim))
    {
        return Err(WorkflowExecutionEvidenceError::InvalidField(
            "canonical label sets".into(),
        ));
    }
    let declared_digest = required_digest(object, "evidence_digest")?;
    let recomputed = digest_value(&without_digest(evidence, "evidence_digest")?)?;
    if declared_digest != recomputed {
        return Err(WorkflowExecutionEvidenceError::DigestMismatch(
            "evidence_digest".into(),
        ));
    }
    Ok(())
}

#[derive(Debug, Clone, Default)]
pub struct WorkflowExecutionEvidenceRegistry {
    generation: u64,
    records: BTreeMap<String, Value>,
}

impl WorkflowExecutionEvidenceRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }

    pub fn digests_for_audit(&self) -> Vec<String> {
        self.records.keys().cloned().collect()
    }

    /// Import one validated evidence record. Identical imports are idempotent; a digest collision
    /// carrying different bytes is refused rather than overwritten.
    pub fn import(&mut self, evidence: &Value) -> Result<Value, WorkflowExecutionEvidenceError> {
        validate_workflow_execution_evidence(evidence)?;
        let digest = evidence
            .get("evidence_digest")
            .and_then(Value::as_str)
            .ok_or_else(|| WorkflowExecutionEvidenceError::InvalidField("evidence_digest".into()))?
            .to_string();
        let already_present = self
            .records
            .get(&digest)
            .is_some_and(|existing| existing == evidence);
        if !already_present && self.records.contains_key(&digest) {
            return Err(WorkflowExecutionEvidenceError::Conflict { digest });
        }
        if !already_present && self.records.len() >= MAX_WORKFLOW_EXECUTION_EVIDENCE_RECORDS {
            return Err(WorkflowExecutionEvidenceError::Full {
                maximum: MAX_WORKFLOW_EXECUTION_EVIDENCE_RECORDS,
            });
        }
        if !already_present {
            let mut candidate = self.clone();
            candidate.records.insert(digest.clone(), evidence.clone());
            candidate.generation = candidate
                .generation
                .checked_add(1)
                .ok_or(WorkflowExecutionEvidenceError::GenerationExhausted)?;
            candidate.ensure_snapshot_bound()?;
            self.records = candidate.records;
            self.generation = candidate.generation;
        }
        Ok(json!({
            "ok": true,
            "schema": WORKFLOW_EXECUTION_EVIDENCE_IMPORT_SCHEMA_VERSION,
            "workflow": "interweave_workflow_execution_evidence_import",
            "evidence_digest": digest,
            "created": !already_present,
            "already_present": already_present,
            "registry_generation": self.generation,
            "registry_size": self.records.len(),
            "execution": "not_started",
            "guarantees": [
                "only a binding- and receipt-validated evidence record is indexed",
                "re-importing identical canonical bytes is idempotent",
                "import never executes, retries, or resumes a workflow"
            ],
            "does_not_claim": [
                "registry presence proves provider execution, domain truth, or release approval",
                "a simulated or replayed receipt is equivalent to an observed-world result"
            ]
        }))
    }

    pub fn get(&self, digest: &str) -> Result<Value, WorkflowExecutionEvidenceError> {
        if !valid_digest(digest) {
            return Err(WorkflowExecutionEvidenceError::DigestMismatch(
                "evidence_digest".into(),
            ));
        }
        let record =
            self.records
                .get(digest)
                .ok_or_else(|| WorkflowExecutionEvidenceError::NotFound {
                    digest: digest.to_string(),
                })?;
        Ok(json!({
            "ok": true,
            "schema": WORKFLOW_EXECUTION_EVIDENCE_GET_SCHEMA_VERSION,
            "workflow": "interweave_workflow_execution_evidence_get",
            "evidence_digest": digest,
            "record": record,
            "execution": "not_started",
            "guarantees": [
                "the returned digest identifies the exact validated evidence record",
                "lookup does not execute or re-evaluate the underlying workflow"
            ],
            "does_not_claim": [
                "the record establishes scientific, clinical, operational, publication, or release validity"
            ]
        }))
    }

    /// Query bounded digest-ordered rows. Full records are opt-in because the receipt may carry
    /// provider-declared observation payloads even when the registry is only being browsed.
    pub fn query(
        &self,
        query: &WorkflowExecutionEvidenceQuery<'_>,
    ) -> Result<Value, WorkflowExecutionEvidenceError> {
        let WorkflowExecutionEvidenceQuery {
            workflow_id,
            subject_id,
            domain,
            plan_digest,
            binding_digest,
            receipt_status,
            provenance_mode,
            after,
            max_items,
            include_records,
        } = *query;
        if !(1..=MAX_WORKFLOW_EXECUTION_EVIDENCE_QUERY_ITEMS).contains(&max_items) {
            return Err(WorkflowExecutionEvidenceError::InvalidField(
                "max_items".into(),
            ));
        }
        for (field, value) in [
            ("plan_digest", plan_digest),
            ("binding_digest", binding_digest),
            ("after", after),
        ] {
            if let Some(value) = value {
                if !valid_digest(value) {
                    return Err(WorkflowExecutionEvidenceError::DigestMismatch(field.into()));
                }
            }
        }
        for (field, value) in [
            ("workflow_id", workflow_id),
            ("subject_id", subject_id),
            ("domain", domain),
            ("receipt_status", receipt_status),
            ("provenance_mode", provenance_mode),
        ] {
            if value.is_some_and(|value| !valid_text(value)) {
                return Err(WorkflowExecutionEvidenceError::InvalidField(field.into()));
            }
        }
        let mut rows = Vec::new();
        let mut has_more = false;
        for (digest, record) in self
            .records
            .iter()
            .filter(|(digest, _)| after.is_none_or(|cursor| digest.as_str() > cursor))
        {
            let matches = workflow_id.is_none_or(|value| {
                record.get("workflow_id").and_then(Value::as_str) == Some(value)
            }) && subject_id.is_none_or(|value| {
                record.get("subject_id").and_then(Value::as_str) == Some(value)
            }) && domain.is_none_or(|value| {
                record
                    .get("domains")
                    .and_then(Value::as_array)
                    .is_some_and(|domains| domains.iter().any(|item| item.as_str() == Some(value)))
            }) && plan_digest.is_none_or(|value| {
                record.get("plan_digest").and_then(Value::as_str) == Some(value)
            }) && binding_digest.is_none_or(|value| {
                record.get("binding_digest").and_then(Value::as_str) == Some(value)
            }) && receipt_status.is_none_or(|value| {
                record.get("receipt_status").and_then(Value::as_str) == Some(value)
            }) && provenance_mode.is_none_or(|value| {
                record.pointer("/provenance/mode").and_then(Value::as_str) == Some(value)
            });
            if !matches {
                continue;
            }
            if rows.len() >= max_items {
                has_more = true;
                break;
            }
            let mut row = json!({
                "evidence_digest": digest,
                "workflow_id": record.get("workflow_id"),
                "subject_id": record.get("subject_id"),
                "domains": record.get("domains"),
                "binding_digest": record.get("binding_digest"),
                "plan_digest": record.get("plan_digest"),
                "provider_id": record.get("provider_id"),
                "receipt_status": record.get("receipt_status"),
                "completed": record.get("completed"),
                "provenance": record.get("provenance")
            });
            if include_records {
                row["record"] = record.clone();
            }
            rows.push(row);
        }
        let next_after = if has_more {
            rows.last()
                .and_then(|row| row.get("evidence_digest"))
                .cloned()
                .unwrap_or(Value::Null)
        } else {
            Value::Null
        };
        Ok(json!({
            "ok": true,
            "schema": WORKFLOW_EXECUTION_EVIDENCE_QUERY_SCHEMA_VERSION,
            "workflow": "interweave_workflow_execution_evidence_query",
            "filters": {
                "workflow_id": workflow_id,
                "subject_id": subject_id,
                "domain": domain,
                "plan_digest": plan_digest,
                "binding_digest": binding_digest,
                "receipt_status": receipt_status,
                "provenance_mode": provenance_mode,
                "after": after,
                "max_items": max_items,
                "include_records": include_records
            },
            "registry_generation": self.generation,
            "registry_size": self.records.len(),
            "rows": rows,
            "next_after": next_after,
            "has_more": has_more,
            "execution": "not_started",
            "guarantees": [
                "rows are ordered by evidence digest and filtered by retained structural metadata",
                "full records are returned only when explicitly requested",
                "query never executes, retries, or re-evaluates a workflow"
            ],
            "does_not_claim": [
                "absence from this bounded registry means a workflow never ran",
                "a matching row establishes provider authenticity or domain validity"
            ]
        }))
    }

    pub fn snapshot(&self) -> Result<Value, WorkflowExecutionEvidenceError> {
        let mut document = json!({
            "schema": WORKFLOW_EXECUTION_EVIDENCE_REGISTRY_SCHEMA_VERSION,
            "generation": self.generation,
            "record_count": self.records.len(),
            "records": self.records.iter().map(|(digest, record)| json!({
                "evidence_digest": digest,
                "record": record
            })).collect::<Vec<_>>(),
            "retention": {
                "max_records": MAX_WORKFLOW_EXECUTION_EVIDENCE_RECORDS,
                "max_bytes": MAX_WORKFLOW_EXECUTION_EVIDENCE_BYTES
            },
            "execution": "not_started"
        });
        let state_digest = digest_value(&document)?;
        document["state_digest"] = Value::String(state_digest);
        let actual = encoded_size(&document)?;
        if actual > MAX_WORKFLOW_EXECUTION_EVIDENCE_BYTES {
            return Err(WorkflowExecutionEvidenceError::SnapshotTooLarge {
                actual,
                maximum: MAX_WORKFLOW_EXECUTION_EVIDENCE_BYTES,
            });
        }
        Ok(document)
    }

    pub fn from_snapshot(document: &Value) -> Result<Self, WorkflowExecutionEvidenceError> {
        let object = document.as_object().ok_or_else(|| {
            WorkflowExecutionEvidenceError::InvalidSnapshot("snapshot must be an object".into())
        })?;
        let actual = encoded_size(document)?;
        if actual > MAX_WORKFLOW_EXECUTION_EVIDENCE_BYTES {
            return Err(WorkflowExecutionEvidenceError::SnapshotTooLarge {
                actual,
                maximum: MAX_WORKFLOW_EXECUTION_EVIDENCE_BYTES,
            });
        }
        exact_text(
            object,
            "schema",
            WORKFLOW_EXECUTION_EVIDENCE_REGISTRY_SCHEMA_VERSION,
        )?;
        exact_text(object, "execution", "not_started")?;
        let expected_retention = json!({
            "max_records": MAX_WORKFLOW_EXECUTION_EVIDENCE_RECORDS,
            "max_bytes": MAX_WORKFLOW_EXECUTION_EVIDENCE_BYTES
        });
        if object.get("retention") != Some(&expected_retention) {
            return Err(WorkflowExecutionEvidenceError::InvalidSnapshot(
                "retention contract does not match the registry bounds".into(),
            ));
        }
        let state_digest = required_digest(object, "state_digest")?;
        let unsigned = without_digest(document, "state_digest")?;
        if digest_value(&unsigned)? != state_digest {
            return Err(WorkflowExecutionEvidenceError::InvalidSnapshot(
                "state_digest does not match snapshot contents".into(),
            ));
        }
        let generation = object
            .get("generation")
            .and_then(Value::as_u64)
            .ok_or_else(|| {
                WorkflowExecutionEvidenceError::InvalidSnapshot("generation is invalid".into())
            })?;
        let rows = object
            .get("records")
            .and_then(Value::as_array)
            .ok_or_else(|| {
                WorkflowExecutionEvidenceError::InvalidSnapshot("records must be an array".into())
            })?;
        if rows.len() > MAX_WORKFLOW_EXECUTION_EVIDENCE_RECORDS {
            return Err(WorkflowExecutionEvidenceError::Full {
                maximum: MAX_WORKFLOW_EXECUTION_EVIDENCE_RECORDS,
            });
        }
        if generation < rows.len() as u64 {
            return Err(WorkflowExecutionEvidenceError::InvalidSnapshot(
                "generation cannot be below the retained record count".into(),
            ));
        }
        if object.get("record_count").and_then(Value::as_u64) != Some(rows.len() as u64) {
            return Err(WorkflowExecutionEvidenceError::InvalidSnapshot(
                "record_count does not match records".into(),
            ));
        }
        let mut registry = Self {
            generation,
            records: BTreeMap::new(),
        };
        for row in rows {
            let row_object = row.as_object().ok_or_else(|| {
                WorkflowExecutionEvidenceError::InvalidSnapshot(
                    "record row must be an object".into(),
                )
            })?;
            let digest = required_digest(row_object, "evidence_digest")?;
            let record = row_object.get("record").ok_or_else(|| {
                WorkflowExecutionEvidenceError::InvalidSnapshot("record is missing".into())
            })?;
            validate_workflow_execution_evidence(record)?;
            if record.get("evidence_digest").and_then(Value::as_str) != Some(digest.as_str()) {
                return Err(WorkflowExecutionEvidenceError::InvalidSnapshot(
                    "row digest does not match record digest".into(),
                ));
            }
            if registry.records.insert(digest, record.clone()).is_some() {
                return Err(WorkflowExecutionEvidenceError::InvalidSnapshot(
                    "snapshot contains duplicate evidence digests".into(),
                ));
            }
        }
        registry.ensure_snapshot_bound()?;
        Ok(registry)
    }

    fn ensure_snapshot_bound(&self) -> Result<(), WorkflowExecutionEvidenceError> {
        let snapshot = self.snapshot()?;
        let actual = encoded_size(&snapshot)?;
        if actual > MAX_WORKFLOW_EXECUTION_EVIDENCE_BYTES {
            return Err(WorkflowExecutionEvidenceError::SnapshotTooLarge {
                actual,
                maximum: MAX_WORKFLOW_EXECUTION_EVIDENCE_BYTES,
            });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bioprism_epistemic::{
        Acquisition, AdaptivePlan, Belief, DecisionProblem, ExecutionGrant, ScriptedExecutor,
    };
    use bioprism_interweave::workflow::WorkflowId;

    fn pair() -> (Value, Value) {
        let problem = DecisionProblem::new(
            vec!["hold".into(), "release".into()],
            vec!["safe".into(), "unsafe".into()],
            vec![0.0, 2.0, 2.0, 0.0],
        )
        .unwrap();
        let belief = Belief::new(vec![0.6, 0.4]).unwrap();
        let acquisition = Acquisition::new(
            "screen",
            0.1,
            vec![
                bioprism_epistemic::Outcome::new("negative", vec![0.9, 0.2]),
                bioprism_epistemic::Outcome::new("positive", vec![0.1, 0.8]),
            ],
            2,
        )
        .unwrap();
        let plan = AdaptivePlan::new(problem, belief, vec![acquisition], 1.0, 1).unwrap();
        let binding = WorkflowExecutionBinding::bind(
            WorkflowId::BiomedicalResearchDataAudit,
            &plan,
            "test-provider",
            ["data.read".into(), "analysis.sandbox".into()],
        )
        .unwrap();
        let digest = plan.digest().unwrap();
        let grant = ExecutionGrant::issue("grant", digest, "test-provider").unwrap();
        let mut executor = ScriptedExecutor::simulated(
            "test-provider",
            vec![("screen".into(), "negative".into())],
        );
        let receipt = binding.execute(&plan, Some(&grant), &mut executor).unwrap();
        (
            serde_json::to_value(binding).unwrap(),
            serde_json::to_value(receipt).unwrap(),
        )
    }

    #[test]
    fn builds_and_validates_simulated_evidence_without_upgrading_provenance() {
        let (binding, receipt) = pair();
        let evidence = build_workflow_execution_evidence(
            &binding,
            &receipt,
            "subject-1",
            &["biomedical_research".into(), "privacy".into()],
            &[],
        )
        .unwrap();
        validate_workflow_execution_evidence(&evidence).unwrap();
        assert_eq!(evidence["provenance"]["mode"], "simulated");
        assert_eq!(evidence["readiness_claimed"], false);
        assert_eq!(evidence["claim_posture"]["status"], "review_required");
    }

    #[test]
    fn registry_import_is_idempotent_and_snapshot_round_trips() {
        let (binding, receipt) = pair();
        let evidence = build_workflow_execution_evidence(
            &binding,
            &receipt,
            "subject-1",
            &["biomedical_research".into()],
            &[],
        )
        .unwrap();
        let mut registry = WorkflowExecutionEvidenceRegistry::new();
        let first = registry.import(&evidence).unwrap();
        let second = registry.import(&evidence).unwrap();
        assert_eq!(first["created"], true);
        assert_eq!(second["already_present"], true);
        let snapshot = registry.snapshot().unwrap();
        let restored = WorkflowExecutionEvidenceRegistry::from_snapshot(&snapshot).unwrap();
        assert_eq!(restored.digests_for_audit(), registry.digests_for_audit());
        let query = restored
            .query(&WorkflowExecutionEvidenceQuery {
                workflow_id: Some("biomedical_research_data_audit"),
                subject_id: None,
                domain: None,
                plan_digest: None,
                binding_digest: None,
                receipt_status: None,
                provenance_mode: None,
                after: None,
                max_items: 10,
                include_records: false,
            })
            .unwrap();
        assert_eq!(query["rows"].as_array().unwrap().len(), 1);
    }

    #[test]
    fn snapshot_restore_rejects_contract_drift_and_generation_regression() {
        let (binding, receipt) = pair();
        let evidence = build_workflow_execution_evidence(
            &binding,
            &receipt,
            "subject-contract",
            &["software".into()],
            &[],
        )
        .unwrap();
        let mut registry = WorkflowExecutionEvidenceRegistry::new();
        registry.import(&evidence).unwrap();
        let snapshot = registry.snapshot().unwrap();

        let mut retention_drift = snapshot.clone();
        retention_drift["retention"]["max_bytes"] = json!(1);
        retention_drift
            .as_object_mut()
            .unwrap()
            .remove("state_digest");
        retention_drift["state_digest"] = json!(digest_value(&retention_drift).unwrap());
        let error = WorkflowExecutionEvidenceRegistry::from_snapshot(&retention_drift)
            .expect_err("retention drift must be refused");
        assert!(error.to_string().contains("retention contract"));

        let mut generation_regression = snapshot;
        generation_regression["generation"] = json!(0);
        generation_regression
            .as_object_mut()
            .unwrap()
            .remove("state_digest");
        generation_regression["state_digest"] =
            json!(digest_value(&generation_regression).unwrap());
        let error = WorkflowExecutionEvidenceRegistry::from_snapshot(&generation_regression)
            .expect_err("generation regression must be refused");
        assert!(error.to_string().contains("generation cannot be below"));
    }

    #[test]
    fn tampering_with_receipt_or_digest_is_refused() {
        let (binding, mut receipt) = pair();
        receipt["adaptive"]["provider"] = json!("other-provider");
        assert!(build_workflow_execution_evidence(
            &binding,
            &receipt,
            "subject-1",
            &["software".into()],
            &[],
        )
        .is_err());
        let (_, receipt) = pair();
        let mut evidence = build_workflow_execution_evidence(
            &binding,
            &receipt,
            "subject-1",
            &["software".into()],
            &[],
        )
        .unwrap();
        evidence["subject_id"] = json!("tampered");
        assert!(validate_workflow_execution_evidence(&evidence).is_err());
    }

    #[test]
    fn duplicate_domains_and_parent_edges_are_refused() {
        let (binding, receipt) = pair();
        assert!(build_workflow_execution_evidence(
            &binding,
            &receipt,
            "subject-1",
            &["software".into(), "Software".into()],
            &[]
        )
        .is_err());
        assert!(build_workflow_execution_evidence(
            &binding,
            &receipt,
            "subject-1",
            &["software".into()],
            &["a".repeat(64), "a".repeat(64)]
        )
        .is_err());

        assert!(build_workflow_execution_evidence(
            &binding,
            &receipt,
            " subject-1",
            &["software".into()],
            &[]
        )
        .is_err());
    }

    #[test]
    fn evidence_rejects_control_metadata_and_noncanonical_digests() {
        let (binding, receipt) = pair();
        assert!(build_workflow_execution_evidence(
            &binding,
            &receipt,
            "subject\u{0000}one",
            &["software".into()],
            &[]
        )
        .is_err());
        assert!(build_workflow_execution_evidence(
            &binding,
            &receipt,
            "subject-one",
            &["software".into()],
            &["A".repeat(64)]
        )
        .is_err());

        let mut evidence = build_workflow_execution_evidence(
            &binding,
            &receipt,
            "subject-one",
            &["software".into()],
            &[],
        )
        .unwrap();
        evidence["binding_digest"] = json!("A".repeat(64));
        assert!(validate_workflow_execution_evidence(&evidence).is_err());
    }

    #[test]
    fn evidence_rejects_duplicate_parent_edges_after_resealing() {
        let (binding, receipt) = pair();
        let parent = "a".repeat(64);
        let mut evidence = build_workflow_execution_evidence(
            &binding,
            &receipt,
            "subject-one",
            &["software".into()],
            std::slice::from_ref(&parent),
        )
        .unwrap();
        evidence["parent_digests"] = json!([parent.clone(), parent]);
        evidence
            .as_object_mut()
            .expect("evidence is an object")
            .remove("evidence_digest");
        let digest = digest_value(&evidence).unwrap();
        evidence["evidence_digest"] = json!(digest);
        assert!(validate_workflow_execution_evidence(&evidence).is_err());
    }

    #[test]
    fn evidence_rejects_noncanonical_claim_metadata_after_resealing() {
        let (binding, receipt) = pair();
        let mut evidence = build_workflow_execution_evidence(
            &binding,
            &receipt,
            "subject-one",
            &["software".into()],
            &[],
        )
        .unwrap();
        evidence["claim_posture"]["does_not_claim"] = json!([
            "provider authentication or consent",
            "scientific, clinical, causal, operational, publication, or release validity",
            "completion of any forbidden workflow effect"
        ]);
        evidence
            .as_object_mut()
            .expect("evidence is an object")
            .remove("evidence_digest");
        evidence["evidence_digest"] = json!(digest_value(&evidence).unwrap());
        let error = validate_workflow_execution_evidence(&evidence)
            .expect_err("resealed noncanonical claim metadata must be refused");
        assert!(error.to_string().contains("canonical"));
    }

    #[test]
    fn registry_lookup_rejects_uppercase_digest_aliases() {
        let (binding, receipt) = pair();
        let evidence = build_workflow_execution_evidence(
            &binding,
            &receipt,
            "subject-one",
            &["software".into()],
            &[],
        )
        .unwrap();
        let digest = evidence["evidence_digest"].as_str().unwrap();
        let mut registry = WorkflowExecutionEvidenceRegistry::new();
        registry.import(&evidence).unwrap();
        assert!(matches!(
            registry.get(&digest.to_uppercase()),
            Err(WorkflowExecutionEvidenceError::DigestMismatch(_))
        ));
    }
}
