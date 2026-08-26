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
    pub request_id: String,
    pub input_schema: String,
    pub output_schema: String,
    pub semantic_profile: String,
    pub compatibility: MultimodalContractCompatibility,
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
fn sorted_unique(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

impl MultimodalEvidenceSurveillanceContractReceipt {
    pub fn validate(&self) -> Result<(), MultimodalEvidenceSurveillanceContractError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.input_schema != INPUT_SCHEMA
            || self.output_schema != OUTPUT_SCHEMA
            || self.semantic_profile.trim().is_empty()
            || self.study_order.len() < 2
            || self.modality_order.len() < 2
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(MultimodalEvidenceSurveillanceContractError::Invalid("multimodal contract identity, schemas, closure, locality, candidates, or effects are incomplete".into()));
        }
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
            if !sorted_unique(values) {
                return Err(MultimodalEvidenceSurveillanceContractError::Invalid(
                    "multimodal contract ordering is not canonical".into(),
                ));
            }
        }
        let classified = self
            .retained_order
            .iter()
            .chain(self.unknown_order.iter())
            .chain(self.denied_order.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        if classified != self.candidate_order.iter().cloned().collect() {
            return Err(MultimodalEvidenceSurveillanceContractError::Invalid(
                "multimodal contract states do not partition candidates".into(),
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
            if digest.as_str().len() != 64 {
                return Err(MultimodalEvidenceSurveillanceContractError::Invalid(
                    "multimodal contract digest is invalid".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("read:local-multimodal-contract:")
                && effect != "block:unsafe-release"
        }) {
            return Err(MultimodalEvidenceSurveillanceContractError::Invalid(
                "multimodal contract effect is outside local-read gate".into(),
            ));
        }
        if self.disposition == MultimodalContractDisposition::Blocked
            && self.effect_receipts != vec!["block:unsafe-release".to_string()]
        {
            return Err(MultimodalEvidenceSurveillanceContractError::Invalid(
                "blocked multimodal contract must be explicitly blocked".into(),
            ));
        }
        self.artifact.validate_metadata().map_err(|error| {
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
    let mut studies = request.required_studies.clone();
    studies.sort();
    studies.dedup();
    let mut modalities = request.required_modalities.clone();
    modalities.sort();
    modalities.dedup();
    let mut claims = request.claims.clone();
    claims.sort_by(|left, right| left.claim_id.cmp(&right.claim_id));
    let claim_ids = claims
        .iter()
        .map(|claim| claim.claim_id.clone())
        .collect::<Vec<_>>();
    if claim_ids.windows(2).any(|pair| pair[0] == pair[1])
        || claim_ids.iter().any(|value| value.trim().is_empty())
    {
        return Err(MultimodalEvidenceSurveillanceContractError::Invalid(
            "multimodal claim identities must be unique and non-empty".into(),
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
                .map(move |modality| format!("{}::{}::required", study, modality))
        })
        .collect::<BTreeSet<_>>();
    candidate_set.extend(required_cells.iter().cloned());
    let candidate_order = candidate_set.into_iter().collect::<Vec<_>>();
    let mut retained = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut denied = BTreeSet::new();
    let mut incomparable = BTreeSet::new();
    let mut migration = BTreeSet::new();
    let mut loss = BTreeSet::new();
    let mut covered = BTreeSet::new();
    for claim in &claims {
        if compatibility == MultimodalContractCompatibility::Breaking {
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
            covered.insert(format!("{}::{}::required", claim.study_id, claim.modality));
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
    let disposition =
        if !request.policy_allow || !request.protected_closure || !request.raw_data_local {
            MultimodalContractDisposition::Blocked
        } else if retained.is_empty() {
            MultimodalContractDisposition::Unknown
        } else if !unknown.is_empty() || !denied.is_empty() {
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
    let comparability_digest = ContentHash::of_value(&json!({"study_order": studies.clone(), "modality_order": modalities.clone(), "semantic_profile": request.semantic_profile, "covered_cells": covered})).map_err(|error| MultimodalEvidenceSurveillanceContractError::Artifact(error.to_string()))?;
    let contract_digest = ContentHash::of_value(&json!({"input_schema": request.input_schema, "output_schema": request.output_schema, "compatibility": compatibility, "candidate_order": candidate_order.clone()})).map_err(|error| MultimodalEvidenceSurveillanceContractError::Artifact(error.to_string()))?;
    let canonical_digest = ContentHash::of_value(&json!({"retained_order": retained_order.clone(), "unknown_order": unknown_order.clone(), "denied_order": denied_order.clone(), "incomparable_order": incomparable_order.clone(), "migration_order": migration_order.clone(), "semantic_loss": semantic_loss.clone()})).map_err(|error| MultimodalEvidenceSurveillanceContractError::Artifact(error.to_string()))?;
    let provenance_digest = ContentHash::of_value(&json!({"request_id": request.request_id, "replay_identity": request.replay_identity, "comparability_digest": comparability_digest, "contract_digest": contract_digest})).map_err(|error| MultimodalEvidenceSurveillanceContractError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "input_schema": request.input_schema, "output_schema": request.output_schema, "semantic_profile": request.semantic_profile, "compatibility": compatibility, "disposition": disposition, "study_order": studies, "modality_order": modalities, "candidate_order": candidate_order, "retained_order": retained_order, "unknown_order": unknown_order, "denied_order": denied_order, "incomparable_order": incomparable_order, "migration_order": migration_order, "semantic_loss": semantic_loss, "comparability_digest": comparability_digest, "contract_digest": contract_digest, "canonical_digest": canonical_digest, "provenance_digest": provenance_digest, "replay_identity": request.replay_identity, "raw_data_local": request.raw_data_local, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("adapter-multimodal-contract:{}", request.request_id),
        "application/vnd.aurora.qualified-multimodal-evidence-set+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| MultimodalEvidenceSurveillanceContractError::Artifact(error.to_string()))?;
    let receipt = MultimodalEvidenceSurveillanceContractReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        input_schema: INPUT_SCHEMA.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        semantic_profile: request.semantic_profile.clone(),
        compatibility,
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
        effect_receipts: if disposition == MultimodalContractDisposition::Blocked {
            vec!["block:unsafe-release".into()]
        } else {
            vec![format!(
                "read:local-multimodal-contract:{}",
                request.request_id
            )]
        },
        artifact,
        raw_data_local: request.raw_data_local,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
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
        assert_eq!(
            model_multimodal_evidence_surveillance_contract(&value)
                .unwrap()
                .effect_receipts,
            vec!["block:unsafe-release"]
        );
    }
    #[test]
    fn digest_is_stable() {
        let first = model_multimodal_evidence_surveillance_contract(&request()).unwrap();
        let second = model_multimodal_evidence_surveillance_contract(&request()).unwrap();
        assert_eq!(first.canonical_digest, second.canonical_digest);
    }
}
