//! Prospective high-throughput evidence-surveillance contract model.
//!
//! Atlas feature: `AFA-adapter-P01-F07`. This typed data primitive binds schema migration to
//! queue capacity, checkpoint identity, and explicit overflow so high-throughput evidence cannot
//! disappear between ingestion and qualification.

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

pub const FEATURE_ID: &str = "AFA-adapter-P01-F07";
pub const CONTRACT_VERSION: &str = "adapter-throughput-evidence-surveillance-contract-model/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceFeed3@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet2@1";
const MAX_TEXT_BYTES: usize = 512;
const MAX_ITEMS: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputContractClaim {
    pub claim_id: String,
    pub sequence: u64,
    pub semantic_type: String,
    pub value_digest: ContentHash,
    pub evidence_state: EvidenceState,
    pub omitted: bool,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputEvidenceSurveillanceContractRequest {
    pub request_id: String,
    pub input_schema: String,
    pub output_schema: String,
    pub batch_id: String,
    pub checkpoint_seq: u64,
    pub previous_checkpoint: Option<ContentHash>,
    pub max_claims: usize,
    pub budget_units: usize,
    pub claims: Vec<ThroughputContractClaim>,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThroughputContractCompatibility {
    Compatible,
    AdditiveMigration,
    Breaking,
    Incompatible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThroughputContractDisposition {
    Compatible,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputEvidenceSurveillanceContractReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub input: ThroughputEvidenceSurveillanceContractRequest,
    pub input_digest: ContentHash,
    pub request_id: String,
    pub input_schema: String,
    pub output_schema: String,
    pub batch_id: String,
    pub checkpoint_seq: u64,
    pub previous_checkpoint: Option<ContentHash>,
    pub max_claims: usize,
    pub budget_units: usize,
    pub compatibility: ThroughputContractCompatibility,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub disposition: ThroughputContractDisposition,
    pub candidate_order: Vec<String>,
    pub sequence_order: Vec<String>,
    pub retained_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub denied_order: Vec<String>,
    pub overflow_order: Vec<String>,
    pub migration_order: Vec<String>,
    pub semantic_loss: Vec<String>,
    pub queue_digest: ContentHash,
    pub checkpoint_digest: ContentHash,
    pub contract_digest: ContentHash,
    pub canonical_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ThroughputEvidenceSurveillanceContractError {
    #[error("invalid throughput contract request: {0}")]
    Invalid(String),
    #[error("throughput contract artifact failed: {0}")]
    Artifact(String),
}
fn validate_text(
    field: &str,
    value: &str,
) -> Result<(), ThroughputEvidenceSurveillanceContractError> {
    if value.is_empty() || value.trim() != value {
        return Err(ThroughputEvidenceSurveillanceContractError::Invalid(
            format!("{field} must be non-empty and trimmed"),
        ));
    }
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(ThroughputEvidenceSurveillanceContractError::Invalid(
            format!("{field} is outside its bounded text contract"),
        ));
    }
    Ok(())
}

fn validate_unique_strings(
    field: &str,
    values: &[String],
) -> Result<(), ThroughputEvidenceSurveillanceContractError> {
    if values.len() > MAX_ITEMS {
        return Err(ThroughputEvidenceSurveillanceContractError::Invalid(
            format!("{field} exceeds its item bound"),
        ));
    }
    let mut unique = BTreeSet::new();
    for value in values {
        validate_text(field, value)?;
        if !unique.insert(value) {
            return Err(ThroughputEvidenceSurveillanceContractError::Invalid(
                format!("{field} contains duplicate values"),
            ));
        }
    }
    Ok(())
}

fn validate_sorted_strings(
    field: &str,
    values: &[String],
) -> Result<(), ThroughputEvidenceSurveillanceContractError> {
    validate_unique_strings(field, values)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ThroughputEvidenceSurveillanceContractError::Invalid(
            format!("{field} ordering is not canonical"),
        ));
    }
    Ok(())
}

fn validate_digest(
    field: &str,
    digest: &ContentHash,
) -> Result<(), ThroughputEvidenceSurveillanceContractError> {
    if digest.as_str().len() != 64
        || !digest
            .as_str()
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(ThroughputEvidenceSurveillanceContractError::Invalid(
            format!("{field} must be a 64-character hex digest"),
        ));
    }
    Ok(())
}

fn canonical_throughput_evidence_surveillance_request(
    request: &ThroughputEvidenceSurveillanceContractRequest,
) -> ThroughputEvidenceSurveillanceContractRequest {
    let mut canonical = request.clone();
    canonical.claims.sort_by(|left, right| {
        left.sequence
            .cmp(&right.sequence)
            .then_with(|| left.claim_id.cmp(&right.claim_id))
    });
    canonical
}

