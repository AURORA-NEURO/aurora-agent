//! Multimodal retrieval-and-synthesis contract model.
//!
//! Atlas feature: `AFA-adapter-P02-F06`.
//!
//! The model is a typed, comparability-aware corpus projection for multiple preclinical imaging
//! and omics studies.  It never merges incompatible studies, silently fills a missing modality,
//! or turns contradictory evidence into a positive synthesis.

use bioprism_foundation::{
    Effect, EvidenceAvailability, EvidenceState, PolicyDecision, ProvenanceLink,
    TypedResearchArtifact, PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P02-F06";
pub const CONTRACT_VERSION: &str = "multimodal-retrieval-synthesis/1.0";
const MAX_TEXT_BYTES: usize = 512;
const MAX_CANDIDATES: usize = 8192;
const MAX_SELECTED: usize = 4096;
const MAX_NOTE_ITEMS: usize = 8192;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopedRetrievalQuery {
    pub query_id: String,
    pub requester: String,
    pub intent: String,
    pub study_ids: Vec<String>,
    pub required_modalities: Vec<String>,
    pub comparability_profile: String,
    pub max_results: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalCandidate {
    pub evidence_id: String,
    pub study_id: String,
    pub modality: String,
    pub comparability_profile: String,
    pub digest: Option<ContentHash>,
    pub availability: EvidenceAvailability,
    pub relevance_score: u16,
    pub negative_result: bool,
    pub locator: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceSynthesisRequest {
    pub request_id: String,
    pub query: ScopedRetrievalQuery,
    pub candidates: Vec<RetrievalCandidate>,
    pub policy_decision: PolicyDecision,
    pub protected_closure_satisfied: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceSynthesisDisposition {
    Passed,
    Blocked,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SynthesisEffectReceipt {
    pub effect: Effect,
    pub authorized: bool,
    pub reason: String,
    pub receipt_digest: ContentHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceSynthesis {
    pub schema_version: String,
    pub synthesis_id: String,
    pub query_id: String,
    pub intent: String,
    pub comparability_profile: String,
    pub selected_evidence_ids: Vec<String>,
    pub selected_study_ids: Vec<String>,
    pub selected_modalities: Vec<String>,
    pub selected_relevance_scores: Vec<u16>,
    pub selected_digests: Vec<Option<ContentHash>>,
    pub evidence_state: EvidenceState,
    pub negative_evidence_ids: Vec<String>,
    pub contradictory_evidence_ids: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub boundary: String,
}

impl EvidenceSynthesis {
    pub fn validate(&self) -> Result<(), RetrievalSynthesisError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.selected_evidence_ids.len() != self.selected_digests.len()
            || self.selected_evidence_ids.len() != self.selected_study_ids.len()
            || self.selected_evidence_ids.len() != self.selected_modalities.len()
            || self.selected_evidence_ids.len() != self.selected_relevance_scores.len()
        {
            return Err(RetrievalSynthesisError::InvalidField(
                "evidence synthesis identity, alignment, or boundary is incomplete".into(),
            ));
        }
        validate_text("synthesis_id", &self.synthesis_id)?;
        validate_text("query_id", &self.query_id)?;
        validate_text("intent", &self.intent)?;
        validate_text("comparability_profile", &self.comparability_profile)?;
        if self.selected_evidence_ids.len() > MAX_SELECTED {
            return Err(RetrievalSynthesisError::InvalidField(
                "selected evidence exceeds its item bound".into(),
            ));
        }
        validate_unique_ids(&self.selected_evidence_ids, "selected_evidence_ids")?;
        for study_id in &self.selected_study_ids {
            validate_text("selected_study_id", study_id)?;
        }
        for modality in &self.selected_modalities {
            validate_text("selected_modality", modality)?;
        }
        if self.selected_digests.iter().any(Option::is_none) {
            return Err(RetrievalSynthesisError::InvalidField(
                "selected evidence must carry content digests".into(),
            ));
        }
        for digest in self.selected_digests.iter().flatten() {
            if *digest == ContentHash::of_bytes(b"") {
                return Err(RetrievalSynthesisError::InvalidField(
                    "selected evidence digests cannot be empty".into(),
                ));
            }
        }
        if self
            .selected_evidence_ids
            .windows(2)
            .zip(self.selected_relevance_scores.windows(2))
            .any(|(ids, scores)| {
                scores[0] < scores[1] || (scores[0] == scores[1] && ids[0] >= ids[1])
            })
        {
            return Err(RetrievalSynthesisError::InvalidField(
                "selected evidence ranking is not relevance-descending with id tie-breaks".into(),
            ));
        }
        validate_sorted_ids(&self.negative_evidence_ids, "negative_evidence_ids")?;
        validate_sorted_ids(
            &self.contradictory_evidence_ids,
            "contradictory_evidence_ids",
        )?;
        validate_sorted_notes(&self.omissions, "synthesis.omissions")?;
        validate_sorted_notes(&self.uncertainty, "synthesis.uncertainty")?;
        if !matches!(
            self.evidence_state,
            EvidenceState::Supported | EvidenceState::Unknown
        ) {
            return Err(RetrievalSynthesisError::InvalidField(
                "evidence synthesis state is outside the retrieval contract".into(),
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalSynthesisReceipt {
    pub schema_version: String,
    pub feature_id: String,
    pub contract_version: String,
    pub input: EvidenceSynthesisRequest,
    pub input_digest: ContentHash,
    pub request_id: String,
    pub query_id: String,
    pub requester: String,
    pub study_ids: Vec<String>,
    pub required_modalities: Vec<String>,
    pub max_results: usize,
    pub policy_decision: PolicyDecision,
    pub protected_closure_satisfied: bool,
    pub raw_data_local: bool,
    pub disposition: EvidenceSynthesisDisposition,
    pub synthesis: EvidenceSynthesis,
    pub effect_receipts: Vec<SynthesisEffectReceipt>,
    pub checks: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub boundary: String,
}

impl RetrievalSynthesisReceipt {
    pub fn validate(&self) -> Result<(), RetrievalSynthesisError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.feature_id != FEATURE_ID
            || self.contract_version != CONTRACT_VERSION
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.query_id != self.synthesis.query_id
            || self.checks.is_empty()
            || self.omissions != self.synthesis.omissions
            || self.uncertainty != self.synthesis.uncertainty
        {
            return Err(RetrievalSynthesisError::InvalidField(
                "retrieval synthesis identity, evidence linkage, or checks are incomplete".into(),
            ));
        }
        validate_text("request_id", &self.request_id)?;
        validate_text("query_id", &self.query_id)?;
        validate_text("requester", &self.requester)?;
        if self.max_results == 0 || self.max_results > MAX_SELECTED {
            return Err(RetrievalSynthesisError::InvalidField(
                "retrieval result bound is outside its contract".into(),
            ));
        }
        validate_unique_ids(&self.study_ids, "study_ids")?;
        validate_sorted_ids(&self.required_modalities, "required_modalities")?;
        if self.required_modalities.is_empty() {
            return Err(RetrievalSynthesisError::InvalidField(
                "retrieval synthesis requires modalities".into(),
            ));
        }
        validate_sorted_notes(&self.checks, "checks")?;
        if self.effect_receipts.len() != 1 {
            return Err(RetrievalSynthesisError::InvalidField(
                "retrieval synthesis requires exactly one local-read effect receipt".into(),
            ));
        }
        let effect = &self.effect_receipts[0];
        if effect.effect != Effect::ReadLocalData {
            return Err(RetrievalSynthesisError::InvalidField(
                "retrieval synthesis may only read institution-local data".into(),
            ));
        }
        validate_text("effect_receipt.reason", &effect.reason)?;
        let blocked = self.disposition == EvidenceSynthesisDisposition::Blocked;
        if effect.authorized == blocked
            || effect.reason
                != if blocked {
                    "policy or locality gate denied retrieval read"
                } else {
                    "retrieval read is policy-authorized"
                }
        {
            return Err(RetrievalSynthesisError::InvalidField(
                "retrieval effect authorization does not match disposition".into(),
            ));
        }
        let effect_payload = json!({
            "request_id": self.request_id,
            "effect": effect.effect,
            "authorized": effect.authorized,
        });
        let expected_effect_digest = ContentHash::of_value(&effect_payload)
            .map_err(|error| RetrievalSynthesisError::Serialization(error.to_string()))?;
        if effect.receipt_digest != expected_effect_digest {
            return Err(RetrievalSynthesisError::InvalidField(
                "retrieval effect receipt digest does not match its authorization".into(),
            ));
        }
        let blocked_gate = self.policy_decision != PolicyDecision::Allow
            || !self.protected_closure_satisfied
            || !self.raw_data_local;
        if (self.disposition == EvidenceSynthesisDisposition::Blocked) != blocked_gate {
            return Err(RetrievalSynthesisError::InvalidField(
                "retrieval disposition does not match policy, closure, and locality gates".into(),
            ));
        }
        let expected_state = if self.disposition == EvidenceSynthesisDisposition::Passed {
            EvidenceState::Supported
        } else {
            EvidenceState::Unknown
        };
        if self.synthesis.evidence_state != expected_state {
            return Err(RetrievalSynthesisError::InvalidField(
                "retrieval evidence state does not match disposition".into(),
            ));
        }
        if self.disposition == EvidenceSynthesisDisposition::Passed
            && (self.synthesis.selected_evidence_ids.is_empty()
                || !self.omissions.is_empty()
                || !self.uncertainty.is_empty())
        {
            return Err(RetrievalSynthesisError::InvalidField(
                "passed retrieval cannot retain unresolved evidence".into(),
            ));
        }
        if self.disposition == EvidenceSynthesisDisposition::Blocked
            && !self.synthesis.selected_evidence_ids.is_empty()
        {
            return Err(RetrievalSynthesisError::InvalidField(
                "blocked retrieval cannot expose selected evidence".into(),
            ));
        }
        self.synthesis.validate()?;
        if self.synthesis.synthesis_id != format!("evidence-synthesis:{}", self.request_id)
            || self.synthesis.selected_evidence_ids.len() > self.max_results
            || self
                .synthesis
                .selected_study_ids
                .iter()
                .any(|study_id| !self.study_ids.contains(study_id))
            || self.synthesis.comparability_profile.is_empty()
            || self.checks != canonical_checks(self.disposition)
        {
            return Err(RetrievalSynthesisError::InvalidField(
                "retrieval selection, query scope, or checks are not bound to the receipt".into(),
            ));
        }
        if self
            .synthesis
            .negative_evidence_ids
            .iter()
            .any(|evidence_id| {
                self.synthesis
                    .contradictory_evidence_ids
                    .contains(evidence_id)
            })
        {
            return Err(RetrievalSynthesisError::InvalidField(
                "negative and contradictory evidence identities must remain disjoint".into(),
            ));
        }
        if self.artifact.artifact_id != self.synthesis.synthesis_id
            || self.artifact.content_type != "application/vnd.aurora.evidence-synthesis+json"
        {
            return Err(RetrievalSynthesisError::Artifact(
                "synthesis artifact is not bound to the synthesis".into(),
            ));
        }
        let expected_provenance = self
            .synthesis
            .selected_evidence_ids
            .iter()
            .zip(&self.synthesis.selected_digests)
            .map(|(evidence_id, digest)| {
                digest
                    .as_ref()
                    .map(|digest| ProvenanceLink {
                        source_id: evidence_id.clone(),
                        relation: "selected-retrieval-evidence".into(),
                        digest: digest.clone(),
                    })
                    .ok_or_else(|| {
                        RetrievalSynthesisError::Artifact(
                            "synthesis provenance is missing a selected evidence digest".into(),
                        )
                    })
            })
            .collect::<Result<Vec<_>, _>>()?;
        if self.artifact.provenance != expected_provenance {
            return Err(RetrievalSynthesisError::Artifact(
                "synthesis provenance is not bound to selected evidence".into(),
            ));
        }
        let payload = synthesis_payload(self);
        self.artifact
            .verify_payload(&payload)
            .map_err(|error| RetrievalSynthesisError::Artifact(error.to_string()))?;
        self.artifact
            .validate_metadata()
            .map_err(|error| RetrievalSynthesisError::Artifact(error.to_string()))?;
        validate_request(&self.input)?;
        if self.input_digest != retrieval_input_digest(&self.input)? {
            return Err(RetrievalSynthesisError::InvalidField(
                "retrieval synthesis retained input digest does not match the request".into(),
            ));
        }
        let expected = build_evidence_synthesis(&self.input)?;
        if self != &expected {
            return Err(RetrievalSynthesisError::InvalidField(
                "retrieval synthesis receipt is not derived from its retained request".into(),
            ));
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, RetrievalSynthesisError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| RetrievalSynthesisError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| RetrievalSynthesisError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum RetrievalSynthesisError {
    #[error("invalid retrieval synthesis field: {0}")]
    InvalidField(String),
    #[error("retrieval synthesis artifact error: {0}")]
    Artifact(String),
    #[error("retrieval synthesis serialization error: {0}")]
    Serialization(String),
}

fn validate_text(field: &str, value: &str) -> Result<(), RetrievalSynthesisError> {
    if value.is_empty() || value.trim() != value {
        return Err(RetrievalSynthesisError::InvalidField(format!(
            "{field} must be non-empty and trimmed"
        )));
    }
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(RetrievalSynthesisError::InvalidField(format!(
            "{field} is outside its bounded text contract"
        )));
    }
    Ok(())
}

fn canonical_evidence_synthesis_request(
    request: &EvidenceSynthesisRequest,
) -> EvidenceSynthesisRequest {
    let mut canonical = request.clone();
    canonical.query.study_ids.sort();
    canonical.candidates.sort_by(|left, right| {
        right
            .relevance_score
            .cmp(&left.relevance_score)
            .then_with(|| left.evidence_id.cmp(&right.evidence_id))
    });
    canonical
}

fn retrieval_input_digest(
    request: &EvidenceSynthesisRequest,
) -> Result<ContentHash, RetrievalSynthesisError> {
    let canonical = canonical_evidence_synthesis_request(request);
    let value = serde_json::to_value(canonical)
        .map_err(|error| RetrievalSynthesisError::Serialization(error.to_string()))?;
    ContentHash::of_value(&value)
        .map_err(|error| RetrievalSynthesisError::Serialization(error.to_string()))
}

fn validate_unique_ids(values: &[String], field: &str) -> Result<(), RetrievalSynthesisError> {
    let mut unique = BTreeSet::new();
    for value in values {
        validate_text(field, value)?;
        if !unique.insert(value) {
            return Err(RetrievalSynthesisError::InvalidField(format!(
                "{field} contains duplicate values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_ids(values: &[String], field: &str) -> Result<(), RetrievalSynthesisError> {
    if values.len() > MAX_NOTE_ITEMS {
        return Err(RetrievalSynthesisError::InvalidField(format!(
            "{field} exceeds its item bound"
        )));
    }
    validate_unique_ids(values, field)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(RetrievalSynthesisError::InvalidField(format!(
            "{field} ordering is not canonical"
        )));
    }
    Ok(())
}

fn validate_sorted_notes(values: &[String], field: &str) -> Result<(), RetrievalSynthesisError> {
    validate_sorted_ids(values, field)
}

fn synthesis_payload(receipt: &RetrievalSynthesisReceipt) -> serde_json::Value {
    json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "feature_id": FEATURE_ID,
        "contract_version": CONTRACT_VERSION,
        "request_id": receipt.request_id,
        "query_id": receipt.query_id,
        "requester": receipt.requester,
        "study_ids": receipt.study_ids,
        "required_modalities": receipt.required_modalities,
        "max_results": receipt.max_results,
        "policy_decision": receipt.policy_decision,
        "protected_closure_satisfied": receipt.protected_closure_satisfied,
        "raw_data_local": receipt.raw_data_local,
        "disposition": receipt.disposition,
        "synthesis": receipt.synthesis,
        "effect_receipts": receipt.effect_receipts,
        "checks": receipt.checks,
        "omissions": receipt.omissions,
        "uncertainty": receipt.uncertainty,
        "boundary": receipt.boundary,
    })
}

fn canonical_checks(disposition: EvidenceSynthesisDisposition) -> Vec<String> {
    let mut checks = vec![
        "study scope and comparability profile are explicit".to_string(),
        "incompatible and unavailable evidence remains omitted".to_string(),
        "negative and contradictory evidence remain visible".to_string(),
        "raw source payloads remain institution-local".to_string(),
    ];
    checks.push(match disposition {
        EvidenceSynthesisDisposition::Passed => {
            "required modalities and evidence digests passed".into()
        }
        EvidenceSynthesisDisposition::Blocked => {
            "policy, protected closure, or locality blocked synthesis".into()
        }
        EvidenceSynthesisDisposition::Unknown => {
            "incomplete comparability or evidence coverage remains unknown".into()
        }
    });
    checks.sort();
    checks
}

pub fn compile_evidence_synthesis(
    request: &EvidenceSynthesisRequest,
) -> Result<RetrievalSynthesisReceipt, RetrievalSynthesisError> {
    let receipt = build_evidence_synthesis(request)?;
    receipt.validate()?;
    Ok(receipt)
}

fn build_evidence_synthesis(
    request: &EvidenceSynthesisRequest,
) -> Result<RetrievalSynthesisReceipt, RetrievalSynthesisError> {
    validate_request(request)?;
    let canonical_request = canonical_evidence_synthesis_request(request);
    let request = &canonical_request;
    let mut candidates = request.candidates.clone();
    candidates.sort_by(|left, right| {
        right
            .relevance_score
            .cmp(&left.relevance_score)
            .then_with(|| left.evidence_id.cmp(&right.evidence_id))
    });
    let blocked = request.policy_decision != PolicyDecision::Allow
        || !request.protected_closure_satisfied
        || !request.raw_data_local;
    let mut scoped_study_ids = request.query.study_ids.clone();
    scoped_study_ids.sort();
    let allowed = |candidate: &RetrievalCandidate| {
        scoped_study_ids.contains(&candidate.study_id)
            && candidate.comparability_profile == request.query.comparability_profile
            && candidate.availability == EvidenceAvailability::Available
            && candidate.digest.is_some()
    };
    let selected = if blocked {
        Vec::new()
    } else {
        candidates
            .iter()
            .filter(|candidate| allowed(candidate))
            .take(request.query.max_results)
            .collect::<Vec<_>>()
    };
    let selected_evidence_ids = selected
        .iter()
        .map(|item| item.evidence_id.clone())
        .collect::<Vec<_>>();
    let selected_modalities = selected
        .iter()
        .map(|item| item.modality.clone())
        .collect::<Vec<_>>();
    let selected_study_ids = selected
        .iter()
        .map(|item| item.study_id.clone())
        .collect::<Vec<_>>();
    let selected_relevance_scores = selected
        .iter()
        .map(|item| item.relevance_score)
        .collect::<Vec<_>>();
    let selected_digests = selected
        .iter()
        .map(|item| item.digest.clone())
        .collect::<Vec<_>>();
    let mut omissions = Vec::new();
    let mut uncertainty = Vec::new();
    if !blocked {
        for modality in &request.query.required_modalities {
            if !selected_modalities
                .iter()
                .any(|candidate| candidate == modality)
            {
                omissions.push(format!(
                    "required modality unavailable or incomparable: {modality}"
                ));
            }
        }
    }
    for candidate in &candidates {
        if candidate.availability != EvidenceAvailability::Available {
            omissions.push(format!(
                "{} evidence is {:?}: {}",
                candidate.evidence_id, candidate.availability, candidate.locator
            ));
        } else if !scoped_study_ids.contains(&candidate.study_id) {
            omissions.push(format!(
                "{} evidence is outside the scoped study set",
                candidate.evidence_id
            ));
        } else if candidate.comparability_profile != request.query.comparability_profile {
            omissions.push(format!(
                "{} evidence has an incompatible comparability profile",
                candidate.evidence_id
            ));
        }
    }
    if candidates.iter().any(|candidate| {
        candidate.availability == EvidenceAvailability::Available
            && scoped_study_ids.contains(&candidate.study_id)
            && candidate.comparability_profile == request.query.comparability_profile
            && candidate.digest.is_none()
    }) {
        uncertainty.push(
            "available scoped evidence without a content digest cannot support a complete synthesis"
                .into(),
        );
    }
    let contradictory_evidence_ids = candidates
        .iter()
        .filter(|candidate| candidate.availability == EvidenceAvailability::Contradictory)
        .map(|candidate| candidate.evidence_id.clone())
        .collect::<Vec<_>>();
    if !contradictory_evidence_ids.is_empty() {
        uncertainty.push(
            "contradictory evidence remains visible and prevents an unqualified synthesis".into(),
        );
    }
    if blocked {
        omissions
            .push("policy, protected-closure, or raw-data-locality gate blocked synthesis".into());
    }
    let disposition = if blocked {
        EvidenceSynthesisDisposition::Blocked
    } else if selected.is_empty() || !omissions.is_empty() || !uncertainty.is_empty() {
        EvidenceSynthesisDisposition::Unknown
    } else {
        EvidenceSynthesisDisposition::Passed
    };
    let evidence_state = if disposition == EvidenceSynthesisDisposition::Passed {
        EvidenceState::Supported
    } else {
        EvidenceState::Unknown
    };
    let negative_evidence_ids = selected
        .iter()
        .filter(|candidate| candidate.negative_result)
        .map(|candidate| candidate.evidence_id.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let contradictory_evidence_ids = contradictory_evidence_ids
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    omissions.sort();
    uncertainty.sort();
    let synthesis = EvidenceSynthesis {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        synthesis_id: format!("evidence-synthesis:{}", request.request_id),
        query_id: request.query.query_id.clone(),
        intent: request.query.intent.clone(),
        comparability_profile: request.query.comparability_profile.clone(),
        selected_evidence_ids,
        selected_study_ids,
        selected_modalities,
        selected_relevance_scores,
        selected_digests,
        evidence_state,
        negative_evidence_ids,
        contradictory_evidence_ids,
        omissions: omissions.clone(),
        uncertainty: uncertainty.clone(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let effect_payload = json!({"request_id": request.request_id, "effect": Effect::ReadLocalData, "authorized": !blocked});
    let effect_receipts = vec![SynthesisEffectReceipt {
        effect: Effect::ReadLocalData,
        authorized: !blocked,
        reason: if blocked {
            "policy or locality gate denied retrieval read".into()
        } else {
            "retrieval read is policy-authorized".into()
        },
        receipt_digest: ContentHash::of_value(&effect_payload)
            .map_err(|error| RetrievalSynthesisError::Serialization(error.to_string()))?,
    }];
    let checks = canonical_checks(disposition);
    let input_digest = retrieval_input_digest(request)?;
    let mut receipt = RetrievalSynthesisReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        contract_version: CONTRACT_VERSION.into(),
        input: canonical_request.clone(),
        input_digest,
        request_id: request.request_id.clone(),
        query_id: request.query.query_id.clone(),
        requester: request.query.requester.clone(),
        study_ids: scoped_study_ids,
        required_modalities: request.query.required_modalities.clone(),
        max_results: request.query.max_results,
        policy_decision: request.policy_decision,
        protected_closure_satisfied: request.protected_closure_satisfied,
        raw_data_local: request.raw_data_local,
        disposition,
        synthesis,
        effect_receipts,
        checks,
        omissions,
        uncertainty,
        artifact: TypedResearchArtifact {
            schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
            artifact_id: format!("evidence-synthesis:{}", request.request_id),
            content_type: "application/vnd.aurora.evidence-synthesis+json".into(),
            content_hash: ContentHash::of_bytes(b""),
            semantic_loss: Vec::new(),
            provenance: Vec::new(),
            boundary: PRECLINICAL_BOUNDARY.into(),
        },
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let payload = synthesis_payload(&receipt);
    let provenance = receipt
        .synthesis
        .selected_evidence_ids
        .iter()
        .zip(&receipt.synthesis.selected_digests)
        .map(|(evidence_id, digest)| {
            digest
                .as_ref()
                .map(|digest| ProvenanceLink {
                    source_id: evidence_id.clone(),
                    relation: "selected-retrieval-evidence".into(),
                    digest: digest.clone(),
                })
                .ok_or_else(|| {
                    RetrievalSynthesisError::Artifact(
                        "synthesis provenance is missing a selected evidence digest".into(),
                    )
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    receipt.artifact = TypedResearchArtifact::from_payload(
        receipt.synthesis.synthesis_id.clone(),
        "application/vnd.aurora.evidence-synthesis+json",
        &payload,
        Vec::new(),
        provenance,
    )
    .map_err(|error| RetrievalSynthesisError::Artifact(error.to_string()))?;
    Ok(receipt)
}

fn validate_request(request: &EvidenceSynthesisRequest) -> Result<(), RetrievalSynthesisError> {
    if request.request_id.trim().is_empty()
        || request.query.query_id.trim().is_empty()
        || request.query.requester.trim().is_empty()
        || request.query.intent.trim().is_empty()
        || request.query.study_ids.is_empty()
        || request.query.required_modalities.is_empty()
        || request.query.comparability_profile.trim().is_empty()
        || request.query.max_results == 0
        || request.query.max_results > MAX_SELECTED
        || request.candidates.is_empty()
        || request.candidates.len() > MAX_CANDIDATES
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(RetrievalSynthesisError::InvalidField(
            "retrieval query, scope, modalities, candidates, and boundary are required".into(),
        ));
    }
    validate_text("request_id", &request.request_id)?;
    validate_text("query_id", &request.query.query_id)?;
    validate_text("requester", &request.query.requester)?;
    validate_text("intent", &request.query.intent)?;
    validate_text(
        "comparability_profile",
        &request.query.comparability_profile,
    )?;
    validate_text("boundary", &request.boundary)?;
    if request.query.study_ids.len() > MAX_CANDIDATES
        || request.query.required_modalities.len() > MAX_CANDIDATES
    {
        return Err(RetrievalSynthesisError::InvalidField(
            "retrieval scope exceeds its item bound".into(),
        ));
    }
    validate_unique_ids(&request.query.study_ids, "study_ids")?;
    validate_sorted_ids(&request.query.required_modalities, "required_modalities")?;
    let mut evidence_ids = BTreeSet::new();
    for candidate in &request.candidates {
        validate_text("evidence_id", &candidate.evidence_id)?;
        validate_text("study_id", &candidate.study_id)?;
        validate_text("modality", &candidate.modality)?;
        validate_text(
            "candidate.comparability_profile",
            &candidate.comparability_profile,
        )?;
        validate_text("locator", &candidate.locator)?;
        if !candidate.locator.starts_with("local://") || candidate.locator.len() <= "local://".len()
        {
            return Err(RetrievalSynthesisError::InvalidField(
                "retrieval candidate locators must use the local:// scheme".into(),
            ));
        }
        if !evidence_ids.insert(candidate.evidence_id.clone()) {
            return Err(RetrievalSynthesisError::InvalidField(
                "retrieval candidate identities and metadata must be non-empty and unique".into(),
            ));
        }
        if candidate
            .digest
            .as_ref()
            .is_some_and(|digest| *digest == ContentHash::of_bytes(b""))
        {
            return Err(RetrievalSynthesisError::InvalidField(
                "candidate digests cannot be empty".into(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> EvidenceSynthesisRequest {
        EvidenceSynthesisRequest {
            request_id: "request:synthesis".into(),
            query: ScopedRetrievalQuery {
                query_id: "query:multimodal".into(),
                requester: "researcher".into(),
                intent: "compare imaging and omics".into(),
                study_ids: vec!["study:a".into()],
                required_modalities: vec!["imaging".into(), "omics".into()],
                comparability_profile: "protocol-v2".into(),
                max_results: 8,
            },
            candidates: vec![
                RetrievalCandidate {
                    evidence_id: "evidence:imaging".into(),
                    study_id: "study:a".into(),
                    modality: "imaging".into(),
                    comparability_profile: "protocol-v2".into(),
                    digest: Some(ContentHash::of_bytes(b"imaging")),
                    availability: EvidenceAvailability::Available,
                    relevance_score: 90,
                    negative_result: false,
                    locator: "local://imaging".into(),
                },
                RetrievalCandidate {
                    evidence_id: "evidence:omics".into(),
                    study_id: "study:a".into(),
                    modality: "omics".into(),
                    comparability_profile: "protocol-v2".into(),
                    digest: Some(ContentHash::of_bytes(b"omics")),
                    availability: EvidenceAvailability::Available,
                    relevance_score: 80,
                    negative_result: true,
                    locator: "local://omics".into(),
                },
            ],
            policy_decision: PolicyDecision::Allow,
            protected_closure_satisfied: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn synthesis_is_comparable_and_preserves_negative_result() {
        let receipt = compile_evidence_synthesis(&request()).unwrap();
        assert_eq!(receipt.disposition, EvidenceSynthesisDisposition::Passed);
        assert_eq!(
            receipt.synthesis.negative_evidence_ids,
            vec!["evidence:omics"]
        );
        assert_eq!(receipt.digest().unwrap(), receipt.digest().unwrap());
    }
    #[test]
    fn missing_modality_stays_unknown() {
        let mut value = request();
        value.candidates.pop();
        let receipt = compile_evidence_synthesis(&value).unwrap();
        assert_eq!(receipt.disposition, EvidenceSynthesisDisposition::Unknown);
        assert!(!receipt.omissions.is_empty());
    }

    #[test]
    fn blocked_read_is_explicitly_unauthorized() {
        let mut value = request();
        value.policy_decision = PolicyDecision::Deny;
        let receipt = compile_evidence_synthesis(&value).unwrap();
        assert_eq!(receipt.disposition, EvidenceSynthesisDisposition::Blocked);
        assert!(!receipt.effect_receipts[0].authorized);
        assert!(receipt.validate().is_ok());
    }

    #[test]
    fn effect_receipt_digest_binds_authorization() {
        let mut receipt = compile_evidence_synthesis(&request()).unwrap();
        receipt.effect_receipts[0].receipt_digest = ContentHash::of_bytes(b"tampered");
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn selected_evidence_arrays_cannot_become_misaligned() {
        let mut receipt = compile_evidence_synthesis(&request()).unwrap();
        receipt.synthesis.selected_digests.pop();
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn duplicate_query_scope_is_rejected() {
        let mut value = request();
        value.query.study_ids.push("study:a".into());
        assert!(compile_evidence_synthesis(&value).is_err());
    }

    #[test]
    fn undigested_scoped_evidence_is_not_selected() {
        let mut value = request();
        value.candidates[0].digest = None;
        let receipt = compile_evidence_synthesis(&value).unwrap();
        assert_eq!(receipt.disposition, EvidenceSynthesisDisposition::Unknown);
        assert!(!receipt
            .synthesis
            .selected_evidence_ids
            .contains(&"evidence:imaging".to_string()));
    }

    #[test]
    fn blocked_retrieval_does_not_expose_selected_evidence() {
        let mut value = request();
        value.policy_decision = PolicyDecision::Deny;
        let receipt = compile_evidence_synthesis(&value).unwrap();
        assert!(receipt.synthesis.selected_evidence_ids.is_empty());
        assert!(receipt.synthesis.selected_digests.is_empty());
    }

    #[test]
    fn retrieval_ranking_metadata_is_verified() {
        let mut receipt = compile_evidence_synthesis(&request()).unwrap();
        receipt.synthesis.selected_relevance_scores.swap(0, 1);
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn retrieval_artifact_payload_is_verified() {
        let mut receipt = compile_evidence_synthesis(&request()).unwrap();
        receipt.artifact.content_hash = ContentHash::of_bytes(b"tampered");
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn reordered_scope_and_candidates_have_stable_identity() {
        let mut reordered = request();
        reordered.query.study_ids.reverse();
        reordered.candidates.reverse();
        let first = compile_evidence_synthesis(&request()).unwrap();
        let second = compile_evidence_synthesis(&reordered).unwrap();
        assert_eq!(first.input_digest, second.input_digest);
        assert_eq!(first.digest().unwrap(), second.digest().unwrap());
    }

    #[test]
    fn retained_request_tampering_is_rejected() {
        let mut receipt = compile_evidence_synthesis(&request()).unwrap();
        receipt.input.query.intent = "tampered intent".into();
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn remote_candidate_locator_is_rejected() {
        let mut value = request();
        value.candidates[0].locator = "https://example.invalid/imaging".into();
        assert!(compile_evidence_synthesis(&value).is_err());
    }
}
