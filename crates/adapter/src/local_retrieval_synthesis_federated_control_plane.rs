//! Local retrieval/synthesis federated control plane.
//! Atlas feature `AFA-adapter-P02-F29`.

use crate::local_retrieval_synthesis_research_workbench::{
    render_local_retrieval_synthesis_research_workbench,
    LocalRetrievalSynthesisResearchWorkbenchRequest,
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

pub const FEATURE_ID: &str = "AFA-adapter-P02-F29";
pub const CONTRACT_VERSION: &str = "adapter-local-retrieval-synthesis-federated-control-plane/1.0";
pub const INPUT_SCHEMA: &str = "ScopedRetrievalQuery1@1";
pub const OUTPUT_SCHEMA: &str = "EvidenceSynthesis5@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalRetrievalSynthesisFederatedControlPlaneRequest {
    pub workbench_request: LocalRetrievalSynthesisResearchWorkbenchRequest,
    pub service_id: String,
    pub node_id: String,
    pub capacity: u32,
    pub active_runs: u32,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_permitted: bool,
    pub health_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LocalRetrievalSynthesisFederatedControlPlaneReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub service_id: String,
    pub node_id: String,
    pub capacity: u32,
    pub active_runs: u32,
    pub admission: String,
    pub scope: String,
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
pub enum LocalRetrievalSynthesisFederatedControlPlaneError {
    #[error("invalid local retrieval federated control request: {0}")]
    Invalid(String),
    #[error("local retrieval federated control artifact failed: {0}")]
    Artifact(String),
    #[error("local retrieval control workbench failed: {0}")]
    Workbench(String),
}

impl LocalRetrievalSynthesisFederatedControlPlaneReceipt {
    pub fn validate(&self) -> Result<(), LocalRetrievalSynthesisFederatedControlPlaneError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.service_id.trim().is_empty()
            || self.node_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.capacity == 0
            || self.active_runs > self.capacity
            || !matches!(
                self.admission.as_str(),
                "admitted" | "degraded" | "approval_required" | "blocked"
            )
            || self.effect_receipts.is_empty()
        {
            return Err(Self::invalid(
                "control-plane identity, capacity, admission, locality, or effects are incomplete",
            ));
        }
        for values in [
            &self.omissions,
            &self.uncertainty,
            &self.counterexamples,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(Self::invalid("control-plane ordering is not canonical"));
            }
        }
        for digest in [
            &self.workbench_digest,
            &self.health_digest,
            &self.replay_identity,
            &self.control_digest,
            &self.artifact.content_hash,
        ] {
            if digest.as_str().len() != 64 {
                return Err(Self::invalid("control-plane digest is invalid"));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("operate:local-federated-control-plane:")
                && !effect.starts_with("approval-required:")
                && effect != "block:unsafe-release"
        }) {
            return Err(Self::invalid(
                "control-plane effect is outside admission gate",
            ));
        }
        self.artifact.validate_metadata().map_err(|error| {
            LocalRetrievalSynthesisFederatedControlPlaneError::Artifact(error.to_string())
        })
    }
    fn invalid(message: &str) -> LocalRetrievalSynthesisFederatedControlPlaneError {
        LocalRetrievalSynthesisFederatedControlPlaneError::Invalid(message.into())
    }
}

pub fn local_retrieval_synthesis_federated_control_plane_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "adapter".into(), consumers: ["consortium administrator".into(),"preclinical researcher".into()].into(), behavior: "operates an institution-local retrieval/synthesis control plane with capacity, health, federation-permission, signed-approval, and fail-closed admission receipts".into(), value: "gives research operators a deterministic local service boundary without allowing unauthorized federation or over-capacity execution".into(), inputs: vec![TypedPort{name:"scoped_retrieval_query".into(),schema:INPUT_SCHEMA.into(),required:true}], outputs: vec![TypedPort{name:"controlled_evidence_synthesis".into(),schema:OUTPUT_SCHEMA.into(),required:true}], effects:[Effect::ReadLocalData,Effect::ExecuteLocalComputation,Effect::WriteLocalArtifact].into(), permissions:["operate:institution-local-control-plane".into()].into(), determinism:Determinism::ByteStable, evidence:vec![EvidenceReference{source_id:"w3c-prov-o".into(),state:EvidenceState::Supported,locator:Some("https://www.w3.org/TR/prov-o/".into())}], authority_requirements:Vec::new(), autonomy_tier:AutonomyTier::A1, surfaces:[ResearchSurface::Ui,ResearchSurface::Api,ResearchSurface::Sdk,ResearchSurface::Cli,ResearchSurface::McpTool,ResearchSurface::Policy,ResearchSurface::Operator].into(), boundary:PRECLINICAL_BOUNDARY.into() }
}

