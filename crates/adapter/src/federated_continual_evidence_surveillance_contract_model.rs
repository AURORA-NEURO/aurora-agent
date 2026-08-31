//! Federated continual evidence-surveillance contract model.
//!
//! Atlas feature: `AFA-adapter-P01-F08`. This contract keeps federation policy,
//! semantic compatibility, quorum, and aggregate-only locality as typed data.

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

pub const FEATURE_ID: &str = "AFA-adapter-P01-F08";
pub const CONTRACT_VERSION: &str =
    "adapter-federated-continual-evidence-surveillance-contract-model/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceFeed4@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet2@1";
const MAX_TEXT_BYTES: usize = 512;
const MAX_ITEMS: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContinualContractClaim {
    pub claim_id: String,
    pub peer_id: String,
    pub institution_id: String,
    pub artifact_kind: String,
    pub semantic_profile: String,
    pub value_digest: ContentHash,
    pub evidence_state: EvidenceState,
    pub signed: bool,
    pub permitted_artifact: bool,
    pub aggregate_only: bool,
    pub omitted: bool,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContinualEvidenceSurveillanceContractRequest {
    pub request_id: String,
    pub input_schema: String,
    pub output_schema: String,
    pub federation_id: String,
    pub purpose: String,
    pub endpoint: String,
    pub semantic_profile: String,
    pub allowed_artifacts: Vec<String>,
    pub min_peer_quorum: usize,
    pub claims: Vec<FederatedContinualContractClaim>,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedContinualContractCompatibility {
    Compatible,
    AdditiveMigration,
    Breaking,
    Incompatible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedContinualContractDisposition {
    Compatible,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContinualEvidenceSurveillanceContractReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub input: FederatedContinualEvidenceSurveillanceContractRequest,
    pub input_digest: ContentHash,
    pub request_id: String,
    pub input_schema: String,
    pub output_schema: String,
    pub federation_id: String,
    pub purpose: String,
    pub endpoint: String,
    pub semantic_profile: String,
    pub allowed_artifacts: Vec<String>,
    pub min_peer_quorum: usize,
    pub compatibility: FederatedContinualContractCompatibility,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub disposition: FederatedContinualContractDisposition,
    pub peer_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub retained_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub denied_order: Vec<String>,
    pub aggregate_order: Vec<String>,
    pub migration_order: Vec<String>,
    pub semantic_loss: Vec<String>,
    pub federation_digest: ContentHash,
    pub envelope_digest: ContentHash,
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
pub enum FederatedContinualEvidenceSurveillanceContractError {
    #[error("invalid federated continual contract request: {0}")]
    Invalid(String),
    #[error("federated continual contract artifact failed: {0}")]
    Artifact(String),
}

fn validate_text(
    field: &str,
    value: &str,
) -> Result<(), FederatedContinualEvidenceSurveillanceContractError> {
    if value.is_empty() || value.trim() != value {
        return Err(
            FederatedContinualEvidenceSurveillanceContractError::Invalid(format!(
                "{field} must be non-empty and trimmed"
            )),
        );
    }
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(
            FederatedContinualEvidenceSurveillanceContractError::Invalid(format!(
                "{field} is outside its bounded text contract"
            )),
        );
    }
    Ok(())
}

fn validate_unique_strings(
    field: &str,
    values: &[String],
) -> Result<(), FederatedContinualEvidenceSurveillanceContractError> {
    if values.len() > MAX_ITEMS {
        return Err(
            FederatedContinualEvidenceSurveillanceContractError::Invalid(format!(
                "{field} exceeds its item bound"
            )),
        );
    }
    let mut unique = BTreeSet::new();
    for value in values {
        validate_text(field, value)?;
        if !unique.insert(value) {
            return Err(
                FederatedContinualEvidenceSurveillanceContractError::Invalid(format!(
                    "{field} contains duplicate values"
                )),
            );
        }
    }
    Ok(())
}

fn validate_sorted_strings(
    field: &str,
    values: &[String],
) -> Result<(), FederatedContinualEvidenceSurveillanceContractError> {
    validate_unique_strings(field, values)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(
            FederatedContinualEvidenceSurveillanceContractError::Invalid(format!(
                "{field} ordering is not canonical"
            )),
        );
    }
    Ok(())
}

fn validate_digest(
    field: &str,
    digest: &ContentHash,
) -> Result<(), FederatedContinualEvidenceSurveillanceContractError> {
    if digest.as_str().len() != 64
        || !digest
            .as_str()
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(
            FederatedContinualEvidenceSurveillanceContractError::Invalid(format!(
                "{field} must be a 64-character hex digest"
            )),
        );
    }
    Ok(())
}

impl FederatedContinualEvidenceSurveillanceContractReceipt {
    pub fn validate(&self) -> Result<(), FederatedContinualEvidenceSurveillanceContractError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.endpoint.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.allowed_artifacts.is_empty()
            || self.min_peer_quorum == 0
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(FederatedContinualEvidenceSurveillanceContractError::Invalid("federated contract identity, schema, locality, candidates, or effects are incomplete".into()));
        }
        validate_text("request_id", &self.request_id)?;
        validate_text("input_schema", &self.input_schema)?;
        validate_text("output_schema", &self.output_schema)?;
        validate_text("federation_id", &self.federation_id)?;
        validate_text("purpose", &self.purpose)?;
        validate_text("endpoint", &self.endpoint)?;
        validate_text("semantic_profile", &self.semantic_profile)?;
        validate_unique_strings("allowed_artifacts", &self.allowed_artifacts)?;
        for values in [
            &self.peer_order,
            &self.candidate_order,
            &self.retained_order,
            &self.unknown_order,
            &self.denied_order,
            &self.aggregate_order,
            &self.migration_order,
            &self.semantic_loss,
            &self.effect_receipts,
        ] {
            validate_sorted_strings("federated contract ordering", values)?;
        }
        let classified = self
            .retained_order
            .iter()
            .chain(self.unknown_order.iter())
            .chain(self.denied_order.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        let classified_count =
            self.retained_order.len() + self.unknown_order.len() + self.denied_order.len();
        if classified != self.candidate_order.iter().cloned().collect()
            || classified_count != self.candidate_order.len()
            || self
                .aggregate_order
                .iter()
                .any(|id| !self.retained_order.contains(id))
        {
            return Err(
                FederatedContinualEvidenceSurveillanceContractError::Invalid(
                    "federated contract states do not partition candidates".into(),
                ),
            );
        }
        let expected_compatibility =
            if self.input_schema == INPUT_SCHEMA && self.output_schema == OUTPUT_SCHEMA {
                FederatedContinualContractCompatibility::AdditiveMigration
            } else if self.input_schema == self.output_schema {
                FederatedContinualContractCompatibility::Compatible
            } else {
                FederatedContinualContractCompatibility::Breaking
            };
        if self.compatibility != expected_compatibility {
            return Err(
                FederatedContinualEvidenceSurveillanceContractError::Invalid(
                    "federated contract compatibility does not match its schema pair".into(),
                ),
            );
        }
        let quorum_incomplete = self.peer_order.len() < self.min_peer_quorum;
        if (quorum_incomplete && !self.aggregate_order.is_empty())
            || (!quorum_incomplete
                && self
                    .aggregate_order
                    .iter()
                    .cloned()
                    .collect::<BTreeSet<_>>()
                    != self.retained_order.iter().cloned().collect::<BTreeSet<_>>())
        {
            return Err(
                FederatedContinualEvidenceSurveillanceContractError::Invalid(
                    "federated aggregate closure does not match peer quorum".into(),
                ),
            );
        }
        let quorum_loss = format!(
            "federation:quorum-incomplete:{}<{}",
            self.peer_order.len(),
            self.min_peer_quorum
        );
        if quorum_incomplete != self.semantic_loss.contains(&quorum_loss) {
            return Err(
                FederatedContinualEvidenceSurveillanceContractError::Invalid(
                    "federated quorum-loss witness does not match peer closure".into(),
                ),
            );
        }
        for claim_id in self.unknown_order.iter().chain(self.denied_order.iter()) {
            let state_loss = [
                format!("claim:{claim_id}:unknown-not-asserted"),
                format!("claim:{claim_id}:policy-closure-locality"),
                format!("claim:{claim_id}:signature-missing"),
                format!("claim:{claim_id}:artifact-not-permitted"),
                format!("claim:{claim_id}:raw-observation-export-denied"),
                format!("claim:{claim_id}:semantic-profile-mismatch"),
                format!("claim:{claim_id}:breaking-schema"),
                format!("claim:{claim_id}:contradicted"),
            ];
            if !state_loss
                .iter()
                .any(|loss| self.semantic_loss.contains(loss))
            {
                return Err(
                    FederatedContinualEvidenceSurveillanceContractError::Invalid(
                        "federated state lacks a semantic-loss witness".into(),
                    ),
                );
            }
        }
        let expected_migration =
            if self.compatibility == FederatedContinualContractCompatibility::AdditiveMigration {
                self.retained_order
                    .iter()
                    .map(|claim_id| format!("claim:{claim_id}:aggregate-only-migration"))
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
            return Err(
                FederatedContinualEvidenceSurveillanceContractError::Invalid(
                    "federated migration witnesses do not match retained claims".into(),
                ),
            );
        }
        let policy_loss = "control:policy-denied".to_string();
        let closure_loss = "control:protected-closure-incomplete".to_string();
        if self.policy_allow == self.semantic_loss.contains(&policy_loss)
            || self.protected_closure == self.semantic_loss.contains(&closure_loss)
        {
            return Err(
                FederatedContinualEvidenceSurveillanceContractError::Invalid(
                    "federated control semantic-loss witnesses do not match policy closure".into(),
                ),
            );
        }
        for digest in [
            &self.federation_digest,
            &self.envelope_digest,
            &self.contract_digest,
            &self.canonical_digest,
            &self.provenance_digest,
            &self.replay_identity,
            &self.artifact.content_hash,
        ] {
            validate_digest("federated contract receipt digest", digest)?;
        }
        let should_block = !self.policy_allow || !self.protected_closure || !self.raw_data_local;
        let expected_disposition = if should_block {
            FederatedContinualContractDisposition::Blocked
        } else if self.retained_order.is_empty() {
            FederatedContinualContractDisposition::Unknown
        } else if !self.unknown_order.is_empty()
            || !self.denied_order.is_empty()
            || self.peer_order.len() < self.min_peer_quorum
        {
            FederatedContinualContractDisposition::Partial
        } else {
            FederatedContinualContractDisposition::Compatible
        };
        if self.disposition != expected_disposition {
            return Err(
                FederatedContinualEvidenceSurveillanceContractError::Invalid(
                    "federated contract disposition does not match quorum and release state".into(),
                ),
            );
        }
        if matches!(
            self.disposition,
            FederatedContinualContractDisposition::Unknown
                | FederatedContinualContractDisposition::Blocked
        ) && !self.retained_order.is_empty()
        {
            return Err(
                FederatedContinualEvidenceSurveillanceContractError::Invalid(
                    "unknown or blocked federated contract cannot retain claims".into(),
                ),
            );
        }
        let expected_effect = if should_block {
            vec!["block:unsafe-release".to_string()]
        } else {
            vec![format!(
                "exchange:aggregate-evidence-contract:{}",
                self.federation_id
            )]
        };
        if self.effect_receipts != expected_effect {
            return Err(
                FederatedContinualEvidenceSurveillanceContractError::Invalid(
                    "federated contract effect does not match its release state".into(),
                ),
            );
        }
        let expected_federation = ContentHash::of_value(&json!({
            "federation_id": self.federation_id,
            "purpose": self.purpose,
            "endpoint": self.endpoint,
            "peer_order": self.peer_order,
            "semantic_profile": self.semantic_profile,
            "min_peer_quorum": self.min_peer_quorum,
        }))
        .map_err(|error| {
            FederatedContinualEvidenceSurveillanceContractError::Artifact(error.to_string())
        })?;
        if self.federation_digest != expected_federation {
            return Err(
                FederatedContinualEvidenceSurveillanceContractError::Invalid(
                    "federation digest does not match peer and quorum identity".into(),
                ),
            );
        }
        let expected_envelope = ContentHash::of_value(&json!({
            "aggregate_order": self.aggregate_order,
            "allowed_artifacts": self.allowed_artifacts,
            "aggregate_only": true,
            "min_peer_quorum": self.min_peer_quorum,
            "federation_digest": self.federation_digest,
        }))
        .map_err(|error| {
            FederatedContinualEvidenceSurveillanceContractError::Artifact(error.to_string())
        })?;
        if self.envelope_digest != expected_envelope {
            return Err(
                FederatedContinualEvidenceSurveillanceContractError::Invalid(
                    "envelope digest does not match aggregate allow-list".into(),
                ),
            );
        }
        let expected_contract = ContentHash::of_value(&json!({
            "input_schema": self.input_schema,
            "output_schema": self.output_schema,
            "compatibility": self.compatibility,
            "candidate_order": self.candidate_order,
            "allowed_artifacts": self.allowed_artifacts,
            "min_peer_quorum": self.min_peer_quorum,
        }))
        .map_err(|error| {
            FederatedContinualEvidenceSurveillanceContractError::Artifact(error.to_string())
        })?;
        if self.contract_digest != expected_contract {
            return Err(
                FederatedContinualEvidenceSurveillanceContractError::Invalid(
                    "contract digest does not match schema and federation closure".into(),
                ),
            );
        }
        let expected_canonical = ContentHash::of_value(&json!({
            "retained_order": self.retained_order,
            "unknown_order": self.unknown_order,
            "denied_order": self.denied_order,
            "aggregate_order": self.aggregate_order,
            "migration_order": self.migration_order,
            "semantic_loss": self.semantic_loss,
        }))
        .map_err(|error| {
            FederatedContinualEvidenceSurveillanceContractError::Artifact(error.to_string())
        })?;
        if self.canonical_digest != expected_canonical {
            return Err(
                FederatedContinualEvidenceSurveillanceContractError::Invalid(
                    "canonical digest does not match federation loss state".into(),
                ),
            );
        }
        let expected_provenance = ContentHash::of_value(&json!({
            "request_id": self.request_id,
            "replay_identity": self.replay_identity,
            "envelope_digest": self.envelope_digest,
            "contract_digest": self.contract_digest,
            "canonical_digest": self.canonical_digest,
        }))
        .map_err(|error| {
            FederatedContinualEvidenceSurveillanceContractError::Artifact(error.to_string())
        })?;
        if self.provenance_digest != expected_provenance {
            return Err(
                FederatedContinualEvidenceSurveillanceContractError::Invalid(
                    "provenance digest does not match federated contract identity".into(),
                ),
            );
        }
        if self.artifact.artifact_id
            != format!(
                "adapter-federated-continual-evidence-contract:{}",
                self.request_id
            )
            || self.artifact.content_type
                != "application/vnd.aurora.federated-continual-evidence-contract+json"
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(
                FederatedContinualEvidenceSurveillanceContractError::Artifact(
                    "federated contract artifact is not bound to the receipt".into(),
                ),
            );
        }
        let payload = json!({
            "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
            "contract_version": CONTRACT_VERSION,
            "feature_id": FEATURE_ID,
            "request_id": self.request_id,
            "input_schema": self.input_schema,
            "output_schema": self.output_schema,
            "federation_id": self.federation_id,
            "purpose": self.purpose,
            "endpoint": self.endpoint,
            "semantic_profile": self.semantic_profile,
            "allowed_artifacts": self.allowed_artifacts,
            "min_peer_quorum": self.min_peer_quorum,
            "compatibility": self.compatibility,
            "policy_allow": self.policy_allow,
            "protected_closure": self.protected_closure,
            "disposition": self.disposition,
            "peer_order": self.peer_order,
            "candidate_order": self.candidate_order,
            "retained_order": self.retained_order,
            "unknown_order": self.unknown_order,
            "denied_order": self.denied_order,
            "aggregate_order": self.aggregate_order,
            "migration_order": self.migration_order,
            "semantic_loss": self.semantic_loss,
            "federation_digest": self.federation_digest,
            "envelope_digest": self.envelope_digest,
            "contract_digest": self.contract_digest,
            "canonical_digest": self.canonical_digest,
            "provenance_digest": self.provenance_digest,
            "replay_identity": self.replay_identity,
            "effect_receipts": self.effect_receipts,
            "raw_data_local": self.raw_data_local,
            "boundary": PRECLINICAL_BOUNDARY,
        });
        self.artifact.verify_payload(&payload).map_err(|error| {
            FederatedContinualEvidenceSurveillanceContractError::Artifact(error.to_string())
        })?;
        self.artifact.validate_metadata().map_err(|error| {
            FederatedContinualEvidenceSurveillanceContractError::Artifact(error.to_string())
        })?;
        if self.input_digest != contract_input_digest(&self.input)? {
            return Err(
                FederatedContinualEvidenceSurveillanceContractError::Invalid(
                    "federated contract retained input digest mismatch".into(),
                ),
            );
        }
        validate_request(&self.input)?;
        let expected = build_federated_continual_evidence_surveillance_contract(&self.input)?;
        if self != &expected {
            return Err(
                FederatedContinualEvidenceSurveillanceContractError::Invalid(
                    "federated contract receipt does not match its retained input".into(),
                ),
            );
        }
        Ok(())
    }

    pub fn digest(
        &self,
    ) -> Result<ContentHash, FederatedContinualEvidenceSurveillanceContractError> {
        self.validate()?;
        let value = serde_json::to_value(self).map_err(|error| {
            FederatedContinualEvidenceSurveillanceContractError::Artifact(error.to_string())
        })?;
        ContentHash::of_value(&value).map_err(|error| {
            FederatedContinualEvidenceSurveillanceContractError::Artifact(error.to_string())
        })
    }
}

