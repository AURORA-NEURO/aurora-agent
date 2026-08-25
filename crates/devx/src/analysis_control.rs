//! Federated statistical, causal, and ML analysis control plane.
//!
//! Atlas feature: `AFA-devx-P13-F31`.
//! The control plane admits typed analysis candidates only when their evidence,
//! comparability, provenance, policy, and federation gates are satisfied. It
//! exchanges digests and manifests; raw research data remains institution-local.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-devx-P13-F31";
pub const CONTRACT_VERSION: &str = "devx-federated-analysis-control-plane/1.0";
pub const SCHEMA_VERSION: &str = "aurora-research-contract/1.0";
pub const PRECLINICAL_BOUNDARY: &str =
    "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisState {
    Supported,
    Unknown,
    Contradicted,
    Unmeasured,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisCandidate {
    pub candidate_id: String,
    pub analysis_class: String,
    pub site_id: String,
    pub scope: String,
    pub estimand: String,
    pub result_digest: ContentHash,
    pub model_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub comparability_digest: Option<ContentHash>,
    pub state: AnalysisState,
    pub quality_score: u16,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisRequest {
    pub request_id: String,
    pub workflow_id: String,
    pub objective_id: String,
    pub scope: String,
    pub required_analysis_classes: Vec<String>,
    pub minimum_quality_score: u16,
    pub candidates: Vec<AnalysisCandidate>,
    pub replay_identity: ContentHash,
    pub budget: u64,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub federation_allow: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisDisposition {
    Ranked,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisPortfolio {
    pub portfolio_id: String,
    pub disposition: AnalysisDisposition,
    pub candidate_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub rank_score_order: Vec<u16>,
    pub class_order: Vec<String>,
    pub result_order: Vec<ContentHash>,
    pub model_order: Vec<ContentHash>,
    pub provenance_order: Vec<ContentHash>,
    pub replay_identity: ContentHash,
    pub portfolio_digest: ContentHash,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisControlReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub objective_id: String,
    pub disposition: AnalysisDisposition,
    pub portfolio: AnalysisPortfolio,
    pub checks: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AnalysisControlError {
    #[error("invalid analysis control request: {0}")]
    Invalid(String),
    #[error("analysis control serialization failed: {0}")]
    Serialization(String),
}

impl AnalysisControlReceipt {
    pub fn validate(&self) -> Result<(), AnalysisControlError> {
        if self.schema_version != SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.objective_id.trim().is_empty()
            || self.checks.is_empty()
            || self.effect_receipts.is_empty()
            || self.portfolio.boundary != PRECLINICAL_BOUNDARY
            || self.portfolio.portfolio_id.trim().is_empty()
            || (self.portfolio.admitted_order.is_empty()
                && self.portfolio.blocked_order.is_empty()
                && self.portfolio.omissions.is_empty()
                && self.portfolio.uncertainty.is_empty()
                && self.portfolio.negative_evidence.is_empty())
        {
            return Err(AnalysisControlError::Invalid(
                "analysis control identity, portfolio, checks, effects, locality, or boundary is incomplete".into(),
            ));
        }
        for values in [
            &self.portfolio.candidate_order,
            &self.portfolio.admitted_order,
            &self.portfolio.blocked_order,
            &self.portfolio.class_order,
            &self.portfolio.omissions,
            &self.portfolio.uncertainty,
            &self.portfolio.negative_evidence,
            &self.checks,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(AnalysisControlError::Invalid(
                    "analysis control ordering is not canonical".into(),
                ));
            }
        }
        if self.portfolio.admitted_order.len() != self.portfolio.rank_score_order.len()
            || self
                .portfolio
                .rank_score_order
                .windows(2)
                .any(|pair| pair[0] < pair[1])
        {
            return Err(AnalysisControlError::Invalid(
                "analysis control ranking is not deterministic".into(),
            ));
        }
        for values in [
            &self.portfolio.result_order,
            &self.portfolio.model_order,
            &self.portfolio.provenance_order,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(AnalysisControlError::Invalid(
                    "analysis control digest ordering is not canonical".into(),
                ));
            }
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, AnalysisControlError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| AnalysisControlError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| AnalysisControlError::Serialization(error.to_string()))
    }
}

pub fn operate_analysis_control(
    request: &AnalysisRequest,
) -> Result<AnalysisControlReceipt, AnalysisControlError> {
    validate_request(request)?;
    let mut candidates = request.candidates.clone();
    candidates.sort_by(|left, right| left.candidate_id.cmp(&right.candidate_id));
    let required_classes = request
        .required_analysis_classes
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut candidate_order = BTreeSet::new();
    let mut admitted = Vec::<(String, u16)>::new();
    let mut blocked = BTreeSet::new();
    let mut classes = BTreeSet::new();
    let mut results = BTreeSet::new();
    let mut models = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut spent = 0_u64;
    for candidate in &candidates {
        candidate_order.insert(candidate.candidate_id.clone());
        let cost = candidate.candidate_id.len() as u64 + 1;
        let budget_ok = cost <= request.budget.saturating_sub(spent);
        let complete = candidate.comparability_digest.is_some()
            && candidate.omissions.is_empty()
            && candidate.uncertainty.is_empty()
            && candidate.scope == request.scope
            && candidate.quality_score >= request.minimum_quality_score;
        let gate = request.policy_allow
            && request.protected_closure
            && request.federation_allow
            && request.signed_approval
            && request.raw_data_local
            && candidate.state == AnalysisState::Supported
            && complete
            && budget_ok;
        if gate {
            spent = spent.saturating_add(cost);
            admitted.push((candidate.candidate_id.clone(), candidate.quality_score));
            classes.insert(candidate.analysis_class.clone());
            results.insert(candidate.result_digest.clone());
            models.insert(candidate.model_digest.clone());
            provenance.insert(candidate.provenance_digest.clone());
        } else {
            blocked.insert(candidate.candidate_id.clone());
            if candidate.state != AnalysisState::Supported {
                negative.insert(
                    format!(
                        "candidate:{}:state-{:?}-not-admitted",
                        candidate.candidate_id, candidate.state
                    )
                    .to_ascii_lowercase(),
                );
            }
            if candidate.comparability_digest.is_none() {
                omissions.insert(format!(
                    "candidate:{}:cross-study-comparability-missing",
                    candidate.candidate_id
                ));
            }
            if candidate.scope != request.scope {
                omissions.insert(format!(
                    "candidate:{}:scope-mismatch",
                    candidate.candidate_id
                ));
            }
            if candidate.quality_score < request.minimum_quality_score {
                uncertainty.insert(format!(
                    "candidate:{}:quality-below-floor",
                    candidate.candidate_id
                ));
            }
            if !candidate.omissions.is_empty() || !candidate.uncertainty.is_empty() {
                uncertainty.insert(format!(
                    "candidate:{}:protected-closure-or-evidence-incomplete",
                    candidate.candidate_id
                ));
            }
            if !budget_ok {
                omissions.insert(format!(
                    "candidate:{}:budget-ceiling-exceeded",
                    candidate.candidate_id
                ));
            }
        }
    }
    for required_class in required_classes {
        if !classes.contains(&required_class) {
            omissions.insert(format!(
                "analysis-class:{required_class}:required-but-not-admitted"
            ));
        }
    }
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.federation_allow {
        negative.insert("request:federation-denied".into());
    }
    if !request.signed_approval {
        omissions.insert("request:signed-approval-required".into());
    }
    admitted.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    let admitted_order = admitted
        .iter()
        .map(|item| item.0.clone())
        .collect::<Vec<_>>();
    let rank_score_order = admitted.iter().map(|item| item.1).collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let disposition = if !request.policy_allow || !request.federation_allow {
        AnalysisDisposition::Blocked
    } else if !request.protected_closure || admitted_order.is_empty() {
        AnalysisDisposition::Unknown
    } else if blocked_order.is_empty() && omissions.is_empty() {
        AnalysisDisposition::Ranked
    } else {
        AnalysisDisposition::Partial
    };
    let mut checks = vec![
        "candidate and digest ordering is canonical".into(),
        "comparability, evidence, provenance, policy, federation, locality, approval, and budget gates are explicit".into(),
        "contradicted, unknown, unmeasured, omitted, and negative analysis states remain researcher-visible".into(),
        "digest-only federated manifests never export raw research data".into(),
    ];
    checks.sort();
    let candidate_order = candidate_order.into_iter().collect::<Vec<_>>();
    let class_order = classes.into_iter().collect::<Vec<_>>();
    let result_order = results.into_iter().collect::<Vec<_>>();
    let model_order = models.into_iter().collect::<Vec<_>>();
    let provenance_order = provenance.into_iter().collect::<Vec<_>>();
    let omissions = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence = negative.into_iter().collect::<Vec<_>>();
    let effect_receipts = candidate_order
        .iter()
        .map(|candidate_id| format!("exchange:digest-only-analysis-manifest:{candidate_id}"))
        .collect::<Vec<_>>();
    let portfolio_id = format!("analysis-portfolio:{}", request.request_id);
    let portfolio_payload = json!({
        "portfolio_id": portfolio_id,
        "disposition": disposition,
        "candidate_order": candidate_order,
        "admitted_order": admitted_order,
        "blocked_order": blocked_order,
        "rank_score_order": rank_score_order,
        "class_order": class_order,
        "result_order": result_order,
        "model_order": model_order,
        "provenance_order": provenance_order,
        "replay_identity": request.replay_identity,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": negative_evidence,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let portfolio_digest = ContentHash::of_value(&portfolio_payload)
        .map_err(|error| AnalysisControlError::Serialization(error.to_string()))?;
    let portfolio = AnalysisPortfolio {
        portfolio_id,
        disposition,
        candidate_order,
        admitted_order,
        blocked_order,
        rank_score_order,
        class_order,
        result_order,
        model_order,
        provenance_order,
        replay_identity: request.replay_identity.clone(),
        portfolio_digest,
        omissions: omissions.clone(),
        uncertainty: uncertainty.clone(),
        negative_evidence: negative_evidence.clone(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let receipt = AnalysisControlReceipt {
        schema_version: SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        objective_id: request.objective_id.clone(),
        disposition,
        portfolio,
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

fn validate_request(request: &AnalysisRequest) -> Result<(), AnalysisControlError> {
    if request.request_id.trim().is_empty()
        || request.workflow_id.trim().is_empty()
        || request.objective_id.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.required_analysis_classes.is_empty()
        || request.candidates.is_empty()
        || request.budget == 0
        || request.boundary != PRECLINICAL_BOUNDARY
        || request
            .required_analysis_classes
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(AnalysisControlError::Invalid(
            "analysis control identity, scope, classes, candidates, budget, or boundary is incomplete".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for candidate in &request.candidates {
        if candidate.candidate_id.trim().is_empty()
            || candidate.analysis_class.trim().is_empty()
            || candidate.site_id.trim().is_empty()
            || candidate.scope.trim().is_empty()
            || candidate.estimand.trim().is_empty()
            || !ids.insert(candidate.candidate_id.clone())
            || candidate.boundary != PRECLINICAL_BOUNDARY
            || candidate
                .omissions
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || candidate
                .uncertainty
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || candidate
                .negative_evidence
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err(AnalysisControlError::Invalid(format!(
                "analysis candidate {} is invalid or duplicated",
                candidate.candidate_id
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

    fn candidate(id: &str, class: &str, state: AnalysisState, score: u16) -> AnalysisCandidate {
        AnalysisCandidate {
            candidate_id: id.into(),
            analysis_class: class.into(),
            site_id: "site:a".into(),
            scope: "organoid:neural".into(),
            estimand: "synaptic-density-delta".into(),
            result_digest: hash(&format!("result:{id}")),
            model_digest: hash(&format!("model:{id}")),
            provenance_digest: hash(&format!("provenance:{id}")),
            comparability_digest: Some(hash("comparability")),
            state,
            quality_score: score,
            omissions: vec![],
            uncertainty: vec![],
            negative_evidence: vec![],
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    fn request(candidates: Vec<AnalysisCandidate>) -> AnalysisRequest {
        AnalysisRequest {
            request_id: "analysis:control".into(),
            workflow_id: "workflow:analysis".into(),
            objective_id: "objective:organoid".into(),
            scope: "organoid:neural".into(),
            required_analysis_classes: vec!["causal".into(), "statistical".into()],
            minimum_quality_score: 70,
            candidates,
            replay_identity: hash("replay"),
            budget: 100,
            policy_allow: true,
            protected_closure: true,
            federation_allow: true,
            signed_approval: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn ranks_supported_candidates_by_score_and_id() {
        let receipt = operate_analysis_control(&request(vec![
            candidate("candidate:b", "causal", AnalysisState::Supported, 80),
            candidate("candidate:a", "statistical", AnalysisState::Supported, 90),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, AnalysisDisposition::Ranked);
        assert_eq!(
            receipt.portfolio.admitted_order,
            vec!["candidate:a", "candidate:b"]
        );
        assert_eq!(receipt.portfolio.rank_score_order, vec![90, 80]);
    }

    #[test]
    fn contradiction_is_negative_evidence() {
        let receipt = operate_analysis_control(&request(vec![
            candidate("candidate:a", "causal", AnalysisState::Contradicted, 90),
            candidate("candidate:b", "statistical", AnalysisState::Supported, 80),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, AnalysisDisposition::Partial);
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|item| item.contains("contradicted")));
    }

    #[test]
    fn missing_comparability_is_explicit() {
        let mut missing = candidate("candidate:a", "causal", AnalysisState::Supported, 90);
        missing.comparability_digest = None;
        let receipt = operate_analysis_control(&request(vec![
            missing,
            candidate("candidate:b", "statistical", AnalysisState::Supported, 80),
        ]))
        .unwrap();
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item.contains("comparability")));
    }

    #[test]
    fn federation_denial_blocks_exchange() {
        let mut input = request(vec![candidate(
            "candidate:a",
            "causal",
            AnalysisState::Supported,
            90,
        )]);
        input.federation_allow = false;
        let receipt = operate_analysis_control(&input).unwrap();
        assert_eq!(receipt.disposition, AnalysisDisposition::Blocked);
        assert!(receipt
            .negative_evidence
            .iter()
            .any(|item| item.contains("federation")));
    }

    #[test]
    fn duplicate_candidates_are_rejected() {
        let result = operate_analysis_control(&request(vec![
            candidate("candidate:a", "causal", AnalysisState::Supported, 90),
            candidate("candidate:a", "statistical", AnalysisState::Supported, 80),
        ]));
        assert!(result.is_err());
    }
}
