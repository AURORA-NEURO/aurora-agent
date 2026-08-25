//! Multimodal researcher workbench for cross-study evidence review.
//!
//! Atlas feature: `AFA-brain-P01-F18`. This is a read-only product surface for comparing
//! study/modality coverage, lineage, and omission state without promoting incomplete closure.

use crate::multimodal_evidence_surveillance::{
    surveil_multimodal_evidence, MultimodalEvidenceDisposition, MultimodalEvidenceFeedRequest,
};
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

pub const FEATURE_ID: &str = "AFA-brain-P01-F18";
pub const CONTRACT_VERSION: &str = "brain-multimodal-research-workbench/1.0";
pub const VIEW_ORDER: [&str; 3] = [
    "view:comparability-matrix",
    "view:modality-coverage",
    "view:source-lineage",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalWorkbenchRequest {
    pub request: MultimodalEvidenceFeedRequest,
    pub workspace_id: String,
    pub requested_view_order: Vec<String>,
    pub requested_panel_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub budget_units: u32,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalWorkbenchReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workspace_id: String,
    pub scope: String,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub disposition: MultimodalEvidenceDisposition,
    pub view_order: Vec<String>,
    pub panel_order: Vec<String>,
    pub action_receipts: Vec<String>,
    pub candidate_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub evidence_digest: ContentHash,
    pub comparability_digest: ContentHash,
    pub workbench_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub budget_units: u32,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MultimodalWorkbenchError {
    #[error("invalid multimodal workbench request: {0}")]
    Invalid(String),
    #[error("multimodal workbench artifact failed: {0}")]
    Artifact(String),
    #[error("multimodal workbench engine failed: {0}")]
    Engine(String),
}

impl MultimodalWorkbenchReceipt {
    pub fn validate(&self) -> Result<(), MultimodalWorkbenchError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workspace_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.study_order.len() < 2
            || self.modality_order.len() < 2
            || self.view_order.is_empty()
            || self.panel_order.is_empty()
            || self.action_receipts.is_empty()
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
            || self.budget_units == 0
        {
            return Err(MultimodalWorkbenchError::Invalid(
                "multimodal workbench identity, study/modality views, evidence, locality, budget, or effects are incomplete".into(),
            ));
        }
        if self
            .qualified_order
            .iter()
            .chain(self.blocked_order.iter())
            .chain(self.unknown_order.iter())
            .any(|id| !self.candidate_order.contains(id))
        {
            return Err(MultimodalWorkbenchError::Invalid(
                "multimodal workbench state is not covered by candidates".into(),
            ));
        }
        for values in [
            &self.study_order,
            &self.modality_order,
            &self.view_order,
            &self.panel_order,
            &self.action_receipts,
            &self.candidate_order,
            &self.qualified_order,
            &self.blocked_order,
            &self.unknown_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(MultimodalWorkbenchError::Invalid(
                    "multimodal workbench ordering is not canonical".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("view:local-multimodal-artifacts:")
                && effect != "block:unsafe-release"
        }) {
            return Err(MultimodalWorkbenchError::Invalid(
                "multimodal workbench effect is not read-only".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| MultimodalWorkbenchError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, MultimodalWorkbenchError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| MultimodalWorkbenchError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| MultimodalWorkbenchError::Artifact(error.to_string()))
    }
}

pub fn multimodal_research_workbench_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "brain".into(),
        consumers: ["multimodal researcher".into(), "study-comparability steward".into()].into(),
        behavior: "renders local comparability, modality-coverage, and source-lineage views for multiple preclinical studies".into(),
        value: "lets researchers inspect multimodal closure and competing evidence without concealing missing modalities".into(),
        inputs: vec![TypedPort { name: "multimodal_workbench_request".into(), schema: "ResearchWorkbenchSpec2@1".into(), required: true }],
        outputs: vec![TypedPort { name: "multimodal_workbench_receipt".into(), schema: "MultimodalWorkbenchReceipt1@1".into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(),
        permissions: ["view:local-multimodal-artifacts".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "json-schema".into(), state: EvidenceState::Supported, locator: Some("https://json-schema.org/specification".into()) }],
        authority_requirements: Vec::new(),
        autonomy_tier: AutonomyTier::A0,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn compile_multimodal_research_workbench(
    request: &MultimodalWorkbenchRequest,
) -> Result<MultimodalWorkbenchReceipt, MultimodalWorkbenchError> {
    validate_request(request)?;
    let evidence = surveil_multimodal_evidence(&request.request)
        .map_err(|error| MultimodalWorkbenchError::Engine(error.to_string()))?;
    let mut omissions = evidence.omissions.iter().cloned().collect::<BTreeSet<_>>();
    let uncertainty = evidence
        .uncertainty
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let negative = evidence
        .negative_evidence
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let study_order = request
        .request
        .study_ids
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let modality_order = request
        .request
        .required_modalities
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let view_order = request.requested_view_order.clone();
    let panel_order = request.requested_panel_order.clone();
    let action_receipts = [
        "action:render-comparability-matrix",
        "action:render-modality-coverage",
        "action:render-source-lineage",
    ]
    .into_iter()
    .map(String::from)
    .collect::<Vec<_>>();
    let actionable = request.policy_allow
        && request.protected_closure
        && request.raw_data_local
        && request.budget_units >= action_receipts.len() as u32
        && evidence.disposition != MultimodalEvidenceDisposition::Blocked;
    if request.budget_units < action_receipts.len() as u32 {
        omissions.insert("workbench:budget-exhausted".into());
    }
    if !request.policy_allow {
        omissions.insert("workbench:policy-denied".into());
    }
    if !request.protected_closure {
        omissions.insert("workbench:protected-closure-incomplete".into());
    }
    if !request.raw_data_local {
        omissions.insert("workbench:raw-data-locality-failed".into());
    }
    let disposition = if actionable {
        evidence.disposition
    } else {
        MultimodalEvidenceDisposition::Blocked
    };
    let evidence_digest = evidence
        .digest()
        .map_err(|error| MultimodalWorkbenchError::Engine(error.to_string()))?;
    let comparability_digest = ContentHash::of_value(&json!({"study_order": study_order, "modality_order": modality_order, "scope": request.request.scope, "candidate_order": evidence.candidate_order, "replay_identity": request.replay_identity})).map_err(|error| MultimodalWorkbenchError::Artifact(error.to_string()))?;
    let workbench_digest = ContentHash::of_value(&json!({"workspace_id": request.workspace_id, "view_order": view_order, "panel_order": panel_order, "action_receipts": action_receipts, "evidence_digest": evidence_digest, "comparability_digest": comparability_digest, "replay_identity": request.replay_identity, "budget_units": request.budget_units})).map_err(|error| MultimodalWorkbenchError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request.request_id, "workspace_id": request.workspace_id, "scope": request.request.scope, "study_order": study_order, "modality_order": modality_order, "disposition": disposition, "view_order": view_order, "panel_order": panel_order, "action_receipts": action_receipts, "candidate_order": evidence.candidate_order, "qualified_order": evidence.qualified_order, "blocked_order": evidence.blocked_order, "unknown_order": evidence.unknown_order, "evidence_digest": evidence_digest, "comparability_digest": comparability_digest, "workbench_digest": workbench_digest, "replay_identity": request.replay_identity, "budget_units": request.budget_units, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-multimodal-workbench:{}", request.workspace_id),
        "application/vnd.aurora.multimodal-workbench-receipt+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| MultimodalWorkbenchError::Artifact(error.to_string()))?;
    let receipt = MultimodalWorkbenchReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request.request_id.clone(),
        workspace_id: request.workspace_id.clone(),
        scope: request.request.scope.clone(),
        study_order,
        modality_order,
        disposition,
        view_order,
        panel_order,
        action_receipts,
        candidate_order: evidence.candidate_order.clone(),
        qualified_order: evidence.qualified_order.clone(),
        blocked_order: evidence.blocked_order.clone(),
        unknown_order: evidence.unknown_order.clone(),
        evidence_digest,
        comparability_digest,
        workbench_digest,
        replay_identity: request.replay_identity.clone(),
        budget_units: request.budget_units,
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts: if actionable {
            vec![format!(
                "view:local-multimodal-artifacts:{}",
                request.workspace_id
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

fn validate_request(request: &MultimodalWorkbenchRequest) -> Result<(), MultimodalWorkbenchError> {
    if request.workspace_id.trim().is_empty()
        || request.requested_view_order
            != VIEW_ORDER
                .iter()
                .map(|value| (*value).to_string())
                .collect::<Vec<_>>()
        || request.requested_panel_order.is_empty()
        || request.budget_units == 0
        || request.request.replay_identity != request.replay_identity
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(MultimodalWorkbenchError::Invalid("multimodal workbench identity, canonical views, panels, budget, replay, or boundary is incomplete".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence_surveillance::EvidenceObservation;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn request(state: EvidenceState) -> MultimodalWorkbenchRequest {
        MultimodalWorkbenchRequest {
            request: MultimodalEvidenceFeedRequest {
                request_id: "request:multimodal-workbench".into(),
                study_ids: vec!["study:a".into(), "study:b".into()],
                scope: "organoid:neural".into(),
                query: "mechanism".into(),
                minimum_relevance_milli: 700,
                required_modalities: vec!["imaging".into(), "transcriptomics".into()],
                observations: vec![EvidenceObservation {
                    evidence_id: "evidence:a".into(),
                    source_id: "source:a".into(),
                    study_id: "study:a".into(),
                    modality: "imaging".into(),
                    scope: "organoid:neural".into(),
                    relevance_milli: 900,
                    state,
                    semantic_digest: hash("semantic"),
                    artifact_digest: hash("artifact"),
                    provenance_digest: hash("provenance"),
                    replay_identity: hash("replay"),
                    omissions: Vec::new(),
                    negative_evidence: Vec::new(),
                    raw_data_local: true,
                    boundary: PRECLINICAL_BOUNDARY.into(),
                }],
                replay_identity: hash("replay"),
                policy_allow: true,
                protected_closure: true,
                raw_data_local: true,
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            workspace_id: "workspace:multimodal".into(),
            requested_view_order: VIEW_ORDER
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            requested_panel_order: vec!["panel:coverage".into(), "panel:lineage".into()],
            replay_identity: hash("replay"),
            budget_units: 8,
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a0_read_only() {
        let manifest = multimodal_research_workbench_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A0);
    }
    #[test]
    fn multimodal_coverage_is_visible() {
        let receipt =
            compile_multimodal_research_workbench(&request(EvidenceState::Supported)).unwrap();
        assert_eq!(receipt.disposition, MultimodalEvidenceDisposition::Partial);
        assert!(receipt.effect_receipts[0].starts_with("view:"));
        assert!(!receipt.omissions.is_empty());
    }
    #[test]
    fn unknown_evidence_is_not_promoted() {
        let receipt =
            compile_multimodal_research_workbench(&request(EvidenceState::Unknown)).unwrap();
        assert_eq!(receipt.disposition, MultimodalEvidenceDisposition::Unknown);
        assert!(!receipt.unknown_order.is_empty());
    }
    #[test]
    fn policy_denial_blocks_view() {
        let mut input = request(EvidenceState::Supported);
        input.policy_allow = false;
        let receipt = compile_multimodal_research_workbench(&input).unwrap();
        assert_eq!(receipt.disposition, MultimodalEvidenceDisposition::Blocked);
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn replay_mismatch_is_rejected() {
        let mut input = request(EvidenceState::Supported);
        input.replay_identity = hash("different");
        assert!(compile_multimodal_research_workbench(&input).is_err());
    }
    #[test]
    fn digest_is_stable() {
        let receipt =
            compile_multimodal_research_workbench(&request(EvidenceState::Unknown)).unwrap();
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
}
