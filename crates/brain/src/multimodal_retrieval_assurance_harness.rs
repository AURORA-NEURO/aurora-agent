//! Multimodal retrieval assurance harness.
//!
//! Atlas feature: `AFA-brain-P02-F26`. This verifier makes study/modality closure and
//! comparability witnesses release predicates for retrieval synthesis.

use crate::multimodal_retrieval_synthesis::{
    synthesize_multimodal_retrieval, MultimodalRetrievalQuery,
};
use crate::retrieval_synthesis::SynthesisDisposition;
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

pub const FEATURE_ID: &str = "AFA-brain-P02-F26";
pub const CONTRACT_VERSION: &str = "brain-multimodal-retrieval-assurance-harness/1.0";
const ASSURANCE_CONTENT_TYPE: &str = "application/vnd.aurora.multimodal-retrieval-assurance+json";
const MAX_TEXT_BYTES: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultimodalRetrievalAssuranceVerdict {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalRetrievalAssuranceReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub scope: String,
    pub verdict: MultimodalRetrievalAssuranceVerdict,
    pub candidate_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub witness_order: Vec<String>,
    pub counterexample_order: Vec<String>,
    pub comparability_digest: ContentHash,
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
pub enum MultimodalRetrievalAssuranceError {
    #[error("invalid multimodal retrieval assurance request: {0}")]
    Invalid(String),
    #[error("multimodal retrieval assurance artifact failed: {0}")]
    Artifact(String),
    #[error("multimodal retrieval assurance synthesis failed: {0}")]
    Engine(String),
}

impl MultimodalRetrievalAssuranceReceipt {
    pub fn validate(&self) -> Result<(), MultimodalRetrievalAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.request_id.trim().is_empty()
            || self.study_order.len() < 2
            || self.modality_order.len() < 2
            || self.scope.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.witness_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(MultimodalRetrievalAssuranceError::Invalid("multimodal retrieval assurance identity, closure, witnesses, locality, or effects are incomplete".into()));
        }
        for (value, field) in [
            (&self.request_id, "request_id"),
            (&self.scope, "scope"),
            (&self.boundary, "boundary"),
        ] {
            validate_text(value, field)?;
        }
        validate_sorted_unique(&self.study_order, "study_order")?;
        validate_sorted_unique(&self.modality_order, "modality_order")?;
        validate_sorted_unique(&self.candidate_order, "candidate_order")?;
        for (values, field) in [
            (&self.qualified_order, "qualified_order"),
            (&self.blocked_order, "blocked_order"),
            (&self.unknown_order, "unknown_order"),
            (&self.witness_order, "witness_order"),
            (&self.counterexample_order, "counterexample_order"),
            (&self.omissions, "omissions"),
            (&self.uncertainty, "uncertainty"),
            (&self.negative_evidence, "negative_evidence"),
            (&self.effect_receipts, "effect_receipts"),
        ] {
            validate_sorted_unique(values, field)?;
        }
        let candidate_keys = identity_keys(&self.candidate_order);
        let qualified_keys = identity_keys(&self.qualified_order);
        let blocked_keys = identity_keys(&self.blocked_order);
        let unknown_keys = identity_keys(&self.unknown_order);
        if !qualified_keys.is_disjoint(&blocked_keys)
            || !unknown_keys.is_subset(&blocked_keys)
            || qualified_keys
                .union(&blocked_keys)
                .cloned()
                .collect::<BTreeSet<_>>()
                != candidate_keys
        {
            return Err(MultimodalRetrievalAssuranceError::Invalid(
                "multimodal assurance candidate states must partition candidates".into(),
            ));
        }
        for digest in [
            &self.comparability_digest,
            &self.synthesis_digest,
            &self.verification_digest,
            &self.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(MultimodalRetrievalAssuranceError::Invalid(
                    "multimodal retrieval assurance digest is invalid".into(),
                ));
            }
        }
        let expected_effect_receipts =
            if self.verdict == MultimodalRetrievalAssuranceVerdict::Qualified {
                vec![format!(
                    "assurance:local-multimodal-retrieval:{}",
                    self.request_id
                )]
            } else {
                vec!["block:unsafe-release".into()]
            };
        if self.effect_receipts != expected_effect_receipts {
            return Err(MultimodalRetrievalAssuranceError::Invalid(
                "multimodal retrieval assurance effect does not match verdict".into(),
            ));
        }
        if !self.raw_data_local
            && (self.verdict != MultimodalRetrievalAssuranceVerdict::Blocked
                || !self
                    .omissions
                    .iter()
                    .any(|item| item == "assurance:raw-data-locality-failed"))
        {
            return Err(MultimodalRetrievalAssuranceError::Invalid(
                "non-local multimodal assurance must be blocked and retain locality evidence"
                    .into(),
            ));
        }
        let expected_verification_digest = ContentHash::of_value(&json!({
            "feature_id": FEATURE_ID,
            "request_id": self.request_id,
            "study_order": self.study_order,
            "modality_order": self.modality_order,
            "candidate_order": self.candidate_order,
            "qualified_order": self.qualified_order,
            "blocked_order": self.blocked_order,
            "unknown_order": self.unknown_order,
            "witness_order": self.witness_order,
            "counterexample_order": self.counterexample_order,
            "verdict": self.verdict,
            "replay_identity": self.replay_identity,
            "raw_data_local": self.raw_data_local,
        }))
        .map_err(|error| MultimodalRetrievalAssuranceError::Artifact(error.to_string()))?;
        if self.verification_digest != expected_verification_digest {
            return Err(MultimodalRetrievalAssuranceError::Invalid(
                "multimodal assurance verification digest is not bound to witnesses".into(),
            ));
        }
        let expected_artifact_id =
            format!("brain-multimodal-retrieval-assurance:{}", self.request_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != ASSURANCE_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(MultimodalRetrievalAssuranceError::Invalid(
                "multimodal assurance artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| MultimodalRetrievalAssuranceError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| MultimodalRetrievalAssuranceError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, MultimodalRetrievalAssuranceError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| MultimodalRetrievalAssuranceError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| MultimodalRetrievalAssuranceError::Artifact(error.to_string()))
    }
}

pub fn multimodal_retrieval_assurance_harness_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "brain".into(), consumers: ["agent developer".into(), "multimodal release gate".into()].into(), behavior: "verifies multimodal retrieval synthesis with study/modality closure, comparability, provenance, replay, and fail-closed witnesses".into(), value: "prevents incomplete imaging and omics coverage from being presented as a comparable retrieval result".into(), inputs: vec![TypedPort { name: "multimodal_retrieval_query".into(), schema: "ScopedRetrievalQuery2@1".into(), required: true }], outputs: vec![TypedPort { name: "multimodal_retrieval_assurance".into(), schema: "MultimodalRetrievalAssuranceReceipt1@1".into(), required: true }], effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["evaluate:retrieval-runs".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "ome-ngff-rfc5".into(), state: EvidenceState::Supported, locator: Some("https://ngff.openmicroscopy.org/rfc/5/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn verify_multimodal_retrieval_assurance(
    request: &MultimodalRetrievalQuery,
) -> Result<MultimodalRetrievalAssuranceReceipt, MultimodalRetrievalAssuranceError> {
    let synthesis = synthesize_multimodal_retrieval(request)
        .map_err(|error| MultimodalRetrievalAssuranceError::Engine(error.to_string()))?;
    let mut witnesses = BTreeSet::from([
        "gate:typed-contract".to_string(),
        "gate:study-closure".to_string(),
        "gate:modality-closure".to_string(),
        "gate:comparability".to_string(),
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
    if synthesis.disposition != SynthesisDisposition::Qualified {
        witnesses.insert("gate:non-qualified-multimodal-evidence-retained".into());
    }
    let verdict = if !request.policy_allow
        || !request.protected_closure
        || !request.raw_data_local
        || !counterexamples.is_empty()
        || synthesis.disposition == SynthesisDisposition::Blocked
    {
        MultimodalRetrievalAssuranceVerdict::Blocked
    } else if synthesis.disposition == SynthesisDisposition::Qualified {
        MultimodalRetrievalAssuranceVerdict::Qualified
    } else {
        MultimodalRetrievalAssuranceVerdict::Unresolved
    };
    let synthesis_digest = synthesis.synthesis_digest.clone();
    let raw_data_local = true;
    let effect_receipts = if verdict == MultimodalRetrievalAssuranceVerdict::Qualified {
        vec![format!(
            "assurance:local-multimodal-retrieval:{}",
            request.request_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let candidate_order = synthesis.candidate_order.clone();
    let qualified_order = synthesis.qualified_order.clone();
    let blocked_order = synthesis.blocked_order.clone();
    let unknown_order = synthesis.unknown_order.clone();
    let witness_order = witnesses.iter().cloned().collect::<Vec<_>>();
    let counterexample_order = counterexamples.iter().cloned().collect::<Vec<_>>();
    let verification_digest = ContentHash::of_value(&json!({"feature_id": FEATURE_ID, "request_id": request.request_id, "study_order": request.study_ids, "modality_order": request.required_modalities, "candidate_order": candidate_order, "qualified_order": qualified_order, "blocked_order": blocked_order, "unknown_order": unknown_order, "witness_order": witness_order, "counterexample_order": counterexample_order, "verdict": verdict, "replay_identity": request.replay_identity, "raw_data_local": raw_data_local})).map_err(|error| MultimodalRetrievalAssuranceError::Artifact(error.to_string()))?;
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "study_order": request.study_ids, "modality_order": request.required_modalities, "scope": request.scope, "verdict": verdict, "candidate_order": candidate_order, "qualified_order": qualified_order, "blocked_order": blocked_order, "unknown_order": unknown_order, "witness_order": witness_order, "counterexample_order": counterexample_order, "comparability_digest": synthesis.comparability_digest, "synthesis_digest": synthesis_digest, "verification_digest": verification_digest, "replay_identity": request.replay_identity, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "effect_receipts": effect_receipts, "raw_data_local": raw_data_local, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "brain-multimodal-retrieval-assurance:{}",
            request.request_id
        ),
        ASSURANCE_CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| MultimodalRetrievalAssuranceError::Artifact(error.to_string()))?;
    let receipt = MultimodalRetrievalAssuranceReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        study_order: request.study_ids.clone(),
        modality_order: request.required_modalities.clone(),
        scope: request.scope.clone(),
        verdict,
        candidate_order: synthesis.candidate_order,
        qualified_order: synthesis.qualified_order,
        blocked_order: synthesis.blocked_order,
        unknown_order: synthesis.unknown_order,
        witness_order: witnesses.into_iter().collect(),
        counterexample_order: counterexamples.into_iter().collect(),
        comparability_digest: synthesis.comparability_digest,
        synthesis_digest,
        verification_digest,
        replay_identity: request.replay_identity.clone(),
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

fn identity_keys(values: &[String]) -> BTreeSet<String> {
    values
        .iter()
        .map(|value| value.to_ascii_lowercase())
        .collect()
}

fn validate_text(value: &str, field: &str) -> Result<(), MultimodalRetrievalAssuranceError> {
    if value.trim().is_empty()
        || value != value.trim()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(MultimodalRetrievalAssuranceError::Invalid(format!(
            "{field} must be bounded, non-empty text without padding or control characters"
        )));
    }
    Ok(())
}

fn validate_unique(
    values: &[String],
    field: &str,
) -> Result<(), MultimodalRetrievalAssuranceError> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_text(value, field)?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(MultimodalRetrievalAssuranceError::Invalid(format!(
                "{field} contains duplicate or case-colliding values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_unique(
    values: &[String],
    field: &str,
) -> Result<(), MultimodalRetrievalAssuranceError> {
    validate_unique(values, field)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(MultimodalRetrievalAssuranceError::Invalid(format!(
            "{field} is not in canonical order"
        )));
    }
    Ok(())
}

fn receipt_payload(receipt: &MultimodalRetrievalAssuranceReceipt) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "study_order": receipt.study_order,
        "modality_order": receipt.modality_order,
        "scope": receipt.scope,
        "verdict": receipt.verdict,
        "candidate_order": receipt.candidate_order,
        "qualified_order": receipt.qualified_order,
        "blocked_order": receipt.blocked_order,
        "unknown_order": receipt.unknown_order,
        "witness_order": receipt.witness_order,
        "counterexample_order": receipt.counterexample_order,
        "comparability_digest": receipt.comparability_digest,
        "synthesis_digest": receipt.synthesis_digest,
        "verification_digest": receipt.verification_digest,
        "replay_identity": receipt.replay_identity,
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
    fn request(state: EvidenceState) -> MultimodalRetrievalQuery {
        let candidate = |id: &str, study: &str, modality: &str| RetrievalCandidate {
            evidence_id: format!("evidence:{id}"),
            source_id: format!("source:{id}"),
            study_id: study.into(),
            scope: "organoid:neural".into(),
            modality: modality.into(),
            support_milli: 900,
            state,
            semantic_digest: hash(id),
            artifact_digest: hash(&format!("a:{id}")),
            provenance_digest: hash(&format!("p:{id}")),
            replay_identity: hash("replay"),
            omissions: Vec::new(),
            negative_evidence: Vec::new(),
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        };
        MultimodalRetrievalQuery {
            request_id: "request:mm-assurance".into(),
            study_ids: vec!["study:a".into(), "study:b".into()],
            scope: "organoid:neural".into(),
            query: "synaptic morphology".into(),
            minimum_support_milli: 700,
            required_modalities: vec!["imaging".into(), "transcriptomics".into()],
            candidates: vec![
                candidate("a", "study:a", "imaging"),
                candidate("b", "study:b", "transcriptomics"),
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
        let manifest = multimodal_retrieval_assurance_harness_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A1);
    }
    #[test]
    fn qualified_has_closure_witnesses() {
        let receipt =
            verify_multimodal_retrieval_assurance(&request(EvidenceState::Supported)).unwrap();
        assert_eq!(
            receipt.verdict,
            MultimodalRetrievalAssuranceVerdict::Qualified
        );
        assert!(receipt
            .witness_order
            .iter()
            .any(|value| value == "gate:comparability"));
    }
    #[test]
    fn unknown_is_unresolved() {
        let receipt =
            verify_multimodal_retrieval_assurance(&request(EvidenceState::Unknown)).unwrap();
        assert_eq!(
            receipt.verdict,
            MultimodalRetrievalAssuranceVerdict::Unresolved
        );
    }
    #[test]
    fn digest_is_stable() {
        let receipt =
            verify_multimodal_retrieval_assurance(&request(EvidenceState::Supported)).unwrap();
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }

    #[test]
    fn locality_failure_is_blocked_and_retained() {
        let mut input = request(EvidenceState::Supported);
        input.raw_data_local = false;
        let receipt = verify_multimodal_retrieval_assurance(&input).unwrap();
        assert_eq!(
            receipt.verdict,
            MultimodalRetrievalAssuranceVerdict::Blocked
        );
        assert!(receipt.raw_data_local);
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item == "assurance:raw-data-locality-failed"));
        assert!(receipt.validate().is_ok());
    }

    #[test]
    fn assurance_artifact_payload_is_bound() {
        let mut receipt =
            verify_multimodal_retrieval_assurance(&request(EvidenceState::Supported)).unwrap();
        receipt.scope = "scope:tampered".into();
        assert!(receipt.validate().is_err());
    }
}
