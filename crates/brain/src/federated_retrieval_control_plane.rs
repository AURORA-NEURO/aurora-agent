//! Institution-local federated retrieval control plane.
//!
//! Atlas feature: `AFA-brain-P02-F32`. This A2 operator product reconciles purpose, signer,
//! approval, aggregate-only, comparability, replay, locality, and budget gates before any
//! federated retrieval summary can be considered for release.

use crate::federated_retrieval_synthesis::{
    synthesize_federated_retrieval, FederatedRetrievalDisposition, FederatedRetrievalQuery,
};
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

pub const FEATURE_ID: &str = "AFA-brain-P02-F32";
pub const CONTRACT_VERSION: &str = "brain-federated-retrieval-control-plane/1.0";
pub const ACTION_ORDER: [&str; 4] = [
    "control:observe",
    "control:reconcile",
    "control:authorize",
    "control:publish",
];
const CONTROL_CONTENT_TYPE: &str = "application/vnd.aurora.federated-retrieval-control-plane+json";
const MAX_TEXT_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedRetrievalControlPlaneRequest {
    pub request: FederatedRetrievalQuery,
    pub plane_id: String,
    pub session_id: String,
    pub requested_action_order: Vec<String>,
    pub budget_units: u32,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signer_valid: bool,
    pub approval_valid: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedRetrievalControlPlaneReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub plane_id: String,
    pub session_id: String,
    pub federation_id: String,
    pub institution_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub endpoint: String,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub disposition: FederatedRetrievalDisposition,
    pub action_order: Vec<String>,
    pub completed_action_order: Vec<String>,
    pub blocked_action_order: Vec<String>,
    pub compensation_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub ranked_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub aggregate_order: Vec<ContentHash>,
    pub comparability_digest: ContentHash,
    pub envelope_digest: ContentHash,
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
pub enum FederatedRetrievalControlPlaneError {
    #[error("invalid federated retrieval control-plane request: {0}")]
    Invalid(String),
    #[error("federated retrieval control-plane artifact failed: {0}")]
    Artifact(String),
    #[error("federated retrieval control-plane synthesis failed: {0}")]
    Engine(String),
}

impl FederatedRetrievalControlPlaneReceipt {
    pub fn validate(&self) -> Result<(), FederatedRetrievalControlPlaneError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.request_id.trim().is_empty()
            || self.plane_id.trim().is_empty()
            || self.session_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.institution_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.endpoint.trim().is_empty()
            || self.study_order.len() < 2
            || self.modality_order.len() < 2
            || self.action_order != ACTION_ORDER
            || self.completed_action_order.is_empty()
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
            || self.budget_units == 0
        {
            return Err(FederatedRetrievalControlPlaneError::Invalid(
                "federated control-plane identity, closure, actions, retrieval, locality, budget, or effects are incomplete".into(),
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
                    FederatedRetrievalControlPlaneError::Invalid(
                        "federated control-plane action is unknown".into(),
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
                return Err(FederatedRetrievalControlPlaneError::Invalid(
                    "federated control-plane action transcript is not canonical".into(),
                ));
            }
        }
        for (value, field) in [
            (&self.request_id, "request_id"),
            (&self.plane_id, "plane_id"),
            (&self.session_id, "session_id"),
            (&self.federation_id, "federation_id"),
            (&self.institution_id, "institution_id"),
            (&self.purpose, "purpose"),
            (&self.semantic_profile, "semantic_profile"),
            (&self.endpoint, "endpoint"),
            (&self.boundary, "boundary"),
        ] {
            validate_text(value, field)?;
        }
        if self.completed_action_order.len() > self.action_order.len()
            || self.completed_action_order != self.action_order[..self.completed_action_order.len()]
            || self.blocked_action_order != self.action_order[self.completed_action_order.len()..]
        {
            return Err(FederatedRetrievalControlPlaneError::Invalid(
                "federated control-plane actions are not a canonical prefix and suffix".into(),
            ));
        }
        validate_sorted_unique(&self.study_order, "study_order")?;
        validate_sorted_unique(&self.modality_order, "modality_order")?;
        validate_sorted_unique(&self.compensation_order, "compensation_order")?;
        validate_sorted_unique(&self.candidate_order, "candidate_order")?;
        for (values, field) in [
            (&self.ranked_order, "ranked_order"),
            (&self.qualified_order, "qualified_order"),
            (&self.blocked_order, "blocked_order"),
            (&self.unknown_order, "unknown_order"),
        ] {
            validate_unique(values, field)?;
        }
        for (values, field) in [
            (&self.omissions, "omissions"),
            (&self.uncertainty, "uncertainty"),
            (&self.negative_evidence, "negative_evidence"),
            (&self.effect_receipts, "effect_receipts"),
        ] {
            validate_sorted_unique(values, field)?;
        }
        let candidate_values = self
            .candidate_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let ranked_values = self.ranked_order.iter().cloned().collect::<BTreeSet<_>>();
        let qualified_values = self
            .qualified_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let blocked_values = self.blocked_order.iter().cloned().collect::<BTreeSet<_>>();
        let unknown_values = self.unknown_order.iter().cloned().collect::<BTreeSet<_>>();
        if ranked_values != candidate_values {
            return Err(FederatedRetrievalControlPlaneError::Invalid(
                "federated control-plane ranked order must contain every candidate exactly once"
                    .into(),
            ));
        }
        if !qualified_values.is_subset(&candidate_values)
            || !blocked_values.is_subset(&candidate_values)
            || !unknown_values.is_subset(&blocked_values)
            || !qualified_values.is_disjoint(&blocked_values)
            || qualified_values
                .union(&blocked_values)
                .cloned()
                .collect::<BTreeSet<_>>()
                != candidate_values
        {
            return Err(FederatedRetrievalControlPlaneError::Invalid(
                "federated control-plane candidate states must partition candidates".into(),
            ));
        }
        validate_digest_order(&self.aggregate_order)?;
        for digest in [
            &self.comparability_digest,
            &self.envelope_digest,
            &self.synthesis_digest,
            &self.control_digest,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(FederatedRetrievalControlPlaneError::Invalid(
                    "federated control-plane digest is invalid".into(),
                ));
            }
        }
        let expected_effect_receipts = if matches!(
            self.disposition,
            FederatedRetrievalDisposition::Qualified | FederatedRetrievalDisposition::Partial
        ) {
            vec![format!(
                "manage:local-federated-retrieval-control:{}",
                self.plane_id
            )]
        } else {
            vec!["block:unsafe-release".into()]
        };
        if self.effect_receipts != expected_effect_receipts {
            return Err(FederatedRetrievalControlPlaneError::Invalid(
                "federated control-plane effect does not match disposition".into(),
            ));
        }
        if !self.raw_data_local {
            return Err(FederatedRetrievalControlPlaneError::Invalid(
                "federated control-plane receipts must declare that emitted data is local".into(),
            ));
        }
        let expected_control_digest = ContentHash::of_value(&json!({
            "feature_id": FEATURE_ID,
            "plane_id": self.plane_id,
            "session_id": self.session_id,
            "federation_id": self.federation_id,
            "institution_id": self.institution_id,
            "purpose": self.purpose,
            "semantic_profile": self.semantic_profile,
            "endpoint": self.endpoint,
            "study_order": self.study_order,
            "modality_order": self.modality_order,
            "action_order": self.action_order,
            "completed": self.completed_action_order,
            "blocked": self.blocked_action_order,
            "compensation": self.compensation_order,
            "candidate_order": self.candidate_order,
            "ranked_order": self.ranked_order,
            "qualified_order": self.qualified_order,
            "blocked_order": self.blocked_order,
            "unknown_order": self.unknown_order,
            "aggregate_order": self.aggregate_order,
            "comparability_digest": self.comparability_digest,
            "envelope_digest": self.envelope_digest,
            "synthesis_digest": self.synthesis_digest,
            "replay_identity": self.replay_identity,
            "raw_data_local": self.raw_data_local,
        }))
        .map_err(|error| FederatedRetrievalControlPlaneError::Artifact(error.to_string()))?;
        if self.control_digest != expected_control_digest {
            return Err(FederatedRetrievalControlPlaneError::Invalid(
                "federated control-plane digest is not bound to control state".into(),
            ));
        }
        let expected_artifact_id =
            format!("brain-federated-retrieval-control-plane:{}", self.plane_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != CONTROL_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(FederatedRetrievalControlPlaneError::Invalid(
                "federated control-plane artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| FederatedRetrievalControlPlaneError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| FederatedRetrievalControlPlaneError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, FederatedRetrievalControlPlaneError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| FederatedRetrievalControlPlaneError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| FederatedRetrievalControlPlaneError::Artifact(error.to_string()))
    }
}

pub fn federated_retrieval_control_plane_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "brain".into(),
        consumers: ["federation steward".into(), "multisite retrieval operator".into()].into(),
        behavior: "manages purpose-bound federated retrieval control actions with signer, approval, aggregate-only, comparability, replay, locality, compensation, and permitted summary release receipts".into(),
        value: "turns consortium retrieval readiness into inspectable local control state without raw-data movement or silent federation authorization".into(),
        inputs: vec![TypedPort { name: "federated_retrieval_control_plane_request".into(), schema: "FederatedRetrievalControlPlaneRequest1@1".into(), required: true }],
        outputs: vec![TypedPort { name: "federated_retrieval_control_plane_receipt".into(), schema: "FederatedRetrievalControlPlaneReceipt1@1".into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(),
        permissions: ["manage:local-federated-retrieval-control".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }],
        authority_requirements: vec![AuthorityRequirement { role: "federated retrieval control approver".into(), reason: "authorize purpose-bound aggregate-only control after signer, comparability, locality, and replay gates close".into() }],
        autonomy_tier: AutonomyTier::A2,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn operate_federated_retrieval_control_plane(
    request: &FederatedRetrievalControlPlaneRequest,
) -> Result<FederatedRetrievalControlPlaneReceipt, FederatedRetrievalControlPlaneError> {
    if request.plane_id.trim().is_empty()
        || request.session_id.trim().is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.request.boundary != PRECLINICAL_BOUNDARY
        || request.requested_action_order != ACTION_ORDER
        || request.budget_units == 0
        || request.request.policy_allow != request.policy_allow
        || request.request.protected_closure != request.protected_closure
        || request.request.signer_valid != request.signer_valid
        || request.request.approval_valid != request.approval_valid
        || request.request.raw_data_local != request.raw_data_local
    {
        return Err(FederatedRetrievalControlPlaneError::Invalid("federated control-plane identity, action order, gate parity, budget, or boundary is invalid".into()));
    }
    let synthesis = synthesize_federated_retrieval(&request.request)
        .map_err(|error| FederatedRetrievalControlPlaneError::Engine(error.to_string()))?;
    let gate = request.policy_allow
        && request.protected_closure
        && request.signer_valid
        && request.approval_valid
        && request.raw_data_local
        && u64::from(request.budget_units) >= u64::try_from(ACTION_ORDER.len()).unwrap_or(u64::MAX);
    let disposition = if gate {
        synthesis.disposition
    } else {
        FederatedRetrievalDisposition::Blocked
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
    if !request.signer_valid {
        omissions.insert("control:signer-invalid".into());
        compensation.insert("compensate:retain-signer-failure".into());
    }
    if !request.approval_valid {
        omissions.insert("control:approval-required".into());
        compensation.insert("compensate:retain-approval-gap".into());
    }
    if !request.raw_data_local {
        omissions.insert("control:raw-data-locality-failed".into());
        compensation.insert("compensate:retain-locality-failure".into());
    }
    if disposition != FederatedRetrievalDisposition::Qualified {
        compensation.insert("compensate:retain-unresolved-federated-retrieval".into());
    }
    let raw_data_local = true;
    let effect_receipts = if matches!(
        disposition,
        FederatedRetrievalDisposition::Qualified | FederatedRetrievalDisposition::Partial
    ) {
        vec![format!(
            "manage:local-federated-retrieval-control:{}",
            request.plane_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let control_digest = ContentHash::of_value(&json!({
        "feature_id": FEATURE_ID,
        "plane_id": request.plane_id,
        "session_id": request.session_id,
        "federation_id": synthesis.federation_id,
        "institution_id": synthesis.institution_id,
        "purpose": synthesis.purpose,
        "semantic_profile": synthesis.semantic_profile,
        "endpoint": synthesis.endpoint,
        "study_order": synthesis.study_order,
        "modality_order": synthesis.modality_order,
        "action_order": ACTION_ORDER,
        "completed": completed_action_order,
        "blocked": blocked_action_order,
        "compensation": compensation,
        "candidate_order": synthesis.candidate_order,
        "ranked_order": synthesis.ranked_order,
        "qualified_order": synthesis.qualified_order,
        "blocked_order": synthesis.blocked_order,
        "unknown_order": synthesis.unknown_order,
        "aggregate_order": synthesis.aggregate_order,
        "comparability_digest": synthesis.comparability_digest,
        "envelope_digest": synthesis.envelope_digest,
        "synthesis_digest": synthesis.synthesis_digest,
        "replay_identity": request.request.replay_identity,
        "raw_data_local": raw_data_local,
    }))
    .map_err(|error| FederatedRetrievalControlPlaneError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request.request_id, "plane_id": request.plane_id, "session_id": request.session_id, "federation_id": synthesis.federation_id, "institution_id": synthesis.institution_id, "purpose": synthesis.purpose, "semantic_profile": synthesis.semantic_profile, "endpoint": synthesis.endpoint, "study_order": synthesis.study_order, "modality_order": synthesis.modality_order, "disposition": disposition, "action_order": ACTION_ORDER, "completed_action_order": completed_action_order, "blocked_action_order": blocked_action_order, "compensation_order": compensation, "candidate_order": synthesis.candidate_order, "ranked_order": synthesis.ranked_order, "qualified_order": synthesis.qualified_order, "blocked_order": synthesis.blocked_order, "unknown_order": synthesis.unknown_order, "aggregate_order": synthesis.aggregate_order, "comparability_digest": synthesis.comparability_digest, "envelope_digest": synthesis.envelope_digest, "synthesis_digest": synthesis.synthesis_digest, "control_digest": control_digest, "replay_identity": request.request.replay_identity, "budget_units": request.budget_units, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "effect_receipts": effect_receipts, "raw_data_local": raw_data_local, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "brain-federated-retrieval-control-plane:{}",
            request.plane_id
        ),
        CONTROL_CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| FederatedRetrievalControlPlaneError::Artifact(error.to_string()))?;
    let receipt = FederatedRetrievalControlPlaneReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request.request_id.clone(),
        plane_id: request.plane_id.clone(),
        session_id: request.session_id.clone(),
        federation_id: synthesis.federation_id,
        institution_id: synthesis.institution_id,
        purpose: synthesis.purpose,
        semantic_profile: synthesis.semantic_profile,
        endpoint: synthesis.endpoint,
        study_order: synthesis.study_order,
        modality_order: synthesis.modality_order,
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
        aggregate_order: synthesis.aggregate_order,
        comparability_digest: synthesis.comparability_digest,
        envelope_digest: synthesis.envelope_digest,
        synthesis_digest: synthesis.synthesis_digest,
        control_digest,
        replay_identity: request.request.replay_identity.clone(),
        budget_units: request.budget_units,
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts,
        artifact,
        raw_data_local,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_text(value: &str, field: &str) -> Result<(), FederatedRetrievalControlPlaneError> {
    if value.trim().is_empty()
        || value != value.trim()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(FederatedRetrievalControlPlaneError::Invalid(format!(
            "{field} must be bounded, non-empty text without padding or control characters"
        )));
    }
    Ok(())
}

fn validate_unique(
    values: &[String],
    field: &str,
) -> Result<(), FederatedRetrievalControlPlaneError> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_text(value, field)?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(FederatedRetrievalControlPlaneError::Invalid(format!(
                "{field} contains duplicate or case-colliding values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_unique(
    values: &[String],
    field: &str,
) -> Result<(), FederatedRetrievalControlPlaneError> {
    validate_unique(values, field)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(FederatedRetrievalControlPlaneError::Invalid(format!(
            "{field} is not in canonical order"
        )));
    }
    Ok(())
}

fn validate_digest_order(
    values: &[ContentHash],
) -> Result<(), FederatedRetrievalControlPlaneError> {
    if values.iter().any(|value| value.as_str().len() != 64)
        || values.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(FederatedRetrievalControlPlaneError::Invalid(
            "federated aggregate order or digest is invalid".into(),
        ));
    }
    Ok(())
}

fn receipt_payload(receipt: &FederatedRetrievalControlPlaneReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "plane_id": receipt.plane_id,
        "session_id": receipt.session_id,
        "federation_id": receipt.federation_id,
        "institution_id": receipt.institution_id,
        "purpose": receipt.purpose,
        "semantic_profile": receipt.semantic_profile,
        "endpoint": receipt.endpoint,
        "study_order": receipt.study_order,
        "modality_order": receipt.modality_order,
        "disposition": receipt.disposition,
        "action_order": receipt.action_order,
        "completed_action_order": receipt.completed_action_order,
        "blocked_action_order": receipt.blocked_action_order,
        "compensation_order": receipt.compensation_order,
        "candidate_order": receipt.candidate_order,
        "ranked_order": receipt.ranked_order,
        "qualified_order": receipt.qualified_order,
        "blocked_order": receipt.blocked_order,
        "unknown_order": receipt.unknown_order,
        "aggregate_order": receipt.aggregate_order,
        "comparability_digest": receipt.comparability_digest,
        "envelope_digest": receipt.envelope_digest,
        "synthesis_digest": receipt.synthesis_digest,
        "control_digest": receipt.control_digest,
        "replay_identity": receipt.replay_identity,
        "budget_units": receipt.budget_units,
        "omissions": receipt.omissions,
        "uncertainty": receipt.uncertainty,
        "negative_evidence": receipt.negative_evidence,
        "effect_receipts": receipt.effect_receipts,
        "raw_data_local": receipt.raw_data_local,
        "boundary": receipt.boundary,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retrieval_synthesis::RetrievalCandidate;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn request() -> FederatedRetrievalControlPlaneRequest {
        let query = FederatedRetrievalQuery {
            request_id: "request:fed-control".into(),
            federation_id: "federation:consortium".into(),
            institution_id: "institution:local".into(),
            purpose: "purpose:preclinical-replication".into(),
            semantic_profile: "profile:ome-ngff".into(),
            endpoint: "urn:aurora:local-federation".into(),
            allowed_artifacts: vec!["qualified-evidence-summary".into()],
            study_ids: vec!["study:one".into(), "study:two".into()],
            scope: "organoid:neural".into(),
            minimum_support_milli: 700,
            required_modalities: vec!["imaging".into(), "transcriptomics".into()],
            candidates: vec![RetrievalCandidate {
                evidence_id: "evidence:fed-control".into(),
                source_id: "source:fed-control".into(),
                study_id: "study:one".into(),
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
            signer_valid: true,
            approval_valid: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        FederatedRetrievalControlPlaneRequest {
            request: query,
            plane_id: "plane:fed-local".into(),
            session_id: "session:fed-control".into(),
            requested_action_order: ACTION_ORDER.iter().map(|value| (*value).into()).collect(),
            budget_units: 8,
            policy_allow: true,
            protected_closure: true,
            signer_valid: true,
            approval_valid: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a2() {
        let manifest = federated_retrieval_control_plane_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A2);
        assert!(!manifest.authority_requirements.is_empty());
    }
    #[test]
    fn control_plane_completes() {
        let receipt = operate_federated_retrieval_control_plane(&request()).unwrap();
        assert_eq!(receipt.completed_action_order.len(), 4);
        assert!(!receipt.aggregate_order.is_empty());
    }
    #[test]
    fn denial_compensates() {
        let mut value = request();
        value.approval_valid = false;
        value.request.approval_valid = false;
        let receipt = operate_federated_retrieval_control_plane(&value).unwrap();
        assert_eq!(receipt.disposition, FederatedRetrievalDisposition::Blocked);
        assert!(!receipt.compensation_order.is_empty());
    }
    #[test]
    fn locality_failure_is_blocked_and_retained() {
        let mut value = request();
        value.raw_data_local = false;
        value.request.raw_data_local = false;
        let receipt = operate_federated_retrieval_control_plane(&value).unwrap();
        assert_eq!(receipt.disposition, FederatedRetrievalDisposition::Blocked);
        assert!(receipt.raw_data_local);
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item == "control:raw-data-locality-failed"));
        assert!(receipt.validate().is_ok());
    }
    #[test]
    fn control_artifact_payload_is_bound() {
        let mut receipt = operate_federated_retrieval_control_plane(&request()).unwrap();
        receipt.endpoint = "urn:tampered".into();
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn case_mismatched_candidate_identity_is_rejected() {
        let mut receipt = operate_federated_retrieval_control_plane(&request()).unwrap();
        receipt.ranked_order[0] = receipt.ranked_order[0].to_ascii_uppercase();
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn digest_is_stable() {
        let receipt = operate_federated_retrieval_control_plane(&request()).unwrap();
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
}
