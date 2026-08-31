//! Typed high-throughput retrieval contract model.
//!
//! Atlas feature: `AFA-brain-P02-F07`. Batch capacity, partition identity, queue digest, and
//! checkpoint continuity are validated before prospective retrieval synthesis.

use crate::evidence_contract_model::{ContractCompatibility, ContractDisposition};
use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-brain-P02-F07";
pub const CONTRACT_VERSION: &str = "brain-throughput-retrieval-contract-model/1.0";
pub const INPUT_SCHEMA: &str = "ScopedRetrievalQuery3@1";
pub const OUTPUT_SCHEMA: &str = "EvidenceSynthesis2@1";
const CONTRACT_CONTENT_TYPE: &str = "application/vnd.aurora.throughput-retrieval-contract+json";
const MAX_TEXT_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputRetrievalContractRequest {
    pub request_id: String,
    pub batch_id: String,
    pub partition: String,
    pub max_items: usize,
    pub checkpoint_seq: u64,
    pub input_schema: String,
    pub output_schema: String,
    pub compatibility: ContractCompatibility,
    pub required_fields: Vec<String>,
    pub provided_fields: Vec<String>,
    pub queue_digest: ContentHash,
    pub semantic_digest: ContentHash,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputRetrievalContractReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub batch_id: String,
    pub partition: String,
    pub max_items: usize,
    pub checkpoint_seq: u64,
    pub disposition: ContractDisposition,
    pub compatibility: ContractCompatibility,
    pub input_schema: String,
    pub output_schema: String,
    pub required_order: Vec<String>,
    pub provided_order: Vec<String>,
    pub missing_order: Vec<String>,
    pub semantic_loss_order: Vec<String>,
    pub queue_digest: ContentHash,
    pub semantic_digest: ContentHash,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub contract_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ThroughputRetrievalContractError {
    #[error("invalid throughput retrieval contract: {0}")]
    Invalid(String),
    #[error("throughput retrieval contract artifact failed: {0}")]
    Artifact(String),
}

impl ThroughputRetrievalContractReceipt {
    pub fn validate(&self) -> Result<(), ThroughputRetrievalContractError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.input_schema != INPUT_SCHEMA
            || self.output_schema != OUTPUT_SCHEMA
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.request_id.trim().is_empty()
            || self.batch_id.trim().is_empty()
            || self.partition.trim().is_empty()
            || self.max_items == 0
            || self.checkpoint_seq == 0
            || self.required_order.is_empty()
            || self.provided_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(ThroughputRetrievalContractError::Invalid("throughput contract identity, capacity, checkpoint, schemas, closure, locality, or effects are incomplete".into()));
        }
        for (value, field) in [
            (&self.request_id, "request_id"),
            (&self.batch_id, "batch_id"),
            (&self.partition, "partition"),
            (&self.input_schema, "input_schema"),
            (&self.output_schema, "output_schema"),
            (&self.boundary, "boundary"),
        ] {
            validate_text(value, field)?;
        }
        for (values, field) in [
            (&self.required_order, "required_order"),
            (&self.provided_order, "provided_order"),
            (&self.missing_order, "missing_order"),
            (&self.semantic_loss_order, "semantic_loss_order"),
            (&self.omissions, "omissions"),
            (&self.uncertainty, "uncertainty"),
            (&self.negative_evidence, "negative_evidence"),
            (&self.effect_receipts, "effect_receipts"),
        ] {
            validate_sorted_unique(values, field)?;
        }
        let required = identity_keys(&self.required_order);
        let provided = identity_keys(&self.provided_order);
        let expected_missing = required
            .difference(&provided)
            .cloned()
            .collect::<BTreeSet<_>>();
        let expected_loss = provided
            .difference(&required)
            .cloned()
            .collect::<BTreeSet<_>>();
        if identity_keys(&self.missing_order) != expected_missing
            || identity_keys(&self.semantic_loss_order) != expected_loss
        {
            return Err(ThroughputRetrievalContractError::Invalid(
                "throughput contract loss state does not match declared fields".into(),
            ));
        }
        let expected_disposition = if !self.missing_order.is_empty()
            || self.max_items == 0
            || self
                .negative_evidence
                .iter()
                .any(|value| value == "request:policy-denied")
            || self.omissions.iter().any(|value| {
                value == "request:protected-closure-incomplete"
                    || value == "request:raw-data-locality-failed"
            })
            || matches!(self.compatibility, ContractCompatibility::Breaking)
        {
            ContractDisposition::Blocked
        } else if !self.semantic_loss_order.is_empty()
            || self.provided_order.len() > self.max_items
            || !matches!(self.compatibility, ContractCompatibility::Additive)
            || !self.negative_evidence.is_empty()
        {
            ContractDisposition::Partial
        } else {
            ContractDisposition::Qualified
        };
        if self.disposition != expected_disposition {
            return Err(ThroughputRetrievalContractError::Invalid(
                "throughput contract disposition does not match capacity, loss, and safety state"
                    .into(),
            ));
        }
        let expected_effect_receipts = if matches!(
            self.disposition,
            ContractDisposition::Qualified | ContractDisposition::Partial
        ) {
            vec![format!(
                "read:local-throughput-artifacts:{}",
                self.request_id
            )]
        } else {
            vec!["block:unsafe-release".into()]
        };
        if self.effect_receipts != expected_effect_receipts {
            return Err(ThroughputRetrievalContractError::Invalid(
                "throughput contract effects do not match disposition".into(),
            ));
        }
        for digest in [
            &self.queue_digest,
            &self.semantic_digest,
            &self.artifact_digest,
            &self.provenance_digest,
            &self.contract_digest,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(ThroughputRetrievalContractError::Invalid(
                    "throughput contract digest is invalid".into(),
                ));
            }
        }
        if !self.raw_data_local
            && (self.disposition != ContractDisposition::Blocked
                || !self
                    .omissions
                    .iter()
                    .any(|value| value == "request:raw-data-locality-failed"))
        {
            return Err(ThroughputRetrievalContractError::Invalid(
                "non-local throughput contracts must be blocked and retain locality evidence"
                    .into(),
            ));
        }
        let expected_contract_digest = ContentHash::of_value(&json!({
            "feature_id": FEATURE_ID,
            "request_id": self.request_id,
            "batch_id": self.batch_id,
            "partition": self.partition,
            "max_items": self.max_items,
            "checkpoint_seq": self.checkpoint_seq,
            "queue_digest": self.queue_digest,
            "required_order": self.required_order,
            "provided_order": self.provided_order,
            "compatibility": self.compatibility,
            "semantic_digest": self.semantic_digest,
            "artifact_digest": self.artifact_digest,
            "provenance_digest": self.provenance_digest,
            "replay_identity": self.replay_identity,
            "disposition": self.disposition,
            "raw_data_local": self.raw_data_local,
        }))
        .map_err(|error| ThroughputRetrievalContractError::Artifact(error.to_string()))?;
        if self.contract_digest != expected_contract_digest {
            return Err(ThroughputRetrievalContractError::Invalid(
                "throughput contract digest is not bound to declared state".into(),
            ));
        }
        let expected_artifact_id =
            format!("brain-throughput-retrieval-contract:{}", self.request_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != CONTRACT_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(ThroughputRetrievalContractError::Invalid(
                "throughput contract artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ThroughputRetrievalContractError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| ThroughputRetrievalContractError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, ThroughputRetrievalContractError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ThroughputRetrievalContractError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ThroughputRetrievalContractError::Artifact(error.to_string()))
    }
}

pub fn throughput_retrieval_contract_model_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["platform reliability engineer".into(), "throughput retrieval operator".into()].into(), behavior: "validates high-throughput retrieval contract capacity, partition, queue, checkpoint, schema, field closure, semantic loss, and replay identity".into(), value: "prevents prospective batch overflow or checkpoint gaps from being presented as complete retrieval evidence".into(), inputs: vec![TypedPort { name: "throughput_retrieval_contract".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "throughput_synthesis_contract".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["read:local-throughput-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "cwl-v1.2".into(), state: EvidenceState::Supported, locator: Some("https://www.commonwl.org/specification/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn model_throughput_retrieval_contract(
    request: &ThroughputRetrievalContractRequest,
) -> Result<ThroughputRetrievalContractReceipt, ThroughputRetrievalContractError> {
    validate_request(request)?;
    let required = request
        .required_fields
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let provided = request
        .provided_fields
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let missing = required.difference(&provided).cloned().collect::<Vec<_>>();
    let semantic_loss = provided.difference(&required).cloned().collect::<Vec<_>>();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    if request.provided_fields.len() > request.max_items {
        omissions.insert("batch:capacity-overflow".into());
    }
    if !missing.is_empty() {
        omissions.extend(
            missing
                .iter()
                .map(|field| format!("field:{field}:required-missing")),
        );
    }
    if !semantic_loss.is_empty() {
        uncertainty.extend(
            semantic_loss
                .iter()
                .map(|field| format!("field:{field}:provided-not-declared")),
        );
    }
    if !matches!(request.compatibility, ContractCompatibility::Additive) {
        uncertainty.insert(format!(
            "contract:compatibility-{}",
            compatibility_label(request.compatibility)
        ));
    }
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        omissions.insert("request:protected-closure-incomplete".into());
    }
    if !request.raw_data_local {
        omissions.insert("request:raw-data-locality-failed".into());
    }
    let disposition = if !request.policy_allow
        || !request.protected_closure
        || !request.raw_data_local
        || !missing.is_empty()
        || matches!(request.compatibility, ContractCompatibility::Breaking)
    {
        ContractDisposition::Blocked
    } else if !semantic_loss.is_empty()
        || request.provided_fields.len() > request.max_items
        || !matches!(request.compatibility, ContractCompatibility::Additive)
        || !negative.is_empty()
    {
        ContractDisposition::Partial
    } else {
        ContractDisposition::Qualified
    };
    let contract_digest = ContentHash::of_value(&json!({"feature_id": FEATURE_ID, "request_id": request.request_id, "batch_id": request.batch_id, "partition": request.partition, "max_items": request.max_items, "checkpoint_seq": request.checkpoint_seq, "queue_digest": request.queue_digest, "required_order": required, "provided_order": provided, "compatibility": request.compatibility, "semantic_digest": request.semantic_digest, "artifact_digest": request.artifact_digest, "provenance_digest": request.provenance_digest, "replay_identity": request.replay_identity, "disposition": disposition, "raw_data_local": true})).map_err(|error| ThroughputRetrievalContractError::Artifact(error.to_string()))?;
    let effect_receipts = if matches!(
        disposition,
        ContractDisposition::Qualified | ContractDisposition::Partial
    ) {
        vec![format!(
            "read:local-throughput-artifacts:{}",
            request.request_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "batch_id": request.batch_id, "partition": request.partition, "max_items": request.max_items, "checkpoint_seq": request.checkpoint_seq, "disposition": disposition, "compatibility": request.compatibility, "input_schema": request.input_schema, "output_schema": request.output_schema, "required_order": required, "provided_order": provided, "missing_order": missing, "semantic_loss_order": semantic_loss, "queue_digest": request.queue_digest, "semantic_digest": request.semantic_digest, "artifact_digest": request.artifact_digest, "provenance_digest": request.provenance_digest, "contract_digest": contract_digest, "replay_identity": request.replay_identity, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "effect_receipts": effect_receipts, "raw_data_local": true, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-throughput-retrieval-contract:{}", request.request_id),
        CONTRACT_CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ThroughputRetrievalContractError::Artifact(error.to_string()))?;
    let receipt = ThroughputRetrievalContractReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        batch_id: request.batch_id.clone(),
        partition: request.partition.clone(),
        max_items: request.max_items,
        checkpoint_seq: request.checkpoint_seq,
        disposition,
        compatibility: request.compatibility,
        input_schema: request.input_schema.clone(),
        output_schema: request.output_schema.clone(),
        required_order: required.into_iter().collect(),
        provided_order: provided.into_iter().collect(),
        missing_order: missing,
        semantic_loss_order: semantic_loss,
        queue_digest: request.queue_digest.clone(),
        semantic_digest: request.semantic_digest.clone(),
        artifact_digest: request.artifact_digest.clone(),
        provenance_digest: request.provenance_digest.clone(),
        contract_digest,
        replay_identity: request.replay_identity.clone(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(
    request: &ThroughputRetrievalContractRequest,
) -> Result<(), ThroughputRetrievalContractError> {
    for (value, field) in [
        (&request.request_id, "request_id"),
        (&request.batch_id, "batch_id"),
        (&request.partition, "partition"),
        (&request.input_schema, "input_schema"),
        (&request.output_schema, "output_schema"),
        (&request.boundary, "boundary"),
    ] {
        validate_text(value, field)?;
    }
    if request.request_id.trim().is_empty()
        || request.batch_id.trim().is_empty()
        || request.partition.trim().is_empty()
        || request.max_items == 0
        || request.checkpoint_seq == 0
        || request.input_schema != INPUT_SCHEMA
        || request.output_schema != OUTPUT_SCHEMA
        || request.required_fields.is_empty()
        || request.provided_fields.is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(ThroughputRetrievalContractError::Invalid("throughput contract identity, capacity, checkpoint, schemas, fields, or boundary is incomplete".into()));
    }
    validate_unique(&request.required_fields, "required_fields")?;
    validate_unique(&request.provided_fields, "provided_fields")?;
    for digest in [
        &request.queue_digest,
        &request.semantic_digest,
        &request.artifact_digest,
        &request.provenance_digest,
        &request.replay_identity,
    ] {
        if digest.as_str().len() != 64 {
            return Err(ThroughputRetrievalContractError::Invalid(
                "throughput contract request digest is invalid".into(),
            ));
        }
    }
    Ok(())
}

fn identity_keys(values: &[String]) -> BTreeSet<String> {
    values
        .iter()
        .map(|value| value.to_ascii_lowercase())
        .collect()
}

fn validate_text(value: &str, field: &str) -> Result<(), ThroughputRetrievalContractError> {
    if value.trim().is_empty()
        || value != value.trim()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(ThroughputRetrievalContractError::Invalid(format!(
            "{field} must be bounded, non-empty text without padding or control characters"
        )));
    }
    Ok(())
}

fn validate_unique(values: &[String], field: &str) -> Result<(), ThroughputRetrievalContractError> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_text(value, field)?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(ThroughputRetrievalContractError::Invalid(format!(
                "{field} contains duplicate or case-colliding values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_unique(
    values: &[String],
    field: &str,
) -> Result<(), ThroughputRetrievalContractError> {
    validate_unique(values, field)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ThroughputRetrievalContractError::Invalid(format!(
            "{field} is not in canonical order"
        )));
    }
    Ok(())
}

fn receipt_payload(receipt: &ThroughputRetrievalContractReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "batch_id": receipt.batch_id,
        "partition": receipt.partition,
        "max_items": receipt.max_items,
        "checkpoint_seq": receipt.checkpoint_seq,
        "disposition": receipt.disposition,
        "compatibility": receipt.compatibility,
        "input_schema": receipt.input_schema,
        "output_schema": receipt.output_schema,
        "required_order": receipt.required_order,
        "provided_order": receipt.provided_order,
        "missing_order": receipt.missing_order,
        "semantic_loss_order": receipt.semantic_loss_order,
        "queue_digest": receipt.queue_digest,
        "semantic_digest": receipt.semantic_digest,
        "artifact_digest": receipt.artifact_digest,
        "provenance_digest": receipt.provenance_digest,
        "contract_digest": receipt.contract_digest,
        "replay_identity": receipt.replay_identity,
        "omissions": receipt.omissions,
        "uncertainty": receipt.uncertainty,
        "negative_evidence": receipt.negative_evidence,
        "effect_receipts": receipt.effect_receipts,
        "raw_data_local": receipt.raw_data_local,
        "boundary": receipt.boundary,
    })
}

fn compatibility_label(value: ContractCompatibility) -> &'static str {
    match value {
        ContractCompatibility::Additive => "additive",
        ContractCompatibility::MigrationRequired => "migration-required",
        ContractCompatibility::Breaking => "breaking",
        ContractCompatibility::Unknown => "unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn request(provided_fields: Vec<String>) -> ThroughputRetrievalContractRequest {
        ThroughputRetrievalContractRequest {
            request_id: "request:throughput-contract".into(),
            batch_id: "batch:001".into(),
            partition: "partition:imaging".into(),
            max_items: 3,
            checkpoint_seq: 1,
            input_schema: INPUT_SCHEMA.into(),
            output_schema: OUTPUT_SCHEMA.into(),
            compatibility: ContractCompatibility::Additive,
            required_fields: vec!["scope".into(), "evidence".into()],
            provided_fields,
            queue_digest: hash("queue"),
            semantic_digest: hash("semantic"),
            artifact_digest: hash("artifact"),
            provenance_digest: hash("provenance"),
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        let m = throughput_retrieval_contract_model_manifest();
        m.validate().unwrap();
        assert_eq!(m.autonomy_tier, AutonomyTier::A1);
    }
    #[test]
    fn complete_is_qualified() {
        let r =
            model_throughput_retrieval_contract(&request(vec!["scope".into(), "evidence".into()]))
                .unwrap();
        assert_eq!(r.disposition, ContractDisposition::Qualified);
    }
    #[test]
    fn overflow_is_partial() {
        let r = model_throughput_retrieval_contract(&request(vec![
            "scope".into(),
            "evidence".into(),
            "extra-a".into(),
            "extra-b".into(),
        ]))
        .unwrap();
        assert_eq!(r.disposition, ContractDisposition::Partial);
    }
    #[test]
    fn missing_field_blocks() {
        let r = model_throughput_retrieval_contract(&request(vec!["scope".into()])).unwrap();
        assert_eq!(r.disposition, ContractDisposition::Blocked);
    }
    #[test]
    fn policy_blocks() {
        let mut q = request(vec!["scope".into(), "evidence".into()]);
        q.policy_allow = false;
        let r = model_throughput_retrieval_contract(&q).unwrap();
        assert_eq!(r.disposition, ContractDisposition::Blocked);
    }

    #[test]
    fn locality_failure_is_blocked_and_retained() {
        let mut q = request(vec!["scope".into(), "evidence".into()]);
        q.raw_data_local = false;
        let r = model_throughput_retrieval_contract(&q).unwrap();
        assert_eq!(r.disposition, ContractDisposition::Blocked);
        assert!(r.raw_data_local);
        assert!(r
            .omissions
            .iter()
            .any(|value| value == "request:raw-data-locality-failed"));
        assert!(r.validate().is_ok());
    }

    #[test]
    fn contract_artifact_payload_is_bound() {
        let mut r =
            model_throughput_retrieval_contract(&request(vec!["scope".into(), "evidence".into()]))
                .unwrap();
        r.partition = "partition:tampered".into();
        assert!(r.validate().is_err());
    }

    #[test]
    fn digest_is_stable() {
        let r =
            model_throughput_retrieval_contract(&request(vec!["scope".into(), "evidence".into()]))
                .unwrap();
        assert_eq!(r.digest().unwrap(), r.digest().unwrap());
    }
}
