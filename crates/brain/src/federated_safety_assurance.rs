//! Federated continual evidence verification and safety assurance harness.
//!
//! Atlas feature: `AFA-brain-P01-F28`. The verifier certifies only purpose-bound aggregate
//! metadata; signer, locality, permitted-artifact, and replay failures stay explicit.

use crate::federated_evidence_surveillance::{
    admit_federated_evidence, FederatedEvidenceDisposition, FederatedEvidenceFeedRequest,
    PERMITTED_ARTIFACT,
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

pub const FEATURE_ID: &str = "AFA-brain-P01-F28";
pub const CONTRACT_VERSION: &str = "brain-federated-evidence-assurance/1.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedAssuranceVerdict {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedAssuranceReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub institution_id: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub endpoint: String,
    pub verdict: FederatedAssuranceVerdict,
    pub candidate_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub aggregate_order: Vec<ContentHash>,
    pub witness_order: Vec<String>,
    pub counterexample_order: Vec<String>,
    pub envelope_digest: ContentHash,
    pub verification_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedAssuranceError {
    #[error("invalid federated assurance request: {0}")]
    Invalid(String),
    #[error("federated assurance artifact failed: {0}")]
    Artifact(String),
    #[error("federated assurance engine failed: {0}")]
    Engine(String),
}

impl FederatedAssuranceReceipt {
    pub fn validate(&self) -> Result<(), FederatedAssuranceError> {
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
            || self.witness_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(FederatedAssuranceError::Invalid("federated assurance identity, witness coverage, locality, or effects are incomplete".into()));
        }
        if self
            .admitted_order
            .iter()
            .chain(self.blocked_order.iter())
            .chain(self.unknown_order.iter())
            .any(|id| !self.candidate_order.contains(id))
        {
            return Err(FederatedAssuranceError::Invalid(
                "federated assurance state is not covered by candidates".into(),
            ));
        }
        for values in [
            &self.candidate_order,
            &self.admitted_order,
            &self.blocked_order,
            &self.unknown_order,
            &self.witness_order,
            &self.counterexample_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(FederatedAssuranceError::Invalid(
                    "federated assurance ordering is not canonical".into(),
                ));
            }
        }
        if self
            .aggregate_order
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(FederatedAssuranceError::Invalid(
                "federated aggregate ordering is not canonical".into(),
            ));
        }
        for value in [
            &self.envelope_digest,
            &self.verification_digest,
            &self.replay_identity,
        ] {
            if value.as_str().len() != 64 {
                return Err(FederatedAssuranceError::Invalid(
                    "federated assurance digest is invalid".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("assurance:federated:") && effect != "block:unsafe-release"
        }) {
            return Err(FederatedAssuranceError::Invalid(
                "federated assurance effect is outside the governed gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| FederatedAssuranceError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, FederatedAssuranceError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| FederatedAssuranceError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| FederatedAssuranceError::Artifact(error.to_string()))
    }
}

pub fn federated_safety_assurance_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["platform reliability engineer".into(), "federation release gate".into()].into(), behavior: "verifies policy-separated evidence exchange with purpose, signer, permitted-artifact, aggregate-only, provenance, replay, and locality witnesses".into(), value: "prevents unauthorized data movement or incomplete evidence from being promoted across institutions".into(), inputs: vec![TypedPort { name: "federated_evidence_feed".into(), schema: "EvidenceFeed4@1".into(), required: true }], outputs: vec![TypedPort { name: "federated_assurance".into(), schema: "QualifiedEvidenceSet7@1".into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["evaluate:capability-runs".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn verify_federated_safety(
    request: &FederatedEvidenceFeedRequest,
) -> Result<FederatedAssuranceReceipt, FederatedAssuranceError> {
    let evidence = admit_federated_evidence(request)
        .map_err(|error| FederatedAssuranceError::Engine(error.to_string()))?;
    let mut omissions = evidence.omissions.iter().cloned().collect::<BTreeSet<_>>();
    let uncertainty = evidence
        .uncertainty
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let negative = evidence
        .negative_evidence
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut witnesses = BTreeSet::from([
        "gate:typed-contract".to_string(),
        "gate:purpose-bound".to_string(),
        "gate:permitted-artifact".to_string(),
        "gate:signer".to_string(),
        "gate:aggregate-only".to_string(),
        "gate:provenance".to_string(),
        "gate:replay-identity".to_string(),
        "gate:locality".to_string(),
        "gate:effect-allow-list".to_string(),
    ]);
    let mut counterexamples = BTreeSet::new();
    if !request.policy_allow {
        counterexamples.insert("counterexample:policy-denied".into());
        omissions.insert("assurance:policy-denied".into());
    }
    if !request.protected_closure {
        counterexamples.insert("counterexample:protected-closure-incomplete".into());
        omissions.insert("assurance:protected-closure-incomplete".into());
    }
    if !request.signer_valid {
        counterexamples.insert("counterexample:signer-invalid".into());
        omissions.insert("assurance:signer-invalid".into());
    }
    if !request.raw_data_local {
        counterexamples.insert("counterexample:raw-data-locality-failed".into());
        omissions.insert("assurance:raw-data-locality-failed".into());
    }
    if !request
        .allowed_artifacts
        .iter()
        .any(|item| item == PERMITTED_ARTIFACT)
    {
        counterexamples.insert("counterexample:permitted-artifact-missing".into());
        omissions.insert("assurance:permitted-artifact-missing".into());
    }
    for observation in &request.observations {
        if observation.replay_identity != request.replay_identity {
            counterexamples.insert(format!(
                "counterexample:{}:replay-mismatch",
                observation.evidence_id
            ));
        }
        if !observation.omissions.is_empty() {
            counterexamples.insert(format!(
                "counterexample:{}:omission",
                observation.evidence_id
            ));
        }
        if !observation.raw_data_local {
            counterexamples.insert(format!(
                "counterexample:{}:raw-data-egress",
                observation.evidence_id
            ));
        }
    }
    if evidence.disposition != FederatedEvidenceDisposition::Qualified {
        witnesses.insert("gate:non-qualified-evidence-retained".into());
    }
    let verdict = if !request.policy_allow
        || !request.protected_closure
        || !request.signer_valid
        || !request.raw_data_local
        || !counterexamples.is_empty()
    {
        FederatedAssuranceVerdict::Blocked
    } else if evidence.disposition == FederatedEvidenceDisposition::Qualified {
        FederatedAssuranceVerdict::Qualified
    } else {
        FederatedAssuranceVerdict::Unresolved
    };
    let envelope_digest = ContentHash::of_value(&json!({"federation_id": request.federation_id, "institution_id": request.institution_id, "purpose": request.purpose, "semantic_profile": request.semantic_profile, "candidate_order": evidence.candidate_order, "aggregate_order": evidence.aggregate_order, "replay_identity": request.replay_identity}))
        .map_err(|error| FederatedAssuranceError::Artifact(error.to_string()))?;
    let verification_digest = ContentHash::of_value(&json!({"feature_id": FEATURE_ID, "request_id": request.request_id, "witness_order": witnesses, "counterexample_order": counterexamples, "verdict": verdict}))
        .map_err(|error| FederatedAssuranceError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "federation_id": request.federation_id, "institution_id": request.institution_id, "purpose": request.purpose, "semantic_profile": request.semantic_profile, "endpoint": request.endpoint, "verdict": verdict, "candidate_order": evidence.candidate_order, "admitted_order": evidence.admitted_order, "blocked_order": evidence.blocked_order, "unknown_order": evidence.unknown_order, "aggregate_order": evidence.aggregate_order, "witness_order": witnesses, "counterexample_order": counterexamples, "envelope_digest": envelope_digest, "verification_digest": verification_digest, "replay_identity": request.replay_identity, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-federated-assurance:{}", request.request_id),
        "application/vnd.aurora.federated-assurance+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| FederatedAssuranceError::Artifact(error.to_string()))?;
    let receipt = FederatedAssuranceReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        institution_id: request.institution_id.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        endpoint: request.endpoint.clone(),
        verdict,
        candidate_order: evidence.candidate_order.clone(),
        admitted_order: evidence.admitted_order.clone(),
        blocked_order: evidence.blocked_order.clone(),
        unknown_order: evidence.unknown_order.clone(),
        aggregate_order: evidence.aggregate_order.clone(),
        witness_order: witnesses.into_iter().collect(),
        counterexample_order: counterexamples.into_iter().collect(),
        envelope_digest,
        verification_digest,
        replay_identity: request.replay_identity.clone(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts: if verdict == FederatedAssuranceVerdict::Qualified {
            vec![format!("assurance:federated:{}", request.request_id)]
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence_surveillance::EvidenceObservation;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn request(state: EvidenceState) -> FederatedEvidenceFeedRequest {
        FederatedEvidenceFeedRequest {
            request_id: "request:federated-assurance".into(),
            federation_id: "federation:commons".into(),
            institution_id: "institution:a".into(),
            purpose: "benchmarking".into(),
            semantic_profile: "preclinical-evidence/v1".into(),
            endpoint: "https://hub.example/research".into(),
            allowed_artifacts: vec![PERMITTED_ARTIFACT.into()],
            observations: vec![EvidenceObservation {
                evidence_id: "evidence:a".into(),
                source_id: "source:a".into(),
                study_id: "study:organoid".into(),
                modality: "imaging".into(),
                scope: "organoid:neural".into(),
                relevance_milli: 900,
                state,
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
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        let m = federated_safety_assurance_manifest();
        m.validate().unwrap();
        assert_eq!(m.autonomy_tier, AutonomyTier::A1);
    }
    #[test]
    fn supported_is_qualified() {
        let r = verify_federated_safety(&request(EvidenceState::Supported)).unwrap();
        assert_eq!(r.verdict, FederatedAssuranceVerdict::Qualified);
        assert!(!r.aggregate_order.is_empty());
    }
    #[test]
    fn unknown_is_unresolved() {
        let r = verify_federated_safety(&request(EvidenceState::Unknown)).unwrap();
        assert_eq!(r.verdict, FederatedAssuranceVerdict::Unresolved);
    }
    #[test]
    fn signer_is_blocked() {
        let mut q = request(EvidenceState::Supported);
        q.signer_valid = false;
        let r = verify_federated_safety(&q).unwrap();
        assert_eq!(r.verdict, FederatedAssuranceVerdict::Blocked);
    }
    #[test]
    fn artifact_policy_is_blocked() {
        let mut q = request(EvidenceState::Supported);
        q.allowed_artifacts = vec!["raw-data".into()];
        let r = verify_federated_safety(&q).unwrap();
        assert_eq!(r.verdict, FederatedAssuranceVerdict::Blocked);
    }
    #[test]
    fn digest_is_stable() {
        let r = verify_federated_safety(&request(EvidenceState::Supported)).unwrap();
        assert_eq!(r.digest().unwrap(), r.digest().unwrap());
    }
}
