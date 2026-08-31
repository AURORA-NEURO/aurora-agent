//! Docgraph P32: document-module lineage and release integrity.
//!
//! This contract turns a documentation graph into an auditable, release-ready
//! product boundary. It validates typed module identities, parent references,
//! acyclic navigation lineage, evidence posture, and deterministic replay. It
//! emits a card for a named researcher or administrator; it never edits source
//! documents, publishes content, or treats an unresolved module as safe.

use bioprism_foundation::{PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const SCHEMA_VERSION: &str = RESEARCH_CONTRACT_SCHEMA_VERSION;
pub const BOUNDARY: &str = PRECLINICAL_BOUNDARY;
pub const CONTENT_TYPE: &str =
    "application/vnd.aurora.docgraph.document-module-integrity-card-1+json";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocumentModule4 {
    pub module_id: String,
    pub parent_module: String,
    pub owner_crate: String,
    pub consumer: String,
    pub behavior: String,
    pub input_schema: String,
    pub output_schema: String,
    pub source_digest: String,
    pub evidence_state: String,
    pub deterministic: bool,
    pub local: bool,
    pub aggregate_only: bool,
    pub required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocumentGraphIntegrityRequest4 {
    pub schema_version: String,
    pub request_id: String,
    pub purpose: String,
    pub modules: Vec<DocumentModule4>,
    pub required_module_order: Vec<String>,
    pub replay_identity: String,
    pub policy_allowed: bool,
    pub protected_closure: bool,
    pub signed_manifest: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub adversarial_events: Vec<String>,
    pub module_budget: usize,
    pub boundary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocumentGraphIntegrityArtifact4 {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: String,
    pub semantic_loss: Vec<String>,
    pub source_digests: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DocumentGraphIntegrityCard7 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub purpose: String,
    pub disposition: String,
    pub module_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub rejected_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub omitted_order: Vec<String>,
    pub lineage_order: Vec<String>,
    pub consumer_order: Vec<String>,
    pub contract_order: Vec<String>,
    pub replay_identity: String,
    pub closure_digest: String,
    pub admitted_module_count: u64,
    pub total_module_count: u64,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
    pub effect_receipts: Vec<String>,
    pub artifact: DocumentGraphIntegrityArtifact4,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum DocumentGraphIntegrityError {
    #[error("document graph integrity input is invalid: {0}")]
    Invalid(String),
    #[error("document graph integrity digest failed: {0}")]
    Digest(String),
}

fn digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}
fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|window| window[0] < window[1])
}
fn invalid(message: impl Into<String>) -> DocumentGraphIntegrityError {
    DocumentGraphIntegrityError::Invalid(message.into())
}

pub fn manifest(feature_id: &str, contract_version: &str, scale: &str, mode: &str) -> Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "capability_id": feature_id,
        "version": contract_version,
        "owner_crate": "docgraph",
        "consumers": ["documentation curator", "context compiler", "researcher workbench", "release auditor"],
        "behavior": format!("qualify typed document graph lineage at {scale} ({mode})"),
        "value": "prevents orphaned, cyclic, stale, or unauditable documentation context from entering research workflows",
        "input_schema": "DocumentGraphIntegrityRequest4@1",
        "output_schema": "DocumentGraphIntegrityCard7@1",
        "effects": ["emit:document-lineage-card", "retain:rejected-and-unresolved-modules", "block:unsafe-context-release"],
        "permissions": ["read:local-document-manifests", "exchange:aggregate-lineage"],
        "determinism": "byte_stable",
        "autonomy_tier": "A1",
        "boundary": BOUNDARY,
    })
}

fn has_cycle(parent: &BTreeMap<String, String>) -> bool {
    fn visit(
        node: &str,
        parent: &BTreeMap<String, String>,
        active: &mut BTreeSet<String>,
        done: &mut BTreeSet<String>,
    ) -> bool {
        if done.contains(node) {
            return false;
        }
        if !active.insert(node.to_owned()) {
            return true;
        }
        if let Some(next) = parent.get(node) {
            if next != "root" && visit(next, parent, active, done) {
                return true;
            }
        }
        active.remove(node);
        done.insert(node.to_owned());
        false
    }
    let mut active = BTreeSet::new();
    let mut done = BTreeSet::new();
    parent
        .keys()
        .any(|node| visit(node, parent, &mut active, &mut done))
}

