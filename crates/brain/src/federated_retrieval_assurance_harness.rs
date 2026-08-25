//! Federated retrieval assurance harness.
//!
//! Atlas feature: `AFA-brain-P02-F28`. Federation release is qualified only when purpose,
//! signer, approval, aggregate-only, comparability, replay, and locality witnesses hold.

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

pub const FEATURE_ID: &str = "AFA-brain-P02-F28";
pub const CONTRACT_VERSION: &str = "brain-federated-retrieval-assurance-harness/1.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedRetrievalAssuranceVerdict {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedRetrievalAssuranceReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub federation_id: String,
    pub institution_id: String,
    pub purpose: String,
    pub endpoint: String,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub verdict: FederatedRetrievalAssuranceVerdict,
    pub candidate_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub aggregate_order: Vec<ContentHash>,
    pub witness_order: Vec<String>,
    pub counterexample_order: Vec<String>,
    pub comparability_digest: ContentHash,
    pub envelope_digest: ContentHash,
    pub synthesis_digest: ContentHash,
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
pub enum FederatedRetrievalAssuranceError {
    #[error("invalid federated retrieval assurance request: {0}")]
    Invalid(String),
    #[error("federated retrieval assurance artifact failed: {0}")]
    Artifact(String),
    #[error("federated retrieval assurance synthesis failed: {0}")]
    Engine(String),
}