fn contract_input_digest(
    request: &ThroughputEvidenceSurveillanceContractRequest,
) -> Result<ContentHash, ThroughputEvidenceSurveillanceContractError> {
    let canonical = canonical_throughput_evidence_surveillance_request(request);
    let value = serde_json::to_value(canonical).map_err(|error| {
        ThroughputEvidenceSurveillanceContractError::Artifact(error.to_string())
    })?;
    ContentHash::of_value(&value)
        .map_err(|error| ThroughputEvidenceSurveillanceContractError::Artifact(error.to_string()))
}

impl ThroughputEvidenceSurveillanceContractReceipt {
    pub fn validate(&self) -> Result<(), ThroughputEvidenceSurveillanceContractError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.batch_id.trim().is_empty()
            || self.checkpoint_seq == 0
            || self.max_claims == 0
            || self.budget_units == 0
            || self.candidate_order.is_empty()
            || self.sequence_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(ThroughputEvidenceSurveillanceContractError::Invalid("throughput contract identity, schemas, checkpoint, locality, candidates, or effects are incomplete".into()));
        }
        validate_text("request_id", &self.request_id)?;
        validate_text("input_schema", &self.input_schema)?;
        validate_text("output_schema", &self.output_schema)?;
        validate_text("batch_id", &self.batch_id)?;
        validate_text("boundary", &self.boundary)?;
        for values in [
            &self.candidate_order,
            &self.retained_order,
            &self.unknown_order,
            &self.denied_order,
            &self.overflow_order,
            &self.migration_order,
            &self.semantic_loss,
            &self.effect_receipts,
        ] {
            validate_sorted_strings("throughput contract ordering", values)?;
        }
        validate_unique_strings("sequence_order", &self.sequence_order)?;
        if let Some(previous_checkpoint) = &self.previous_checkpoint {
            validate_digest("previous_checkpoint", previous_checkpoint)?;
        }
        let candidate_set = self
            .candidate_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let sequence_set = self.sequence_order.iter().cloned().collect::<BTreeSet<_>>();
        if candidate_set != sequence_set {
            return Err(ThroughputEvidenceSurveillanceContractError::Invalid(
                "throughput sequence and candidate closures differ".into(),
            ));
        }
        let classified = self
            .retained_order
            .iter()
            .chain(self.unknown_order.iter())
            .chain(self.denied_order.iter())
            .chain(self.overflow_order.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        let classified_count = self.retained_order.len()
            + self.unknown_order.len()
            + self.denied_order.len()
            + self.overflow_order.len();
        if classified != candidate_set || classified_count != self.candidate_order.len() {
            return Err(ThroughputEvidenceSurveillanceContractError::Invalid(
                "throughput contract states do not partition candidates".into(),
            ));
        }
        let admission = self
            .max_claims
            .min(self.budget_units)
            .min(self.sequence_order.len());
        let admitted_set = self.sequence_order[..admission]
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let mut expected_overflow = self.sequence_order[admission..].to_vec();
        expected_overflow.sort();
        if self.overflow_order != expected_overflow
            || !self
                .retained_order
                .iter()
                .chain(self.unknown_order.iter())
                .chain(self.denied_order.iter())
                .all(|claim_id| admitted_set.contains(claim_id))
        {
            return Err(ThroughputEvidenceSurveillanceContractError::Invalid(
                "throughput overflow does not match sequence-ordered admission".into(),
            ));
        }
        let capacity_loss = format!(
            "queue:capacity-overflow:{}",
            self.candidate_order.len().saturating_sub(self.max_claims)
        );
        if (self.candidate_order.len() > self.max_claims)
            != self.semantic_loss.contains(&capacity_loss)
        {
            return Err(ThroughputEvidenceSurveillanceContractError::Invalid(
                "throughput capacity-loss witness does not match queue size".into(),
            ));
        }
        let budget_loss = format!(
            "queue:budget-bounded:{}",
            self.max_claims.saturating_sub(self.budget_units)
        );
        if (self.budget_units < self.max_claims) != self.semantic_loss.contains(&budget_loss) {
            return Err(ThroughputEvidenceSurveillanceContractError::Invalid(
                "throughput budget-loss witness does not match admission budget".into(),
            ));
        }
        for claim_id in self.unknown_order.iter().chain(self.denied_order.iter()) {
            let state_loss = [
                format!("claim:{claim_id}:unknown-not-asserted"),
                format!("claim:{claim_id}:release-gate"),
                format!("claim:{claim_id}:breaking-schema"),
                format!("claim:{claim_id}:contradicted-retained"),
            ];
            if !state_loss
                .iter()
                .any(|loss| self.semantic_loss.contains(loss))
            {
                return Err(ThroughputEvidenceSurveillanceContractError::Invalid(
                    "throughput state lacks a semantic-loss witness".into(),
                ));
            }
        }
        let expected_migration =
            if self.compatibility == ThroughputContractCompatibility::AdditiveMigration {
                self.retained_order
                    .iter()
                    .map(|claim_id| format!("claim:{claim_id}:sequence-preserved"))
                    .collect::<BTreeSet<_>>()
            } else {
                BTreeSet::new()
            };
        if self
            .migration_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
            != expected_migration
        {
            return Err(ThroughputEvidenceSurveillanceContractError::Invalid(
                "throughput migration witnesses do not match retained sequence claims".into(),
            ));
        }
        let policy_loss = "control:policy-denied".to_string();
        let closure_loss = "control:protected-closure-incomplete".to_string();
        if self.policy_allow == self.semantic_loss.contains(&policy_loss)
            || self.protected_closure == self.semantic_loss.contains(&closure_loss)
        {
            return Err(ThroughputEvidenceSurveillanceContractError::Invalid(
                "throughput control semantic-loss witnesses do not match policy closure".into(),
            ));
        }
        for digest in [
            &self.queue_digest,
            &self.checkpoint_digest,
            &self.contract_digest,
            &self.canonical_digest,
            &self.provenance_digest,
            &self.replay_identity,
            &self.artifact.content_hash,
        ] {
            validate_digest("throughput contract receipt digest", digest)?;
        }
        let expected_compatibility =
            if self.input_schema == INPUT_SCHEMA && self.output_schema == OUTPUT_SCHEMA {
                ThroughputContractCompatibility::AdditiveMigration
            } else if self.input_schema == self.output_schema {
                ThroughputContractCompatibility::Compatible
            } else {
                ThroughputContractCompatibility::Breaking
            };
        if self.compatibility != expected_compatibility {
            return Err(ThroughputEvidenceSurveillanceContractError::Invalid(
                "throughput contract compatibility does not match its schema pair".into(),
            ));
        }
        let should_block = !self.policy_allow || !self.protected_closure || !self.raw_data_local;
        let expected_disposition = if should_block {
            ThroughputContractDisposition::Blocked
        } else if self.retained_order.is_empty() {
            ThroughputContractDisposition::Unknown
        } else if self.compatibility == ThroughputContractCompatibility::Breaking
            || !self.unknown_order.is_empty()
            || !self.denied_order.is_empty()
            || !self.overflow_order.is_empty()
        {
            ThroughputContractDisposition::Partial
        } else {
            ThroughputContractDisposition::Compatible
        };
        if self.disposition != expected_disposition {
            return Err(ThroughputEvidenceSurveillanceContractError::Invalid(
                "throughput contract disposition does not match queue and release state".into(),
            ));
        }
        if matches!(
            self.disposition,
            ThroughputContractDisposition::Unknown | ThroughputContractDisposition::Blocked
        ) && !self.retained_order.is_empty()
        {
            return Err(ThroughputEvidenceSurveillanceContractError::Invalid(
                "unknown or blocked throughput contract cannot retain claims".into(),
            ));
        }
        let expected_effect = if should_block {
            vec!["block:unsafe-release".to_string()]
        } else {
            vec![format!(
                "read:local-throughput-contract:{}",
                self.request_id
            )]
        };
        if self.effect_receipts != expected_effect {
            return Err(ThroughputEvidenceSurveillanceContractError::Invalid(
                "throughput contract effect does not match its release state".into(),
            ));
        }
        let expected_queue = ContentHash::of_value(&json!({
            "batch_id": self.batch_id,
            "candidate_order": self.candidate_order,
            "sequence_order": self.sequence_order,
            "overflow_order": self.overflow_order,
            "max_claims": self.max_claims,
            "budget_units": self.budget_units,
        }))
        .map_err(|error| {
            ThroughputEvidenceSurveillanceContractError::Artifact(error.to_string())
        })?;
        if self.queue_digest != expected_queue {
            return Err(ThroughputEvidenceSurveillanceContractError::Invalid(
                "queue digest does not match sequence admission and overflow".into(),
            ));
        }
        let expected_checkpoint = ContentHash::of_value(&json!({
            "batch_id": self.batch_id,
            "checkpoint_seq": self.checkpoint_seq,
            "previous_checkpoint": self.previous_checkpoint,
            "queue_digest": self.queue_digest,
        }))
        .map_err(|error| {
            ThroughputEvidenceSurveillanceContractError::Artifact(error.to_string())
        })?;
        if self.checkpoint_digest != expected_checkpoint {
            return Err(ThroughputEvidenceSurveillanceContractError::Invalid(
                "checkpoint digest does not match queue lineage".into(),
            ));
        }
        let expected_contract = ContentHash::of_value(&json!({
            "input_schema": self.input_schema,
            "output_schema": self.output_schema,
            "compatibility": self.compatibility,
            "candidate_order": self.candidate_order,
            "max_claims": self.max_claims,
            "budget_units": self.budget_units,
        }))
        .map_err(|error| {
            ThroughputEvidenceSurveillanceContractError::Artifact(error.to_string())
        })?;
        if self.contract_digest != expected_contract {
            return Err(ThroughputEvidenceSurveillanceContractError::Invalid(
                "contract digest does not match schema and queue capacity".into(),
            ));
        }
        let expected_canonical = ContentHash::of_value(&json!({
            "retained_order": self.retained_order,
            "unknown_order": self.unknown_order,
            "denied_order": self.denied_order,
            "overflow_order": self.overflow_order,
            "migration_order": self.migration_order,
            "semantic_loss": self.semantic_loss,
        }))
        .map_err(|error| {
            ThroughputEvidenceSurveillanceContractError::Artifact(error.to_string())
        })?;
        if self.canonical_digest != expected_canonical {
            return Err(ThroughputEvidenceSurveillanceContractError::Invalid(
                "canonical digest does not match queue loss state".into(),
            ));
        }
        let expected_provenance = ContentHash::of_value(&json!({
            "request_id": self.request_id,
            "replay_identity": self.replay_identity,
            "checkpoint_digest": self.checkpoint_digest,
            "contract_digest": self.contract_digest,
            "canonical_digest": self.canonical_digest,
        }))
        .map_err(|error| {
            ThroughputEvidenceSurveillanceContractError::Artifact(error.to_string())
        })?;
        if self.provenance_digest != expected_provenance {
            return Err(ThroughputEvidenceSurveillanceContractError::Invalid(
                "provenance digest does not match throughput contract identity".into(),
            ));
        }
        if self.artifact.artifact_id
            != format!("adapter-throughput-evidence-contract:{}", self.request_id)
            || self.artifact.content_type
                != "application/vnd.aurora.throughput-evidence-contract+json"
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(ThroughputEvidenceSurveillanceContractError::Artifact(
                "throughput contract artifact is not bound to the receipt".into(),
            ));
        }
        let payload = json!({
            "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
            "contract_version": CONTRACT_VERSION,
            "feature_id": FEATURE_ID,
            "request_id": self.request_id,
            "input_schema": self.input_schema,
            "output_schema": self.output_schema,
            "batch_id": self.batch_id,
            "checkpoint_seq": self.checkpoint_seq,
            "previous_checkpoint": self.previous_checkpoint,
            "max_claims": self.max_claims,
            "budget_units": self.budget_units,
            "compatibility": self.compatibility,
            "policy_allow": self.policy_allow,
            "protected_closure": self.protected_closure,
            "disposition": self.disposition,
            "candidate_order": self.candidate_order,
            "sequence_order": self.sequence_order,
            "retained_order": self.retained_order,
            "unknown_order": self.unknown_order,
            "denied_order": self.denied_order,
            "overflow_order": self.overflow_order,
            "migration_order": self.migration_order,
            "semantic_loss": self.semantic_loss,
            "queue_digest": self.queue_digest,
            "checkpoint_digest": self.checkpoint_digest,
            "contract_digest": self.contract_digest,
            "canonical_digest": self.canonical_digest,
            "provenance_digest": self.provenance_digest,
            "replay_identity": self.replay_identity,
            "effect_receipts": self.effect_receipts,
            "raw_data_local": self.raw_data_local,
            "boundary": PRECLINICAL_BOUNDARY,
        });
        self.artifact.verify_payload(&payload).map_err(|error| {
            ThroughputEvidenceSurveillanceContractError::Artifact(error.to_string())
        })?;
        self.artifact.validate_metadata().map_err(|error| {
            ThroughputEvidenceSurveillanceContractError::Artifact(error.to_string())
        })?;
        if self.input_digest != contract_input_digest(&self.input)? {
            return Err(ThroughputEvidenceSurveillanceContractError::Invalid(
                "throughput contract retained input digest mismatch".into(),
            ));
        }
        let expected = build_throughput_evidence_surveillance_contract(&self.input)?;
        if self != &expected {
            return Err(ThroughputEvidenceSurveillanceContractError::Invalid(
                "throughput contract receipt does not match its retained input".into(),
            ));
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, ThroughputEvidenceSurveillanceContractError> {
        self.validate()?;
        let value = serde_json::to_value(self).map_err(|error| {
            ThroughputEvidenceSurveillanceContractError::Artifact(error.to_string())
        })?;
        ContentHash::of_value(&value).map_err(|error| {
            ThroughputEvidenceSurveillanceContractError::Artifact(error.to_string())
        })
    }
}

