//! Local evidence-surveillance copilot for one preclinical study.
//!
//! Atlas feature: `AFA-adapter-P01-F09`.
//!
//! This capability turns a bounded, institution-local evidence feed into a deterministic
//! qualified set.  It never treats a missing, stale, protected, or contradictory item as a
//! usable source; every such item is preserved as an omission or uncertainty.  The copilot is
//! an A1 local computation: it can rank and qualify metadata, but it cannot publish raw bytes,
//! make a clinical decision, or upgrade an unknown evidence state.

use bioprism_foundation::{
    Effect, EvidenceAvailability, EvidenceState, PolicyDecision, ProvenanceLink,
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P01-F09";
pub const CONTRACT_VERSION: &str = "evidence-surveillance-copilot/1.0";
const MAX_TEXT_BYTES: usize = 512;
const MAX_FEED_ITEMS: usize = 8192;
const MAX_SOURCE_IDS: usize = 8192;
const MAX_NOTE_ITEMS: usize = 8192;

/// One source advertised by the local evidence feed.  The payload itself never crosses this
/// contract; only its digest and qualification metadata do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceFeedItem {
    pub source_id: String,
    pub source_type: String,
    pub locator: String,
    pub digest: Option<ContentHash>,
    pub availability: EvidenceAvailability,
    pub published_at: String,
    pub relevance_score: u16,
    pub negative_result: bool,
}

/// Typed input for a single-study evidence-surveillance run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceFeedRequest {
    pub request_id: String,
    pub study_id: String,
    pub intent: String,
    pub required_source_ids: Vec<String>,
    pub feed: Vec<EvidenceFeedItem>,
    pub policy_decision: PolicyDecision,
    pub protected_closure_satisfied: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceSurveillanceDisposition {
    Passed,
    Blocked,
    Unknown,
}

/// A machine-readable authorization/result for each effect considered by the copilot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectReceipt {
    pub effect: Effect,
    pub authorized: bool,
    pub reason: String,
    pub receipt_digest: ContentHash,
}

/// The typed product emitted by the surveillance run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualifiedEvidenceSet {
    pub schema_version: String,
    pub set_id: String,
    pub study_id: String,
    pub intent: String,
    pub selected_source_ids: Vec<String>,
    pub selected_source_scores: Vec<u16>,
    pub selected_source_digests: Vec<Option<ContentHash>>,
    pub evidence_state: EvidenceState,
    pub negative_source_ids: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub ordering_rule: String,
    pub boundary: String,
}

