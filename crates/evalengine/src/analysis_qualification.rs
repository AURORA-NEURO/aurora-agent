//! Omission-aware qualification of declared statistical and causal analysis candidates.
//!
//! Atlas feature: `AFA-evalengine-P13-F01`.
//!
//! This product does not fit a model or assert biological truth. It verifies that a caller-supplied
//! analysis candidate has a declared estimand, assumptions, uncertainty, input-artifact coverage,
//! and identification status, then emits a deterministic qualification receipt. Protected
//! omissions and unidentified candidates can only lower the verdict.

use bioprism_foundation::{
    AutonomyTier, AuthorityRequirement, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-evalengine-P13-F01";
pub const FEATURE_CONTRACT_VERSION: &str = "0.1.0";

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
pub struct AnalysisQualificationRequest {
    pub question: AnalysisQuestion,
    pub candidates: Vec<AnalysisCandidate>,
    pub protected_omissions: Vec<String>,
    pub raw_data_local: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualificationVerdict {
    Qualified,
    Conditional,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct QualifiedAnalysisResult {
    pub schema_version: String,
    pub feature_id: String,
    pub question_id: String,
    pub estimand: String,
    pub verdict: QualificationVerdict,
    pub selected_candidate: Option<String>,
    pub candidate_order: Vec<String>,
    pub uncertainty: Vec<String>,
    pub omissions: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub reasons: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

impl QualifiedAnalysisResult {
    pub fn validate(&self) -> Result<(), AnalysisQualificationError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION || self.feature_id != FEATURE_ID {
            return Err(AnalysisQualificationError::Contract(
                "analysis qualification schema or feature mismatch".into(),
            ));
        }
        if self.question_id.trim().is_empty() || self.estimand.trim().is_empty() || !self.raw_data_local {
            return Err(AnalysisQualificationError::InvalidRequest(
                "question, estimand, and local-data declaration are required".into(),
            ));
        }
        if self.candidate_order.is_empty() || self.reasons.is_empty() {
            return Err(AnalysisQualificationError::InvalidRequest(
                "candidate order and reasons are required".into(),
            ));
        }
        if self.verdict == QualificationVerdict::Qualified && self.selected_candidate.is_none() {
            return Err(AnalysisQualificationError::InvalidRequest(
                "qualified result needs a selected candidate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| AnalysisQualificationError::Contract(error.to_string()))?;
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, AnalysisQualificationError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| AnalysisQualificationError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| AnalysisQualificationError::Serialization(error.to_string()))
    }
}

#[derive(Debug, Error)]
pub enum AnalysisQualificationError {
    #[error("invalid analysis qualification request: {0}")]
    InvalidRequest(String),
    #[error("analysis qualification contract rejected: {0}")]
    Contract(String),
    #[error("duplicate analysis candidate {0}")]
    DuplicateCandidate(String),
    #[error("candidate uses a method not allowed by the question: {0}")]
    MethodNotAllowed(String),
    #[error("candidate does not cover required artifact inputs: {0}")]
    MissingArtifactCoverage(String),
    #[error("analysis candidate score is invalid: {0}")]
    InvalidScore(String),
    #[error("analysis candidate tie prevents deterministic selection")]
    SelectionTie,
    #[error("cannot serialize analysis qualification result: {0}")]
    Serialization(String),
}

pub fn analysis_qualification_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: FEATURE_CONTRACT_VERSION.into(),
        owner_crate: "evalengine".into(),
        consumers: ["benchmark curator".into(), "preclinical analyst".into()].into(),
        behavior: "qualifies declared statistical and causal analysis candidates against estimand, assumptions, uncertainty, input coverage, and identification status".into(),
        value: "prevents incomplete or unidentified analytical results from being presented as confident research conclusions".into(),
        inputs: vec![TypedPort {
            name: "analysis_qualification_request".into(),
            schema: "AnalysisQualificationRequest@1".into(),
            required: true,
        }],
        outputs: vec![TypedPort {
            name: "qualified_analysis_result".into(),
            schema: "QualifiedAnalysisResult@1".into(),
            required: true,
        }],
        effects: [Effect::ReadLocalData, Effect::WriteLocalArtifact, Effect::ExecuteLocalComputation]
            .into(),
        permissions: ["read:local-analysis-artifacts".into(), "write:local-analysis-receipt".into()]
            .into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference {
            source_id: "w3c-prov-o".into(),
            state: EvidenceState::Supported,
            locator: Some("https://www.w3.org/TR/prov-o/".into()),
        }],
        authority_requirements: vec![AuthorityRequirement {
            role: "analysis curator".into(),
            reason: "the estimand, allowed methods, and protected omissions are accountable research inputs".into(),
        }],
        autonomy_tier: AutonomyTier::A1,
        surfaces: [
            ResearchSurface::Cli,
            ResearchSurface::Api,
            ResearchSurface::Sdk,
            ResearchSurface::McpTool,
            ResearchSurface::Operator,
        ]
        .into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn qualify_analysis(
    request: &AnalysisQualificationRequest,
) -> Result<QualifiedAnalysisResult, AnalysisQualificationError> {
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
        return Err(AnalysisQualificationError::SelectionTie);
    }
    let top = candidates.first();
    let selected = top
        .filter(|candidate| candidate.identification != IdentificationStatus::Unidentified)
        .map(|candidate| candidate.candidate_id.clone());
    let mut uncertainty = candidates
        .iter()
        .map(|candidate| format!("{}: {}", candidate.candidate_id, candidate.uncertainty))
        .collect::<Vec<_>>();
    if let Some(candidate) = top.filter(|candidate| candidate.identification == IdentificationStatus::PartiallyIdentified) {
        uncertainty.push(format!("{}: identification is partial; causal interpretation is bounded", candidate.candidate_id));
    }
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
        QualificationVerdict::Blocked
    } else if !request.protected_omissions.is_empty()
        || top.is_some_and(|candidate| candidate.identification == IdentificationStatus::PartiallyIdentified)
    {
        QualificationVerdict::Conditional
    } else {
        QualificationVerdict::Qualified
    };
    let mut reasons = vec![format!(
        "candidate order is deterministic by declared selection score and candidate id; {} candidates evaluated",
        candidates.len()
    )];
    if selected.is_none() {
        reasons.push("no candidate has a sufficient identification status for qualification".into());
    }
    if !request.protected_omissions.is_empty() {
        reasons.push("protected omissions prevent an unconditional analytical qualification".into());
    }
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "feature_id": FEATURE_ID,
        "question_id": request.question.question_id,
        "estimand": request.question.estimand,
        "verdict": verdict,
        "selected_candidate": selected,
        "candidate_order": candidates.iter().map(|candidate| candidate.candidate_id.clone()).collect::<Vec<_>>(),
        "uncertainty": uncertainty,
        "omissions": request.protected_omissions,
        "negative_evidence": negative_evidence,
        "reasons": reasons,
        "raw_data_local": true,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let provenance = request
        .question
        .required_artifact_digests
        .iter()
        .enumerate()
        .map(|(index, digest)| bioprism_foundation::ProvenanceLink {
            source_id: format!("analysis-input-{index}"),
            relation: "qualified-from".into(),
            digest: digest.clone(),
        })
        .collect();
    let artifact = TypedResearchArtifact::from_payload(
        format!("analysis-qualified:{}", request.question.question_id),
        "application/vnd.aurora.qualified-analysis+json",
        &payload,
        Vec::new(),
        provenance,
    )
    .map_err(|error| AnalysisQualificationError::Contract(error.to_string()))?;
    let result = QualifiedAnalysisResult {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        question_id: request.question.question_id.clone(),
        estimand: request.question.estimand.clone(),
        verdict,
        selected_candidate: selected,
        candidate_order: candidates.iter().map(|candidate| candidate.candidate_id.clone()).collect(),
        uncertainty,
        omissions: request.protected_omissions.clone(),
        negative_evidence,
        reasons,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    result.validate()?;
    Ok(result)
}

fn validate_request(request: &AnalysisQualificationRequest) -> Result<(), AnalysisQualificationError> {
    let question = &request.question;
    if question.question_id.trim().is_empty()
        || question.intent.trim().is_empty()
        || question.estimand.trim().is_empty()
        || question.required_artifact_digests.is_empty()
        || question.allowed_methods.is_empty()
        || request.candidates.is_empty()
        || !request.raw_data_local
    {
        return Err(AnalysisQualificationError::InvalidRequest(
            "question identity, estimand, required artifacts, methods, candidates, and local-data declaration are required".into(),
        ));
    }
    let allowed = question.allowed_methods.iter().collect::<BTreeSet<_>>();
    let required = question.required_artifact_digests.iter().collect::<BTreeSet<_>>();
    let mut candidates = BTreeSet::new();
    for candidate in &request.candidates {
        if candidate.candidate_id.trim().is_empty()
            || candidate.method.trim().is_empty()
            || candidate.estimand.trim().is_empty()
            || candidate.assumptions.is_empty()
            || candidate.effect_estimate.trim().is_empty()
            || candidate.uncertainty.trim().is_empty()
        {
            return Err(AnalysisQualificationError::InvalidRequest(
                "candidate id, method, estimand, assumptions, estimate, and uncertainty are required".into(),
            ));
        }
        if !candidates.insert(candidate.candidate_id.clone()) {
            return Err(AnalysisQualificationError::DuplicateCandidate(
                candidate.candidate_id.clone(),
            ));
        }
        if !allowed.contains(&candidate.method) {
            return Err(AnalysisQualificationError::MethodNotAllowed(
                candidate.method.clone(),
            ));
        }
        if candidate.estimand != question.estimand {
            return Err(AnalysisQualificationError::InvalidRequest(format!(
                "candidate {} estimand differs from the question",
                candidate.candidate_id
            )));
        }
        if !candidate.selection_score.is_finite() || !(0.0..=1.0).contains(&candidate.selection_score) {
            return Err(AnalysisQualificationError::InvalidScore(candidate.candidate_id.clone()));
        }
        let inputs = candidate.input_artifacts.iter().collect::<BTreeSet<_>>();
        if !required.is_subset(&inputs) {
            return Err(AnalysisQualificationError::MissingArtifactCoverage(
                candidate.candidate_id.clone(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> AnalysisQualificationRequest {
        let digest = ContentHash::of_bytes(b"dataset");
        AnalysisQualificationRequest {
            question: AnalysisQuestion {
                question_id: "question:effect".into(),
                intent: "compare perturbation against control".into(),
                estimand: "average treatment effect in organoid model".into(),
                required_artifact_digests: vec![digest.clone()],
                allowed_methods: vec!["doubly-robust".into(), "descriptive".into()],
            },
            candidates: vec![
                AnalysisCandidate {
                    candidate_id: "candidate-b".into(),
                    method: "descriptive".into(),
                    estimand: "average treatment effect in organoid model".into(),
                    assumptions: vec!["measurement is comparable".into()],
                    effect_estimate: "0.2".into(),
                    uncertainty: "bootstrap interval [0.0,0.4]".into(),
                    selection_score: 0.55,
                    input_artifacts: vec![digest.clone()],
                    identification: IdentificationStatus::PartiallyIdentified,
                    negative_evidence: vec!["null replication not yet available".into()],
                },
                AnalysisCandidate {
                    candidate_id: "candidate-a".into(),
                    method: "doubly-robust".into(),
                    estimand: "average treatment effect in organoid model".into(),
                    assumptions: vec!["no unmeasured confounding in declared scope".into()],
                    effect_estimate: "0.3".into(),
                    uncertainty: "95% interval [0.1,0.5]".into(),
                    selection_score: 0.8,
                    input_artifacts: vec![digest],
                    identification: IdentificationStatus::Identified,
                    negative_evidence: vec![],
                },
            ],
            protected_omissions: vec![],
            raw_data_local: true,
        }
    }

    #[test]
    fn qualification_is_deterministic_and_selects_declared_identified_candidate() {
        let mut reversed = request();
        reversed.candidates.reverse();
        let left = qualify_analysis(&request()).unwrap();
        let right = qualify_analysis(&reversed).unwrap();
        assert_eq!(left.selected_candidate.as_deref(), Some("candidate-a"));
        assert_eq!(left.verdict, QualificationVerdict::Qualified);
        assert_eq!(left.digest().unwrap(), right.digest().unwrap());
    }

    #[test]
    fn protected_omission_lowers_verdict_to_conditional() {
        let mut request = request();
        request.protected_omissions.push("missing independent site".into());
        let result = qualify_analysis(&request).unwrap();
        assert_eq!(result.verdict, QualificationVerdict::Conditional);
        assert!(result.omissions[0].contains("independent"));
    }

    #[test]
    fn unidentified_candidates_block_qualification() {
        let mut request = request();
        for candidate in &mut request.candidates {
            candidate.identification = IdentificationStatus::Unidentified;
        }
        let result = qualify_analysis(&request).unwrap();
        assert_eq!(result.verdict, QualificationVerdict::Blocked);
        assert!(result.selected_candidate.is_none());
    }

    #[test]
    fn missing_artifact_coverage_is_rejected() {
        let mut request = request();
        request.candidates[0].input_artifacts.clear();
        assert!(matches!(
            qualify_analysis(&request).unwrap_err(),
            AnalysisQualificationError::MissingArtifactCoverage(_)
        ));
    }
}