pub fn federated_continual_evidence_surveillance_contract_model_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "adapter".into(), consumers: ["consortium administrator".into(), "federation schema steward".into()].into(), behavior: "models EvidenceFeed4 into a versioned QualifiedEvidenceSet2 contract with aggregate-only federation, quorum, omission, and migration witnesses".into(), value: "enables continual consortium evidence alerts without hiding unknown evidence or moving raw observations".into(), inputs: vec![TypedPort { name: "evidence_feed".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "qualified_evidence_set".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ReadLocalData, Effect::FederationExport, Effect::WriteLocalArtifact].into(), permissions: ["read:local-research-artifacts".into(), "export:permitted-aggregate-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "W3C PROV-O".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub(crate) fn canonical_federated_continual_evidence_surveillance_contract_request(
    request: &FederatedContinualEvidenceSurveillanceContractRequest,
) -> FederatedContinualEvidenceSurveillanceContractRequest {
    let mut canonical = request.clone();
    canonical.allowed_artifacts.sort();
    canonical
        .claims
        .sort_by(|left, right| left.claim_id.cmp(&right.claim_id));
    canonical
}

fn contract_input_digest(
    request: &FederatedContinualEvidenceSurveillanceContractRequest,
) -> Result<ContentHash, FederatedContinualEvidenceSurveillanceContractError> {
    let canonical = canonical_federated_continual_evidence_surveillance_contract_request(request);
    let value = serde_json::to_value(canonical).map_err(|error| {
        FederatedContinualEvidenceSurveillanceContractError::Artifact(error.to_string())
    })?;
    ContentHash::of_value(&value).map_err(|error| {
        FederatedContinualEvidenceSurveillanceContractError::Artifact(error.to_string())
    })
}

pub fn model_federated_continual_evidence_surveillance_contract(
    request: &FederatedContinualEvidenceSurveillanceContractRequest,
) -> Result<
    FederatedContinualEvidenceSurveillanceContractReceipt,
    FederatedContinualEvidenceSurveillanceContractError,
> {
    validate_request(request)?;
    let receipt = build_federated_continual_evidence_surveillance_contract(request)?;
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(
    request: &FederatedContinualEvidenceSurveillanceContractRequest,
) -> Result<(), FederatedContinualEvidenceSurveillanceContractError> {
    if request.request_id.trim().is_empty()
        || request.input_schema.trim().is_empty()
        || request.output_schema.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.endpoint.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.allowed_artifacts.is_empty()
        || request.min_peer_quorum == 0
        || request.min_peer_quorum > MAX_ITEMS
        || request.claims.is_empty()
        || request.claims.len() > MAX_ITEMS
        || request.replay_identity.as_str().len() != 64
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
    {
        return Err(FederatedContinualEvidenceSurveillanceContractError::Invalid("federated contract identity, schema, purpose, allow-list, quorum, claims, replay, locality, or boundary is invalid".into()));
    }
    validate_text("request_id", &request.request_id)?;
    validate_text("input_schema", &request.input_schema)?;
    validate_text("output_schema", &request.output_schema)?;
    validate_text("federation_id", &request.federation_id)?;
    validate_text("purpose", &request.purpose)?;
    validate_text("endpoint", &request.endpoint)?;
    validate_text("semantic_profile", &request.semantic_profile)?;
    validate_text("boundary", &request.boundary)?;
    validate_unique_strings("allowed_artifacts", &request.allowed_artifacts)?;
    validate_digest("replay_identity", &request.replay_identity)?;
    for claim in &request.claims {
        validate_text("claim.claim_id", &claim.claim_id)?;
        validate_text("claim.peer_id", &claim.peer_id)?;
        validate_text("claim.institution_id", &claim.institution_id)?;
        validate_text("claim.artifact_kind", &claim.artifact_kind)?;
        validate_text("claim.semantic_profile", &claim.semantic_profile)?;
        validate_digest("claim.value_digest", &claim.value_digest)?;
    }
    Ok(())
}

fn build_federated_continual_evidence_surveillance_contract(
    request: &FederatedContinualEvidenceSurveillanceContractRequest,
) -> Result<
    FederatedContinualEvidenceSurveillanceContractReceipt,
    FederatedContinualEvidenceSurveillanceContractError,
> {
    let canonical_request =
        canonical_federated_continual_evidence_surveillance_contract_request(request);
    let request = &canonical_request;
    let mut claims = request.claims.clone();
    claims.sort_by(|left, right| left.claim_id.cmp(&right.claim_id));
    let candidate_order = claims
        .iter()
        .map(|claim| claim.claim_id.clone())
        .collect::<Vec<_>>();
    if candidate_order.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(
            FederatedContinualEvidenceSurveillanceContractError::Invalid(
                "federated claim identities must be unique and non-empty".into(),
            ),
        );
    }
    let compatibility =
        if request.input_schema == INPUT_SCHEMA && request.output_schema == OUTPUT_SCHEMA {
            FederatedContinualContractCompatibility::AdditiveMigration
        } else if request.input_schema == request.output_schema {
            FederatedContinualContractCompatibility::Compatible
        } else {
            FederatedContinualContractCompatibility::Breaking
        };
    let mut retained = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut denied = BTreeSet::new();
    let mut peers = BTreeSet::new();
    let mut aggregate = BTreeSet::new();
    let mut migration = BTreeSet::new();
    let mut loss = BTreeSet::new();
    for claim in &claims {
        if !request.policy_allow || !request.protected_closure || !request.raw_data_local {
            denied.insert(claim.claim_id.clone());
            loss.insert(format!("claim:{}:policy-closure-locality", claim.claim_id));
        } else if !claim.signed {
            denied.insert(claim.claim_id.clone());
            loss.insert(format!("claim:{}:signature-missing", claim.claim_id));
        } else if !claim.permitted_artifact
            || !request.allowed_artifacts.contains(&claim.artifact_kind)
        {
            denied.insert(claim.claim_id.clone());
            loss.insert(format!("claim:{}:artifact-not-permitted", claim.claim_id));
        } else if !claim.aggregate_only {
            denied.insert(claim.claim_id.clone());
            loss.insert(format!(
                "claim:{}:raw-observation-export-denied",
                claim.claim_id
            ));
        } else if claim.semantic_profile != request.semantic_profile {
            denied.insert(claim.claim_id.clone());
            loss.insert(format!(
                "claim:{}:semantic-profile-mismatch",
                claim.claim_id
            ));
        } else if compatibility == FederatedContinualContractCompatibility::Breaking {
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
            loss.insert(format!("claim:{}:contradicted", claim.claim_id));
        } else {
            retained.insert(claim.claim_id.clone());
            peers.insert(claim.peer_id.clone());
            aggregate.insert(claim.claim_id.clone());
            if compatibility == FederatedContinualContractCompatibility::AdditiveMigration {
                migration.insert(format!("claim:{}:aggregate-only-migration", claim.claim_id));
            }
            if claim.negative_result {
                loss.insert(format!("claim:{}:negative-result-retained", claim.claim_id));
            }
        }
    }
    let quorum_incomplete = peers.len() < request.min_peer_quorum;
    if quorum_incomplete {
        aggregate.clear();
    }
    if peers.len() < request.min_peer_quorum {
        loss.insert(format!(
            "federation:quorum-incomplete:{}<{}",
            peers.len(),
            request.min_peer_quorum
        ));
    }
    if !request.policy_allow {
        loss.insert("control:policy-denied".into());
    }
    if !request.protected_closure {
        loss.insert("control:protected-closure-incomplete".into());
    }
    let disposition =
        if !request.policy_allow || !request.protected_closure || !request.raw_data_local {
            FederatedContinualContractDisposition::Blocked
        } else if retained.is_empty() {
            FederatedContinualContractDisposition::Unknown
        } else if !unknown.is_empty() || !denied.is_empty() || peers.len() < request.min_peer_quorum
        {
            FederatedContinualContractDisposition::Partial
        } else {
            FederatedContinualContractDisposition::Compatible
        };
    let peer_order = peers.iter().cloned().collect::<Vec<_>>();
    let retained_order = retained.iter().cloned().collect::<Vec<_>>();
    let unknown_order = unknown.iter().cloned().collect::<Vec<_>>();
    let denied_order = denied.iter().cloned().collect::<Vec<_>>();
    let aggregate_order = aggregate.iter().cloned().collect::<Vec<_>>();
    let migration_order = migration.iter().cloned().collect::<Vec<_>>();
    let semantic_loss = loss.iter().cloned().collect::<Vec<_>>();
    let federation_digest = ContentHash::of_value(&json!({
        "federation_id": request.federation_id,
        "purpose": request.purpose,
        "endpoint": request.endpoint,
        "peer_order": peer_order.clone(),
        "semantic_profile": request.semantic_profile,
        "min_peer_quorum": request.min_peer_quorum,
    }))
    .map_err(|error| {
        FederatedContinualEvidenceSurveillanceContractError::Artifact(error.to_string())
    })?;
    let envelope_digest = ContentHash::of_value(&json!({
        "aggregate_order": aggregate_order.clone(),
        "allowed_artifacts": request.allowed_artifacts.clone(),
        "aggregate_only": true,
        "min_peer_quorum": request.min_peer_quorum,
        "federation_digest": federation_digest,
    }))
    .map_err(|error| {
        FederatedContinualEvidenceSurveillanceContractError::Artifact(error.to_string())
    })?;
    let contract_digest = ContentHash::of_value(&json!({
        "input_schema": request.input_schema,
        "output_schema": request.output_schema,
        "compatibility": compatibility,
        "candidate_order": candidate_order.clone(),
        "allowed_artifacts": request.allowed_artifacts.clone(),
        "min_peer_quorum": request.min_peer_quorum,
    }))
    .map_err(|error| {
        FederatedContinualEvidenceSurveillanceContractError::Artifact(error.to_string())
    })?;
    let canonical_digest = ContentHash::of_value(&json!({
        "retained_order": retained_order.clone(),
        "unknown_order": unknown_order.clone(),
        "denied_order": denied_order.clone(),
        "aggregate_order": aggregate_order.clone(),
        "migration_order": migration_order.clone(),
        "semantic_loss": semantic_loss.clone(),
    }))
    .map_err(|error| {
        FederatedContinualEvidenceSurveillanceContractError::Artifact(error.to_string())
    })?;
    let provenance_digest = ContentHash::of_value(&json!({
        "request_id": request.request_id,
        "replay_identity": request.replay_identity,
        "envelope_digest": envelope_digest,
        "contract_digest": contract_digest,
        "canonical_digest": canonical_digest,
    }))
    .map_err(|error| {
        FederatedContinualEvidenceSurveillanceContractError::Artifact(error.to_string())
    })?;
    let effect_receipts = if disposition == FederatedContinualContractDisposition::Blocked {
        vec!["block:unsafe-release".into()]
    } else {
        vec![format!(
            "exchange:aggregate-evidence-contract:{}",
            request.federation_id
        )]
    };
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "input_schema": request.input_schema,
        "output_schema": request.output_schema,
        "federation_id": request.federation_id,
        "purpose": request.purpose,
        "endpoint": request.endpoint,
        "semantic_profile": request.semantic_profile,
        "allowed_artifacts": request.allowed_artifacts,
        "min_peer_quorum": request.min_peer_quorum,
        "compatibility": compatibility,
        "policy_allow": request.policy_allow,
        "protected_closure": request.protected_closure,
        "disposition": disposition,
        "peer_order": peer_order,
        "candidate_order": candidate_order,
        "retained_order": retained_order,
        "unknown_order": unknown_order,
        "denied_order": denied_order,
        "aggregate_order": aggregate_order,
        "migration_order": migration_order,
        "semantic_loss": semantic_loss,
        "federation_digest": federation_digest,
        "envelope_digest": envelope_digest,
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
            "adapter-federated-continual-evidence-contract:{}",
            request.request_id
        ),
        "application/vnd.aurora.federated-continual-evidence-contract+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| {
        FederatedContinualEvidenceSurveillanceContractError::Artifact(error.to_string())
    })?;
    let input_digest = contract_input_digest(request)?;
    let receipt = FederatedContinualEvidenceSurveillanceContractReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        input: canonical_request.clone(),
        input_digest,
        request_id: request.request_id.clone(),
        input_schema: request.input_schema.clone(),
        output_schema: request.output_schema.clone(),
        federation_id: request.federation_id.clone(),
        purpose: request.purpose.clone(),
        endpoint: request.endpoint.clone(),
        semantic_profile: request.semantic_profile.clone(),
        allowed_artifacts: request.allowed_artifacts.clone(),
        min_peer_quorum: request.min_peer_quorum,
        compatibility,
        policy_allow: request.policy_allow,
        protected_closure: request.protected_closure,
        disposition,
        peer_order,
        candidate_order,
        retained_order,
        unknown_order,
        denied_order,
        aggregate_order,
        migration_order,
        semantic_loss,
        federation_digest,
        envelope_digest,
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
    fn request() -> FederatedContinualEvidenceSurveillanceContractRequest {
        let digest = hash("federated-contract");
        let claim = |id: &str, peer: &str| FederatedContinualContractClaim {
            claim_id: id.into(),
            peer_id: peer.into(),
            institution_id: format!("institution:{peer}"),
            artifact_kind: "aggregate-evidence".into(),
            semantic_profile: "profile:v1".into(),
            value_digest: digest.clone(),
            evidence_state: EvidenceState::Supported,
            signed: true,
            permitted_artifact: true,
            aggregate_only: true,
            omitted: false,
            negative_result: false,
        };
        FederatedContinualEvidenceSurveillanceContractRequest {
            request_id: "request:federated-contract".into(),
            input_schema: INPUT_SCHEMA.into(),
            output_schema: OUTPUT_SCHEMA.into(),
            federation_id: "federation:one".into(),
            purpose: "compare preclinical evidence".into(),
            endpoint: "local://federation".into(),
            semantic_profile: "profile:v1".into(),
            allowed_artifacts: vec!["aggregate-evidence".into()],
            min_peer_quorum: 2,
            claims: vec![claim("claim:a", "peer:a"), claim("claim:b", "peer:b")],
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
            federated_continual_evidence_surveillance_contract_model_manifest().autonomy_tier,
            AutonomyTier::A1
        );
    }
    #[test]
    fn quorum_contract_is_compatible() {
        assert_eq!(
            model_federated_continual_evidence_surveillance_contract(&request())
                .unwrap()
                .disposition,
            FederatedContinualContractDisposition::Compatible
        );
    }
    #[test]
    fn quorum_gap_is_partial() {
        let mut value = request();
        value.min_peer_quorum = 3;
        let receipt = model_federated_continual_evidence_surveillance_contract(&value).unwrap();
        assert_eq!(
            receipt.disposition,
            FederatedContinualContractDisposition::Partial
        );
        assert!(receipt.aggregate_order.is_empty());
    }
    #[test]
    fn unsigned_is_denied() {
        let mut value = request();
        value.claims[0].signed = false;
        assert!(
            model_federated_continual_evidence_surveillance_contract(&value)
                .unwrap()
                .denied_order
                .iter()
                .any(|item| item == "claim:a")
        );
    }
    #[test]
    fn raw_export_is_denied() {
        let mut value = request();
        value.claims[0].aggregate_only = false;
        assert!(
            model_federated_continual_evidence_surveillance_contract(&value)
                .unwrap()
                .semantic_loss
                .iter()
                .any(|item| item.contains("raw-observation-export-denied"))
        );
    }
    #[test]
    fn unknown_is_not_asserted() {
        let mut value = request();
        value.claims[0].evidence_state = EvidenceState::Unknown;
        assert!(
            model_federated_continual_evidence_surveillance_contract(&value)
                .unwrap()
                .semantic_loss
                .iter()
                .any(|item| item.contains("unknown-not-asserted"))
        );
    }
    #[test]
    fn policy_blocks() {
        let mut value = request();
        value.policy_allow = false;
        let receipt = model_federated_continual_evidence_surveillance_contract(&value).unwrap();
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
        assert!(receipt.retained_order.is_empty());
        assert!(receipt.aggregate_order.is_empty());
        assert_eq!(
            receipt.disposition,
            FederatedContinualContractDisposition::Blocked
        );
    }
    #[test]
    fn breaking_schema_preserves_requested_pair() {
        let mut value = request();
        value.input_schema = "EvidenceFeed9@1".into();
        let receipt = model_federated_continual_evidence_surveillance_contract(&value).unwrap();
        assert_eq!(receipt.input_schema, "EvidenceFeed9@1");
        assert_eq!(
            receipt.compatibility,
            FederatedContinualContractCompatibility::Breaking
        );
    }
    #[test]
    fn duplicate_allowed_artifact_is_rejected() {
        let mut value = request();
        value.allowed_artifacts.push("aggregate-evidence".into());
        assert!(model_federated_continual_evidence_surveillance_contract(&value).is_err());
    }
    #[test]
    fn tampered_federation_digest_is_rejected() {
        let mut receipt =
            model_federated_continual_evidence_surveillance_contract(&request()).unwrap();
        receipt.federation_digest = hash("tampered-federation");
        assert!(receipt.validate().is_err());
    }
    #[test]
    fn tampered_artifact_payload_is_rejected() {
        let mut receipt =
            model_federated_continual_evidence_surveillance_contract(&request()).unwrap();
        receipt.artifact.content_hash = hash("tampered-payload");
        assert!(receipt.validate().is_err());
    }
    #[test]
    fn replay_is_stable() {
        let first = model_federated_continual_evidence_surveillance_contract(&request()).unwrap();
        let second = model_federated_continual_evidence_surveillance_contract(&request()).unwrap();
        assert_eq!(first.envelope_digest, second.envelope_digest);
    }

    #[test]
    fn reordered_claims_and_allow_list_have_stable_identity() {
        let mut reordered = request();
        reordered.claims.reverse();
        reordered.allowed_artifacts.reverse();
        let first = model_federated_continual_evidence_surveillance_contract(&request()).unwrap();
        let second = model_federated_continual_evidence_surveillance_contract(&reordered).unwrap();
        assert_eq!(first.input_digest, second.input_digest);
        assert_eq!(first.envelope_digest, second.envelope_digest);
        assert_eq!(first.digest().unwrap(), second.digest().unwrap());
    }

    #[test]
    fn receipt_rejects_tampered_retained_claim() {
        let mut receipt =
            model_federated_continual_evidence_surveillance_contract(&request()).unwrap();
        receipt.input.claims[0].value_digest = hash("tampered-claim");
        let error = receipt.validate().unwrap_err();
        assert!(error.to_string().contains("retained input digest mismatch"));
    }
}
