//! Typed retrieval-contract compiler for Worldgen P02 F05–F08.
//!
//! This boundary does not retrieve, publish, or execute anything. It converts a declared
//! `ScopedRetrievalQuery` shape into an auditable compatibility receipt and makes every loss
//! explicit before an inference engine can consume the contract.

use super::retrieval_support::{RetrievalCandidate, BOUNDARY, SCHEMA_VERSION};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const OUTPUT_SCHEMA: &str = "EvidenceSynthesis2@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.worldgen.retrieval-contract-receipt+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalContractRequest {
    pub request_id: String,
    pub consumer: String,
    pub scope: String,
    pub semantic_profile: String,
    pub input_schema: String,
    pub output_schema: String,
    pub required_candidate_order: Vec<String>,
    pub candidates: Vec<RetrievalCandidate>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalContractReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub consumer: String,
    pub scope: String,
    pub semantic_profile: String,
    pub input_schema: String,
    pub output_schema: String,
    pub compatibility: String,
    pub disposition: String,
    pub candidate_order: Vec<String>,
    pub compatible_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub omitted_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub migration_order: Vec<String>,
    pub semantic_loss_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub contract_digest: ContentHash,
    pub artifact: serde_json::Value,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RetrievalContractError {
    #[error("invalid retrieval contract request: {0}")]
    Invalid(String),
    #[error("invalid retrieval contract receipt: {0}")]
    Receipt(String),
    #[error("retrieval contract artifact failed: {0}")]
    Artifact(String),
}

fn digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64 && value.as_str().bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn ordered(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn sorted(value: &serde_json::Value) -> Vec<String> {
    let mut output = value
        .as_array()
        .expect("contract sets serialize as arrays")
        .iter()
        .map(|item| item.as_str().expect("contract set values are strings").to_owned())
        .collect::<Vec<_>>();
    output.sort();
    output.dedup();
    output
}

impl RetrievalContractReceipt {
    pub fn validate(&self) -> Result<(), RetrievalContractError> {
        if self.schema_version != SCHEMA_VERSION
            || self.boundary != BOUNDARY
            || self.artifact.get("boundary").and_then(|value| value.as_str()) != Some(BOUNDARY)
            || self.artifact.get("content_type").and_then(|value| value.as_str()) != Some(CONTENT_TYPE)
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.consumer.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.input_schema.trim().is_empty()
            || self.output_schema != OUTPUT_SCHEMA
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(RetrievalContractError::Receipt(
                "retrieval contract identity, schemas, locality, or effects are incomplete".into(),
            ));
        }
        for values in [
            &self.candidate_order,
            &self.compatible_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.omitted_order,
            &self.negative_evidence_order,
            &self.migration_order,
            &self.semantic_loss_order,
            &self.effect_receipts,
        ] {
            if !ordered(values) {
                return Err(RetrievalContractError::Receipt(
                    "retrieval contract ordering is not canonical".into(),
                ));
            }
        }
        let ids = self.candidate_order.iter().cloned().collect::<BTreeSet<_>>();
        let parts = self
            .compatible_order
            .iter()
            .chain(&self.unresolved_order)
            .chain(&self.blocked_order)
            .chain(&self.omitted_order)
            .cloned()
            .collect::<Vec<_>>();
        if ids.len() != self.candidate_order.len()
            || parts.len() != ids.len()
            || parts.iter().cloned().collect::<BTreeSet<_>>() != ids
        {
            return Err(RetrievalContractError::Receipt(
                "retrieval contract candidate states do not partition".into(),
            ));
        }
        if !digest(&self.replay_identity)
            || !digest(&self.contract_digest)
            || self.artifact.get("content_hash").and_then(|value| value.as_str()) != Some(self.contract_digest.as_str())
        {
            return Err(RetrievalContractError::Receipt(
                "retrieval contract digest is invalid or inconsistent".into(),
            ));
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, RetrievalContractError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| RetrievalContractError::Receipt(error.to_string()))?;
        ContentHash::of_value(&value).map_err(|error| RetrievalContractError::Receipt(error.to_string()))
    }
}

pub fn manifest(feature_id: &str, version: &str, input_schema: &str, scale: &str, autonomy: &str) -> serde_json::Value {
    json!({
        "schema_version": SCHEMA_VERSION,
        "capability_id": feature_id,
        "version": version,
        "owner_crate": "worldgen",
        "consumers": ["benchmark curator", "research program lead", "preclinical neuroscientist", "bioinformatician"],
        "behavior": format!("compile a typed retrieval contract for {scale} with deterministic compatibility and semantic-loss receipts"),
        "value": "prevents schema drift and silent defaults before evidence synthesis",
        "input_schema": input_schema,
        "output_schema": OUTPUT_SCHEMA,
        "effects": ["none:contract-validation"],
        "permissions": ["read:local-research-artifacts"],
        "determinism": "byte_stable",
        "autonomy_tier": autonomy,
        "boundary": BOUNDARY,
    })
}

