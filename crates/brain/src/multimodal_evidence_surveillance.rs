//! Multimodal, multi-study evidence-surveillance assurance.
//!
//! Atlas feature: `AFA-brain-P01-F02`. This capability qualifies only caller-supplied,
//! institution-local observations and refuses to call a partial modality/study set complete.

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

pub const FEATURE_ID: &str = "AFA-brain-P01-F02";
pub const CONTRACT_VERSION: &str = "brain-evidence-surveillance-multimodal/1.0";
pub const MAX_OBSERVATIONS: usize = 4096;
const SURVEILLANCE_CONTENT_TYPE: &str =
    "application/vnd.aurora.qualified-multimodal-evidence-set+json";
const MAX_ITEMS: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalEvidenceFeedRequest {
    pub request_id: String,
    pub study_ids: Vec<String>,
    pub scope: String,
    pub query: String,
    pub minimum_relevance_milli: u16,
    pub required_modalities: Vec<String>,
    pub observations: Vec<EvidenceObservation>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultimodalEvidenceDisposition {
    Qualified,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualifiedMultimodalEvidenceSet {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub study_order: Vec<String>,
    pub scope: String,
    pub disposition: MultimodalEvidenceDisposition,
    pub candidate_order: Vec<String>,
    pub qualified_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub source_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub relevance_order: Vec<u16>,
    pub semantic_order: Vec<ContentHash>,
    pub artifact_order: Vec<ContentHash>,
    pub provenance_order: Vec<ContentHash>,
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
pub enum MultimodalEvidenceError {
    #[error("invalid multimodal evidence request: {0}")]
    Invalid(String),
    #[error("multimodal evidence artifact failed: {0}")]
    Artifact(String),
}

impl QualifiedMultimodalEvidenceSet {
    pub fn validate(&self) -> Result<(), MultimodalEvidenceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.study_order.len() < 2
            || self.candidate_order.is_empty()
            || self.relevance_order.len() != self.candidate_order.len()
            || self.effect_receipts.is_empty()
        {
            return Err(MultimodalEvidenceError::Invalid(
                "identity, study coverage, ranking, locality, relevance, or effects are incomplete"
                    .into(),
            ));
        }
        let collections = [
            &self.study_order,
            &self.candidate_order,
            &self.qualified_order,
            &self.blocked_order,
            &self.unknown_order,
            &self.source_order,
            &self.modality_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ];
        if collections.iter().any(|values| values.len() > MAX_ITEMS) {
            return Err(MultimodalEvidenceError::Invalid(
                "multimodal evidence collection exceeds the bounded contract limit".into(),
            ));
        }
        if self.relevance_order.iter().any(|value| *value > 1000) {
            return Err(MultimodalEvidenceError::Invalid(
                "multimodal evidence relevance is outside the bounded range".into(),
            ));
        }
        let candidates = self.candidate_order.iter().collect::<BTreeSet<_>>();
        let qualified = self.qualified_order.iter().collect::<BTreeSet<_>>();
        let blocked = self.blocked_order.iter().collect::<BTreeSet<_>>();
        let unknown = self.unknown_order.iter().collect::<BTreeSet<_>>();
        let mut covered = qualified.clone();
        covered.extend(blocked.iter());
        if covered != candidates || !qualified.is_disjoint(&blocked) || !unknown.is_subset(&blocked)
        {
            return Err(MultimodalEvidenceError::Invalid(
                "multimodal evidence states must partition candidates without overlap".into(),
            ));
        }
        for values in collections {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(MultimodalEvidenceError::Invalid(
                    "ordering is not canonical".into(),
                ));
            }
        }
        let gate_blocked = self.negative_evidence.iter().any(|item| {
            item == "request:policy-denied" || item == "request:raw-data-locality-failed"
        }) || self
            .uncertainty
            .iter()
            .any(|item| item == "request:protected-closure-incomplete");
        let expected_disposition = if gate_blocked {
            MultimodalEvidenceDisposition::Blocked
        } else if self.qualified_order.is_empty() {
            MultimodalEvidenceDisposition::Unknown
        } else if self.blocked_order.is_empty()
            && self.omissions.is_empty()
            && self.uncertainty.is_empty()
            && self.negative_evidence.is_empty()
        {
            MultimodalEvidenceDisposition::Qualified
        } else {
            MultimodalEvidenceDisposition::Partial
        };
        if self.disposition != expected_disposition {
            return Err(MultimodalEvidenceError::Invalid(
                "multimodal evidence disposition does not match state or gates".into(),
            ));
        }
        let expected_effect = if self.qualified_order.is_empty() {
            vec!["block:unsafe-release".into()]
        } else {
            vec![format!("read:local-research-artifacts:{}", self.request_id)]
        };
        if self.effect_receipts != expected_effect {
            return Err(MultimodalEvidenceError::Invalid(
                "multimodal evidence effect does not match qualified state".into(),
            ));
        }
        for digest in [&self.replay_identity, &self.artifact.content_hash] {
            if digest.as_str().len() != 64 {
                return Err(MultimodalEvidenceError::Invalid(
                    "multimodal evidence digest is invalid".into(),
                ));
            }
        }
        let expected_artifact_id = format!("brain-multimodal-evidence:{}", self.request_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != SURVEILLANCE_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(MultimodalEvidenceError::Invalid(
                "multimodal evidence artifact identity or provenance is inconsistent".into(),
            ));
        }
        for values in [
            &self.semantic_order,
            &self.artifact_order,
            &self.provenance_order,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(MultimodalEvidenceError::Invalid(
                    "digest ordering is not canonical".into(),
                ));
            }
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| MultimodalEvidenceError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| MultimodalEvidenceError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, MultimodalEvidenceError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| MultimodalEvidenceError::Artifact(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| MultimodalEvidenceError::Artifact(error.to_string()))
    }
}

pub fn multimodal_evidence_surveillance_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "brain".into(),
        consumers: ["platform reliability engineer".into(), "multimodal study steward".into()].into(),
        behavior: "qualifies evidence across required studies and modalities without converting incomplete coverage into confidence".into(),
        value: "provides deterministic, omission-aware multimodal evidence sets for preclinical research".into(),
        inputs: vec![TypedPort { name: "evidence_feed".into(), schema: "EvidenceFeed2@1".into(), required: true }],
        outputs: vec![TypedPort { name: "qualified_evidence_set".into(), schema: "QualifiedEvidenceSet1@1".into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(),
        permissions: ["read:local-research-artifacts".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "json-schema".into(), state: EvidenceState::Supported, locator: Some("https://json-schema.org/specification".into()) }],
        authority_requirements: Vec::new(),
        autonomy_tier: AutonomyTier::A1,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn surveil_multimodal_evidence(
    request: &MultimodalEvidenceFeedRequest,
) -> Result<QualifiedMultimodalEvidenceSet, MultimodalEvidenceError> {
    validate_request(request)?;
    let mut observations = request.observations.clone();
    observations.sort_by(|left, right| {
        right
            .relevance_milli
            .cmp(&left.relevance_milli)
            .then(left.evidence_id.cmp(&right.evidence_id))
    });
    let candidate_order = observations
        .iter()
        .map(|item| item.evidence_id.clone())
        .collect::<Vec<_>>();
    let relevance_order = observations
        .iter()
        .map(|item| item.relevance_milli)
        .collect::<Vec<_>>();
    let required_studies = request.study_ids.iter().cloned().collect::<BTreeSet<_>>();
    let required_modalities = request
        .required_modalities
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut qualified = Vec::new();
    let mut blocked = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut sources = BTreeSet::new();
    let mut modalities = BTreeSet::new();
    let mut semantics = BTreeSet::new();
    let mut artifacts = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for observation in &observations {
        let admissible = request.policy_allow
            && request.protected_closure
            && request.raw_data_local
            && observation.raw_data_local
            && observation.state == EvidenceState::Supported
            && required_studies.contains(&observation.study_id)
            && observation.scope == request.scope
            && required_modalities.contains(&observation.modality)
            && observation.relevance_milli >= request.minimum_relevance_milli
            && observation.replay_identity == request.replay_identity
            && observation.omissions.is_empty()
            && observation.negative_evidence.is_empty();
        if admissible {
            qualified.push(observation.evidence_id.clone());
            sources.insert(observation.source_id.clone());
            modalities.insert(observation.modality.clone());
            semantics.insert(observation.semantic_digest.clone());
            artifacts.insert(observation.artifact_digest.clone());
            provenance.insert(observation.provenance_digest.clone());
        } else {
            blocked.insert(observation.evidence_id.clone());
            if matches!(
                observation.state,
                EvidenceState::Unknown | EvidenceState::Speculative
            ) {
                unknown.insert(observation.evidence_id.clone());
                uncertainty.insert(
                    format!(
                        "evidence:{}:state-{:?}-not-qualified",
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
            if !required_studies.contains(&observation.study_id) {
                omissions.insert(format!(
                    "evidence:{}:study-not-required",
                    observation.evidence_id
                ));
            }
            if !required_modalities.contains(&observation.modality) {
                omissions.insert(format!(
                    "evidence:{}:modality-not-required",
                    observation.evidence_id
                ));
            }
            if observation.scope != request.scope {
                omissions.insert(format!(
                    "evidence:{}:scope-mismatch",
                    observation.evidence_id
                ));
            }
            if observation.relevance_milli < request.minimum_relevance_milli {
                uncertainty.insert(format!(
                    "evidence:{}:relevance-below-threshold",
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
    let covered_studies = observations
        .iter()
        .filter(|item| qualified.contains(&item.evidence_id))
        .map(|item| item.study_id.clone())
        .collect::<BTreeSet<_>>();
    let missing_studies = required_studies
        .difference(&covered_studies)
        .cloned()
        .collect::<Vec<_>>();
    let missing_modalities = required_modalities
        .difference(&modalities)
        .cloned()
        .collect::<Vec<_>>();
    for study in &missing_studies {
        omissions.insert(format!("study:{}:required-coverage-missing", study));
    }
    for modality in &missing_modalities {
        omissions.insert(format!("modality:{}:required-coverage-missing", modality));
    }
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.raw_data_local {
        negative.insert("request:raw-data-locality-failed".into());
    }
    let disposition =
        if !request.policy_allow || !request.protected_closure || !request.raw_data_local {
            MultimodalEvidenceDisposition::Blocked
        } else if qualified.is_empty() {
            MultimodalEvidenceDisposition::Unknown
        } else if missing_studies.is_empty()
            && missing_modalities.is_empty()
            && blocked.is_empty()
            && omissions.is_empty()
            && uncertainty.is_empty()
            && negative.is_empty()
        {
            MultimodalEvidenceDisposition::Qualified
        } else {
            MultimodalEvidenceDisposition::Partial
        };
    let has_qualified = !qualified.is_empty();
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "study_order": request.study_ids, "scope": request.scope, "disposition": disposition, "candidate_order": candidate_order, "qualified_order": qualified, "blocked_order": blocked, "unknown_order": unknown, "source_order": sources, "modality_order": modalities, "relevance_order": relevance_order, "semantic_order": semantics, "artifact_order": artifacts, "provenance_order": provenance, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "replay_identity": request.replay_identity, "effect_receipts": if has_qualified { vec![format!("read:local-research-artifacts:{}", request.request_id)] } else { vec!["block:unsafe-release".to_string()] }, "raw_data_local": true, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-multimodal-evidence:{}", request.request_id),
        SURVEILLANCE_CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| MultimodalEvidenceError::Artifact(error.to_string()))?;
    let receipt = QualifiedMultimodalEvidenceSet {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        study_order: request.study_ids.clone(),
        scope: request.scope.clone(),
        disposition,
        candidate_order,
        qualified_order: qualified,
        blocked_order: blocked.into_iter().collect(),
        unknown_order: unknown.into_iter().collect(),
        source_order: sources.into_iter().collect(),
        modality_order: modalities.into_iter().collect(),
        relevance_order,
        semantic_order: semantics.into_iter().collect(),
        artifact_order: artifacts.into_iter().collect(),
        provenance_order: provenance.into_iter().collect(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        replay_identity: request.replay_identity.clone(),
        effect_receipts: if has_qualified {
            vec![format!(
                "read:local-research-artifacts:{}",
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

fn receipt_payload(receipt: &QualifiedMultimodalEvidenceSet) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "study_order": receipt.study_order,
        "scope": receipt.scope,
        "disposition": receipt.disposition,
        "candidate_order": receipt.candidate_order,
        "qualified_order": receipt.qualified_order,
        "blocked_order": receipt.blocked_order,
        "unknown_order": receipt.unknown_order,
        "source_order": receipt.source_order,
        "modality_order": receipt.modality_order,
        "relevance_order": receipt.relevance_order,
        "semantic_order": receipt.semantic_order,
        "artifact_order": receipt.artifact_order,
        "provenance_order": receipt.provenance_order,
        "omissions": receipt.omissions,
        "uncertainty": receipt.uncertainty,
        "negative_evidence": receipt.negative_evidence,
        "replay_identity": receipt.replay_identity,
        "effect_receipts": receipt.effect_receipts,
        "raw_data_local": receipt.raw_data_local,
        "boundary": receipt.boundary,
    })
}

fn validate_request(
    request: &MultimodalEvidenceFeedRequest,
) -> Result<(), MultimodalEvidenceError> {
    if request.request_id.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.query.trim().is_empty()
        || request.study_ids.len() < 2
        || request.required_modalities.len() < 2
        || request.minimum_relevance_milli > 1000
        || request.observations.is_empty()
        || request.observations.len() > MAX_OBSERVATIONS
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(MultimodalEvidenceError::Invalid("multimodal request identity, study/modality floors, query, observations, threshold, or boundary is incomplete".into()));
    }
    if request.study_ids.windows(2).any(|pair| pair[0] >= pair[1])
        || request
            .required_modalities
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(MultimodalEvidenceError::Invalid(
            "study and modality requirements must be unique and canonical".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for observation in &request.observations {
        if observation.evidence_id.trim().is_empty()
            || observation.source_id.trim().is_empty()
            || observation.study_id.trim().is_empty()
            || observation.modality.trim().is_empty()
            || observation.scope.trim().is_empty()
            || observation.relevance_milli > 1000
            || observation.boundary != PRECLINICAL_BOUNDARY
            || !ids.insert(observation.evidence_id.clone())
        {
            return Err(MultimodalEvidenceError::Invalid(format!(
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
    use crate::evidence_surveillance::EvidenceObservation;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn observation(
        id: &str,
        study: &str,
        modality: &str,
        state: EvidenceState,
    ) -> EvidenceObservation {
        EvidenceObservation {
            evidence_id: format!("evidence:{id}"),
            source_id: format!("source:{id}"),
            study_id: study.into(),
            modality: modality.into(),
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
    fn request(observations: Vec<EvidenceObservation>) -> MultimodalEvidenceFeedRequest {
        MultimodalEvidenceFeedRequest {
            request_id: "request:multimodal".into(),
            study_ids: vec!["study:a".into(), "study:b".into()],
            scope: "organoid:neural".into(),
            query: "synaptic density".into(),
            minimum_relevance_milli: 700,
            required_modalities: vec!["imaging".into(), "transcriptomics".into()],
            observations,
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1_and_typed() {
        let manifest = multimodal_evidence_surveillance_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A1);
    }
    #[test]
    fn complete_study_and_modality_floor_qualifies() {
        let receipt = surveil_multimodal_evidence(&request(vec![
            observation("a", "study:a", "imaging", EvidenceState::Supported),
            observation("b", "study:b", "transcriptomics", EvidenceState::Supported),
        ]))
        .unwrap();
        assert_eq!(
            receipt.disposition,
            MultimodalEvidenceDisposition::Qualified
        );
        assert_eq!(receipt.candidate_order, vec!["evidence:a", "evidence:b"]);
    }
    #[test]
    fn missing_modality_is_explicit_partial() {
        let receipt = surveil_multimodal_evidence(&request(vec![
            observation("a", "study:a", "imaging", EvidenceState::Supported),
            observation("b", "study:b", "imaging", EvidenceState::Supported),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, MultimodalEvidenceDisposition::Partial);
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item.contains("transcriptomics")));
    }
    #[test]
    fn unknown_and_contradicted_are_retained() {
        let receipt = surveil_multimodal_evidence(&request(vec![
            observation("a", "study:a", "imaging", EvidenceState::Unknown),
            observation(
                "b",
                "study:b",
                "transcriptomics",
                EvidenceState::Contradicted,
            ),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, MultimodalEvidenceDisposition::Unknown);
        assert!(!receipt.unknown_order.is_empty());
        assert!(!receipt.negative_evidence.is_empty());
    }
    #[test]
    fn policy_denial_blocks_release() {
        let mut input = request(vec![
            observation("a", "study:a", "imaging", EvidenceState::Supported),
            observation("b", "study:b", "transcriptomics", EvidenceState::Supported),
        ]);
        input.policy_allow = false;
        let receipt = surveil_multimodal_evidence(&input).unwrap();
        assert_eq!(receipt.disposition, MultimodalEvidenceDisposition::Blocked);
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn duplicate_observation_is_rejected() {
        let mut duplicate =
            observation("a", "study:b", "transcriptomics", EvidenceState::Supported);
        duplicate.source_id = "source:other".into();
        assert!(surveil_multimodal_evidence(&request(vec![
            observation("a", "study:a", "imaging", EvidenceState::Supported),
            duplicate
        ]))
        .is_err());
    }
}
