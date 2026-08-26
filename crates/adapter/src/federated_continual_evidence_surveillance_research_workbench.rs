//! Federated continual evidence-surveillance researcher workbench.
//! Atlas feature `AFA-adapter-P01-F20`: an A1 read-only view over signed,
//! permitted aggregate contributions; raw observations never enter the view artifact.

use crate::federated_continual_evidence_surveillance_research_copilot::{
    run_federated_continual_evidence_surveillance_research_copilot,
    FederatedContinualEvidenceSurveillanceResearchCopilotRequest,
    FederatedContinualResearchCopilotDisposition,
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

pub const FEATURE_ID: &str = "AFA-adapter-P01-F20";
pub const CONTRACT_VERSION: &str =
    "adapter-federated-continual-evidence-surveillance-research-workbench/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceFeed4@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet5@1";
const VIEWS: [&str; 4] = [
    "view:peers",
    "view:aggregate",
    "view:omissions",
    "view:provenance",
];
const PANELS: [&str; 4] = [
    "panel:denied",
    "panel:negative",
    "panel:qualified",
    "panel:unknown",
];
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContinualEvidenceSurveillanceResearchWorkbenchRequest {
    pub copilot_request: FederatedContinualEvidenceSurveillanceResearchCopilotRequest,
    pub workbench_id: String,
    pub scope: String,
    pub requested_view_order: Vec<String>,
    pub requested_panel_order: Vec<String>,
    pub budget_units: u32,
    pub replay_identity: ContentHash,
    pub boundary: String,
}
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedContinualEvidenceSurveillanceResearchWorkbenchReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workbench_id: String,
    pub scope: String,
    pub federation_id: String,
    pub purpose: String,
    pub endpoint: String,
    pub disposition: FederatedContinualResearchCopilotDisposition,
    pub view_order: Vec<String>,
    pub panel_order: Vec<String>,
    pub peer_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub aggregate_order: Vec<String>,
    pub replay_identity: ContentHash,
    pub copilot_run_digest: ContentHash,
    pub workbench_digest: ContentHash,
    pub federation_digest: ContentHash,
    pub envelope_digest: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum FederatedContinualEvidenceSurveillanceResearchWorkbenchError {
    #[error("invalid federated workbench request: {0}")]
    Invalid(String),
    #[error("federated workbench artifact failed: {0}")]
    Artifact(String),
    #[error("federated workbench copilot failed: {0}")]
    Copilot(String),
}
impl FederatedContinualEvidenceSurveillanceResearchWorkbenchReceipt {
    pub fn validate(
        &self,
    ) -> Result<(), FederatedContinualEvidenceSurveillanceResearchWorkbenchError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workbench_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.endpoint.trim().is_empty()
            || self.view_order != VIEWS.iter().map(|v| (*v).to_string()).collect::<Vec<_>>()
            || self.panel_order != PANELS.iter().map(|v| (*v).to_string()).collect::<Vec<_>>()
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(FederatedContinualEvidenceSurveillanceResearchWorkbenchError::Invalid("federated workbench identity, canonical views, locality, candidates, or effects are incomplete".into()));
        }
        for values in [
            &self.peer_order,
            &self.candidate_order,
            &self.qualified_order,
            &self.unknown_order,
            &self.blocked_order,
            &self.aggregate_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|p| p[0] >= p[1]) {
                return Err(
                    FederatedContinualEvidenceSurveillanceResearchWorkbenchError::Invalid(
                        "federated workbench ordering is not canonical".into(),
                    ),
                );
            }
        }
        let classified = self
            .qualified_order
            .iter()
            .chain(self.unknown_order.iter())
            .chain(self.blocked_order.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        if classified != self.candidate_order.iter().cloned().collect() {
            return Err(
                FederatedContinualEvidenceSurveillanceResearchWorkbenchError::Invalid(
                    "federated workbench states do not partition candidates".into(),
                ),
            );
        }
        for value in [
            &self.replay_identity,
            &self.copilot_run_digest,
            &self.workbench_digest,
            &self.federation_digest,
            &self.envelope_digest,
            &self.artifact.content_hash,
        ] {
            if value.as_str().len() != 64 {
                return Err(
                    FederatedContinualEvidenceSurveillanceResearchWorkbenchError::Invalid(
                        "federated workbench digest is invalid".into(),
                    ),
                );
            }
        }
        if self.effect_receipts.iter().any(|e| {
            !e.starts_with("view:federated-evidence-workbench:") && e != "block:unsafe-release"
        }) {
            return Err(
                FederatedContinualEvidenceSurveillanceResearchWorkbenchError::Invalid(
                    "federated workbench effect is outside read-only gate".into(),
                ),
            );
        }
        self.artifact.validate_metadata().map_err(|e| {
            FederatedContinualEvidenceSurveillanceResearchWorkbenchError::Artifact(e.to_string())
        })
    }
}
pub fn federated_continual_evidence_surveillance_research_workbench_manifest() -> CapabilityManifest
{
    CapabilityManifest{schema_version:RESEARCH_CONTRACT_SCHEMA_VERSION.into(),capability_id:FEATURE_ID.into(),version:CONTRACT_VERSION.into(),owner_crate:"adapter".into(),consumers:["preclinical researcher".into(),"consortium administrator".into()].into(),behavior:"renders a deterministic federated continual EvidenceFeed4 workbench with peer, aggregate, omission, denied, unknown, negative, qualified, and provenance panels without moving raw observations".into(),value:"gives preclinical researchers an accessible policy-separated view of permitted cross-institution evidence while retaining signer, locality, quorum, and negative-result evidence".into(),inputs:vec![TypedPort{name:"federated_evidence_workbench_request".into(),schema:INPUT_SCHEMA.into(),required:true}],outputs:vec![TypedPort{name:"qualified_federated_evidence_workbench_set".into(),schema:OUTPUT_SCHEMA.into(),required:true}],effects:[Effect::ReadLocalData,Effect::ExecuteLocalComputation].into(),permissions:["view:authorized-research-state".into()].into(),determinism:Determinism::ByteStable,evidence:vec![EvidenceReference{source_id:"json-schema".into(),state:EvidenceState::Supported,locator:Some("https://json-schema.org/specification".into())}],authority_requirements:Vec::new(),autonomy_tier:AutonomyTier::A1,surfaces:[ResearchSurface::Ui,ResearchSurface::Api,ResearchSurface::Sdk,ResearchSurface::Cli,ResearchSurface::Operator].into(),boundary:PRECLINICAL_BOUNDARY.into()}
}
pub fn render_federated_continual_evidence_surveillance_research_workbench(
    request: &FederatedContinualEvidenceSurveillanceResearchWorkbenchRequest,
) -> Result<
    FederatedContinualEvidenceSurveillanceResearchWorkbenchReceipt,
    FederatedContinualEvidenceSurveillanceResearchWorkbenchError,
