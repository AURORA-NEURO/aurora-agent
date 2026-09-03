//! Federated interpretation and visualisation assurance for research results.
//!
//! Atlas feature: `AFA-runtime-P14-F28`.
//!
//! The assurance harness checks that an interpretation surface is backed by comparable,
//! replayable, policy-permitted evidence before it can be released.  It deliberately emits a
//! typed read-only artifact: it does not render a chart, infer biology, move data, or make a
//! clinical decision.

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    LossSeverity, ProvenanceLink, ResearchContractError, ResearchSurface, SemanticLoss,
    TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-runtime-P14-F28";
pub const CONTRACT_VERSION: &str = "runtime-federated-continual-interpretation-assurance/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceBackedResult4@1";
pub const OUTPUT_SCHEMA: &str = "InteractiveInterpretation7@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.runtime-interactive-interpretation-7+json";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterpretationEvidenceState {
    Proven,
    Supported,
    Unknown,
    Unmeasured,
    Contradicted,
    Negative,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InterpretationCandidate4 {
    pub candidate_id: String,
    pub study_order: u32,
    pub modality_order: u32,
    pub semantic_profile: String,
    pub interpretation_score_milli: i64,
    pub evidence_state: InterpretationEvidenceState,
    pub comparability_digest: ContentHash,
    pub result_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: String,
    #[serde(default)]
    pub local: bool,
    #[serde(default)]
    pub aggregate_only: bool,
    #[serde(default)]
    pub policy_allowed: bool,
    #[serde(default)]
    pub omission_order: Vec<String>,
    #[serde(default)]
    pub uncertainty_order: Vec<String>,
    #[serde(default)]
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceBackedResult4 {
    pub schema_version: String,
    pub request_id: String,
    pub researcher: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_study_order: u32,
    pub required_modality_order: u32,
    pub comparability_digest: ContentHash,
    pub replay_identity: String,
    pub candidates: Vec<InterpretationCandidate4>,
    #[serde(default)]
    pub policy_allowed: bool,
    #[serde(default)]
    pub protected_closure: bool,
    #[serde(default)]
    pub signed_approval: bool,
    #[serde(default)]
    pub federation_allowed: bool,
    #[serde(default = "default_true")]
    pub raw_data_local: bool,
    #[serde(default)]
    pub aggregate_only: bool,
    #[serde(default)]
    pub adversarial_event_order: Vec<String>,
    pub boundary: String,
}

fn default_true() -> bool { true }

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InterpretationArtifact7 {
    pub request_id: String,
    pub candidate_order: Vec<String>,
    pub qualified: Vec<String>,
    pub unresolved: Vec<String>,
    pub blocked: Vec<String>,
    pub incomparable: Vec<String>,
    pub missing_study_order: Vec<u32>,
    pub missing_modality_order: Vec<u32>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_results: Vec<String>,
    pub replay_identity: String,
    pub interpretation_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InteractiveInterpretation7 {
    pub schema_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub candidate_order: Vec<String>,
    pub qualified: Vec<String>,
    pub unresolved: Vec<String>,
    pub blocked: Vec<String>,
    pub incomparable: Vec<String>,
    pub missing_study_order: Vec<u32>,
    pub missing_modality_order: Vec<u32>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_results: Vec<String>,
    pub replay_identity: String,
    pub interpretation_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub locality: String,
    pub boundary: String,
}

impl InteractiveInterpretation7 {
    pub fn validate(&self) -> Result<(), InterpretationAssuranceError> {
        if self.schema_version != OUTPUT_SCHEMA
            || self.feature_id != FEATURE_ID
            || self.request_id.trim().is_empty()
            || self.replay_identity.trim().is_empty()
            || self.effect_receipts != vec!["block:unsafe-release".to_string()]
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.locality != "raw-data-local; aggregate-only federation" 
            || self.candidate_order.is_empty()
        {
            return Err(InterpretationAssuranceError::InvalidOutput(
                "interpretation identity, release effect, locality, or boundary is incomplete".into(),
            ));
        }
        self.artifact.validate_metadata().map_err(InterpretationAssuranceError::Contract)
    }
}

#[derive(Debug, Error)]
pub enum InterpretationAssuranceError {
    #[error("invalid interpretation request: {0}")]
    InvalidRequest(String),
    #[error("invalid interpretation output: {0}")]
    InvalidOutput(String),
    #[error("research contract error: {0}")]
    Contract(#[from] ResearchContractError),
    #[error("serialization error: {0}")]
    Serialization(String),
}

pub fn interpretation_assurance_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "runtime".into(),
        consumers: ["laboratory automation engineer".into(), "research workbench".into()].into(),
        behavior: "assures federated interpretation surfaces using typed evidence, replay, comparability, omission, and policy gates".into(),
        value: "prevents unsupported interpretation release while preserving useful aggregate-only research views".into(),
        inputs: vec![TypedPort { name: "evidence_backed_result".into(), schema: INPUT_SCHEMA.into(), required: true }],
        outputs: vec![TypedPort { name: "interactive_interpretation".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::WriteLocalArtifact, Effect::ExecuteLocalComputation].into(),
        permissions: ["evaluate:capability-runs".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "fixture:runtime-interpretation-assurance".into(), state: EvidenceState::Supported, locator: Some("fixtures/runtime".into()) }],
        authority_requirements: Vec::new(),
        autonomy_tier: AutonomyTier::A1,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Protocol].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

fn validate_request(request: &EvidenceBackedResult4) -> Result<(), InterpretationAssuranceError> {
    if request.schema_version != INPUT_SCHEMA
        || request.request_id.trim().is_empty()
        || request.researcher.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.replay_identity.trim().is_empty()
        || request.candidates.is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(InterpretationAssuranceError::InvalidRequest(
            "typed evidence result is incomplete or outside the preclinical boundary".into(),
        ));
    }
    for candidate in &request.candidates {
        if candidate.candidate_id.trim().is_empty()
            || candidate.semantic_profile.trim().is_empty()
            || candidate.replay_identity.trim().is_empty()
            || candidate.comparability_digest == ContentHash::of_bytes(b"")
            || candidate.result_digest == ContentHash::of_bytes(b"")
            || candidate.provenance_digest == ContentHash::of_bytes(b"")
        {
            return Err(InterpretationAssuranceError::InvalidRequest(
                "candidate identity, typed profile, replay, provenance, or digest is incomplete".into(),
            ));
        }
    }
    Ok(())
}

fn digest<T: Serialize>(value: &T) -> Result<ContentHash, InterpretationAssuranceError> {
    let payload = serde_json::to_value(value)
        .map_err(|error| InterpretationAssuranceError::Serialization(error.to_string()))?;
    ContentHash::of_value(&payload)
        .map_err(|error| InterpretationAssuranceError::Serialization(error.to_string()))
}

pub fn assure_interpretation(
    request: &EvidenceBackedResult4,
) -> Result<InteractiveInterpretation7, InterpretationAssuranceError> {
    validate_request(request)?;
    let global_block = !request.policy_allowed
        || !request.protected_closure
        || !request.signed_approval
        || !request.federation_allowed
        || !request.raw_data_local
        || !request.aggregate_only
        || !request.adversarial_event_order.is_empty();
    let mut candidates = request.candidates.clone();
    candidates.sort_by(|left, right| {
        right.interpretation_score_milli.cmp(&left.interpretation_score_milli)
            .then_with(|| left.study_order.cmp(&right.study_order))
            .then_with(|| left.modality_order.cmp(&right.modality_order))
            .then_with(|| left.candidate_id.cmp(&right.candidate_id))
    });
    let candidate_order = candidates.iter().map(|candidate| candidate.candidate_id.clone()).collect::<Vec<_>>();
    let mut qualified = Vec::new();
    let mut unresolved = Vec::new();
    let mut blocked = Vec::new();
    let mut incomparable = Vec::new();
    let mut missing_study = BTreeSet::new();
    let mut missing_modality = BTreeSet::new();
    let mut omissions = request.adversarial_event_order.clone();
    let mut uncertainty = Vec::new();
    let mut negative_results = Vec::new();
    for candidate in &candidates {
        omissions.extend(candidate.omission_order.clone());
        uncertainty.extend(candidate.uncertainty_order.clone());
        if candidate.negative_result || matches!(candidate.evidence_state, InterpretationEvidenceState::Negative) {
            negative_results.push(candidate.candidate_id.clone());
        }
        if candidate.study_order != request.required_study_order {
            missing_study.insert(candidate.study_order);
        }
        if candidate.modality_order != request.required_modality_order {
            missing_modality.insert(candidate.modality_order);
        }
        let comparable = candidate.semantic_profile == request.semantic_profile
            && candidate.comparability_digest == request.comparability_digest;
        if global_block || !candidate.policy_allowed || !candidate.local || !candidate.aggregate_only {
            blocked.push(candidate.candidate_id.clone());
        } else if !comparable {
            incomparable.push(candidate.candidate_id.clone());
        } else {
            match candidate.evidence_state {
                InterpretationEvidenceState::Proven | InterpretationEvidenceState::Supported => qualified.push(candidate.candidate_id.clone()),
                InterpretationEvidenceState::Contradicted => unresolved.push(candidate.candidate_id.clone()),
                InterpretationEvidenceState::Unknown | InterpretationEvidenceState::Unmeasured | InterpretationEvidenceState::Negative => unresolved.push(candidate.candidate_id.clone()),
            }
        }
    }
    omissions.sort();
    omissions.dedup();
    uncertainty.sort();
    uncertainty.dedup();
    let mut negative_results = negative_results;
    negative_results.sort();
    let interpretation = InterpretationArtifact7 {
        request_id: request.request_id.clone(), candidate_order: candidate_order.clone(),
        qualified: qualified.clone(), unresolved: unresolved.clone(), blocked: blocked.clone(), incomparable: incomparable.clone(),
        missing_study_order: missing_study.iter().copied().collect(), missing_modality_order: missing_modality.iter().copied().collect(),
        omissions: omissions.clone(), uncertainty: uncertainty.clone(), negative_results: negative_results.clone(),
        replay_identity: request.replay_identity.clone(), interpretation_digest: ContentHash::of_bytes(b"") ,
    };
    let interpretation_digest = digest(&json!({
        "request_id": request.request_id, "candidate_order": candidate_order, "qualified": qualified,
        "unresolved": unresolved, "blocked": blocked, "incomparable": incomparable,
        "missing_study_order": interpretation.missing_study_order, "missing_modality_order": interpretation.missing_modality_order,
        "omissions": omissions, "uncertainty": uncertainty, "negative_results": negative_results,
        "replay_identity": request.replay_identity,
    }))?;
    let artifact_payload = json!({ "interpretation": interpretation, "interpretation_digest": interpretation_digest });
    let artifact_digest = ContentHash::of_value(&artifact_payload)
        .map_err(|error| InterpretationAssuranceError::Serialization(error.to_string()))?;
    let artifact = TypedResearchArtifact {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        artifact_id: format!("runtime-interpretation:{}", request.request_id),
        content_type: CONTENT_TYPE.into(),
        content_hash: artifact_digest,
        semantic_loss: vec![SemanticLoss { field: "raw_data_and_unresolved_evidence".into(), reason: "raw data remains local and unresolved or blocked evidence is never presented as a release conclusion".into(), severity: LossSeverity::Bounded }],
        provenance: vec![ProvenanceLink { source_id: "evidence-backed-result".into(), relation: "interpretation-input".into(), digest: request.comparability_digest.clone() }],
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let output = InteractiveInterpretation7 {
        schema_version: OUTPUT_SCHEMA.into(), feature_id: FEATURE_ID.into(), request_id: request.request_id.clone(),
        candidate_order, qualified, unresolved, blocked, incomparable,
        missing_study_order: interpretation.missing_study_order, missing_modality_order: interpretation.missing_modality_order,
        omissions, uncertainty, negative_results, replay_identity: request.replay_identity.clone(), interpretation_digest,
        artifact, effect_receipts: vec!["block:unsafe-release".into()], locality: "raw-data-local; aggregate-only federation".into(), boundary: PRECLINICAL_BOUNDARY.into(),
    };
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(text: &str) -> ContentHash { ContentHash::of_bytes(text.as_bytes()) }

    fn fixture() -> EvidenceBackedResult4 {
        EvidenceBackedResult4 {
            schema_version: INPUT_SCHEMA.into(), request_id: "req-interpret-1".into(), researcher: "researcher".into(), purpose: "compare preclinical modalities".into(), semantic_profile: "mouse-cortex-v1".into(), required_study_order: 1, required_modality_order: 1, comparability_digest: hash("cmp"), replay_identity: "replay-1".into(),
            candidates: vec![InterpretationCandidate4 { candidate_id: "cand-1".into(), study_order: 1, modality_order: 1, semantic_profile: "mouse-cortex-v1".into(), interpretation_score_milli: 900, evidence_state: InterpretationEvidenceState::Supported, comparability_digest: hash("cmp"), result_digest: hash("result"), provenance_digest: hash("prov"), replay_identity: "replay-cand".into(), local: true, aggregate_only: true, policy_allowed: true, omission_order: vec![], uncertainty_order: vec!["sample-size".into()], negative_result: false }],
            policy_allowed: true, protected_closure: true, signed_approval: true, federation_allowed: true, raw_data_local: true, aggregate_only: true, adversarial_event_order: vec![], boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn release_gate_is_always_explicit() {
        let output = assure_interpretation(&fixture()).unwrap();
        assert_eq!(output.effect_receipts, vec!["block:unsafe-release"]);
        assert_eq!(output.qualified, vec!["cand-1"]);
    }

    #[test]
    fn missing_modality_is_retained() {
        let mut request = fixture();
        request.candidates[0].modality_order = 2;
        let output = assure_interpretation(&request).unwrap();
        assert_eq!(output.missing_modality_order, vec![2]);
    }

    #[test]
    fn adversarial_event_blocks_all_candidates() {
        let mut request = fixture();
        request.adversarial_event_order = vec!["poisoned-artifact".into()];
        let output = assure_interpretation(&request).unwrap();
        assert_eq!(output.blocked, vec!["cand-1"]);
        assert!(output.qualified.is_empty());
    }
}
