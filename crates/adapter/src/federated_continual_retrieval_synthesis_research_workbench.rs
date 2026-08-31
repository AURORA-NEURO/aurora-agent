//! Federated continual autonomous retrieval-and-synthesis researcher workbench.
//! Atlas feature `AFA-adapter-P02-F20`.
use crate::federated_continual_retrieval_synthesis_workflow_fabric::{
    schedule_federated_continual_retrieval_synthesis_workflow,
    FederatedContinualRetrievalSynthesisWorkflowRequest,
};
use crate::retrieval_synthesis::EvidenceSynthesisDisposition;
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
pub const FEATURE_ID: &str = "AFA-adapter-P02-F20";
pub const CONTRACT_VERSION: &str =
    "adapter-federated-continual-retrieval-synthesis-research-workbench/1.0";
pub const INPUT_SCHEMA: &str = "ScopedRetrievalQuery4@1";
pub const OUTPUT_SCHEMA: &str = "EvidenceSynthesis5@1";
pub const CANONICAL_VIEWS: [&str; 4] = [
    "view:overview",
    "view:peers",
    "view:omissions",
    "view:provenance",
];
pub const CANONICAL_PANELS: [&str; 4] = [
    "panel:aggregate",
    "panel:negative",
    "panel:provenance",
    "panel:quorum",
];
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContinualRetrievalSynthesisResearchWorkbenchRequest {
    pub workflow_request: FederatedContinualRetrievalSynthesisWorkflowRequest,
    pub workspace_id: String,
    pub scope: String,
    pub requested_view_order: Vec<String>,
    pub requested_panel_order: Vec<String>,
    pub budget_units: u32,
    pub replay_identity: ContentHash,
    pub boundary: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContinualRetrievalSynthesisResearchWorkbenchReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workspace_id: String,
    pub workflow_id: String,
    pub query_id: String,
    pub scope: String,
    pub intent: String,
    pub federation_id: String,
    pub purpose: String,
    pub peer_ids: Vec<String>,
    pub min_peer_quorum: u32,
    pub aggregate_only: bool,
    pub endpoint: String,
    pub disposition: EvidenceSynthesisDisposition,
    pub view_order: Vec<String>,
    pub panel_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub omitted_order: Vec<String>,
    pub overflow_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_order: Vec<String>,
    pub contradictory_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub workflow_run_digest: ContentHash,
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
pub enum FederatedContinualRetrievalSynthesisResearchWorkbenchError {
    #[error("invalid federated workbench request: {0}")]
    Invalid(String),
    #[error("federated workbench artifact failed: {0}")]
    Artifact(String),
    #[error("federated workbench workflow failed: {0}")]
    Workflow(String),
}
impl FederatedContinualRetrievalSynthesisResearchWorkbenchReceipt {
    pub fn validate(
        &self,
    ) -> Result<(), FederatedContinualRetrievalSynthesisResearchWorkbenchError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workspace_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.query_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.peer_ids.len() < (self.min_peer_quorum as usize)
            || !self.aggregate_only
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
            return Err(FederatedContinualRetrievalSynthesisResearchWorkbenchError::Invalid("federated workbench identity, quorum, views, locality, candidates, or effects are incomplete".into()));
        }
        if self.peer_ids.windows(2).any(|p| p[0] >= p[1]) {
            return Err(
                FederatedContinualRetrievalSynthesisResearchWorkbenchError::Invalid(
                    "peer order is not canonical".into(),
                ),
            );
        }
        for values in [
            &self.candidate_order,
            &self.selected_order,
            &self.omitted_order,
            &self.overflow_order,
            &self.uncertainty_order,
            &self.negative_order,
            &self.contradictory_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|p| p[0] >= p[1]) {
                return Err(
                    FederatedContinualRetrievalSynthesisResearchWorkbenchError::Invalid(
                        "federated workbench ordering is not canonical".into(),
                    ),
                );
            }
        }
        let classified = self
            .selected_order
            .iter()
            .chain(self.omitted_order.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        if classified != self.candidate_order.iter().cloned().collect() {
            return Err(
                FederatedContinualRetrievalSynthesisResearchWorkbenchError::Invalid(
                    "federated workbench evidence states do not partition candidates".into(),
                ),
            );
        }
        if !self
            .overflow_order
            .iter()
            .all(|x| self.omitted_order.contains(x))
        {
            return Err(
                FederatedContinualRetrievalSynthesisResearchWorkbenchError::Invalid(
                    "federated overflow must be omitted".into(),
                ),
            );
        }
        for value in [
            &self.replay_identity,
            &self.workflow_run_digest,
            &self.workbench_digest,
            &self.artifact.content_hash,
        ] {
            if value.as_str().len() != 64 {
                return Err(
                    FederatedContinualRetrievalSynthesisResearchWorkbenchError::Invalid(
                        "federated workbench digest is invalid".into(),
                    ),
                );
            }
        }
        if self.effect_receipts.iter().any(|e| {
            !e.starts_with("view:federated-retrieval-workbench:") && e != "block:unsafe-release"
        }) {
            return Err(
                FederatedContinualRetrievalSynthesisResearchWorkbenchError::Invalid(
                    "federated workbench effect is outside read-only gate".into(),
                ),
            );
        }
        self.artifact.validate_metadata().map_err(|e| {
            FederatedContinualRetrievalSynthesisResearchWorkbenchError::Artifact(e.to_string())
        })
    }
}
pub fn federated_continual_retrieval_synthesis_research_workbench_manifest() -> CapabilityManifest {
    CapabilityManifest{schema_version:RESEARCH_CONTRACT_SCHEMA_VERSION.into(),capability_id:FEATURE_ID.into(),version:CONTRACT_VERSION.into(),owner_crate:"adapter".into(),consumers:["consortium administrator".into(),"preclinical researcher".into()].into(),behavior:"renders a purpose-bound federated continual retrieval synthesis workbench with peer, aggregate, quorum, omission, and provenance views".into(),value:"provides an auditable A1 researcher interaction without moving raw observations or hiding quorum and federation denials".into(),inputs:vec![TypedPort{name:"scoped_retrieval_query".into(),schema:INPUT_SCHEMA.into(),required:true}],outputs:vec![TypedPort{name:"evidence_synthesis".into(),schema:OUTPUT_SCHEMA.into(),required:true}],effects:[Effect::ReadLocalData,Effect::ExecuteLocalComputation].into(),permissions:["view:authorized-research-state".into()].into(),determinism:Determinism::ByteStable,evidence:vec![EvidenceReference{source_id:"w3c-prov-o".into(),state:EvidenceState::Supported,locator:Some("https://www.w3.org/TR/prov-o/".into())}],authority_requirements:Vec::new(),autonomy_tier:AutonomyTier::A1,surfaces:[ResearchSurface::Ui,ResearchSurface::Api,ResearchSurface::Sdk,ResearchSurface::Cli,ResearchSurface::Operator].into(),boundary:PRECLINICAL_BOUNDARY.into()}
}
pub fn render_federated_continual_retrieval_synthesis_research_workbench(
    request: &FederatedContinualRetrievalSynthesisResearchWorkbenchRequest,
) -> Result<
    FederatedContinualRetrievalSynthesisResearchWorkbenchReceipt,
    FederatedContinualRetrievalSynthesisResearchWorkbenchError,