> {
    validate_request(request)?;
    let c =
        run_federated_continual_evidence_surveillance_research_copilot(&request.copilot_request)
            .map_err(|e| {
                FederatedContinualEvidenceSurveillanceResearchWorkbenchError::Copilot(e.to_string())
            })?;
    let views = VIEWS.iter().map(|v| (*v).to_string()).collect::<Vec<_>>();
    let panels = PANELS.iter().map(|v| (*v).to_string()).collect::<Vec<_>>();
    let candidate = c.candidate_order.clone();
    let qualified = c.selected_order.clone();
    let unknown = c.unresolved_order.clone();
    let blocked = c.denied_order.clone();
    let aggregate = c.aggregate_order.clone();
    let cv = serde_json::to_value(&c).map_err(|e| {
        FederatedContinualEvidenceSurveillanceResearchWorkbenchError::Artifact(e.to_string())
    })?;
    let copilot_run_digest = ContentHash::of_value(&cv).map_err(|e| {
        FederatedContinualEvidenceSurveillanceResearchWorkbenchError::Artifact(e.to_string())
    })?;
    let workbench_digest=ContentHash::of_value(&json!({"workbench_id":request.workbench_id,"scope":request.scope,"federation_id":request.copilot_request.federation_id,"purpose":request.copilot_request.purpose,"views":views,"panels":panels,"candidate":candidate,"qualified":qualified,"unknown":unknown,"blocked":blocked,"aggregate":aggregate,"replay_identity":request.replay_identity,"copilot_run_digest":copilot_run_digest})).map_err(|e|FederatedContinualEvidenceSurveillanceResearchWorkbenchError::Artifact(e.to_string()))?;
    let mut omissions = c.omissions.clone();
    omissions.push("workbench:read-only-federated-view".into());
    omissions.sort();
    omissions.dedup();
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.copilot_request.request_id,"workbench_id":request.workbench_id,"scope":request.scope,"federation_id":request.copilot_request.federation_id,"purpose":request.copilot_request.purpose,"endpoint":request.copilot_request.endpoint,"disposition":c.disposition,"view_order":views,"panel_order":panels,"peer_order":c.peer_order,"candidate_order":candidate,"qualified_order":qualified,"unknown_order":unknown,"blocked_order":blocked,"aggregate_order":aggregate,"replay_identity":request.replay_identity,"copilot_run_digest":copilot_run_digest,"workbench_digest":workbench_digest,"federation_digest":c.federation_digest,"envelope_digest":c.envelope_digest,"omissions":omissions,"uncertainty":c.uncertainty,"negative_evidence":c.negative_evidence,"boundary":PRECLINICAL_BOUNDARY,"raw_data_local":true});
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "adapter-federated-evidence-workbench:{}",
            request.workbench_id
        ),
        "application/vnd.aurora.federated-evidence-workbench+json",
        &payload,
        vec![],
        vec![],
    )
    .map_err(|e| {
        FederatedContinualEvidenceSurveillanceResearchWorkbenchError::Artifact(e.to_string())
    })?;
    let receipt = FederatedContinualEvidenceSurveillanceResearchWorkbenchReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.copilot_request.request_id.clone(),
        workbench_id: request.workbench_id.clone(),
        scope: request.scope.clone(),
        federation_id: request.copilot_request.federation_id.clone(),
        purpose: request.copilot_request.purpose.clone(),
        endpoint: request.copilot_request.endpoint.clone(),
        disposition: c.disposition,
        view_order: views,
        panel_order: panels,
        peer_order: c.peer_order.clone(),
        candidate_order: candidate,
        qualified_order: qualified,
        unknown_order: unknown,
        blocked_order: blocked,
        aggregate_order: aggregate,
        replay_identity: request.replay_identity.clone(),
        copilot_run_digest,
        workbench_digest,
        federation_digest: c.federation_digest.clone(),
        envelope_digest: c.envelope_digest.clone(),
        omissions,
        uncertainty: c.uncertainty.clone(),
        negative_evidence: c.negative_evidence.clone(),
        effect_receipts: vec![format!(
            "view:federated-evidence-workbench:{}",
            request.workbench_id
        )],
        artifact,
        raw_data_local: true,
        boundary: request.boundary.clone(),
    };
    receipt.validate()?;
    Ok(receipt)
}
fn validate_request(
    r: &FederatedContinualEvidenceSurveillanceResearchWorkbenchRequest,
) -> Result<(), FederatedContinualEvidenceSurveillanceResearchWorkbenchError> {
    if r.workbench_id.trim().is_empty()
        || r.scope.trim().is_empty()
        || r.budget_units == 0
        || r.boundary != PRECLINICAL_BOUNDARY
        || r.copilot_request.boundary != PRECLINICAL_BOUNDARY
        || !r.copilot_request.raw_data_local
        || !r.copilot_request.dry_run
    {
        return Err(
            FederatedContinualEvidenceSurveillanceResearchWorkbenchError::Invalid(
                "federated workbench identity, budget, dry-run, locality, or boundary is invalid"
                    .into(),
            ),
        );
    }
    if r.requested_view_order != VIEWS.iter().map(|v| (*v).to_string()).collect::<Vec<_>>()
        || r.requested_panel_order != PANELS.iter().map(|v| (*v).to_string()).collect::<Vec<_>>()
        || r.replay_identity.as_str().len() != 64
        || r.copilot_request.replay_identity.as_str().len() != 64
    {
        return Err(
            FederatedContinualEvidenceSurveillanceResearchWorkbenchError::Invalid(
                "federated workbench views, panels, or replay identity is invalid".into(),
            ),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::federated_continual_evidence_surveillance_research_copilot::FederatedCopilotEvidenceContribution;
    fn request() -> FederatedContinualEvidenceSurveillanceResearchWorkbenchRequest {
        let c = FederatedContinualEvidenceSurveillanceResearchCopilotRequest {
            request_id: "req-20".into(),
            agent_id: "researcher-20".into(),
            federation_id: "federation-20".into(),
            purpose: "evidence surveillance".into(),
            endpoint: "local://federation".into(),
            semantic_profile: "profile-v1".into(),
            allowed_artifacts: vec!["qualified-evidence".into()],
            min_peer_quorum: 2,
            declared_tools: vec!["evidence.inspect".into()],
            requested_tool: "evidence.inspect".into(),
            max_tool_calls: 1,
            dry_run: true,
            approval_reference: None,
            approval_granted: false,
            contributions: vec![
                FederatedCopilotEvidenceContribution {
                    peer_id: "peer-a".into(),
                    institution_id: "inst-a".into(),
                    source_id: "source-a".into(),
                    semantic_profile: "profile-v1".into(),
                    artifact_kind: "qualified-evidence".into(),
                    digest: Some(ContentHash::of_bytes(b"a")),
                    signed: true,
                    permitted_artifact: true,
                    aggregate_only: true,
                    evidence_state: EvidenceState::Supported,
                    negative_result: false,
                },
                FederatedCopilotEvidenceContribution {
                    peer_id: "peer-b".into(),
                    institution_id: "inst-b".into(),
                    source_id: "source-b".into(),
                    semantic_profile: "profile-v1".into(),
                    artifact_kind: "qualified-evidence".into(),
                    digest: Some(ContentHash::of_bytes(b"b")),
                    signed: true,
                    permitted_artifact: true,
                    aggregate_only: true,
                    evidence_state: EvidenceState::Supported,
                    negative_result: false,
                },
            ],
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            replay_identity: ContentHash::of_bytes(b"copilot-20"),
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        FederatedContinualEvidenceSurveillanceResearchWorkbenchRequest {
            copilot_request: c,
            workbench_id: "wb-20".into(),
            scope: "federation:federation-20".into(),
            requested_view_order: VIEWS.iter().map(|v| (*v).to_string()).collect(),
            requested_panel_order: PANELS.iter().map(|v| (*v).to_string()).collect(),
            budget_units: 4,
            replay_identity: ContentHash::of_bytes(b"wb-20"),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            federated_continual_evidence_surveillance_research_workbench_manifest().autonomy_tier,
            AutonomyTier::A1
        )
    }
    #[test]
    fn renders_view() {
        let r = render_federated_continual_evidence_surveillance_research_workbench(&request())
            .unwrap();
        assert_eq!(r.feature_id, FEATURE_ID)
    }
    #[test]
    fn policy_denial_visible() {
        let mut r = request();
        r.copilot_request.policy_allow = false;
        assert!(render_federated_continual_evidence_surveillance_research_workbench(&r).is_ok())
    }
    #[test]
    fn rejects_non_dry_run() {
        let mut r = request();
        r.copilot_request.dry_run = false;
        assert!(render_federated_continual_evidence_surveillance_research_workbench(&r).is_err())
    }
    #[test]
    fn rejects_panels() {
        let mut r = request();
        r.requested_panel_order.reverse();
        assert!(render_federated_continual_evidence_surveillance_research_workbench(&r).is_err())
    }
    #[test]
    fn replay_stable() {
        let r = request();
        assert_eq!(
            render_federated_continual_evidence_surveillance_research_workbench(&r).unwrap(),
            render_federated_continual_evidence_surveillance_research_workbench(&r).unwrap()
        )
    }
}
