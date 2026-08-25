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
    pub question_id: String,
    pub estimand: String,
    pub verdict: AnalysisPortfolioVerdict,
    pub selected_candidate: Option<String>,
    pub candidate_order: Vec<String>,
    pub uncertainty: Vec<String>,
    pub omissions: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub semantic_loss: Vec<SemanticLoss>,
    pub reasons: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
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
            || self.estimand.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.reasons.is_empty()
            || !self.raw_data_local
            || self.boundary != PRECLINICAL_BOUNDARY
        {
            return Err(AnalysisPortfolioError::InvalidRequest(
                "analysis identity, candidates, reasons, locality, and boundary are required"
                    .into(),
            ));
        }
        if self
            .candidate_order
            .windows(2)
            .any(|pair| pair[0] > pair[1])
            || self.candidate_order.iter().collect::<BTreeSet<_>>().len()
                != self.candidate_order.len()
        {
            return Err(AnalysisPortfolioError::InvalidRequest(
                "candidate order must be canonical and unique".into(),
            ));
        }
        if self.verdict == AnalysisPortfolioVerdict::Qualified && self.selected_candidate.is_none()
        {
            return Err(AnalysisPortfolioError::InvalidRequest(
                "qualified portfolio needs a selected candidate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| AnalysisPortfolioError::Contract(error.to_string()))?;
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
    validate_request(request)?;
    let mut candidates = request.candidates.clone();
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
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "question_id": request.question.question_id,
        "estimand": request.question.estimand,
        "verdict": verdict,
        "selected_candidate": selected,
        "candidate_order": candidates.iter().map(|candidate| candidate.candidate_id.clone()).collect::<Vec<_>>(),
        "uncertainty": uncertainty,
        "omissions": request.protected_omissions,
        "negative_evidence": negative_evidence,
        "semantic_loss": semantic_loss,
        "reasons": reasons,
        "raw_data_local": true,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let provenance = request
        .question
        .required_artifact_digests
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
        question_id: request.question.question_id.clone(),
        estimand: request.question.estimand.clone(),
        verdict,
        selected_candidate: selected,
        candidate_order: candidates
            .iter()
            .map(|candidate| candidate.candidate_id.clone())
            .collect(),
        uncertainty,
        omissions: request.protected_omissions.clone(),
        negative_evidence,
        semantic_loss,
        reasons,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
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
}