pub fn throughput_evidence_surveillance_contract_model_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "adapter".into(), consumers: ["preclinical researcher".into(), "queue schema steward".into()].into(), behavior: "models EvidenceFeed3 into QualifiedEvidenceSet2 with bounded queue, checkpoint, migration, and overflow witnesses".into(), value: "makes high-throughput capacity loss and replay identity part of the scientific data contract".into(), inputs: vec![TypedPort { name: "evidence_feed".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "qualified_evidence_set".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ReadLocalData, Effect::WriteLocalArtifact].into(), permissions: ["read:local-research-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "OpenTelemetry".into(), state: EvidenceState::Supported, locator: Some("https://opentelemetry.io/docs/specs/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn model_throughput_evidence_surveillance_contract(
    request: &ThroughputEvidenceSurveillanceContractRequest,
) -> Result<
    ThroughputEvidenceSurveillanceContractReceipt,
    ThroughputEvidenceSurveillanceContractError,
> {
    let receipt = build_throughput_evidence_surveillance_contract(request)?;
    receipt.validate()?;
    Ok(receipt)
}

fn build_throughput_evidence_surveillance_contract(
    request: &ThroughputEvidenceSurveillanceContractRequest,
) -> Result<
    ThroughputEvidenceSurveillanceContractReceipt,
    ThroughputEvidenceSurveillanceContractError,
> {
    if request.request_id.trim().is_empty()
        || request.input_schema.trim().is_empty()
        || request.output_schema.trim().is_empty()
        || request.batch_id.trim().is_empty()
        || request.checkpoint_seq == 0
        || request.max_claims == 0
        || request.budget_units == 0
        || request.max_claims > MAX_ITEMS
        || request.budget_units > MAX_ITEMS
        || request.claims.is_empty()
        || request.replay_identity.as_str().len() != 64
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
    {
        return Err(ThroughputEvidenceSurveillanceContractError::Invalid("throughput contract identity, schemas, batch/checkpoint, capacity, budget, claims, replay, locality, or boundary is invalid".into()));
    }
    validate_text("request_id", &request.request_id)?;
    validate_text("input_schema", &request.input_schema)?;
    validate_text("output_schema", &request.output_schema)?;
    validate_text("batch_id", &request.batch_id)?;
    validate_text("boundary", &request.boundary)?;
    validate_digest("replay_identity", &request.replay_identity)?;
    if let Some(previous_checkpoint) = &request.previous_checkpoint {
        validate_digest("previous_checkpoint", previous_checkpoint)?;
    }
    if request.claims.len() > MAX_ITEMS {
        return Err(ThroughputEvidenceSurveillanceContractError::Invalid(
            "claims exceeds its item bound".into(),
        ));
    }
    for claim in &request.claims {
        validate_text("claim.claim_id", &claim.claim_id)?;
        validate_text("claim.semantic_type", &claim.semantic_type)?;
        validate_digest("claim.value_digest", &claim.value_digest)?;
    }
    let mut claims = request.claims.clone();
    claims.sort_by(|left, right| {
        left.sequence
            .cmp(&right.sequence)
            .then_with(|| left.claim_id.cmp(&right.claim_id))
    });
    let claim_ids = claims
        .iter()
        .map(|claim| claim.claim_id.clone())
        .collect::<Vec<_>>();
    if claim_ids.windows(2).any(|pair| pair[0] == pair[1])
        || claims
            .windows(2)
            .any(|pair| pair[0].sequence == pair[1].sequence)
    {
        return Err(ThroughputEvidenceSurveillanceContractError::Invalid(
            "throughput claim identities and sequence numbers must be unique".into(),
        ));
    }
    let compatibility =
        if request.input_schema == INPUT_SCHEMA && request.output_schema == OUTPUT_SCHEMA {
            ThroughputContractCompatibility::AdditiveMigration
        } else if request.input_schema == request.output_schema {
            ThroughputContractCompatibility::Compatible
        } else {
            ThroughputContractCompatibility::Breaking
        };
    let admission = request
        .max_claims
        .min(request.budget_units)
        .min(claims.len());
    let (admitted, overflow) = claims.split_at(admission);
    let sequence_order = claim_ids.clone();
    let mut candidate_order = claim_ids.clone();
    candidate_order.sort();
    let overflow_order = overflow
        .iter()
        .map(|claim| claim.claim_id.clone())
        .collect::<BTreeSet<_>>();
    let mut retained = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut denied = BTreeSet::new();
    let mut migration = BTreeSet::new();
    let mut loss = BTreeSet::new();
    let global_release_blocked =
        !request.policy_allow || !request.protected_closure || !request.raw_data_local;
    if claims.len() > request.max_claims {
        loss.insert(format!(
            "queue:capacity-overflow:{}",
            claims.len() - request.max_claims
        ));
    }
    if request.budget_units < request.max_claims {
        loss.insert(format!(
            "queue:budget-bounded:{}",
            request.max_claims - request.budget_units
        ));
    }
    for claim in admitted {
        if global_release_blocked {
            denied.insert(claim.claim_id.clone());
            loss.insert(format!("claim:{}:release-gate", claim.claim_id));
        } else if compatibility == ThroughputContractCompatibility::Breaking {
            denied.insert(claim.claim_id.clone());
            loss.insert(format!("claim:{}:breaking-schema", claim.claim_id));
        } else if claim.omitted
            || matches!(
                claim.evidence_state,
                EvidenceState::Unknown | EvidenceState::Speculative
            )
        {
            unknown.insert(claim.claim_id.clone());
            loss.insert(format!("claim:{}:unknown-not-asserted", claim.claim_id));
        } else if claim.evidence_state == EvidenceState::Contradicted {
            denied.insert(claim.claim_id.clone());
            loss.insert(format!("claim:{}:contradicted-retained", claim.claim_id));
        } else {
            retained.insert(claim.claim_id.clone());
            if compatibility == ThroughputContractCompatibility::AdditiveMigration {
                migration.insert(format!("claim:{}:sequence-preserved", claim.claim_id));
            }
            if claim.negative_result {
                loss.insert(format!("claim:{}:negative-result-retained", claim.claim_id));
            }
        }
    }
    if !request.policy_allow {
        loss.insert("control:policy-denied".into());
    }
    if !request.protected_closure {
        loss.insert("control:protected-closure-incomplete".into());
    }
    let disposition = if global_release_blocked {
        ThroughputContractDisposition::Blocked
    } else if retained.is_empty() {
        ThroughputContractDisposition::Unknown
    } else if compatibility == ThroughputContractCompatibility::Breaking
        || !unknown.is_empty()
        || !denied.is_empty()
        || !overflow_order.is_empty()
    {
        ThroughputContractDisposition::Partial
    } else {
        ThroughputContractDisposition::Compatible
    };
    let retained_order = retained.iter().cloned().collect::<Vec<_>>();
    let unknown_order = unknown.iter().cloned().collect::<Vec<_>>();
    let denied_order = denied.iter().cloned().collect::<Vec<_>>();
    let overflow_order = overflow_order.into_iter().collect::<Vec<_>>();
    let migration_order = migration.iter().cloned().collect::<Vec<_>>();
    let semantic_loss = loss.iter().cloned().collect::<Vec<_>>();
    let queue_digest = ContentHash::of_value(&json!({
        "batch_id": request.batch_id,
        "candidate_order": candidate_order.clone(),
        "sequence_order": sequence_order.clone(),
        "overflow_order": overflow_order.clone(),
        "max_claims": request.max_claims,
        "budget_units": request.budget_units,
    }))
    .map_err(|error| ThroughputEvidenceSurveillanceContractError::Artifact(error.to_string()))?;
    let checkpoint_digest = ContentHash::of_value(&json!({
        "batch_id": request.batch_id,
        "checkpoint_seq": request.checkpoint_seq,
        "previous_checkpoint": request.previous_checkpoint,
        "queue_digest": queue_digest,
    }))
    .map_err(|error| ThroughputEvidenceSurveillanceContractError::Artifact(error.to_string()))?;
    let contract_digest = ContentHash::of_value(&json!({
        "input_schema": request.input_schema,
        "output_schema": request.output_schema,
        "compatibility": compatibility,
        "candidate_order": candidate_order.clone(),
        "max_claims": request.max_claims,
        "budget_units": request.budget_units,
    }))
    .map_err(|error| ThroughputEvidenceSurveillanceContractError::Artifact(error.to_string()))?;
    let canonical_digest = ContentHash::of_value(&json!({
        "retained_order": retained_order.clone(),
        "unknown_order": unknown_order.clone(),
        "denied_order": denied_order.clone(),
        "overflow_order": overflow_order.clone(),
        "migration_order": migration_order.clone(),
        "semantic_loss": semantic_loss.clone(),
    }))
    .map_err(|error| ThroughputEvidenceSurveillanceContractError::Artifact(error.to_string()))?;
    let provenance_digest = ContentHash::of_value(&json!({
        "request_id": request.request_id,
        "replay_identity": request.replay_identity,
        "checkpoint_digest": checkpoint_digest,
        "contract_digest": contract_digest,
        "canonical_digest": canonical_digest,
    }))
    .map_err(|error| ThroughputEvidenceSurveillanceContractError::Artifact(error.to_string()))?;
    let effect_receipts = if disposition == ThroughputContractDisposition::Blocked {
        vec!["block:unsafe-release".into()]
    } else {
        vec![format!(
            "read:local-throughput-contract:{}",
            request.request_id
        )]
    };
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "input_schema": request.input_schema,
        "output_schema": request.output_schema,
        "batch_id": request.batch_id,
        "checkpoint_seq": request.checkpoint_seq,
        "previous_checkpoint": request.previous_checkpoint,
        "max_claims": request.max_claims,
        "budget_units": request.budget_units,
        "compatibility": compatibility,
        "policy_allow": request.policy_allow,
        "protected_closure": request.protected_closure,
        "disposition": disposition,
        "candidate_order": candidate_order,
        "sequence_order": sequence_order,
        "retained_order": retained_order,
        "unknown_order": unknown_order,
        "denied_order": denied_order,
        "overflow_order": overflow_order,
        "migration_order": migration_order,
        "semantic_loss": semantic_loss,
        "queue_digest": queue_digest,
        "checkpoint_digest": checkpoint_digest,
        "contract_digest": contract_digest,
        "canonical_digest": canonical_digest,
        "provenance_digest": provenance_digest,
        "replay_identity": request.replay_identity,
        "effect_receipts": effect_receipts,
        "raw_data_local": request.raw_data_local,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "adapter-throughput-evidence-contract:{}",
            request.request_id
        ),
        "application/vnd.aurora.throughput-evidence-contract+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ThroughputEvidenceSurveillanceContractError::Artifact(error.to_string()))?;
    let canonical_request = canonical_throughput_evidence_surveillance_request(request);
    let receipt = ThroughputEvidenceSurveillanceContractReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        input: canonical_request,
        input_digest: contract_input_digest(request)?,
        request_id: request.request_id.clone(),
        input_schema: request.input_schema.clone(),
        output_schema: request.output_schema.clone(),
        batch_id: request.batch_id.clone(),
        checkpoint_seq: request.checkpoint_seq,
        previous_checkpoint: request.previous_checkpoint.clone(),
        max_claims: request.max_claims,
        budget_units: request.budget_units,
        compatibility,
        policy_allow: request.policy_allow,
        protected_closure: request.protected_closure,
        disposition,
        candidate_order,
        sequence_order,
        retained_order,
        unknown_order,
        denied_order,
        overflow_order,
        migration_order,
        semantic_loss,
        queue_digest,
        checkpoint_digest,
        contract_digest,
        canonical_digest,
        provenance_digest,
        replay_identity: request.replay_identity.clone(),
        effect_receipts,
        artifact,
        raw_data_local: request.raw_data_local,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn request() -> ThroughputEvidenceSurveillanceContractRequest {
        let digest = hash("throughput-contract");
        let claim = |id: &str, sequence: u64, state: EvidenceState| ThroughputContractClaim {
            claim_id: id.into(),
            sequence,
            semantic_type: "evidence".into(),
            value_digest: digest.clone(),
            evidence_state: state,
            omitted: false,
            negative_result: false,
        };
        ThroughputEvidenceSurveillanceContractRequest {
            request_id: "request:throughput-contract".into(),
            input_schema: INPUT_SCHEMA.into(),
            output_schema: OUTPUT_SCHEMA.into(),
            batch_id: "batch:one".into(),
            checkpoint_seq: 8,
            previous_checkpoint: Some(digest.clone()),
            max_claims: 4,
            budget_units: 4,
            claims: vec![
                claim("claim:a", 1, EvidenceState::Supported),
                claim("claim:b", 2, EvidenceState::Supported),
            ],
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            replay_identity: digest,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            throughput_evidence_surveillance_contract_model_manifest().autonomy_tier,
            AutonomyTier::A1
        );
    }
    #[test]
    fn bounded_contract_is_compatible() {
        assert_eq!(
            model_throughput_evidence_surveillance_contract(&request())
                .unwrap()
                .disposition,
            ThroughputContractDisposition::Compatible
        );
    }
    #[test]
    fn overflow_is_partial() {
        let mut value = request();
        value.max_claims = 1;
        let receipt = model_throughput_evidence_surveillance_contract(&value).unwrap();
        assert_eq!(receipt.disposition, ThroughputContractDisposition::Partial);
        assert_eq!(receipt.overflow_order.len(), 1);
    }
    #[test]
    fn unknown_is_preserved() {
        let mut value = request();
        value.claims[0].evidence_state = EvidenceState::Unknown;
        assert!(model_throughput_evidence_surveillance_contract(&value)
            .unwrap()
            .semantic_loss
            .iter()
            .any(|item| item.contains("unknown-not-asserted")));
    }
    #[test]
    fn breaking_is_partial() {
        let mut value = request();
        value.input_schema = "EvidenceFeed9@1".into();
        assert_eq!(
            model_throughput_evidence_surveillance_contract(&value)
                .unwrap()
                .compatibility,
            ThroughputContractCompatibility::Breaking
        );
    }
    #[test]
    fn policy_blocks() {
        let mut value = request();
        value.policy_allow = false;
        let receipt = model_throughput_evidence_surveillance_contract(&value).unwrap();
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
        assert!(receipt.retained_order.is_empty());
        assert_eq!(receipt.disposition, ThroughputContractDisposition::Blocked);
    }
    #[test]
    fn sequence_order_drives_overflow() {
        let mut value = request();
        value.claims[0].sequence = 4;
        value.claims[1].sequence = 1;
        value.max_claims = 1;
        let receipt = model_throughput_evidence_surveillance_contract(&value).unwrap();
        assert_eq!(receipt.sequence_order, vec!["claim:b", "claim:a"]);
        assert_eq!(receipt.overflow_order, vec!["claim:a"]);
    }
    #[test]
    fn duplicate_sequence_is_rejected() {
        let mut value = request();
        value.claims[1].sequence = value.claims[0].sequence;
        assert!(model_throughput_evidence_surveillance_contract(&value).is_err());
    }
    #[test]
    fn breaking_schema_preserves_requested_pair() {
        let mut value = request();
        value.input_schema = "EvidenceFeed9@1".into();
        let receipt = model_throughput_evidence_surveillance_contract(&value).unwrap();
        assert_eq!(receipt.input_schema, "EvidenceFeed9@1");
        assert_eq!(
            receipt.compatibility,
            ThroughputContractCompatibility::Breaking
        );
    }
    #[test]
    fn tampered_queue_digest_is_rejected() {
        let mut receipt = model_throughput_evidence_surveillance_contract(&request()).unwrap();
        receipt.queue_digest = hash("tampered-queue");
        assert!(receipt.validate().is_err());
    }
    #[test]
    fn tampered_artifact_payload_is_rejected() {
        let mut receipt = model_throughput_evidence_surveillance_contract(&request()).unwrap();
        receipt.artifact.content_hash = hash("tampered-payload");
        assert!(receipt.validate().is_err());
    }
    #[test]
    fn checkpoint_is_stable() {
        let first = model_throughput_evidence_surveillance_contract(&request()).unwrap();
        let second = model_throughput_evidence_surveillance_contract(&request()).unwrap();
        assert_eq!(first.checkpoint_digest, second.checkpoint_digest);
    }

    #[test]
    fn reordered_arrivals_share_the_same_retained_input_identity() {
        let mut reordered = request();
        reordered.claims.reverse();
        let first = model_throughput_evidence_surveillance_contract(&request()).unwrap();
        let second = model_throughput_evidence_surveillance_contract(&reordered).unwrap();
        assert_eq!(first.input_digest, second.input_digest);
        assert_eq!(first.digest().unwrap(), second.digest().unwrap());
    }

    #[test]
    fn receipt_rejects_tampered_retained_claim() {
        let mut receipt = model_throughput_evidence_surveillance_contract(&request()).unwrap();
        receipt.input.claims[0].sequence = 99;
        let error = receipt.validate().unwrap_err();
        assert!(error.to_string().contains("retained input digest mismatch"));
    }
}
