//! Prospective high-throughput evidence contract model.
//!
//! Atlas feature: `AFA-brain-P01-F07`. The contract primitive makes queue capacity and
//! checkpoint continuity explicit before a high-throughput batch is admitted.

use crate::evidence_contract_model::ContractCompatibility;
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

pub const FEATURE_ID: &str = "AFA-brain-P01-F07";
pub const CONTRACT_VERSION: &str = "brain-throughput-evidence-contract/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceFeed3@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet2@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputContractModelRequest {
    pub request_id: String,
    pub batch_id: String,
    pub partition: String,
    pub input_schema: String,
    pub output_schema: String,
    pub compatibility: ContractCompatibility,
    pub required_fields: Vec<String>,
    pub provided_fields: Vec<String>,
    pub max_items: usize,
    pub observed_items: usize,
    pub checkpoint_seq: u64,
    pub queue_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThroughputContractDisposition {
    Qualified,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputContractModelReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub batch_id: String,
    pub partition: String,
    pub disposition: ThroughputContractDisposition,
    pub compatibility: ContractCompatibility,
    pub input_schema: String,
    pub output_schema: String,
    pub required_order: Vec<String>,
    pub provided_order: Vec<String>,
    pub missing_order: Vec<String>,
    pub semantic_loss_order: Vec<String>,
    pub max_items: usize,
    pub observed_items: usize,
    pub admitted_items: usize,
    pub checkpoint_seq: u64,
    pub queue_digest: ContentHash,
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
pub enum ThroughputContractModelError {
    #[error("invalid throughput evidence contract: {0}")]
    Invalid(String),
    #[error("throughput contract artifact failed: {0}")]
    Artifact(String),
}

impl ThroughputContractModelReceipt {
    pub fn validate(&self) -> Result<(), ThroughputContractModelError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.input_schema != INPUT_SCHEMA
            || self.output_schema != OUTPUT_SCHEMA
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.batch_id.trim().is_empty()
            || self.partition.trim().is_empty()
            || self.required_order.is_empty()
            || self.provided_order.is_empty()
            || self.max_items == 0
            || self.checkpoint_seq == 0
            || self.effect_receipts.is_empty()
        {
            return Err(ThroughputContractModelError::Invalid("throughput identity, schemas, fields, capacity, checkpoint, locality, or effects are incomplete".into()));
        }
        if self.admitted_items > self.max_items || self.admitted_items > self.observed_items {
            return Err(ThroughputContractModelError::Invalid(
                "admitted item count exceeds declared capacity or observations".into(),
            ));
        }
        if self
            .missing_order
            .iter()
            .any(|field| !self.required_order.contains(field))
            || self
                .semantic_loss_order
                .iter()
                .any(|field| !self.provided_order.contains(field))
        {
            return Err(ThroughputContractModelError::Invalid(
                "throughput loss state is outside declared fields".into(),
            ));
        }
        for values in [
            &self.required_order,
            &self.provided_order,
            &self.missing_order,
            &self.semantic_loss_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(ThroughputContractModelError::Invalid(
                    "throughput contract ordering is not canonical".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("read:local-research-artifacts:")
                && effect != "block:unsafe-release"
        }) {
            return Err(ThroughputContractModelError::Invalid(
                "effect is outside throughput contract gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ThroughputContractModelError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, ThroughputContractModelError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ThroughputContractModelError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ThroughputContractModelError::Artifact(error.to_string()))
    }
}

pub fn throughput_contract_model_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["laboratory automation engineer".into(), "research operations steward".into()].into(), behavior: "models high-throughput EvidenceFeed contracts with capacity, checkpoint, queue, schema, and semantic-loss closure".into(), value: "prevents batch overflow and checkpoint ambiguity from becoming silent evidence loss".into(), inputs: vec![TypedPort { name: "throughput_evidence_contract".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "throughput_qualified_contract".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["read:local-research-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "json-schema".into(), state: EvidenceState::Supported, locator: Some("https://json-schema.org/specification".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn model_throughput_contract(
    request: &ThroughputContractModelRequest,
) -> Result<ThroughputContractModelReceipt, ThroughputContractModelError> {
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
    let capacity_exceeded = request.observed_items > request.max_items;
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
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
    if capacity_exceeded {
        omissions.insert(format!("batch:{}:capacity-exceeded", request.batch_id));
    }
    if request.compatibility != ContractCompatibility::Additive {
        uncertainty.insert(format!(
            "contract:compatibility-{}",
            compatibility_label(request.compatibility)
        ));
    }
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.raw_data_local {
        negative.insert("request:raw-data-locality-failed".into());
    }
    let blocked_by_gate =
        !request.policy_allow || !request.protected_closure || !request.raw_data_local;
    let disposition = if blocked_by_gate {
        ThroughputContractDisposition::Blocked
    } else if request.observed_items == 0 {
        ThroughputContractDisposition::Unknown
    } else if missing.is_empty()
        && semantic_loss.is_empty()
        && !capacity_exceeded
        && request.compatibility == ContractCompatibility::Additive
    {
        ThroughputContractDisposition::Qualified
    } else {
        ThroughputContractDisposition::Partial
    };
    let admitted_items = request.observed_items.min(request.max_items);
    let contract_digest = ContentHash::of_value(&json!({"batch_id": request.batch_id, "partition": request.partition, "input_schema": request.input_schema, "output_schema": request.output_schema, "compatibility": request.compatibility, "required_order": required, "provided_order": provided, "max_items": request.max_items, "observed_items": request.observed_items, "checkpoint_seq": request.checkpoint_seq, "queue_digest": request.queue_digest})).map_err(|error| ThroughputContractModelError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "batch_id": request.batch_id, "partition": request.partition, "disposition": disposition, "compatibility": request.compatibility, "input_schema": INPUT_SCHEMA, "output_schema": OUTPUT_SCHEMA, "required_order": required, "provided_order": provided, "missing_order": missing, "semantic_loss_order": semantic_loss, "max_items": request.max_items, "observed_items": request.observed_items, "admitted_items": admitted_items, "checkpoint_seq": request.checkpoint_seq, "queue_digest": request.queue_digest, "contract_digest": contract_digest, "replay_identity": request.replay_identity, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-throughput-contract:{}", request.request_id),
        "application/vnd.aurora.throughput-evidence-contract+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ThroughputContractModelError::Artifact(error.to_string()))?;
    let has_qualified = disposition == ThroughputContractDisposition::Qualified;
    let receipt = ThroughputContractModelReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        batch_id: request.batch_id.clone(),
        partition: request.partition.clone(),
        disposition,
        compatibility: request.compatibility,
        input_schema: INPUT_SCHEMA.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        required_order: required.into_iter().collect(),
        provided_order: provided.into_iter().collect(),
        missing_order: missing,
        semantic_loss_order: semantic_loss,
        max_items: request.max_items,
        observed_items: request.observed_items,
        admitted_items,
        checkpoint_seq: request.checkpoint_seq,
        queue_digest: request.queue_digest.clone(),
        contract_digest,
        replay_identity: request.replay_identity.clone(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts: if has_qualified {
            vec![format!(
                "read:local-research-artifacts:{}",
                request.request_id
            )]
        } else {
            vec!["block:unsafe-release".into()]
        },
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(
    request: &ThroughputContractModelRequest,
) -> Result<(), ThroughputContractModelError> {
    if request.request_id.trim().is_empty()
        || request.batch_id.trim().is_empty()
        || request.partition.trim().is_empty()
        || request.input_schema != INPUT_SCHEMA
        || request.output_schema != OUTPUT_SCHEMA
        || request.required_fields.is_empty()
        || request.provided_fields.is_empty()
        || request.max_items == 0
        || request.checkpoint_seq == 0
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(ThroughputContractModelError::Invalid(
            "throughput identity, schemas, fields, capacity, checkpoint, or boundary is incomplete"
                .into(),
        ));
    }
    if request
        .required_fields
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
        || request
            .provided_fields
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(ThroughputContractModelError::Invalid(
            "throughput fields must be unique and canonical".into(),
        ));
    }
    Ok(())
}
fn compatibility_label(value: ContractCompatibility) -> &'static str {
    match value {
        ContractCompatibility::Additive => "additive",
        ContractCompatibility::MigrationRequired => "migration_required",
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
    fn request(observed_items: usize) -> ThroughputContractModelRequest {
        ThroughputContractModelRequest {
            request_id: "request:throughput-contract".into(),
            batch_id: "batch:001".into(),
            partition: "partition:imaging".into(),
            input_schema: INPUT_SCHEMA.into(),
            output_schema: OUTPUT_SCHEMA.into(),
            compatibility: ContractCompatibility::Additive,
            required_fields: vec![
                "checkpoint".into(),
                "queue_digest".into(),
                "replay_identity".into(),
            ],
            provided_fields: vec![
                "checkpoint".into(),
                "queue_digest".into(),
                "replay_identity".into(),
            ],
            max_items: 100,
            observed_items,
            checkpoint_seq: 3,
            queue_digest: hash("queue"),
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1_and_typed() {
        let manifest = throughput_contract_model_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A1);
    }
    #[test]
    fn complete_capacity_contract_qualifies() {
        let receipt = model_throughput_contract(&request(80)).unwrap();
        assert_eq!(
            receipt.disposition,
            ThroughputContractDisposition::Qualified
        );
        assert_eq!(receipt.admitted_items, 80);
    }
    #[test]
    fn capacity_excess_is_partial_and_explicit() {
        let receipt = model_throughput_contract(&request(120)).unwrap();
        assert_eq!(receipt.disposition, ThroughputContractDisposition::Partial);
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item.contains("capacity-exceeded")));
        assert_eq!(receipt.admitted_items, 100);
    }
    #[test]
    fn empty_batch_is_unknown() {
        let receipt = model_throughput_contract(&request(0)).unwrap();
        assert_eq!(receipt.disposition, ThroughputContractDisposition::Unknown);
    }
    #[test]
    fn migration_is_uncertain() {
        let mut input = request(80);
        input.compatibility = ContractCompatibility::MigrationRequired;
        let receipt = model_throughput_contract(&input).unwrap();
        assert_eq!(receipt.disposition, ThroughputContractDisposition::Partial);
        assert!(receipt
            .uncertainty
            .iter()
            .any(|item| item.contains("migration_required")));
    }
    #[test]
    fn policy_denial_blocks() {
        let mut input = request(80);
        input.policy_allow = false;
        let receipt = model_throughput_contract(&input).unwrap();
        assert_eq!(receipt.disposition, ThroughputContractDisposition::Blocked);
    }
}
