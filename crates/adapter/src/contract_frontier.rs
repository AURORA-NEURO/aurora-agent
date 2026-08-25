//! Adapter contract frontier capability-manifest gateway.
//!
//! Atlas feature: `AFA-adapter-P25-F22`.
//!
//! This boundary turns a versioned adapter contract declaration into a canonical capability
//! manifest suitable for MCP, HTTP, event, and SDK negotiation. It exchanges metadata and
//! content hashes only. Contract drift, missing comparability, protected-closure gaps, and
//! denied effects remain explicit and cannot become an interoperability pass.

use bioprism_foundation::{
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P25-F22";
pub const CONTRACT_VERSION: &str = "adapter-contract-frontier/1.0";
pub const CURRENT_CONTRACT_VERSION: &str = "2.0.0";
pub const COMPATIBLE_CONTRACT_VERSION: &str = "1.0.0";

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
    pub negotiated_version: String,
    pub disposition: ManifestDisposition,
    pub input_schema: String,
    pub output_schema: String,
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
            || self.adapter_id.trim().is_empty()
            || self.capability_id.trim().is_empty()
            || self.negotiated_version.trim().is_empty()
            || self.input_schema.trim().is_empty()
            || self.output_schema.trim().is_empty()
            || self.modality_order.is_empty()
            || self.checks.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(ContractFrontierError::InvalidRequest("manifest identity, schemas, modalities, checks, effects, locality, and boundary are required".into()));
        }
        for values in [
            &self.modality_order,
            &self.effect_order,
            &self.permission_order,
            &self.semantic_loss,
            &self.checks,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(ContractFrontierError::InvalidRequest(
                    "manifest ordering is not canonical".into(),
                ));
            }
        }
        if self
            .artifact_digest_order
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(ContractFrontierError::InvalidRequest(
                "manifest artifact ordering is not canonical".into(),
            ));
        }
        self.artifact
            .validate_metadata()
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
    validate_input(input)?;
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
    let (negotiated_version, version_disposition) =
        if input.source_contract_version == CURRENT_CONTRACT_VERSION {
            (
                CURRENT_CONTRACT_VERSION.into(),
                ManifestDisposition::Accepted,
            )
        } else if input.source_contract_version == COMPATIBLE_CONTRACT_VERSION
            && input
                .supported_contract_versions
                .iter()
                .any(|value| value == CURRENT_CONTRACT_VERSION)
        {
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
    let mut effect_receipts = if matches!(
        disposition,
        ManifestDisposition::Accepted | ManifestDisposition::Migrated
    ) {
        vec!["exchange:permitted-capability-manifest-and-digests".into()]
    } else {
        vec![format!("block:manifest-exchange:{:?}", disposition).to_lowercase()]
    };
    effect_receipts.sort();
    if disposition == ManifestDisposition::Incompatible {
        effect_receipts.push("block:incompatible-contract".into());
        effect_receipts.sort();
    }
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "adapter_id": input.adapter_id, "capability_id": input.capability_id, "negotiated_version": negotiated_version, "disposition": disposition, "input_schema": input.input_schema, "output_schema": input.output_schema, "modality_order": modalities, "effect_order": effects, "permission_order": permissions, "artifact_digest_order": artifacts, "omissions": omissions, "uncertainty": uncertainty, "semantic_loss": semantic_loss, "checks": checks, "effect_receipts": effect_receipts, "raw_data_local": true, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("adapter-capability-manifest:{}", input.adapter_id),
        "application/vnd.aurora.adapter-capability-manifest+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ContractFrontierError::Contract(error.to_string()))?;
    let result = AdapterCapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        adapter_id: input.adapter_id.clone(),
        capability_id: input.capability_id.clone(),
        negotiated_version,
        disposition,
        input_schema: input.input_schema.clone(),
        output_schema: input.output_schema.clone(),
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
    result.validate()?;
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
        return Err(ContractFrontierError::InvalidRequest("adapter, capability, versions, schemas, modalities, artifacts, comparability, locality, and boundary are required".into()));
    }
    if input
        .modality_order
        .iter()
        .chain(input.effects.iter())
        .chain(input.permissions.iter())
        .any(|value| value.trim().is_empty())
    {
        return Err(ContractFrontierError::InvalidRequest(
            "contract names cannot be empty".into(),
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
        input
            .supported_contract_versions
            .push(CURRENT_CONTRACT_VERSION.into());
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
}