fn validate_card(card: &DocumentGraphIntegrityCard7) -> Result<(), DocumentGraphIntegrityError> {
    if card.schema_version != SCHEMA_VERSION
        || card.feature_id.is_empty()
        || card.request_id.is_empty()
        || card.purpose.is_empty()
        || card.boundary != BOUNDARY
        || card.artifact.boundary != BOUNDARY
        || !card.raw_data_local
        || !card.aggregate_only
        || !digest(&card.replay_identity)
        || !digest(&card.closure_digest)
        || card.artifact.content_type != CONTENT_TYPE
        || card.artifact.content_hash != card.closure_digest
        || card.admitted_module_count > card.total_module_count
    {
        return Err(invalid(
            "document identity, locality, artifact, digest, boundary, or count is incomplete",
        ));
    }
    for values in [
        &card.module_order,
        &card.admitted_order,
        &card.rejected_order,
        &card.unknown_order,
        &card.omitted_order,
        &card.lineage_order,
        &card.consumer_order,
        &card.contract_order,
        &card.effect_receipts,
    ] {
        if !canonical(values) {
            return Err(invalid("document graph vectors are not canonical"));
        }
    }
    let ids = card.module_order.iter().collect::<BTreeSet<_>>();
    let states = card
        .admitted_order
        .iter()
        .chain(&card.rejected_order)
        .chain(&card.unknown_order)
        .chain(&card.omitted_order)
        .collect::<Vec<_>>();
    if states.len() != ids.len() || states.into_iter().collect::<BTreeSet<_>>() != ids {
        return Err(invalid("document module states do not partition modules"));
    }
    if card.admitted_module_count != card.admitted_order.len() as u64 {
        return Err(invalid(
            "admitted module count does not match admitted order",
        ));
    }
    Ok(())
}

