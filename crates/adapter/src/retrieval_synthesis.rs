//! Multimodal retrieval-and-synthesis contract model.
//!
//! Atlas feature: `AFA-adapter-P02-F06`.
//!
//! The model is a typed, comparability-aware corpus projection for multiple preclinical imaging
//! and omics studies.  It never merges incompatible studies, silently fills a missing modality,
//! or turns contradictory evidence into a positive synthesis.

use bioprism_foundation::{
    Effect, EvidenceAvailability, EvidenceState, PolicyDecision, TypedResearchArtifact,
    PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P02-F06";
pub const CONTRACT_VERSION: &str = "multimodal-retrieval-synthesis/1.0";

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
    pub selected_modalities: Vec<String>,
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
            || self.synthesis_id.trim().is_empty()
            || self.query_id.trim().is_empty()
            || self.intent.trim().is_empty()
            || self.comparability_profile.trim().is_empty()
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.selected_evidence_ids.len() != self.selected_digests.len()
            || self.selected_evidence_ids.len() != self.selected_modalities.len()
            || self
                .selected_evidence_ids
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                .len()
                != self.selected_evidence_ids.len()
        {
            return Err(RetrievalSynthesisError::InvalidField(
                "evidence synthesis identity, alignment, or boundary is incomplete".into(),
            ));
        }
        if self.evidence_state == EvidenceState::Proven
            && (!self.omissions.is_empty() || !self.uncertainty.is_empty())
        {
            return Err(RetrievalSynthesisError::InvalidField(
                "synthesis cannot claim proven with unresolved omissions".into(),
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
    pub request_id: String,
    pub query_id: String,
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
            || self.request_id.trim().is_empty()
            || self.query_id.trim().is_empty()
            || self.query_id != self.synthesis.query_id
            || self.checks.is_empty()
            || self.effect_receipts.is_empty()
            || self.omissions != self.synthesis.omissions
            || self.uncertainty != self.synthesis.uncertainty
        {
            return Err(RetrievalSynthesisError::InvalidField(
                "retrieval synthesis identity, evidence linkage, or checks are incomplete".into(),
            ));
        }
        self.synthesis.validate()?;
        self.artifact
            .validate_metadata()
            .map_err(|error| RetrievalSynthesisError::Artifact(error.to_string()))
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

pub fn compile_evidence_synthesis(
    request: &EvidenceSynthesisRequest,
) -> Result<RetrievalSynthesisReceipt, RetrievalSynthesisError> {
    validate_request(request)?;
    let mut candidates = request.candidates.clone();
    candidates.sort_by(|left, right| {
        right
            .relevance_score
            .cmp(&left.relevance_score)
            .then_with(|| left.evidence_id.cmp(&right.evidence_id))
    });
    let allowed = |candidate: &RetrievalCandidate| {
        request.query.study_ids.contains(&candidate.study_id)
            && candidate.comparability_profile == request.query.comparability_profile
            && candidate.availability == EvidenceAvailability::Available
    };
    let selected = candidates
        .iter()
        .filter(|candidate| allowed(candidate))
        .take(request.query.max_results)
        .collect::<Vec<_>>();
    let selected_evidence_ids = selected
        .iter()
        .map(|item| item.evidence_id.clone())
        .collect::<Vec<_>>();
    let selected_modalities = selected
        .iter()
        .map(|item| item.modality.clone())
        .collect::<Vec<_>>();
    let selected_digests = selected
        .iter()
        .map(|item| item.digest.clone())
        .collect::<Vec<_>>();
    let mut omissions = Vec::new();
    let mut uncertainty = Vec::new();
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
    for candidate in &candidates {
        if candidate.availability != EvidenceAvailability::Available {
            omissions.push(format!(
                "{} evidence is {:?}: {}",
                candidate.evidence_id, candidate.availability, candidate.locator
            ));
        } else if !request.query.study_ids.contains(&candidate.study_id) {
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
    if selected.iter().any(|candidate| candidate.digest.is_none()) {
        uncertainty.push(
            "selected evidence without a content digest cannot support a complete synthesis".into(),
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
    let blocked = request.policy_decision != PolicyDecision::Allow
        || !request.protected_closure_satisfied
        || !request.raw_data_local;
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
        .collect::<Vec<_>>();
    let synthesis = EvidenceSynthesis {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        synthesis_id: format!("evidence-synthesis:{}", request.request_id),
        query_id: request.query.query_id.clone(),
        intent: request.query.intent.clone(),
        comparability_profile: request.query.comparability_profile.clone(),
        selected_evidence_ids,
        selected_modalities,
        selected_digests,
        evidence_state,
        negative_evidence_ids,
        contradictory_evidence_ids,
        omissions: omissions.clone(),
        uncertainty: uncertainty.clone(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let payload = serde_json::to_value(&synthesis)
        .map_err(|error| RetrievalSynthesisError::Serialization(error.to_string()))?;
    let artifact = TypedResearchArtifact::from_payload(
        synthesis.synthesis_id.clone(),
        "application/vnd.aurora.evidence-synthesis+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| RetrievalSynthesisError::Artifact(error.to_string()))?;
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
    let mut checks = vec![
        "study scope and comparability profile are explicit".into(),
        "incompatible and unavailable evidence remains omitted".into(),
        "negative and contradictory evidence remain visible".into(),
        "raw source payloads remain institution-local".into(),
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
    let receipt = RetrievalSynthesisReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        contract_version: CONTRACT_VERSION.into(),
        request_id: request.request_id.clone(),
        query_id: request.query.query_id.clone(),
        disposition,
        synthesis,
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

fn validate_request(request: &EvidenceSynthesisRequest) -> Result<(), RetrievalSynthesisError> {
    if request.request_id.trim().is_empty()
        || request.query.query_id.trim().is_empty()
        || request.query.requester.trim().is_empty()
        || request.query.intent.trim().is_empty()
        || request.query.study_ids.is_empty()
        || request.query.required_modalities.is_empty()
        || request.query.comparability_profile.trim().is_empty()
        || request.query.max_results == 0
        || request.candidates.is_empty()
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(RetrievalSynthesisError::InvalidField(
            "retrieval query, scope, modalities, candidates, and boundary are required".into(),
        ));
    }
    let mut evidence_ids = std::collections::BTreeSet::new();
    for candidate in &request.candidates {
        if candidate.evidence_id.trim().is_empty()
            || candidate.study_id.trim().is_empty()
            || candidate.modality.trim().is_empty()
            || candidate.comparability_profile.trim().is_empty()
            || candidate.locator.trim().is_empty()
            || !evidence_ids.insert(candidate.evidence_id.clone())
        {
            return Err(RetrievalSynthesisError::InvalidField(
                "retrieval candidate identities and metadata must be non-empty and unique".into(),
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
}