impl FederatedRetrievalAssuranceReceipt {
    pub fn validate(&self) -> Result<(), FederatedRetrievalAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.institution_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.endpoint.trim().is_empty()
            || self.study_order.len() < 2
            || self.modality_order.len() < 2
            || self.candidate_order.is_empty()
            || self.witness_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(FederatedRetrievalAssuranceError::Invalid("federated assurance identity, closure, witnesses, locality, or effects are incomplete".into()));
        }
        if self
            .qualified_order
            .iter()
            .chain(self.blocked_order.iter())
            .chain(self.unknown_order.iter())
            .any(|id| !self.candidate_order.contains(id))
        {
            return Err(FederatedRetrievalAssuranceError::Invalid(
                "federated assurance state is not covered by candidates".into(),
            ));
        }
        if self
            .aggregate_order
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        {
            return Err(FederatedRetrievalAssuranceError::Invalid(
                "federated aggregate order is not canonical".into(),
            ));
        }
        for values in [
            &self.study_order,
            &self.modality_order,
            &self.candidate_order,
            &self.qualified_order,
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
                return Err(FederatedRetrievalAssuranceError::Invalid(
                    "federated assurance ordering is not canonical".into(),
                ));
            }
        }
        for digest in [
            &self.comparability_digest,
            &self.envelope_digest,
            &self.synthesis_digest,
            &self.verification_digest,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(FederatedRetrievalAssuranceError::Invalid(
                    "federated assurance digest is invalid".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("assurance:local-federated-retrieval:")
                && effect != "block:unsafe-release"
        }) {
            return Err(FederatedRetrievalAssuranceError::Invalid(
                "federated assurance effect is outside the local gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| FederatedRetrievalAssuranceError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, FederatedRetrievalAssuranceError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| FederatedRetrievalAssuranceError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| FederatedRetrievalAssuranceError::Artifact(error.to_string()))
    }
}

pub fn federated_retrieval_assurance_harness_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["federation research steward".into(), "federated release gate".into()].into(), behavior: "verifies federated retrieval with purpose, signer, approval, aggregate-envelope, comparability, replay, and locality witnesses".into(), value: "prevents unauthorized data movement or incomplete cross-institution retrieval from being promoted".into(), inputs: vec![TypedPort { name: "federated_retrieval_query".into(), schema: "ScopedRetrievalQuery4@1".into(), required: true }], outputs: vec![TypedPort { name: "federated_retrieval_assurance".into(), schema: "FederatedRetrievalAssuranceReceipt1@1".into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["evaluate:retrieval-runs".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }], authority_requirements: vec![AuthorityRequirement { role: "federated retrieval approver".into(), reason: "approve purpose-bound aggregate-only evidence exchange after signer, comparability, and locality gates close".into() }], autonomy_tier: AutonomyTier::A2, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn verify_federated_retrieval_assurance(
    request: &FederatedRetrievalQuery,
) -> Result<FederatedRetrievalAssuranceReceipt, FederatedRetrievalAssuranceError> {
    let synthesis = synthesize_federated_retrieval(request)
        .map_err(|error| FederatedRetrievalAssuranceError::Engine(error.to_string()))?;
    let mut witnesses = BTreeSet::from([
        "gate:typed-contract".to_string(),
        "gate:purpose".to_string(),
        "gate:signer".to_string(),
        "gate:approval".to_string(),
        "gate:aggregate-only".to_string(),
        "gate:comparability".to_string(),
        "gate:replay-identity".to_string(),
        "gate:locality".to_string(),
        "gate:effect-allow-list".to_string(),
    ]);
    let mut counterexamples = BTreeSet::new();
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
    if !request.approval_valid {
        counterexamples.insert("counterexample:approval-invalid".into());
        omissions.insert("assurance:approval-invalid".into());
    }
    if !request.raw_data_local {
        counterexamples.insert("counterexample:raw-data-locality-failed".into());
        omissions.insert("assurance:raw-data-locality-failed".into());
    }
    if synthesis.disposition != FederatedRetrievalDisposition::Qualified {
        witnesses.insert("gate:non-qualified-federated-evidence-retained".into());
    }
    let verdict = if !request.policy_allow
        || !request.protected_closure
        || !request.signer_valid
        || !request.approval_valid
        || !request.raw_data_local
        || !counterexamples.is_empty()
        || synthesis.disposition == FederatedRetrievalDisposition::Blocked
    {
        FederatedRetrievalAssuranceVerdict::Blocked
    } else if synthesis.disposition == FederatedRetrievalDisposition::Qualified {
        FederatedRetrievalAssuranceVerdict::Qualified
    } else {
        FederatedRetrievalAssuranceVerdict::Unresolved
    };
    let verification_digest = ContentHash::of_value(&json!({"feature_id": FEATURE_ID, "request_id": request.request_id, "federation_id": request.federation_id, "institution_id": request.institution_id, "candidate_order": synthesis.candidate_order, "witness_order": witnesses, "counterexample_order": counterexamples, "verdict": verdict, "replay_identity": request.replay_identity})).map_err(|error| FederatedRetrievalAssuranceError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "federation_id": request.federation_id, "institution_id": request.institution_id, "purpose": request.purpose, "endpoint": request.endpoint, "study_order": request.study_ids, "modality_order": request.required_modalities, "verdict": verdict, "candidate_order": synthesis.candidate_order, "qualified_order": synthesis.qualified_order, "blocked_order": synthesis.blocked_order, "unknown_order": synthesis.unknown_order, "aggregate_order": synthesis.aggregate_order, "witness_order": witnesses, "counterexample_order": counterexamples, "comparability_digest": synthesis.comparability_digest, "envelope_digest": synthesis.envelope_digest, "synthesis_digest": synthesis.synthesis_digest, "verification_digest": verification_digest, "replay_identity": request.replay_identity, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-federated-retrieval-assurance:{}", request.request_id),
        "application/vnd.aurora.federated-retrieval-assurance+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| FederatedRetrievalAssuranceError::Artifact(error.to_string()))?;
    let receipt = FederatedRetrievalAssuranceReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        institution_id: request.institution_id.clone(),
        purpose: request.purpose.clone(),
        endpoint: request.endpoint.clone(),
        study_order: request.study_ids.clone(),
        modality_order: request.required_modalities.clone(),
        verdict,
        candidate_order: synthesis.candidate_order,
        qualified_order: synthesis.qualified_order,
        blocked_order: synthesis.blocked_order,
        unknown_order: synthesis.unknown_order,
        aggregate_order: synthesis.aggregate_order,
        witness_order: witnesses.into_iter().collect(),
        counterexample_order: counterexamples.into_iter().collect(),
        comparability_digest: synthesis.comparability_digest,
        envelope_digest: synthesis.envelope_digest,
        synthesis_digest: synthesis.synthesis_digest,
        verification_digest,
        replay_identity: request.replay_identity.clone(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts: if verdict == FederatedRetrievalAssuranceVerdict::Qualified {
            vec![format!(
                "assurance:local-federated-retrieval:{}",
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::retrieval_synthesis::RetrievalCandidate;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn request() -> FederatedRetrievalQuery {
        FederatedRetrievalQuery {
            request_id: "request:federated-assurance".into(),
            federation_id: "federation:one".into(),
            institution_id: "institution:a".into(),
            purpose: "preclinical-evidence-benchmark".into(),
            semantic_profile: "profile:organoid-v1".into(),
            endpoint: "endpoint:local".into(),
            allowed_artifacts: vec!["qualified-evidence-summary".into()],
            study_ids: vec!["study:a".into(), "study:b".into()],
            scope: "organoid:neural".into(),
            minimum_support_milli: 700,
            required_modalities: vec!["imaging".into(), "transcriptomics".into()],
            candidates: vec![RetrievalCandidate {
                evidence_id: "evidence:federated".into(),
                source_id: "source:federated".into(),
                study_id: "study:a".into(),
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
        }
    }
    #[test]
    fn manifest_is_a2() {
        let manifest = federated_retrieval_assurance_harness_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A2);
    }
    #[test]
    fn signer_denial_blocks() {
        let mut value = request();
        value.signer_valid = false;
        let receipt = verify_federated_retrieval_assurance(&value).unwrap();
        assert_eq!(receipt.verdict, FederatedRetrievalAssuranceVerdict::Blocked);
    }
    #[test]
    fn locality_witness_is_retained() {
        let receipt = verify_federated_retrieval_assurance(&request()).unwrap();
        assert!(receipt
            .witness_order
            .iter()
            .any(|value| value == "gate:locality"));
    }
    #[test]
    fn digest_is_stable() {
        let receipt = verify_federated_retrieval_assurance(&request()).unwrap();
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
}