pub fn qualify(
    request: &DocumentGraphIntegrityRequest4,
    feature_id: &str,
    contract_version: &str,
    scale: &str,
    mode: &str,
) -> Result<DocumentGraphIntegrityCard7, DocumentGraphIntegrityError> {
    if request.schema_version != SCHEMA_VERSION
        || request.request_id.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.modules.is_empty()
        || request.module_budget == 0
        || !digest(&request.replay_identity)
        || request.boundary != BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
        || !canonical(&request.required_module_order)
        || !canonical(&request.adversarial_events)
    {
        return Err(invalid(
            "document identity, ordering, replay, locality, boundary, or budget is invalid",
        ));
    }
    let mut modules = request.modules.clone();
    modules.sort_by(|left, right| left.module_id.cmp(&right.module_id));
    let mut seen = BTreeSet::new();
    let mut parent = BTreeMap::new();
    let mut admitted = BTreeSet::new();
    let mut rejected = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut omitted = BTreeSet::new();
    let mut lineage = BTreeSet::new();
    let mut consumers = BTreeSet::new();
    let mut contracts = BTreeSet::new();
    let mut effects = BTreeSet::new();
    let mut sources = BTreeSet::new();
    let mut semantic_loss = Vec::new();
    for module in &modules {
        if module.module_id.trim().is_empty()
            || module.parent_module.trim().is_empty()
            || module.owner_crate.trim().is_empty()
            || module.consumer.trim().is_empty()
            || module.behavior.trim().is_empty()
            || module.input_schema.trim().is_empty()
            || module.output_schema.trim().is_empty()
            || !digest(&module.source_digest)
            || module.evidence_state.trim().is_empty()
            || !module.local
            || !module.aggregate_only
        {
            return Err(invalid("module identity, lineage, consumer, typed ports, evidence, or locality is incomplete"));
        }
        if !seen.insert(module.module_id.clone()) {
            return Err(invalid(format!(
                "duplicate document module {}",
                module.module_id
            )));
        }
        parent.insert(module.module_id.clone(), module.parent_module.clone());
        lineage.insert(format!("{}<-{}", module.module_id, module.parent_module));
        consumers.insert(module.consumer.clone());
        contracts.insert(format!("{}→{}", module.input_schema, module.output_schema));
        effects.insert(format!("document:{}", module.module_id));
        sources.insert(module.source_digest.clone());
        match module.evidence_state.as_str() {
            "supported" | "proven" if module.required && module.deterministic => {
                admitted.insert(module.module_id.clone());
            }
            "contradicted" | "rejected" => {
                rejected.insert(module.module_id.clone());
                semantic_loss.push(module.module_id.clone());
            }
            "unknown" | "speculative" | "unmeasured" => {
                unknown.insert(module.module_id.clone());
                semantic_loss.push(module.module_id.clone());
            }
            _ => {
                omitted.insert(module.module_id.clone());
                semantic_loss.push(module.module_id.clone());
            }
        }
    }
    if parent
        .values()
        .any(|ancestor| ancestor != "root" && !seen.contains(ancestor))
        || has_cycle(&parent)
    {
        return Err(invalid("document graph has an orphan parent or cycle"));
    }
    if request
        .required_module_order
        .iter()
        .collect::<BTreeSet<_>>()
        != seen.iter().collect::<BTreeSet<_>>()
    {
        return Err(invalid(
            "required module order is not the canonical module set",
        ));
    }
    let global_block = !request.policy_allowed
        || !request.protected_closure
        || !request.signed_manifest
        || !request.raw_data_local
        || !request.aggregate_only
        || !request.adversarial_events.is_empty()
        || modules.len() > request.module_budget;
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
        "qualified"
    };
    let module_order = seen.iter().cloned().collect::<Vec<_>>();
    let body = json!({"schema_version":SCHEMA_VERSION,"contract_version":contract_version,"feature_id":feature_id,"request_id":request.request_id,"purpose":request.purpose,"disposition":disposition,"module_order":module_order});
    let closure_digest = ContentHash::of_value(&body)
        .map_err(|error| DocumentGraphIntegrityError::Digest(error.to_string()))?
        .to_string();
    let admitted_order = admitted.into_iter().collect::<Vec<_>>();
    let rejected_order = rejected.into_iter().collect::<Vec<_>>();
    let unknown_order = unknown.into_iter().collect::<Vec<_>>();
    let omitted_order = omitted.into_iter().collect::<Vec<_>>();
    let card = DocumentGraphIntegrityCard7 {
        schema_version: SCHEMA_VERSION.into(),
        contract_version: contract_version.into(),
        feature_id: feature_id.into(),
        request_id: request.request_id.clone(),
        purpose: request.purpose.clone(),
        disposition: disposition.into(),
        module_order,
        admitted_order: admitted_order.clone(),
        rejected_order,
        unknown_order,
        omitted_order,
        lineage_order: lineage.into_iter().collect(),
        consumer_order: consumers.into_iter().collect(),
        contract_order: contracts.into_iter().collect(),
        replay_identity: request.replay_identity.clone(),
        closure_digest: closure_digest.clone(),
        admitted_module_count: admitted_order.len() as u64,
        total_module_count: modules.len() as u64,
        raw_data_local: true,
        aggregate_only: true,
        boundary: BOUNDARY.into(),
        effect_receipts: if disposition == "qualified" {
            vec![format!("prepare:document-lineage:{}", request.request_id)]
        } else {
            vec!["block:unsafe-context-release".into()]
        },
        artifact: DocumentGraphIntegrityArtifact4 {
            artifact_id: format!("docgraph-lineage:{}", request.request_id),
            content_type: CONTENT_TYPE.into(),
            content_hash: closure_digest,
            semantic_loss: if global_block {
                seen.iter().cloned().collect()
            } else {
                semantic_loss
            },
            source_digests: sources.into_iter().collect(),
            boundary: BOUNDARY.into(),
        },
    };
    validate_card(&card)?;
    let _ = (scale, mode);
    Ok(card)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> DocumentGraphIntegrityRequest4 {
        DocumentGraphIntegrityRequest4 {
            schema_version: SCHEMA_VERSION.into(),
            request_id: "doc-1".into(),
            purpose: "qualify context modules".into(),
            modules: vec![
                DocumentModule4 {
                    module_id: "doc-a".into(),
                    parent_module: "root".into(),
                    owner_crate: "docgraph".into(),
                    consumer: "context compiler".into(),
                    behavior: "compile source context".into(),
                    input_schema: "DocumentSource@1".into(),
                    output_schema: "ContextCard@1".into(),
                    source_digest: "a".repeat(64),
                    evidence_state: "supported".into(),
                    deterministic: true,
                    local: true,
                    aggregate_only: true,
                    required: true,
                },
                DocumentModule4 {
                    module_id: "doc-b".into(),
                    parent_module: "doc-a".into(),
                    owner_crate: "docgraph".into(),
                    consumer: "release auditor".into(),
                    behavior: "emit omission receipt".into(),
                    input_schema: "ContextCard@1".into(),
                    output_schema: "ReadingReceipt@1".into(),
                    source_digest: "b".repeat(64),
                    evidence_state: "proven".into(),
                    deterministic: true,
                    local: true,
                    aggregate_only: true,
                    required: true,
                },
            ],
            required_module_order: vec!["doc-a".into(), "doc-b".into()],
            replay_identity: "c".repeat(64),
            policy_allowed: true,
            protected_closure: true,
            signed_manifest: true,
            raw_data_local: true,
            aggregate_only: true,
            adversarial_events: vec![],
            module_budget: 4,
            boundary: BOUNDARY.into(),
        }
    }
    #[test]
    fn qualifies_acyclic_document_lineage() {
        let card = qualify(
            &request(),
            "AFA-docgraph-P32-F01",
            "v1",
            "local",
            "inference",
        )
        .unwrap();
        assert_eq!(card.disposition, "qualified");
        assert_eq!(card.admitted_module_count, 2);
    }
    #[test]
    fn rejects_cycle_before_release() {
        let mut q = request();
        q.modules[0].parent_module = "doc-b".into();
        assert!(
            matches!(qualify(&q,"AFA-docgraph-P32-F02","v1","local","inference"),Err(DocumentGraphIntegrityError::Invalid(message)) if message.contains("cycle"))
        );
    }
}
