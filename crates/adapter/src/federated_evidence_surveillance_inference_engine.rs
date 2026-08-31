//! Federated continual evidence-surveillance inference engine.
//!
//! Atlas feature: `AFA-adapter-P01-F04`. Only signed, permitted aggregate artifacts cross the
//! federation boundary; peer failures remain explicit and raw observations stay local.

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

pub const FEATURE_ID: &str = "AFA-adapter-P01-F04";
pub const CONTRACT_VERSION: &str = "adapter-federated-evidence-surveillance-inference-engine/1.0";
pub const INPUT_SCHEMA: &str = "EvidenceFeed4@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedEvidenceSet1@1";
const MAX_TEXT_BYTES: usize = 512;
const MAX_ITEMS: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedEvidenceObservation {
    pub peer_id: String,
    pub institution_id: String,
    pub source_id: String,
    pub study_id: String,
    pub semantic_profile: String,
    pub artifact_kind: String,
    pub digest: Option<ContentHash>,
    pub availability: EvidenceAvailability,
    pub evidence_state: EvidenceState,
    pub relevance_score: u16,
    pub signed: bool,
    pub permitted_artifact: bool,
    pub aggregate_only: bool,
    pub raw_data_local: bool,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedEvidenceSurveillanceRequest {
    pub request_id: String,
    pub federation_id: String,
    pub purpose: String,
    pub endpoint: String,
    pub semantic_profile: String,
    pub allowed_artifacts: Vec<String>,
    pub min_peer_quorum: usize,
    pub observations: Vec<FederatedEvidenceObservation>,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub replay_identity: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FederatedEvidenceSurveillanceDisposition {
    Completed,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedQualifiedEvidenceSet {
    pub schema_version: String,
    pub set_id: String,
    pub federation_id: String,
    pub purpose: String,
    pub peer_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub selected_digests: Vec<ContentHash>,
    pub aggregate_order: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_order: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederatedEvidenceSurveillanceReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub input: FederatedEvidenceSurveillanceRequest,
    pub input_digest: ContentHash,
    pub request_id: String,
    pub federation_id: String,
    pub purpose: String,
    pub endpoint: String,
    pub semantic_profile: String,
    pub allowed_artifacts: Vec<String>,
    pub min_peer_quorum: usize,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub disposition: FederatedEvidenceSurveillanceDisposition,
    pub peer_order: Vec<String>,
    pub candidate_order: Vec<String>,
    pub ranked_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub denied_order: Vec<String>,
    pub aggregate_order: Vec<String>,
    pub federation_digest: ContentHash,
    pub envelope_digest: ContentHash,
    pub evidence_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub qualified_set: FederatedQualifiedEvidenceSet,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederatedEvidenceSurveillanceError {
    #[error("invalid federated evidence request: {0}")]
    Invalid(String),
    #[error("federated evidence artifact failed: {0}")]
    Artifact(String),
}
fn validate_text(field: &str, value: &str) -> Result<(), FederatedEvidenceSurveillanceError> {
    if value.is_empty() || value.trim() != value {
        return Err(FederatedEvidenceSurveillanceError::Invalid(format!(
            "{field} must be non-empty and trimmed"
        )));
    }
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(FederatedEvidenceSurveillanceError::Invalid(format!(
            "{field} is outside its bounded text contract"
        )));
    }
    Ok(())
}

fn validate_unique_strings(
    field: &str,
    values: &[String],
) -> Result<(), FederatedEvidenceSurveillanceError> {
    if values.len() > MAX_ITEMS {
        return Err(FederatedEvidenceSurveillanceError::Invalid(format!(
            "{field} exceeds its item bound"
        )));
    }
    let mut unique = BTreeSet::new();
    for value in values {
        validate_text(field, value)?;
        if !unique.insert(value) {
            return Err(FederatedEvidenceSurveillanceError::Invalid(format!(
                "{field} contains duplicate values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_strings(
    field: &str,
    values: &[String],
) -> Result<(), FederatedEvidenceSurveillanceError> {
    validate_unique_strings(field, values)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(FederatedEvidenceSurveillanceError::Invalid(format!(
            "{field} ordering is not canonical"
        )));
    }
    Ok(())
}

fn validate_digest(
    field: &str,
    digest: &ContentHash,
) -> Result<(), FederatedEvidenceSurveillanceError> {
    if digest.as_str().len() != 64
        || !digest
            .as_str()
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(FederatedEvidenceSurveillanceError::Invalid(format!(
            "{field} must be a 64-character hex digest"
        )));
    }
    Ok(())
}

pub(crate) fn canonical_federated_evidence_surveillance_request(
    request: &FederatedEvidenceSurveillanceRequest,
) -> FederatedEvidenceSurveillanceRequest {
    let mut canonical = request.clone();
    canonical.observations.sort_by(|left, right| {
        right
            .relevance_score
            .cmp(&left.relevance_score)
            .then_with(|| left.peer_id.cmp(&right.peer_id))
            .then_with(|| left.source_id.cmp(&right.source_id))
    });
    canonical
}

fn federated_evidence_input_digest(
    request: &FederatedEvidenceSurveillanceRequest,
) -> Result<ContentHash, FederatedEvidenceSurveillanceError> {
    let canonical = canonical_federated_evidence_surveillance_request(request);
    let value = serde_json::to_value(canonical)
        .map_err(|error| FederatedEvidenceSurveillanceError::Artifact(error.to_string()))?;
    ContentHash::of_value(&value)
        .map_err(|error| FederatedEvidenceSurveillanceError::Artifact(error.to_string()))
}

impl FederatedEvidenceSurveillanceReceipt {
    pub fn validate(&self) -> Result<(), FederatedEvidenceSurveillanceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.federation_id.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.endpoint.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.allowed_artifacts.is_empty()
            || self.min_peer_quorum == 0
            || self.min_peer_quorum > MAX_ITEMS
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
            || self.qualified_set.federation_id != self.federation_id
        {
            return Err(FederatedEvidenceSurveillanceError::Invalid("federated identity, locality, candidates, effects, or qualified-set linkage is incomplete".into()));
        }
        validate_text("request_id", &self.request_id)?;
        validate_text("federation_id", &self.federation_id)?;
        validate_text("purpose", &self.purpose)?;
        validate_text("endpoint", &self.endpoint)?;
        validate_text("semantic_profile", &self.semantic_profile)?;
        validate_text("boundary", &self.boundary)?;
        validate_sorted_strings("allowed_artifacts", &self.allowed_artifacts)?;
        validate_sorted_strings("peer_order", &self.peer_order)?;
        validate_sorted_strings("candidate_order", &self.candidate_order)?;
        validate_unique_strings("ranked_order", &self.ranked_order)?;
        validate_sorted_strings("selected_order", &self.selected_order)?;
        validate_sorted_strings("unresolved_order", &self.unresolved_order)?;
        validate_sorted_strings("denied_order", &self.denied_order)?;
        validate_sorted_strings("aggregate_order", &self.aggregate_order)?;
        validate_sorted_strings("omissions", &self.omissions)?;
        validate_sorted_strings("uncertainty", &self.uncertainty)?;
        validate_sorted_strings("negative_evidence", &self.negative_evidence)?;
        validate_sorted_strings("effect_receipts", &self.effect_receipts)?;
        validate_sorted_strings("qualified_set.peer_order", &self.qualified_set.peer_order)?;
        validate_sorted_strings(
            "qualified_set.selected_order",
            &self.qualified_set.selected_order,
        )?;
        validate_sorted_strings(
            "qualified_set.aggregate_order",
            &self.qualified_set.aggregate_order,
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
            return Err(FederatedEvidenceSurveillanceError::Invalid(
                "federated ranking must cover candidates exactly".into(),
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
            || self.aggregate_order != self.selected_order
            || self.qualified_set.aggregate_order != self.aggregate_order
        {
            return Err(FederatedEvidenceSurveillanceError::Invalid(
                "federated states do not partition candidates".into(),
            ));
        }
        if self.qualified_set.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.qualified_set.set_id
                != format!("qualified-federated-evidence:{}", self.federation_id)
            || self.qualified_set.federation_id != self.federation_id
            || self.qualified_set.purpose != self.purpose
            || self.qualified_set.peer_order != self.peer_order
            || self.qualified_set.selected_digests.len() != self.selected_order.len()
            || self.qualified_set.omissions != self.omissions
            || self.qualified_set.uncertainty != self.uncertainty
            || self.qualified_set.negative_order != self.negative_evidence
            || self.qualified_set.boundary != PRECLINICAL_BOUNDARY
        {
            return Err(FederatedEvidenceSurveillanceError::Invalid(
                "federated qualified evidence set is not bound to the receipt".into(),
            ));
        }
        for digest in &self.qualified_set.selected_digests {
            validate_digest("qualified_set.selected_digest", digest)?;
        }
        for digest in [
            &self.federation_digest,
            &self.envelope_digest,
            &self.evidence_digest,
            &self.provenance_digest,
            &self.replay_identity,
            &self.artifact.content_hash,
        ] {
            validate_digest("federated receipt digest", digest)?;
        }
        let quorum_incomplete = self.peer_order.len() < self.min_peer_quorum;
        let should_block = !self.policy_allow || !self.protected_closure || !self.raw_data_local;
        if (self.disposition == FederatedEvidenceSurveillanceDisposition::Blocked) != should_block {
            return Err(FederatedEvidenceSurveillanceError::Invalid(
                "federated disposition does not match policy, closure, and locality gates".into(),
            ));
        }
        if self.disposition == FederatedEvidenceSurveillanceDisposition::Completed
            && (quorum_incomplete
                || !self.unresolved_order.is_empty()
                || !self.denied_order.is_empty())
        {
            return Err(FederatedEvidenceSurveillanceError::Invalid(
                "completed federated surveillance cannot retain unresolved, denied, or quorum-incomplete states".into(),
            ));
        }
        if matches!(
            self.disposition,
            FederatedEvidenceSurveillanceDisposition::Unknown
                | FederatedEvidenceSurveillanceDisposition::Blocked
        ) && !self.selected_order.is_empty()
        {
            return Err(FederatedEvidenceSurveillanceError::Invalid(
                "unknown or blocked federated surveillance cannot retain selected evidence".into(),
            ));
        }
        let expected_effect = if should_block {
            vec!["block:unsafe-release".to_string()]
        } else {
            vec![format!(
                "exchange:aggregate-evidence:{}",
                self.federation_id
            )]
        };
        if self.effect_receipts != expected_effect {
            return Err(FederatedEvidenceSurveillanceError::Invalid(
                "federated effect does not match its release state".into(),
            ));
        }
        let expected_federation = ContentHash::of_value(&json!({
            "federation_id": self.federation_id,
            "purpose": self.purpose,
            "endpoint": self.endpoint,
            "peer_order": self.peer_order,
            "semantic_profile": self.semantic_profile,
        }))
        .map_err(|error| FederatedEvidenceSurveillanceError::Artifact(error.to_string()))?;
        if self.federation_digest != expected_federation {
            return Err(FederatedEvidenceSurveillanceError::Invalid(
                "federation digest does not match purpose and peer scope".into(),
            ));
        }
        let expected_envelope = ContentHash::of_value(&json!({
            "aggregate_order": self.aggregate_order,
            "allowed_artifacts": self.allowed_artifacts,
            "raw_data_local": self.raw_data_local,
            "aggregate_only": true,
            "federation_digest": self.federation_digest,
        }))
        .map_err(|error| FederatedEvidenceSurveillanceError::Artifact(error.to_string()))?;
        if self.envelope_digest != expected_envelope {
            return Err(FederatedEvidenceSurveillanceError::Invalid(
                "federated envelope digest does not match aggregate-only policy".into(),
            ));
        }
        let expected_evidence = ContentHash::of_value(&json!({
            "candidate_order": self.candidate_order,
            "selected_order": self.selected_order,
            "unresolved_order": self.unresolved_order,
            "denied_order": self.denied_order,
        }))
        .map_err(|error| FederatedEvidenceSurveillanceError::Artifact(error.to_string()))?;
        if self.evidence_digest != expected_evidence {
            return Err(FederatedEvidenceSurveillanceError::Invalid(
                "federated evidence digest does not match classified states".into(),
            ));
        }
        let expected_provenance = ContentHash::of_value(&json!({
            "request_id": self.request_id,
            "replay_identity": self.replay_identity,
            "envelope_digest": self.envelope_digest,
            "evidence_digest": self.evidence_digest,
        }))
        .map_err(|error| FederatedEvidenceSurveillanceError::Artifact(error.to_string()))?;
        if self.provenance_digest != expected_provenance {
            return Err(FederatedEvidenceSurveillanceError::Invalid(
                "federated provenance digest does not match request identity".into(),
            ));
        }
        if self.artifact.artifact_id != self.qualified_set.set_id
            || self.artifact.content_type
                != "application/vnd.aurora.qualified-federated-evidence-set+json"
            || !self.artifact.semantic_loss.is_empty()
            || !self.artifact.provenance.is_empty()
        {
            return Err(FederatedEvidenceSurveillanceError::Artifact(
                "federated artifact is not bound to the qualified evidence set".into(),
            ));
        }
        let qualified_payload = serde_json::to_value(&self.qualified_set)
            .map_err(|error| FederatedEvidenceSurveillanceError::Artifact(error.to_string()))?;
        self.artifact
            .verify_payload(&qualified_payload)
            .map_err(|error| FederatedEvidenceSurveillanceError::Artifact(error.to_string()))?;
        self.artifact
            .validate_metadata()
            .map_err(|error| FederatedEvidenceSurveillanceError::Artifact(error.to_string()))?;
        if self.input_digest != federated_evidence_input_digest(&self.input)? {
            return Err(FederatedEvidenceSurveillanceError::Invalid(
                "federated evidence surveillance retained input digest mismatch".into(),
            ));
        }
        let expected = build_federated_evidence_surveillance(&self.input)?;
        if self != &expected {
            return Err(FederatedEvidenceSurveillanceError::Invalid(
                "federated evidence surveillance receipt does not match its retained input".into(),
            ));
        }
        Ok(())
    }
}

pub fn federated_evidence_surveillance_inference_engine_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "adapter".into(), consumers: ["preclinical researcher".into(), "federation steward".into()].into(), behavior: "qualifies signed purpose-bound aggregate evidence across institutions without moving raw observations".into(), value: "enables continual consortium discovery while preserving locality, signer, permission, semantic, and quorum gates".into(), inputs: vec![TypedPort { name: "evidence_feed".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "qualified_evidence_set".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ReadLocalData, Effect::FederationExport, Effect::WriteLocalArtifact].into(), permissions: ["read:local-research-artifacts".into(), "export:permitted-aggregate-artifacts".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "W3C PROV-O".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }], authority_requirements: Vec::new(), autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

pub fn run_federated_evidence_surveillance(
    request: &FederatedEvidenceSurveillanceRequest,
) -> Result<FederatedEvidenceSurveillanceReceipt, FederatedEvidenceSurveillanceError> {
    let receipt = build_federated_evidence_surveillance(request)?;
    receipt.validate()?;
    Ok(receipt)
}

fn build_federated_evidence_surveillance(
    request: &FederatedEvidenceSurveillanceRequest,
) -> Result<FederatedEvidenceSurveillanceReceipt, FederatedEvidenceSurveillanceError> {
    let canonical_request = canonical_federated_evidence_surveillance_request(request);
    let request = &canonical_request;
    if request.request_id.trim().is_empty()
        || request.federation_id.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.endpoint.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.min_peer_quorum == 0
        || request.observations.is_empty()
        || request.allowed_artifacts.is_empty()
        || request.min_peer_quorum > MAX_ITEMS
        || request.allowed_artifacts.len() > MAX_ITEMS
        || request.observations.len() > MAX_ITEMS
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
    {
        return Err(FederatedEvidenceSurveillanceError::Invalid("federated identity, purpose, endpoint, quorum, observations, allow-list, replay, locality, or boundary is invalid".into()));
    }
    validate_text("request_id", &request.request_id)?;
    validate_text("federation_id", &request.federation_id)?;
    validate_text("purpose", &request.purpose)?;
    validate_text("endpoint", &request.endpoint)?;
    validate_text("semantic_profile", &request.semantic_profile)?;
    validate_text("boundary", &request.boundary)?;
    validate_sorted_strings("allowed_artifacts", &request.allowed_artifacts)?;
    validate_digest("replay_identity", &request.replay_identity)?;
    let mut observation_keys = BTreeSet::new();
    for item in &request.observations {
        validate_text("observation.peer_id", &item.peer_id)?;
        validate_text("observation.institution_id", &item.institution_id)?;
        validate_text("observation.source_id", &item.source_id)?;
        validate_text("observation.study_id", &item.study_id)?;
        validate_text("observation.semantic_profile", &item.semantic_profile)?;
        validate_text("observation.artifact_kind", &item.artifact_kind)?;
        let observation_key = format!(
            "{}::{}::{}",
            item.peer_id, item.institution_id, item.source_id
        );
        if !observation_keys.insert(observation_key) {
            return Err(FederatedEvidenceSurveillanceError::Invalid(
                "federated observation keys must be unique".into(),
            ));
        }
        if let Some(digest) = &item.digest {
            validate_digest("observation.digest", digest)?;
        }
    }
    let mut observations = request.observations.clone();
    observations.sort_by(|left, right| {
        right
            .relevance_score
            .cmp(&left.relevance_score)
            .then_with(|| left.peer_id.cmp(&right.peer_id))
            .then_with(|| left.source_id.cmp(&right.source_id))
    });
    let key = |item: &FederatedEvidenceObservation| {
        format!(
            "{}::{}::{}",
            item.peer_id, item.institution_id, item.source_id
        )
    };
    let ranked_order = observations.iter().map(key).collect::<Vec<_>>();
    let mut candidate_order = ranked_order.clone();
    candidate_order.sort();
    if candidate_order.windows(2).any(|pair| pair[0] == pair[1])
        || observations
            .iter()
            .any(|item| item.peer_id.trim().is_empty() || item.institution_id.trim().is_empty())
    {
        return Err(FederatedEvidenceSurveillanceError::Invalid(
            "federated observation identities must be unique and non-empty".into(),
        ));
    }
    let mut selected = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut denied = BTreeSet::new();
    let mut peers = BTreeSet::new();
    let mut aggregate = BTreeSet::new();
    let mut digest_map = BTreeMap::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    for item in &observations {
        let item_key = key(item);
        if !request.policy_allow
            || !request.protected_closure
            || !request.raw_data_local
            || !item.raw_data_local
        {
            denied.insert(item_key.clone());
            omissions.insert(format!("evidence:{}:policy-closure-locality", item_key));
        } else if !item.signed {
            denied.insert(item_key.clone());
            omissions.insert(format!("evidence:{}:signature-missing", item_key));
        } else if !item.permitted_artifact
            || !request.allowed_artifacts.contains(&item.artifact_kind)
        {
            denied.insert(item_key.clone());
            omissions.insert(format!("evidence:{}:artifact-not-permitted", item_key));
        } else if !item.aggregate_only {
            denied.insert(item_key.clone());
            negative.insert(format!(
                "evidence:{}:raw-observation-export-denied",
                item_key
            ));
        } else if item.semantic_profile != request.semantic_profile {
            denied.insert(item_key.clone());
            omissions.insert(format!("evidence:{}:semantic-profile-mismatch", item_key));
            negative.insert(format!("evidence:{}:incomparable", item_key));
        } else if item.availability != EvidenceAvailability::Available {
            unresolved.insert(item_key.clone());
            omissions.insert(format!(
                "evidence:{}:availability-{:?}",
                item_key, item.availability
            ));
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
            selected.insert(item_key.clone());
            peers.insert(item.peer_id.clone());
            aggregate.insert(item_key.clone());
            if let Some(digest) = item.digest.clone() {
                digest_map.insert(item_key, digest);
            } else {
                return Err(FederatedEvidenceSurveillanceError::Invalid(
                    "selected federated evidence must have a content digest".into(),
                ));
            }
            if item.negative_result {
                negative.insert(format!("evidence:{}:negative-result", key(item)));
            }
        }
    }
    if peers.len() < request.min_peer_quorum {
        omissions.insert(format!(
            "federation:quorum-incomplete:{}<{}",
            peers.len(),
            request.min_peer_quorum
        ));
        uncertainty.insert("federation:quorum-unresolved".into());
        for item_key in selected.clone() {
            selected.remove(&item_key);
            aggregate.remove(&item_key);
            unresolved.insert(item_key.clone());
            omissions.insert(format!("evidence:{}:quorum-unresolved", item_key));
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
    let disposition = if !request.policy_allow
        || !request.protected_closure
        || !request.raw_data_local
    {
        FederatedEvidenceSurveillanceDisposition::Blocked
    } else if peers.len() < request.min_peer_quorum {
        FederatedEvidenceSurveillanceDisposition::Partial
    } else if selected.is_empty() {
        FederatedEvidenceSurveillanceDisposition::Unknown
    } else if !unresolved.is_empty() || !denied.is_empty() || peers.len() < request.min_peer_quorum
    {
        FederatedEvidenceSurveillanceDisposition::Partial
    } else {
        FederatedEvidenceSurveillanceDisposition::Completed
    };
    let peer_order = peers.iter().cloned().collect::<Vec<_>>();
    let selected_order = selected.iter().cloned().collect::<Vec<_>>();
    let aggregate_order = aggregate.iter().cloned().collect::<Vec<_>>();
    let unresolved_order = unresolved.iter().cloned().collect::<Vec<_>>();
    let denied_order = denied.iter().cloned().collect::<Vec<_>>();
    let omissions_vec = omissions.iter().cloned().collect::<Vec<_>>();
    let uncertainty_vec = uncertainty.iter().cloned().collect::<Vec<_>>();
    let negative_vec = negative.iter().cloned().collect::<Vec<_>>();
    let selected_digests = selected_order
        .iter()
        .filter_map(|item| digest_map.get(item).cloned())
        .collect::<Vec<_>>();
    let federation_digest = ContentHash::of_value(&json!({"federation_id": request.federation_id, "purpose": request.purpose, "endpoint": request.endpoint, "peer_order": peer_order.clone(), "semantic_profile": request.semantic_profile})).map_err(|error| FederatedEvidenceSurveillanceError::Artifact(error.to_string()))?;
    let envelope_digest = ContentHash::of_value(&json!({"aggregate_order": aggregate_order.clone(), "allowed_artifacts": request.allowed_artifacts.clone(), "raw_data_local": request.raw_data_local, "aggregate_only": true, "federation_digest": federation_digest})).map_err(|error| FederatedEvidenceSurveillanceError::Artifact(error.to_string()))?;
    let evidence_digest = ContentHash::of_value(&json!({"candidate_order": candidate_order.clone(), "selected_order": selected_order.clone(), "unresolved_order": unresolved_order.clone(), "denied_order": denied_order.clone()})).map_err(|error| FederatedEvidenceSurveillanceError::Artifact(error.to_string()))?;
    let provenance_digest = ContentHash::of_value(&json!({"request_id": request.request_id, "replay_identity": request.replay_identity, "envelope_digest": envelope_digest, "evidence_digest": evidence_digest})).map_err(|error| FederatedEvidenceSurveillanceError::Artifact(error.to_string()))?;
    let qualified_set = FederatedQualifiedEvidenceSet {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        set_id: format!("qualified-federated-evidence:{}", request.federation_id),
        federation_id: request.federation_id.clone(),
        purpose: request.purpose.clone(),
        peer_order: peer_order.clone(),
        selected_order: selected_order.clone(),
        selected_digests,
        aggregate_order: aggregate_order.clone(),
        omissions: omissions_vec.clone(),
        uncertainty: uncertainty_vec.clone(),
        negative_order: negative_vec.clone(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let payload = serde_json::to_value(&qualified_set)
        .map_err(|error| FederatedEvidenceSurveillanceError::Artifact(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        qualified_set.set_id.clone(),
        "application/vnd.aurora.qualified-federated-evidence-set+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| FederatedEvidenceSurveillanceError::Artifact(error.to_string()))?;
    let input_digest = federated_evidence_input_digest(request)?;
    let receipt = FederatedEvidenceSurveillanceReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        input: canonical_request.clone(),
        input_digest,
        request_id: request.request_id.clone(),
        federation_id: request.federation_id.clone(),
        purpose: request.purpose.clone(),
        endpoint: request.endpoint.clone(),
        semantic_profile: request.semantic_profile.clone(),
        allowed_artifacts: request.allowed_artifacts.clone(),
        min_peer_quorum: request.min_peer_quorum,
        policy_allow: request.policy_allow,
        protected_closure: request.protected_closure,
        disposition,
        peer_order,
        candidate_order,
        ranked_order,
        selected_order,
        unresolved_order,
        denied_order,
        aggregate_order,
        federation_digest,
        envelope_digest,
        evidence_digest,
        provenance_digest,
        replay_identity: request.replay_identity.clone(),
        omissions: omissions_vec,
        uncertainty: uncertainty_vec,
        negative_evidence: negative_vec,
        effect_receipts: if disposition == FederatedEvidenceSurveillanceDisposition::Blocked {
            vec!["block:unsafe-release".into()]
        } else {
            vec![format!(
                "exchange:aggregate-evidence:{}",
                request.federation_id
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
    fn request() -> FederatedEvidenceSurveillanceRequest {
        let digest = hash("federated-evidence");
        let observation = |peer: &str, source: &str| FederatedEvidenceObservation {
            peer_id: peer.into(),
            institution_id: format!("institution:{peer}"),
            source_id: source.into(),
            study_id: "study:one".into(),
            semantic_profile: "profile:v1".into(),
            artifact_kind: "aggregate-evidence".into(),
            digest: Some(digest.clone()),
            availability: EvidenceAvailability::Available,
            evidence_state: EvidenceState::Supported,
            relevance_score: 90,
            signed: true,
            permitted_artifact: true,
            aggregate_only: true,
            raw_data_local: true,
            negative_result: false,
        };
        FederatedEvidenceSurveillanceRequest {
            request_id: "request:federated".into(),
            federation_id: "federation:one".into(),
            purpose: "compare preclinical evidence".into(),
            endpoint: "local://federation".into(),
            semantic_profile: "profile:v1".into(),
            allowed_artifacts: vec!["aggregate-evidence".into()],
            min_peer_quorum: 2,
            observations: vec![
                observation("peer:a", "source:a"),
                observation("peer:b", "source:b"),
            ],
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
            federated_evidence_surveillance_inference_engine_manifest().autonomy_tier,
            AutonomyTier::A1
        );
    }
    #[test]
    fn quorum_completes() {
        assert_eq!(
            run_federated_evidence_surveillance(&request())
                .unwrap()
                .disposition,
            FederatedEvidenceSurveillanceDisposition::Completed
        );
    }
    #[test]
    fn quorum_gap_is_partial() {
        let mut value = request();
        value.min_peer_quorum = 3;
        let receipt = run_federated_evidence_surveillance(&value).unwrap();
        assert_eq!(
            receipt.disposition,
            FederatedEvidenceSurveillanceDisposition::Partial
        );
        assert!(receipt.selected_order.is_empty());
        assert!(receipt.aggregate_order.is_empty());
    }
    #[test]
    fn unsigned_is_denied() {
        let mut value = request();
        value.observations[0].signed = false;
        assert!(run_federated_evidence_surveillance(&value)
            .unwrap()
            .denied_order
            .iter()
            .any(|item| item.contains("peer:a")));
    }
    #[test]
    fn raw_export_is_denied() {
        let mut value = request();
        value.observations[0].aggregate_only = false;
        let receipt = run_federated_evidence_surveillance(&value).unwrap();
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|item| item.contains("raw-observation-export-denied")));
    }
    #[test]
    fn unknown_is_not_asserted() {
        let mut value = request();
        value.observations[0].evidence_state = EvidenceState::Unknown;
        assert!(run_federated_evidence_surveillance(&value)
            .unwrap()
            .uncertainty
            .iter()
            .any(|item| item.contains("unknown-not-asserted")));
    }
    #[test]
    fn policy_blocks() {
        let mut value = request();
        value.policy_allow = false;
        let receipt = run_federated_evidence_surveillance(&value).unwrap();
        assert_eq!(receipt.effect_receipts, vec!["block:unsafe-release"]);
        assert!(receipt.selected_order.is_empty());
    }
    #[test]
    fn duplicate_federated_key_is_rejected() {
        let mut value = request();
        value.observations.push(value.observations[0].clone());
        assert!(run_federated_evidence_surveillance(&value).is_err());
    }
    #[test]
    fn tampered_federation_digest_is_rejected() {
        let mut receipt = run_federated_evidence_surveillance(&request()).unwrap();
        receipt.federation_digest = hash("tampered-federation");
        assert!(receipt.validate().is_err());
    }
    #[test]
    fn tampered_envelope_digest_is_rejected() {
        let mut receipt = run_federated_evidence_surveillance(&request()).unwrap();
        receipt.envelope_digest = hash("tampered-envelope");
        assert!(receipt.validate().is_err());
    }
    #[test]
    fn tampered_artifact_payload_is_rejected() {
        let mut receipt = run_federated_evidence_surveillance(&request()).unwrap();
        receipt.artifact.content_hash = hash("tampered-payload");
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn tampered_retained_request_is_rejected() {
        let mut receipt = run_federated_evidence_surveillance(&request()).unwrap();
        receipt.input.endpoint = "local://tampered".into();
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn replay_is_stable() {
        let first = run_federated_evidence_surveillance(&request()).unwrap();
        let second = run_federated_evidence_surveillance(&request()).unwrap();
        assert_eq!(first.envelope_digest, second.envelope_digest);
    }

    #[test]
    fn reordered_observations_share_the_same_retained_input_identity() {
        let mut reordered = request();
        reordered.observations.reverse();
        let first = run_federated_evidence_surveillance(&request()).unwrap();
        let second = run_federated_evidence_surveillance(&reordered).unwrap();
        assert_eq!(first.input_digest, second.input_digest);
        assert_eq!(first.envelope_digest, second.envelope_digest);
        assert_eq!(first, second);
    }
}
