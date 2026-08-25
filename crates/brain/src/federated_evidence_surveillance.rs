//! Federated continual evidence-surveillance release gate.
//!
//! Atlas feature: `AFA-brain-P01-F04`. Only a permitted aggregate envelope crosses an
//! institution boundary; raw observations and unapproved artifacts remain local.

use crate::evidence_surveillance::EvidenceObservation;
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

pub const FEATURE_ID: &str = "AFA-brain-P01-F04";
pub const CONTRACT_VERSION: &str = "brain-evidence-surveillance-federated/1.0";
pub const PERMITTED_ARTIFACT: &str = "qualified-evidence-summary";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedEvidenceFeedRequest {
    pub request_id: String,
    pub federation_id: String,
    pub institution_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub endpoint: String,
    pub allowed_artifacts: Vec<String>,
    pub observations: Vec<EvidenceObservation>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signer_valid: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedEvidenceDisposition {
    Qualified,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedEvidenceReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub institution_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub endpoint: String,
    pub disposition: FederatedEvidenceDisposition,
    pub candidate_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub aggregate_order: Vec<ContentHash>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub replay_identity: ContentHash,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedEvidenceError {
    #[error("invalid federated evidence request: {0}")]
    Invalid(String),
    #[error("federated evidence artifact failed: {0}")]
    Artifact(String),
}

impl FederatedEvidenceReceipt {
    pub fn validate(&self) -> Result<(), FederatedEvidenceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.institution_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.endpoint.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(FederatedEvidenceError::Invalid(
                "federation identity, envelope, locality, ranking, or effects are incomplete"
                    .into(),
            ));
        }
        if self
            .admitted_order
            .iter()
            .chain(self.blocked_order.iter())
            .chain(self.unknown_order.iter())
            .any(|id| !self.candidate_order.contains(id))
        {
            return Err(FederatedEvidenceError::Invalid(
                "federation state is not covered by candidate order".into(),
            ));
        }
        for values in [
            &self.candidate_order,
            &self.admitted_order,
            &self.blocked_order,
            &self.unknown_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(FederatedEvidenceError::Invalid(
                    "federation ordering is not canonical".into(),
                ));
            }
        }
        if self
            .aggregate_order
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(FederatedEvidenceError::Invalid(
                "aggregate digest ordering is not canonical".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("exchange:permitted-artifacts:") && effect != "block:unsafe-release"
        }) {
            return Err(FederatedEvidenceError::Invalid(
                "effect is outside federation exchange gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| FederatedEvidenceError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, FederatedEvidenceError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| FederatedEvidenceError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| FederatedEvidenceError::Artifact(error.to_string()))
    }
}

pub fn federated_evidence_surveillance_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["research workflow operator".into(), "federation steward".into()].into(), behavior: "admits policy-separated local evidence into a signed aggregate-only federation envelope".into(), value: "enables continual consortium surveillance without raw-data movement or unreviewed federation".into(), inputs: vec![TypedPort { name: "evidence_feed".into(), schema: "EvidenceFeed4@1".into(), required: true }], outputs: vec![TypedPort { name: "qualified_evidence_set".into(), schema: "QualifiedEvidenceSet1@1".into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact, Effect::FederationExport].into(), permissions: ["read:local-research-artifacts".into(), "export:permitted-research-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "json-schema".into(), state: EvidenceState::Supported, locator: Some("https://json-schema.org/specification".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn admit_federated_evidence(
    request: &FederatedEvidenceFeedRequest,
) -> Result<FederatedEvidenceReceipt, FederatedEvidenceError> {
    validate_request(request)?;
    let mut observations = request.observations.clone();
    observations.sort_by(|left, right| left.evidence_id.cmp(&right.evidence_id));
    let candidate_order = observations
        .iter()
        .map(|item| item.evidence_id.clone())
        .collect::<Vec<_>>();
    let mut admitted = Vec::new();
    let mut blocked = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut aggregate = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let exchange_allowed = request.policy_allow
        && request.protected_closure
        && request.signer_valid
        && request.raw_data_local
        && request
            .allowed_artifacts
            .iter()
            .any(|item| item == PERMITTED_ARTIFACT);
    for observation in &observations {
        let ok = exchange_allowed
            && observation.raw_data_local
            && observation.state == EvidenceState::Supported
            && observation.replay_identity == request.replay_identity
            && observation.omissions.is_empty()
            && observation.negative_evidence.is_empty();
        if ok {
            admitted.push(observation.evidence_id.clone());
            aggregate.insert(observation.artifact_digest.clone());
        } else {
            blocked.insert(observation.evidence_id.clone());
            if matches!(
                observation.state,
                EvidenceState::Unknown | EvidenceState::Speculative
            ) {
                unknown.insert(observation.evidence_id.clone());
                uncertainty.insert(
                    format!(
                        "evidence:{}:state-{:?}-not-admitted",
                        observation.evidence_id, observation.state
                    )
                    .to_ascii_lowercase(),
                );
            }
            if observation.state == EvidenceState::Contradicted {
                negative.insert(format!(
                    "evidence:{}:contradicted-negative-evidence",
                    observation.evidence_id
                ));
            }
            if observation.replay_identity != request.replay_identity {
                uncertainty.insert(format!(
                    "evidence:{}:replay-mismatch",
                    observation.evidence_id
                ));
            }
            if !observation.omissions.is_empty() {
                uncertainty.insert(format!(
                    "evidence:{}:protected-closure-incomplete",
                    observation.evidence_id
                ));
            }
            if !observation.negative_evidence.is_empty() {
                negative.insert(format!(
                    "evidence:{}:negative-result-retained",
                    observation.evidence_id
                ));
            }
        }
    }
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.signer_valid {
        negative.insert("request:signer-invalid".into());
    }
    if !request.raw_data_local {
        negative.insert("request:raw-data-locality-failed".into());
    }
    if !request
        .allowed_artifacts
        .iter()
        .any(|item| item == PERMITTED_ARTIFACT)
    {
        omissions.insert("federation:permitted-artifact-missing".into());
    }
    let disposition = if !exchange_allowed {
        FederatedEvidenceDisposition::Blocked
    } else if admitted.is_empty() {
        FederatedEvidenceDisposition::Unknown
    } else if blocked.is_empty()
        && omissions.is_empty()
        && uncertainty.is_empty()
        && negative.is_empty()
    {
        FederatedEvidenceDisposition::Qualified
    } else {
        FederatedEvidenceDisposition::Partial
    };
    let envelope_digest = ContentHash::of_value(&json!({"federation_id": request.federation_id, "institution_id": request.institution_id, "purpose": request.purpose, "semantic_profile": request.semantic_profile, "candidate_order": candidate_order, "aggregate_order": aggregate, "replay_identity": request.replay_identity})).map_err(|error| FederatedEvidenceError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "federation_id": request.federation_id, "institution_id": request.institution_id, "purpose": request.purpose, "semantic_profile": request.semantic_profile, "endpoint": request.endpoint, "disposition": disposition, "candidate_order": candidate_order, "admitted_order": admitted, "blocked_order": blocked, "unknown_order": unknown, "aggregate_order": aggregate, "envelope_digest": envelope_digest, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "replay_identity": request.replay_identity, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-federated-evidence:{}", request.request_id),
        "application/vnd.aurora.federation-envelope+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| FederatedEvidenceError::Artifact(error.to_string()))?;
    let has_admitted = !admitted.is_empty();
    let receipt = FederatedEvidenceReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        institution_id: request.institution_id.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        endpoint: request.endpoint.clone(),
        disposition,
        candidate_order,
        admitted_order: admitted,
        blocked_order: blocked.into_iter().collect(),
        unknown_order: unknown.into_iter().collect(),
        aggregate_order: aggregate.into_iter().collect(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        replay_identity: request.replay_identity.clone(),
        effect_receipts: if has_admitted {
            vec![format!(
                "exchange:permitted-artifacts:{}",
                request.request_id
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

fn validate_request(request: &FederatedEvidenceFeedRequest) -> Result<(), FederatedEvidenceError> {
    if request.request_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.institution_id.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.endpoint.trim().is_empty()
        || request.allowed_artifacts.is_empty()
        || request.observations.is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(FederatedEvidenceError::Invalid("federation identity, purpose, endpoint, artifact policy, observations, or boundary is incomplete".into()));
    }
    let mut ids = BTreeSet::new();
    for observation in &request.observations {
        if observation.evidence_id.trim().is_empty()
            || observation.boundary != PRECLINICAL_BOUNDARY
            || !ids.insert(observation.evidence_id.clone())
        {
            return Err(FederatedEvidenceError::Invalid(format!(
                "observation {} is invalid or duplicated",
                observation.evidence_id
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn observation(id: &str, state: EvidenceState) -> EvidenceObservation {
        EvidenceObservation {
            evidence_id: format!("evidence:{id}"),
            source_id: format!("source:{id}"),
            study_id: "study:organoid".into(),
            modality: "imaging".into(),
            scope: "organoid:neural".into(),
            relevance_milli: 900,
            state,
            semantic_digest: hash(&format!("semantic:{id}")),
            artifact_digest: hash(&format!("artifact:{id}")),
            provenance_digest: hash(&format!("provenance:{id}")),
            replay_identity: hash("replay"),
            omissions: Vec::new(),
            negative_evidence: Vec::new(),
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    fn request(observations: Vec<EvidenceObservation>) -> FederatedEvidenceFeedRequest {
        FederatedEvidenceFeedRequest {
            request_id: "request:federation".into(),
            federation_id: "federation:commons".into(),
            institution_id: "institution:a".into(),
            purpose: "benchmarking".into(),
            semantic_profile: "preclinical-evidence/v1".into(),
            endpoint: "https://hub.example/research".into(),
            allowed_artifacts: vec![PERMITTED_ARTIFACT.into()],
            observations,
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            signer_valid: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1_and_federation_scoped() {
        let manifest = federated_evidence_surveillance_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A1);
        assert!(manifest.effects.contains(&Effect::FederationExport));
    }
    #[test]
    fn signed_permitted_summary_is_admitted() {
        let receipt = admit_federated_evidence(&request(vec![
            observation("b", EvidenceState::Supported),
            observation("a", EvidenceState::Supported),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, FederatedEvidenceDisposition::Qualified);
        assert_eq!(receipt.effect_receipts.len(), 1);
        assert!(!receipt.aggregate_order.is_empty());
    }
    #[test]
    fn artifact_policy_denial_blocks_exchange() {
        let mut input = request(vec![observation("a", EvidenceState::Supported)]);
        input.allowed_artifacts = vec!["raw-data".into()];
        let receipt = admit_federated_evidence(&input).unwrap();
        assert_eq!(receipt.disposition, FederatedEvidenceDisposition::Blocked);
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn unknown_and_contradicted_are_retained() {
        let receipt = admit_federated_evidence(&request(vec![
            observation("a", EvidenceState::Unknown),
            observation("b", EvidenceState::Contradicted),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, FederatedEvidenceDisposition::Unknown);
        assert!(!receipt.unknown_order.is_empty());
        assert!(!receipt.negative_evidence.is_empty());
    }
    #[test]
    fn signer_failure_is_explicit() {
        let mut input = request(vec![observation("a", EvidenceState::Supported)]);
        input.signer_valid = false;
        let receipt = admit_federated_evidence(&input).unwrap();
        assert_eq!(receipt.disposition, FederatedEvidenceDisposition::Blocked);
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|item| item.contains("signer")));
    }
    #[test]
    fn duplicate_observation_is_rejected() {
        let mut duplicate = observation("a", EvidenceState::Supported);
        duplicate.source_id = "source:other".into();
        assert!(admit_federated_evidence(&request(vec![
            observation("a", EvidenceState::Supported),
            duplicate
        ]))
        .is_err());
    }
}
