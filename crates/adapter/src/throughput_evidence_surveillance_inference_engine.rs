//! Prospective high-throughput evidence-surveillance admission engine.
//!
//! Atlas feature: `AFA-adapter-P01-F03`. Bounded admission and checkpoint witnesses make queue
//! pressure observable; an overflow is never silently treated as absent evidence.

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

pub const FEATURE_ID: &str = "AFA-adapter-P01-F03";
pub const CONTRACT_VERSION: &str = "adapter-throughput-evidence-surveillance-inference-engine/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceFeed3@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet1@1";
const MAX_TEXT_BYTES: usize = 512;
const MAX_ITEMS: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputEvidenceObservation {
    pub observation_id: String,
    pub batch_id: String,
    pub sequence: u64,
    pub study_id: String,
    pub modality: String,
    pub digest: Option<ContentHash>,
    pub availability: EvidenceAvailability,
    pub evidence_state: EvidenceState,
    pub relevance_score: u16,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputEvidenceSurveillanceRequest {
    pub request_id: String,
    pub batch_id: String,
    pub checkpoint_seq: u64,
    pub previous_checkpoint: Option<ContentHash>,
    pub observations: Vec<ThroughputEvidenceObservation>,
    pub max_items: usize,
    pub budget_units: usize,
    pub min_relevance_score: u16,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ThroughputEvidenceSurveillanceDisposition {
    Completed,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputQualifiedEvidenceSet {
    pub schema_version: String,
    pub set_id: String,
    pub batch_id: String,
    pub checkpoint_seq: u64,
    pub selected_order: Vec<String>,
    pub selected_digests: Vec<ContentHash>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_order: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ThroughputEvidenceSurveillanceReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub input: ThroughputEvidenceSurveillanceRequest,
    pub input_digest: ContentHash,
    pub request_id: String,
    pub batch_id: String,
    pub checkpoint_seq: u64,
    pub previous_checkpoint: Option<ContentHash>,
    pub max_items: usize,
    pub budget_units: usize,
    pub min_relevance_score: u16,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub disposition: ThroughputEvidenceSurveillanceDisposition,
    pub candidate_order: Vec<String>,
    pub ranked_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub denied_order: Vec<String>,
    pub overflow_order: Vec<String>,
    pub queue_digest: ContentHash,
    pub checkpoint_digest: ContentHash,
    pub evidence_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub qualified_set: ThroughputQualifiedEvidenceSet,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ThroughputEvidenceSurveillanceError {
    #[error("invalid throughput evidence surveillance request: {0}")]
    Invalid(String),
    #[error("throughput evidence artifact failed: {0}")]
    Artifact(String),
}

fn validate_text(field: &str, value: &str) -> Result<(), ThroughputEvidenceSurveillanceError> {
    if value.is_empty() || value.trim() != value {
        return Err(ThroughputEvidenceSurveillanceError::Invalid(format!(
            "{field} must be non-empty and trimmed"
        )));
    }
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(ThroughputEvidenceSurveillanceError::Invalid(format!(
            "{field} is outside its bounded text contract"
        )));
    }
    Ok(())
}

fn validate_unique_strings(
    field: &str,
    values: &[String],
) -> Result<(), ThroughputEvidenceSurveillanceError> {
    if values.len() > MAX_ITEMS {
        return Err(ThroughputEvidenceSurveillanceError::Invalid(format!(
            "{field} exceeds its item bound"
        )));
    }
    let mut unique = BTreeSet::new();
    for value in values {
        validate_text(field, value)?;
        if !unique.insert(value) {
            return Err(ThroughputEvidenceSurveillanceError::Invalid(format!(
                "{field} contains duplicate values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_strings(
    field: &str,
    values: &[String],
) -> Result<(), ThroughputEvidenceSurveillanceError> {
    validate_unique_strings(field, values)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(ThroughputEvidenceSurveillanceError::Invalid(format!(
            "{field} ordering is not canonical"
        )));
    }
    Ok(())
}

fn validate_digest(
    field: &str,
    digest: &ContentHash,
) -> Result<(), ThroughputEvidenceSurveillanceError> {
    if digest.as_str().len() != 64
        || !digest
            .as_str()
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(ThroughputEvidenceSurveillanceError::Invalid(format!(
            "{field} must be a 64-character hex digest"
        )));
    }
    Ok(())
}

fn canonical_throughput_evidence_surveillance_request(
    request: &ThroughputEvidenceSurveillanceRequest,
) -> ThroughputEvidenceSurveillanceRequest {
    let mut canonical = request.clone();
    canonical.observations.sort_by(|left, right| {
        left.sequence
            .cmp(&right.sequence)
            .then_with(|| left.observation_id.cmp(&right.observation_id))
    });
    canonical
}

fn throughput_input_digest(
    request: &ThroughputEvidenceSurveillanceRequest,
) -> Result<ContentHash, ThroughputEvidenceSurveillanceError> {
    let canonical = canonical_throughput_evidence_surveillance_request(request);
    let value = serde_json::to_value(canonical)
        .map_err(|error| ThroughputEvidenceSurveillanceError::Artifact(error.to_string()))?;
    ContentHash::of_value(&value)
        .map_err(|error| ThroughputEvidenceSurveillanceError::Artifact(error.to_string()))
}

impl ThroughputEvidenceSurveillanceReceipt {
    pub fn validate(&self) -> Result<(), ThroughputEvidenceSurveillanceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.batch_id.trim().is_empty()
            || self.checkpoint_seq == 0
            || self.max_items == 0
            || self.max_items > MAX_ITEMS
            || self.budget_units == 0
            || self.budget_units > MAX_ITEMS
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
            || self.qualified_set.batch_id != self.batch_id
            || self.qualified_set.checkpoint_seq != self.checkpoint_seq
        {
            return Err(ThroughputEvidenceSurveillanceError::Invalid("throughput identity, checkpoint, bounded admission, locality, candidates, effects, or qualified-set linkage is incomplete".into()));
        }
        validate_text("request_id", &self.request_id)?;
        validate_text("batch_id", &self.batch_id)?;
        validate_text("boundary", &self.boundary)?;
        validate_sorted_strings("candidate_order", &self.candidate_order)?;
        validate_unique_strings("ranked_order", &self.ranked_order)?;
        validate_sorted_strings("selected_order", &self.selected_order)?;
        validate_sorted_strings("unresolved_order", &self.unresolved_order)?;
        validate_sorted_strings("denied_order", &self.denied_order)?;
        validate_sorted_strings("overflow_order", &self.overflow_order)?;
        validate_sorted_strings("omissions", &self.omissions)?;
        validate_sorted_strings("uncertainty", &self.uncertainty)?;
        validate_sorted_strings("negative_evidence", &self.negative_evidence)?;
        validate_sorted_strings("effect_receipts", &self.effect_receipts)?;
        validate_sorted_strings(
            "qualified_set.selected_order",
            &self.qualified_set.selected_order,
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
            return Err(ThroughputEvidenceSurveillanceError::Invalid(
                "throughput ranking must cover candidates exactly".into(),
            ));
        }
        let classified = self
            .selected_order
            .iter()
            .chain(self.unresolved_order.iter())
            .chain(self.denied_order.iter())
            .chain(self.overflow_order.iter())
            .cloned()
            .collect::<BTreeSet<_>>();
        if classified != self.candidate_order.iter().cloned().collect()
            || self.qualified_set.selected_order != self.selected_order
        {
            return Err(ThroughputEvidenceSurveillanceError::Invalid(
                "throughput states do not partition candidates".into(),
            ));
        }
        let admission_limit = self.max_items.min(self.budget_units);
        let expected_overflow_len = self.candidate_order.len().saturating_sub(admission_limit);
        let mut expected_overflow =
            self.ranked_order[admission_limit.min(self.ranked_order.len())..].to_vec();
        expected_overflow.sort();
        if self.overflow_order.len() != expected_overflow_len
            || self.overflow_order != expected_overflow
        {
            return Err(ThroughputEvidenceSurveillanceError::Invalid(
                "throughput overflow does not match ranked admission capacity".into(),
            ));
        }
        if self.qualified_set.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.qualified_set.set_id
                != format!("qualified-throughput-evidence:{}", self.batch_id)
            || self.qualified_set.selected_digests.len() != self.selected_order.len()
            || self.qualified_set.omissions != self.omissions
            || self.qualified_set.uncertainty != self.uncertainty
            || self.qualified_set.negative_order != self.negative_evidence
            || self.qualified_set.boundary != PRECLINICAL_BOUNDARY
        {
            return Err(ThroughputEvidenceSurveillanceError::Invalid(
                "throughput qualified evidence set is not bound to the receipt".into(),
            ));
        }
        for digest in &self.qualified_set.selected_digests {
            validate_digest("qualified_set.selected_digest", digest)?;
        }
        for digest in [
            &self.queue_digest,
            &self.checkpoint_digest,
            &self.evidence_digest,
            &self.provenance_digest,
            &self.replay_identity,
            &self.artifact.content_hash,
        ] {
            validate_digest("throughput receipt digest", digest)?;
        }
        if let Some(previous_checkpoint) = &self.previous_checkpoint {
            validate_digest("previous_checkpoint", previous_checkpoint)?;
        }
        let should_block = !self.policy_allow || !self.protected_closure || !self.raw_data_local;
        if (self.disposition == ThroughputEvidenceSurveillanceDisposition::Blocked) != should_block
        {
            return Err(ThroughputEvidenceSurveillanceError::Invalid(
                "throughput disposition does not match policy, closure, and locality gates".into(),
            ));
        }
        if matches!(
            self.disposition,
            ThroughputEvidenceSurveillanceDisposition::Unknown
                | ThroughputEvidenceSurveillanceDisposition::Blocked
        ) && !self.selected_order.is_empty()
        {
            return Err(ThroughputEvidenceSurveillanceError::Invalid(
                "unknown or blocked throughput surveillance cannot retain selected evidence".into(),
            ));
        }
        if self.disposition == ThroughputEvidenceSurveillanceDisposition::Completed
            && (!self.unresolved_order.is_empty()
                || !self.denied_order.is_empty()
                || !self.overflow_order.is_empty())
        {
            return Err(ThroughputEvidenceSurveillanceError::Invalid(
                "completed throughput surveillance cannot retain unresolved, denied, or overflow states".into(),
            ));
        }
        let expected_effect = if should_block {
            vec!["block:unsafe-release".to_string()]
        } else {
            vec![format!("read:local-throughput-evidence:{}", self.batch_id)]
        };
        if self.effect_receipts != expected_effect {
            return Err(ThroughputEvidenceSurveillanceError::Invalid(
                "throughput effect does not match its release state".into(),
            ));
        }
        let expected_queue = ContentHash::of_value(&json!({
            "batch_id": self.batch_id,
            "candidate_order": self.candidate_order,
            "ranked_order": self.ranked_order,
            "overflow_order": self.overflow_order,
        }))
        .map_err(|error| ThroughputEvidenceSurveillanceError::Artifact(error.to_string()))?;
        if self.queue_digest != expected_queue {
            return Err(ThroughputEvidenceSurveillanceError::Invalid(
                "throughput queue digest does not match ranked admission".into(),
            ));
        }
        let expected_checkpoint = ContentHash::of_value(&json!({
            "batch_id": self.batch_id,
            "checkpoint_seq": self.checkpoint_seq,
            "previous_checkpoint": self.previous_checkpoint,
            "queue_digest": self.queue_digest,
        }))
        .map_err(|error| ThroughputEvidenceSurveillanceError::Artifact(error.to_string()))?;
        if self.checkpoint_digest != expected_checkpoint {
            return Err(ThroughputEvidenceSurveillanceError::Invalid(
                "throughput checkpoint digest does not match its predecessor and queue".into(),
            ));
        }
        let expected_evidence = ContentHash::of_value(&json!({
            "selected_order": self.selected_order,
            "unresolved_order": self.unresolved_order,
            "denied_order": self.denied_order,
        }))
        .map_err(|error| ThroughputEvidenceSurveillanceError::Artifact(error.to_string()))?;
        if self.evidence_digest != expected_evidence {
            return Err(ThroughputEvidenceSurveillanceError::Invalid(
                "throughput evidence digest does not match classified states".into(),
            ));
        }
        let expected_provenance = ContentHash::of_value(&json!({
            "request_id": self.request_id,
            "replay_identity": self.replay_identity,
            "checkpoint_digest": self.checkpoint_digest,
            "evidence_digest": self.evidence_digest,
        }))
        .map_err(|error| ThroughputEvidenceSurveillanceError::Artifact(error.to_string()))?;
        if self.provenance_digest != expected_provenance {
            return Err(ThroughputEvidenceSurveillanceError::Invalid(
                "throughput provenance digest does not match request identity".into(),
            ));
        }
        if self.artifact.artifact_id != self.qualified_set.set_id
            || self.artifact.content_type
                != "application/vnd.aurora.qualified-throughput-evidence-set+json"
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(ThroughputEvidenceSurveillanceError::Artifact(
                "throughput artifact is not bound to the qualified evidence set".into(),
            ));
        }
        let qualified_payload = serde_json::to_value(&self.qualified_set)
            .map_err(|error| ThroughputEvidenceSurveillanceError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&qualified_payload)
            .map_err(|error| ThroughputEvidenceSurveillanceError::Artifact(error.to_string()))?;
        self.artifact
            .validate_metadata()
            .map_err(|error| ThroughputEvidenceSurveillanceError::Artifact(error.to_string()))?;
        if self.input_digest != throughput_input_digest(&self.input)? {
            return Err(ThroughputEvidenceSurveillanceError::Invalid(
                "throughput surveillance retained input digest mismatch".into(),
            ));
        }
        let expected = build_throughput_evidence_surveillance(&self.input)?;
        if self != &expected {
            return Err(ThroughputEvidenceSurveillanceError::Invalid(
                "throughput surveillance receipt does not match its retained input".into(),
            ));
        }
        Ok(())
    }
}

pub fn throughput_evidence_surveillance_inference_engine_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "adapter".into(), consumers: ["AURORA extension developer".into(), "queue operator".into()].into(), behavior: "deterministically admits bounded prospective evidence batches with checkpoint and capacity witnesses".into(), value: "preserves high-throughput omissions and overflow instead of silently dropping evidence".into(), inputs: vec![TypedPort { name: "evidence_feed".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "qualified_evidence_set".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ReadLocalData, Effect::WriteLocalArtifact].into(), permissions: ["read:local-research-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "OpenTelemetry".into(), state: EvidenceState::Supported, locator: Some("https://opentelemetry.io/docs/specs/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn run_throughput_evidence_surveillance(
    request: &ThroughputEvidenceSurveillanceRequest,
) -> Result<ThroughputEvidenceSurveillanceReceipt, ThroughputEvidenceSurveillanceError> {
    let receipt = build_throughput_evidence_surveillance(request)?;
    receipt.validate()?;
    Ok(receipt)
}

fn build_throughput_evidence_surveillance(
    request: &ThroughputEvidenceSurveillanceRequest,
) -> Result<ThroughputEvidenceSurveillanceReceipt, ThroughputEvidenceSurveillanceError> {
    if request.request_id.trim().is_empty()
        || request.batch_id.trim().is_empty()
        || request.checkpoint_seq == 0
        || request.observations.is_empty()
        || request.max_items == 0
        || request.budget_units == 0
        || request.max_items > MAX_ITEMS
        || request.budget_units > MAX_ITEMS
        || request.observations.len() > MAX_ITEMS
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
    {
        return Err(ThroughputEvidenceSurveillanceError::Invalid("throughput identity, checkpoint, observations, capacity, budget, replay, locality, or boundary is invalid".into()));
    }
    validate_text("request_id", &request.request_id)?;
    validate_text("batch_id", &request.batch_id)?;
    validate_text("boundary", &request.boundary)?;
    validate_digest("replay_identity", &request.replay_identity)?;
    if let Some(previous_checkpoint) = &request.previous_checkpoint {
        validate_digest("previous_checkpoint", previous_checkpoint)?;
    }
    let mut observation_ids = BTreeSet::new();
    let mut sequences = BTreeSet::new();
    for item in &request.observations {
        if item.batch_id != request.batch_id {
            return Err(ThroughputEvidenceSurveillanceError::Invalid(
                "observation batch or identity mismatch".into(),
            ));
        }
        validate_text("observation_id", &item.observation_id)?;
        validate_text("observation.batch_id", &item.batch_id)?;
        validate_text("observation.study_id", &item.study_id)?;
        validate_text("observation.modality", &item.modality)?;
        if !observation_ids.insert(item.observation_id.clone()) {
            return Err(ThroughputEvidenceSurveillanceError::Invalid(
                "throughput observation identities must be unique".into(),
            ));
        }
        if !sequences.insert(item.sequence) {
            return Err(ThroughputEvidenceSurveillanceError::Invalid(
                "throughput sequence values must be unique".into(),
            ));
        }
        if let Some(digest) = &item.digest {
            validate_digest("observation.digest", digest)?;
        }
    }
    let mut observations = request.observations.clone();
    observations.sort_by(|left, right| {
        left.sequence
            .cmp(&right.sequence)
            .then_with(|| left.observation_id.cmp(&right.observation_id))
    });
    let key = |item: &ThroughputEvidenceObservation| item.observation_id.clone();
    let ranked_order = observations.iter().map(key).collect::<Vec<_>>();
    let mut candidate_order = ranked_order.clone();
    candidate_order.sort();
    if candidate_order.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(ThroughputEvidenceSurveillanceError::Invalid(
            "throughput observation identities must be unique".into(),
        ));
    }
    let admission_limit = request.max_items.min(request.budget_units);
    let (admitted, overflow) = observations.split_at(admission_limit.min(observations.len()));
    let overflow_order = overflow.iter().map(key).collect::<BTreeSet<_>>();
    let mut selected = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut denied = BTreeSet::new();
    let mut digest_map = BTreeMap::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    if observations.len() > request.max_items {
        omissions.insert(format!(
            "queue:capacity-exceeded:{}",
            observations.len() - request.max_items
        ));
    }
    if request.budget_units < request.max_items {
        omissions.insert(format!(
            "queue:budget-bounded:{}",
            request.max_items - request.budget_units
        ));
    }
    for item in admitted {
        let item_key = key(item);
        if !request.policy_allow || !request.protected_closure || !request.raw_data_local {
            denied.insert(item_key.clone());
            omissions.insert(format!("evidence:{}:policy-closure-locality", item_key));
        } else if item.availability != EvidenceAvailability::Available {
            unresolved.insert(item_key.clone());
            omissions.insert(format!(
                "evidence:{}:availability-{:?}",
                item_key, item.availability
            ));
        } else if item.relevance_score < request.min_relevance_score {
            unresolved.insert(item_key.clone());
            uncertainty.insert(format!("evidence:{}:relevance-below-threshold", item_key));
        } else if item.digest.is_none() {
            unresolved.insert(item_key.clone());
            omissions.insert(format!("evidence:{}:content-digest-missing", item_key));
        } else if matches!(
            item.evidence_state,
            EvidenceState::Unknown | EvidenceState::Speculative
        ) {
            unresolved.insert(item_key.clone());
            uncertainty.insert(format!("evidence:{}:unknown-not-asserted", item_key));
        } else if item.evidence_state == EvidenceState::Contradicted {
            denied.insert(item_key.clone());
            negative.insert(format!("evidence:{}:contradicted", item_key));
        } else {
            if let Some(digest) = item.digest.clone() {
                selected.insert(item_key.clone());
                digest_map.insert(item_key.clone(), digest);
                if item.negative_result {
                    negative.insert(format!("evidence:{}:negative-result", item_key));
                }
            } else {
                unresolved.insert(item_key.clone());
                omissions.insert(format!("evidence:{}:content-digest-missing", item_key));
            }
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
            ThroughputEvidenceSurveillanceDisposition::Blocked
        } else if selected.is_empty() {
            ThroughputEvidenceSurveillanceDisposition::Unknown
        } else if !unresolved.is_empty() || !denied.is_empty() || !overflow_order.is_empty() {
            ThroughputEvidenceSurveillanceDisposition::Partial
        } else {
            ThroughputEvidenceSurveillanceDisposition::Completed
        };
    let selected_order = selected.iter().cloned().collect::<Vec<_>>();
    let unresolved_order = unresolved.iter().cloned().collect::<Vec<_>>();
    let denied_order = denied.iter().cloned().collect::<Vec<_>>();
    let overflow_order = overflow_order.into_iter().collect::<Vec<_>>();
    let omissions_vec = omissions.iter().cloned().collect::<Vec<_>>();
    let uncertainty_vec = uncertainty.iter().cloned().collect::<Vec<_>>();
    let negative_vec = negative.iter().cloned().collect::<Vec<_>>();
    let queue_digest = ContentHash::of_value(&json!({"batch_id": request.batch_id, "candidate_order": candidate_order.clone(), "ranked_order": ranked_order.clone(), "overflow_order": overflow_order.clone()})).map_err(|error| ThroughputEvidenceSurveillanceError::Artifact(error.to_string()))?;
    let checkpoint_digest = ContentHash::of_value(&json!({"batch_id": request.batch_id, "checkpoint_seq": request.checkpoint_seq, "previous_checkpoint": request.previous_checkpoint, "queue_digest": queue_digest})).map_err(|error| ThroughputEvidenceSurveillanceError::Artifact(error.to_string()))?;
    let evidence_digest = ContentHash::of_value(&json!({"selected_order": selected_order.clone(), "unresolved_order": unresolved_order.clone(), "denied_order": denied_order.clone()})).map_err(|error| ThroughputEvidenceSurveillanceError::Artifact(error.to_string()))?;
    let provenance_digest = ContentHash::of_value(&json!({"request_id": request.request_id, "replay_identity": request.replay_identity, "checkpoint_digest": checkpoint_digest, "evidence_digest": evidence_digest})).map_err(|error| ThroughputEvidenceSurveillanceError::Artifact(error.to_string()))?;
    let selected_digests = selected_order
        .iter()
        .filter_map(|item| digest_map.get(item).cloned())
        .collect::<Vec<_>>();
    let qualified_set = ThroughputQualifiedEvidenceSet {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        set_id: format!("qualified-throughput-evidence:{}", request.batch_id),
        batch_id: request.batch_id.clone(),
        checkpoint_seq: request.checkpoint_seq,
        selected_order: selected_order.clone(),
        selected_digests,
        omissions: omissions_vec.clone(),
        uncertainty: uncertainty_vec.clone(),
        negative_order: negative_vec.clone(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let payload = serde_json::to_value(&qualified_set)
        .map_err(|error| ThroughputEvidenceSurveillanceError::Artifact(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        qualified_set.set_id.clone(),
        "application/vnd.aurora.qualified-throughput-evidence-set+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| ThroughputEvidenceSurveillanceError::Artifact(error.to_string()))?;
    let canonical_request = canonical_throughput_evidence_surveillance_request(request);
    let receipt = ThroughputEvidenceSurveillanceReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        input: canonical_request,
        input_digest: throughput_input_digest(request)?,
        request_id: request.request_id.clone(),
        batch_id: request.batch_id.clone(),
        checkpoint_seq: request.checkpoint_seq,
        previous_checkpoint: request.previous_checkpoint.clone(),
        max_items: request.max_items,
        budget_units: request.budget_units,
        min_relevance_score: request.min_relevance_score,
        policy_allow: request.policy_allow,
        protected_closure: request.protected_closure,
        disposition,
        candidate_order,
        ranked_order,
        selected_order,
        unresolved_order,
        denied_order,
        overflow_order,
        queue_digest,
        checkpoint_digest,
        evidence_digest,
        provenance_digest,
        replay_identity: request.replay_identity.clone(),
        omissions: omissions_vec,
        uncertainty: uncertainty_vec,
        negative_evidence: negative_vec,
        effect_receipts: if disposition == ThroughputEvidenceSurveillanceDisposition::Blocked {
            vec!["block:unsafe-release".into()]
        } else {
            vec![format!(
                "read:local-throughput-evidence:{}",
                request.batch_id
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
    fn request() -> ThroughputEvidenceSurveillanceRequest {
        let digest = hash("throughput-evidence");
        let observation =
            |id: &str, sequence: u64, state: EvidenceState| ThroughputEvidenceObservation {
                observation_id: id.into(),
                batch_id: "batch:one".into(),
                sequence,
                study_id: "study:one".into(),
                modality: "imaging".into(),
                digest: Some(digest.clone()),
                availability: EvidenceAvailability::Available,
                evidence_state: state,
                relevance_score: 90,
                negative_result: id == "negative",
            };
        ThroughputEvidenceSurveillanceRequest {
            request_id: "request:throughput".into(),
            batch_id: "batch:one".into(),
            checkpoint_seq: 7,
            previous_checkpoint: Some(digest.clone()),
            observations: vec![
                observation("obs:a", 1, EvidenceState::Supported),
                observation("negative", 2, EvidenceState::Supported),
            ],
            max_items: 4,
            budget_units: 4,
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
            throughput_evidence_surveillance_inference_engine_manifest().autonomy_tier,
            AutonomyTier::A1
        );
    }
    #[test]
    fn bounded_batch_completes() {
        let receipt = run_throughput_evidence_surveillance(&request()).unwrap();
        assert_eq!(
            receipt.disposition,
            ThroughputEvidenceSurveillanceDisposition::Completed
        );
    }
    #[test]
    fn overflow_is_partial() {
        let mut value = request();
        value.max_items = 1;
        let receipt = run_throughput_evidence_surveillance(&value).unwrap();
        assert_eq!(
            receipt.disposition,
            ThroughputEvidenceSurveillanceDisposition::Partial
        );
        assert_eq!(receipt.overflow_order.len(), 1);
    }
    #[test]
    fn overflow_tail_is_canonicalized() {
        let mut value = request();
        let digest = hash("overflow-tail");
        value.max_items = 2;
        value.observations.extend([
            ThroughputEvidenceObservation {
                observation_id: "z-last".into(),
                batch_id: "batch:one".into(),
                sequence: 3,
                study_id: "study:one".into(),
                modality: "imaging".into(),
                digest: Some(digest.clone()),
                availability: EvidenceAvailability::Available,
                evidence_state: EvidenceState::Supported,
                relevance_score: 90,
                negative_result: false,
            },
            ThroughputEvidenceObservation {
                observation_id: "a-last".into(),
                batch_id: "batch:one".into(),
                sequence: 4,
                study_id: "study:one".into(),
                modality: "imaging".into(),
                digest: Some(digest),
                availability: EvidenceAvailability::Available,
                evidence_state: EvidenceState::Supported,
                relevance_score: 90,
                negative_result: false,
            },
        ]);
        let receipt = run_throughput_evidence_surveillance(&value).unwrap();
        assert_eq!(receipt.overflow_order, vec!["a-last", "z-last"]);
    }
    #[test]
    fn missing_digest_is_unresolved() {
        let mut value = request();
        value.observations[0].digest = None;
        let receipt = run_throughput_evidence_surveillance(&value).unwrap();
        assert!(receipt.unresolved_order.contains(&"obs:a".to_string()));
    }
    #[test]
    fn unknown_is_not_asserted() {
        let mut value = request();
        value.observations[0].evidence_state = EvidenceState::Unknown;
        let receipt = run_throughput_evidence_surveillance(&value).unwrap();
        assert!(receipt
            .uncertainty
            .iter()
            .any(|item| item.contains("unknown-not-asserted")));
    }
    #[test]
    fn policy_blocks() {
        let mut value = request();
        value.policy_allow = false;
        let receipt = run_throughput_evidence_surveillance(&value).unwrap();
        assert_eq!(
            receipt.disposition,
            ThroughputEvidenceSurveillanceDisposition::Blocked
        );
        assert!(receipt.selected_order.is_empty());
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
    }
    #[test]
    fn duplicate_sequence_is_rejected() {
        let mut value = request();
        value.observations[1].sequence = value.observations[0].sequence;
        assert!(run_throughput_evidence_surveillance(&value).is_err());
    }
    #[test]
    fn tampered_queue_digest_is_rejected() {
        let mut receipt = run_throughput_evidence_surveillance(&request()).unwrap();
        receipt.queue_digest = hash("tampered-queue");
        assert!(receipt.validate().is_err());
    }
    #[test]
    fn tampered_checkpoint_predecessor_is_rejected() {
        let mut receipt = run_throughput_evidence_surveillance(&request()).unwrap();
        receipt.previous_checkpoint = Some(hash("different-predecessor"));
        assert!(receipt.validate().is_err());
    }
    #[test]
    fn tampered_artifact_payload_is_rejected() {
        let mut receipt = run_throughput_evidence_surveillance(&request()).unwrap();
        receipt.artifact.content_hash = hash("tampered-payload");
        assert!(receipt.validate().is_err());
    }
    #[test]
    fn checkpoint_digest_is_stable() {
        let first = run_throughput_evidence_surveillance(&request()).unwrap();
        let second = run_throughput_evidence_surveillance(&request()).unwrap();
        assert_eq!(first.checkpoint_digest, second.checkpoint_digest);
    }

    #[test]
    fn reordered_observations_share_the_same_retained_input_identity() {
        let mut reordered = request();
        reordered.observations.reverse();
        let first = run_throughput_evidence_surveillance(&request()).unwrap();
        let second = run_throughput_evidence_surveillance(&reordered).unwrap();
        assert_eq!(first.input_digest, second.input_digest);
        assert_eq!(first.checkpoint_digest, second.checkpoint_digest);
    }

    #[test]
    fn receipt_rejects_tampered_retained_observation() {
        let mut receipt = run_throughput_evidence_surveillance(&request()).unwrap();
        receipt.input.observations[0].sequence = 99;
        let error = receipt.validate().unwrap_err();
        assert!(error.to_string().contains("retained input digest mismatch"));
    }
}
