//! Local single-study analysis model portfolio qualification.
//!
//! Atlas feature: `AFA-adapter-P13-F01`.
//!
//! This product compares caller-declared statistical or causal analysis candidates against a
//! typed estimand, method allow-list, artifact coverage, identification status, uncertainty, and
//! negative evidence. It does not fit a model or infer biology; it emits a deterministic
//! qualification artifact that a separately governed analysis runner can consume.

use bioprism_foundation::{
    LossSeverity, ProvenanceLink, SemanticLoss, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-adapter-P13-F01";
pub const CONTRACT_VERSION: &str = "local-analysis-model-portfolio/1.0";
const MAX_TEXT_BYTES: usize = 512;
const MAX_ITEMS: usize = 16_384;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisQuestion {
    pub question_id: String,
    pub intent: String,
    pub estimand: String,
    pub required_artifact_digests: Vec<ContentHash>,
    pub allowed_methods: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum IdentificationStatus {
    Identified,
    PartiallyIdentified,
    Unidentified,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisCandidate {
    pub candidate_id: String,
    pub method: String,
    pub estimand: String,
    pub assumptions: Vec<String>,
    pub effect_estimate: String,
    pub uncertainty: String,
    pub selection_score: f64,
    pub input_artifacts: Vec<ContentHash>,
    pub identification: IdentificationStatus,
    pub negative_evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisPortfolioRequest {
    pub question: AnalysisQuestion,
    pub candidates: Vec<AnalysisCandidate>,
    pub protected_omissions: Vec<String>,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisPortfolioVerdict {
    Qualified,
    Conditional,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AnalysisPortfolioReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub input: AnalysisPortfolioRequest,
    pub input_digest: ContentHash,
    pub question_id: String,
    pub intent: String,
    pub estimand: String,
    pub allowed_methods: Vec<String>,
    pub required_artifact_digests: Vec<ContentHash>,
    pub verdict: AnalysisPortfolioVerdict,
    pub selected_candidate: Option<String>,
    pub candidate_order: Vec<String>,
    pub candidate_score_order: Vec<f64>,
    pub candidate_identification_order: Vec<IdentificationStatus>,
    pub question_digest: ContentHash,
    pub candidate_digest: ContentHash,
    pub uncertainty: Vec<String>,
    pub omissions: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub semantic_loss: Vec<SemanticLoss>,
    pub reasons: Vec<String>,
    pub effect_receipt: String,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

fn validate_text(field: &str, value: &str) -> Result<(), AnalysisPortfolioError> {
    if value.is_empty() || value.trim() != value {
        return Err(AnalysisPortfolioError::InvalidRequest(format!(
            "{field} must be non-empty and trimmed"
        )));
    }
    if value.len() > MAX_TEXT_BYTES || value.chars().any(char::is_control) {
        return Err(AnalysisPortfolioError::InvalidRequest(format!(
            "{field} is outside its bounded text contract"
        )));
    }
    Ok(())
}

fn validate_unique_strings(field: &str, values: &[String]) -> Result<(), AnalysisPortfolioError> {
    if values.len() > MAX_ITEMS {
        return Err(AnalysisPortfolioError::InvalidRequest(format!(
            "{field} exceeds its item bound"
        )));
    }
    let mut unique = BTreeSet::new();
    for value in values {
        validate_text(field, value)?;
        if !unique.insert(value) {
            return Err(AnalysisPortfolioError::InvalidRequest(format!(
                "{field} contains duplicate values"
            )));
        }
    }
    Ok(())
}

fn validate_sorted_strings(field: &str, values: &[String]) -> Result<(), AnalysisPortfolioError> {
    validate_unique_strings(field, values)?;
    if values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(AnalysisPortfolioError::InvalidRequest(format!(
            "{field} ordering is not canonical"
        )));
    }
    Ok(())
}

fn validate_digest(field: &str, digest: &ContentHash) -> Result<(), AnalysisPortfolioError> {
    if digest.as_str().len() != 64
        || !digest
            .as_str()
            .chars()
            .all(|character| character.is_ascii_hexdigit())
    {
        return Err(AnalysisPortfolioError::InvalidRequest(format!(
            "{field} must be a 64-character hex digest"
        )));
    }
    Ok(())
}

fn analysis_portfolio_input_digest(
    request: &AnalysisPortfolioRequest,
) -> Result<ContentHash, AnalysisPortfolioError> {
    let value = serde_json::to_value(&canonical_analysis_portfolio_request(request))
        .map_err(|error| AnalysisPortfolioError::Serialization(error.to_string()))?;
    ContentHash::of_value(&value)
        .map_err(|error| AnalysisPortfolioError::Serialization(error.to_string()))
}

fn canonical_analysis_portfolio_request(
    request: &AnalysisPortfolioRequest,
) -> AnalysisPortfolioRequest {
    let mut canonical = request.clone();
    canonical.question.required_artifact_digests.sort();
    canonical.question.allowed_methods.sort();
    canonical.protected_omissions.sort();
    for candidate in &mut canonical.candidates {
        candidate.assumptions.sort();
        candidate.input_artifacts.sort();
        candidate.negative_evidence.sort();
    }
    canonical
        .candidates
        .sort_by(|left, right| left.candidate_id.cmp(&right.candidate_id));
    canonical
}

fn validate_sorted_digests(
    field: &str,
    digests: &[ContentHash],
) -> Result<(), AnalysisPortfolioError> {
    if digests.len() > MAX_ITEMS {
        return Err(AnalysisPortfolioError::InvalidRequest(format!(
            "{field} exceeds its item bound"
        )));
    }
    for digest in digests {
        validate_digest(field, digest)?;
    }
    if digests
        .windows(2)
        .any(|pair| pair[0].as_str() >= pair[1].as_str())
    {
        return Err(AnalysisPortfolioError::InvalidRequest(format!(
            "{field} ordering is not canonical"
        )));
    }
    Ok(())
}

fn validate_unique_digests(
    field: &str,
    digests: &[ContentHash],
) -> Result<(), AnalysisPortfolioError> {
    if digests.len() > MAX_ITEMS {
        return Err(AnalysisPortfolioError::InvalidRequest(format!(
            "{field} exceeds its item bound"
        )));
    }
    let mut unique = BTreeSet::new();
    for digest in digests {
        validate_digest(field, digest)?;
        if !unique.insert(digest.as_str().to_string()) {
            return Err(AnalysisPortfolioError::InvalidRequest(format!(
                "{field} contains duplicate values"
            )));
        }
    }
    Ok(())
}

impl AnalysisPortfolioReceipt {
    pub fn validate(&self) -> Result<(), AnalysisPortfolioError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
        {
            return Err(AnalysisPortfolioError::Contract(
                "analysis portfolio identity mismatch".into(),
            ));
        }
        if self.question_id.trim().is_empty()
            || self.intent.trim().is_empty()
            || self.estimand.trim().is_empty()
            || self.allowed_methods.is_empty()
            || self.required_artifact_digests.is_empty()
            || self.candidate_order.is_empty()
            || self.candidate_score_order.is_empty()
            || self.candidate_identification_order.is_empty()
            || self.reasons.is_empty()
            || !self.raw_data_local
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.effect_receipt.is_empty()
        {
            return Err(AnalysisPortfolioError::InvalidRequest(
                "analysis identity, candidates, reasons, locality, and boundary are required"
                    .into(),
            ));
        }
        validate_text("question_id", &self.question_id)?;
        validate_text("intent", &self.intent)?;
        validate_text("estimand", &self.estimand)?;
        validate_text("boundary", &self.boundary)?;
        validate_sorted_strings("allowed_methods", &self.allowed_methods)?;
        validate_sorted_digests("required_artifact_digests", &self.required_artifact_digests)?;
        validate_unique_strings("candidate_order", &self.candidate_order)?;
        validate_unique_strings("uncertainty", &self.uncertainty)?;
        validate_sorted_strings("omissions", &self.omissions)?;
        validate_unique_strings("negative_evidence", &self.negative_evidence)?;
        validate_unique_strings("reasons", &self.reasons)?;
        validate_text("effect_receipt", &self.effect_receipt)?;
        if self.candidate_order.len() != self.candidate_score_order.len()
            || self.candidate_order.len() != self.candidate_identification_order.len()
        {
            return Err(AnalysisPortfolioError::InvalidRequest(
                "candidate ordering vectors must have equal length".into(),
            ));
        }
        for score in &self.candidate_score_order {
            if !score.is_finite() || !(0.0..=1.0).contains(score) {
                return Err(AnalysisPortfolioError::InvalidScore(
                    "receipt candidate score".into(),
                ));
            }
        }
        for pair in self
            .candidate_score_order
            .windows(2)
            .zip(self.candidate_order.windows(2))
        {
            let (scores, ids) = pair;
            if scores[0] < scores[1]
                || ((scores[0] - scores[1]).abs() < f64::EPSILON && ids[0] >= ids[1])
            {
                return Err(AnalysisPortfolioError::InvalidRequest(
                    "candidate order must follow descending score and ascending tie-break id"
                        .into(),
                ));
            }
        }
        if self.candidate_order.len() > 1
            && (self.candidate_score_order[0] - self.candidate_score_order[1]).abs() < f64::EPSILON
        {
            return Err(AnalysisPortfolioError::SelectionTie);
        }
        let expected_selected =
            if self.candidate_identification_order[0] == IdentificationStatus::Unidentified {
                None
            } else {
                Some(self.candidate_order[0].clone())
            };
        if self.selected_candidate != expected_selected {
            return Err(AnalysisPortfolioError::InvalidRequest(
                "selected candidate does not match score and identification closure".into(),
            ));
        }
        if let Some(selected) = &self.selected_candidate {
            if !self.candidate_order.contains(selected) {
                return Err(AnalysisPortfolioError::InvalidRequest(
                    "selected candidate must be in the candidate closure".into(),
                ));
            }
        }
        let expected_verdict = if self.selected_candidate.is_none() {
            AnalysisPortfolioVerdict::Blocked
        } else if !self.omissions.is_empty()
            || self.candidate_identification_order[0] == IdentificationStatus::PartiallyIdentified
        {
            AnalysisPortfolioVerdict::Conditional
        } else {
            AnalysisPortfolioVerdict::Qualified
        };
        if self.verdict != expected_verdict {
            return Err(AnalysisPortfolioError::InvalidRequest(
                "analysis portfolio verdict does not match selection and omissions".into(),
            ));
        }
        let expected_loss = if self.verdict == AnalysisPortfolioVerdict::Blocked {
            vec![SemanticLoss {
                field: "identification".into(),
                reason: "no candidate supports the requested estimand under declared assumptions"
                    .into(),
                severity: LossSeverity::DecisionRelevant,
            }]
        } else {
            Vec::new()
        };
        if self.semantic_loss != expected_loss {
            return Err(AnalysisPortfolioError::Contract(
                "analysis semantic-loss closure does not match verdict".into(),
            ));
        }
        if self.uncertainty.len() != self.candidate_order.len()
            || self.uncertainty.iter().any(|entry| {
                !self
                    .candidate_order
                    .iter()
                    .any(|id| entry.starts_with(&format!("{id}: ")))
            })
            || self.negative_evidence.iter().any(|entry| {
                !self
                    .candidate_order
                    .iter()
                    .any(|id| entry.starts_with(&format!("{id}: ")))
            })
        {
            return Err(AnalysisPortfolioError::InvalidRequest(
                "candidate uncertainty and negative evidence are not scoped".into(),
            ));
        }
        let mut expected_reasons = vec![format!(
            "candidate order is deterministic by score and candidate id; {} candidates evaluated",
            self.candidate_order.len()
        )];
        if self.selected_candidate.is_none() {
            expected_reasons
                .push("no candidate has sufficient identification for qualification".into());
        }
        if !self.omissions.is_empty() {
            expected_reasons
                .push("protected omissions prevent unconditional analytical qualification".into());
        }
        if !self.negative_evidence.is_empty() {
            expected_reasons.push("negative evidence is retained as a first-class result".into());
        }
        if self.reasons != expected_reasons {
            return Err(AnalysisPortfolioError::InvalidRequest(
                "analysis portfolio reasons are not bound to the result".into(),
            ));
        }
        if self.effect_receipt != format!("read:local-analysis-portfolio:{}", self.question_id) {
            return Err(AnalysisPortfolioError::Contract(
                "analysis portfolio effect is outside the local-read gate".into(),
            ));
        }
        validate_digest("question_digest", &self.question_digest)?;
        validate_digest("candidate_digest", &self.candidate_digest)?;
        let expected_question = ContentHash::of_value(&json!({
            "question_id": self.question_id,
            "intent": self.intent,
            "estimand": self.estimand,
            "required_artifact_digests": self.required_artifact_digests,
            "allowed_methods": self.allowed_methods,
        }))
        .map_err(|error| AnalysisPortfolioError::Serialization(error.to_string()))?;
        if self.question_digest != expected_question {
            return Err(AnalysisPortfolioError::Contract(
                "question digest does not match the portfolio question".into(),
            ));
        }
        let expected_provenance = self
            .required_artifact_digests
            .iter()
            .enumerate()
            .map(|(index, digest)| ProvenanceLink {
                source_id: format!("analysis-input-{index}"),
                relation: "qualified-from-local-artifact".into(),
                digest: digest.clone(),
            })
            .collect::<Vec<_>>();
        if self.artifact.artifact_id != format!("analysis-portfolio:{}", self.question_id)
            || self.artifact.content_type != "application/vnd.aurora.analysis-portfolio+json"
            || self.artifact.semantic_loss != self.semantic_loss
            || self.artifact.provenance != expected_provenance
        {
            return Err(AnalysisPortfolioError::Contract(
                "analysis portfolio artifact is not bound to the receipt".into(),
            ));
        }
        let payload = json!({
            "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
            "contract_version": CONTRACT_VERSION,
            "feature_id": FEATURE_ID,
            "question_id": self.question_id,
            "intent": self.intent,
            "estimand": self.estimand,
            "allowed_methods": self.allowed_methods,
            "required_artifact_digests": self.required_artifact_digests,
            "verdict": self.verdict,
            "selected_candidate": self.selected_candidate,
            "candidate_order": self.candidate_order,
            "candidate_score_order": self.candidate_score_order,
            "candidate_identification_order": self.candidate_identification_order,
            "question_digest": self.question_digest,
            "candidate_digest": self.candidate_digest,
            "uncertainty": self.uncertainty,
            "omissions": self.omissions,
            "negative_evidence": self.negative_evidence,
            "semantic_loss": self.semantic_loss,
            "reasons": self.reasons,
            "effect_receipt": self.effect_receipt,
            "raw_data_local": self.raw_data_local,
            "boundary": PRECLINICAL_BOUNDARY,
        });
        self.artifact
            .verify_payload(&payload)
            .map_err(|error| AnalysisPortfolioError::Contract(error.to_string()))?;
        self.artifact
            .validate_metadata()
            .map_err(|error| AnalysisPortfolioError::Contract(error.to_string()))?;
        validate_request(&self.input)?;
        if self.input_digest != analysis_portfolio_input_digest(&self.input)? {
            return Err(AnalysisPortfolioError::Contract(
                "analysis portfolio retained input digest does not match the request".into(),
            ));
        }
        let expected = build_analysis_portfolio(&self.input)?;
        if self != &expected {
            return Err(AnalysisPortfolioError::Contract(
                "analysis portfolio receipt is not derived from its retained request".into(),
            ));
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, AnalysisPortfolioError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| AnalysisPortfolioError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| AnalysisPortfolioError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum AnalysisPortfolioError {
    #[error("invalid analysis portfolio request: {0}")]
    InvalidRequest(String),
    #[error("analysis portfolio contract rejected: {0}")]
    Contract(String),
    #[error("duplicate analysis candidate {0}")]
    DuplicateCandidate(String),
    #[error("analysis method is not allowed: {0}")]
    MethodNotAllowed(String),
    #[error("analysis candidate lacks required artifact coverage: {0}")]
    MissingArtifactCoverage(String),
    #[error("analysis candidate score is invalid: {0}")]
    InvalidScore(String),
    #[error("analysis candidate selection tie")]
    SelectionTie,
    #[error("analysis portfolio serialization failed: {0}")]
    Serialization(String),
}

pub fn qualify_analysis_portfolio(
    request: &AnalysisPortfolioRequest,
) -> Result<AnalysisPortfolioReceipt, AnalysisPortfolioError> {
    let receipt = build_analysis_portfolio(request)?;
    receipt.validate()?;
    Ok(receipt)
}

fn build_analysis_portfolio(
    request: &AnalysisPortfolioRequest,
) -> Result<AnalysisPortfolioReceipt, AnalysisPortfolioError> {
    validate_request(request)?;
    let mut allowed_methods = request.question.allowed_methods.clone();
    allowed_methods.sort();
    let mut required_artifact_digests = request.question.required_artifact_digests.clone();
    required_artifact_digests.sort_by(|left, right| left.as_str().cmp(right.as_str()));
    let mut candidates = request.candidates.clone();
    for candidate in &mut candidates {
        candidate.assumptions.sort();
        candidate
            .input_artifacts
            .sort_by(|left, right| left.as_str().cmp(right.as_str()));
        candidate.negative_evidence.sort();
    }
    candidates.sort_by(|left, right| {
        right
            .selection_score
            .partial_cmp(&left.selection_score)
            .unwrap_or(std::cmp::Ordering::Equal)
            .then(left.candidate_id.cmp(&right.candidate_id))
    });
    if candidates.len() > 1
        && (candidates[0].selection_score - candidates[1].selection_score).abs() < f64::EPSILON
    {
        return Err(AnalysisPortfolioError::SelectionTie);
    }
    let selected = candidates
        .first()
        .filter(|candidate| candidate.identification != IdentificationStatus::Unidentified)
        .map(|candidate| candidate.candidate_id.clone());
    let candidate_order = candidates
        .iter()
        .map(|candidate| candidate.candidate_id.clone())
        .collect::<Vec<_>>();
    let candidate_score_order = candidates
        .iter()
        .map(|candidate| candidate.selection_score)
        .collect::<Vec<_>>();
    let candidate_identification_order = candidates
        .iter()
        .map(|candidate| candidate.identification)
        .collect::<Vec<_>>();
    let question_digest = ContentHash::of_value(&json!({
        "question_id": request.question.question_id,
        "intent": request.question.intent,
        "estimand": request.question.estimand,
        "required_artifact_digests": required_artifact_digests.clone(),
        "allowed_methods": allowed_methods.clone(),
    }))
    .map_err(|error| AnalysisPortfolioError::Serialization(error.to_string()))?;
    let candidate_digest = ContentHash::of_value(
        &serde_json::to_value(&candidates)
            .map_err(|error| AnalysisPortfolioError::Serialization(error.to_string()))?,
    )
    .map_err(|error| AnalysisPortfolioError::Serialization(error.to_string()))?;
    let uncertainty = candidates
        .iter()
        .map(|candidate| format!("{}: {}", candidate.candidate_id, candidate.uncertainty))
        .collect::<Vec<_>>();
    let negative_evidence = candidates
        .iter()
        .flat_map(|candidate| {
            candidate
                .negative_evidence
                .iter()
                .map(move |evidence| format!("{}: {}", candidate.candidate_id, evidence))
        })
        .collect::<Vec<_>>();
    let verdict = if selected.is_none() {
        AnalysisPortfolioVerdict::Blocked
    } else if !request.protected_omissions.is_empty()
        || candidates.first().is_some_and(|candidate| {
            candidate.identification == IdentificationStatus::PartiallyIdentified
        })
    {
        AnalysisPortfolioVerdict::Conditional
    } else {
        AnalysisPortfolioVerdict::Qualified
    };
    let mut reasons = vec![format!(
        "candidate order is deterministic by score and candidate id; {} candidates evaluated",
        candidates.len()
    )];
    if selected.is_none() {
        reasons.push("no candidate has sufficient identification for qualification".into());
    }
    if !request.protected_omissions.is_empty() {
        reasons.push("protected omissions prevent unconditional analytical qualification".into());
    }
    if !negative_evidence.is_empty() {
        reasons.push("negative evidence is retained as a first-class result".into());
    }
    let semantic_loss = if verdict == AnalysisPortfolioVerdict::Blocked {
        vec![SemanticLoss {
            field: "identification".into(),
            reason: "no candidate supports the requested estimand under declared assumptions"
                .into(),
            severity: LossSeverity::DecisionRelevant,
        }]
    } else {
        Vec::new()
    };
    let mut omissions = request.protected_omissions.clone();
    omissions.sort();
    let effect_receipt = format!(
        "read:local-analysis-portfolio:{}",
        request.question.question_id
    );
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "question_id": request.question.question_id,
        "intent": request.question.intent,
        "estimand": request.question.estimand,
        "allowed_methods": allowed_methods,
        "required_artifact_digests": required_artifact_digests,
        "verdict": verdict,
        "selected_candidate": selected,
        "candidate_order": candidate_order,
        "candidate_score_order": candidate_score_order,
        "candidate_identification_order": candidate_identification_order,
        "question_digest": question_digest,
        "candidate_digest": candidate_digest,
        "uncertainty": uncertainty,
        "omissions": omissions,
        "negative_evidence": negative_evidence,
        "semantic_loss": semantic_loss,
        "reasons": reasons,
        "effect_receipt": effect_receipt,
        "raw_data_local": true,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let provenance = required_artifact_digests
        .iter()
        .enumerate()
        .map(|(index, digest)| ProvenanceLink {
            source_id: format!("analysis-input-{index}"),
            relation: "qualified-from-local-artifact".into(),
            digest: digest.clone(),
        })
        .collect();
    let artifact = TypedResearchArtifact::from_payload(
        format!("analysis-portfolio:{}", request.question.question_id),
        "application/vnd.aurora.analysis-portfolio+json",
        &payload,
        semantic_loss.clone(),
        provenance,
    )
    .map_err(|error| AnalysisPortfolioError::Contract(error.to_string()))?;
    let receipt = AnalysisPortfolioReceipt {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        input: canonical_analysis_portfolio_request(request),
        input_digest: analysis_portfolio_input_digest(request)?,
        question_id: request.question.question_id.clone(),
        intent: request.question.intent.clone(),
        estimand: request.question.estimand.clone(),
        allowed_methods: allowed_methods.clone(),
        required_artifact_digests: required_artifact_digests.clone(),
        verdict,
        selected_candidate: selected,
        candidate_order: candidates
            .iter()
            .map(|candidate| candidate.candidate_id.clone())
            .collect(),
        candidate_score_order,
        candidate_identification_order,
        question_digest,
        candidate_digest,
        uncertainty,
        omissions,
        negative_evidence,
        semantic_loss,
        reasons,
        effect_receipt,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    Ok(receipt)
}

fn validate_request(request: &AnalysisPortfolioRequest) -> Result<(), AnalysisPortfolioError> {
    let question = &request.question;
    if question.question_id.trim().is_empty()
        || question.intent.trim().is_empty()
        || question.estimand.trim().is_empty()
        || question.required_artifact_digests.is_empty()
        || question.allowed_methods.is_empty()
        || request.candidates.is_empty()
        || !request.raw_data_local
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(AnalysisPortfolioError::InvalidRequest("question, estimand, required artifacts, methods, candidates, locality, and boundary are required".into()));
    }
    validate_text("question.question_id", &question.question_id)?;
    validate_text("question.intent", &question.intent)?;
    validate_text("question.estimand", &question.estimand)?;
    validate_unique_digests(
        "question.required_artifact_digests",
        &question.required_artifact_digests,
    )?;
    validate_unique_strings("question.allowed_methods", &question.allowed_methods)?;
    validate_unique_strings("protected_omissions", &request.protected_omissions)?;
    if request.candidates.len() > MAX_ITEMS {
        return Err(AnalysisPortfolioError::InvalidRequest(
            "candidates exceeds its item bound".into(),
        ));
    }
    let allowed = question.allowed_methods.iter().collect::<BTreeSet<_>>();
    let required = question
        .required_artifact_digests
        .iter()
        .collect::<BTreeSet<_>>();
    let mut ids = BTreeSet::new();
    for candidate in &request.candidates {
        if candidate.candidate_id.trim().is_empty()
            || candidate.method.trim().is_empty()
            || candidate.estimand.trim().is_empty()
            || candidate.assumptions.is_empty()
            || candidate.effect_estimate.trim().is_empty()
            || candidate.uncertainty.trim().is_empty()
        {
            return Err(AnalysisPortfolioError::InvalidRequest("candidate identity, method, estimand, assumptions, estimate, and uncertainty are required".into()));
        }
        validate_text("candidate.candidate_id", &candidate.candidate_id)?;
        validate_text("candidate.method", &candidate.method)?;
        validate_text("candidate.estimand", &candidate.estimand)?;
        validate_unique_strings("candidate.assumptions", &candidate.assumptions)?;
        validate_text("candidate.effect_estimate", &candidate.effect_estimate)?;
        validate_text("candidate.uncertainty", &candidate.uncertainty)?;
        validate_unique_strings("candidate.negative_evidence", &candidate.negative_evidence)?;
        validate_unique_digests("candidate.input_artifacts", &candidate.input_artifacts)?;
        if !ids.insert(candidate.candidate_id.clone()) {
            return Err(AnalysisPortfolioError::DuplicateCandidate(
                candidate.candidate_id.clone(),
            ));
        }
        if !allowed.contains(&candidate.method) {
            return Err(AnalysisPortfolioError::MethodNotAllowed(
                candidate.method.clone(),
            ));
        }
        if candidate.estimand != question.estimand {
            return Err(AnalysisPortfolioError::InvalidRequest(format!(
                "candidate {} estimand differs from question",
                candidate.candidate_id
            )));
        }
        if !candidate.selection_score.is_finite()
            || !(0.0..=1.0).contains(&candidate.selection_score)
        {
            return Err(AnalysisPortfolioError::InvalidScore(
                candidate.candidate_id.clone(),
            ));
        }
        if !required.is_subset(&candidate.input_artifacts.iter().collect::<BTreeSet<_>>()) {
            return Err(AnalysisPortfolioError::MissingArtifactCoverage(
                candidate.candidate_id.clone(),
            ));
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

    fn request() -> AnalysisPortfolioRequest {
        let digest = ContentHash::of_bytes(b"local-study");
        AnalysisPortfolioRequest {
            question: AnalysisQuestion {
                question_id: "question:effect".into(),
                intent: "compare perturbation and control".into(),
                estimand: "average treatment effect in organoid model".into(),
                required_artifact_digests: vec![digest.clone()],
                allowed_methods: vec!["doubly-robust".into(), "descriptive".into()],
            },
            candidates: vec![
                AnalysisCandidate {
                    candidate_id: "candidate-b".into(),
                    method: "descriptive".into(),
                    estimand: "average treatment effect in organoid model".into(),
                    assumptions: vec!["measurement comparable".into()],
                    effect_estimate: "0.2".into(),
                    uncertainty: "interval [0.0,0.4]".into(),
                    selection_score: 0.55,
                    input_artifacts: vec![digest.clone()],
                    identification: IdentificationStatus::PartiallyIdentified,
                    negative_evidence: vec!["null replication not available".into()],
                },
                AnalysisCandidate {
                    candidate_id: "candidate-a".into(),
                    method: "doubly-robust".into(),
                    estimand: "average treatment effect in organoid model".into(),
                    assumptions: vec!["declared confounding scope".into()],
                    effect_estimate: "0.3".into(),
                    uncertainty: "interval [0.1,0.5]".into(),
                    selection_score: 0.8,
                    input_artifacts: vec![digest],
                    identification: IdentificationStatus::Identified,
                    negative_evidence: Vec::new(),
                },
            ],
            protected_omissions: Vec::new(),
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn portfolio_selection_is_deterministic_and_retains_negative_evidence() {
        let mut reversed = request();
        reversed.candidates.reverse();
        let left = qualify_analysis_portfolio(&request()).unwrap();
        let right = qualify_analysis_portfolio(&reversed).unwrap();
        assert_eq!(left.digest().unwrap(), right.digest().unwrap());
        assert_eq!(left.selected_candidate.as_deref(), Some("candidate-a"));
        assert_eq!(left.verdict, AnalysisPortfolioVerdict::Qualified);
        assert!(left
            .reasons
            .iter()
            .any(|reason| reason.contains("negative evidence")));
    }
    #[test]
    fn protected_omission_is_conditional() {
        let mut request = request();
        request
            .protected_omissions
            .push("missing independent site".into());
        let receipt = qualify_analysis_portfolio(&request).unwrap();
        assert_eq!(receipt.verdict, AnalysisPortfolioVerdict::Conditional);
    }
    #[test]
    fn unidentified_candidates_block() {
        let mut request = request();
        request
            .candidates
            .iter_mut()
            .for_each(|candidate| candidate.identification = IdentificationStatus::Unidentified);
        let receipt = qualify_analysis_portfolio(&request).unwrap();
        assert_eq!(receipt.verdict, AnalysisPortfolioVerdict::Blocked);
        assert!(receipt.selected_candidate.is_none());
    }
    #[test]
    fn method_and_artifact_gates_fail_closed() {
        let mut request = request();
        request.candidates[0].method = "forbidden".into();
        assert!(qualify_analysis_portfolio(&request).is_err());
    }

    #[test]
    fn nested_input_order_is_canonicalized() {
        let mut reversed = request();
        reversed.question.allowed_methods.reverse();
        reversed.candidates[0].assumptions.reverse();
        reversed.candidates[0].negative_evidence.reverse();
        reversed.candidates[0].input_artifacts.reverse();
        assert_eq!(
            qualify_analysis_portfolio(&request())
                .unwrap()
                .digest()
                .unwrap(),
            qualify_analysis_portfolio(&reversed)
                .unwrap()
                .digest()
                .unwrap()
        );
    }

    #[test]
    fn score_order_tampering_is_rejected() {
        let mut receipt = qualify_analysis_portfolio(&request()).unwrap();
        receipt.candidate_score_order.reverse();
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn candidate_digest_tampering_is_rejected() {
        let mut receipt = qualify_analysis_portfolio(&request()).unwrap();
        receipt.candidate_digest = hash("tampered-candidates");
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn artifact_payload_tampering_is_rejected() {
        let mut receipt = qualify_analysis_portfolio(&request()).unwrap();
        receipt.artifact.content_hash = hash("tampered-payload");
        assert!(receipt.validate().is_err());
    }

    #[test]
    fn retained_request_tampering_is_rejected() {
        let mut receipt = qualify_analysis_portfolio(&request()).unwrap();
        receipt.input.question.intent = "tampered intent".into();
        assert!(receipt.validate().is_err());
    }
}
