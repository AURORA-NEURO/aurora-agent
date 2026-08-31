//! Multimodal multi-study evidence-surveillance inference engine.
//!
//! Atlas feature: `AFA-adapter-P01-F02`. This A1 scientific algorithm makes study × modality
//! comparability an explicit gate: it never treats one complete modality as a complete study.

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceAvailability, EvidenceReference,
    EvidenceState, ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P01-F02";
pub const CONTRACT_VERSION: &str = "adapter-multimodal-evidence-surveillance-inference-engine/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceFeed2@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet1@1";
const MAX_TEXT_BYTES: usize = 512;
const MAX_ITEMS: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalEvidenceObservation {
    pub source_id: String,
    pub study_id: String,
    pub modality: String,
    pub source_type: String,
    pub locator: String,
    pub semantic_profile: String,
    pub digest: Option<ContentHash>,
    pub availability: EvidenceAvailability,
    pub evidence_state: EvidenceState,
    pub relevance_score: u16,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalEvidenceSurveillanceRequest {
    pub request_id: String,
    pub intent: String,
    pub required_studies: Vec<String>,
    pub required_modalities: Vec<String>,
    pub semantic_profile: String,
    pub observations: Vec<MultimodalEvidenceObservation>,
    pub min_relevance_score: u16,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MultimodalEvidenceSurveillanceDisposition {
    Completed,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalQualifiedEvidenceSet {
    pub schema_version: String,
    pub set_id: String,
    pub intent: String,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub selected_digests: Vec<ContentHash>,
    pub coverage_order: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_order: Vec<String>,
    pub semantic_profile: String,
    pub evidence_state: EvidenceState,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultimodalEvidenceSurveillanceReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub input: MultimodalEvidenceSurveillanceRequest,
    pub input_digest: ContentHash,
    pub request_id: String,
    pub intent: String,
    pub semantic_profile: String,
    pub min_relevance_score: u16,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub disposition: MultimodalEvidenceSurveillanceDisposition,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub ranked_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub denied_order: Vec<String>,
    pub comparability_digest: ContentHash,
    pub evidence_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub qualified_set: MultimodalQualifiedEvidenceSet,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MultimodalEvidenceSurveillanceError {
    #[error("invalid multimodal evidence surveillance request: {0}")]
    Invalid(String),
    #[error("multimodal evidence surveillance artifact failed: {0}")]
    Artifact(String),
}

fn validate_text(field: &str, value: &str) -> Result<(), MultimodalEvidenceSurveillanceError> {
    if value.is_empty() || value.trim() != value {
        return Err(MultimodalEvidenceSurveillanceError::Invalid(format!(
            "{field} must be non-empty and trimmed"
        )));
    }
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(MultimodalEvidenceSurveillanceError::Invalid(format!(
            "{field} is outside its bounded text contract"
        )));
    }
    Ok(())
}

fn validate_unique_strings(
    field: &str,
    values: &[String],
) -> Result<(), MultimodalEvidenceSurveillanceError> {
    if values.len() > MAX_ITEMS {
        return Err(MultimodalEvidenceSurveillanceError::Invalid(format!(
            "{field} exceeds its item bound"
        )));
    }
    let mut unique = BTreeSet::new();
    for value in values {
        validate_text(field, value)?;
        if !unique.insert(value) {
            return Err(MultimodalEvidenceSurveillanceError::Invalid(format!(
                "{field} contains duplicate values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_strings(
    field: &str,
    values: &[String],
) -> Result<(), MultimodalEvidenceSurveillanceError> {
    validate_unique_strings(field, values)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(MultimodalEvidenceSurveillanceError::Invalid(format!(
            "{field} ordering is not canonical"
        )));
    }
    Ok(())
}

fn validate_digest(
    field: &str,
    digest: &ContentHash,
) -> Result<(), MultimodalEvidenceSurveillanceError> {
    if digest.as_str().len() != 64
        || !digest
            .as_str()
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(MultimodalEvidenceSurveillanceError::Invalid(format!(
            "{field} must be a 64-character hex digest"
        )));
    }
    Ok(())
}

fn canonical_multimodal_evidence_surveillance_request(
    request: &MultimodalEvidenceSurveillanceRequest,
) -> MultimodalEvidenceSurveillanceRequest {
    let mut canonical = request.clone();
    canonical.required_studies.sort();
    canonical.required_modalities.sort();
    canonical.observations.sort_by(|left, right| {
        right
            .relevance_score
            .cmp(&left.relevance_score)
            .then_with(|| left.study_id.cmp(&right.study_id))
            .then_with(|| left.modality.cmp(&right.modality))
            .then_with(|| left.source_id.cmp(&right.source_id))
    });
    canonical
}

fn multimodal_input_digest(
    request: &MultimodalEvidenceSurveillanceRequest,
) -> Result<ContentHash, MultimodalEvidenceSurveillanceError> {
    let canonical = canonical_multimodal_evidence_surveillance_request(request);
    let value = serde_json::to_value(canonical)
        .map_err(|error| MultimodalEvidenceSurveillanceError::Artifact(error.to_string()))?;
    ContentHash::of_value(&value)
        .map_err(|error| MultimodalEvidenceSurveillanceError::Artifact(error.to_string()))
}

impl MultimodalEvidenceSurveillanceReceipt {
    pub fn validate(&self) -> Result<(), MultimodalEvidenceSurveillanceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.intent.trim().is_empty()
            || self.study_order.len() < 2
            || self.modality_order.len() < 2
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
            || self.qualified_set.intent != self.intent
        {
            return Err(MultimodalEvidenceSurveillanceError::Invalid("multimodal identity, study/modality closure, locality, candidates, effects, or qualified-set linkage is incomplete".into()));
        }
        validate_text("request_id", &self.request_id)?;
        validate_text("intent", &self.intent)?;
        validate_text("semantic_profile", &self.semantic_profile)?;
        validate_text("boundary", &self.boundary)?;
        validate_sorted_strings("study_order", &self.study_order)?;
        validate_sorted_strings("modality_order", &self.modality_order)?;
        if !self.modality_order.iter().any(|value| value == "imaging")
            || !self.modality_order.iter().any(|value| value == "omics")
        {
            return Err(MultimodalEvidenceSurveillanceError::Invalid(
                "multimodal modality closure must include imaging and omics".into(),
            ));
        }
        validate_sorted_strings("candidate_order", &self.candidate_order)?;
        validate_unique_strings("ranked_order", &self.ranked_order)?;
        validate_sorted_strings("selected_order", &self.selected_order)?;
        validate_sorted_strings("unresolved_order", &self.unresolved_order)?;
        validate_sorted_strings("denied_order", &self.denied_order)?;
        validate_sorted_strings("omissions", &self.omissions)?;
        validate_sorted_strings("uncertainty", &self.uncertainty)?;
        validate_sorted_strings("negative_evidence", &self.negative_evidence)?;
        validate_sorted_strings("effect_receipts", &self.effect_receipts)?;
        validate_sorted_strings("qualified_set.study_order", &self.qualified_set.study_order)?;
        validate_sorted_strings(
            "qualified_set.modality_order",
            &self.qualified_set.modality_order,
        )?;
        validate_sorted_strings(
            "qualified_set.selected_order",
            &self.qualified_set.selected_order,
        )?;
        validate_sorted_strings(
            "qualified_set.coverage_order",
            &self.qualified_set.coverage_order,
        )?;
        validate_sorted_strings("qualified_set.omissions", &self.qualified_set.omissions)?;
        validate_sorted_strings("qualified_set.uncertainty", &self.qualified_set.uncertainty)?;
        validate_sorted_strings(
            "qualified_set.negative_order",
            &self.qualified_set.negative_order,
        )?;
        if self.ranked_order.len() != self.candidate_order.len()
            || self.ranked_order.iter().collect::<BTreeSet<_>>()
                != self.candidate_order.iter().collect::<BTreeSet<_>>()
        {
            return Err(MultimodalEvidenceSurveillanceError::Invalid(
                "multimodal ranking must cover candidates exactly".into(),
            ));
        }
        let classified = self
            .selected_order
            .iter()
            .chain(self.unresolved_order.iter())
            .chain(self.denied_order.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        if classified != self.candidate_order.iter().cloned().collect()
            || self.qualified_set.selected_order != self.selected_order
        {
            return Err(MultimodalEvidenceSurveillanceError::Invalid(
                "multimodal states do not partition candidates".into(),
            ));
        }
        let required_cells = self
            .study_order
            .iter()
            .flat_map(|study| {
                self.modality_order
                    .iter()
                    .map(move |modality| format!("{}::{}", study, modality))
            })
            .collect::<BTreeSet<_>>();
        let coverage = self
            .qualified_set
            .coverage_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        if !coverage.is_subset(&required_cells)
            || self.qualified_set.study_order != self.study_order
            || self.qualified_set.modality_order != self.modality_order
        {
            return Err(MultimodalEvidenceSurveillanceError::Invalid(
                "multimodal coverage is outside its declared study-by-modality grid".into(),
            ));
        }
        for digest in [
            &self.comparability_digest,
            &self.evidence_digest,
            &self.provenance_digest,
            &self.replay_identity,
            &self.artifact.content_hash,
        ] {
            validate_digest("multimodal receipt digest", digest)?;
        }
        for digest in &self.qualified_set.selected_digests {
            validate_digest("qualified_set.selected_digest", digest)?;
        }
        if self.qualified_set.selected_digests.len() != self.selected_order.len()
            || self.qualified_set.intent != self.intent
            || self.qualified_set.semantic_profile != self.semantic_profile
            || self.qualified_set.omissions != self.omissions
            || self.qualified_set.uncertainty != self.uncertainty
            || self.qualified_set.negative_order != self.negative_evidence
            || self.qualified_set.set_id
                != format!("qualified-multimodal-evidence:{}", self.request_id)
            || self.qualified_set.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.qualified_set.evidence_state
                != if self.disposition == MultimodalEvidenceSurveillanceDisposition::Completed {
                    EvidenceState::Supported
                } else {
                    EvidenceState::Unknown
                }
            || self.qualified_set.boundary != PRECLINICAL_BOUNDARY
        {
            return Err(MultimodalEvidenceSurveillanceError::Invalid(
                "multimodal qualified evidence set is not bound to the receipt".into(),
            ));
        }
        let should_block = !self.policy_allow || !self.protected_closure || !self.raw_data_local;
        if (self.disposition == MultimodalEvidenceSurveillanceDisposition::Blocked) != should_block
        {
            return Err(MultimodalEvidenceSurveillanceError::Invalid(
                "multimodal disposition does not match policy, closure, and locality gates".into(),
            ));
        }
        if matches!(
            self.disposition,
            MultimodalEvidenceSurveillanceDisposition::Unknown
                | MultimodalEvidenceSurveillanceDisposition::Blocked
        ) && !self.selected_order.is_empty()
        {
            return Err(MultimodalEvidenceSurveillanceError::Invalid(
                "unknown or blocked multimodal surveillance cannot retain selected evidence".into(),
            ));
        }
        if self.disposition == MultimodalEvidenceSurveillanceDisposition::Completed
            && (coverage != required_cells
                || !self.unresolved_order.is_empty()
                || !self.denied_order.is_empty())
        {
            return Err(MultimodalEvidenceSurveillanceError::Invalid(
                "completed multimodal surveillance cannot retain incomplete cells or denied evidence".into(),
            ));
        }
        let expected_effect = if should_block {
            vec!["block:unsafe-release".to_string()]
        } else {
            vec![format!(
                "read:local-multimodal-evidence:{}",
                self.request_id
            )]
        };
        if self.effect_receipts != expected_effect {
            return Err(MultimodalEvidenceSurveillanceError::Invalid(
                "multimodal effect does not match its release state".into(),
            ));
        }
        let expected_comparability = ContentHash::of_value(&json!({
            "study_order": self.study_order,
            "modality_order": self.modality_order,
            "semantic_profile": self.semantic_profile,
            "coverage_order": self.qualified_set.coverage_order,
        }))
        .map_err(|error| MultimodalEvidenceSurveillanceError::Artifact(error.to_string()))?;
        if self.comparability_digest != expected_comparability {
            return Err(MultimodalEvidenceSurveillanceError::Invalid(
                "multimodal comparability digest does not match the coverage grid".into(),
            ));
        }
        let expected_evidence = ContentHash::of_value(&json!({
            "candidate_order": self.candidate_order,
            "ranked_order": self.ranked_order,
            "selected_order": self.selected_order,
            "unresolved_order": self.unresolved_order,
            "denied_order": self.denied_order,
        }))
        .map_err(|error| MultimodalEvidenceSurveillanceError::Artifact(error.to_string()))?;
        if self.evidence_digest != expected_evidence {
            return Err(MultimodalEvidenceSurveillanceError::Invalid(
                "multimodal evidence digest does not match classified states".into(),
            ));
        }
        let expected_provenance = ContentHash::of_value(&json!({
            "request_id": self.request_id,
            "replay_identity": self.replay_identity,
            "comparability_digest": self.comparability_digest,
            "evidence_digest": self.evidence_digest,
        }))
        .map_err(|error| MultimodalEvidenceSurveillanceError::Artifact(error.to_string()))?;
        if self.provenance_digest != expected_provenance {
            return Err(MultimodalEvidenceSurveillanceError::Invalid(
                "multimodal provenance digest does not match request identity".into(),
            ));
        }
        if self.artifact.artifact_id != self.qualified_set.set_id
            || self.artifact.content_type
                != "application/vnd.aurora.qualified-multimodal-evidence-set+json"
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(MultimodalEvidenceSurveillanceError::Artifact(
                "multimodal artifact is not bound to the qualified evidence set".into(),
            ));
        }
        let qualified_payload = serde_json::to_value(&self.qualified_set)
            .map_err(|error| MultimodalEvidenceSurveillanceError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&qualified_payload)
            .map_err(|error| MultimodalEvidenceSurveillanceError::Artifact(error.to_string()))?;
        self.artifact
            .validate_metadata()
            .map_err(|error| MultimodalEvidenceSurveillanceError::Artifact(error.to_string()))?;
        if self.input_digest != multimodal_input_digest(&self.input)? {
            return Err(MultimodalEvidenceSurveillanceError::Invalid(
                "multimodal surveillance retained input digest mismatch".into(),
            ));
        }
        let expected = build_multimodal_evidence_surveillance(&self.input)?;
        if self != &expected {
            return Err(MultimodalEvidenceSurveillanceError::Invalid(
                "multimodal surveillance receipt does not match its retained input".into(),
            ));
        }
        Ok(())
    }
}

pub fn multimodal_evidence_surveillance_inference_engine_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "adapter".into(), consumers: ["integration engineer".into(), "multimodal evidence steward".into()].into(), behavior: "deterministically computes a comparable study-by-modality evidence stream while retaining incomplete cells and negative results".into(), value: "prevents single-modality evidence from being promoted as complete multi-study discovery".into(), inputs: vec![TypedPort { name: "evidence_feed".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "qualified_evidence_set".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ReadLocalData, Effect::WriteLocalArtifact].into(), permissions: ["read:local-research-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "OME-NGFF".into(), state: EvidenceState::Supported, locator: Some("https://ngff.openmicroscopy.org/rfc/5/".into()) }, EvidenceReference { source_id: "AnnData".into(), state: EvidenceState::Supported, locator: Some("https://anndata.readthedocs.io/en/stable/fileformat-prose.html".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn run_multimodal_evidence_surveillance(
    request: &MultimodalEvidenceSurveillanceRequest,
) -> Result<MultimodalEvidenceSurveillanceReceipt, MultimodalEvidenceSurveillanceError> {
    let receipt = build_multimodal_evidence_surveillance(request)?;
    receipt.validate()?;
    Ok(receipt)
}

fn build_multimodal_evidence_surveillance(
    request: &MultimodalEvidenceSurveillanceRequest,
) -> Result<MultimodalEvidenceSurveillanceReceipt, MultimodalEvidenceSurveillanceError> {
    if request.request_id.trim().is_empty()
        || request.intent.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_studies.len() < 2
        || request.required_modalities.len() < 2
        || request.observations.is_empty()
        || request.required_studies.len() > MAX_ITEMS
        || request.required_modalities.len() > MAX_ITEMS
        || request.observations.len() > MAX_ITEMS
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
    {
        return Err(MultimodalEvidenceSurveillanceError::Invalid("multimodal identity, required study/modality closure, observations, replay, locality, or boundary is invalid".into()));
    }
    validate_text("request_id", &request.request_id)?;
    validate_text("intent", &request.intent)?;
    validate_text("semantic_profile", &request.semantic_profile)?;
    validate_text("boundary", &request.boundary)?;
    validate_digest("replay_identity", &request.replay_identity)?;
    validate_unique_strings("required_studies", &request.required_studies)?;
    validate_unique_strings("required_modalities", &request.required_modalities)?;
    let mut studies = request.required_studies.clone();
    studies.sort();
    studies.dedup();
    let mut modalities = request.required_modalities.clone();
    modalities.sort();
    modalities.dedup();
    if studies.len() < 2
        || modalities.len() < 2
        || !modalities.iter().any(|value| value == "imaging")
        || !modalities.iter().any(|value| value == "omics")
    {
        return Err(MultimodalEvidenceSurveillanceError::Invalid("multimodal request must declare at least two studies and imaging plus omics modalities".into()));
    }
    let mut observation_keys = BTreeSet::new();
    for observation in &request.observations {
        validate_text("observation.source_id", &observation.source_id)?;
        validate_text("observation.study_id", &observation.study_id)?;
        validate_text("observation.modality", &observation.modality)?;
        validate_text("observation.source_type", &observation.source_type)?;
        validate_text("observation.locator", &observation.locator)?;
        validate_text(
            "observation.semantic_profile",
            &observation.semantic_profile,
        )?;
        if !observation.locator.starts_with("local://") {
            return Err(MultimodalEvidenceSurveillanceError::Invalid(
                "multimodal observation locator must use the local:// scheme".into(),
            ));
        }
        let observation_key = format!(
            "{}::{}::{}",
            observation.study_id, observation.modality, observation.source_id
        );
        if !observation_keys.insert(observation_key) {
            return Err(MultimodalEvidenceSurveillanceError::Invalid(
                "multimodal observation keys must be unique".into(),
            ));
        }
        if let Some(digest) = &observation.digest {
            validate_digest("observation.digest", digest)?;
        }
    }
    let mut observations = request.observations.clone();
    observations.sort_by(|left, right| {
        right
            .relevance_score
            .cmp(&left.relevance_score)
            .then_with(|| left.study_id.cmp(&right.study_id))
            .then_with(|| left.modality.cmp(&right.modality))
            .then_with(|| left.source_id.cmp(&right.source_id))
    });
    let key = |observation: &MultimodalEvidenceObservation| {
        format!(
            "{}::{}::{}",
            observation.study_id, observation.modality, observation.source_id
        )
    };
    let ranked_order = observations.iter().map(key).collect::<Vec<_>>();
    let mut candidate_order = ranked_order.clone();
    candidate_order.sort();
    let required_cells = studies
        .iter()
        .flat_map(|study| {
            modalities
                .iter()
                .map(move |modality| format!("{}::{}", study, modality))
        })
        .collect::<BTreeSet<_>>();
    let mut selected = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut denied = BTreeSet::new();
    let mut digest_map = BTreeMap::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut selected_cells = BTreeSet::new();
    for observation in &observations {
        let item_key = key(observation);
        let cell = format!("{}::{}", observation.study_id, observation.modality);
        if !studies.contains(&observation.study_id)
            || !modalities.contains(&observation.modality)
            || observation.source_type.trim().is_empty()
            || observation.locator.trim().is_empty()
            || !request.policy_allow
            || !request.protected_closure
            || !request.raw_data_local
        {
            denied.insert(item_key.clone());
            omissions.insert(format!("evidence:{}:scope-policy-locality", item_key));
        } else if observation.semantic_profile != request.semantic_profile {
            denied.insert(item_key.clone());
            omissions.insert(format!("evidence:{}:semantic-profile-mismatch", item_key));
            negative.insert(format!("evidence:{}:incomparable", item_key));
        } else if observation.availability != EvidenceAvailability::Available {
            unresolved.insert(item_key.clone());
            omissions.insert(format!(
                "evidence:{}:availability-{:?}",
                item_key, observation.availability
            ));
        } else if observation.relevance_score < request.min_relevance_score {
            unresolved.insert(item_key.clone());
            uncertainty.insert(format!("evidence:{}:relevance-below-threshold", item_key));
        } else if observation.digest.is_none() {
            unresolved.insert(item_key.clone());
            omissions.insert(format!("evidence:{}:content-digest-missing", item_key));
        } else if matches!(
            observation.evidence_state,
            EvidenceState::Unknown | EvidenceState::Speculative
        ) {
            unresolved.insert(item_key.clone());
            uncertainty.insert(format!("evidence:{}:unknown-not-asserted", item_key));
        } else if observation.evidence_state == EvidenceState::Contradicted {
            denied.insert(item_key.clone());
            negative.insert(format!("evidence:{}:contradicted", item_key));
        } else {
            if let Some(digest) = observation.digest.clone() {
                selected.insert(item_key.clone());
                selected_cells.insert(cell);
                digest_map.insert(item_key, digest);
                if observation.negative_result {
                    negative.insert(format!("evidence:{}:negative-result", key(observation)));
                }
            } else {
                unresolved.insert(item_key.clone());
                omissions.insert(format!("evidence:{}:content-digest-missing", item_key));
            }
        }
    }
    for cell in &required_cells {
        if !selected_cells.contains(cell) {
            omissions.insert(format!("cell:{}:required-modality-study-missing", cell));
            uncertainty.insert(format!("cell:{}:comparability-incomplete", cell));
        }
    }
    if !request.policy_allow {
        omissions.insert("control:policy-denied".into());
    }
    if !request.protected_closure {
        omissions.insert("control:protected-closure-incomplete".into());
    }
    if !request.raw_data_local {
        omissions.insert("control:raw-data-locality-failed".into());
    }
    let disposition =
        if !request.policy_allow || !request.protected_closure || !request.raw_data_local {
            MultimodalEvidenceSurveillanceDisposition::Blocked
        } else if selected.is_empty() {
            MultimodalEvidenceSurveillanceDisposition::Unknown
        } else if !unresolved.is_empty() || !denied.is_empty() || selected_cells != required_cells {
            MultimodalEvidenceSurveillanceDisposition::Partial
        } else {
            MultimodalEvidenceSurveillanceDisposition::Completed
        };
    let selected_order = selected.iter().cloned().collect::<Vec<_>>();
    let selected_digests = selected_order
        .iter()
        .filter_map(|item| digest_map.get(item).cloned())
        .collect::<Vec<_>>();
    let coverage_order = selected_cells.iter().cloned().collect::<Vec<_>>();
    let omissions_vec = omissions.iter().cloned().collect::<Vec<_>>();
    let uncertainty_vec = uncertainty.iter().cloned().collect::<Vec<_>>();
    let negative_vec = negative.iter().cloned().collect::<Vec<_>>();
    let comparability_digest = ContentHash::of_value(&json!({"study_order": studies.clone(), "modality_order": modalities.clone(), "semantic_profile": request.semantic_profile, "coverage_order": coverage_order.clone()})).map_err(|error| MultimodalEvidenceSurveillanceError::Artifact(error.to_string()))?;
    let evidence_digest = ContentHash::of_value(&json!({"candidate_order": candidate_order.clone(), "ranked_order": ranked_order.clone(), "selected_order": selected_order.clone(), "unresolved_order": unresolved.clone(), "denied_order": denied.clone()})).map_err(|error| MultimodalEvidenceSurveillanceError::Artifact(error.to_string()))?;
    let provenance_digest = ContentHash::of_value(&json!({"request_id": request.request_id, "replay_identity": request.replay_identity, "comparability_digest": comparability_digest, "evidence_digest": evidence_digest})).map_err(|error| MultimodalEvidenceSurveillanceError::Artifact(error.to_string()))?;
    let qualified_set = MultimodalQualifiedEvidenceSet {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        set_id: format!("qualified-multimodal-evidence:{}", request.request_id),
        intent: request.intent.clone(),
        study_order: studies.clone(),
        modality_order: modalities.clone(),
        selected_order: selected_order.clone(),
        selected_digests,
        coverage_order,
        omissions: omissions_vec.clone(),
        uncertainty: uncertainty_vec.clone(),
        negative_order: negative_vec.clone(),
        semantic_profile: request.semantic_profile.clone(),
        evidence_state: if disposition == MultimodalEvidenceSurveillanceDisposition::Completed {
            EvidenceState::Supported
        } else {
            EvidenceState::Unknown
        },
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let payload = serde_json::to_value(&qualified_set)
        .map_err(|error| MultimodalEvidenceSurveillanceError::Artifact(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        qualified_set.set_id.clone(),
        "application/vnd.aurora.qualified-multimodal-evidence-set+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| MultimodalEvidenceSurveillanceError::Artifact(error.to_string()))?;
    let canonical_request = canonical_multimodal_evidence_surveillance_request(request);
    let receipt = MultimodalEvidenceSurveillanceReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        input: canonical_request,
        input_digest: multimodal_input_digest(request)?,
        request_id: request.request_id.clone(),
        intent: request.intent.clone(),
        semantic_profile: request.semantic_profile.clone(),
        min_relevance_score: request.min_relevance_score,
        policy_allow: request.policy_allow,
        protected_closure: request.protected_closure,
        disposition,
        study_order: studies,
        modality_order: modalities,
        candidate_order,
        ranked_order,
        selected_order,
        unresolved_order: unresolved.into_iter().collect(),
        denied_order: denied.into_iter().collect(),
        comparability_digest,
        evidence_digest,
        provenance_digest,
        replay_identity: request.replay_identity.clone(),
        omissions: omissions_vec,
        uncertainty: uncertainty_vec,
        negative_evidence: negative_vec,
        effect_receipts: if disposition == MultimodalEvidenceSurveillanceDisposition::Blocked {
            vec!["block:unsafe-release".into()]
        } else {
            vec![format!(
                "read:local-multimodal-evidence:{}",
                request.request_id
            )]
        },
        qualified_set,
        artifact,
        raw_data_local: request.raw_data_local,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    Ok(receipt)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn request() -> MultimodalEvidenceSurveillanceRequest {
        let digest = hash("multimodal-evidence");
        let observation =
            |study: &str, modality: &str, source: &str, score: u16, state: EvidenceState| {
                MultimodalEvidenceObservation {
                    source_id: source.into(),
                    study_id: study.into(),
                    modality: modality.into(),
                    source_type: "local-record".into(),
                    locator: format!("local://{study}/{modality}/{source}"),
                    semantic_profile: "profile:ome-ngff-anndata-v1".into(),
                    digest: Some(digest.clone()),
                    availability: EvidenceAvailability::Available,
                    evidence_state: state,
                    relevance_score: score,
                    negative_result: source == "negative",
                }
            };
        MultimodalEvidenceSurveillanceRequest {
            request_id: "request:multimodal-evidence".into(),
            intent: "compare mechanism evidence".into(),
            required_studies: vec!["study:a".into(), "study:b".into()],
            required_modalities: vec!["imaging".into(), "omics".into()],
            semantic_profile: "profile:ome-ngff-anndata-v1".into(),
            observations: vec![
                observation(
                    "study:a",
                    "imaging",
                    "source:a-image",
                    95,
                    EvidenceState::Supported,
                ),
                observation(
                    "study:a",
                    "omics",
                    "source:a-omics",
                    90,
                    EvidenceState::Supported,
                ),
                observation(
                    "study:b",
                    "imaging",
                    "source:b-image",
                    85,
                    EvidenceState::Supported,
                ),
                observation("study:b", "omics", "negative", 80, EvidenceState::Supported),
            ],
            min_relevance_score: 70,
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            replay_identity: digest,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            multimodal_evidence_surveillance_inference_engine_manifest().autonomy_tier,
            AutonomyTier::A1
        );
    }
    #[test]
    fn comparable_cells_complete() {
        let receipt = run_multimodal_evidence_surveillance(&request()).unwrap();
        assert_eq!(
            receipt.disposition,
            MultimodalEvidenceSurveillanceDisposition::Completed
        );
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|value| value.contains("negative-result")));
    }
    #[test]
    fn missing_modality_is_partial() {
        let mut value = request();
        value.observations.pop();
        let receipt = run_multimodal_evidence_surveillance(&value).unwrap();
        assert_eq!(
            receipt.disposition,
            MultimodalEvidenceSurveillanceDisposition::Partial
        );
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item.contains("study:b::omics")));
    }
    #[test]
    fn semantic_profile_is_incomparable() {
        let mut value = request();
        value.observations[0].semantic_profile = "profile:other".into();
        let receipt = run_multimodal_evidence_surveillance(&value).unwrap();
        assert!(receipt
            .denied_order
            .iter()
            .any(|item| item.contains("study:a::imaging")));
    }
    #[test]
    fn unknown_is_not_asserted() {
        let mut value = request();
        value.observations[0].evidence_state = EvidenceState::Unknown;
        let receipt = run_multimodal_evidence_surveillance(&value).unwrap();
        assert!(receipt
            .uncertainty
            .iter()
            .any(|item| item.contains("unknown-not-asserted")));
    }
    #[test]
    fn policy_blocks() {
        let mut value = request();
        value.policy_allow = false;
        let receipt = run_multimodal_evidence_surveillance(&value).unwrap();
        assert_eq!(
            receipt.disposition,
            MultimodalEvidenceSurveillanceDisposition::Blocked
        );
        assert!(receipt.selected_order.is_empty());
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn external_locator_is_rejected_at_multimodal_boundary() {
        let mut value = request();
        value.observations[0].locator = "https://example.invalid/observation".into();
        assert!(run_multimodal_evidence_surveillance(&value).is_err());
    }
    #[test]
    fn duplicate_observation_key_is_rejected() {
        let mut value = request();
        value.observations.push(value.observations[0].clone());
        assert!(run_multimodal_evidence_surveillance(&value).is_err());
    }
    #[test]
    fn tampered_comparability_digest_is_rejected() {
        let mut receipt = run_multimodal_evidence_surveillance(&request()).unwrap();
        receipt.comparability_digest = hash("tampered-comparability");
        assert!(receipt.validate().is_err());
    }
    #[test]
    fn tampered_artifact_payload_is_rejected() {
        let mut receipt = run_multimodal_evidence_surveillance(&request()).unwrap();
        receipt.artifact.content_hash = hash("tampered-payload");
        assert!(receipt.validate().is_err());
    }
    #[test]
    fn replay_digest_is_stable() {
        let receipt = run_multimodal_evidence_surveillance(&request()).unwrap();
        let first = serde_json::to_value(&receipt).unwrap();
        let second =
            serde_json::to_value(&run_multimodal_evidence_surveillance(&request()).unwrap())
                .unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn reordered_dimensions_share_the_same_retained_input_identity() {
        let mut reordered = request();
        reordered.required_studies.reverse();
        reordered.required_modalities.reverse();
        reordered.observations.reverse();
        let first = run_multimodal_evidence_surveillance(&request()).unwrap();
        let second = run_multimodal_evidence_surveillance(&reordered).unwrap();
        assert_eq!(first.input_digest, second.input_digest);
        assert_eq!(
            serde_json::to_value(&first).unwrap(),
            serde_json::to_value(&second).unwrap()
        );
    }

    #[test]
    fn receipt_rejects_tampered_retained_modality_observation() {
        let mut receipt = run_multimodal_evidence_surveillance(&request()).unwrap();
        receipt.input.observations[0].modality = "tampered-modality".into();
        let error = receipt.validate().unwrap_err();
        assert!(error.to_string().contains("retained input digest mismatch"));
    }
}