pub fn operate_local_retrieval_synthesis_federated_control_plane(
    request: &LocalRetrievalSynthesisFederatedControlPlaneRequest,
) -> Result<
    LocalRetrievalSynthesisFederatedControlPlaneReceipt,
    LocalRetrievalSynthesisFederatedControlPlaneError,
> {
    if request.service_id.trim().is_empty()
        || request.node_id.trim().is_empty()
        || request.capacity == 0
        || request.active_runs > request.capacity
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.workbench_request.boundary != PRECLINICAL_BOUNDARY
        || !request
            .workbench_request
            .copilot_request
            .synthesis_request
            .raw_data_local
        || request.replay_identity != request.workbench_request.replay_identity
        || request.replay_identity.as_str().len() != 64
        || request.health_digest.as_str().len() != 64
    {
        return Err(LocalRetrievalSynthesisFederatedControlPlaneError::Invalid(
            "control-plane identity, capacity, replay, health, locality, or boundary is invalid"
                .into(),
        ));
    }
    let workbench = render_local_retrieval_synthesis_research_workbench(&request.workbench_request)
        .map_err(|error| {
            LocalRetrievalSynthesisFederatedControlPlaneError::Workbench(error.to_string())
        })?;
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
    if active_runs_saturated(request.active_runs, request.capacity) {
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
    } else if active_runs_saturated(request.active_runs, request.capacity) {
        "degraded"
    } else {
        "admitted"
    };
    let effect = if admission == "admitted" {
        format!(
            "operate:local-federated-control-plane:{}",
            request.service_id
        )
    } else if admission == "approval_required" {
        format!("approval-required:{}", request.service_id)
    } else {
        "block:unsafe-release".into()
    };
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":workbench.request_id,"service_id":request.service_id,"node_id":request.node_id,"capacity":request.capacity,"active_runs":request.active_runs,"admission":admission,"scope":workbench.scope,"workbench_digest":workbench.workbench_digest,"health_digest":request.health_digest,"replay_identity":request.replay_identity,"omissions":omissions,"uncertainty":uncertainty,"counterexamples":counter,"raw_data_local":true,"boundary":PRECLINICAL_BOUNDARY});
    let control_digest = ContentHash::of_value(&payload).map_err(|error| {
        LocalRetrievalSynthesisFederatedControlPlaneError::Artifact(error.to_string())
    })?;
    let artifact = TypedResearchArtifact::from_payload(
        format!("adapter-local-federated-control:{}", request.service_id),
        "application/vnd.aurora.local-retrieval-synthesis-federated-control-plane+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| {
        LocalRetrievalSynthesisFederatedControlPlaneError::Artifact(error.to_string())
    })?;
    let receipt = LocalRetrievalSynthesisFederatedControlPlaneReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: workbench.request_id.clone(),
        service_id: request.service_id.clone(),
        node_id: request.node_id.clone(),
        capacity: request.capacity,
        active_runs: request.active_runs,
        admission: admission.into(),
        scope: workbench.scope.clone(),
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
fn active_runs_saturated(active_runs: u32, capacity: u32) -> bool {
    active_runs.saturating_mul(100) >= capacity.saturating_mul(90)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn manifest_is_a1_and_local() {
        let m = local_retrieval_synthesis_federated_control_plane_manifest();
        assert_eq!(m.autonomy_tier, AutonomyTier::A1);
        assert_eq!(INPUT_SCHEMA, "ScopedRetrievalQuery1@1");
    }
}
