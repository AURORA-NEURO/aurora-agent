//! Local single-study evidence-surveillance inference engine.
//!
//! Atlas feature: `AFA-brain-P01-F01`.
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

pub const FEATURE_ID: &str = "AFA-brain-P01-F01";
pub const CONTRACT_VERSION: &str = "brain-evidence-surveillance/1.0";
pub const MAX_OBSERVATIONS: usize = 4096;
const SURVEILLANCE_CONTENT_TYPE: &str = "application/vnd.aurora.qualified-evidence-set+json";
const MAX_TEXT_BYTES: usize = 512;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceFeedRequest {
    pub request_id: String,
    pub study_id: String,
    pub scope: String,
    pub query: String,
    pub minimum_relevance_milli: u16,
    pub observations: Vec<EvidenceObservation>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceObservation {
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
pub enum EvidenceSurveillanceDisposition {
    Qualified,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualifiedEvidenceSet {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub study_id: String,
    pub scope: String,
    pub disposition: EvidenceSurveillanceDisposition,
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
pub enum EvidenceSurveillanceError {
    #[error("invalid evidence surveillance request: {0}")]
    Invalid(String),
    #[error("evidence surveillance artifact failed: {0}")]
    Artifact(String),
    #[error("evidence surveillance serialization failed: {0}")]
    Serialization(String),
}

impl QualifiedEvidenceSet {
    pub fn validate(&self) -> Result<(), EvidenceSurveillanceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.request_id.trim().is_empty()
            || self.study_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.relevance_order.len() != self.candidate_order.len()
            || self.effect_receipts.is_empty()
        {
            return Err(EvidenceSurveillanceError::Invalid(
                "evidence identity, ranking, locality, relevance, or effects are incomplete".into(),
            ));
        }
        for (value, field) in [
            (&self.request_id, "request_id"),
            (&self.study_id, "study_id"),
            (&self.scope, "scope"),
            (&self.boundary, "boundary"),
        ] {
            validate_text(value, field)?;
        }
        for (values, field) in [
            (&self.candidate_order, "candidate_order"),
            (&self.qualified_order, "qualified_order"),
            (&self.blocked_order, "blocked_order"),
            (&self.unknown_order, "unknown_order"),
            (&self.source_order, "source_order"),
            (&self.modality_order, "modality_order"),
            (&self.omissions, "omissions"),
            (&self.uncertainty, "uncertainty"),
            (&self.negative_evidence, "negative_evidence"),
            (&self.effect_receipts, "effect_receipts"),
        ] {
            validate_sorted_unique(values, field)?;
        }
        for (values, field) in [
            (&self.semantic_order, "semantic_order"),
            (&self.artifact_order, "artifact_order"),
            (&self.provenance_order, "provenance_order"),
        ] {
            validate_digest_order(values, field)?;
        }
        if self.relevance_order.iter().any(|value| *value > 1000) {
            return Err(EvidenceSurveillanceError::Invalid(
                "evidence relevance is outside the bounded range".into(),
            ));
        }
        let qualified_keys = identity_keys(&self.qualified_order);
        let blocked_keys = identity_keys(&self.blocked_order);
        let unknown_keys = identity_keys(&self.unknown_order);
        let mut admitted_or_blocked = self
            .qualified_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        admitted_or_blocked.extend(self.blocked_order.iter().cloned());
        if admitted_or_blocked
            != self
                .candidate_order
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>()
            || !qualified_keys.is_disjoint(&blocked_keys)
            || !unknown_keys.is_subset(&blocked_keys)
        {
            return Err(EvidenceSurveillanceError::Invalid(
                "evidence states must partition candidates without overlap".into(),
            ));
        }
        let gate_blocked = self
            .negative_evidence
            .iter()
            .any(|item| item == "request:policy-denied")
            || self
                .negative_evidence
                .iter()
                .any(|item| item == "request:raw-data-locality-failed")
            || self
                .uncertainty
                .iter()
                .any(|item| item == "request:protected-closure-incomplete");
        let expected_disposition = if !self.raw_data_local || gate_blocked {
            EvidenceSurveillanceDisposition::Blocked
        } else if self.qualified_order.is_empty() {
            EvidenceSurveillanceDisposition::Unknown
        } else if self.blocked_order.is_empty()
            && self.omissions.is_empty()
            && self.uncertainty.is_empty()
            && self.negative_evidence.is_empty()
        {
            EvidenceSurveillanceDisposition::Qualified
        } else {
            EvidenceSurveillanceDisposition::Partial
        };
        if self.disposition != expected_disposition {
            return Err(EvidenceSurveillanceError::Invalid(
                "evidence disposition does not match state, locality, or gates".into(),
            ));
        }
        if !self.raw_data_local {
            return Err(EvidenceSurveillanceError::Invalid(
                "evidence receipts must declare local emitted data".into(),
            ));
        }
        let expected_effect_receipts = if matches!(
            self.disposition,
            EvidenceSurveillanceDisposition::Qualified | EvidenceSurveillanceDisposition::Partial
        ) {
            vec![format!("read:local-research-artifacts:{}", self.request_id)]
        } else {
            vec!["block:unsafe-release".into()]
        };
        if self.effect_receipts != expected_effect_receipts {
            return Err(EvidenceSurveillanceError::Invalid(
                "evidence effect does not match disposition".into(),
            ));
        }
        let expected_artifact_id = format!("brain-evidence-surveillance:{}", self.request_id);
        if self.artifact.artifact_id != expected_artifact_id
            || self.artifact.content_type != SURVEILLANCE_CONTENT_TYPE
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(EvidenceSurveillanceError::Invalid(
                "evidence artifact identity or provenance is inconsistent".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| EvidenceSurveillanceError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&receipt_payload(self))
            .map_err(|error| EvidenceSurveillanceError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, EvidenceSurveillanceError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| EvidenceSurveillanceError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| EvidenceSurveillanceError::Serialization(error.to_string()))
    }
}

fn validate_text(value: &str, field: &str) -> Result<(), EvidenceSurveillanceError> {
    if value.trim().is_empty()
        || value != value.trim()
        || value.len() > MAX_TEXT_BYTES
        || value.chars().any(char::is_control)
    {
        return Err(EvidenceSurveillanceError::Invalid(format!(
            "{field} must be bounded, non-empty text without padding or control characters"
        )));
    }
    Ok(())
}

fn validate_unique(values: &[String], field: &str) -> Result<(), EvidenceSurveillanceError> {
    let mut seen = BTreeSet::new();
    for value in values {
        validate_text(value, field)?;
        if !seen.insert(value.to_ascii_lowercase()) {
            return Err(EvidenceSurveillanceError::Invalid(format!(
                "{field} contains duplicate or case-colliding values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_unique(values: &[String], field: &str) -> Result<(), EvidenceSurveillanceError> {
    validate_unique(values, field)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(EvidenceSurveillanceError::Invalid(format!(
            "{field} is not in canonical order"
        )));
    }
    Ok(())
}

fn identity_keys(values: &[String]) -> BTreeSet<String> {
    values
        .iter()
        .map(|value| value.to_ascii_lowercase())
        .collect()
}

fn validate_digest_order(
    values: &[ContentHash],
    field: &str,
) -> Result<(), EvidenceSurveillanceError> {
    if values.iter().any(|value| value.as_str().len() != 64)
        || values.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(EvidenceSurveillanceError::Invalid(format!(
            "{field} is not a canonical digest order"
        )));
    }
    Ok(())
}

fn receipt_payload(receipt: &QualifiedEvidenceSet) -> serde_json::Value {
    json!({
        "schema_version": receipt.schema_version,
        "contract_version": receipt.contract_version,
        "feature_id": receipt.feature_id,
        "request_id": receipt.request_id,
        "study_id": receipt.study_id,
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

pub fn evidence_surveillance_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "brain".into(),
        consumers: ["agent developer".into(), "research data steward".into()].into(),
        behavior: "qualifies a local single-study evidence feed deterministically without fetching sources or converting unknown evidence into confidence".into(),
        value: "provides replayable, omission-aware evidence alerts for the autonomous research kernel".into(),
        inputs: vec![TypedPort { name: "evidence_feed".into(), schema: "EvidenceFeed1@1".into(), required: true }],
        outputs: vec![TypedPort { name: "qualified_evidence_set".into(), schema: "QualifiedEvidenceSet1@1".into(), required: true }],
        effects: [Effect::ReadLocalData, Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(),
        permissions: ["read:local-research-artifacts".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference { source_id: "json-schema".into(), state: EvidenceState::Supported, locator: Some("https://json-schema.org/specification".into()) }],
        authority_requirements: Vec::new(),
        autonomy_tier: AutonomyTier::A0,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn surveil_evidence(
    request: &EvidenceFeedRequest,
) -> Result<QualifiedEvidenceSet, EvidenceSurveillanceError> {
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
    let locality_gate = request.raw_data_local
        && observations
            .iter()
            .all(|observation| observation.raw_data_local);
    if !locality_gate {
        negative.insert("request:raw-data-locality-failed".into());
    }
    let disposition = if !request.policy_allow || !request.protected_closure || !locality_gate {
        EvidenceSurveillanceDisposition::Blocked
    } else if qualified.is_empty() {
        EvidenceSurveillanceDisposition::Unknown
    } else if blocked.is_empty()
        && omissions.is_empty()
        && uncertainty.is_empty()
        && negative.is_empty()
    {
        EvidenceSurveillanceDisposition::Qualified
    } else {
        EvidenceSurveillanceDisposition::Partial
    };
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let unknown_order = unknown.into_iter().collect::<Vec<_>>();
    let source_order = sources.into_iter().collect::<Vec<_>>();
    let modality_order = modalities.into_iter().collect::<Vec<_>>();
    let semantic_order = semantics.into_iter().collect::<Vec<_>>();
    let artifact_order = artifacts.into_iter().collect::<Vec<_>>();
    let provenance_order = provenance.into_iter().collect::<Vec<_>>();
    let omissions = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence = negative.into_iter().collect::<Vec<_>>();
    let effect_receipts = if matches!(
        disposition,
        EvidenceSurveillanceDisposition::Qualified | EvidenceSurveillanceDisposition::Partial
    ) {
        vec![format!(
            "read:local-research-artifacts:{}",
            request.request_id
        )]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let raw_data_local = true;
    let receipt_without_artifact = QualifiedEvidenceSet {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        study_id: request.study_id.clone(),
        scope: request.scope.clone(),
        disposition,
        candidate_order,
        qualified_order: qualified,
        blocked_order,
        unknown_order,
        source_order,
        modality_order,
        relevance_order,
        semantic_order,
        artifact_order,
        provenance_order,
        omissions,
        uncertainty,
        negative_evidence,
        replay_identity: request.replay_identity.clone(),
        effect_receipts,
        artifact: TypedResearchArtifact::from_payload(
            "placeholder",
            SURVEILLANCE_CONTENT_TYPE,
            &json!({}),
            Vec::new(),
            Vec::new(),
        )
        .map_err(|error| EvidenceSurveillanceError::Artifact(error.to_string()))?,
        raw_data_local,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let artifact = TypedResearchArtifact::from_payload(
        format!("brain-evidence-surveillance:{}", request.request_id),
        SURVEILLANCE_CONTENT_TYPE,
        &receipt_payload(&receipt_without_artifact),
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| EvidenceSurveillanceError::Artifact(error.to_string()))?;
    let receipt = QualifiedEvidenceSet {
        artifact,
        ..receipt_without_artifact
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &EvidenceFeedRequest) -> Result<(), EvidenceSurveillanceError> {
    if request.request_id.trim().is_empty()
        || request.study_id.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.query.trim().is_empty()
        || request.minimum_relevance_milli > 1000
        || request.observations.is_empty()
        || request.observations.len() > MAX_OBSERVATIONS
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(EvidenceSurveillanceError::Invalid(
            "evidence request identity, query, observations, threshold, or boundary is incomplete"
                .into(),
        ));
    }
    for (value, field) in [
        (&request.request_id, "request_id"),
        (&request.study_id, "study_id"),
        (&request.scope, "scope"),
        (&request.query, "query"),
        (&request.boundary, "boundary"),
    ] {
        validate_text(value, field)?;
    }
    if request.replay_identity.as_str().len() != 64 {
        return Err(EvidenceSurveillanceError::Invalid(
            "evidence request replay identity is invalid".into(),
        ));
    }
    let observation_ids = request
        .observations
        .iter()
        .map(|observation| observation.evidence_id.clone())
        .collect::<Vec<_>>();
    validate_unique(&observation_ids, "observation.evidence_ids")?;
    for observation in &request.observations {
        for (value, field) in [
            (&observation.evidence_id, "observation.evidence_id"),
            (&observation.source_id, "observation.source_id"),
            (&observation.study_id, "observation.study_id"),
            (&observation.modality, "observation.modality"),
            (&observation.scope, "observation.scope"),
            (&observation.boundary, "observation.boundary"),
        ] {
            validate_text(value, field)?;
        }
        if observation.relevance_milli > 1000 || observation.boundary != PRECLINICAL_BOUNDARY {
            return Err(EvidenceSurveillanceError::Invalid(format!(
                "observation {} is outside the bounded evidence contract",
                observation.evidence_id
            )));
        }
        for digest in [
            &observation.semantic_digest,
            &observation.artifact_digest,
            &observation.provenance_digest,
            &observation.replay_identity,
        ] {
            if digest.as_str().len() != 64 {
                return Err(EvidenceSurveillanceError::Invalid(
                    "observation digest is invalid".into(),
                ));
            }
        }
        validate_unique(&observation.omissions, "observation.omissions")?;
        validate_unique(
            &observation.negative_evidence,
            "observation.negative_evidence",
        )?;
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
    fn request(observations: Vec<EvidenceObservation>) -> EvidenceFeedRequest {
        EvidenceFeedRequest {
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
        let manifest = evidence_surveillance_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A0);
    }
    #[test]
    fn supported_observations_are_qualified_and_ranked() {
        let receipt = surveil_evidence(&request(vec![
            observation("b", EvidenceState::Supported),
            observation("a", EvidenceState::Supported),
        ]))
        .unwrap();
        assert_eq!(
            receipt.disposition,
            EvidenceSurveillanceDisposition::Qualified
        );
        assert_eq!(receipt.candidate_order, vec!["evidence:a", "evidence:b"]);
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
    #[test]
    fn unknown_and_contradicted_observations_remain_visible() {
        let receipt = surveil_evidence(&request(vec![
            observation("a", EvidenceState::Supported),
            observation("b", EvidenceState::Unknown),
            observation("c", EvidenceState::Contradicted),
        ]))
        .unwrap();
        assert_eq!(
            receipt.disposition,
            EvidenceSurveillanceDisposition::Partial
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
        let receipt = surveil_evidence(&input).unwrap();
        assert_eq!(
            receipt.disposition,
            EvidenceSurveillanceDisposition::Blocked
        );
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn replay_mismatch_is_partial_and_explicit() {
        let mut input = request(vec![observation("a", EvidenceState::Supported)]);
        input.observations[0].replay_identity = hash("different");
        let receipt = surveil_evidence(&input).unwrap();
        assert_eq!(
            receipt.disposition,
            EvidenceSurveillanceDisposition::Unknown
        );
        assert!(receipt
            .uncertainty
            .iter()
            .any(|value| value.contains("replay-mismatch")));
    }
    #[test]
    fn non_local_observation_is_blocked_and_retained() {
        let mut input = request(vec![observation("a", EvidenceState::Supported)]);
        input.observations[0].raw_data_local = false;
        let receipt = surveil_evidence(&input).unwrap();
        assert_eq!(
            receipt.disposition,
            EvidenceSurveillanceDisposition::Blocked
        );
        assert!(receipt.raw_data_local);
        assert!(receipt
            .negative_evidence
            .contains(&"request:raw-data-locality-failed".into()));
    }

    #[test]
    fn artifact_payload_is_bound() {
        let mut receipt =
            surveil_evidence(&request(vec![observation("a", EvidenceState::Supported)])).unwrap();
        receipt.artifact.content_hash = hash("tampered");
        assert!(matches!(
            receipt.validate(),
            Err(EvidenceSurveillanceError::Artifact(_))
        ));
    }

    #[test]
    fn duplicate_observation_is_rejected() {
        let mut duplicate = observation("a", EvidenceState::Supported);
        duplicate.source_id = "source:other".into();
        assert!(surveil_evidence(&request(vec![
            observation("a", EvidenceState::Supported),
            duplicate
        ]))
        .is_err());
    }
}
