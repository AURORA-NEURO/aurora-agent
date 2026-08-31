//! Multimodal retrieval-and-synthesis assurance harness.
//!
//! Atlas feature: `AFA-biolang-P02-F26`.
//!
//! This module verifies a scoped retrieval corpus before a synthesis can be released.  It is
//! deliberately independent from a retrieval provider: providers submit typed candidates and the
//! harness deterministically checks scope, cross-study comparability, evidence state, content
//! addressing, protected closure, policy, approval, locality, and budget.  Unknown and negative
//! evidence remain visible; the harness never turns missing evidence into a positive conclusion.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-biolang-P02-F26";
pub const CONTRACT_VERSION: &str = "biolang-multimodal-retrieval-synthesis-assurance/1.0";
pub const SCHEMA_VERSION: &str = "aurora-research-contract/1.0";
pub const PRECLINICAL_BOUNDARY: &str =
    "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetrievalEvidenceState {
    Supported,
    Unknown,
    Contradicted,
    Unmeasured,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetrievalAssuranceDisposition {
    Passed,
    Conditional,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalCandidate {
    pub evidence_id: String,
    pub study_id: String,
    pub modality: String,
    pub comparability_profile: String,
    pub state: RetrievalEvidenceState,
    pub artifact_digest: Option<ContentHash>,
    pub provenance_digest: Option<ContentHash>,
    pub relevance_milli: u32,
    pub negative_result: bool,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalAssuranceRequest {
    pub request_id: String,
    pub workflow_id: String,
    pub query_id: String,
    pub intent: String,
    pub scope: String,
    pub study_order: Vec<String>,
    pub required_modalities: Vec<String>,
    pub comparability_profile: String,
    pub max_results: usize,
    pub candidates: Vec<RetrievalCandidate>,
    pub replay_identity: ContentHash,
    pub budget: u64,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalAssuranceSummary {
    pub summary_id: String,
    pub disposition: RetrievalAssuranceDisposition,
    pub candidate_order: Vec<String>,
    pub ranked_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub artifact_order: Vec<ContentHash>,
    pub provenance_order: Vec<ContentHash>,
    pub selected_count: u32,
    pub blocked_count: u32,
    pub unknown_count: u32,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub replay_identity: ContentHash,
    pub summary_digest: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetrievalAssuranceReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub query_id: String,
    pub disposition: RetrievalAssuranceDisposition,
    pub summary: RetrievalAssuranceSummary,
    pub checks: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RetrievalAssuranceError {
    #[error("invalid retrieval assurance request: {0}")]
    Invalid(String),
    #[error("retrieval assurance serialization failed: {0}")]
    Serialization(String),
}

impl RetrievalAssuranceReceipt {
    pub fn validate(&self) -> Result<(), RetrievalAssuranceError> {
        if self.schema_version != SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.query_id.trim().is_empty()
            || self.checks.is_empty()
            || self.effect_receipts.is_empty()
            || self.summary.boundary != PRECLINICAL_BOUNDARY
            || self.summary.summary_id.trim().is_empty()
            || (self.summary.selected_order.is_empty()
                && self.summary.blocked_order.is_empty()
                && self.summary.unknown_order.is_empty()
                && self.summary.omissions.is_empty()
                && self.summary.uncertainty.is_empty()
                && self.summary.negative_evidence.is_empty())
        {
            return Err(RetrievalAssuranceError::Invalid(
                "retrieval assurance identity, summary, checks, effects, locality, or boundary is incomplete".into(),
            ));
        }
        for values in [
            &self.summary.candidate_order,
            &self.summary.selected_order,
            &self.summary.blocked_order,
            &self.summary.unknown_order,
            &self.summary.study_order,
            &self.summary.modality_order,
            &self.summary.omissions,
            &self.summary.uncertainty,
            &self.summary.negative_evidence,
            &self.checks,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(RetrievalAssuranceError::Invalid(
                    "retrieval assurance ordering is not canonical".into(),
                ));
            }
        }
        let candidate_ids = self.summary.candidate_order.iter().collect::<BTreeSet<_>>();
        let ranked_ids = self.summary.ranked_order.iter().collect::<BTreeSet<_>>();
        if candidate_ids != ranked_ids {
            return Err(RetrievalAssuranceError::Invalid(
                "retrieval assurance candidate and ranked orders disagree".into(),
            ));
        }
        let mut classified_ids = BTreeSet::new();
        for values in [
            &self.summary.selected_order,
            &self.summary.blocked_order,
            &self.summary.unknown_order,
        ] {
            for id in values {
                if !candidate_ids.contains(id) {
                    if !id.starts_with("request:") {
                        return Err(RetrievalAssuranceError::Invalid(
                            "retrieval assurance classification is outside the candidate set"
                                .into(),
                        ));
                    }
                    continue;
                }
                if !classified_ids.insert(id) {
                    return Err(RetrievalAssuranceError::Invalid(
                        "retrieval assurance candidate classification is duplicated".into(),
                    ));
                }
            }
        }
        for values in [&self.summary.artifact_order, &self.summary.provenance_order] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(RetrievalAssuranceError::Invalid(
                    "retrieval assurance digest ordering is not canonical".into(),
                ));
            }
        }
        if self.effect_receipts.iter().any(|effect| {
            !effect.starts_with("evaluate:retrieval-assurance:") && effect != "block:unsafe-release"
        }) {
            return Err(RetrievalAssuranceError::Invalid(
                "retrieval assurance effect is outside the evaluation or unsafe-release boundary"
                    .into(),
            ));
        }
        if u64::from(self.summary.selected_count)
            != u64::try_from(self.summary.selected_order.len()).unwrap_or(u64::MAX)
            || u64::from(self.summary.blocked_count)
                != u64::try_from(self.summary.blocked_order.len()).unwrap_or(u64::MAX)
            || u64::from(self.summary.unknown_count)
                != u64::try_from(self.summary.unknown_order.len()).unwrap_or(u64::MAX)
        {
            return Err(RetrievalAssuranceError::Invalid(
                "retrieval assurance summary counts do not match canonical orders".into(),
            ));
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, RetrievalAssuranceError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| RetrievalAssuranceError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| RetrievalAssuranceError::Serialization(error.to_string()))
    }
}

pub fn assure_retrieval_synthesis(
    request: &RetrievalAssuranceRequest,
) -> Result<RetrievalAssuranceReceipt, RetrievalAssuranceError> {
    validate_request(request)?;
    let mut candidates = request.candidates.clone();
    candidates.sort_by(|left, right| {
        right
            .relevance_milli
            .cmp(&left.relevance_milli)
            .then_with(|| left.evidence_id.cmp(&right.evidence_id))
    });
    let ranked_order = candidates
        .iter()
        .map(|candidate| candidate.evidence_id.clone())
        .collect::<Vec<_>>();
    let candidate_order = {
        let mut ids = candidates
            .iter()
            .map(|candidate| candidate.evidence_id.clone())
            .collect::<Vec<_>>();
        ids.sort();
        ids
    };
    let studies = request.study_order.iter().cloned().collect::<BTreeSet<_>>();
    let modalities = request
        .required_modalities
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut selected = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut study_order = BTreeSet::new();
    let mut modality_order = BTreeSet::new();
    let mut artifacts = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut spent = 0_u64;
    let mut selected_modalities = BTreeSet::new();
    for candidate in &candidates {
        let cost = u64::try_from(candidate.evidence_id.len())
            .unwrap_or(u64::MAX)
            .saturating_add(1);
        let within_scope = studies.contains(&candidate.study_id);
        let profile_ok = candidate.comparability_profile == request.comparability_profile;
        let modality_required = modalities.contains(&candidate.modality);
        let budget_ok = cost <= request.budget.saturating_sub(spent);
        study_order.insert(candidate.study_id.clone());
        modality_order.insert(candidate.modality.clone());
        if !within_scope {
            blocked.insert(candidate.evidence_id.clone());
            omissions.insert(format!(
                "evidence:{}:outside-scoped-study-set",
                candidate.evidence_id
            ));
            continue;
        }
        if !profile_ok {
            blocked.insert(candidate.evidence_id.clone());
            omissions.insert(format!(
                "evidence:{}:incompatible-comparability-profile",
                candidate.evidence_id
            ));
            continue;
        }
        if !modality_required {
            uncertainty.insert(format!(
                "evidence:{}:optional-modality-not-required",
                candidate.evidence_id
            ));
        }
        if !budget_ok {
            blocked.insert(candidate.evidence_id.clone());
            omissions.insert(format!(
                "evidence:{}:budget-ceiling-exceeded",
                candidate.evidence_id
            ));
            continue;
        }
        match candidate.state {
            RetrievalEvidenceState::Contradicted => {
                blocked.insert(candidate.evidence_id.clone());
                negative.insert(format!(
                    "evidence:{}:contradicted-retrieval-evidence",
                    candidate.evidence_id
                ));
                continue;
            }
            RetrievalEvidenceState::Unknown | RetrievalEvidenceState::Unmeasured => {
                unknown.insert(candidate.evidence_id.clone());
                uncertainty.insert(
                    format!(
                        "evidence:{}:state-{:?}-not-admitted",
                        candidate.evidence_id, candidate.state
                    )
                    .to_ascii_lowercase(),
                );
                continue;
            }
            RetrievalEvidenceState::Supported => {}
        }
        if !candidate.omissions.is_empty() {
            unknown.insert(candidate.evidence_id.clone());
            omissions.extend(
                candidate
                    .omissions
                    .iter()
                    .map(|item| format!("evidence:{}:{item}", candidate.evidence_id)),
            );
            continue;
        }
        if !candidate.uncertainty.is_empty() {
            unknown.insert(candidate.evidence_id.clone());
            uncertainty.extend(
                candidate
                    .uncertainty
                    .iter()
                    .map(|item| format!("evidence:{}:{item}", candidate.evidence_id)),
            );
            continue;
        }
        if candidate.artifact_digest.is_none() || candidate.provenance_digest.is_none() {
            unknown.insert(candidate.evidence_id.clone());
            omissions.insert(format!(
                "evidence:{}:artifact-or-provenance-digest-missing",
                candidate.evidence_id
            ));
            continue;
        }
        if selected.len() >= request.max_results {
            uncertainty.insert(format!(
                "evidence:{}:max-results-admission-ceiling",
                candidate.evidence_id
            ));
            continue;
        }
        selected.insert(candidate.evidence_id.clone());
        selected_modalities.insert(candidate.modality.clone());
        let (Some(artifact_digest), Some(provenance_digest)) = (
            candidate.artifact_digest.clone(),
            candidate.provenance_digest.clone(),
        ) else {
            selected.remove(&candidate.evidence_id);
            unknown.insert(candidate.evidence_id.clone());
            omissions.insert(format!(
                "evidence:{}:artifact-or-provenance-digest-missing",
                candidate.evidence_id
            ));
            continue;
        };
        artifacts.insert(artifact_digest);
        provenance.insert(provenance_digest);
        spent = spent.saturating_add(cost);
        if candidate.negative_result {
            negative.insert(format!(
                "evidence:{}:negative-result-retained",
                candidate.evidence_id
            ));
        }
    }
    for modality in &request.required_modalities {
        if !selected_modalities.contains(modality) {
            omissions.insert(format!("modality:{}:required-but-not-admitted", modality));
        }
    }
    if !request.policy_allow {
        blocked.insert("request:policy-denied".into());
        negative.insert("request:policy-denied-no-synthesis-release".into());
    }
    if !request.protected_closure {
        unknown.insert("request:protected-closure-incomplete".into());
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.signed_approval {
        blocked.insert("request:signed-approval-required".into());
        omissions.insert("request:signed-approval-required".into());
    }
    if !request.raw_data_local {
        blocked.insert("request:raw-data-locality-required".into());
        omissions.insert("request:raw-data-locality-required".into());
    }
    let selected_order = selected.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let unknown_order = unknown.into_iter().collect::<Vec<_>>();
    let study_order = study_order.into_iter().collect::<Vec<_>>();
    let modality_order = modality_order.into_iter().collect::<Vec<_>>();
    let artifact_order = artifacts.into_iter().collect::<Vec<_>>();
    let provenance_order = provenance.into_iter().collect::<Vec<_>>();
    let omissions = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence = negative.into_iter().collect::<Vec<_>>();
    let disposition =
        if !request.policy_allow || !request.signed_approval || !request.raw_data_local {
            RetrievalAssuranceDisposition::Blocked
        } else if selected_order.is_empty() {
            RetrievalAssuranceDisposition::Unknown
        } else if !blocked_order.is_empty()
            || !unknown_order.is_empty()
            || !omissions.is_empty()
            || !uncertainty.is_empty()
            || !request.protected_closure
        {
            RetrievalAssuranceDisposition::Conditional
        } else {
            RetrievalAssuranceDisposition::Passed
        };
    let mut checks = vec![
        "candidate and ranked ordering is deterministic with relevance then evidence-id tie breaks".to_string(),
        "study scope, modality coverage, and comparability profile are explicit gates".to_string(),
        "artifact and provenance digests are required before synthesis admission".to_string(),
        "unknown, unmeasured, contradicted, omitted, and negative evidence remains visible".to_string(),
        "policy, protected closure, signed approval, locality, and budget gates fail closed".to_string(),
        "raw retrieval payloads remain institution-local; only typed assurance manifests are emitted".to_string(),
    ];
    checks.sort();
    let mut effect_receipts = selected_order
        .iter()
        .map(|id| format!("evaluate:retrieval-assurance:{id}"))
        .collect::<Vec<_>>();
    if disposition != RetrievalAssuranceDisposition::Passed {
        effect_receipts.push("block:unsafe-release".into());
    }
    effect_receipts.sort();
    let summary_id = format!("retrieval-assurance-summary:{}", request.request_id);
    let summary_payload = json!({
        "summary_id": summary_id,
        "disposition": disposition,
        "candidate_order": candidate_order,
        "ranked_order": ranked_order,
        "selected_order": selected_order,
        "blocked_order": blocked_order,
        "unknown_order": unknown_order,
        "study_order": study_order,
        "modality_order": modality_order,
        "artifact_order": artifact_order,
        "provenance_order": provenance_order,
        "selected_count": selected_order.len(),
        "blocked_count": blocked_order.len(),
        "unknown_count": unknown_order.len(),
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": negative_evidence,
        "replay_identity": request.replay_identity,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let summary_digest = ContentHash::of_value(&summary_payload)
        .map_err(|error| RetrievalAssuranceError::Serialization(error.to_string()))?;
    let summary = RetrievalAssuranceSummary {
        summary_id,
        disposition,
        candidate_order,
        ranked_order,
        selected_order: selected_order.clone(),
        blocked_order: blocked_order.clone(),
        unknown_order: unknown_order.clone(),
        study_order,
        modality_order,
        artifact_order,
        provenance_order,
        selected_count: u32::try_from(selected_order.len())
            .map_err(|_| RetrievalAssuranceError::Invalid("selected count exceeds u32".into()))?,
        blocked_count: u32::try_from(blocked_order.len())
            .map_err(|_| RetrievalAssuranceError::Invalid("blocked count exceeds u32".into()))?,
        unknown_count: u32::try_from(unknown_order.len())
            .map_err(|_| RetrievalAssuranceError::Invalid("unknown count exceeds u32".into()))?,
        omissions: omissions.clone(),
        uncertainty: uncertainty.clone(),
        negative_evidence: negative_evidence.clone(),
        replay_identity: request.replay_identity.clone(),
        summary_digest,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let receipt = RetrievalAssuranceReceipt {
        schema_version: SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        query_id: request.query_id.clone(),
        disposition,
        summary,
        checks,
        omissions,
        uncertainty,
        negative_evidence,
        effect_receipts,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &RetrievalAssuranceRequest) -> Result<(), RetrievalAssuranceError> {
    if request.request_id.trim().is_empty()
        || request.workflow_id.trim().is_empty()
        || request.query_id.trim().is_empty()
        || request.intent.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.study_order.is_empty()
        || request.required_modalities.is_empty()
        || request.comparability_profile.trim().is_empty()
        || request.max_results == 0
        || request.candidates.is_empty()
        || request.budget == 0
        || u64::try_from(request.candidates.len()).map_or(true, |count| count > u64::from(u32::MAX))
        || request.boundary != PRECLINICAL_BOUNDARY
        || request
            .study_order
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
        || request
            .required_modalities
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(RetrievalAssuranceError::Invalid(
            "retrieval assurance identity, scope, study/modality closure, budget, or boundary is incomplete".into(),
        ));
    }
    let mut evidence_ids = BTreeSet::new();
    for candidate in &request.candidates {
        if candidate.evidence_id.trim().is_empty()
            || candidate.study_id.trim().is_empty()
            || candidate.modality.trim().is_empty()
            || candidate.comparability_profile.trim().is_empty()
            || candidate.boundary != PRECLINICAL_BOUNDARY
            || !evidence_ids.insert(candidate.evidence_id.clone())
            || candidate
                .omissions
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || candidate
                .uncertainty
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err(RetrievalAssuranceError::Invalid(format!(
                "retrieval candidate {} is invalid or duplicated",
                candidate.evidence_id
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }

    fn candidate(
        id: &str,
        study: &str,
        modality: &str,
        state: RetrievalEvidenceState,
        negative_result: bool,
    ) -> RetrievalCandidate {
        RetrievalCandidate {
            evidence_id: id.into(),
            study_id: study.into(),
            modality: modality.into(),
            comparability_profile: "protocol-v2".into(),
            state,
            artifact_digest: Some(hash(&format!("artifact:{id}"))),
            provenance_digest: Some(hash(&format!("provenance:{id}"))),
            relevance_milli: if modality == "imaging" { 900 } else { 800 },
            negative_result,
            omissions: vec![],
            uncertainty: vec![],
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    fn request(candidates: Vec<RetrievalCandidate>) -> RetrievalAssuranceRequest {
        RetrievalAssuranceRequest {
            request_id: "retrieval:assurance".into(),
            workflow_id: "workflow:synthesis".into(),
            query_id: "query:multimodal".into(),
            intent: "compare imaging and omics".into(),
            scope: "organoid:neural".into(),
            study_order: vec!["study:a".into(), "study:b".into()],
            required_modalities: vec!["imaging".into(), "omics".into()],
            comparability_profile: "protocol-v2".into(),
            max_results: 8,
            candidates,
            replay_identity: hash("replay"),
            budget: 10_000,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn passes_comparable_multimodal_corpus_and_retains_negative_result() {
        let receipt = assure_retrieval_synthesis(&request(vec![
            candidate(
                "evidence:imaging",
                "study:a",
                "imaging",
                RetrievalEvidenceState::Supported,
                false,
            ),
            candidate(
                "evidence:omics",
                "study:a",
                "omics",
                RetrievalEvidenceState::Supported,
                true,
            ),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, RetrievalAssuranceDisposition::Passed);
        assert_eq!(receipt.summary.selected_count, 2);
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|item| item.contains("negative-result")));
        assert!(receipt
            .effect_receipts
            .iter()
            .any(|item| item.starts_with("evaluate:retrieval-assurance:")));
    }

    #[test]
    fn missing_required_modality_is_unknown_and_not_released() {
        let receipt = assure_retrieval_synthesis(&request(vec![candidate(
            "evidence:imaging",
            "study:a",
            "imaging",
            RetrievalEvidenceState::Supported,
            false,
        )]))
        .unwrap();
        assert_eq!(
            receipt.disposition,
            RetrievalAssuranceDisposition::Conditional
        );
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item.contains("modality:omics")));
        assert!(receipt
            .effect_receipts
            .iter()
            .any(|item| item == "block:unsafe-release"));
    }

    #[test]
    fn contradictory_candidate_is_blocked_with_negative_evidence() {
        let receipt = assure_retrieval_synthesis(&request(vec![
            candidate(
                "evidence:imaging",
                "study:a",
                "imaging",
                RetrievalEvidenceState::Supported,
                false,
            ),
            candidate(
                "evidence:omics",
                "study:a",
                "omics",
                RetrievalEvidenceState::Contradicted,
                false,
            ),
        ]))
        .unwrap();
        assert_eq!(receipt.summary.blocked_count, 1);
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|item| item.contains("contradicted")));
        assert_ne!(receipt.disposition, RetrievalAssuranceDisposition::Passed);
    }

    #[test]
    fn policy_denial_blocks_unsafe_release() {
        let mut request = request(vec![
            candidate(
                "evidence:imaging",
                "study:a",
                "imaging",
                RetrievalEvidenceState::Supported,
                false,
            ),
            candidate(
                "evidence:omics",
                "study:a",
                "omics",
                RetrievalEvidenceState::Supported,
                false,
            ),
        ]);
        request.policy_allow = false;
        let receipt = assure_retrieval_synthesis(&request).unwrap();
        assert_eq!(receipt.disposition, RetrievalAssuranceDisposition::Blocked);
        assert!(receipt
            .effect_receipts
            .iter()
            .any(|item| item == "block:unsafe-release"));
    }

    #[test]
    fn duplicate_candidates_are_rejected() {
        let result = assure_retrieval_synthesis(&request(vec![
            candidate(
                "evidence:duplicate",
                "study:a",
                "imaging",
                RetrievalEvidenceState::Supported,
                false,
            ),
            candidate(
                "evidence:duplicate",
                "study:a",
                "omics",
                RetrievalEvidenceState::Supported,
                false,
            ),
        ]));
        assert!(result.is_err());
    }

    #[test]
    fn non_local_raw_data_blocks_release_without_emitting_raw_data() {
        let mut request = request(vec![candidate(
            "evidence:imaging",
            "study:a",
            "imaging",
            RetrievalEvidenceState::Supported,
            false,
        )]);
        request.raw_data_local = false;
        let receipt = assure_retrieval_synthesis(&request).unwrap();
        assert_eq!(receipt.disposition, RetrievalAssuranceDisposition::Blocked);
        assert!(receipt.raw_data_local);
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item == "request:raw-data-locality-required"));
    }
}
