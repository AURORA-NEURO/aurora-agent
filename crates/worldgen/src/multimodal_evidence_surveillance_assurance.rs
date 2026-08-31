//! Local single-study evidence-surveillance assurance harness.
//!
//! Atlas feature: `AFA-worldgen-P01-F26`.
//!
//! The engine only qualifies caller-supplied, institution-local observations. It never fetches
//! sources, turns absence into confidence, or treats a correct answer from an incomplete basis as
//! a pass.

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

pub const FEATURE_ID: &str = "AFA-worldgen-P01-F26";
pub const CONTRACT_VERSION: &str = "worldgen-multimodal-evidence-surveillance-assurance/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceFeed2@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet7@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.worldgen-qualified-evidence-set-7+json";
pub const MAX_OBSERVATIONS: usize = 4096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InfluenceEvidenceFeedRequest {
    pub request_id: String,
    pub study_id: String,
    pub scope: String,
    pub query: String,
    pub minimum_relevance_milli: u16,
    pub observations: Vec<InfluenceEvidenceObservation>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InfluenceEvidenceObservation {
    pub evidence_id: String,
    pub source_id: String,
    pub study_id: String,
    pub modality: String,
    pub scope: String,
    pub relevance_milli: u16,
    pub state: EvidenceState,
    pub semantic_digest: ContentHash,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub omissions: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InfluenceEvidenceSurveillanceDisposition {
    Qualified,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InfluenceQualifiedEvidenceSet {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub study_id: String,
    pub scope: String,
    pub disposition: InfluenceEvidenceSurveillanceDisposition,
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
pub enum InfluenceEvidenceSurveillanceError {
    #[error("invalid evidence surveillance request: {0}")]
    Invalid(String),
    #[error("evidence surveillance artifact failed: {0}")]
    Artifact(String),
    #[error("evidence surveillance serialization failed: {0}")]
    Serialization(String),
}

impl InfluenceQualifiedEvidenceSet {
    pub fn validate(&self) -> Result<(), InfluenceEvidenceSurveillanceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.study_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.relevance_order.len() != self.candidate_order.len()
            || self.effect_receipts.is_empty()
        {
            return Err(InfluenceEvidenceSurveillanceError::Invalid(
                "evidence identity, ranking, locality, relevance, or effects are incomplete".into(),
            ));
        }
        if self
            .qualified_order
            .iter()
            .chain(self.blocked_order.iter())
            .chain(self.unknown_order.iter())
            .any(|id| !self.candidate_order.contains(id))
        {
            return Err(InfluenceEvidenceSurveillanceError::Invalid(
                "evidence state is not covered by candidate order".into(),
            ));
        }
        for values in [
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
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(InfluenceEvidenceSurveillanceError::Invalid(
                    "evidence ordering is not canonical".into(),
                ));
            }
        }
        for values in [
            &self.semantic_order,
            &self.artifact_order,
            &self.provenance_order,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(InfluenceEvidenceSurveillanceError::Invalid(
                    "evidence digest ordering is not canonical".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("read:local-research-artifacts:")
                && effect != "block:unsafe-release"
        }) {
            return Err(InfluenceEvidenceSurveillanceError::Invalid(
                "effect is outside the evidence-surveillance gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| InfluenceEvidenceSurveillanceError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, InfluenceEvidenceSurveillanceError> {
        self.validate()?;
        let value = serde_json::to_value(self).map_err(|error| {
            InfluenceEvidenceSurveillanceError::Serialization(error.to_string())
        })?;
        ContentHash::of_value(&value)
            .map_err(|error| InfluenceEvidenceSurveillanceError::Serialization(error.to_string()))
    }
}

pub fn worldgen_multimodal_evidence_surveillance_assurance_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "worldgen".into(),
        consumers: ["agent developer".into(), "research data steward".into()].into(),
        behavior: "qualifies a local single-study evidence feed deterministically without fetching sources or converting unknown evidence into confidence".into(),
        value: "provides replayable, omission-aware evidence alerts for the autonomous research kernel".into(),
        inputs: vec![TypedPort { name: "evidence_feed".into(), schema: INPUT_SCHEMA.into(), required: true }],
        outputs: vec![TypedPort { name: "qualified_evidence_set".into(), schema: OUTPUT_SCHEMA.into(), required: true }],
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

pub fn assure_worldgen_multimodal_evidence_surveillance(
    request: &InfluenceEvidenceFeedRequest,
) -> Result<InfluenceQualifiedEvidenceSet, InfluenceEvidenceSurveillanceError> {
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
            && observation.study_id == request.study_id
            && observation.scope == request.scope
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
            if !request.policy_allow {
                negative.insert("request:policy-denied".into());
            }
            if !request.protected_closure {
                uncertainty.insert("request:protected-closure-incomplete".into());
            }
            if !request.raw_data_local || !observation.raw_data_local {
                negative.insert(format!(
                    "evidence:{}:raw-data-locality-failed",
                    observation.evidence_id
                ));
            }
            if observation.study_id != request.study_id {
                omissions.insert(format!(
                    "evidence:{}:study-mismatch",
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
    let disposition =
        if !request.policy_allow || !request.protected_closure || !request.raw_data_local {
            InfluenceEvidenceSurveillanceDisposition::Blocked
        } else if qualified.is_empty() {
            InfluenceEvidenceSurveillanceDisposition::Unknown
        } else if blocked.is_empty()
            && omissions.is_empty()
            && uncertainty.is_empty()
            && negative.is_empty()
        {
            InfluenceEvidenceSurveillanceDisposition::Qualified
        } else {
            InfluenceEvidenceSurveillanceDisposition::Partial
        };
    let payload = json!({"schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION, "contract_version": CONTRACT_VERSION, "feature_id": FEATURE_ID, "request_id": request.request_id, "study_id": request.study_id, "scope": request.scope, "disposition": disposition, "candidate_order": candidate_order, "qualified_order": qualified, "blocked_order": blocked, "unknown_order": unknown, "source_order": sources, "modality_order": modalities, "omissions": omissions, "uncertainty": uncertainty, "negative_evidence": negative, "replay_identity": request.replay_identity, "boundary": PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!(
            "influence-local-evidence-surveillance-assurance:{}",
            request.request_id
        ),
        CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| InfluenceEvidenceSurveillanceError::Artifact(error.to_string()))?;
    let has_qualified = !qualified.is_empty();
    let receipt = InfluenceQualifiedEvidenceSet {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        study_id: request.study_id.clone(),
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

fn validate_request(
    request: &InfluenceEvidenceFeedRequest,
) -> Result<(), InfluenceEvidenceSurveillanceError> {
    if request.request_id.trim().is_empty()
        || request.study_id.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.query.trim().is_empty()
        || request.minimum_relevance_milli > 1000
        || request.observations.is_empty()
        || request.observations.len() > MAX_OBSERVATIONS
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(InfluenceEvidenceSurveillanceError::Invalid(
            "evidence request identity, query, observations, threshold, or boundary is incomplete"
                .into(),
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
            return Err(InfluenceEvidenceSurveillanceError::Invalid(format!(
                "observation {} is invalid or duplicated",
                observation.evidence_id
            )));
        }
    }
    Ok(())
}

use assure_worldgen_multimodal_evidence_surveillance as assure_local_evidence_surveillance;
use worldgen_multimodal_evidence_surveillance_assurance_manifest as influence_local_evidence_surveillance_manifest;

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn observation(id: &str, state: EvidenceState) -> InfluenceEvidenceObservation {
        InfluenceEvidenceObservation {
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
    fn request(observations: Vec<InfluenceEvidenceObservation>) -> InfluenceEvidenceFeedRequest {
        InfluenceEvidenceFeedRequest {
            request_id: "request:evidence".into(),
            study_id: "study:organoid".into(),
            scope: "organoid:neural".into(),
            query: "synaptic density".into(),
            minimum_relevance_milli: 700,
            observations,
            replay_identity: hash("replay"),
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a0_and_typed() {
        let manifest = influence_local_evidence_surveillance_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A1);
    }
    #[test]
    fn supported_observations_are_qualified_and_ranked() {
        let receipt = assure_local_evidence_surveillance(&request(vec![
            observation("b", EvidenceState::Supported),
            observation("a", EvidenceState::Supported),
        ]))
        .unwrap();
        assert_eq!(
            receipt.disposition,
            InfluenceEvidenceSurveillanceDisposition::Qualified
        );
        assert_eq!(receipt.candidate_order, vec!["evidence:a", "evidence:b"]);
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
    #[test]
    fn unknown_and_contradicted_observations_remain_visible() {
        let receipt = assure_local_evidence_surveillance(&request(vec![
            observation("a", EvidenceState::Supported),
            observation("b", EvidenceState::Unknown),
            observation("c", EvidenceState::Contradicted),
        ]))
        .unwrap();
        assert_eq!(
            receipt.disposition,
            InfluenceEvidenceSurveillanceDisposition::Partial
        );
        assert!(receipt.unknown_order.contains(&"evidence:b".into()));
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|value| value.contains("evidence:c")));
    }
    #[test]
    fn policy_denial_blocks_evidence_release() {
        let mut input = request(vec![observation("a", EvidenceState::Supported)]);
        input.policy_allow = false;
        let receipt = assure_local_evidence_surveillance(&input).unwrap();
        assert_eq!(
            receipt.disposition,
            InfluenceEvidenceSurveillanceDisposition::Blocked
        );
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn replay_mismatch_is_partial_and_explicit() {
        let mut input = request(vec![observation("a", EvidenceState::Supported)]);
        input.observations[0].replay_identity = hash("different");
        let receipt = assure_local_evidence_surveillance(&input).unwrap();
        assert_eq!(
            receipt.disposition,
            InfluenceEvidenceSurveillanceDisposition::Unknown
        );
        assert!(receipt
            .uncertainty
            .iter()
            .any(|value| value.contains("replay-mismatch")));
    }
    #[test]
    fn duplicate_observation_is_rejected() {
        let mut duplicate = observation("a", EvidenceState::Supported);
        duplicate.source_id = "source:other".into();
        assert!(assure_local_evidence_surveillance(&request(vec![
            observation("a", EvidenceState::Supported),
            duplicate
        ]))
        .is_err());
    }
}
