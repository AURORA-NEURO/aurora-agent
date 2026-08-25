//! Institution-local retrieval federated control plane.
//!
//! Atlas feature: `AFA-brain-P02-F29`. This operator surface manages bounded local retrieval
//! state and permitted summary release without granting raw-data movement.

use crate::retrieval_synthesis::{
    synthesize_retrieval, ScopedRetrievalQuery, SynthesisDisposition,
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

pub const FEATURE_ID: &str = "AFA-brain-P02-F29";
pub const CONTRACT_VERSION: &str = "brain-retrieval-federated-control-plane/1.0";
pub const ACTION_ORDER: [&str; 4] = [
    "control:observe",
    "control:reconcile",
    "control:authorize",
    "control:publish",
];

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalFederatedControlPlaneRequest {
    pub request: ScopedRetrievalQuery,
    pub plane_id: String,
    pub session_id: String,
    pub requested_action_order: Vec<String>,
    pub budget_units: u32,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalFederatedControlPlaneReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub plane_id: String,
    pub session_id: String,
    pub disposition: SynthesisDisposition,
    pub action_order: Vec<String>,
    pub completed_action_order: Vec<String>,
    pub blocked_action_order: Vec<String>,
    pub compensation_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub ranked_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub synthesis_digest: ContentHash,
    pub control_digest: ContentHash,
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
pub enum RetrievalFederatedControlPlaneError {
    #[error("invalid retrieval federated control-plane request: {0}")]
    Invalid(String),
    #[error("retrieval federated control-plane artifact failed: {0}")]
    Artifact(String),
    #[error("retrieval federated control-plane synthesis failed: {0}")]
    Engine(String),
}

impl RetrievalFederatedControlPlaneReceipt {
    pub fn validate(&self) -> Result<(), RetrievalFederatedControlPlaneError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.plane_id.trim().is_empty()
            || self.session_id.trim().is_empty()
            || self.action_order != ACTION_ORDER
            || self.completed_action_order.is_empty()
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
            || self.budget_units == 0
        {
            return Err(RetrievalFederatedControlPlaneError::Invalid("control-plane identity, actions, retrieval, locality, budget, or effects are incomplete".into()));
        }
        if self
            .ranked_order
            .iter()
            .chain(self.qualified_order.iter())
            .chain(self.blocked_order.iter())
            .chain(self.unknown_order.iter())
            .any(|id| !self.candidate_order.contains(id))
        {
            return Err(RetrievalFederatedControlPlaneError::Invalid(
                "control-plane evidence state is not covered by candidates".into(),
            ));
        }
        let action_position =
            |action: &String| ACTION_ORDER.iter().position(|expected| expected == action);
        for values in [&self.completed_action_order, &self.blocked_action_order] {
            let positions = values
                .iter()
                .map(action_position)
                .collect::<Option<Vec<_>>>()
                .ok_or_else(|| {
                    RetrievalFederatedControlPlaneError::Invalid(
                        "control-plane action is unknown".into(),
                    )
                })?;
            if positions.windows(2).any(|pair| pair[0] >= pair[1])
                || values.iter().any(|action| {
                    self.completed_action_order
                        .iter()
                        .filter(|candidate| *candidate == action)
                        .count()
                        + self
                            .blocked_action_order
                            .iter()
                            .filter(|candidate| *candidate == action)
                            .count()
                        > 1
                })
            {
                return Err(RetrievalFederatedControlPlaneError::Invalid(
                    "control-plane action transcript is not canonical".into(),
                ));
            }
        }
        for values in [
            &self.compensation_order,
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
                return Err(RetrievalFederatedControlPlaneError::Invalid(
                    "control-plane ordering is not canonical".into(),
                ));
            }
        }
        for digest in [
            &self.synthesis_digest,
            &self.control_digest,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(RetrievalFederatedControlPlaneError::Invalid(
                    "control-plane digest is invalid".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("manage:local-retrieval-control:")
                && effect != "block:unsafe-release"
        }) {
            return Err(RetrievalFederatedControlPlaneError::Invalid(
                "control-plane effect is outside local management gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| RetrievalFederatedControlPlaneError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, RetrievalFederatedControlPlaneError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| RetrievalFederatedControlPlaneError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| RetrievalFederatedControlPlaneError::Artifact(error.to_string()))
    }
}

pub fn retrieval_federated_control_plane_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["agent developer".into(), "federated operator".into()].into(), behavior: "manages bounded institution-local retrieval control actions with reconciliation, compensation, replay, and permitted summary release receipts".into(), value: "turns retrieval operations and federation readiness into inspectable local control state without raw-data movement".into(), inputs: vec![TypedPort { name: "retrieval_control_plane_request".into(), schema: "ScopedRetrievalQuery1@1".into(), required: true }], outputs: vec![TypedPort { name: "retrieval_control_plane_receipt".into(), schema: "RetrievalFederatedControlPlaneReceipt1@1".into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["manage:local-retrieval-control".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn operate_retrieval_federated_control_plane(
    request: &RetrievalFederatedControlPlaneRequest,
) -> Result<RetrievalFederatedControlPlaneReceipt, RetrievalFederatedControlPlaneError> {
    if request.plane_id.trim().is_empty()
        || request.session_id.trim().is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.request.boundary != PRECLINICAL_BOUNDARY
        || request.requested_action_order != ACTION_ORDER
        || request.budget_units == 0
    {
        return Err(RetrievalFederatedControlPlaneError::Invalid(
            "control-plane identity, action order, budget, or boundary is invalid".into(),
        ));
    }
    let synthesis = synthesize_retrieval(&request.request)
        .map_err(|error| RetrievalFederatedControlPlaneError::Engine(error.to_string()))?;
    let gate = request.policy_allow
        && request.protected_closure
        && request.raw_data_local
        && request.budget_units >= ACTION_ORDER.len() as u32;
    let disposition = if gate {
        synthesis.disposition
    } else {
        SynthesisDisposition::Blocked
    };
    let completed_action_order: Vec<String> = if gate {
        ACTION_ORDER
            .iter()
            .map(|value| (*value).to_string())
            .collect()
    } else {
        ACTION_ORDER[..2]
            .iter()
            .map(|value| (*value).to_string())
            .collect()
    };
    let blocked_action_order: Vec<String> = if gate {
        Vec::new()
    } else {
        ACTION_ORDER[2..]
            .iter()
            .map(|value| (*value).to_string())
            .collect()
    };
    let mut compensation = BTreeSet::new();
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
    if !request.policy_allow {
        omissions.insert("control:policy-denied".into());
        compensation.insert("compensate:retain-policy-denial".into());
    }
    if !request.protected_closure {
        omissions.insert("control:protected-closure-incomplete".into());
        compensation.insert("compensate:retain-closure-gap".into());
    }
    if !request.raw_data_local {
        omissions.insert("control:raw-data-locality-failed".into());
        compensation.insert("compensate:retain-locality-failure".into());
    }
    if disposition != SynthesisDisposition::Qualified {
        compensation.insert("compensate:retain-unresolved-retrieval".into());
    }
    let control_digest = ContentHash::of_value(&json!({"feature_id": FEATURE_ID, "plane_id": request.plane_id, "session_id": request.session_id, "action_order": ACTION_ORDER, "completed": completed_action_order, "blocked": blocked_action_order, "compensation": compensation, "synthesis_digest": synthesis.synthesis_digest, "replay_identity": request.request.replay_identity})).map_err(|error| RetrievalFederatedControlPlaneError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request.request_id, "plane_id": request.plane_id, "session_id": request.session_id, "disposition": disposition, "action_order": ACTION_ORDER, "completed_action_order": completed_action_order, "blocked_action_order": blocked_action_order, "compensation_order": compensation, "synthesis_digest": synthesis.synthesis_digest, "control_digest": control_digest, "replay_identity": request.request.replay_identity, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-retrieval-control-plane:{}", request.plane_id),
        "application/vnd.aurora.retrieval-control-plane+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| RetrievalFederatedControlPlaneError::Artifact(error.to_string()))?;
    let effect_receipts = if matches!(
        disposition,
        SynthesisDisposition::Qualified | SynthesisDisposition::Partial
    ) {
        vec![format!(
            "manage:local-retrieval-control:{}",
            request.plane_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let receipt = RetrievalFederatedControlPlaneReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request.request_id.clone(),
        plane_id: request.plane_id.clone(),
        session_id: request.session_id.clone(),
        disposition,
        action_order: ACTION_ORDER.iter().map(|value| (*value).into()).collect(),
        completed_action_order,
        blocked_action_order,
        compensation_order: compensation.into_iter().collect(),
        candidate_order: synthesis.candidate_order,
        ranked_order: synthesis.ranked_order,
        qualified_order: synthesis.qualified_order,
        blocked_order: synthesis.blocked_order,
        unknown_order: synthesis.unknown_order,
        synthesis_digest: synthesis.synthesis_digest,
        control_digest,
        replay_identity: request.request.replay_identity.clone(),
        budget_units: request.budget_units,
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts,
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
    use crate::retrieval_synthesis::RetrievalCandidate;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn request() -> RetrievalFederatedControlPlaneRequest {
        RetrievalFederatedControlPlaneRequest {
            request: ScopedRetrievalQuery {
                request_id: "request:control".into(),
                study_id: "study:organoid".into(),
                scope: "organoid:neural".into(),
                query: "synaptic morphology".into(),
                minimum_support_milli: 700,
                candidates: vec![RetrievalCandidate {
                    evidence_id: "evidence:control".into(),
                    source_id: "source:control".into(),
                    study_id: "study:organoid".into(),
                    scope: "organoid:neural".into(),
                    modality: "imaging".into(),
                    support_milli: 900,
                    state: EvidenceState::Supported,
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
            plane_id: "plane:local".into(),
            session_id: "session:control".into(),
            requested_action_order: ACTION_ORDER.iter().map(|value| (*value).into()).collect(),
            budget_units: 8,
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        let manifest = retrieval_federated_control_plane_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A1);
    }
    #[test]
    fn control_plane_completes() {
        let receipt = operate_retrieval_federated_control_plane(&request()).unwrap();
        assert_eq!(receipt.disposition, SynthesisDisposition::Qualified);
        assert_eq!(receipt.completed_action_order.len(), 4);
    }
    #[test]
    fn denial_compensates() {
        let mut value = request();
        value.policy_allow = false;
        let receipt = operate_retrieval_federated_control_plane(&value).unwrap();
        assert_eq!(receipt.disposition, SynthesisDisposition::Blocked);
        assert!(!receipt.compensation_order.is_empty());
    }
    #[test]
    fn digest_is_stable() {
        let receipt = operate_retrieval_federated_control_plane(&request()).unwrap();
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
}