impl QualifiedEvidenceSet {
    pub fn validate(&self) -> Result<(), EvidenceSurveillanceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.boundary != PRECLINICAL_BOUNDARY
        {
            return Err(EvidenceSurveillanceError::InvalidField(
                "qualified evidence identity, ordering, or boundary is incomplete".into(),
            ));
        }
        validate_text("set_id", &self.set_id)?;
        validate_text("study_id", &self.study_id)?;
        validate_text("intent", &self.intent)?;
        validate_text("ordering_rule", &self.ordering_rule)?;
        if self.ordering_rule != "relevance_score descending, source_id ascending" {
            return Err(EvidenceSurveillanceError::InvalidField(
                "qualified evidence ordering rule is not the declared ranking contract".into(),
            ));
        }
        if self.selected_source_ids.len() != self.selected_source_scores.len()
            || self.selected_source_ids.len() != self.selected_source_digests.len()
            || self.selected_source_ids.len() > MAX_SOURCE_IDS
        {
            return Err(EvidenceSurveillanceError::InvalidField(
                "qualified evidence sources are not deterministically ordered".into(),
            ));
        }
        validate_unique_ids(&self.selected_source_ids, "selected_source_ids")?;
        if self
            .selected_source_ids
            .windows(2)
            .zip(self.selected_source_scores.windows(2))
            .any(|(ids, scores)| {
                scores[0] < scores[1] || (scores[0] == scores[1] && ids[0] >= ids[1])
            })
        {
            return Err(EvidenceSurveillanceError::InvalidField(
                "qualified evidence ranking is not relevance-descending with source-id tie-breaks"
                    .into(),
            ));
        }
        validate_sorted_ids(&self.negative_source_ids, "negative_source_ids")?;
        if self
            .negative_source_ids
            .iter()
            .any(|source| !self.selected_source_ids.contains(source))
        {
            return Err(EvidenceSurveillanceError::InvalidField(
                "negative sources must be selected sources".into(),
            ));
        }
        validate_sorted_notes(&self.omissions, "qualified_set.omissions")?;
        validate_sorted_notes(&self.uncertainty, "qualified_set.uncertainty")?;
        if self.selected_source_digests.iter().any(Option::is_none) {
            return Err(EvidenceSurveillanceError::InvalidField(
                "qualified evidence sources must carry content digests".into(),
            ));
        }
        for digest in self.selected_source_digests.iter().flatten() {
            if *digest == ContentHash::of_bytes(b"") {
                return Err(EvidenceSurveillanceError::InvalidField(
                    "selected source digests cannot be empty".into(),
                ));
            }
        }
        if !matches!(
            self.evidence_state,
            EvidenceState::Supported | EvidenceState::Unknown
        ) {
            return Err(EvidenceSurveillanceError::InvalidField(
                "qualified evidence state is outside the surveillance contract".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceSurveillanceReceipt {
    pub schema_version: String,
    pub feature_id: String,
    pub contract_version: String,
    pub input: EvidenceFeedRequest,
    pub input_digest: ContentHash,
    pub request_id: String,
    pub study_id: String,
    pub intent: String,
    pub selected_source_ids: Vec<String>,
    pub disposition: EvidenceSurveillanceDisposition,
    pub qualified_set: QualifiedEvidenceSet,
    pub effect_receipts: Vec<EffectReceipt>,
    pub checks: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub boundary: String,
}

impl EvidenceSurveillanceReceipt {
    pub fn validate(&self) -> Result<(), EvidenceSurveillanceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.feature_id != FEATURE_ID
            || self.contract_version != CONTRACT_VERSION
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.checks.is_empty()
            || self.selected_source_ids != self.qualified_set.selected_source_ids
            || self.qualified_set.study_id != self.study_id
            || self.qualified_set.intent != self.intent
            || self.omissions != self.qualified_set.omissions
            || self.uncertainty != self.qualified_set.uncertainty
        {
            return Err(EvidenceSurveillanceError::InvalidField(
                "evidence surveillance identity, effects, checks, or qualified-set linkage is incomplete".into(),
            ));
        }
        validate_text("request_id", &self.request_id)?;
        validate_text("study_id", &self.study_id)?;
        validate_text("intent", &self.intent)?;
        validate_sorted_notes(&self.checks, "checks")?;
        if self.effect_receipts.len() != 1 {
            return Err(EvidenceSurveillanceError::InvalidField(
                "evidence surveillance requires exactly one local-feed effect receipt".into(),
            ));
        }
        let effect = &self.effect_receipts[0];
        if effect.effect != Effect::ReadLocalData {
            return Err(EvidenceSurveillanceError::InvalidField(
                "evidence surveillance may only read the local feed".into(),
            ));
        }
        validate_text("effect_receipt.reason", &effect.reason)?;
        let blocked = self.disposition == EvidenceSurveillanceDisposition::Blocked;
        if effect.authorized == blocked
            || effect.reason
                != if blocked {
                    "policy or locality gate denied local feed read"
                } else {
                    "local evidence feed read is policy-authorized"
                }
        {
            return Err(EvidenceSurveillanceError::InvalidField(
                "feed-read authorization does not match disposition".into(),
            ));
        }
        let effect_payload = json!({
            "request_id": self.request_id,
            "effect": effect.effect,
            "authorized": effect.authorized,
        });
        let expected_effect_digest = ContentHash::of_value(&effect_payload)
            .map_err(|error| EvidenceSurveillanceError::Serialization(error.to_string()))?;
        if effect.receipt_digest != expected_effect_digest {
            return Err(EvidenceSurveillanceError::InvalidField(
                "feed-read effect digest does not match its authorization".into(),
            ));
        }
        let expected_state = if self.disposition == EvidenceSurveillanceDisposition::Passed {
            EvidenceState::Supported
        } else {
            EvidenceState::Unknown
        };
        if self.qualified_set.evidence_state != expected_state {
            return Err(EvidenceSurveillanceError::InvalidField(
                "qualified evidence state does not match disposition".into(),
            ));
        }
        if self.disposition == EvidenceSurveillanceDisposition::Passed
            && (self.qualified_set.selected_source_ids.is_empty()
                || !self.omissions.is_empty()
                || !self.uncertainty.is_empty())
        {
            return Err(EvidenceSurveillanceError::InvalidField(
                "passed surveillance cannot retain unresolved evidence".into(),
            ));
        }
        if blocked
            && (!self.qualified_set.selected_source_ids.is_empty()
                || !self.qualified_set.negative_source_ids.is_empty())
        {
            return Err(EvidenceSurveillanceError::InvalidField(
                "blocked surveillance cannot expose qualified source selections".into(),
            ));
        }
        if self.checks != canonical_checks(self.disposition) {
            return Err(EvidenceSurveillanceError::InvalidField(
                "evidence surveillance checks are not bound to the disposition".into(),
            ));
        }
        self.qualified_set.validate()?;
        if self.artifact.artifact_id != self.qualified_set.set_id
            || self.artifact.content_type != "application/vnd.aurora.qualified-evidence-set+json"
            || !self.artifact.semantic_loss.is_empty()
        {
            return Err(EvidenceSurveillanceError::Artifact(
                "qualified evidence artifact is not bound to the set".into(),
            ));
        }
        let expected_provenance = self
            .qualified_set
            .selected_source_ids
            .iter()
            .zip(&self.qualified_set.selected_source_digests)
            .map(|(source_id, digest)| {
                digest
                    .as_ref()
                    .map(|digest| ProvenanceLink {
                        source_id: source_id.clone(),
                        relation: "qualified-evidence-source".into(),
                        digest: digest.clone(),
                    })
                    .ok_or_else(|| {
                        EvidenceSurveillanceError::Artifact(
                            "qualified evidence provenance is missing a source digest".into(),
                        )
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        if self.artifact.provenance != expected_provenance {
            return Err(EvidenceSurveillanceError::Artifact(
                "qualified evidence provenance is not bound to selected sources".into(),
            ));
        }
        let qualified_payload = serde_json::to_value(&self.qualified_set)
            .map_err(|error| EvidenceSurveillanceError::Serialization(error.to_string()))?;
        self.artifact
            .verify_payload(&qualified_payload)
            .map_err(|error| EvidenceSurveillanceError::Artifact(error.to_string()))?;
        self.artifact
            .validate_metadata()
            .map_err(|error| EvidenceSurveillanceError::Artifact(error.to_string()))?;
        validate_request(&self.input)?;
        if self.input_digest != evidence_surveillance_input_digest(&self.input)? {
            return Err(EvidenceSurveillanceError::InvalidField(
                "evidence surveillance retained input digest does not match the request".into(),
            ));
        }
        let expected = build_evidence_surveillance(&self.input)?;
        if self != &expected {
            return Err(EvidenceSurveillanceError::InvalidField(
                "evidence surveillance receipt is not derived from its retained request".into(),
            ));
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, EvidenceSurveillanceError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| EvidenceSurveillanceError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| EvidenceSurveillanceError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum EvidenceSurveillanceError {
    #[error("invalid evidence surveillance field: {0}")]
    InvalidField(String),
    #[error("evidence surveillance artifact error: {0}")]
    Artifact(String),
    #[error("evidence surveillance serialization error: {0}")]
    Serialization(String),
}

fn validate_text(field: &str, value: &str) -> Result<(), EvidenceSurveillanceError> {
    if value.is_empty() || value.trim() != value {
        return Err(EvidenceSurveillanceError::InvalidField(format!(
            "{field} must be non-empty and trimmed"
        )));
    }
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(EvidenceSurveillanceError::InvalidField(format!(
            "{field} is outside its bounded text contract"
        )));
    }
    Ok(())
}

fn evidence_surveillance_input_digest(
    request: &EvidenceFeedRequest,
) -> Result<ContentHash, EvidenceSurveillanceError> {
    let value = serde_json::to_value(&canonical_evidence_feed_request(request))
        .map_err(|error| EvidenceSurveillanceError::Serialization(error.to_string()))?;
    ContentHash::of_value(&value)
        .map_err(|error| EvidenceSurveillanceError::Serialization(error.to_string()))
}

pub(crate) fn canonical_evidence_feed_request(
    request: &EvidenceFeedRequest,
) -> EvidenceFeedRequest {
    let mut canonical = request.clone();
    canonical.required_source_ids.sort();
    canonical.feed.sort_by(|left, right| {
        right
            .relevance_score
            .cmp(&left.relevance_score)
            .then_with(|| left.source_id.cmp(&right.source_id))
    });
    canonical
}

fn validate_unique_ids(values: &[String], field: &str) -> Result<(), EvidenceSurveillanceError> {
    let mut unique = BTreeSet::new();
    for value in values {
        validate_text(field, value)?;
        if !unique.insert(value) {
            return Err(EvidenceSurveillanceError::InvalidField(format!(
                "{field} contains duplicate values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_ids(values: &[String], field: &str) -> Result<(), EvidenceSurveillanceError> {
    if values.len() > MAX_SOURCE_IDS {
        return Err(EvidenceSurveillanceError::InvalidField(format!(
            "{field} exceeds its item bound"
        )));
    }
    validate_unique_ids(values, field)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(EvidenceSurveillanceError::InvalidField(format!(
            "{field} ordering is not canonical"
        )));
    }
    Ok(())
}

fn validate_sorted_notes(values: &[String], field: &str) -> Result<(), EvidenceSurveillanceError> {
    if values.len() > MAX_NOTE_ITEMS {
        return Err(EvidenceSurveillanceError::InvalidField(format!(
            "{field} exceeds its item bound"
        )));
    }
    validate_unique_ids(values, field)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(EvidenceSurveillanceError::InvalidField(format!(
            "{field} ordering is not canonical"
        )));
    }
    Ok(())
}

pub fn run_evidence_surveillance(
    request: &EvidenceFeedRequest,
) -> Result<EvidenceSurveillanceReceipt, EvidenceSurveillanceError> {
    let receipt = build_evidence_surveillance(request)?;
    receipt.validate()?;
    Ok(receipt)
}

fn build_evidence_surveillance(
    request: &EvidenceFeedRequest,
) -> Result<EvidenceSurveillanceReceipt, EvidenceSurveillanceError> {
    validate_request(request)?;
    let mut feed = request.feed.clone();
    feed.sort_by(|left, right| {
        right
            .relevance_score
            .cmp(&left.relevance_score)
            .then_with(|| left.source_id.cmp(&right.source_id))
    });
    let available: Vec<&EvidenceFeedItem> = feed
        .iter()
        .filter(|item| item.availability == EvidenceAvailability::Available)
        .collect();
    let qualified_available: Vec<&EvidenceFeedItem> = available
        .iter()
        .filter(|item| item.digest.is_some())
        .copied()
        .collect();
    let missing_required = request
        .required_source_ids
        .iter()
        .filter(|required| {
            !qualified_available
                .iter()
                .any(|item| &item.source_id == *required)
        })
        .cloned()
        .collect::<Vec<_>>();
    let mut uncertainty = Vec::new();
    if available.iter().any(|item| item.digest.is_none()) {
        uncertainty.push("one or more available sources lack a content digest".into());
    }
    let blocked = request.policy_decision != PolicyDecision::Allow
        || !request.protected_closure_satisfied
        || !request.raw_data_local;
    let selected = if blocked {
        Vec::new()
    } else {
        qualified_available
    };
    let selected_source_ids = selected
        .iter()
        .map(|item| item.source_id.clone())
        .collect::<Vec<_>>();
    let selected_source_scores = selected
        .iter()
        .map(|item| item.relevance_score)
        .collect::<Vec<_>>();
    let selected_source_digests = selected
        .iter()
        .map(|item| item.digest.clone())
        .collect::<Vec<_>>();
    let mut omissions = missing_required
        .iter()
        .map(|source| format!("required evidence source is not qualified: {source}"))
        .collect::<Vec<_>>();
    omissions.extend(feed.iter().filter_map(|item| match item.availability {
        EvidenceAvailability::Available => None,
        state => Some(format!(
            "{} evidence source is {:?}: {}",
            item.source_id, state, item.locator
        )),
    }));
    if selected_source_ids.is_empty() {
        uncertainty.push("no available evidence source can support a qualified set".into());
    }
    let disposition = if blocked {
        omissions.push(
            "policy, protected-closure, or raw-data-locality gate blocked the copilot".into(),
        );
        EvidenceSurveillanceDisposition::Blocked
    } else if selected_source_ids.is_empty()
        || !missing_required.is_empty()
        || !uncertainty.is_empty()
    {
        EvidenceSurveillanceDisposition::Unknown
    } else {
        EvidenceSurveillanceDisposition::Passed
    };
    let evidence_state = match disposition {
        EvidenceSurveillanceDisposition::Passed => EvidenceState::Supported,
        EvidenceSurveillanceDisposition::Blocked | EvidenceSurveillanceDisposition::Unknown => {
            EvidenceState::Unknown
        }
    };
    let negative_source_ids = available
        .iter()
        .filter(|item| selected_source_ids.contains(&item.source_id))
        .filter(|item| item.negative_result)
        .map(|item| item.source_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    omissions.sort();
    uncertainty.sort();
    let qualified_set = QualifiedEvidenceSet {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        set_id: format!("qualified-evidence:{}", request.request_id),
        study_id: request.study_id.clone(),
        intent: request.intent.clone(),
        selected_source_ids: selected_source_ids.clone(),
        selected_source_scores,
        selected_source_digests,
        evidence_state,
        negative_source_ids,
        omissions: omissions.clone(),
        uncertainty: uncertainty.clone(),
        ordering_rule: "relevance_score descending, source_id ascending".into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let payload = serde_json::to_value(&qualified_set)
        .map_err(|error| EvidenceSurveillanceError::Serialization(error.to_string()))?;
    let provenance = qualified_set
        .selected_source_ids
        .iter()
        .zip(&qualified_set.selected_source_digests)
        .map(|(source_id, digest)| {
            digest
                .as_ref()
                .map(|digest| ProvenanceLink {
                    source_id: source_id.clone(),
                    relation: "qualified-evidence-source".into(),
                    digest: digest.clone(),
                })
                .ok_or_else(|| {
                    EvidenceSurveillanceError::Artifact(
                        "qualified evidence provenance is missing a source digest".into(),
                    )
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let artifact = TypedResearchArtifact::from_payload(
        qualified_set.set_id.clone(),
        "application/vnd.aurora.qualified-evidence-set+json",
        &payload,
        Vec::new(),
        provenance,
    )
    .map_err(|error| EvidenceSurveillanceError::Artifact(error.to_string()))?;
    let effect_payload = json!({"request_id": request.request_id, "effect": Effect::ReadLocalData, "authorized": !blocked});
    let effect_digest = ContentHash::of_value(&effect_payload)
        .map_err(|error| EvidenceSurveillanceError::Serialization(error.to_string()))?;
    let effect_receipts = vec![EffectReceipt {
        effect: Effect::ReadLocalData,
        authorized: !blocked,
        reason: if blocked {
            "policy or locality gate denied local feed read".into()
        } else {
            "local evidence feed read is policy-authorized".into()
        },
        receipt_digest: effect_digest,
    }];
    let checks = canonical_checks(disposition);
    let receipt = EvidenceSurveillanceReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        contract_version: CONTRACT_VERSION.into(),
        input: canonical_evidence_feed_request(request),
        input_digest: evidence_surveillance_input_digest(request)?,
        request_id: request.request_id.clone(),
        study_id: request.study_id.clone(),
        intent: request.intent.clone(),
        selected_source_ids,
        disposition,
        qualified_set,
        effect_receipts,
        checks,
        omissions,
        uncertainty,
        artifact,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    Ok(receipt)
}

fn validate_request(request: &EvidenceFeedRequest) -> Result<(), EvidenceSurveillanceError> {
    if request.request_id.trim().is_empty()
        || request.study_id.trim().is_empty()
        || request.intent.trim().is_empty()
        || request.feed.is_empty()
        || request.feed.len() > MAX_FEED_ITEMS
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(EvidenceSurveillanceError::InvalidField(
            "evidence feed identity, intent, feed, and boundary are required".into(),
        ));
    }
    validate_text("request_id", &request.request_id)?;
    validate_text("study_id", &request.study_id)?;
    validate_text("intent", &request.intent)?;
    validate_text("boundary", &request.boundary)?;
    if request.required_source_ids.len() > MAX_SOURCE_IDS {
        return Err(EvidenceSurveillanceError::InvalidField(
            "required_source_ids exceeds its item bound".into(),
        ));
    }
    validate_unique_ids(&request.required_source_ids, "required_source_ids")?;
    let mut ids = BTreeSet::new();
    for item in &request.feed {
        validate_text("source_id", &item.source_id)?;
        validate_text("source_type", &item.source_type)?;
        validate_text("locator", &item.locator)?;
        validate_text("published_at", &item.published_at)?;
        if !item.locator.starts_with("local://") || item.locator.len() <= "local://".len() {
            return Err(EvidenceSurveillanceError::InvalidField(
                "evidence feed locators must identify institution-local metadata".into(),
            ));
        }
        if !ids.insert(item.source_id.clone()) {
            return Err(EvidenceSurveillanceError::InvalidField(
                "feed source identities and metadata must be non-empty and unique".into(),
            ));
        }
        if item
            .digest
            .as_ref()
            .is_some_and(|digest| *digest == ContentHash::of_bytes(b""))
        {
            return Err(EvidenceSurveillanceError::InvalidField(
                "feed source digests cannot be empty".into(),
            ));
        }
    }
    Ok(())
}

fn canonical_checks(disposition: EvidenceSurveillanceDisposition) -> Vec<String> {
    let mut checks = vec![
        "feed sources are canonically ordered by relevance and source identity".into(),
        "stale, contradictory, missing, and protected sources remain omissions".into(),
        "raw evidence payloads remain institution-local".into(),
    ];
    checks.push(match disposition {
        EvidenceSurveillanceDisposition::Passed => {
            "required source coverage and digests passed".into()
        }
        EvidenceSurveillanceDisposition::Blocked => {
            "policy, protected closure, or locality blocked surveillance".into()
        }
        EvidenceSurveillanceDisposition::Unknown => {
            "incomplete feed coverage remains unknown rather than promoted".into()
        }
    });
    checks.sort();
    checks
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> EvidenceFeedRequest {
        EvidenceFeedRequest {
            request_id: "request:surveillance".into(),
            study_id: "study:organoid".into(),
            intent: "monitor mechanism evidence".into(),
            required_source_ids: vec!["source:primary".into()],
            feed: vec![
                EvidenceFeedItem {
                    source_id: "source:secondary".into(),
                    source_type: "preprint".into(),
                    locator: "local://secondary".into(),
                    digest: Some(ContentHash::of_bytes(b"secondary")),
                    availability: EvidenceAvailability::Available,
                    published_at: "2026-01-01".into(),
                    relevance_score: 80,
                    negative_result: true,
                },
                EvidenceFeedItem {
                    source_id: "source:primary".into(),
                    source_type: "paper".into(),
                    locator: "local://primary".into(),
                    digest: Some(ContentHash::of_bytes(b"primary")),
                    availability: EvidenceAvailability::Available,
                    published_at: "2026-01-02".into(),
                    relevance_score: 90,
                    negative_result: false,
                },
            ],
            policy_decision: PolicyDecision::Allow,
            protected_closure_satisfied: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn surveillance_orders_sources_and_preserves_negative_evidence() {
        let receipt = run_evidence_surveillance(&request()).unwrap();
        assert_eq!(receipt.disposition, EvidenceSurveillanceDisposition::Passed);
        assert_eq!(
            receipt.selected_source_ids,
            vec!["source:primary", "source:secondary"]
        );
        assert_eq!(
            receipt.qualified_set.negative_source_ids,
            vec!["source:secondary"]
        );
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }

    #[test]
    fn missing_required_source_stays_unknown() {
        let mut value = request();
        value.feed[1].availability = EvidenceAvailability::Stale;
        let receipt = run_evidence_surveillance(&value).unwrap();
        assert_eq!(
            receipt.disposition,
            EvidenceSurveillanceDisposition::Unknown
        );
        assert!(!receipt.omissions.is_empty());
    }

    #[test]
    fn denied_policy_blocks_without_authorized_effect() {
        let mut value = request();
        value.policy_decision = PolicyDecision::Deny;
        let receipt = run_evidence_surveillance(&value).unwrap();
        assert_eq!(
            receipt.disposition,
            EvidenceSurveillanceDisposition::Blocked
        );
        assert!(!receipt.effect_receipts[0].authorized);
    }

    #[test]
    fn selected_source_without_digest_stays_unknown() {
        let mut value = request();
        value.feed[0].digest = None;
        let receipt = run_evidence_surveillance(&value).unwrap();
        assert_eq!(
            receipt.disposition,
            EvidenceSurveillanceDisposition::Unknown
        );
        assert!(receipt
            .uncertainty
            .iter()
            .any(|reason| reason.contains("content digest")));
        assert!(!receipt
            .qualified_set
            .selected_source_ids
            .contains(&"source:secondary".to_string()));
    }

    #[test]
    fn feed_effect_digest_is_not_forgeable() {
        let mut receipt = run_evidence_surveillance(&request()).unwrap();
        receipt.effect_receipts[0].receipt_digest = ContentHash::of_bytes(b"tampered");
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn negative_source_must_be_in_the_selected_set() {
        let mut receipt = run_evidence_surveillance(&request()).unwrap();
        receipt
            .qualified_set
            .negative_source_ids
            .push("source:unselected".into());
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn duplicate_required_source_is_rejected() {
        let mut value = request();
        value.required_source_ids.push("source:primary".into());
        assert!(run_evidence_surveillance(&value).is_err());
    }

    #[test]
    fn remote_locator_is_rejected_before_qualification() {
        let mut value = request();
        value.feed[0].locator = "https://example.invalid/secondary".into();
        assert!(run_evidence_surveillance(&value).is_err());
    }

    #[test]
    fn blocked_surveillance_does_not_expose_selected_sources() {
        let mut value = request();
        value.policy_decision = PolicyDecision::Deny;
        let receipt = run_evidence_surveillance(&value).unwrap();
        assert!(receipt.selected_source_ids.is_empty());
        assert!(receipt.qualified_set.negative_source_ids.is_empty());
    }

    #[test]
    fn ranking_scores_are_part_of_the_qualified_set_contract() {
        let mut receipt = run_evidence_surveillance(&request()).unwrap();
        receipt.qualified_set.selected_source_scores.swap(0, 1);
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn qualified_artifact_payload_is_verified() {
        let mut receipt = run_evidence_surveillance(&request()).unwrap();
        receipt.artifact.content_hash = ContentHash::of_bytes(b"tampered");
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn retained_request_tampering_is_rejected() {
        let mut receipt = run_evidence_surveillance(&request()).unwrap();
        receipt.input.intent = "tampered intent".into();
        assert!(receipt.validate().is_err());
    }
}
