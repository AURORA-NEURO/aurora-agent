//! Prospective high-throughput retrieval-and-synthesis researcher workbench.
//! Atlas feature `AFA-adapter-P02-F19`.
use crate::retrieval_synthesis::EvidenceSynthesisDisposition;
use crate::throughput_retrieval_synthesis_research_copilot::{
    run_throughput_retrieval_synthesis_research_copilot,
    ThroughputRetrievalSynthesisResearchCopilotRequest,
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
pub const FEATURE_ID: &str = "AFA-adapter-P02-F19";
pub const CONTRACT_VERSION: &str = "adapter-throughput-retrieval-synthesis-research-workbench/1.0";
pub const INPUT_SCHEMA: &str = "ScopedRetrievalQuery3@1";
pub const OUTPUT_SCHEMA: &str = "EvidenceSynthesis5@1";
pub const CANONICAL_VIEWS: [&str; 4] = [
    "view:overview",
    "view:queue",
    "view:omissions",
    "view:provenance",
];
pub const CANONICAL_PANELS: [&str; 4] = [
    "panel:negative",
    "panel:overflow",
    "panel:provenance",
    "panel:qualified",
];
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputRetrievalSynthesisResearchWorkbenchRequest {
    pub copilot_request: ThroughputRetrievalSynthesisResearchCopilotRequest,
    pub workspace_id: String,
    pub scope: String,
    pub requested_view_order: Vec<String>,
    pub requested_panel_order: Vec<String>,
    pub budget_units: u32,
    pub replay_identity: ContentHash,
    pub boundary: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputRetrievalSynthesisResearchWorkbenchReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workspace_id: String,
    pub query_id: String,
    pub scope: String,
    pub intent: String,
    pub batch_id: String,
    pub checkpoint_seq: u64,
    pub capacity: u32,
    pub queue_digest: ContentHash,
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
pub enum ThroughputRetrievalSynthesisResearchWorkbenchError {
    #[error("invalid throughput retrieval workbench request: {0}")]
    Invalid(String),
    #[error("throughput retrieval workbench artifact failed: {0}")]
    Artifact(String),
    #[error("throughput retrieval workbench copilot failed: {0}")]
    Copilot(String),
}
impl ThroughputRetrievalSynthesisResearchWorkbenchReceipt {
    pub fn validate(&self) -> Result<(), ThroughputRetrievalSynthesisResearchWorkbenchError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workspace_id.trim().is_empty()
            || self.query_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.batch_id.trim().is_empty()
            || self.checkpoint_seq == 0
            || self.capacity == 0
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
            return Err(ThroughputRetrievalSynthesisResearchWorkbenchError::Invalid("throughput workbench identity, queue, views, candidates, locality, or effects are incomplete".into()));
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
                return Err(ThroughputRetrievalSynthesisResearchWorkbenchError::Invalid(
                    "throughput workbench ordering is not canonical".into(),
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
            return Err(ThroughputRetrievalSynthesisResearchWorkbenchError::Invalid(
                "throughput workbench evidence states do not partition candidates".into(),
            ));
        }
        if !self
            .overflow_order
            .iter()
            .all(|x| self.omitted_order.contains(x))
        {
            return Err(ThroughputRetrievalSynthesisResearchWorkbenchError::Invalid(
                "throughput overflow must be omitted".into(),
            ));
        }
        for value in [
            &self.replay_identity,
            &self.copilot_run_digest,
            &self.workbench_digest,
            &self.queue_digest,
            &self.artifact.content_hash,
        ] {
            if value.as_str().len() != 64 {
                return Err(ThroughputRetrievalSynthesisResearchWorkbenchError::Invalid(
                    "throughput workbench digest is invalid".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|e| {
            !e.starts_with("view:throughput-retrieval-workbench:") && e != "block:unsafe-release"
        }) {
            return Err(ThroughputRetrievalSynthesisResearchWorkbenchError::Invalid(
                "throughput workbench effect is outside read-only gate".into(),
            ));
        }
        self.artifact.validate_metadata().map_err(|e| {
            ThroughputRetrievalSynthesisResearchWorkbenchError::Artifact(e.to_string())
        })
    }
}
pub fn throughput_retrieval_synthesis_research_workbench_manifest() -> CapabilityManifest {
    CapabilityManifest{schema_version:RESEARCH_CONTRACT_SCHEMA_VERSION.into(),capability_id:FEATURE_ID.into(),version:CONTRACT_VERSION.into(),owner_crate:"adapter".into(),consumers:["preclinical researcher".into(),"integration engineer".into()].into(),behavior:"renders a bounded throughput retrieval synthesis workbench with queue, overflow, omission, and provenance views".into(),value:"provides auditable high-throughput research interaction without hiding queue overflow or negative evidence".into(),inputs:vec![TypedPort{name:"scoped_retrieval_query".into(),schema:INPUT_SCHEMA.into(),required:true}],outputs:vec![TypedPort{name:"evidence_synthesis".into(),schema:OUTPUT_SCHEMA.into(),required:true}],effects:[Effect::ReadLocalData,Effect::ExecuteLocalComputation].into(),permissions:["view:authorized-research-state".into()].into(),determinism:Determinism::ByteStable,evidence:vec![EvidenceReference{source_id:"open-telemetry".into(),state:EvidenceState::Supported,locator:Some("https://opentelemetry.io/docs/specs/".into())}],authority_requirements:Vec::new(),autonomy_tier:AutonomyTier::A1,surfaces:[ResearchSurface::Ui,ResearchSurface::Api,ResearchSurface::Sdk,ResearchSurface::Cli,ResearchSurface::Operator].into(),boundary:PRECLINICAL_BOUNDARY.into()}
}
pub fn render_throughput_retrieval_synthesis_research_workbench(
    request: &ThroughputRetrievalSynthesisResearchWorkbenchRequest,
) -> Result<
    ThroughputRetrievalSynthesisResearchWorkbenchReceipt,
    ThroughputRetrievalSynthesisResearchWorkbenchError,
> {
    validate_request(request)?;
    let copilot = run_throughput_retrieval_synthesis_research_copilot(&request.copilot_request)
        .map_err(|e| ThroughputRetrievalSynthesisResearchWorkbenchError::Copilot(e.to_string()))?;
    let candidate_order = copilot.candidate_order.clone();
    let selected_order = copilot.selected_order.clone();
    let omitted_order = copilot.omitted_order.clone();
    let copilot_run_digest =
        ContentHash::of_value(&serde_json::to_value(&copilot).map_err(|e| {
            ThroughputRetrievalSynthesisResearchWorkbenchError::Artifact(e.to_string())
        })?)
        .map_err(|e| ThroughputRetrievalSynthesisResearchWorkbenchError::Artifact(e.to_string()))?;
    let workbench_digest=ContentHash::of_value(&json!({"workspace_id":request.workspace_id,"scope":request.scope,"batch_id":request.copilot_request.batch_id,"checkpoint_seq":request.copilot_request.checkpoint_seq,"capacity":request.copilot_request.capacity,"queue_digest":request.copilot_request.queue_digest,"views":CANONICAL_VIEWS,"panels":CANONICAL_PANELS,"candidate_order":candidate_order,"selected_order":selected_order,"omitted_order":omitted_order,"replay_identity":request.replay_identity,"copilot_run_digest":copilot_run_digest})).map_err(|e|ThroughputRetrievalSynthesisResearchWorkbenchError::Artifact(e.to_string()))?;
    let omissions = omitted_order
        .iter()
        .map(|id| format!("evidence:{id}:omitted"))
        .chain(
            copilot
                .overflow_order
                .iter()
                .map(|id| format!("evidence:{id}:overflow")),
        )
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.copilot_request.synthesis_request.request_id,"workspace_id":request.workspace_id,"query_id":request.copilot_request.synthesis_request.query.query_id,"scope":request.scope,"intent":request.copilot_request.synthesis_request.query.intent,"batch_id":request.copilot_request.batch_id,"checkpoint_seq":request.copilot_request.checkpoint_seq,"capacity":request.copilot_request.capacity,"queue_digest":request.copilot_request.queue_digest,"disposition":copilot.disposition,"view_order":CANONICAL_VIEWS,"panel_order":CANONICAL_PANELS,"candidate_order":candidate_order,"selected_order":selected_order,"omitted_order":omitted_order,"overflow_order":copilot.overflow_order,"uncertainty_order":copilot.uncertainty_order,"negative_order":copilot.negative_order,"contradictory_order":copilot.contradictory_order,"replay_identity":request.replay_identity,"copilot_run_digest":copilot_run_digest,"workbench_digest":workbench_digest,"omissions":omissions,"uncertainty":copilot.uncertainty_order,"negative_evidence":copilot.negative_order,"raw_data_local":true,"boundary":PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "adapter-throughput-retrieval-workbench:{}",
            request.workspace_id
        ),
        "application/vnd.aurora.throughput-retrieval-synthesis-research-workbench+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|e| ThroughputRetrievalSynthesisResearchWorkbenchError::Artifact(e.to_string()))?;
    let receipt = ThroughputRetrievalSynthesisResearchWorkbenchReceipt {
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
        batch_id: request.copilot_request.batch_id.clone(),
        checkpoint_seq: request.copilot_request.checkpoint_seq,
        capacity: request.copilot_request.capacity,
        queue_digest: request.copilot_request.queue_digest.clone(),
        disposition: copilot.disposition,
        view_order: CANONICAL_VIEWS.iter().map(|x| (*x).into()).collect(),
        panel_order: CANONICAL_PANELS.iter().map(|x| (*x).into()).collect(),
        candidate_order,
        selected_order,
        omitted_order,
        overflow_order: copilot.overflow_order.clone(),
        uncertainty_order: copilot.uncertainty_order.clone(),
        negative_order: copilot.negative_order.clone(),
        contradictory_order: copilot.contradictory_order.clone(),
        replay_identity: request.replay_identity.clone(),
        copilot_run_digest,
        workbench_digest,
        omissions,
        uncertainty: copilot.uncertainty_order.clone(),
        negative_evidence: copilot.negative_order.clone(),
        effect_receipts: vec![format!(
            "view:throughput-retrieval-workbench:{}",
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
    request: &ThroughputRetrievalSynthesisResearchWorkbenchRequest,
) -> Result<(), ThroughputRetrievalSynthesisResearchWorkbenchError> {
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
        return Err(ThroughputRetrievalSynthesisResearchWorkbenchError::Invalid("throughput workbench identity, views, budget, locality, replay, or boundary is invalid".into()));
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            throughput_retrieval_synthesis_research_workbench_manifest().autonomy_tier,
            AutonomyTier::A1
        );
    }
}
