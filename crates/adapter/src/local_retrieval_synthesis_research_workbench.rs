//! Local single-study retrieval-and-synthesis researcher workbench.
//!
//! Atlas feature `AFA-adapter-P02-F17`: an A0, read-only interaction surface
//! over the typed local retrieval copilot. Views are deterministic and retain
//! omissions, uncertainty, negative evidence, and provenance for replay.

use std::collections::BTreeSet;

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

use crate::local_retrieval_synthesis_research_copilot::{
    run_local_retrieval_synthesis_research_copilot, LocalRetrievalSynthesisResearchCopilotRequest,
};
use crate::retrieval_synthesis::EvidenceSynthesisDisposition;

pub const FEATURE_ID: &str = "AFA-adapter-P02-F17";
pub const CONTRACT_VERSION: &str = "adapter-local-retrieval-synthesis-research-workbench/1.0";
pub const INPUT_SCHEMA: &str = "ScopedRetrievalQuery1@1";
pub const OUTPUT_SCHEMA: &str = "EvidenceSynthesis5@1";
pub const CANONICAL_VIEWS: [&str; 4] = [
    "view:overview",
    "view:evidence",
    "view:omissions",
    "view:provenance",
];
pub const CANONICAL_PANELS: [&str; 4] = [
    "panel:negative",
    "panel:provenance",
    "panel:qualified",
    "panel:unknown",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalRetrievalSynthesisResearchWorkbenchRequest {
    pub copilot_request: LocalRetrievalSynthesisResearchCopilotRequest,
    pub workspace_id: String,
    pub scope: String,
    pub requested_view_order: Vec<String>,
    pub requested_panel_order: Vec<String>,
    pub budget_units: u32,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalRetrievalSynthesisResearchWorkbenchReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workspace_id: String,
    pub query_id: String,
    pub scope: String,
    pub intent: String,
    pub disposition: EvidenceSynthesisDisposition,
    pub view_order: Vec<String>,
    pub panel_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub omitted_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_order: Vec<String>,
    pub contradictory_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub copilot_run_digest: ContentHash,
    pub workbench_digest: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum LocalRetrievalSynthesisResearchWorkbenchError {
    #[error("invalid local retrieval workbench request: {0}")]
    Invalid(String),
    #[error("local retrieval workbench artifact failed: {0}")]
    Artifact(String),
    #[error("local retrieval workbench copilot failed: {0}")]
    Copilot(String),
}

impl LocalRetrievalSynthesisResearchWorkbenchReceipt {
    pub fn validate(&self) -> Result<(), LocalRetrievalSynthesisResearchWorkbenchError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workspace_id.trim().is_empty()
            || self.query_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.intent.trim().is_empty()
            || self.view_order
                != CANONICAL_VIEWS
                    .iter()
                    .map(|x| (*x).to_string())
                    .collect::<Vec<_>>()
            || self.panel_order
                != CANONICAL_PANELS
                    .iter()
                    .map(|x| (*x).to_string())
                    .collect::<Vec<_>>()
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(LocalRetrievalSynthesisResearchWorkbenchError::Invalid(
                "workbench identity, views, candidates, locality, or effects are incomplete".into(),
            ));
        }
        for values in [
            &self.candidate_order,
            &self.selected_order,
            &self.omitted_order,
            &self.uncertainty_order,
            &self.negative_order,
            &self.contradictory_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(LocalRetrievalSynthesisResearchWorkbenchError::Invalid(
                    "workbench ordering is not canonical".into(),
                ));
            }
        }
        let classified = self
            .selected_order
            .iter()
            .chain(self.omitted_order.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        if classified != self.candidate_order.iter().cloned().collect() {
            return Err(LocalRetrievalSynthesisResearchWorkbenchError::Invalid(
                "workbench evidence states do not partition candidates".into(),
            ));
        }
        for value in [
            &self.replay_identity,
            &self.copilot_run_digest,
            &self.workbench_digest,
            &self.artifact.content_hash,
        ] {
            if value.as_str().len() != 64 {
                return Err(LocalRetrievalSynthesisResearchWorkbenchError::Invalid(
                    "workbench digest is invalid".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("view:local-retrieval-workbench:")
                && effect != "block:unsafe-release"
        }) {
            return Err(LocalRetrievalSynthesisResearchWorkbenchError::Invalid(
                "workbench effect is outside read-only view gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|e| LocalRetrievalSynthesisResearchWorkbenchError::Artifact(e.to_string()))
    }
}

pub fn local_retrieval_synthesis_research_workbench_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "adapter".into(), consumers: ["integration engineer".into(), "preclinical researcher".into()].into(), behavior: "renders an omission-aware local retrieval and synthesis researcher interaction with deterministic views and provenance".into(), value: "provides a separately versioned read-only workbench without silently turning unknown evidence into a conclusion".into(), inputs: vec![TypedPort { name: "scoped_retrieval_query".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "evidence_synthesis".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation].into(), permissions: ["view:authorized-research-state".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A0, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn render_local_retrieval_synthesis_research_workbench(
    request: &LocalRetrievalSynthesisResearchWorkbenchRequest,
) -> Result<
    LocalRetrievalSynthesisResearchWorkbenchReceipt,
    LocalRetrievalSynthesisResearchWorkbenchError,
> {
    validate_request(request)?;
    let copilot = run_local_retrieval_synthesis_research_copilot(&request.copilot_request)
        .map_err(|e| LocalRetrievalSynthesisResearchWorkbenchError::Copilot(e.to_string()))?;
    let candidate_order = copilot.candidate_order.clone();
    let selected_order = copilot.selected_order.clone();
    let omitted_order = copilot.omitted_order.clone();
    let omissions = copilot
        .omitted_order
        .iter()
        .map(|id| format!("evidence:{id}:omitted"))
        .chain(
            copilot
                .contradictory_order
                .iter()
                .map(|id| format!("evidence:{id}:contradictory")),
        )
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let uncertainty = copilot
        .uncertainty_order
        .iter()
        .map(|id| format!("evidence:{id}"))
        .collect::<Vec<_>>();
    let copilot_run_digest = ContentHash::of_value(
        &serde_json::to_value(&copilot)
            .map_err(|e| LocalRetrievalSynthesisResearchWorkbenchError::Artifact(e.to_string()))?,
    )
    .map_err(|e| LocalRetrievalSynthesisResearchWorkbenchError::Artifact(e.to_string()))?;
    let workbench_digest = ContentHash::of_value(&json!({"workspace_id":request.workspace_id,"scope":request.scope,"views":CANONICAL_VIEWS,"panels":CANONICAL_PANELS,"candidate_order":candidate_order,"selected_order":selected_order,"omitted_order":omitted_order,"replay_identity":request.replay_identity,"copilot_run_digest":copilot_run_digest})).map_err(|e| LocalRetrievalSynthesisResearchWorkbenchError::Artifact(e.to_string()))?;
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.copilot_request.synthesis_request.request_id,"workspace_id":request.workspace_id,"query_id":request.copilot_request.synthesis_request.query.query_id,"scope":request.scope,"intent":request.copilot_request.synthesis_request.query.intent,"disposition":copilot.disposition,"view_order":CANONICAL_VIEWS,"panel_order":CANONICAL_PANELS,"candidate_order":candidate_order,"selected_order":selected_order,"omitted_order":omitted_order,"uncertainty_order":copilot.uncertainty_order,"negative_order":copilot.negative_order,"contradictory_order":copilot.contradictory_order,"replay_identity":request.replay_identity,"copilot_run_digest":copilot_run_digest,"workbench_digest":workbench_digest,"omissions":omissions,"uncertainty":uncertainty,"negative_evidence":copilot.negative_order,"raw_data_local":true,"boundary":PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("adapter-local-retrieval-workbench:{}", request.workspace_id),
        "application/vnd.aurora.local-retrieval-synthesis-research-workbench+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|e| LocalRetrievalSynthesisResearchWorkbenchError::Artifact(e.to_string()))?;
    let receipt = LocalRetrievalSynthesisResearchWorkbenchReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.copilot_request.synthesis_request.request_id.clone(),
        workspace_id: request.workspace_id.clone(),
        query_id: request
            .copilot_request
            .synthesis_request
            .query
            .query_id
            .clone(),
        scope: request.scope.clone(),
        intent: request
            .copilot_request
            .synthesis_request
            .query
            .intent
            .clone(),
        disposition: copilot.disposition,
        view_order: CANONICAL_VIEWS.iter().map(|x| (*x).to_string()).collect(),
        panel_order: CANONICAL_PANELS.iter().map(|x| (*x).to_string()).collect(),
        candidate_order,
        selected_order,
        omitted_order,
        uncertainty_order: copilot.uncertainty_order.clone(),
        negative_order: copilot.negative_order.clone(),
        contradictory_order: copilot.contradictory_order.clone(),
        replay_identity: request.replay_identity.clone(),
        copilot_run_digest,
        workbench_digest,
        omissions,
        uncertainty,
        negative_evidence: copilot.negative_order.clone(),
        effect_receipts: vec![format!(
            "view:local-retrieval-workbench:{}",
            request.workspace_id
        )],
        artifact,
        raw_data_local: true,
        boundary: request.boundary.clone(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(
    request: &LocalRetrievalSynthesisResearchWorkbenchRequest,
) -> Result<(), LocalRetrievalSynthesisResearchWorkbenchError> {
    if request.workspace_id.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.requested_view_order
            != CANONICAL_VIEWS
                .iter()
                .map(|x| (*x).to_string())
                .collect::<Vec<_>>()
        || request.requested_panel_order
            != CANONICAL_PANELS
                .iter()
                .map(|x| (*x).to_string())
                .collect::<Vec<_>>()
        || request.budget_units == 0
        || request.replay_identity.as_str().len() != 64
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.copilot_request.boundary != PRECLINICAL_BOUNDARY
        || !request.copilot_request.synthesis_request.raw_data_local
    {
        return Err(LocalRetrievalSynthesisResearchWorkbenchError::Invalid(
            "workbench identity, views, budget, locality, replay, or boundary is invalid".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::local_retrieval_synthesis_research_copilot::LocalRetrievalSynthesisResearchCopilotRequest;
    use crate::retrieval_synthesis::{
        EvidenceSynthesisRequest, RetrievalCandidate, ScopedRetrievalQuery,
    };
    use bioprism_foundation::{EvidenceAvailability, PolicyDecision};
    fn request() -> LocalRetrievalSynthesisResearchWorkbenchRequest {
        let h = ContentHash::of_bytes(b"f17");
        LocalRetrievalSynthesisResearchWorkbenchRequest {
            copilot_request: LocalRetrievalSynthesisResearchCopilotRequest {
                synthesis_request: EvidenceSynthesisRequest {
                    request_id: "request:f17".into(),
                    query: ScopedRetrievalQuery {
                        query_id: "query:f17".into(),
                        requester: "researcher:f17".into(),
                        intent: "retrieve preclinical evidence".into(),
                        study_ids: vec!["study:f17".into()],
                        required_modalities: vec!["imaging".into()],
                        comparability_profile: "profile:f17".into(),
                        max_results: 4,
                    },
                    candidates: vec![RetrievalCandidate {
                        evidence_id: "evidence:f17".into(),
                        study_id: "study:f17".into(),
                        modality: "imaging".into(),
                        comparability_profile: "profile:f17".into(),
                        digest: Some(h.clone()),
                        availability: EvidenceAvailability::Available,
                        relevance_score: 90,
                        negative_result: true,
                        locator: "local://f17".into(),
                    }],
                    policy_decision: PolicyDecision::Allow,
                    protected_closure_satisfied: true,
                    raw_data_local: true,
                    boundary: PRECLINICAL_BOUNDARY.into(),
                },
                copilot_id: "copilot:f17".into(),
                agent_id: "agent:f17".into(),
                recommendation_mode: "evidence-ranked-read-only".into(),
                approval_required: true,
                schema_profile: "ScopedRetrievalQuery@1".into(),
                canonicalization: "aurora-json-canonical-v1".into(),
                consumer: "preclinical-researcher".into(),
                algorithm_version: "f17".into(),
                requested_output: "EvidenceSynthesis1@1".into(),
                budget_units: 8,
                replay_identity: h.clone(),
                boundary: PRECLINICAL_BOUNDARY.into(),
            },
            workspace_id: "workspace:f17".into(),
            scope: "scope:f17".into(),
            requested_view_order: CANONICAL_VIEWS.iter().map(|x| (*x).into()).collect(),
            requested_panel_order: CANONICAL_PANELS.iter().map(|x| (*x).into()).collect(),
            budget_units: 8,
            replay_identity: h,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a0() {
        assert_eq!(
            local_retrieval_synthesis_research_workbench_manifest().autonomy_tier,
            AutonomyTier::A0
        )
    }
    #[test]
    fn renders_workbench() {
        let r = render_local_retrieval_synthesis_research_workbench(&request()).unwrap();
        assert_eq!(r.feature_id, FEATURE_ID);
        assert_eq!(r.view_order.len(), 4);
        assert_eq!(r.panel_order.len(), 4)
    }
    #[test]
    fn preserves_negative() {
        let r = render_local_retrieval_synthesis_research_workbench(&request()).unwrap();
        assert_eq!(r.negative_order, vec!["evidence:f17"])
    }
    #[test]
    fn rejects_view_order() {
        let mut q = request();
        q.requested_view_order.reverse();
        assert!(render_local_retrieval_synthesis_research_workbench(&q).is_err())
    }
    #[test]
    fn rejects_nonlocal() {
        let mut q = request();
        q.copilot_request.synthesis_request.raw_data_local = false;
        assert!(render_local_retrieval_synthesis_research_workbench(&q).is_err())
    }
    #[test]
    fn replay_stable() {
        let a = render_local_retrieval_synthesis_research_workbench(&request()).unwrap();
        let b = render_local_retrieval_synthesis_research_workbench(&request()).unwrap();
        assert_eq!(a.workbench_digest, b.workbench_digest)
    }
}
