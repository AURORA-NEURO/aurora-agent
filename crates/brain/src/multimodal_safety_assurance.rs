//! Multimodal multi-study evidence verification and safety assurance harness.
//!
//! Atlas feature: `AFA-brain-P01-F26`. Study/modality closure and comparability are release
//! predicates; an incomplete modality set is never promoted to a qualified result.

use crate::multimodal_evidence_surveillance::{
    surveil_multimodal_evidence, MultimodalEvidenceDisposition, MultimodalEvidenceFeedRequest,
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

pub const FEATURE_ID: &str = "AFA-brain-P01-F26";
pub const CONTRACT_VERSION: &str = "brain-multimodal-evidence-assurance/1.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultimodalAssuranceVerdict {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalAssuranceReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub scope: String,
    pub verdict: MultimodalAssuranceVerdict,
    pub candidate_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub witness_order: Vec<String>,
    pub counterexample_order: Vec<String>,
    pub evidence_digest: ContentHash,
    pub comparability_digest: ContentHash,
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
pub enum MultimodalAssuranceError {
    #[error("invalid multimodal assurance request: {0}")]
    Invalid(String),
    #[error("multimodal assurance artifact failed: {0}")]
    Artifact(String),
    #[error("multimodal assurance engine failed: {0}")]
    Engine(String),
}

impl MultimodalAssuranceReceipt {
    pub fn validate(&self) -> Result<(), MultimodalAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.study_order.len() < 2
            || self.modality_order.len() < 2
            || self.scope.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.witness_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(MultimodalAssuranceError::Invalid("multimodal assurance identity, coverage, witness, locality, or effects are incomplete".into()));
        }
        if self
            .qualified_order
            .iter()
            .chain(self.blocked_order.iter())
            .chain(self.unknown_order.iter())
            .any(|id| !self.candidate_order.contains(id))
        {
            return Err(MultimodalAssuranceError::Invalid(
                "multimodal assurance state is not covered by candidates".into(),
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
                return Err(MultimodalAssuranceError::Invalid(
                    "multimodal assurance ordering is not canonical".into(),
                ));
            }
        }
        for value in [
            &self.evidence_digest,
            &self.comparability_digest,
            &self.verification_digest,
            &self.replay_identity,
        ] {
            if value.as_str().len() != 64 {
                return Err(MultimodalAssuranceError::Invalid(
                    "multimodal assurance digest is invalid".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("assurance:local-multimodal:") && effect != "block:unsafe-release"
        }) {
            return Err(MultimodalAssuranceError::Invalid(
                "multimodal assurance effect is outside the local gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| MultimodalAssuranceError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, MultimodalAssuranceError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| MultimodalAssuranceError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| MultimodalAssuranceError::Artifact(error.to_string()))
    }
}

pub fn multimodal_safety_assurance_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["research workflow operator".into(), "multimodal release gate".into()].into(), behavior: "verifies multi-study evidence with modality closure, comparability, provenance, replay, and fail-closed witnesses".into(), value: "prevents incomplete imaging and omics coverage from being presented as a comparable research result".into(), inputs: vec![TypedPort { name: "multimodal_evidence_feed".into(), schema: "EvidenceFeed2@1".into(), required: true }], outputs: vec![TypedPort { name: "multimodal_assurance".into(), schema: "QualifiedEvidenceSet7@1".into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["evaluate:capability-runs".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "slsa-provenance-1.2".into(), state: EvidenceState::Supported, locator: Some("https://slsa.dev/spec/v1.2/provenance".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn verify_multimodal_safety(
    request: &MultimodalEvidenceFeedRequest,
) -> Result<MultimodalAssuranceReceipt, MultimodalAssuranceError> {
    let evidence = surveil_multimodal_evidence(request)
        .map_err(|error| MultimodalAssuranceError::Engine(error.to_string()))?;
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
    let study_order = request.study_ids.iter().cloned().collect::<BTreeSet<_>>();
    let modality_order = request
        .required_modalities
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let observed_studies = request
        .observations
        .iter()
        .map(|item| item.study_id.clone())
        .collect::<BTreeSet<_>>();
    let observed_modalities = request
        .observations
        .iter()
        .map(|item| item.modality.clone())
        .collect::<BTreeSet<_>>();
    let mut witnesses = BTreeSet::from([
        "gate:typed-contract".to_string(),
        "gate:study-floor".to_string(),
        "gate:modality-floor".to_string(),
        "gate:comparability".to_string(),
        "gate:provenance".to_string(),
        "gate:replay-identity".to_string(),
        "gate:locality".to_string(),
        "gate:effect-allow-list".to_string(),
    ]);
    let mut counterexamples = BTreeSet::new();
    for study in study_order.difference(&observed_studies) {
        counterexamples.insert(format!("counterexample:study:{}:missing", study));
    }
    for modality in modality_order.difference(&observed_modalities) {
        counterexamples.insert(format!("counterexample:modality:{}:missing", modality));
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
    }
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
    if evidence.disposition != MultimodalEvidenceDisposition::Qualified {
        witnesses.insert("gate:non-qualified-evidence-retained".into());
    }
    let verdict = if !request.policy_allow
        || !request.protected_closure
        || !request.raw_data_local
        || !counterexamples.is_empty()
    {
        MultimodalAssuranceVerdict::Blocked
    } else if evidence.disposition == MultimodalEvidenceDisposition::Qualified {
        MultimodalAssuranceVerdict::Qualified
    } else {
        MultimodalAssuranceVerdict::Unresolved
    };
    let evidence_digest = evidence
        .digest()
        .map_err(|error| MultimodalAssuranceError::Engine(error.to_string()))?;
    let comparability_digest = ContentHash::of_value(&json!({"study_order": study_order, "modality_order": modality_order, "candidate_order": evidence.candidate_order, "semantic_order": evidence.semantic_order, "replay_identity": request.replay_identity}))
        .map_err(|error| MultimodalAssuranceError::Artifact(error.to_string()))?;
    let verification_digest = ContentHash::of_value(&json!({"feature_id": FEATURE_ID, "request_id": request.request_id, "witness_order": witnesses, "counterexample_order": counterexamples, "verdict": verdict}))
        .map_err(|error| MultimodalAssuranceError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "study_order": study_order, "modality_order": modality_order, "scope": request.scope, "verdict": verdict, "candidate_order": evidence.candidate_order, "qualified_order": evidence.qualified_order, "blocked_order": evidence.blocked_order, "unknown_order": evidence.unknown_order, "witness_order": witnesses, "counterexample_order": counterexamples, "evidence_digest": evidence_digest, "comparability_digest": comparability_digest, "verification_digest": verification_digest, "replay_identity": request.replay_identity, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-multimodal-assurance:{}", request.request_id),
        "application/vnd.aurora.multimodal-assurance+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| MultimodalAssuranceError::Artifact(error.to_string()))?;
    let receipt = MultimodalAssuranceReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        study_order: study_order.into_iter().collect(),
        modality_order: modality_order.into_iter().collect(),
        scope: request.scope.clone(),
        verdict,
        candidate_order: evidence.candidate_order.clone(),
        qualified_order: evidence.qualified_order.clone(),
        blocked_order: evidence.blocked_order.clone(),
        unknown_order: evidence.unknown_order.clone(),
        witness_order: witnesses.into_iter().collect(),
        counterexample_order: counterexamples.into_iter().collect(),
        evidence_digest,
        comparability_digest,
        verification_digest,
        replay_identity: request.replay_identity.clone(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        effect_receipts: if verdict == MultimodalAssuranceVerdict::Qualified {
            vec![format!("assurance:local-multimodal:{}", request.request_id)]
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
    fn request(state: EvidenceState) -> MultimodalEvidenceFeedRequest {
        MultimodalEvidenceFeedRequest {
            request_id: "request:multimodal-assurance".into(),
            study_ids: vec!["study:a".into(), "study:b".into()],
            scope: "organoid:neural".into(),
            query: "synaptic morphology".into(),
            minimum_relevance_milli: 700,
            required_modalities: vec!["imaging".into(), "transcriptomics".into()],
            observations: vec![
                EvidenceObservation {
                    evidence_id: "evidence:a".into(),
                    source_id: "source:a".into(),
                    study_id: "study:a".into(),
                    modality: "imaging".into(),
                    scope: "organoid:neural".into(),
                    relevance_milli: 900,
                    state,
                    semantic_digest: hash("semantic:a"),
                    artifact_digest: hash("artifact:a"),
                    provenance_digest: hash("provenance:a"),
                    replay_identity: hash("replay"),
                    omissions: Vec::new(),
                    negative_evidence: Vec::new(),
                    raw_data_local: true,
                    boundary: PRECLINICAL_BOUNDARY.into(),
                },
                EvidenceObservation {
                    evidence_id: "evidence:b".into(),
                    source_id: "source:b".into(),
                    study_id: "study:b".into(),
                    modality: "transcriptomics".into(),
                    scope: "organoid:neural".into(),
                    relevance_milli: 900,
                    state,
                    semantic_digest: hash("semantic:b"),
                    artifact_digest: hash("artifact:b"),
                    provenance_digest: hash("provenance:b"),
                    replay_identity: hash("replay"),
                    omissions: Vec::new(),
                    negative_evidence: Vec::new(),
                    raw_data_local: true,
                    boundary: PRECLINICAL_BOUNDARY.into(),
                },
            ],
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        let m = multimodal_safety_assurance_manifest();
        m.validate().unwrap();
        assert_eq!(m.autonomy_tier, AutonomyTier::A1);
    }
    #[test]
    fn complete_is_qualified() {
        let r = verify_multimodal_safety(&request(EvidenceState::Supported)).unwrap();
        assert_eq!(r.verdict, MultimodalAssuranceVerdict::Qualified);
    }
    #[test]
    fn unknown_is_unresolved() {
        let r = verify_multimodal_safety(&request(EvidenceState::Unknown)).unwrap();
        assert_eq!(r.verdict, MultimodalAssuranceVerdict::Unresolved);
    }
    #[test]
    fn missing_modality_is_blocked() {
        let mut q = request(EvidenceState::Supported);
        q.required_modalities = vec!["electrophysiology".into(), "imaging".into()];
        let r = verify_multimodal_safety(&q).unwrap();
        assert_eq!(r.verdict, MultimodalAssuranceVerdict::Blocked);
    }
    #[test]
    fn policy_is_blocked() {
        let mut q = request(EvidenceState::Supported);
        q.policy_allow = false;
        let r = verify_multimodal_safety(&q).unwrap();
        assert_eq!(r.verdict, MultimodalAssuranceVerdict::Blocked);
    }
    #[test]
    fn digest_is_stable() {
        let r = verify_multimodal_safety(&request(EvidenceState::Supported)).unwrap();
        assert_eq!(r.digest().unwrap(), r.digest().unwrap());
    }
}