pub fn compile(
    request: &RetrievalContractRequest,
    feature_id: &str,
    contract_version: &str,
    expected_input_schema: &str,
) -> Result<RetrievalContractReceipt, RetrievalContractError> {
    if request.request_id.trim().is_empty()
        || request.consumer.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.input_schema.trim().is_empty()
        || request.output_schema.trim().is_empty()
        || request.required_candidate_order.is_empty()
        || request.candidates.is_empty()
        || request.boundary != BOUNDARY
        || request.output_schema != OUTPUT_SCHEMA
        || !request.raw_data_local
        || !request.aggregate_only
        || !digest(&request.replay_identity)
    {
        return Err(RetrievalContractError::Invalid(
            "retrieval contract identity, schemas, candidates, replay, locality, or boundary is invalid".into(),
        ));
    }
    let mut candidate_order = request
        .candidates
        .iter()
        .map(|candidate| candidate.candidate_id.clone())
        .collect::<Vec<_>>();
    candidate_order.sort();
    candidate_order.dedup();
    if candidate_order.len() != request.candidates.len()
        || request.required_candidate_order.iter().any(|id| !candidate_order.contains(id))
    {
        return Err(RetrievalContractError::Invalid(
            "retrieval contract candidate identifiers do not match the declared set".into(),
        ));
    }

    let schema_break = request.input_schema != expected_input_schema;
    let mut compatible = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut omitted = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut migration = BTreeSet::new();
    let mut semantic_loss = BTreeSet::new();
    for candidate in &request.candidates {
        if candidate.negative_result {
            negative.insert(format!("candidate:{}:negative-result-retained", candidate.candidate_id));
        }
        if !request.required_candidate_order.contains(&candidate.candidate_id) {
            omitted.insert(format!("candidate:{}:not-required", candidate.candidate_id));
            semantic_loss.insert(format!("candidate:{}:outside-required-closure", candidate.candidate_id));
            continue;
        }
        if schema_break {
            migration.insert(format!("request:input-schema:{}->{}", request.input_schema, expected_input_schema));
            semantic_loss.insert(format!("candidate:{}:schema-version-unresolved", candidate.candidate_id));
            unresolved.insert(candidate.candidate_id.clone());
        } else if !request.policy_allow || !request.protected_closure || !candidate.permitted {
            blocked.insert(candidate.candidate_id.clone());
        } else if candidate.evidence_state == "supported" && candidate.comparable && candidate.replay_identity == request.replay_identity {
            compatible.insert(candidate.candidate_id.clone());
        } else if matches!(candidate.evidence_state.as_str(), "unknown" | "unmeasured" | "speculative") {
            unresolved.insert(candidate.candidate_id.clone());
        } else {
            blocked.insert(candidate.candidate_id.clone());
        }
        if !candidate.comparable {
            semantic_loss.insert(format!("candidate:{}:incomparable", candidate.candidate_id));
        }
        if candidate.replay_identity != request.replay_identity {
            semantic_loss.insert(format!("candidate:{}:replay-mismatch", candidate.candidate_id));
        }
    }
    for required in &request.required_candidate_order {
        if !candidate_order.contains(required) {
            omitted.insert(format!("candidate:{}:missing", required));
            semantic_loss.insert(format!("candidate:{}:missing-required", required));
        }
    }
    if !request.policy_allow {
        semantic_loss.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        semantic_loss.insert("request:protected-closure-incomplete".into());
    }
    let compatibility = if schema_break { "breaking" } else if !migration.is_empty() { "additive_migration" } else { "compatible" };
    let disposition = if schema_break || !request.policy_allow || !request.protected_closure {
        "blocked"
    } else if compatible.is_empty() {
        "unknown"
    } else if unresolved.is_empty() && blocked.is_empty() && omitted.is_empty() && semantic_loss.is_empty() {
        "compatible"
    } else {
        "partial"
    };
    let effects = vec!["none:contract-validation".to_string()];
    let payload = json!({
        "schema_version": SCHEMA_VERSION,
        "contract_version": contract_version,
        "feature_id": feature_id,
        "request_id": request.request_id,
        "consumer": request.consumer,
        "scope": request.scope,
        "semantic_profile": request.semantic_profile,
        "input_schema": request.input_schema,
        "output_schema": request.output_schema,
        "compatibility": compatibility,
        "disposition": disposition,
        "candidate_order": candidate_order,
        "compatible_order": compatible,
        "unresolved_order": unresolved,
        "blocked_order": blocked,
        "omitted_order": omitted,
        "negative_evidence_order": negative,
        "migration_order": migration,
        "semantic_loss_order": semantic_loss,
        "replay_identity": request.replay_identity,
        "effect_receipts": effects,
        "raw_data_local": true,
        "aggregate_only": true,
        "boundary": BOUNDARY,
    });
    let contract_digest = ContentHash::of_value(&payload)
        .map_err(|error| RetrievalContractError::Artifact(error.to_string()))?;
    let receipt = RetrievalContractReceipt {
        schema_version: SCHEMA_VERSION.into(),
        contract_version: contract_version.into(),
        feature_id: feature_id.into(),
        request_id: request.request_id.clone(),
        consumer: request.consumer.clone(),
        scope: request.scope.clone(),
        semantic_profile: request.semantic_profile.clone(),
        input_schema: request.input_schema.clone(),
        output_schema: request.output_schema.clone(),
        compatibility: compatibility.into(),
        disposition: disposition.into(),
        candidate_order: sorted(&payload["candidate_order"]),
        compatible_order: sorted(&payload["compatible_order"]),
        unresolved_order: sorted(&payload["unresolved_order"]),
        blocked_order: sorted(&payload["blocked_order"]),
        omitted_order: sorted(&payload["omitted_order"]),
        negative_evidence_order: sorted(&payload["negative_evidence_order"]),
        migration_order: sorted(&payload["migration_order"]),
        semantic_loss_order: sorted(&payload["semantic_loss_order"]),
        replay_identity: request.replay_identity.clone(),
        contract_digest: contract_digest.clone(),
        artifact: json!({"artifact_id": format!("retrieval-contract:{}", request.request_id), "content_type": CONTENT_TYPE, "content_hash": contract_digest, "semantic_loss": sorted(&payload["semantic_loss_order"]), "boundary": BOUNDARY}),
        effect_receipts: effects,
        raw_data_local: true,
        aggregate_only: true,
        boundary: BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(seed: &str) -> ContentHash { ContentHash::of_bytes(seed.as_bytes()) }
    fn candidate(id: &str, state: &str) -> RetrievalCandidate {
        RetrievalCandidate { candidate_id: id.into(), source_id: format!("src:{id}"), title: format!("Study {id}"), study_id: format!("study:{id}"), modality: "imaging".into(), relevance_milli: 900, freshness_milli: 900, evidence_state: state.into(), content_digest: hash(&format!("c:{id}")), provenance_digest: hash(&format!("p:{id}")), replay_identity: hash("replay"), estimated_units: 1, permitted: true, comparable: true, negative_result: false }
    }
    fn request() -> RetrievalContractRequest {
        RetrievalContractRequest { request_id: "contract:req".into(), consumer: "benchmark-curator".into(), scope: "study:local".into(), semantic_profile: "prov-v1".into(), input_schema: "ScopedRetrievalQuery1@1".into(), output_schema: OUTPUT_SCHEMA.into(), required_candidate_order: vec!["a".into(), "b".into()], candidates: vec![candidate("b", "supported"), candidate("a", "supported")], replay_identity: hash("replay"), policy_allow: true, protected_closure: true, raw_data_local: true, aggregate_only: true, boundary: BOUNDARY.into() }
    }
    #[test] fn compatible_contract_is_canonical() { let receipt = compile(&request(), "AFA-worldgen-P02-F05", "worldgen-local-retrieval-synthesis-contract/1.0", "ScopedRetrievalQuery1@1").unwrap(); assert_eq!(receipt.disposition, "compatible"); assert_eq!(receipt.compatible_order, vec!["a", "b"]); assert_eq!(receipt.effect_receipts, vec!["none:contract-validation"]); }
    #[test] fn unknown_evidence_is_unresolved() { let mut req = request(); req.input_schema = "ScopedRetrievalQuery2@1".into(); req.candidates[0] = candidate("b", "unknown"); let receipt = compile(&req, "AFA-worldgen-P02-F06", "worldgen-multimodal-retrieval-synthesis-contract/1.0", "ScopedRetrievalQuery2@1").unwrap(); assert!(receipt.unresolved_order.contains(&"b".into())); assert_eq!(receipt.disposition, "partial"); }
    #[test] fn schema_migration_fails_closed() { let mut req = request(); req.input_schema = "ScopedRetrievalQuery0@1".into(); let receipt = compile(&req, "AFA-worldgen-P02-F07", "worldgen-throughput-retrieval-synthesis-contract/1.0", "ScopedRetrievalQuery3@1").unwrap(); assert_eq!(receipt.compatibility, "breaking"); assert_eq!(receipt.disposition, "blocked"); assert!(!receipt.semantic_loss_order.is_empty()); }
    #[test] fn policy_and_closure_are_explicit() { let mut req = request(); req.policy_allow = false; req.protected_closure = false; let receipt = compile(&req, "AFA-worldgen-P02-F08", "worldgen-federated-continual-retrieval-synthesis-contract/1.0", "ScopedRetrievalQuery4@1").unwrap(); assert_eq!(receipt.disposition, "blocked"); }
}
