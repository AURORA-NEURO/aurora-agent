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
    Effect, EvidenceAvailability, EvidenceState, PolicyDecision, TypedResearchArtifact,
    PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P01-F09";
pub const CONTRACT_VERSION: &str = "evidence-surveillance-copilot/1.0";

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
            || self.set_id.trim().is_empty()
            || self.study_id.trim().is_empty()
            || self.intent.trim().is_empty()
            || self.ordering_rule.trim().is_empty()
            || self.boundary != PRECLINICAL_BOUNDARY
        {
            return Err(EvidenceSurveillanceError::InvalidField(
                "qualified evidence identity, ordering, or boundary is incomplete".into(),
            ));
        }
        if self.selected_source_ids.len() != self.selected_source_digests.len()
            || self
                .selected_source_ids
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != self.selected_source_ids.len()
        {
            return Err(EvidenceSurveillanceError::InvalidField(
                "qualified evidence sources are not deterministically ordered".into(),
            ));
        }
        if self.evidence_state == EvidenceState::Proven
            && (!self.omissions.is_empty() || !self.uncertainty.is_empty())
        {
            return Err(EvidenceSurveillanceError::InvalidField(
                "qualified evidence cannot claim proven with unresolved omissions".into(),
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
            || self.request_id.trim().is_empty()
            || self.study_id.trim().is_empty()
            || self.intent.trim().is_empty()
            || self.checks.is_empty()
            || self.effect_receipts.is_empty()
            || self.selected_source_ids != self.qualified_set.selected_source_ids
            || self.qualified_set.study_id != self.study_id
            || self.qualified_set.intent != self.intent
        {
            return Err(EvidenceSurveillanceError::InvalidField(
                "evidence surveillance identity, effects, checks, or qualified-set linkage is incomplete".into(),
            ));
        }
        self.qualified_set.validate()?;
        self.artifact
            .validate_metadata()
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

#[derive(Debug, Error)]
pub enum EvidenceSurveillanceError {
    #[error("invalid evidence surveillance field: {0}")]
    InvalidField(String),
    #[error("evidence surveillance artifact error: {0}")]
    Artifact(String),
    #[error("evidence surveillance serialization error: {0}")]
    Serialization(String),
}

pub fn run_evidence_surveillance(
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
    let selected_source_ids = available
        .iter()
        .map(|item| item.source_id.clone())
        .collect::<Vec<_>>();
    let selected_source_digests = available
        .iter()
        .map(|item| item.digest.clone())
        .collect::<Vec<_>>();
    let missing_required = request
        .required_source_ids
        .iter()
        .filter(|required| !available.iter().any(|item| &item.source_id == *required))
        .cloned()
        .collect::<Vec<_>>();
    let mut omissions = missing_required
        .iter()
        .map(|source| format!("required evidence source unavailable: {source}"))
        .collect::<Vec<_>>();
    omissions.extend(feed.iter().filter_map(|item| match item.availability {
        EvidenceAvailability::Available => None,
        state => Some(format!(
            "{} evidence source is {:?}: {}",
            item.source_id, state, item.locator
        )),
    }));
    let mut uncertainty = Vec::new();
    if selected_source_ids.is_empty() {
        uncertainty.push("no available evidence source can support a qualified set".into());
    }
    if selected_source_ids.iter().any(|source| {
        !feed
            .iter()
            .any(|item| &item.source_id == source && item.digest.is_some())
    }) {
        uncertainty.push("one or more selected sources lack a content digest".into());
    }
    let blocked = request.policy_decision != PolicyDecision::Allow
        || !request.protected_closure_satisfied
        || !request.raw_data_local;
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
        .filter(|item| item.negative_result)
        .map(|item| item.source_id.clone())
        .collect::<Vec<_>>();
    let qualified_set = QualifiedEvidenceSet {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        set_id: format!("qualified-evidence:{}", request.request_id),
        study_id: request.study_id.clone(),
        intent: request.intent.clone(),
        selected_source_ids: selected_source_ids.clone(),
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
    let artifact = TypedResearchArtifact::from_payload(
        qualified_set.set_id.clone(),
        "application/vnd.aurora.qualified-evidence-set+json",
        &payload,
        Vec::new(),
        Vec::new(),
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
    let receipt = EvidenceSurveillanceReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        contract_version: CONTRACT_VERSION.into(),
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
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &EvidenceFeedRequest) -> Result<(), EvidenceSurveillanceError> {
    if request.request_id.trim().is_empty()
        || request.study_id.trim().is_empty()
        || request.intent.trim().is_empty()
        || request.feed.is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(EvidenceSurveillanceError::InvalidField(
            "evidence feed identity, intent, feed, and boundary are required".into(),
        ));
    }
    let mut ids = std::collections::BTreeSet::new();
    for item in &request.feed {
        if item.source_id.trim().is_empty()
            || item.source_type.trim().is_empty()
            || item.locator.trim().is_empty()
            || item.published_at.trim().is_empty()
            || !ids.insert(item.source_id.clone())
        {
            return Err(EvidenceSurveillanceError::InvalidField(
                "feed source identities and metadata must be non-empty and unique".into(),
            ));
        }
    }
    if request
        .required_source_ids
        .iter()
        .any(|required| required.trim().is_empty())
    {
        return Err(EvidenceSurveillanceError::InvalidField(
            "required source identities must be non-empty".into(),
        ));
    }
    Ok(())
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
}
