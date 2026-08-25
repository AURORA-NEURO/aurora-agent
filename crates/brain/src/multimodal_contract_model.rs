//! Multimodal evidence contract model and comparability gate.
//!
//! Atlas feature: `AFA-brain-P01-F06`. It is a typed data product: every study×modality
//! binding is checked for schema, units, coordinates, provenance, and semantic compatibility.

use crate::evidence_contract_model::ContractCompatibility;
use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-brain-P01-F06";
pub const CONTRACT_VERSION: &str = "brain-multimodal-evidence-contract/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceFeed2@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet2@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModalitySchemaBinding {
    pub study_id: String,
    pub modality: String,
    pub schema: String,
    pub unit_system: String,
    pub coordinate_system: String,
    pub semantic_digest: ContentHash,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalEvidenceContractRequest {
    pub request_id: String,
    pub study_ids: Vec<String>,
    pub scope: String,
    pub required_modalities: Vec<String>,
    pub comparability_profile: String,
    pub input_schema: String,
    pub output_schema: String,
    pub compatibility: ContractCompatibility,
    pub bindings: Vec<ModalitySchemaBinding>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultimodalContractDisposition {
    Qualified,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalEvidenceContractReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub study_order: Vec<String>,
    pub scope: String,
    pub comparability_profile: String,
    pub disposition: MultimodalContractDisposition,
    pub compatibility: ContractCompatibility,
    pub input_schema: String,
    pub output_schema: String,
    pub modality_order: Vec<String>,
    pub binding_order: Vec<String>,
    pub missing_order: Vec<String>,
    pub semantic_disagreement_order: Vec<String>,
    pub schema_order: Vec<String>,
    pub unit_order: Vec<String>,
    pub coordinate_order: Vec<String>,
    pub semantic_order: Vec<ContentHash>,
    pub artifact_order: Vec<ContentHash>,
    pub provenance_order: Vec<ContentHash>,
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
pub enum MultimodalContractModelError {
    #[error("invalid multimodal evidence contract: {0}")]
    Invalid(String),
    #[error("multimodal contract artifact failed: {0}")]
    Artifact(String),
}

impl MultimodalEvidenceContractReceipt {
    pub fn validate(&self) -> Result<(), MultimodalContractModelError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.input_schema != INPUT_SCHEMA
            || self.output_schema != OUTPUT_SCHEMA
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.comparability_profile.trim().is_empty()
            || self.study_order.len() < 2
            || self.modality_order.len() < 2
            || self.binding_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(MultimodalContractModelError::Invalid(
                "multimodal identity, schema, study/modality closure, locality, or effects are incomplete".into(),
            ));
        }
        for values in [
            &self.study_order,
            &self.modality_order,
            &self.binding_order,
            &self.missing_order,
            &self.semantic_disagreement_order,
            &self.schema_order,
            &self.unit_order,
            &self.coordinate_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(MultimodalContractModelError::Invalid(
                    "multimodal contract ordering is not canonical".into(),
                ));
            }
        }
        for values in [
            &self.semantic_order,
            &self.artifact_order,
            &self.provenance_order,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(MultimodalContractModelError::Invalid(
                    "multimodal digest ordering is not canonical".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("read:local-research-artifacts:")
                && effect != "block:unsafe-release"
        }) {
            return Err(MultimodalContractModelError::Invalid(
                "effect is outside the multimodal contract gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| MultimodalContractModelError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, MultimodalContractModelError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| MultimodalContractModelError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| MultimodalContractModelError::Artifact(error.to_string()))
    }
}

pub fn multimodal_contract_model_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "brain".into(),
        consumers: ["platform reliability engineer".into(), "multimodal study steward".into()].into(),
        behavior: "models multimodal evidence contracts and emits explicit study, modality, schema, unit, coordinate, semantic, and provenance comparability verdicts".into(),
        value: "prevents semantic drift from presenting incomparable imaging and omics evidence as a qualified set".into(),
        inputs: vec![TypedPort { name: "multimodal_evidence_contract".into(), schema: INPUT_SCHEMA.into(), required: true }],
        outputs: vec![TypedPort { name: "qualified_multimodal_contract".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(),
        permissions: ["read:local-research-artifacts".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![
            EvidenceReference { source_id: "ro-crate-1.3".into(), state: EvidenceState::Supported, locator: Some("https://www.researchobject.org/ro-crate/specification.html".into()) },
            EvidenceReference { source_id: "anndata-format".into(), state: EvidenceState::Supported, locator: Some("https://anndata.readthedocs.io/en/stable/fileformat-prose.html".into()) },
        ],
        authority_requirements: Vec::new(),
        autonomy_tier: AutonomyTier::A1,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn model_multimodal_evidence_contract(
    request: &MultimodalEvidenceContractRequest,
) -> Result<MultimodalEvidenceContractReceipt, MultimodalContractModelError> {
    validate_request(request)?;
    let studies = request.study_ids.iter().cloned().collect::<BTreeSet<_>>();
    let modalities = request
        .required_modalities
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut binding_map = BTreeMap::<String, &ModalitySchemaBinding>::new();
    let mut semantic_by_modality = BTreeMap::<String, BTreeSet<ContentHash>>::new();
    let mut schema_order = BTreeSet::new();
    let mut unit_order = BTreeSet::new();
    let mut coordinate_order = BTreeSet::new();
    let mut semantic_order = BTreeSet::new();
    let mut artifact_order = BTreeSet::new();
    let mut provenance_order = BTreeSet::new();
    for binding in &request.bindings {
        let key = format!("{}:{}", binding.study_id, binding.modality);
        binding_map.insert(key, binding);
        semantic_by_modality
            .entry(binding.modality.clone())
            .or_default()
            .insert(binding.semantic_digest.clone());
        schema_order.insert(format!("{}:{}", binding.modality, binding.schema));
        unit_order.insert(format!("{}:{}", binding.modality, binding.unit_system));
        coordinate_order.insert(format!(
            "{}:{}",
            binding.modality, binding.coordinate_system
        ));
        semantic_order.insert(binding.semantic_digest.clone());
        artifact_order.insert(binding.artifact_digest.clone());
        provenance_order.insert(binding.provenance_digest.clone());
    }
    let mut missing = BTreeSet::new();
    let mut disagreement = BTreeSet::new();
    for study in &studies {
        for modality in &modalities {
            let key = format!("{}:{}", study, modality);
            if !binding_map.contains_key(&key) {
                missing.insert(key);
            }
        }
    }
    for (modality, digests) in &semantic_by_modality {
        if digests.len() > 1 {
            disagreement.insert(format!(
                "modality:{}:semantic-digest-disagreement",
                modality
            ));
        }
    }
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    if !missing.is_empty() {
        omissions.extend(
            missing
                .iter()
                .map(|key| format!("binding:{}:required-missing", key)),
        );
    }
    if !disagreement.is_empty() {
        negative.extend(disagreement.iter().cloned());
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
        MultimodalContractDisposition::Blocked
    } else if request.bindings.is_empty() {
        MultimodalContractDisposition::Unknown
    } else if missing.is_empty()
        && disagreement.is_empty()
        && request.compatibility == ContractCompatibility::Additive
    {
        MultimodalContractDisposition::Qualified
    } else {
        MultimodalContractDisposition::Partial
    };
    let binding_order = binding_map.keys().cloned().collect::<Vec<_>>();
    let contract_digest = ContentHash::of_value(&json!({"study_order": studies, "modality_order": modalities, "binding_order": binding_order, "schema_order": schema_order, "unit_order": unit_order, "coordinate_order": coordinate_order, "semantic_order": semantic_order, "compatibility": request.compatibility, "comparability_profile": request.comparability_profile})).map_err(|error| MultimodalContractModelError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "study_order": studies, "scope": request.scope, "comparability_profile": request.comparability_profile, "disposition": disposition, "compatibility": request.compatibility, "input_schema": INPUT_SCHEMA, "output_schema": OUTPUT_SCHEMA, "modality_order": modalities, "binding_order": binding_order, "missing_order": missing, "semantic_disagreement_order": disagreement, "schema_order": schema_order, "unit_order": unit_order, "coordinate_order": coordinate_order, "semantic_order": semantic_order, "artifact_order": artifact_order, "provenance_order": provenance_order, "contract_digest": contract_digest, "replay_identity": request.replay_identity, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-multimodal-contract:{}", request.request_id),
        "application/vnd.aurora.multimodal-evidence-contract+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| MultimodalContractModelError::Artifact(error.to_string()))?;
    let has_qualified = disposition == MultimodalContractDisposition::Qualified;
    let receipt = MultimodalEvidenceContractReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        study_order: studies.into_iter().collect(),
        scope: request.scope.clone(),
        comparability_profile: request.comparability_profile.clone(),
        disposition,
        compatibility: request.compatibility,
        input_schema: INPUT_SCHEMA.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        modality_order: modalities.into_iter().collect(),
        binding_order,
        missing_order: missing.into_iter().collect(),
        semantic_disagreement_order: disagreement.into_iter().collect(),
        schema_order: schema_order.into_iter().collect(),
        unit_order: unit_order.into_iter().collect(),
        coordinate_order: coordinate_order.into_iter().collect(),
        semantic_order: semantic_order.into_iter().collect(),
        artifact_order: artifact_order.into_iter().collect(),
        provenance_order: provenance_order.into_iter().collect(),
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
    request: &MultimodalEvidenceContractRequest,
) -> Result<(), MultimodalContractModelError> {
    if request.request_id.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.comparability_profile.trim().is_empty()
        || request.input_schema != INPUT_SCHEMA
        || request.output_schema != OUTPUT_SCHEMA
        || request.study_ids.len() < 2
        || request.required_modalities.len() < 2
        || request.bindings.is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(MultimodalContractModelError::Invalid("multimodal identity, schemas, study/modality floors, bindings, or boundary is incomplete".into()));
    }
    if request.study_ids.windows(2).any(|pair| pair[0] >= pair[1])
        || request
            .required_modalities
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(MultimodalContractModelError::Invalid(
            "study and modality requirements must be unique and canonical".into(),
        ));
    }
    let mut keys = BTreeSet::new();
    for binding in &request.bindings {
        if binding.study_id.trim().is_empty()
            || binding.modality.trim().is_empty()
            || binding.schema.trim().is_empty()
            || binding.unit_system.trim().is_empty()
            || binding.coordinate_system.trim().is_empty()
            || !request.study_ids.contains(&binding.study_id)
            || !request.required_modalities.contains(&binding.modality)
            || !keys.insert(format!("{}:{}", binding.study_id, binding.modality))
        {
            return Err(MultimodalContractModelError::Invalid(format!(
                "binding {}:{} is invalid, out of scope, or duplicated",
                binding.study_id, binding.modality
            )));
        }
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
    fn binding(study: &str, modality: &str, semantic: &str) -> ModalitySchemaBinding {
        ModalitySchemaBinding {
            study_id: study.into(),
            modality: modality.into(),
            schema: format!("{modality}/1"),
            unit_system: "si".into(),
            coordinate_system: "sample-relative".into(),
            semantic_digest: hash(semantic),
            artifact_digest: hash(&format!("artifact:{study}:{modality}")),
            provenance_digest: hash(&format!("provenance:{study}:{modality}")),
        }
    }
    fn request(bindings: Vec<ModalitySchemaBinding>) -> MultimodalEvidenceContractRequest {
        MultimodalEvidenceContractRequest {
            request_id: "request:multimodal-contract".into(),
            study_ids: vec!["study:a".into(), "study:b".into()],
            scope: "organoid:neural".into(),
            required_modalities: vec!["imaging".into(), "transcriptomics".into()],
            comparability_profile: "preclinical-multimodal/v1".into(),
            input_schema: INPUT_SCHEMA.into(),
            output_schema: OUTPUT_SCHEMA.into(),
            compatibility: ContractCompatibility::Additive,
            bindings,
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1_and_typed() {
        let manifest = multimodal_contract_model_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A1);
    }
    #[test]
    fn complete_study_modality_matrix_qualifies() {
        let receipt = model_multimodal_evidence_contract(&request(vec![
            binding("study:a", "imaging", "image-semantic"),
            binding("study:a", "transcriptomics", "rna-semantic"),
            binding("study:b", "imaging", "image-semantic"),
            binding("study:b", "transcriptomics", "rna-semantic"),
        ]))
        .unwrap();
        assert_eq!(
            receipt.disposition,
            MultimodalContractDisposition::Qualified
        );
        assert!(receipt.missing_order.is_empty());
    }
    #[test]
    fn missing_binding_is_partial() {
        let receipt = model_multimodal_evidence_contract(&request(vec![
            binding("study:a", "imaging", "image-semantic"),
            binding("study:b", "imaging", "image-semantic"),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, MultimodalContractDisposition::Partial);
        assert!(receipt
            .missing_order
            .iter()
            .any(|item| item.contains("transcriptomics")));
    }
    #[test]
    fn semantic_disagreement_is_retained() {
        let receipt = model_multimodal_evidence_contract(&request(vec![
            binding("study:a", "imaging", "image-one"),
            binding("study:a", "transcriptomics", "rna-semantic"),
            binding("study:b", "imaging", "image-two"),
            binding("study:b", "transcriptomics", "rna-semantic"),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, MultimodalContractDisposition::Partial);
        assert!(!receipt.semantic_disagreement_order.is_empty());
    }
    #[test]
    fn policy_denial_blocks_contract() {
        let mut input = request(vec![
            binding("study:a", "imaging", "image-semantic"),
            binding("study:a", "transcriptomics", "rna-semantic"),
            binding("study:b", "imaging", "image-semantic"),
            binding("study:b", "transcriptomics", "rna-semantic"),
        ]);
        input.policy_allow = false;
        let receipt = model_multimodal_evidence_contract(&input).unwrap();
        assert_eq!(receipt.disposition, MultimodalContractDisposition::Blocked);
    }
    #[test]
    fn duplicate_binding_is_rejected() {
        let first = binding("study:a", "imaging", "image-semantic");
        let duplicate = first.clone();
        assert!(model_multimodal_evidence_contract(&request(vec![first, duplicate])).is_err());
    }
}
