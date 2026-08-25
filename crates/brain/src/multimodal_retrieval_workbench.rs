//! Multimodal retrieval researcher workbench.
//!
//! Atlas feature: `AFA-brain-P02-F18`. Study/modality coverage and comparability remain visible
//! in a read-only workbench rather than being silently completed by the UI.

use crate::multimodal_retrieval_synthesis::{
    synthesize_multimodal_retrieval, MultimodalRetrievalQuery,
};
use crate::retrieval_synthesis::SynthesisDisposition;
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

pub const FEATURE_ID: &str = "AFA-brain-P02-F18";
pub const CONTRACT_VERSION: &str = "brain-multimodal-retrieval-research-workbench/1.0";
pub const VIEW_ORDER: [&str; 3] = [
    "view:multimodal-retrieval-matrix",
    "view:modality-coverage",
    "view:source-lineage",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalRetrievalWorkbenchRequest {
    pub request: MultimodalRetrievalQuery,
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
pub struct MultimodalRetrievalWorkbenchReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workspace_id: String,
    pub scope: String,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub disposition: SynthesisDisposition,
    pub view_order: Vec<String>,
    pub panel_order: Vec<String>,
    pub action_receipts: Vec<String>,
    pub candidate_order: Vec<String>,
    pub ranked_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub comparability_digest: ContentHash,
    pub synthesis_digest: ContentHash,
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
pub enum MultimodalRetrievalWorkbenchError {
    #[error("invalid multimodal retrieval workbench request: {0}")]
    Invalid(String),
    #[error("multimodal retrieval workbench artifact failed: {0}")]
    Artifact(String),
    #[error("multimodal retrieval workbench engine failed: {0}")]
    Engine(String),
}

impl MultimodalRetrievalWorkbenchReceipt {
    pub fn validate(&self) -> Result<(), MultimodalRetrievalWorkbenchError> {
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
            return Err(MultimodalRetrievalWorkbenchError::Invalid("multimodal workbench identity, coverage, views, panels, retrieval, locality, budget, or effects are incomplete".into()));
        }
        if self
            .ranked_order
            .iter()
            .chain(self.qualified_order.iter())
            .chain(self.blocked_order.iter())
            .chain(self.unknown_order.iter())
            .any(|id| !self.candidate_order.contains(id))
        {
            return Err(MultimodalRetrievalWorkbenchError::Invalid(
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
            &self.ranked_order,
            &self.qualified_order,
            &self.blocked_order,
            &self.unknown_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(MultimodalRetrievalWorkbenchError::Invalid(
                    "multimodal workbench ordering is not canonical".into(),
                ));
            }
        }
        for digest in [
            &self.comparability_digest,
            &self.synthesis_digest,
            &self.workbench_digest,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(MultimodalRetrievalWorkbenchError::Invalid(
                    "multimodal workbench digest is invalid".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("view:local-multimodal-retrieval-artifacts:")
                && effect != "block:unsafe-release"
        }) {
            return Err(MultimodalRetrievalWorkbenchError::Invalid(
                "multimodal workbench effect is not read-only".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| MultimodalRetrievalWorkbenchError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, MultimodalRetrievalWorkbenchError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| MultimodalRetrievalWorkbenchError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| MultimodalRetrievalWorkbenchError::Artifact(error.to_string()))
    }
}

pub fn multimodal_retrieval_workbench_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["researcher".into(), "multimodal retrieval curator".into()].into(), behavior: "renders local multimodal retrieval matrix, modality coverage, and source-lineage views with deterministic read-only receipts".into(), value: "gives researchers an auditable cross-study workbench without hiding missing modalities, comparability loss, or unresolved evidence".into(), inputs: vec![TypedPort { name: "multimodal_retrieval_workbench_request".into(), schema: "ResearchWorkbenchSpec3@1".into(), required: true }], outputs: vec![TypedPort { name: "multimodal_retrieval_workbench_receipt".into(), schema: "MultimodalRetrievalWorkbenchReceipt1@1".into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["view:local-multimodal-retrieval-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "ome-ngff-rfc5".into(), state: EvidenceState::Supported, locator: Some("https://ngff.openmicroscopy.org/rfc/5/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A0, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn compile_multimodal_retrieval_workbench(
    request: &MultimodalRetrievalWorkbenchRequest,
) -> Result<MultimodalRetrievalWorkbenchReceipt, MultimodalRetrievalWorkbenchError> {
    validate_request(request)?;
    let synthesis = synthesize_multimodal_retrieval(&request.request)
        .map_err(|error| MultimodalRetrievalWorkbenchError::Engine(error.to_string()))?;
    let view_order = request.requested_view_order.clone();
    let panel_order = request.requested_panel_order.clone();
    let action_receipts = [
        "action:render-multimodal-retrieval-matrix",
        "action:render-modality-coverage",
        "action:render-source-lineage",
    ]
    .into_iter()
    .map(String::from)
    .collect::<Vec<_>>();
    let mut omissions = synthesis.omissions.iter().cloned().collect::<BTreeSet<_>>();
    let uncertainty = synthesis
        .uncertainty
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let negative = synthesis
        .negative_evidence
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let actionable = request.policy_allow
        && request.protected_closure
        && request.raw_data_local
        && request.budget_units >= action_receipts.len() as u32
        && synthesis.disposition != SynthesisDisposition::Blocked;
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
        synthesis.disposition
    } else {
        SynthesisDisposition::Blocked
    };
    let synthesis_digest = synthesis
        .digest()
        .map_err(|error| MultimodalRetrievalWorkbenchError::Engine(error.to_string()))?;
    let workbench_digest = ContentHash::of_value(&json!({"workspace_id": request.workspace_id, "view_order": view_order, "panel_order": panel_order, "action_receipts": action_receipts, "comparability_digest": synthesis.comparability_digest, "synthesis_digest": synthesis_digest, "replay_identity": request.replay_identity, "budget_units": request.budget_units})).map_err(|error| MultimodalRetrievalWorkbenchError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request.request_id, "workspace_id": request.workspace_id, "scope": request.request.scope, "study_order": request.request.study_ids, "modality_order": request.request.required_modalities, "disposition": disposition, "view_order": view_order, "panel_order": panel_order, "action_receipts": action_receipts, "candidate_order": synthesis.candidate_order, "ranked_order": synthesis.ranked_order, "qualified_order": synthesis.qualified_order, "blocked_order": synthesis.blocked_order, "unknown_order": synthesis.unknown_order, "comparability_digest": synthesis.comparability_digest, "synthesis_digest": synthesis_digest, "workbench_digest": workbench_digest, "replay_identity": request.replay_identity, "budget_units": request.budget_units, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "brain-multimodal-retrieval-workbench:{}",
            request.workspace_id
        ),
        "application/vnd.aurora.multimodal-retrieval-workbench-receipt+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| MultimodalRetrievalWorkbenchError::Artifact(error.to_string()))?;
    let receipt = MultimodalRetrievalWorkbenchReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request.request_id.clone(),
        workspace_id: request.workspace_id.clone(),
        scope: request.request.scope.clone(),
        study_order: request
            .request
            .study_ids
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        modality_order: request
            .request
            .required_modalities
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        disposition,
        view_order,
        panel_order,
        action_receipts,
        candidate_order: synthesis.candidate_order,
        ranked_order: synthesis.ranked_order,
        qualified_order: synthesis.qualified_order,
        blocked_order: synthesis.blocked_order,
        unknown_order: synthesis.unknown_order,
        comparability_digest: synthesis.comparability_digest,
        synthesis_digest,
        workbench_digest,
        replay_identity: request.replay_identity.clone(),
        budget_units: request.budget_units,
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts: if actionable {
            vec![format!(
                "view:local-multimodal-retrieval-artifacts:{}",
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

fn validate_request(
    request: &MultimodalRetrievalWorkbenchRequest,
) -> Result<(), MultimodalRetrievalWorkbenchError> {
    let expected = VIEW_ORDER
        .iter()
        .map(|value| (*value).to_string())
        .collect::<Vec<_>>();
    if request.workspace_id.trim().is_empty()
        || request.requested_view_order != expected
        || request.requested_panel_order.is_empty()
        || request.budget_units == 0
        || request.request.replay_identity != request.replay_identity
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(MultimodalRetrievalWorkbenchError::Invalid("multimodal workbench identity, canonical views, panels, budget, replay, or boundary is incomplete".into()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retrieval_synthesis::RetrievalCandidate;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn request(state: EvidenceState) -> MultimodalRetrievalWorkbenchRequest {
        let candidates = [
            ("evidence:a-imaging", "study:a", "imaging"),
            ("evidence:a-omics", "study:a", "transcriptomics"),
            ("evidence:b-imaging", "study:b", "imaging"),
            ("evidence:b-omics", "study:b", "transcriptomics"),
        ]
        .into_iter()
        .map(|(evidence_id, study_id, modality)| RetrievalCandidate {
            evidence_id: evidence_id.into(),
            source_id: format!("source:{study_id}:{modality}"),
            study_id: study_id.into(),
            scope: "organoid:neural".into(),
            modality: modality.into(),
            support_milli: 900,
            state,
            semantic_digest: hash(evidence_id),
            artifact_digest: hash(&format!("artifact:{evidence_id}")),
            provenance_digest: hash(&format!("provenance:{evidence_id}")),
            replay_identity: hash("replay"),
            omissions: Vec::new(),
            negative_evidence: Vec::new(),
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        })
        .collect();
        MultimodalRetrievalWorkbenchRequest {
            request: MultimodalRetrievalQuery {
                request_id: "request:multimodal-workbench".into(),
                study_ids: vec!["study:a".into(), "study:b".into()],
                scope: "organoid:neural".into(),
                query: "synaptic density".into(),
                minimum_support_milli: 700,
                required_modalities: vec!["imaging".into(), "transcriptomics".into()],
                candidates,
                replay_identity: hash("replay"),
                policy_allow: true,
                protected_closure: true,
                raw_data_local: true,
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            workspace_id: "workspace:multimodal-retrieval".into(),
            requested_view_order: VIEW_ORDER
                .iter()
                .map(|value| (*value).to_string())
                .collect(),
            requested_panel_order: vec![
                "panel:comparability".into(),
                "panel:modality-omissions".into(),
            ]
            .into_iter()
            .collect(),
            replay_identity: hash("replay"),
            budget_units: 8,
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a0() {
        let manifest = multimodal_retrieval_workbench_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A0);
    }
    #[test]
    fn supported_view_is_read_only() {
        let receipt =
            compile_multimodal_retrieval_workbench(&request(EvidenceState::Supported)).unwrap();
        assert!(
            receipt.effect_receipts[0].starts_with("view:local-multimodal-retrieval-artifacts:")
        );
    }
    #[test]
    fn unknown_remains_visible() {
        let receipt =
            compile_multimodal_retrieval_workbench(&request(EvidenceState::Unknown)).unwrap();
        assert!(!receipt.unknown_order.is_empty());
    }
    #[test]
    fn policy_denial_blocks() {
        let mut input = request(EvidenceState::Supported);
        input.policy_allow = false;
        let receipt = compile_multimodal_retrieval_workbench(&input).unwrap();
        assert_eq!(receipt.disposition, SynthesisDisposition::Blocked);
    }
    #[test]
    fn view_protocol_is_required() {
        let mut input = request(EvidenceState::Supported);
        input.requested_view_order.reverse();
        assert!(compile_multimodal_retrieval_workbench(&input).is_err());
    }
    #[test]
    fn replay_mismatch_is_rejected() {
        let mut input = request(EvidenceState::Supported);
        input.replay_identity = hash("different");
        assert!(compile_multimodal_retrieval_workbench(&input).is_err());
    }
}
