//! Multimodal retrieval/synthesis federated control plane.
//! Atlas feature `AFA-adapter-P02-F30`.

use crate::multimodal_retrieval_synthesis_research_workbench::{
    render_multimodal_retrieval_synthesis_research_workbench,
    MultimodalRetrievalSynthesisResearchWorkbenchRequest,
};
use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P02-F30";
pub const CONTRACT_VERSION: &str =
    "adapter-multimodal-retrieval-synthesis-federated-control-plane/1.0";
pub const INPUT_SCHEMA: &str = "ScopedRetrievalQuery2@1";
pub const OUTPUT_SCHEMA: &str = "EvidenceSynthesis5@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalRetrievalSynthesisFederatedControlPlaneRequest {
    pub workbench_request: MultimodalRetrievalSynthesisResearchWorkbenchRequest,
    pub service_id: String,
    pub federation_id: String,
    pub purpose: String,
    pub peer_ids: Vec<String>,
    pub min_peer_quorum: u32,
    pub capacity: u32,
    pub active_runs: u32,
    pub signed_approval: bool,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub federation_permitted: bool,
    pub expected_scope: String,
    pub expected_modalities: Vec<String>,
    pub expected_comparability_digest: ContentHash,
    pub health_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalRetrievalSynthesisFederatedControlPlaneReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub service_id: String,
    pub federation_id: String,
    pub purpose: String,
    pub peer_ids: Vec<String>,
    pub min_peer_quorum: u32,
    pub aggregate_only: bool,
    pub capacity: u32,
    pub active_runs: u32,
    pub admission: String,
    pub scope: String,
    pub required_modalities: Vec<String>,
    pub comparability_digest: ContentHash,
    pub workbench_digest: ContentHash,
    pub health_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub control_digest: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub counterexamples: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum MultimodalRetrievalSynthesisFederatedControlPlaneError {
    #[error("invalid multimodal federated control request: {0}")]
    Invalid(String),
    #[error("multimodal federated control artifact failed: {0}")]
    Artifact(String),
    #[error("multimodal federated control workbench failed: {0}")]
    Workbench(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}
fn saturated(active: u32, capacity: u32) -> bool {
    active.saturating_mul(100) >= capacity.saturating_mul(90)
}

impl MultimodalRetrievalSynthesisFederatedControlPlaneReceipt {
    pub fn validate(&self) -> Result<(), MultimodalRetrievalSynthesisFederatedControlPlaneError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.service_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.peer_ids.len() < self.min_peer_quorum as usize
            || self.min_peer_quorum == 0
            || !canonical(&self.peer_ids)
            || !self.aggregate_only
            || self.capacity == 0
            || self.active_runs > self.capacity
            || self.admission.is_empty()
            || self.scope.trim().is_empty()
            || self.required_modalities.len() < 2
            || !canonical(&self.required_modalities)
            || self.effect_receipts.is_empty()
        {
            return Err(Self::invalid("multimodal federated control identity, quorum, modality closure, capacity, locality, or effects are incomplete"));
        }
        if !matches!(
            self.admission.as_str(),
            "admitted" | "degraded" | "approval_required" | "blocked"
        ) {
            return Err(Self::invalid(
                "multimodal federated control admission is unknown",
            ));
        }
        for values in [
            &self.omissions,
            &self.uncertainty,
            &self.counterexamples,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(Self::invalid(
                    "multimodal federated control ordering is not canonical",
                ));
            }
        }
        for digest in [
            &self.comparability_digest,
            &self.workbench_digest,
            &self.health_digest,
            &self.replay_identity,
            &self.control_digest,
            &self.artifact.content_hash,
        ] {
            if digest.as_str().len() != 64 {
                return Err(Self::invalid(
                    "multimodal federated control digest is invalid",
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("operate:multimodal-federated-control-plane:")
                && !effect.starts_with("approval-required:")
                && effect != "block:unsafe-release"
        }) {
            return Err(Self::invalid(
                "multimodal federated control effect is outside admission gate",
            ));
        }
        self.artifact.validate_metadata().map_err(|error| {
            MultimodalRetrievalSynthesisFederatedControlPlaneError::Artifact(error.to_string())
        })
    }
    fn invalid(message: &str) -> MultimodalRetrievalSynthesisFederatedControlPlaneError {
        MultimodalRetrievalSynthesisFederatedControlPlaneError::Invalid(message.into())
    }
}

pub fn multimodal_retrieval_synthesis_federated_control_plane_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version:RESEARCH_CONTRACT_SCHEMA_VERSION.into(),capability_id:FEATURE_ID.into(),version:CONTRACT_VERSION.into(),owner_crate:"adapter".into(),consumers:["integration engineer".into(),"consortium administrator".into()].into(),behavior:"operates an A2 multimodal federated retrieval/synthesis control plane with comparability, peer quorum, aggregate-only locality, signed approval, health, capacity, and fail-closed admission receipts".into(),value:"enables governed multi-institution imaging and omics workflows without allowing incomparable evidence or unauthorized raw-data movement".into(),inputs:vec![TypedPort{name:"multimodal_scoped_retrieval_query".into(),schema:INPUT_SCHEMA.into(),required:true}],outputs:vec![TypedPort{name:"controlled_multimodal_evidence_synthesis".into(),schema:OUTPUT_SCHEMA.into(),required:true}],effects:[Effect::ReadLocalData,Effect::ExecuteLocalComputation,Effect::WriteLocalArtifact].into(),permissions:["operate:multimodal-federated-control-plane".into()].into(),determinism:Determinism::ByteStable,evidence:vec![EvidenceReference{source_id:"ome-ngff-rfc5".into(),state:EvidenceState::Supported,locator:Some("https://ngff.openmicroscopy.org/rfc/5/".into())},EvidenceReference{source_id:"anndata".into(),state:EvidenceState::Supported,locator:Some("https://anndata.readthedocs.io/en/stable/fileformat-prose.html".into())}],authority_requirements:Vec::new(),autonomy_tier:AutonomyTier::A2,surfaces:[ResearchSurface::Ui,ResearchSurface::Api,ResearchSurface::Sdk,ResearchSurface::Cli,ResearchSurface::McpTool,ResearchSurface::Policy,ResearchSurface::Operator].into(),boundary:PRECLINICAL_BOUNDARY.into() }
}

