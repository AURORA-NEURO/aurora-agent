//! Adapter contract frontier capability-manifest gateway.
//!
//! Atlas feature: `AFA-adapter-P25-F22`.
//!
//! This boundary turns a versioned adapter contract declaration into a canonical capability
//! manifest suitable for MCP, HTTP, event, and SDK negotiation. It exchanges metadata and
//! content hashes only. Contract drift, missing comparability, protected-closure gaps, and
//! denied effects remain explicit and cannot become an interoperability pass.

use bioprism_foundation::{
    LossSeverity, ProvenanceLink, SemanticLoss, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P25-F22";
pub const CONTRACT_VERSION: &str = "adapter-contract-frontier/1.0";
pub const CURRENT_CONTRACT_VERSION: &str = "2.0.0";
pub const COMPATIBLE_CONTRACT_VERSION: &str = "1.0.0";
const MAX_TEXT_BYTES: usize = 512;
const MAX_VERSIONS: usize = 32;
const MAX_ITEMS: usize = 8192;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterContractInput {
    pub adapter_id: String,
    pub capability_id: String,
    pub source_contract_version: String,
    pub supported_contract_versions: Vec<String>,
    pub input_schema: String,
    pub output_schema: String,
    pub effects: Vec<String>,
    pub permissions: Vec<String>,
    pub modality_order: Vec<String>,
    pub artifact_digests: Vec<ContentHash>,
    pub comparability_profile: String,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ManifestDisposition {
    Accepted,
    Migrated,
    ApprovalRequired,
    Blocked,
    Incompatible,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterCapabilityManifest {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub adapter_id: String,
    pub capability_id: String,
    pub source_contract_version: String,
    pub supported_contract_versions: Vec<String>,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub negotiated_version: String,
    pub disposition: ManifestDisposition,
    pub input_schema: String,
    pub output_schema: String,
    pub comparability_profile: String,
    pub modality_order: Vec<String>,
    pub effect_order: Vec<String>,
    pub permission_order: Vec<String>,
    pub artifact_digest_order: Vec<ContentHash>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub semantic_loss: Vec<String>,
    pub checks: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

impl AdapterCapabilityManifest {
    pub fn validate(&self) -> Result<(), ContractFrontierError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
        {
            return Err(ContractFrontierError::Contract(
                "adapter contract frontier identity mismatch".into(),
            ));
        }
        if self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.modality_order.is_empty()
            || self.artifact_digest_order.is_empty()
            || self.supported_contract_versions.is_empty()
            || self.checks.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(ContractFrontierError::InvalidRequest(
                "manifest identity, schemas, modalities, checks, effects, locality, and boundary are required".into(),
            ));
        }
        for (field, value) in [
            ("adapter_id", self.adapter_id.as_str()),
            ("capability_id", self.capability_id.as_str()),
            (
                "source_contract_version",
                self.source_contract_version.as_str(),
            ),
            ("negotiated_version", self.negotiated_version.as_str()),
            ("input_schema", self.input_schema.as_str()),
            ("output_schema", self.output_schema.as_str()),
            ("comparability_profile", self.comparability_profile.as_str()),
            ("boundary", self.boundary.as_str()),
        ] {
            validate_text(field, value)?;
        }
        for (field, values) in [
            (
                "supported_contract_versions",
                &self.supported_contract_versions,
            ),
            ("modality_order", &self.modality_order),
            ("effect_order", &self.effect_order),
            ("permission_order", &self.permission_order),
            ("semantic_loss", &self.semantic_loss),
            ("omissions", &self.omissions),
            ("uncertainty", &self.uncertainty),
            ("checks", &self.checks),
            ("effect_receipts", &self.effect_receipts),
        ] {
            validate_sorted_strings(field, values)?;
        }
        if self.artifact_digest_order.len() > MAX_ITEMS
            || self
                .artifact_digest_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err(ContractFrontierError::InvalidRequest(
                "manifest artifact ordering is not canonical".into(),
            ));
        }
        let expected_effect = match self.disposition {
            ManifestDisposition::Accepted | ManifestDisposition::Migrated => {
                "exchange:permitted-capability-manifest-and-digests"
            }
            ManifestDisposition::ApprovalRequired => "block:manifest-exchange:approval-required",
            ManifestDisposition::Blocked => "block:manifest-exchange:blocked",
            ManifestDisposition::Incompatible => "block:incompatible-contract",
            ManifestDisposition::Unknown => "block:manifest-exchange:unknown",
        };
        if self.effect_receipts != vec![expected_effect.to_string()] {
            return Err(ContractFrontierError::InvalidRequest(
                "manifest effect does not match its disposition".into(),
            ));
        }
        if matches!(self.disposition, ManifestDisposition::Accepted)
            && !self.semantic_loss.is_empty()
        {
            return Err(ContractFrontierError::InvalidRequest(
                "accepted manifest cannot contain semantic loss".into(),
            ));
        }
        if matches!(self.disposition, ManifestDisposition::Migrated)
            && self.semantic_loss != vec!["legacy_fields:unknown".to_string()]
        {
            return Err(ContractFrontierError::InvalidRequest(
                "migrated manifest requires its legacy-field loss receipt".into(),
            ));
        }
        let input = AdapterContractInput {
            adapter_id: self.adapter_id.clone(),
            capability_id: self.capability_id.clone(),
            source_contract_version: self.source_contract_version.clone(),
            supported_contract_versions: self.supported_contract_versions.clone(),
            input_schema: self.input_schema.clone(),
            output_schema: self.output_schema.clone(),
            effects: self.effect_order.clone(),
            permissions: self.permission_order.clone(),
            modality_order: self.modality_order.clone(),
            artifact_digests: self.artifact_digest_order.clone(),
            comparability_profile: self.comparability_profile.clone(),
            policy_allow: self.policy_allow,
            protected_closure: self.protected_closure,
            raw_data_local: self.raw_data_local,
            boundary: self.boundary.clone(),
        };
        let expected = compile_adapter_capability_manifest_internal(&input, false)?;
        if self != &expected {
            return Err(ContractFrontierError::Contract(
                "capability manifest is not derived from its retained contract declaration".into(),
            ));
        }
        if self.artifact.artifact_id != format!("adapter-capability-manifest:{}", self.adapter_id)
            || self.artifact.content_type
                != "application/vnd.aurora.adapter-capability-manifest+json"
            || self.artifact.semantic_loss != typed_semantic_loss(&self.semantic_loss)
            || self.artifact.provenance != manifest_provenance(&self.artifact_digest_order)
        {
            return Err(ContractFrontierError::Contract(
                "capability manifest artifact is not bound to the manifest".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| ContractFrontierError::Contract(error.to_string()))?;
        self.artifact
            .verify_payload(&manifest_payload(self))
            .map_err(|error| ContractFrontierError::Contract(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, ContractFrontierError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| ContractFrontierError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| ContractFrontierError::Serialization(error.to_string()))
    }
}

fn validate_text(field: &str, value: &str) -> Result<(), ContractFrontierError> {
    if value.is_empty() || value.trim() != value {
        return Err(ContractFrontierError::InvalidRequest(format!(
            "{field} must be non-empty and trimmed"
        )));
    }
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(ContractFrontierError::InvalidRequest(format!(
            "{field} is outside its bounded text contract"
        )));
    }
    Ok(())
}

fn validate_input_strings(
    field: &str,
    values: &[String],
    max_items: usize,
) -> Result<(), ContractFrontierError> {
    if values.len() > max_items {
        return Err(ContractFrontierError::InvalidRequest(format!(
            "{field} exceeds its item bound"
        )));
    }
    let mut unique = BTreeSet::new();
    for value in values {
        validate_text(field, value)?;
        if !unique.insert(value) {
            return Err(ContractFrontierError::InvalidRequest(format!(
                "{field} contains duplicate values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_strings(field: &str, values: &[String]) -> Result<(), ContractFrontierError> {
    validate_input_strings(field, values, MAX_ITEMS)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ContractFrontierError::InvalidRequest(format!(
            "{field} ordering is not canonical"
        )));
    }
    Ok(())
}

fn typed_semantic_loss(values: &[String]) -> Vec<SemanticLoss> {
    values
        .iter()
        .map(|field| SemanticLoss {
            field: field.clone(),
            reason: "contract-frontier migration semantics are not inferred".into(),
            severity: LossSeverity::Unknown,
        })
        .collect()
}

fn manifest_provenance(digests: &[ContentHash]) -> Vec<ProvenanceLink> {
    digests
        .iter()
        .enumerate()
        .map(|(index, digest)| ProvenanceLink {
            source_id: format!("input-artifact:{index}"),
            relation: "capability-manifest-input-digest".into(),
            digest: digest.clone(),
        })
        .collect()
}

fn manifest_payload(manifest: &AdapterCapabilityManifest) -> serde_json::Value {
    manifest_payload_from_parts(
        &manifest.schema_version,
        &manifest.contract_version,
        &manifest.feature_id,
        &manifest.adapter_id,
        &manifest.capability_id,
        &manifest.source_contract_version,
        &manifest.supported_contract_versions,
        manifest.policy_allow,
        manifest.protected_closure,
        &manifest.negotiated_version,
        manifest.disposition,
        &manifest.input_schema,
        &manifest.output_schema,
        &manifest.comparability_profile,
        &manifest.modality_order,
        &manifest.effect_order,
        &manifest.permission_order,
        &manifest.artifact_digest_order,
        &manifest.omissions,
        &manifest.uncertainty,
        &manifest.semantic_loss,
        &manifest.checks,
        &manifest.effect_receipts,
        &manifest.artifact.provenance,
        manifest.raw_data_local,
        &manifest.boundary,
    )
}

#[allow(clippy::too_many_arguments)]
fn manifest_payload_from_parts(
    schema_version: &str,
    contract_version: &str,
    feature_id: &str,
    adapter_id: &str,
    capability_id: &str,
    source_contract_version: &str,
    supported_contract_versions: &[String],
    policy_allow: bool,
    protected_closure: bool,
    negotiated_version: &str,
    disposition: ManifestDisposition,
    input_schema: &str,
    output_schema: &str,
    comparability_profile: &str,
    modality_order: &[String],
    effect_order: &[String],
    permission_order: &[String],
    artifact_digest_order: &[ContentHash],
    omissions: &[String],
    uncertainty: &[String],
    semantic_loss: &[String],
    checks: &[String],
    effect_receipts: &[String],
    provenance: &[ProvenanceLink],
    raw_data_local: bool,
    boundary: &str,
) -> serde_json::Value {
    json!({
        "schema_version": schema_version,
        "contract_version": contract_version,
        "feature_id": feature_id,
        "adapter_id": adapter_id,
        "capability_id": capability_id,
        "source_contract_version": source_contract_version,
        "supported_contract_versions": supported_contract_versions,
        "policy_allow": policy_allow,
        "protected_closure": protected_closure,
        "negotiated_version": negotiated_version,
        "disposition": disposition,
        "input_schema": input_schema,
        "output_schema": output_schema,
        "comparability_profile": comparability_profile,
        "modality_order": modality_order,
        "effect_order": effect_order,
        "permission_order": permission_order,
        "artifact_digest_order": artifact_digest_order,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "semantic_loss": semantic_loss,
        "checks": checks,
        "effect_receipts": effect_receipts,
        "provenance": provenance,
        "raw_data_local": raw_data_local,
        "boundary": boundary,
    })
}

#[derive(Debug, Error)]
pub enum ContractFrontierError {
    #[error("invalid adapter contract frontier input: {0}")]
    InvalidRequest(String),
    #[error("adapter contract frontier contract rejected: {0}")]
    Contract(String),
    #[error("adapter contract frontier serialization failed: {0}")]
    Serialization(String),
}

pub fn compile_adapter_capability_manifest(
    input: &AdapterContractInput,
) -> Result<AdapterCapabilityManifest, ContractFrontierError> {
    compile_adapter_capability_manifest_internal(input, true)
}

fn compile_adapter_capability_manifest_internal(
    input: &AdapterContractInput,
    validate_output: bool,
) -> Result<AdapterCapabilityManifest, ContractFrontierError> {
    validate_input(input)?;
    let mut supported_versions = input.supported_contract_versions.clone();
    supported_versions.sort();
    let mut modalities = input.modality_order.clone();
    modalities.sort();
    modalities.dedup();
    let mut effects = input.effects.clone();
    effects.sort();
    effects.dedup();
    let mut permissions = input.permissions.clone();
    permissions.sort();
    permissions.dedup();
    let mut artifacts = input.artifact_digests.clone();
    artifacts.sort();
    artifacts.dedup();
    let advertises_current = input
        .supported_contract_versions
        .iter()
        .any(|value| value == CURRENT_CONTRACT_VERSION);
    let (negotiated_version, version_disposition) = if !advertises_current {
        (
            CURRENT_CONTRACT_VERSION.into(),
            ManifestDisposition::Incompatible,
        )
    } else if input.source_contract_version == CURRENT_CONTRACT_VERSION {
        (
            CURRENT_CONTRACT_VERSION.into(),
            ManifestDisposition::Accepted,
        )
    } else if input.source_contract_version == COMPATIBLE_CONTRACT_VERSION {
        (
            CURRENT_CONTRACT_VERSION.into(),
            ManifestDisposition::Migrated,
        )
    } else {
        (
            input.source_contract_version.clone(),
            ManifestDisposition::Incompatible,
        )
    };
    let mut omissions = Vec::new();
    let mut uncertainty = Vec::new();
    let mut semantic_loss = Vec::new();
    let mut checks = vec![
        "input and output schemas are pinned in the manifest".into(),
        "effect and permission names canonicalized".into(),
        "multimodal artifact digests remain local-addressed".into(),
    ];
    if version_disposition == ManifestDisposition::Migrated {
        omissions.push("legacy contract fields remain unknown after additive migration".into());
        uncertainty.push("semantic parity for omitted legacy fields is unmeasured".into());
        semantic_loss.push("legacy_fields:unknown".into());
        checks.push("migration requires an explicit compatibility fixture".into());
    }
    checks.sort();
    semantic_loss.sort();
    let disposition = if !input.policy_allow {
        omissions.push("policy denied capability exchange".into());
        ManifestDisposition::Blocked
    } else if !input.protected_closure {
        omissions.push("protected closure is incomplete".into());
        uncertainty.push("manifest cannot be promoted without protected closure".into());
        ManifestDisposition::ApprovalRequired
    } else if input.comparability_profile == "unknown" {
        uncertainty.push("multimodal comparability profile is unknown".into());
        ManifestDisposition::Unknown
    } else {
        version_disposition
    };
    omissions.sort();
    uncertainty.sort();
    let effect_receipts = vec![match disposition {
        ManifestDisposition::Accepted | ManifestDisposition::Migrated => {
            "exchange:permitted-capability-manifest-and-digests"
        }
        ManifestDisposition::ApprovalRequired => "block:manifest-exchange:approval-required",
        ManifestDisposition::Blocked => "block:manifest-exchange:blocked",
        ManifestDisposition::Incompatible => "block:incompatible-contract",
        ManifestDisposition::Unknown => "block:manifest-exchange:unknown",
    }
    .into()];
    let provenance = manifest_provenance(&artifacts);
    let payload = manifest_payload_from_parts(
        RESEARCH_CONTRACT_SCHEMA_VERSION,
        CONTRACT_VERSION,
        FEATURE_ID,
        &input.adapter_id,
        &input.capability_id,
        &input.source_contract_version,
        &supported_versions,
        input.policy_allow,
        input.protected_closure,
        &negotiated_version,
        disposition,
        &input.input_schema,
        &input.output_schema,
        &input.comparability_profile,
        &modalities,
        &effects,
        &permissions,
        &artifacts,
        &omissions,
        &uncertainty,
        &semantic_loss,
        &checks,
        &effect_receipts,
        &provenance,
        true,
        PRECLINICAL_BOUNDARY,
    );
    let artifact = TypedResearchArtifact::from_payload(
        format!("adapter-capability-manifest:{}", input.adapter_id),
        "application/vnd.aurora.adapter-capability-manifest+json",
        &payload,
        typed_semantic_loss(&semantic_loss),
        provenance,
    )
    .map_err(|error| ContractFrontierError::Contract(error.to_string()))?;
    let result = AdapterCapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        adapter_id: input.adapter_id.clone(),
        capability_id: input.capability_id.clone(),
        source_contract_version: input.source_contract_version.clone(),
        supported_contract_versions: supported_versions,
        policy_allow: input.policy_allow,
        protected_closure: input.protected_closure,
        negotiated_version,
        disposition,
        input_schema: input.input_schema.clone(),
        output_schema: input.output_schema.clone(),
        comparability_profile: input.comparability_profile.clone(),
        modality_order: modalities,
        effect_order: effects,
        permission_order: permissions,
        artifact_digest_order: artifacts,
        omissions,
        uncertainty,
        semantic_loss,
        checks,
        effect_receipts,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    if validate_output {
        result.validate()?;
    }
    Ok(result)
}

fn validate_input(input: &AdapterContractInput) -> Result<(), ContractFrontierError> {
    if input.adapter_id.trim().is_empty()
        || input.capability_id.trim().is_empty()
        || input.source_contract_version.trim().is_empty()
        || input.supported_contract_versions.is_empty()
        || input.input_schema.trim().is_empty()
        || input.output_schema.trim().is_empty()
        || input.modality_order.is_empty()
        || input.artifact_digests.is_empty()
        || input.comparability_profile.trim().is_empty()
        || !input.raw_data_local
        || input.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(ContractFrontierError::InvalidRequest(
            "adapter, capability, versions, schemas, modalities, artifacts, comparability, locality, and boundary are required".into(),
        ));
    }
    for (field, value) in [
        ("adapter_id", input.adapter_id.as_str()),
        ("capability_id", input.capability_id.as_str()),
        (
            "source_contract_version",
            input.source_contract_version.as_str(),
        ),
        ("input_schema", input.input_schema.as_str()),
        ("output_schema", input.output_schema.as_str()),
        (
            "comparability_profile",
            input.comparability_profile.as_str(),
        ),
        ("boundary", input.boundary.as_str()),
    ] {
        validate_text(field, value)?;
    }
    validate_input_strings(
        "supported_contract_versions",
        &input.supported_contract_versions,
        MAX_VERSIONS,
    )?;
    if !input
        .supported_contract_versions
        .iter()
        .any(|value| value == &input.source_contract_version)
    {
        return Err(ContractFrontierError::InvalidRequest(
            "supported versions must include the source contract version".into(),
        ));
    }
    for (field, values) in [
        ("modality_order", &input.modality_order),
        ("effects", &input.effects),
        ("permissions", &input.permissions),
    ] {
        validate_input_strings(field, values, MAX_ITEMS)?;
    }
    if input.artifact_digests.len() > MAX_ITEMS
        || input
            .artifact_digests
            .iter()
            .any(|digest| *digest == ContentHash::of_bytes(b""))
    {
        return Err(ContractFrontierError::InvalidRequest(
            "artifact digests are outside their bounded contract".into(),
        ));
    }
    let mut digests = input.artifact_digests.clone();
    digests.sort();
    digests.dedup();
    if digests.len() != input.artifact_digests.len() {
        return Err(ContractFrontierError::InvalidRequest(
            "artifact digests cannot contain duplicates".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn input() -> AdapterContractInput {
        AdapterContractInput {
            adapter_id: "adapter:multimodal".into(),
            capability_id: "capability:harmonize".into(),
            source_contract_version: CURRENT_CONTRACT_VERSION.into(),
            supported_contract_versions: vec![CURRENT_CONTRACT_VERSION.into()],
            input_schema: "AdapterContractInput2".into(),
            output_schema: "AdapterCapabilityManifest6".into(),
            effects: vec!["exchange:permitted-artifacts".into()],
            permissions: vec!["connect:approved-endpoints".into()],
            modality_order: vec!["omics".into(), "imaging".into()],
            artifact_digests: vec![
                ContentHash::of_bytes(b"omics"),
                ContentHash::of_bytes(b"imaging"),
            ],
            comparability_profile: "explicit-multimodal-v1".into(),
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_deterministic_and_canonical() {
        let first = compile_adapter_capability_manifest(&input()).unwrap();
        let second = compile_adapter_capability_manifest(&input()).unwrap();
        assert_eq!(first.digest().unwrap(), second.digest().unwrap());
        assert_eq!(first.disposition, ManifestDisposition::Accepted);
    }
    #[test]
    fn legacy_contract_requires_migration_loss() {
        let mut input = input();
        input.source_contract_version = COMPATIBLE_CONTRACT_VERSION.into();
        input.supported_contract_versions = vec![
            COMPATIBLE_CONTRACT_VERSION.into(),
            CURRENT_CONTRACT_VERSION.into(),
        ];
        let result = compile_adapter_capability_manifest(&input).unwrap();
        assert_eq!(result.disposition, ManifestDisposition::Migrated);
        assert!(!result.semantic_loss.is_empty());
    }
    #[test]
    fn incomplete_closure_requires_approval() {
        let mut input = input();
        input.protected_closure = false;
        let result = compile_adapter_capability_manifest(&input).unwrap();
        assert_eq!(result.disposition, ManifestDisposition::ApprovalRequired);
    }
    #[test]
    fn unknown_comparability_is_not_pass() {
        let mut input = input();
        input.comparability_profile = "unknown".into();
        let result = compile_adapter_capability_manifest(&input).unwrap();
        assert_eq!(result.disposition, ManifestDisposition::Unknown);
    }
    #[test]
    fn denied_policy_blocks_manifest_exchange() {
        let mut input = input();
        input.policy_allow = false;
        let result = compile_adapter_capability_manifest(&input).unwrap();
        assert_eq!(result.disposition, ManifestDisposition::Blocked);
    }

    #[test]
    fn unsupported_version_has_one_terminal_block_effect() {
        let mut value = input();
        value.source_contract_version = "3.0.0".into();
        value.supported_contract_versions = vec!["3.0.0".into()];
        let result = compile_adapter_capability_manifest(&value).unwrap();
        assert_eq!(result.disposition, ManifestDisposition::Incompatible);
        assert_eq!(result.effect_receipts, vec!["block:incompatible-contract"]);
    }

    #[test]
    fn comparability_profile_is_preserved_in_the_manifest() {
        let result = compile_adapter_capability_manifest(&input()).unwrap();
        assert_eq!(result.comparability_profile, "explicit-multimodal-v1");
    }

    #[test]
    fn duplicate_contract_declaration_is_rejected() {
        let mut value = input();
        value.effects.push(value.effects[0].clone());
        assert!(compile_adapter_capability_manifest(&value).is_err());
    }

    #[test]
    fn empty_artifact_digest_is_rejected() {
        let mut value = input();
        value.artifact_digests[0] = ContentHash::of_bytes(b"");
        assert!(compile_adapter_capability_manifest(&value).is_err());
    }

    #[test]
    fn forged_manifest_effect_is_rejected() {
        let mut manifest = compile_adapter_capability_manifest(&input()).unwrap();
        manifest.effect_receipts = vec!["block:incompatible-contract".into()];
        assert!(manifest.validate().is_err());
    }

    #[test]
    fn forged_policy_state_is_rejected() {
        let mut manifest = compile_adapter_capability_manifest(&input()).unwrap();
        manifest.policy_allow = false;
        assert!(manifest.validate().is_err());
    }

    #[test]
    fn manifest_artifact_provenance_tampering_is_rejected() {
        let mut manifest = compile_adapter_capability_manifest(&input()).unwrap();
        manifest.artifact.provenance[0].digest = ContentHash::of_bytes(b"tampered");
        assert!(manifest.validate().is_err());
    }

    #[test]
    fn contract_declaration_order_is_canonicalized() {
        let mut canonical_input = input();
        canonical_input.supported_contract_versions = vec![
            COMPATIBLE_CONTRACT_VERSION.into(),
            CURRENT_CONTRACT_VERSION.into(),
        ];
        let mut reordered = input();
        reordered.supported_contract_versions = vec![
            CURRENT_CONTRACT_VERSION.into(),
            COMPATIBLE_CONTRACT_VERSION.into(),
        ];
        let canonical = compile_adapter_capability_manifest(&canonical_input).unwrap();
        let reordered = compile_adapter_capability_manifest(&reordered).unwrap();
        assert_eq!(canonical.digest().unwrap(), reordered.digest().unwrap());
    }
}
