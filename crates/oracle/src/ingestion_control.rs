//! Federated multimodal-ingestion control plane.
//!
//! Atlas feature: `AFA-oracle-P06-F30`.
//!
//! The oracle crate qualifies modality manifests before a harmonizer or downstream evaluator can
//! use them. Raw payloads never leave the originating institution; only quality, semantic,
//! provenance, and artifact digests enter the signed aggregate manifest.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-oracle-P06-F30";
pub const CONTRACT_VERSION: &str = "oracle-federated-multimodal-ingestion-control/1.0";
pub const MAX_MODALITIES: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModalityState {
    Accepted,
    Missing,
    Unknown,
    Contradicted,
    Unmeasured,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModalityManifest {
    pub modality_id: String,
    pub study_id: String,
    pub semantic_profile: String,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub oracle_id: String,
    pub quality_milli: u16,
    pub state: ModalityState,
    pub checked: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedIngestionControlRequest {
    pub request_id: String,
    pub workflow_id: String,
    pub federation_id: String,
    pub study_scope: String,
    pub required_modalities: Vec<String>,
    pub minimum_quality_milli: u16,
    pub modalities: Vec<ModalityManifest>,
    pub replay_identity: ContentHash,
    pub budget: u64,
    pub policy_allow: bool,
    pub federation_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IngestionControlDisposition {
    Qualified,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedIngestionControlReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub federation_id: String,
    pub disposition: IngestionControlDisposition,
    pub modality_order: Vec<String>,
    pub accepted_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub study_order: Vec<String>,
    pub semantic_profile_order: Vec<String>,
    pub artifact_order: Vec<ContentHash>,
    pub provenance_order: Vec<ContentHash>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub replay_identity: ContentHash,
    pub effect_receipts: Vec<String>,
    pub aggregate_manifest: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum IngestionControlError {
    #[error("invalid federated ingestion control request: {0}")]
    Invalid(String),
    #[error("federated ingestion aggregate artifact failed: {0}")]
    Artifact(String),
    #[error("federated ingestion serialization failed: {0}")]
    Serialization(String),
}

impl FederatedIngestionControlReceipt {
    pub fn validate(&self) -> Result<(), IngestionControlError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.modality_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(IngestionControlError::Invalid(
                "identity, modality order, locality, effects, or boundary is incomplete".into(),
            ));
        }
        for values in [
            &self.modality_order,
            &self.accepted_order,
            &self.blocked_order,
            &self.unknown_order,
            &self.study_order,
            &self.semantic_profile_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(IngestionControlError::Invalid(
                    "ingestion ordering is not canonical".into(),
                ));
            }
        }
        for values in [&self.artifact_order, &self.provenance_order] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(IngestionControlError::Invalid(
                    "ingestion digest ordering is not canonical".into(),
                ));
            }
        }
        self.aggregate_manifest
            .validate_metadata()
            .map_err(|error| IngestionControlError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, IngestionControlError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| IngestionControlError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| IngestionControlError::Serialization(error.to_string()))
    }
}

pub fn federated_ingestion_control_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: "0.1.0".into(), owner_crate: "oracle".into(), consumers: ["multimodal harmonization service".into(), "federation steward".into(), "preclinical researcher".into()].into(), behavior: "qualifies typed multimodal manifests with oracle-backed quality and semantic-profile gates, retaining missing and contradictory modalities while exporting aggregate-only digests".into(), value: "prevents incomplete or unmeasured modality bundles from appearing as complete multi-study research objects".into(), inputs: vec![TypedPort { name: "federated_ingestion_control_request".into(), schema: "FederatedIngestionControlRequest@1".into(), required: true }], outputs: vec![TypedPort { name: "federated_ingestion_control_receipt".into(), schema: "FederatedIngestionControlReceipt@1".into(), required: true }], effects: [Effect::ReadLocalData, Effect::WriteLocalArtifact, Effect::ExecuteLocalComputation, Effect::FederationExport].into(), permissions: ["read:local-modality-manifest".into(), "exchange:aggregate-ingestion-manifest".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "ome-ngff-0.5".into(), state: EvidenceState::Supported, locator: Some("https://ngff.openmicroscopy.org/rfc/5/".into()) }], authority_requirements: vec![AuthorityRequirement { role: "institutional federation steward".into(), reason: "approve aggregate-only cross-institution modality manifest exchange".into() }], autonomy_tier: AutonomyTier::A2, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into()
    }
}

