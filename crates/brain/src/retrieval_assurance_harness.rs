//! Local retrieval assurance harness.
//!
//! Atlas feature: `AFA-brain-P02-F25`. This release-facing verifier checks a retrieval synthesis
//! receipt with witnesses, counterexamples, provenance, replay, omission, and fail-closed gates.

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

pub const FEATURE_ID: &str = "AFA-brain-P02-F25";
pub const CONTRACT_VERSION: &str = "brain-retrieval-assurance-harness/1.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetrievalAssuranceVerdict {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalAssuranceReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub study_id: String,
    pub scope: String,
    pub verdict: RetrievalAssuranceVerdict,
    pub candidate_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub witness_order: Vec<String>,
    pub counterexample_order: Vec<String>,
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
pub enum RetrievalAssuranceError {
    #[error("invalid retrieval assurance request: {0}")]
    Invalid(String),
    #[error("retrieval assurance artifact failed: {0}")]
    Artifact(String),
    #[error("retrieval assurance synthesis failed: {0}")]
    Engine(String),
}

impl RetrievalAssuranceReceipt {
    pub fn validate(&self) -> Result<(), RetrievalAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.study_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.witness_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(RetrievalAssuranceError::Invalid(
                "retrieval assurance identity, witnesses, locality, or effects are incomplete"
                    .into(),
            ));
        }
        if self
            .qualified_order
            .iter()
            .chain(self.blocked_order.iter())
            .chain(self.unknown_order.iter())
            .any(|id| !self.candidate_order.contains(id))
        {
            return Err(RetrievalAssuranceError::Invalid(
                "retrieval assurance state is not covered by candidates".into(),
            ));
        }
        for values in [
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
                return Err(RetrievalAssuranceError::Invalid(
                    "retrieval assurance ordering is not canonical".into(),
                ));
            }
        }
        for digest in [
            &self.synthesis_digest,
            &self.verification_digest,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(RetrievalAssuranceError::Invalid(
                    "retrieval assurance digest is invalid".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("assurance:local-retrieval:") && effect != "block:unsafe-release"
        }) {
            return Err(RetrievalAssuranceError::Invalid(
                "retrieval assurance effect is outside the local release gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| RetrievalAssuranceError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, RetrievalAssuranceError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| RetrievalAssuranceError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| RetrievalAssuranceError::Artifact(error.to_string()))
    }
}

pub fn retrieval_assurance_harness_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "brain".into(),
        consumers: ["research workflow operator".into(), "local release gate".into()].into(),
        behavior: "verifies local retrieval synthesis with provenance, replay, omission, witness, counterexample, and fail-closed release gates".into(),
        value: "prevents unresolved or unauthorized retrieval evidence from being promoted as a qualified research result".into(),
        inputs: vec![TypedPort { name: "scoped_retrieval_query".into(), schema: "ScopedRetrievalQuery1@1".into(), required: true }],
        outputs: vec![TypedPort { name: "retrieval_assurance".into(), schema: "RetrievalAssuranceReceipt1@1".into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(),
        permissions: ["evaluate:retrieval-runs".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "slsa-provenance-1.2".into(), state: EvidenceState::Supported, locator: Some("https://slsa.dev/spec/v1.2/provenance".into()) }],
        authority_requirements: Vec::new(),
        autonomy_tier: AutonomyTier::A0,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn verify_retrieval_assurance(
    request: &ScopedRetrievalQuery,
) -> Result<RetrievalAssuranceReceipt, RetrievalAssuranceError> {
    let synthesis = synthesize_retrieval(request)
        .map_err(|error| RetrievalAssuranceError::Engine(error.to_string()))?;
    let mut witnesses = BTreeSet::from([
        "gate:typed-contract".to_string(),
        "gate:protected-closure".to_string(),
        "gate:provenance".to_string(),
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
    if !request.raw_data_local {
        counterexamples.insert("counterexample:raw-data-locality-failed".into());
        omissions.insert("assurance:raw-data-locality-failed".into());
    }
    for candidate in &request.candidates {
        if candidate.replay_identity != request.replay_identity {
            counterexamples.insert(format!(
                "counterexample:{}:replay-mismatch",
                candidate.evidence_id
            ));
        }
        if !candidate.omissions.is_empty() {
            counterexamples.insert(format!("counterexample:{}:omission", candidate.evidence_id));
        }
    }
    if synthesis.disposition != SynthesisDisposition::Qualified {
        witnesses.insert("gate:non-qualified-evidence-retained".into());
    }
    let verdict = if !request.policy_allow
        || !request.protected_closure
        || !request.raw_data_local
        || !counterexamples.is_empty()
        || synthesis.disposition == SynthesisDisposition::Blocked
    {
        RetrievalAssuranceVerdict::Blocked
    } else if synthesis.disposition == SynthesisDisposition::Qualified {
        RetrievalAssuranceVerdict::Qualified
    } else {
        RetrievalAssuranceVerdict::Unresolved
    };
    let synthesis_digest = synthesis
        .digest()
        .map_err(|error| RetrievalAssuranceError::Engine(error.to_string()))?;
    let verification_digest = ContentHash::of_value(&json!({"feature_id": FEATURE_ID, "request_id": request.request_id, "candidate_order": synthesis.candidate_order, "witness_order": witnesses, "counterexample_order": counterexamples, "verdict": verdict, "replay_identity": request.replay_identity}))
        .map_err(|error| RetrievalAssuranceError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "study_id": request.study_id, "scope": request.scope, "verdict": verdict, "candidate_order": synthesis.candidate_order, "qualified_order": synthesis.qualified_order, "blocked_order": synthesis.blocked_order, "unknown_order": synthesis.unknown_order, "witness_order": witnesses, "counterexample_order": counterexamples, "synthesis_digest": synthesis_digest, "verification_digest": verification_digest, "replay_identity": request.replay_identity, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-retrieval-assurance:{}", request.request_id),
        "application/vnd.aurora.retrieval-assurance+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| RetrievalAssuranceError::Artifact(error.to_string()))?;
    let receipt = RetrievalAssuranceReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        study_id: request.study_id.clone(),
        scope: request.scope.clone(),
        verdict,
        candidate_order: synthesis.candidate_order,
        qualified_order: synthesis.qualified_order,
        blocked_order: synthesis.blocked_order,
        unknown_order: synthesis.unknown_order,
        witness_order: witnesses.into_iter().collect(),
        counterexample_order: counterexamples.into_iter().collect(),
        synthesis_digest,
        verification_digest,
        replay_identity: request.replay_identity.clone(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts: if verdict == RetrievalAssuranceVerdict::Qualified {
            vec![format!("assurance:local-retrieval:{}", request.request_id)]
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
    fn request(state: EvidenceState) -> ScopedRetrievalQuery {
        ScopedRetrievalQuery {
            request_id: "request:retrieval-assurance".into(),
            study_id: "study:organoid".into(),
            scope: "organoid:neural".into(),
            query: "synaptic morphology".into(),
            minimum_support_milli: 700,
            candidates: vec![RetrievalCandidate {
                evidence_id: "evidence:assurance".into(),
                source_id: "source:assurance".into(),
                study_id: "study:organoid".into(),
                scope: "organoid:neural".into(),
                modality: "imaging".into(),
                support_milli: 900,
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
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a0() {
        let manifest = retrieval_assurance_harness_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A0);
    }
    #[test]
    fn qualified_retrieval_has_witnesses() {
        let receipt = verify_retrieval_assurance(&request(EvidenceState::Supported)).unwrap();
        assert_eq!(receipt.verdict, RetrievalAssuranceVerdict::Qualified);
        assert!(receipt
            .witness_order
            .iter()
            .any(|value| value == "gate:provenance"));
    }
    #[test]
    fn unknown_retrieval_is_unresolved() {
        let receipt = verify_retrieval_assurance(&request(EvidenceState::Unknown)).unwrap();
        assert_eq!(receipt.verdict, RetrievalAssuranceVerdict::Unresolved);
    }
    #[test]
    fn policy_denial_blocks() {
        let mut value = request(EvidenceState::Supported);
        value.policy_allow = false;
        let receipt = verify_retrieval_assurance(&value).unwrap();
        assert_eq!(receipt.verdict, RetrievalAssuranceVerdict::Blocked);
    }
    #[test]
    fn digest_is_stable() {
        let receipt = verify_retrieval_assurance(&request(EvidenceState::Supported)).unwrap();
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
}
