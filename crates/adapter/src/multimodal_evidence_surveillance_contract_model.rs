//! Multimodal multi-study evidence-surveillance contract model.
//!
//! Atlas feature: `AFA-adapter-P01-F06`. The contract makes semantic-profile and study×modality
//! comparability part of the typed data primitive, not an informal caller convention.

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

pub const FEATURE_ID: &str = "AFA-adapter-P01-F06";
pub const CONTRACT_VERSION: &str = "adapter-multimodal-evidence-surveillance-contract-model/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceFeed2@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet2@1";
const MAX_TEXT_BYTES: usize = 512;
const MAX_ITEMS: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalContractClaim {
    pub claim_id: String,
    pub study_id: String,
    pub modality: String,
    pub semantic_profile: String,
    pub value_digest: ContentHash,
    pub evidence_state: EvidenceState,
    pub omitted: bool,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalEvidenceSurveillanceContractRequest {
    pub request_id: String,
    pub input_schema: String,
    pub output_schema: String,
    pub semantic_profile: String,
    pub required_studies: Vec<String>,
    pub required_modalities: Vec<String>,
    pub claims: Vec<MultimodalContractClaim>,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultimodalContractCompatibility {
    Compatible,
    AdditiveMigration,
    Breaking,
    Incompatible,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultimodalContractDisposition {
    Compatible,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalEvidenceSurveillanceContractReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub input: MultimodalEvidenceSurveillanceContractRequest,
    pub input_digest: ContentHash,
    pub request_id: String,
    pub input_schema: String,
    pub output_schema: String,
    pub semantic_profile: String,
    pub compatibility: MultimodalContractCompatibility,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub disposition: MultimodalContractDisposition,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub retained_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub denied_order: Vec<String>,
    pub incomparable_order: Vec<String>,
    pub migration_order: Vec<String>,
    pub semantic_loss: Vec<String>,
    pub comparability_digest: ContentHash,
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
pub enum MultimodalEvidenceSurveillanceContractError {
    #[error("invalid multimodal contract request: {0}")]
    Invalid(String),
    #[error("multimodal contract artifact failed: {0}")]
    Artifact(String),
}

fn validate_text(
    field: &str,
    value: &str,
) -> Result<(), MultimodalEvidenceSurveillanceContractError> {
    if value.is_empty() || value.trim() != value {
        return Err(MultimodalEvidenceSurveillanceContractError::Invalid(
            format!("{field} must be non-empty and trimmed"),
        ));
    }
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(MultimodalEvidenceSurveillanceContractError::Invalid(
            format!("{field} is outside its bounded text contract"),
        ));
    }
    Ok(())
}

fn validate_unique_strings(
    field: &str,
    values: &[String],
) -> Result<(), MultimodalEvidenceSurveillanceContractError> {
    if values.len() > MAX_ITEMS {
        return Err(MultimodalEvidenceSurveillanceContractError::Invalid(
            format!("{field} exceeds its item bound"),
        ));
    }
    let mut unique = BTreeSet::new();
    for value in values {
        validate_text(field, value)?;
        if !unique.insert(value) {
            return Err(MultimodalEvidenceSurveillanceContractError::Invalid(
                format!("{field} contains duplicate values"),
            ));
        }
    }
    Ok(())
}

fn validate_sorted_strings(
    field: &str,
    values: &[String],
) -> Result<(), MultimodalEvidenceSurveillanceContractError> {
    validate_unique_strings(field, values)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(MultimodalEvidenceSurveillanceContractError::Invalid(
            format!("{field} ordering is not canonical"),
        ));
    }
    Ok(())
}

fn validate_digest(
    field: &str,
    digest: &ContentHash,
) -> Result<(), MultimodalEvidenceSurveillanceContractError> {
    if digest.as_str().len() != 64
        || !digest
            .as_str()
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(MultimodalEvidenceSurveillanceContractError::Invalid(
            format!("{field} must be a 64-character hex digest"),
        ));
    }
    Ok(())
}

fn canonical_multimodal_evidence_surveillance_request(
    request: &MultimodalEvidenceSurveillanceContractRequest,
) -> MultimodalEvidenceSurveillanceContractRequest {
    let mut canonical = request.clone();
    canonical.required_studies.sort();
    canonical.required_modalities.sort();
    canonical
        .claims
        .sort_by(|left, right| left.claim_id.cmp(&right.claim_id));
    canonical
}

fn contract_input_digest(
    request: &MultimodalEvidenceSurveillanceContractRequest,
) -> Result<ContentHash, MultimodalEvidenceSurveillanceContractError> {
    let canonical = canonical_multimodal_evidence_surveillance_request(request);
    let value = serde_json::to_value(canonical).map_err(|error| {
        MultimodalEvidenceSurveillanceContractError::Artifact(error.to_string())
    })?;
    ContentHash::of_value(&value)
        .map_err(|error| MultimodalEvidenceSurveillanceContractError::Artifact(error.to_string()))
}

fn required_cell(study: &str, modality: &str) -> String {
    format!("{study}::{modality}::required")
}

impl MultimodalEvidenceSurveillanceContractReceipt {
    pub fn validate(&self) -> Result<(), MultimodalEvidenceSurveillanceContractError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.study_order.len() < 2
            || self.modality_order.len() < 2
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(MultimodalEvidenceSurveillanceContractError::Invalid("multimodal contract identity, schemas, closure, locality, candidates, or effects are incomplete".into()));
        }
        validate_text("request_id", &self.request_id)?;
        validate_text("input_schema", &self.input_schema)?;
        validate_text("output_schema", &self.output_schema)?;
        validate_text("semantic_profile", &self.semantic_profile)?;
        validate_text("boundary", &self.boundary)?;
        for values in [
            &self.study_order,
            &self.modality_order,
            &self.candidate_order,
            &self.retained_order,
            &self.unknown_order,
            &self.denied_order,
            &self.incomparable_order,
            &self.migration_order,
            &self.semantic_loss,
            &self.effect_receipts,
        ] {
            validate_sorted_strings("multimodal contract ordering", values)?;
        }
        let required_cells = self
            .study_order
            .iter()
            .flat_map(|study| {
                self.modality_order
                    .iter()
                    .map(move |modality| required_cell(study, modality))
            })
            .collect::<BTreeSet<_>>();
        let candidate_set = self
            .candidate_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        if !required_cells.is_subset(&candidate_set) {
            return Err(MultimodalEvidenceSurveillanceContractError::Invalid(
                "multimodal candidate closure omits a required study×modality cell".into(),
            ));
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
        if classified != candidate_set || classified_count != self.candidate_order.len() {
            return Err(MultimodalEvidenceSurveillanceContractError::Invalid(
                "multimodal contract states do not partition candidates".into(),
            ));
        }
        let retained_set = self.retained_order.iter().cloned().collect::<BTreeSet<_>>();
        let unknown_set = self.unknown_order.iter().cloned().collect::<BTreeSet<_>>();
        let denied_set = self.denied_order.iter().cloned().collect::<BTreeSet<_>>();
        if required_cells.iter().any(|cell| {
            (!retained_set.contains(cell) && !unknown_set.contains(cell))
                || denied_set.contains(cell)
        }) {
            return Err(MultimodalEvidenceSurveillanceContractError::Invalid(
                "required study×modality cells must be retained or unresolved, never denied".into(),
            ));
        }
        let covered_cells = required_cells
            .intersection(&retained_set)
            .cloned()
            .collect::<BTreeSet<_>>();
        let expected_compatibility =
            if self.input_schema == INPUT_SCHEMA && self.output_schema == OUTPUT_SCHEMA {
                MultimodalContractCompatibility::AdditiveMigration
            } else if self.input_schema == self.output_schema {
                MultimodalContractCompatibility::Compatible
            } else {
                MultimodalContractCompatibility::Breaking
            };
        if self.compatibility != expected_compatibility {
            return Err(MultimodalEvidenceSurveillanceContractError::Invalid(
                "multimodal contract compatibility does not match its schema pair".into(),
            ));
        }
        let expected_migration =
            if self.compatibility == MultimodalContractCompatibility::AdditiveMigration {
                retained_set
                    .difference(&required_cells)
                    .map(|claim_id| format!("claim:{claim_id}:study-modality-preserved"))
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
            return Err(MultimodalEvidenceSurveillanceContractError::Invalid(
                "multimodal migration witnesses do not match retained claims".into(),
            ));
        }
        let expected_incomparable = self
            .semantic_loss
            .iter()
            .filter_map(|loss| {
                loss.strip_prefix("claim:")
                    .and_then(|value| value.strip_suffix(":semantic-profile-incomparable"))
                    .map(str::to_owned)
            })
            .collect::<BTreeSet<_>>();
        if self
            .incomparable_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
            != expected_incomparable
            || !expected_incomparable.is_subset(&denied_set)
        {
            return Err(MultimodalEvidenceSurveillanceContractError::Invalid(
                "multimodal incomparable witnesses do not match semantic-loss closure".into(),
            ));
        }
        for item in &self.unknown_order {
            let cell_loss = format!("cell:{item}:comparability-incomplete");
            let claim_losses = [
                format!("claim:{item}:omitted-preserved"),
                format!("claim:{item}:unknown-not-asserted"),
                format!("claim:{item}:required-unresolved"),
            ];
            if (required_cells.contains(item) && !self.semantic_loss.contains(&cell_loss))
                || (!required_cells.contains(item)
                    && !claim_losses
                        .iter()
                        .any(|loss| self.semantic_loss.contains(loss)))
            {
                return Err(MultimodalEvidenceSurveillanceContractError::Invalid(
                    "multimodal unknown state lacks a semantic-loss witness".into(),
                ));
            }
        }
        for item in &self.denied_order {
            let claim_losses = [
                format!("claim:{item}:release-gate"),
                format!("claim:{item}:breaking-schema"),
                format!("claim:{item}:scope-mismatch"),
                format!("claim:{item}:semantic-profile-incomparable"),
                format!("claim:{item}:contradicted-retained"),
            ];
            if !claim_losses
                .iter()
                .any(|loss| self.semantic_loss.contains(loss))
            {
                return Err(MultimodalEvidenceSurveillanceContractError::Invalid(
                    "multimodal denied state lacks a semantic-loss witness".into(),
                ));
            }
        }
        let policy_loss = "control:policy-denied".to_string();
        let closure_loss = "control:protected-closure-incomplete".to_string();
        if self.policy_allow == self.semantic_loss.contains(&policy_loss)
            || self.protected_closure == self.semantic_loss.contains(&closure_loss)
        {
            return Err(MultimodalEvidenceSurveillanceContractError::Invalid(
                "multimodal control semantic-loss witnesses do not match policy closure".into(),
            ));
        }
        for digest in [
            &self.comparability_digest,
            &self.contract_digest,
            &self.canonical_digest,
            &self.provenance_digest,
            &self.replay_identity,
            &self.artifact.content_hash,
        ] {
            validate_digest("multimodal contract receipt digest", digest)?;
        }
        let should_block = !self.policy_allow || !self.protected_closure || !self.raw_data_local;
        let expected_disposition = if should_block {
            MultimodalContractDisposition::Blocked
        } else if self.retained_order.is_empty() {
            MultimodalContractDisposition::Unknown
        } else if self.compatibility == MultimodalContractCompatibility::Breaking
            || !self.unknown_order.is_empty()
            || !self.denied_order.is_empty()
        {
            MultimodalContractDisposition::Partial
        } else {
            MultimodalContractDisposition::Compatible
        };
        if self.disposition != expected_disposition {
            return Err(MultimodalEvidenceSurveillanceContractError::Invalid(
                "multimodal contract disposition does not match compatibility and closure".into(),
            ));
        }
        if matches!(
            self.disposition,
            MultimodalContractDisposition::Unknown | MultimodalContractDisposition::Blocked
        ) && !self.retained_order.is_empty()
        {
            return Err(MultimodalEvidenceSurveillanceContractError::Invalid(
                "unknown or blocked multimodal contract cannot retain claims or cells".into(),
            ));
        }
        let expected_effect = if should_block {
            vec!["block:unsafe-release".to_string()]
        } else {
            vec![format!(
                "read:local-multimodal-contract:{}",
                self.request_id
            )]
        };
        if self.effect_receipts != expected_effect {
            return Err(MultimodalEvidenceSurveillanceContractError::Invalid(
                "multimodal contract effect does not match its release state".into(),
            ));
        }
        let expected_comparability = ContentHash::of_value(&json!({
            "study_order": self.study_order,
            "modality_order": self.modality_order,
            "semantic_profile": self.semantic_profile,
            "covered_cells": covered_cells,
        }))
        .map_err(|error| {
            MultimodalEvidenceSurveillanceContractError::Artifact(error.to_string())
        })?;
        if self.comparability_digest != expected_comparability {
            return Err(MultimodalEvidenceSurveillanceContractError::Invalid(
                "comparability digest does not match study×modality closure".into(),
            ));
        }
        let expected_contract = ContentHash::of_value(&json!({
            "input_schema": self.input_schema,
            "output_schema": self.output_schema,
            "compatibility": self.compatibility,
            "study_order": self.study_order,
            "modality_order": self.modality_order,
            "candidate_order": self.candidate_order,
        }))
        .map_err(|error| {
            MultimodalEvidenceSurveillanceContractError::Artifact(error.to_string())
        })?;
        if self.contract_digest != expected_contract {
            return Err(MultimodalEvidenceSurveillanceContractError::Invalid(
                "contract digest does not match schema and multimodal closure".into(),
            ));
        }
        let expected_canonical = ContentHash::of_value(&json!({
            "retained_order": self.retained_order,
            "unknown_order": self.unknown_order,
            "denied_order": self.denied_order,
            "incomparable_order": self.incomparable_order,
            "migration_order": self.migration_order,
            "semantic_loss": self.semantic_loss,
            "comparability_digest": self.comparability_digest,
        }))
        .map_err(|error| {
            MultimodalEvidenceSurveillanceContractError::Artifact(error.to_string())
        })?;
        if self.canonical_digest != expected_canonical {
            return Err(MultimodalEvidenceSurveillanceContractError::Invalid(
                "canonical digest does not match multimodal semantic-loss state".into(),
            ));
        }
        let expected_provenance = ContentHash::of_value(&json!({
            "request_id": self.request_id,
            "replay_identity": self.replay_identity,
            "comparability_digest": self.comparability_digest,
            "contract_digest": self.contract_digest,
            "canonical_digest": self.canonical_digest,
        }))
        .map_err(|error| {
            MultimodalEvidenceSurveillanceContractError::Artifact(error.to_string())
        })?;
        if self.provenance_digest != expected_provenance {
            return Err(MultimodalEvidenceSurveillanceContractError::Invalid(
                "provenance digest does not match multimodal contract identity".into(),
            ));
        }
        if self.artifact.artifact_id
            != format!("adapter-multimodal-evidence-contract:{}", self.request_id)
            || self.artifact.content_type
                != "application/vnd.aurora.multimodal-evidence-contract+json"
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(MultimodalEvidenceSurveillanceContractError::Artifact(
                "multimodal contract artifact is not bound to the receipt".into(),
            ));
        }
        let payload = json!({
            "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
            "contract_version": CONTRACT_VERSION,
            "feature_id": FEATURE_ID,
            "request_id": self.request_id,
            "input_schema": self.input_schema,
            "output_schema": self.output_schema,
            "semantic_profile": self.semantic_profile,
            "compatibility": self.compatibility,
            "policy_allow": self.policy_allow,
            "protected_closure": self.protected_closure,
            "disposition": self.disposition,
            "study_order": self.study_order,
            "modality_order": self.modality_order,
            "candidate_order": self.candidate_order,
            "retained_order": self.retained_order,
            "unknown_order": self.unknown_order,
            "denied_order": self.denied_order,
            "incomparable_order": self.incomparable_order,
            "migration_order": self.migration_order,
            "semantic_loss": self.semantic_loss,
            "comparability_digest": self.comparability_digest,
            "contract_digest": self.contract_digest,
            "canonical_digest": self.canonical_digest,
            "provenance_digest": self.provenance_digest,
            "replay_identity": self.replay_identity,
            "effect_receipts": self.effect_receipts,
            "raw_data_local": self.raw_data_local,
            "boundary": PRECLINICAL_BOUNDARY,
        });
        self.artifact.verify_payload(&payload).map_err(|error| {
            MultimodalEvidenceSurveillanceContractError::Artifact(error.to_string())
        })?;
        self.artifact.validate_metadata().map_err(|error| {
            MultimodalEvidenceSurveillanceContractError::Artifact(error.to_string())
        })?;
        if self.input_digest != contract_input_digest(&self.input)? {
            return Err(MultimodalEvidenceSurveillanceContractError::Invalid(
                "multimodal contract retained input digest mismatch".into(),
            ));
        }
        let expected = build_multimodal_evidence_surveillance_contract(&self.input)?;
        if self != &expected {
            return Err(MultimodalEvidenceSurveillanceContractError::Invalid(
                "multimodal contract receipt does not match its retained input".into(),
            ));
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, MultimodalEvidenceSurveillanceContractError> {
        self.validate()?;
        let value = serde_json::to_value(self).map_err(|error| {
            MultimodalEvidenceSurveillanceContractError::Artifact(error.to_string())
        })?;
        ContentHash::of_value(&value).map_err(|error| {
            MultimodalEvidenceSurveillanceContractError::Artifact(error.to_string())
        })
    }
}

pub fn multimodal_evidence_surveillance_contract_model_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "adapter".into(), consumers: ["AURORA extension developer".into(), "multimodal schema steward".into()].into(), behavior: "models EvidenceFeed2 into a comparable QualifiedEvidenceSet2 with semantic-profile and study×modality closure".into(), value: "prevents cross-study modality mismatch from being hidden by a typed contract boundary".into(), inputs: vec![TypedPort { name: "evidence_feed".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "qualified_evidence_set".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ReadLocalData, Effect::WriteLocalArtifact].into(), permissions: ["read:local-research-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "OME-NGFF".into(), state: EvidenceState::Supported, locator: Some("https://ngff.openmicroscopy.org/rfc/5/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn model_multimodal_evidence_surveillance_contract(
    request: &MultimodalEvidenceSurveillanceContractRequest,
) -> Result<
    MultimodalEvidenceSurveillanceContractReceipt,
    MultimodalEvidenceSurveillanceContractError,
> {
    let receipt = build_multimodal_evidence_surveillance_contract(request)?;
    receipt.validate()?;
    Ok(receipt)
}

fn build_multimodal_evidence_surveillance_contract(
    request: &MultimodalEvidenceSurveillanceContractRequest,
) -> Result<
    MultimodalEvidenceSurveillanceContractReceipt,
    MultimodalEvidenceSurveillanceContractError,
> {
    if request.request_id.trim().is_empty()
        || request.input_schema.trim().is_empty()
        || request.output_schema.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_studies.len() < 2
        || request.required_modalities.len() < 2
        || request.claims.is_empty()
        || request.replay_identity.as_str().len() != 64
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
    {
        return Err(MultimodalEvidenceSurveillanceContractError::Invalid("multimodal contract identity, schemas, semantic profile, study/modality closure, claims, replay, locality, or boundary is invalid".into()));
    }
    validate_text("request_id", &request.request_id)?;
    validate_text("input_schema", &request.input_schema)?;
    validate_text("output_schema", &request.output_schema)?;
    validate_text("semantic_profile", &request.semantic_profile)?;
    validate_text("boundary", &request.boundary)?;
    validate_digest("replay_identity", &request.replay_identity)?;
    validate_unique_strings("required_studies", &request.required_studies)?;
    validate_unique_strings("required_modalities", &request.required_modalities)?;
    if request.claims.len() > MAX_ITEMS {
        return Err(MultimodalEvidenceSurveillanceContractError::Invalid(
            "claims exceeds its item bound".into(),
        ));
    }
    let mut studies = request.required_studies.clone();
    studies.sort();
    let mut modalities = request.required_modalities.clone();
    modalities.sort();
    if studies.len() < 2 || modalities.len() < 2 {
        return Err(MultimodalEvidenceSurveillanceContractError::Invalid(
            "multimodal contract requires at least two unique studies and modalities".into(),
        ));
    }
    for claim in &request.claims {
        validate_text("claim.claim_id", &claim.claim_id)?;
        validate_text("claim.study_id", &claim.study_id)?;
        validate_text("claim.modality", &claim.modality)?;
        validate_text("claim.semantic_profile", &claim.semantic_profile)?;
        validate_digest("claim.value_digest", &claim.value_digest)?;
    }
    let mut claims = request.claims.clone();
    claims.sort_by(|left, right| left.claim_id.cmp(&right.claim_id));
    let claim_ids = claims
        .iter()
        .map(|claim| claim.claim_id.clone())
        .collect::<Vec<_>>();
    if claim_ids.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(MultimodalEvidenceSurveillanceContractError::Invalid(
            "multimodal claim identities must be unique".into(),
        ));
    }
    let compatibility =
        if request.input_schema == INPUT_SCHEMA && request.output_schema == OUTPUT_SCHEMA {
            MultimodalContractCompatibility::AdditiveMigration
        } else if request.input_schema == request.output_schema {
            MultimodalContractCompatibility::Compatible
        } else {
            MultimodalContractCompatibility::Breaking
        };
    let mut candidate_set = claim_ids.iter().cloned().collect::<BTreeSet<_>>();
    let required_cells = studies
        .iter()
        .flat_map(|study| {
            modalities
                .iter()
                .map(move |modality| required_cell(study, modality))
        })
        .collect::<BTreeSet<_>>();
    if claim_ids
        .iter()
        .any(|claim_id| required_cells.contains(claim_id))
    {
        return Err(MultimodalEvidenceSurveillanceContractError::Invalid(
            "claim identity collides with a required study×modality cell".into(),
        ));
    }
    candidate_set.extend(required_cells.iter().cloned());
    let candidate_order = candidate_set.into_iter().collect::<Vec<_>>();
    let mut retained = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut denied = BTreeSet::new();
    let mut incomparable = BTreeSet::new();
    let mut migration = BTreeSet::new();
    let mut loss = BTreeSet::new();
    let mut covered = BTreeSet::new();
    let global_release_blocked =
        !request.policy_allow || !request.protected_closure || !request.raw_data_local;
    for claim in &claims {
        if global_release_blocked {
            denied.insert(claim.claim_id.clone());
            loss.insert(format!("claim:{}:release-gate", claim.claim_id));
        } else if compatibility == MultimodalContractCompatibility::Breaking {
            denied.insert(claim.claim_id.clone());
            loss.insert(format!("claim:{}:breaking-schema", claim.claim_id));
        } else if claim.study_id.trim().is_empty()
            || claim.modality.trim().is_empty()
            || !studies.contains(&claim.study_id)
            || !modalities.contains(&claim.modality)
        {
            denied.insert(claim.claim_id.clone());
            loss.insert(format!("claim:{}:scope-mismatch", claim.claim_id));
        } else if claim.semantic_profile != request.semantic_profile {
            denied.insert(claim.claim_id.clone());
            incomparable.insert(claim.claim_id.clone());
            loss.insert(format!(
                "claim:{}:semantic-profile-incomparable",
                claim.claim_id
            ));
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
            covered.insert(required_cell(&claim.study_id, &claim.modality));
            if compatibility == MultimodalContractCompatibility::AdditiveMigration {
                migration.insert(format!("claim:{}:study-modality-preserved", claim.claim_id));
            }
            if claim.negative_result {
                loss.insert(format!("claim:{}:negative-result-retained", claim.claim_id));
            }
        }
    }
    for cell in required_cells {
        if !covered.contains(&cell) {
            unknown.insert(cell.clone());
            loss.insert(format!("cell:{}:comparability-incomplete", cell));
        } else {
            retained.insert(cell);
        }
    }
    if !request.policy_allow {
        loss.insert("control:policy-denied".into());
    }
    if !request.protected_closure {
        loss.insert("control:protected-closure-incomplete".into());
    }
    let disposition = if global_release_blocked {
        MultimodalContractDisposition::Blocked
    } else if retained.is_empty() {
        MultimodalContractDisposition::Unknown
    } else if compatibility == MultimodalContractCompatibility::Breaking
        || !unknown.is_empty()
        || !denied.is_empty()
    {
        MultimodalContractDisposition::Partial
    } else {
        MultimodalContractDisposition::Compatible
    };
    let retained_order = retained.iter().cloned().collect::<Vec<_>>();
    let unknown_order = unknown.iter().cloned().collect::<Vec<_>>();
    let denied_order = denied.iter().cloned().collect::<Vec<_>>();
    let incomparable_order = incomparable.iter().cloned().collect::<Vec<_>>();
    let migration_order = migration.iter().cloned().collect::<Vec<_>>();
    let semantic_loss = loss.iter().cloned().collect::<Vec<_>>();
    let comparability_digest = ContentHash::of_value(&json!({
        "study_order": studies.clone(),
        "modality_order": modalities.clone(),
        "semantic_profile": request.semantic_profile,
        "covered_cells": covered.clone(),
    }))
    .map_err(|error| MultimodalEvidenceSurveillanceContractError::Artifact(error.to_string()))?;
    let contract_digest = ContentHash::of_value(&json!({
        "input_schema": request.input_schema,
        "output_schema": request.output_schema,
        "compatibility": compatibility,
        "study_order": studies.clone(),
        "modality_order": modalities.clone(),
        "candidate_order": candidate_order.clone(),
    }))
    .map_err(|error| MultimodalEvidenceSurveillanceContractError::Artifact(error.to_string()))?;
    let canonical_digest = ContentHash::of_value(&json!({
        "retained_order": retained_order.clone(),
        "unknown_order": unknown_order.clone(),
        "denied_order": denied_order.clone(),
        "incomparable_order": incomparable_order.clone(),
        "migration_order": migration_order.clone(),
        "semantic_loss": semantic_loss.clone(),
        "comparability_digest": comparability_digest.clone(),
    }))
    .map_err(|error| MultimodalEvidenceSurveillanceContractError::Artifact(error.to_string()))?;
    let provenance_digest = ContentHash::of_value(&json!({
        "request_id": request.request_id,
        "replay_identity": request.replay_identity,
        "comparability_digest": comparability_digest,
        "contract_digest": contract_digest,
        "canonical_digest": canonical_digest,
    }))
    .map_err(|error| MultimodalEvidenceSurveillanceContractError::Artifact(error.to_string()))?;
    let effect_receipts = if disposition == MultimodalContractDisposition::Blocked {
        vec!["block:unsafe-release".into()]
    } else {
        vec![format!(
            "read:local-multimodal-contract:{}",
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
        "semantic_profile": request.semantic_profile,
        "compatibility": compatibility,
        "policy_allow": request.policy_allow,
        "protected_closure": request.protected_closure,
        "disposition": disposition,
        "study_order": studies,
        "modality_order": modalities,
        "candidate_order": candidate_order,
        "retained_order": retained_order,
        "unknown_order": unknown_order,
        "denied_order": denied_order,
        "incomparable_order": incomparable_order,
        "migration_order": migration_order,
        "semantic_loss": semantic_loss,
        "comparability_digest": comparability_digest,
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
            "adapter-multimodal-evidence-contract:{}",
            request.request_id
        ),
        "application/vnd.aurora.multimodal-evidence-contract+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| MultimodalEvidenceSurveillanceContractError::Artifact(error.to_string()))?;
    let canonical_request = canonical_multimodal_evidence_surveillance_request(request);
    let receipt = MultimodalEvidenceSurveillanceContractReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        input: canonical_request,
        input_digest: contract_input_digest(request)?,
        request_id: request.request_id.clone(),
        input_schema: request.input_schema.clone(),
        output_schema: request.output_schema.clone(),
        semantic_profile: request.semantic_profile.clone(),
        compatibility,
        policy_allow: request.policy_allow,
        protected_closure: request.protected_closure,
        disposition,
        study_order: studies,
        modality_order: modalities,
        candidate_order,
        retained_order,
        unknown_order,
        denied_order,
        incomparable_order,
        migration_order,
        semantic_loss,
        comparability_digest,
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
    fn request() -> MultimodalEvidenceSurveillanceContractRequest {
        let digest = hash("multimodal-contract");
        let claim = |id: &str, study: &str, modality: &str, profile: &str, state: EvidenceState| {
            MultimodalContractClaim {
                claim_id: id.into(),
                study_id: study.into(),
                modality: modality.into(),
                semantic_profile: profile.into(),
                value_digest: digest.clone(),
                evidence_state: state,
                omitted: false,
                negative_result: false,
            }
        };
        MultimodalEvidenceSurveillanceContractRequest {
            request_id: "request:mm-contract".into(),
            input_schema: INPUT_SCHEMA.into(),
            output_schema: OUTPUT_SCHEMA.into(),
            semantic_profile: "profile:v1".into(),
            required_studies: vec!["study:a".into(), "study:b".into()],
            required_modalities: vec!["imaging".into(), "omics".into()],
            claims: vec![
                claim(
                    "a:image",
                    "study:a",
                    "imaging",
                    "profile:v1",
                    EvidenceState::Supported,
                ),
                claim(
                    "a:omics",
                    "study:a",
                    "omics",
                    "profile:v1",
                    EvidenceState::Supported,
                ),
                claim(
                    "b:image",
                    "study:b",
                    "imaging",
                    "profile:v1",
                    EvidenceState::Supported,
                ),
                claim(
                    "b:omics",
                    "study:b",
                    "omics",
                    "profile:v1",
                    EvidenceState::Supported,
                ),
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
            multimodal_evidence_surveillance_contract_model_manifest().autonomy_tier,
            AutonomyTier::A1
        );
    }
    #[test]
    fn comparable_cells_complete() {
        assert_eq!(
            model_multimodal_evidence_surveillance_contract(&request())
                .unwrap()
                .disposition,
            MultimodalContractDisposition::Compatible
        );
    }
    #[test]
    fn missing_cell_is_unknown() {
        let mut value = request();
        value.claims.pop();
        let receipt = model_multimodal_evidence_surveillance_contract(&value).unwrap();
        assert!(receipt
            .unknown_order
            .iter()
            .any(|item| item.contains("study:b::omics::required")));
    }
    #[test]
    fn semantic_mismatch_is_incomparable() {
        let mut value = request();
        value.claims[0].semantic_profile = "profile:other".into();
        let receipt = model_multimodal_evidence_surveillance_contract(&value).unwrap();
        assert!(receipt.incomparable_order.contains(&"a:image".to_string()));
    }
    #[test]
    fn unknown_is_preserved() {
        let mut value = request();
        value.claims[0].evidence_state = EvidenceState::Unknown;
        assert!(model_multimodal_evidence_surveillance_contract(&value)
            .unwrap()
            .semantic_loss
            .iter()
            .any(|item| item.contains("unknown-not-asserted")));
    }
    #[test]
    fn policy_blocks() {
        let mut value = request();
        value.policy_allow = false;
        let receipt = model_multimodal_evidence_surveillance_contract(&value).unwrap();
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
        assert!(receipt.retained_order.is_empty());
        assert_eq!(receipt.disposition, MultimodalContractDisposition::Blocked);
    }
    #[test]
    fn breaking_schema_preserves_requested_pair() {
        let mut value = request();
        value.input_schema = "EvidenceFeed9@1".into();
        let receipt = model_multimodal_evidence_surveillance_contract(&value).unwrap();
        assert_eq!(receipt.input_schema, "EvidenceFeed9@1");
        assert_eq!(
            receipt.compatibility,
            MultimodalContractCompatibility::Breaking
        );
    }
    #[test]
    fn duplicate_required_study_is_rejected() {
        let mut value = request();
        value.required_studies.push("study:a".into());
        assert!(model_multimodal_evidence_surveillance_contract(&value).is_err());
    }
    #[test]
    fn tampered_comparability_digest_is_rejected() {
        let mut receipt = model_multimodal_evidence_surveillance_contract(&request()).unwrap();
        receipt.comparability_digest = hash("tampered-comparability");
        assert!(receipt.validate().is_err());
    }
    #[test]
    fn tampered_artifact_payload_is_rejected() {
        let mut receipt = model_multimodal_evidence_surveillance_contract(&request()).unwrap();
        receipt.artifact.content_hash = hash("tampered-payload");
        assert!(receipt.validate().is_err());
    }
    #[test]
    fn digest_is_stable() {
        let first = model_multimodal_evidence_surveillance_contract(&request()).unwrap();
        let second = model_multimodal_evidence_surveillance_contract(&request()).unwrap();
        assert_eq!(first.canonical_digest, second.canonical_digest);
    }

    #[test]
    fn reordered_dimensions_share_the_same_retained_input_identity() {
        let mut reordered = request();
        reordered.required_studies.reverse();
        reordered.required_modalities.reverse();
        reordered.claims.reverse();
        let first = model_multimodal_evidence_surveillance_contract(&request()).unwrap();
        let second = model_multimodal_evidence_surveillance_contract(&reordered).unwrap();
        assert_eq!(first.input_digest, second.input_digest);
        assert_eq!(first.digest().unwrap(), second.digest().unwrap());
    }

    #[test]
    fn receipt_rejects_tampered_retained_claim() {
        let mut receipt = model_multimodal_evidence_surveillance_contract(&request()).unwrap();
        receipt.input.claims[0].modality = "tampered-modality".into();
        let error = receipt.validate().unwrap_err();
        assert!(error.to_string().contains("retained input digest mismatch"));
    }
}