> {
    validate_request(request)?;
    let workflow =
        schedule_federated_continual_retrieval_synthesis_workflow(&request.workflow_request)
            .map_err(|e| {
                FederatedContinualRetrievalSynthesisResearchWorkbenchError::Workflow(e.to_string())
            })?;
    let workflow_run_digest =
        ContentHash::of_value(&serde_json::to_value(&workflow).map_err(|e| {
            FederatedContinualRetrievalSynthesisResearchWorkbenchError::Artifact(e.to_string())
        })?)
        .map_err(|e| {
            FederatedContinualRetrievalSynthesisResearchWorkbenchError::Artifact(e.to_string())
        })?;
    let workbench_digest=ContentHash::of_value(&json!({"workspace_id":request.workspace_id,"scope":request.scope,"federation_id":workflow.federation_id,"purpose":workflow.purpose,"peer_ids":workflow.peer_ids,"quorum":workflow.min_peer_quorum,"aggregate_only":workflow.aggregate_only,"views":CANONICAL_VIEWS,"panels":CANONICAL_PANELS,"candidate_order":workflow.candidate_order,"selected_order":workflow.selected_order,"omitted_order":workflow.omitted_order,"replay_identity":request.replay_identity,"workflow_run_digest":workflow_run_digest})).map_err(|e|FederatedContinualRetrievalSynthesisResearchWorkbenchError::Artifact(e.to_string()))?;
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":workflow.request_id,"workspace_id":request.workspace_id,"workflow_id":workflow.workflow_id,"query_id":workflow.query_id,"scope":request.scope,"intent":request.workflow_request.request.synthesis_request.query.intent,"federation_id":workflow.federation_id,"purpose":workflow.purpose,"peer_ids":workflow.peer_ids,"min_peer_quorum":workflow.min_peer_quorum,"aggregate_only":workflow.aggregate_only,"endpoint":workflow.endpoint,"disposition":workflow.disposition,"view_order":CANONICAL_VIEWS,"panel_order":CANONICAL_PANELS,"candidate_order":workflow.candidate_order,"selected_order":workflow.selected_order,"omitted_order":workflow.omitted_order,"overflow_order":workflow.overflow_order,"uncertainty_order":workflow.uncertainty_order,"negative_order":workflow.negative_order,"contradictory_order":workflow.contradictory_order,"replay_identity":request.replay_identity,"workflow_run_digest":workflow_run_digest,"workbench_digest":workbench_digest,"omissions":workflow.omissions,"uncertainty":workflow.uncertainty,"negative_evidence":workflow.negative_evidence,"raw_data_local":true,"boundary":PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "adapter-federated-retrieval-workbench:{}",
            request.workspace_id
        ),
        "application/vnd.aurora.federated-continual-retrieval-synthesis-research-workbench+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|e| {
        FederatedContinualRetrievalSynthesisResearchWorkbenchError::Artifact(e.to_string())
    })?;
    let receipt = FederatedContinualRetrievalSynthesisResearchWorkbenchReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: workflow.request_id.clone(),
        workspace_id: request.workspace_id.clone(),
        workflow_id: workflow.workflow_id.clone(),
        query_id: workflow.query_id.clone(),
        scope: request.scope.clone(),
        intent: request
            .workflow_request
            .request
            .synthesis_request
            .query
            .intent
            .clone(),
        federation_id: workflow.federation_id.clone(),
        purpose: workflow.purpose.clone(),
        peer_ids: workflow.peer_ids.clone(),
        min_peer_quorum: workflow.min_peer_quorum,
        aggregate_only: workflow.aggregate_only,
        endpoint: workflow.endpoint.clone(),
        disposition: workflow.disposition,
        view_order: CANONICAL_VIEWS.iter().map(|x| (*x).into()).collect(),
        panel_order: CANONICAL_PANELS.iter().map(|x| (*x).into()).collect(),
        candidate_order: workflow.candidate_order.clone(),
        selected_order: workflow.selected_order.clone(),
        omitted_order: workflow.omitted_order.clone(),
        overflow_order: workflow.overflow_order.clone(),
        uncertainty_order: workflow.uncertainty_order.clone(),
        negative_order: workflow.negative_order.clone(),
        contradictory_order: workflow.contradictory_order.clone(),
        replay_identity: request.replay_identity.clone(),
        workflow_run_digest,
        workbench_digest,
        omissions: workflow.omissions.clone(),
        uncertainty: workflow.uncertainty.clone(),
        negative_evidence: workflow.negative_evidence.clone(),
        effect_receipts: vec![format!(
            "view:federated-retrieval-workbench:{}",
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
    request: &FederatedContinualRetrievalSynthesisResearchWorkbenchRequest,
) -> Result<(), FederatedContinualRetrievalSynthesisResearchWorkbenchError> {
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
        || request.workflow_request.boundary != PRECLINICAL_BOUNDARY
        || !request.workflow_request.raw_data_local
    {
        return Err(FederatedContinualRetrievalSynthesisResearchWorkbenchError::Invalid("federated workbench identity, views, budget, locality, replay, or boundary is invalid".into()));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            federated_continual_retrieval_synthesis_research_workbench_manifest().autonomy_tier,
            AutonomyTier::A1
        );
    }
}