pub fn operate_multimodal_retrieval_synthesis_federated_control_plane(
    request: &MultimodalRetrievalSynthesisFederatedControlPlaneRequest,
) -> Result<
    MultimodalRetrievalSynthesisFederatedControlPlaneReceipt,
    MultimodalRetrievalSynthesisFederatedControlPlaneError,
> {
    if request.service_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.expected_scope.trim().is_empty()
        || request.peer_ids.len() < request.min_peer_quorum as usize
        || request.min_peer_quorum == 0
        || !canonical(&request.peer_ids)
        || request.capacity == 0
        || request.active_runs > request.capacity
        || request.expected_modalities.len() < 2
        || !canonical(&request.expected_modalities)
        || request.expected_comparability_digest.as_str().len() != 64
        || request.health_digest.as_str().len() != 64
        || request.replay_identity.as_str().len() != 64
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.workbench_request.boundary != PRECLINICAL_BOUNDARY
        || !request
            .workbench_request
            .copilot_request
            .synthesis_request
            .raw_data_local
        || request.replay_identity != request.workbench_request.replay_identity
    {
        return Err(MultimodalRetrievalSynthesisFederatedControlPlaneError::Invalid("multimodal federated control identity, quorum, modality/comparability, capacity, replay, locality, or boundary is invalid".into()));
    }
    let workbench =
        render_multimodal_retrieval_synthesis_research_workbench(&request.workbench_request)
            .map_err(|error| {
                MultimodalRetrievalSynthesisFederatedControlPlaneError::Workbench(error.to_string())
            })?;
    if workbench.required_modalities != request.expected_modalities
        || workbench.comparability_digest != request.expected_comparability_digest
        || workbench.scope != request.expected_scope
    {
        return Err(MultimodalRetrievalSynthesisFederatedControlPlaneError::Invalid("multimodal workbench does not match control-plane modality, comparability, or scope contract".into()));
    }
    let mut omissions = workbench.omissions.clone();
    let mut uncertainty = workbench.uncertainty.clone();
    let mut counter = Vec::new();
    if !request.policy_allow {
        counter.push("policy authorization denied".into());
        omissions.push("policy authorization".into());
    }
    if !request.protected_closure {
        counter.push("protected closure incomplete".into());
        uncertainty.push("protected closure".into());
    }
    if !request.signed_approval {
        counter.push("signed approval missing".into());
        uncertainty.push("signed approval".into());
    }
    if !request.federation_permitted {
        counter.push("federation permission denied".into());
        omissions.push("federation permission".into());
    }
    if saturated(request.active_runs, request.capacity) {
        uncertainty.push("capacity headroom is exhausted".into());
    }
    omissions.sort();
    omissions.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    counter.sort();
    counter.dedup();
    let admission = if !request.policy_allow || !request.federation_permitted {
        "blocked"
    } else if !request.signed_approval || !request.protected_closure {
        "approval_required"
    } else if saturated(request.active_runs, request.capacity) {
        "degraded"
    } else {
        "admitted"
    };
    let effect = if admission == "admitted" {
        format!(
            "operate:multimodal-federated-control-plane:{}",
            request.service_id
        )
    } else if admission == "approval_required" {
        format!("approval-required:{}", request.service_id)
    } else {
        "block:unsafe-release".into()
    };
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":workbench.request_id,"service_id":request.service_id,"federation_id":request.federation_id,"purpose":request.purpose,"peer_ids":request.peer_ids,"min_peer_quorum":request.min_peer_quorum,"aggregate_only":true,"capacity":request.capacity,"active_runs":request.active_runs,"admission":admission,"scope":workbench.scope,"required_modalities":workbench.required_modalities,"comparability_digest":workbench.comparability_digest,"workbench_digest":workbench.workbench_digest,"health_digest":request.health_digest,"replay_identity":request.replay_identity,"omissions":omissions,"uncertainty":uncertainty,"counterexamples":counter,"raw_data_local":true,"boundary":PRECLINICAL_BOUNDARY});
    let control_digest = ContentHash::of_value(&payload).map_err(|error| {
        MultimodalRetrievalSynthesisFederatedControlPlaneError::Artifact(error.to_string())
    })?;
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "adapter-multimodal-federated-control:{}",
            request.service_id
        ),
        "application/vnd.aurora.multimodal-retrieval-synthesis-federated-control-plane+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| {
        MultimodalRetrievalSynthesisFederatedControlPlaneError::Artifact(error.to_string())
    })?;
    let receipt = MultimodalRetrievalSynthesisFederatedControlPlaneReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: workbench.request_id.clone(),
        service_id: request.service_id.clone(),
        federation_id: request.federation_id.clone(),
        purpose: request.purpose.clone(),
        peer_ids: request.peer_ids.clone(),
        min_peer_quorum: request.min_peer_quorum,
        aggregate_only: true,
        capacity: request.capacity,
        active_runs: request.active_runs,
        admission: admission.into(),
        scope: workbench.scope.clone(),
        required_modalities: workbench.required_modalities.clone(),
        comparability_digest: workbench.comparability_digest.clone(),
        workbench_digest: workbench.workbench_digest.clone(),
        health_digest: request.health_digest.clone(),
        replay_identity: request.replay_identity.clone(),
        control_digest,
        omissions,
        uncertainty,
        counterexamples: counter,
        effect_receipts: vec![effect],
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manifest_is_a2_and_federated() {
        let m = multimodal_retrieval_synthesis_federated_control_plane_manifest();
        assert_eq!(m.autonomy_tier, AutonomyTier::A2);
        assert_eq!(INPUT_SCHEMA, "ScopedRetrievalQuery2@1");
    }
}
