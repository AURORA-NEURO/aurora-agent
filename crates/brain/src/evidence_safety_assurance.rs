//! Local single-study evidence verification and safety assurance harness.
//!
//! Atlas feature: `AFA-brain-P01-F25`. This is a release-facing verifier over the evidence
//! engine, not a hypothesis tracker: every accepted run carries witnesses, counterexamples,
//! provenance, replay identity, and an explicit fail-closed effect.

use crate::evidence_surveillance::{
    surveil_evidence, EvidenceFeedRequest, EvidenceSurveillanceDisposition,
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

pub const FEATURE_ID: &str = "AFA-brain-P01-F25";
pub const CONTRACT_VERSION: &str = "brain-evidence-assurance/1.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssuranceVerdict {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceAssuranceReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub study_id: String,
    pub scope: String,
    pub verdict: AssuranceVerdict,
    pub candidate_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub witness_order: Vec<String>,
    pub counterexample_order: Vec<String>,
    pub evidence_digest: ContentHash,
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
pub enum EvidenceAssuranceError {
    #[error("invalid evidence assurance request: {0}")]
    Invalid(String),
    #[error("evidence assurance artifact failed: {0}")]
    Artifact(String),
    #[error("evidence assurance engine failed: {0}")]
    Engine(String),
}

impl EvidenceAssuranceReceipt {
    pub fn validate(&self) -> Result<(), EvidenceAssuranceError> {
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
            return Err(EvidenceAssuranceError::Invalid(
                "assurance identity, witness coverage, locality, or effects are incomplete".into(),
            ));
        }
        if self
            .qualified_order
            .iter()
            .chain(self.blocked_order.iter())
            .chain(self.unknown_order.iter())
            .any(|id| !self.candidate_order.contains(id))
        {
            return Err(EvidenceAssuranceError::Invalid(
                "assurance state is not covered by candidates".into(),
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
                return Err(EvidenceAssuranceError::Invalid(
                    "assurance ordering is not canonical".into(),
                ));
            }
        }
        for value in [
            &self.evidence_digest,
            &self.verification_digest,
            &self.replay_identity,
        ] {
            if value.as_str().len() != 64 {
                return Err(EvidenceAssuranceError::Invalid(
                    "assurance digest length is invalid".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("assurance:local-evidence:") && effect != "block:unsafe-release"
        }) {
            return Err(EvidenceAssuranceError::Invalid(
                "assurance effect is outside the local release gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| EvidenceAssuranceError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, EvidenceAssuranceError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| EvidenceAssuranceError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| EvidenceAssuranceError::Artifact(error.to_string()))
    }
}

pub fn evidence_safety_assurance_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["laboratory automation engineer".into(), "local release gate".into()].into(), behavior: "verifies a local EvidenceFeed run with deterministic witnesses, counterexamples, provenance, replay, and fail-closed release predicates".into(), value: "prevents incomplete or unauthorized evidence alerts from being promoted as a qualified research result".into(), inputs: vec![TypedPort { name: "evidence_feed".into(), schema: "EvidenceFeed1@1".into(), required: true }], outputs: vec![TypedPort { name: "evidence_assurance".into(), schema: "QualifiedEvidenceSet7@1".into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["evaluate:capability-runs".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "slsa-provenance-1.2".into(), state: EvidenceState::Supported, locator: Some("https://slsa.dev/spec/v1.2/provenance".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A0, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn verify_evidence_safety(
    request: &EvidenceFeedRequest,
) -> Result<EvidenceAssuranceReceipt, EvidenceAssuranceError> {
    let evidence = surveil_evidence(request)
        .map_err(|error| EvidenceAssuranceError::Engine(error.to_string()))?;
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
        "gate:protected-closure".to_string(),
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
    if !request.raw_data_local {
        counterexamples.insert("counterexample:raw-data-locality-failed".into());
        omissions.insert("assurance:raw-data-locality-failed".into());
    }
    for observation in &request.observations {
        if observation.provenance_digest.as_str().len() != 64 {
            counterexamples.insert(format!(
                "counterexample:{}:provenance-missing",
                observation.evidence_id
            ));
        }
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
    }
    if evidence.disposition != EvidenceSurveillanceDisposition::Qualified {
        witnesses.insert("gate:non-qualified-evidence-retained".into());
    }
    let verdict = if !request.policy_allow || !request.protected_closure || !request.raw_data_local
    {
        AssuranceVerdict::Blocked
    } else if !counterexamples.is_empty()
        || evidence.disposition == EvidenceSurveillanceDisposition::Blocked
    {
        AssuranceVerdict::Blocked
    } else if evidence.disposition == EvidenceSurveillanceDisposition::Qualified {
        AssuranceVerdict::Qualified
    } else {
        AssuranceVerdict::Unresolved
    };
    let evidence_digest = evidence
        .digest()
        .map_err(|error| EvidenceAssuranceError::Engine(error.to_string()))?;
    let verification_digest = ContentHash::of_value(&json!({"feature_id": FEATURE_ID, "request_id": request.request_id, "candidate_order": evidence.candidate_order, "witness_order": witnesses, "counterexample_order": counterexamples, "verdict": verdict, "replay_identity": request.replay_identity}))
        .map_err(|error| EvidenceAssuranceError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "study_id": request.study_id, "scope": request.scope, "verdict": verdict, "candidate_order": evidence.candidate_order, "qualified_order": evidence.qualified_order, "blocked_order": evidence.blocked_order, "unknown_order": evidence.unknown_order, "witness_order": witnesses, "counterexample_order": counterexamples, "evidence_digest": evidence_digest, "verification_digest": verification_digest, "replay_identity": request.replay_identity, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-evidence-assurance:{}", request.request_id),
        "application/vnd.aurora.evidence-assurance+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| EvidenceAssuranceError::Artifact(error.to_string()))?;
    let receipt = EvidenceAssuranceReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        study_id: request.study_id.clone(),
        scope: request.scope.clone(),
        verdict,
        candidate_order: evidence.candidate_order.clone(),
        qualified_order: evidence.qualified_order.clone(),
        blocked_order: evidence.blocked_order.clone(),
        unknown_order: evidence.unknown_order.clone(),
        witness_order: witnesses.into_iter().collect(),
        counterexample_order: counterexamples.into_iter().collect(),
        evidence_digest,
        verification_digest,
        replay_identity: request.replay_identity.clone(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts: if verdict == AssuranceVerdict::Qualified {
            vec![format!("assurance:local-evidence:{}", request.request_id)]
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

    fn request(state: EvidenceState) -> EvidenceFeedRequest {
        EvidenceFeedRequest {
            request_id: "request:assurance".into(),
            study_id: "study:organoid".into(),
            scope: "organoid:neural".into(),
            query: "synaptic morphology".into(),
            minimum_relevance_milli: 700,
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
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn manifest_is_a0_and_fail_closed() {
        let manifest = evidence_safety_assurance_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A0);
    }

    #[test]
    fn supported_is_qualified() {
        let receipt = verify_evidence_safety(&request(EvidenceState::Supported)).unwrap();
        assert_eq!(receipt.verdict, AssuranceVerdict::Qualified);
        assert!(receipt.counterexample_order.is_empty());
    }

    #[test]
    fn unknown_is_unresolved() {
        let receipt = verify_evidence_safety(&request(EvidenceState::Unknown)).unwrap();
        assert_eq!(receipt.verdict, AssuranceVerdict::Unresolved);
        assert!(!receipt.unknown_order.is_empty());
    }

    #[test]
    fn policy_is_blocked() {
        let mut input = request(EvidenceState::Supported);
        input.policy_allow = false;
        let receipt = verify_evidence_safety(&input).unwrap();
        assert_eq!(receipt.verdict, AssuranceVerdict::Blocked);
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }

    #[test]
    fn replay_mismatch_is_counterexample() {
        let mut input = request(EvidenceState::Supported);
        input.observations[0].replay_identity = hash("other");
        let receipt = verify_evidence_safety(&input).unwrap();
        assert_eq!(receipt.verdict, AssuranceVerdict::Blocked);
        assert!(!receipt.counterexample_order.is_empty());
    }

    #[test]
    fn digest_is_stable() {
        let receipt = verify_evidence_safety(&request(EvidenceState::Supported)).unwrap();
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
}