pub fn control_federated_ingestion(
    request: &FederatedIngestionControlRequest,
) -> Result<FederatedIngestionControlReceipt, IngestionControlError> {
    validate_request(request)?;
    let mut modalities = request.modalities.clone();
    modalities.sort_by(|left, right| {
        left.quality_milli
            .cmp(&right.quality_milli)
            .reverse()
            .then(left.modality_id.cmp(&right.modality_id))
    });
    let modality_order = modalities
        .iter()
        .map(|item| item.modality_id.clone())
        .collect::<Vec<_>>();
    let mut accepted = Vec::new();
    let mut blocked = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut studies = BTreeSet::new();
    let mut profiles = BTreeSet::new();
    let mut artifacts = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut spent = 0_u64;
    for modality in &modalities {
        let cost = modality.modality_id.len() as u64 + 1;
        let budget_ok = cost <= request.budget.saturating_sub(spent);
        let complete = modality.state == ModalityState::Accepted
            && modality.quality_milli >= request.minimum_quality_milli
            && modality.checked
            && modality.raw_data_local
            && modality.study_id == request.study_scope
            && !modality.oracle_id.trim().is_empty()
            && budget_ok;
        let admitted = request.policy_allow
            && request.federation_allow
            && request.protected_closure
            && request.signed_approval
            && request.raw_data_local
            && complete;
        if admitted {
            spent = spent.saturating_add(cost);
            accepted.push(modality.modality_id.clone());
            studies.insert(modality.study_id.clone());
            profiles.insert(modality.semantic_profile.clone());
            artifacts.insert(modality.artifact_digest.clone());
            provenance.insert(modality.provenance_digest.clone());
        } else {
            blocked.insert(modality.modality_id.clone());
            if matches!(
                modality.state,
                ModalityState::Unknown | ModalityState::Unmeasured
            ) {
                unknown.insert(modality.modality_id.clone());
                uncertainty.insert(
                    format!(
                        "modality:{}:state-{:?}-not-admitted",
                        modality.modality_id, modality.state
                    )
                    .to_ascii_lowercase(),
                );
            }
            if modality.state == ModalityState::Contradicted {
                negative.insert(format!(
                    "modality:{}:contradicted-negative-evidence",
                    modality.modality_id
                ));
            }
            if modality.quality_milli < request.minimum_quality_milli {
                omissions.insert(format!(
                    "modality:{}:quality-below-threshold",
                    modality.modality_id
                ));
            }
            if !modality.checked {
                uncertainty.insert(format!(
                    "modality:{}:oracle-check-missing",
                    modality.modality_id
                ));
            }
            if modality.study_id != request.study_scope {
                omissions.insert(format!(
                    "modality:{}:study-scope-mismatch",
                    modality.modality_id
                ));
            }
            if !modality.raw_data_local || !request.raw_data_local {
                negative.insert(format!(
                    "modality:{}:raw-data-locality-failed",
                    modality.modality_id
                ));
            }
            if !budget_ok {
                omissions.insert(format!(
                    "modality:{}:budget-exhausted",
                    modality.modality_id
                ));
            }
        }
    }
    for required in &request.required_modalities {
        if !accepted.contains(required) {
            omissions.insert(format!("modality:{}:required-but-not-admitted", required));
        }
    }
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.federation_allow {
        negative.insert("request:federation-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.signed_approval {
        omissions.insert("request:signed-approval-required".into());
    }
    let disposition = if !request.policy_allow
        || !request.federation_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.raw_data_local
    {
        IngestionControlDisposition::Blocked
    } else if accepted.is_empty()
        || request
            .required_modalities
            .iter()
            .any(|required| !accepted.contains(required))
    {
        IngestionControlDisposition::Unknown
    } else if blocked.is_empty()
        && omissions.is_empty()
        && uncertainty.is_empty()
        && negative.is_empty()
    {
        IngestionControlDisposition::Qualified
    } else {
        IngestionControlDisposition::Partial
    };
    let mut checks: Vec<String> = vec!["modality ordering is deterministic by quality then modality identity".into(), "required-modality, quality, oracle-check, study-scope, locality, policy, federation, approval, and budget gates are explicit".into(), "missing, unknown, unmeasured, contradicted, omitted, and negative modalities remain unresolved".into(), "raw modality payloads remain institution-local; federation exports aggregate digests only".into()];
    checks.sort();
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "workflow_id": request.workflow_id, "federation_id": request.federation_id, "disposition": disposition, "modality_order": modality_order, "accepted_order": accepted, "blocked_order": blocked, "unknown_order": unknown, "study_order": studies, "semantic_profile_order": profiles, "artifact_order": artifacts, "provenance_order": provenance, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "replay_identity": request.replay_identity, "boundary": PRECLINICAL_BOUNDARY});
    let aggregate_manifest = TypedResearchArtifact::from_payload(
        format!("federated-ingestion-manifest:{}", request.request_id),
        "application/vnd.aurora.federated-ingestion-manifest+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| IngestionControlError::Artifact(error.to_string()))?;
    let effect_receipts = if accepted.is_empty() {
        vec!["block:federated-ingestion-release".into()]
    } else {
        vec![format!(
            "exchange:aggregate-ingestion-manifest:{}",
            request.request_id
        )]
    };
    let receipt = FederatedIngestionControlReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        federation_id: request.federation_id.clone(),
        disposition,
        modality_order,
        accepted_order: accepted,
        blocked_order: blocked.into_iter().collect(),
        unknown_order: unknown.into_iter().collect(),
        study_order: studies.into_iter().collect(),
        semantic_profile_order: profiles.into_iter().collect(),
        artifact_order: artifacts.into_iter().collect(),
        provenance_order: provenance.into_iter().collect(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        replay_identity: request.replay_identity.clone(),
        effect_receipts,
        aggregate_manifest,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(
    request: &FederatedIngestionControlRequest,
) -> Result<(), IngestionControlError> {
    if request.request_id.trim().is_empty()
        || request.workflow_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.study_scope.trim().is_empty()
        || request.required_modalities.is_empty()
        || request.minimum_quality_milli > 1000
        || request.modalities.is_empty()
        || request.modalities.len() > MAX_MODALITIES
        || request.budget == 0
        || request.boundary != PRECLINICAL_BOUNDARY
        || request
            .required_modalities
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(IngestionControlError::Invalid(
            "request identity, required modalities, quality, budget, or boundary is incomplete"
                .into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for modality in &request.modalities {
        if modality.modality_id.trim().is_empty()
            || modality.study_id.trim().is_empty()
            || modality.semantic_profile.trim().is_empty()
            || modality.oracle_id.trim().is_empty()
            || modality.quality_milli > 1000
            || modality.boundary != PRECLINICAL_BOUNDARY
            || !ids.insert(modality.modality_id.clone())
        {
            return Err(IngestionControlError::Invalid(format!(
                "modality {} is invalid or duplicated",
                modality.modality_id
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn modality(id: &str, state: ModalityState) -> ModalityManifest {
        ModalityManifest {
            modality_id: id.into(),
            study_id: "study:organoid".into(),
            semantic_profile: format!("profile:{id}"),
            artifact_digest: hash(&format!("artifact:{id}")),
            provenance_digest: hash(&format!("provenance:{id}")),
            oracle_id: "oracle:qc".into(),
            quality_milli: 900,
            state,
            checked: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    fn request(modalities: Vec<ModalityManifest>) -> FederatedIngestionControlRequest {
        FederatedIngestionControlRequest {
            request_id: "request:ingestion".into(),
            workflow_id: "workflow:ingestion".into(),
            federation_id: "federation:organoid".into(),
            study_scope: "study:organoid".into(),
            required_modalities: vec!["imaging".into(), "omics".into()],
            minimum_quality_milli: 700,
            modalities,
            replay_identity: hash("replay"),
            budget: 1000,
            policy_allow: true,
            federation_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_typed_a2_and_aggregate_only() {
        let manifest = federated_ingestion_control_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A2);
        assert!(manifest.effects.contains(&Effect::FederationExport));
    }
    #[test]
    fn qualifies_required_modalities() {
        let receipt = control_federated_ingestion(&request(vec![
            modality("omics", ModalityState::Accepted),
            modality("imaging", ModalityState::Accepted),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, IngestionControlDisposition::Qualified);
        assert_eq!(receipt.accepted_order, vec!["imaging", "omics"]);
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
    #[test]
    fn unknown_modality_is_retained() {
        let receipt = control_federated_ingestion(&request(vec![
            modality("imaging", ModalityState::Accepted),
            modality("omics", ModalityState::Unknown),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, IngestionControlDisposition::Unknown);
        assert!(receipt.unknown_order.contains(&"omics".into()));
    }
    #[test]
    fn contradiction_is_negative_evidence() {
        let receipt = control_federated_ingestion(&request(vec![
            modality("imaging", ModalityState::Accepted),
            modality("omics", ModalityState::Contradicted),
        ]))
        .unwrap();
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|item| item.contains("omics")));
    }
    #[test]
    fn federation_denial_blocks_release() {
        let mut input = request(vec![
            modality("imaging", ModalityState::Accepted),
            modality("omics", ModalityState::Accepted),
        ]);
        input.federation_allow = false;
        let receipt = control_federated_ingestion(&input).unwrap();
        assert_eq!(receipt.disposition, IngestionControlDisposition::Blocked);
        assert_eq!(
            receipt.effect_receipts,
            vec!["block:federated-ingestion-release"]
        );
    }
    #[test]
    fn duplicate_modalities_are_rejected() {
        let result = control_federated_ingestion(&request(vec![
            modality("imaging", ModalityState::Accepted),
            modality("imaging", ModalityState::Accepted),
        ]));
        assert!(result.is_err());
    }
}
