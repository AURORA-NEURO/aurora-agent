//! Federated mechanism-exploration control plane.
//!
//! Atlas feature: `AFA-benchcompiler-P08-F30`.
//! This boundary operates a ranked portfolio of competing preclinical
//! mechanisms without collapsing disagreement into one conclusion. It emits
//! only typed, content-addressed summaries and retains blocked, unknown, and
//! contradictory candidates.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-benchcompiler-P08-F30";
pub const CONTRACT_VERSION: &str = "benchcompiler-federated-mechanism-control/1.0";
pub const SCHEMA_VERSION: &str = "aurora-research-contract/1.0";
pub const PRECLINICAL_BOUNDARY: &str =
    "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismState {
    Supported,
    Unknown,
    Contradicted,
    Unmeasured,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StudyContext {
    pub study_id: String,
    pub scope: String,
    pub modality_order: Vec<String>,
    pub comparability_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismCandidate {
    pub mechanism_id: String,
    pub study_id: String,
    pub statement_digest: ContentHash,
    pub evidence_order: Vec<ContentHash>,
    pub provenance_digest: ContentHash,
    pub comparability_digest: ContentHash,
    pub support_score: u16,
    pub state: MechanismState,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismQuestion {
    pub request_id: String,
    pub workflow_id: String,
    pub objective_id: String,
    pub required_modalities: Vec<String>,
    pub studies: Vec<StudyContext>,
    pub candidates: Vec<MechanismCandidate>,
    pub replay_identity: ContentHash,
    pub budget: u64,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub federation_allow: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismDisposition {
    Ranked,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismPortfolio {
    pub portfolio_id: String,
    pub disposition: MechanismDisposition,
    pub study_order: Vec<String>,
    pub ranked_order: Vec<String>,
    pub rank_score_order: Vec<u16>,
    pub competing_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub comparability_digest: Option<ContentHash>,
    pub evidence_order: Vec<ContentHash>,
    pub provenance_order: Vec<ContentHash>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub replay_identity: ContentHash,
    pub portfolio_digest: ContentHash,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismControlReceipt {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub objective_id: String,
    pub disposition: MechanismDisposition,
    pub portfolio: MechanismPortfolio,
    pub checks: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MechanismControlError {
    #[error("invalid mechanism control request: {0}")]
    Invalid(String),
    #[error("mechanism control serialization failed: {0}")]
    Serialization(String),
}

impl MechanismControlReceipt {
    pub fn validate(&self) -> Result<(), MechanismControlError> {
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
            || (self.portfolio.ranked_order.is_empty()
                && self.portfolio.blocked_order.is_empty()
                && self.portfolio.omissions.is_empty()
                && self.portfolio.uncertainty.is_empty()
                && self.portfolio.negative_evidence.is_empty())
            || self.portfolio.ranked_order.len() != self.portfolio.rank_score_order.len()
        {
            return Err(MechanismControlError::Invalid(
                "mechanism identity, portfolio, ranking, checks, effects, locality, or boundary is incomplete".into(),
            ));
        }
        for values in [
            &self.portfolio.study_order,
            &self.portfolio.competing_order,
            &self.portfolio.blocked_order,
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
                return Err(MechanismControlError::Invalid(
                    "mechanism control ordering is not canonical".into(),
                ));
            }
        }
        let mut seen = BTreeSet::new();
        for mechanism_id in &self.portfolio.ranked_order {
            if !seen.insert(mechanism_id) {
                return Err(MechanismControlError::Invalid(
                    "ranked mechanism IDs are duplicated".into(),
                ));
            }
        }
        if self
            .portfolio
            .rank_score_order
            .windows(2)
            .any(|pair| pair[0] < pair[1])
        {
            return Err(MechanismControlError::Invalid(
                "mechanism scores are not ranked descending".into(),
            ));
        }
        for values in [
            &self.portfolio.evidence_order,
            &self.portfolio.provenance_order,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(MechanismControlError::Invalid(
                    "mechanism control digest ordering is not canonical".into(),
                ));
            }
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, MechanismControlError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| MechanismControlError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| MechanismControlError::Serialization(error.to_string()))
    }
}

pub fn operate_mechanism_control(
    request: &MechanismQuestion,
) -> Result<MechanismControlReceipt, MechanismControlError> {
    validate_request(request)?;
    let mut studies = request.studies.clone();
    studies.sort_by(|left, right| left.study_id.cmp(&right.study_id));
    let study_order = studies
        .iter()
        .map(|study| study.study_id.clone())
        .collect::<Vec<_>>();
    let baseline_comparability = studies
        .first()
        .map(|study| study.comparability_digest.clone());
    let known_studies = studies
        .iter()
        .map(|study| (study.study_id.clone(), study.comparability_digest.clone()))
        .collect::<BTreeMap<_, _>>();
    let mut candidates = request.candidates.clone();
    candidates.sort_by(|left, right| left.mechanism_id.cmp(&right.mechanism_id));
    let mut admitted = Vec::<(String, u16)>::new();
    let mut blocked = BTreeSet::new();
    let mut competing = BTreeSet::new();
    let mut evidence = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut spent = 0_u64;
    for candidate in &candidates {
        let cost = candidate.evidence_order.len() as u64 + candidate.support_score as u64 / 10 + 1;
        let budget_ok = cost <= request.budget.saturating_sub(spent);
        let study_comparability = known_studies.get(&candidate.study_id);
        let comparable = study_comparability == Some(&candidate.comparability_digest)
            && baseline_comparability.as_ref() == Some(&candidate.comparability_digest);
        let complete = !candidate.evidence_order.is_empty()
            && candidate.omissions.is_empty()
            && candidate.uncertainty.is_empty();
        let gate = request.policy_allow
            && request.protected_closure
            && request.signed_approval
            && request.federation_allow
            && request.raw_data_local
            && candidate.state == MechanismState::Supported
            && comparable
            && complete
            && budget_ok;
        if gate {
            spent = spent.saturating_add(cost);
            admitted.push((candidate.mechanism_id.clone(), candidate.support_score));
            competing.insert(candidate.mechanism_id.clone());
            evidence.extend(candidate.evidence_order.iter().cloned());
            provenance.insert(candidate.provenance_digest.clone());
        } else {
            blocked.insert(candidate.mechanism_id.clone());
            if !comparable {
                omissions.insert(format!(
                    "mechanism:{}:cross-study-comparability-mismatch",
                    candidate.mechanism_id
                ));
                negative.insert(format!(
                    "mechanism:{}:comparability-not-admitted",
                    candidate.mechanism_id
                ));
            }
            if candidate.state != MechanismState::Supported {
                negative.insert(
                    format!(
                        "mechanism:{}:state-{:?}-not-admitted",
                        candidate.mechanism_id, candidate.state
                    )
                    .to_ascii_lowercase(),
                );
            }
            if !complete {
                omissions.insert(format!(
                    "mechanism:{}:evidence-or-protected-closure-incomplete",
                    candidate.mechanism_id
                ));
            }
            if !budget_ok {
                omissions.insert(format!(
                    "mechanism:{}:budget-ceiling-exceeded",
                    candidate.mechanism_id
                ));
            }
        }
    }
    admitted.sort_by(|left, right| right.1.cmp(&left.1).then_with(|| left.0.cmp(&right.0)));
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.signed_approval {
        uncertainty.insert("request:signed-approval-missing".into());
    }
    if !request.federation_allow {
        negative.insert("request:federation-summary-exchange-denied".into());
    }
    let ranked_order = admitted
        .iter()
        .map(|item| item.0.clone())
        .collect::<Vec<_>>();
    let rank_score_order = admitted.iter().map(|item| item.1).collect::<Vec<_>>();
    let competing_order = competing.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let disposition =
        if !request.policy_allow || !request.signed_approval || !request.federation_allow {
            MechanismDisposition::Blocked
        } else if !request.protected_closure {
            MechanismDisposition::Unknown
        } else if ranked_order.is_empty() {
            MechanismDisposition::Unknown
        } else if blocked_order.is_empty() {
            MechanismDisposition::Ranked
        } else {
            MechanismDisposition::Partial
        };
    let mut checks = vec![
        "deterministic support-score ranking with mechanism-id tie break".into(),
        "comparability, evidence, provenance, policy, authority, federation, locality, and budget gates".into(),
        "competing supported mechanisms remain represented instead of being collapsed into a single truth".into(),
        "unknown, contradicted, unmeasured, omitted, and denied mechanisms retain negative evidence".into(),
    ];
    checks.sort();
    let omissions = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence = negative.into_iter().collect::<Vec<_>>();
    let evidence_order = evidence.into_iter().collect::<Vec<_>>();
    let provenance_order = provenance.into_iter().collect::<Vec<_>>();
    let mut effect_receipts = if !ranked_order.is_empty() {
        ranked_order
            .iter()
            .map(|id| format!("exchange:permitted-mechanism-summary:{id}"))
            .collect::<Vec<_>>()
    } else {
        vec![format!("block:mechanism-control:{disposition:?}").to_ascii_lowercase()]
    };
    effect_receipts.sort();
    let portfolio_id = format!("mechanism-portfolio:{}", request.request_id);
    let portfolio_payload = json!({
        "portfolio_id": portfolio_id,
        "disposition": disposition,
        "study_order": study_order,
        "ranked_order": ranked_order,
        "rank_score_order": rank_score_order,
        "competing_order": competing_order,
        "blocked_order": blocked_order,
        "comparability_digest": baseline_comparability,
        "evidence_order": evidence_order,
        "provenance_order": provenance_order,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": negative_evidence,
        "replay_identity": request.replay_identity,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let portfolio_digest = ContentHash::of_value(&portfolio_payload)
        .map_err(|error| MechanismControlError::Serialization(error.to_string()))?;
    let portfolio = MechanismPortfolio {
        portfolio_id,
        disposition,
        study_order,
        ranked_order,
        rank_score_order,
        competing_order,
        blocked_order,
        comparability_digest: baseline_comparability,
        evidence_order,
        provenance_order,
        omissions: omissions.clone(),
        uncertainty: uncertainty.clone(),
        negative_evidence: negative_evidence.clone(),
        replay_identity: request.replay_identity.clone(),
        portfolio_digest,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let receipt = MechanismControlReceipt {
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

fn validate_request(request: &MechanismQuestion) -> Result<(), MechanismControlError> {
    if request.request_id.trim().is_empty()
        || request.workflow_id.trim().is_empty()
        || request.objective_id.trim().is_empty()
        || request.required_modalities.is_empty()
        || request.studies.len() < 2
        || request.candidates.is_empty()
        || request.budget == 0
        || request.boundary != PRECLINICAL_BOUNDARY
        || request
            .required_modalities
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(MechanismControlError::Invalid(
            "mechanism identity, modalities, multi-study contexts, candidates, budget, or boundary is incomplete".into(),
        ));
    }
    let mut studies = BTreeSet::new();
    for study in &request.studies {
        if study.study_id.trim().is_empty()
            || study.scope.trim().is_empty()
            || study.modality_order.is_empty()
            || study.boundary != PRECLINICAL_BOUNDARY
            || !studies.insert(study.study_id.clone())
            || study
                .modality_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
        {
            return Err(MechanismControlError::Invalid(format!(
                "study {} is invalid or duplicated",
                study.study_id
            )));
        }
    }
    let mut candidates = BTreeSet::new();
    for candidate in &request.candidates {
        if candidate.mechanism_id.trim().is_empty()
            || candidate.study_id.trim().is_empty()
            || !studies.contains(&candidate.study_id)
            || !candidates.insert(candidate.mechanism_id.clone())
            || candidate.boundary != PRECLINICAL_BOUNDARY
            || candidate
                .evidence_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
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
            return Err(MechanismControlError::Invalid(format!(
                "mechanism candidate {} is invalid or duplicated",
                candidate.mechanism_id
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

    fn study(id: &str) -> StudyContext {
        StudyContext {
            study_id: id.into(),
            scope: "organoid:neural".into(),
            modality_order: vec!["imaging".into(), "omics".into()],
            comparability_digest: hash("comparability"),
            provenance_digest: hash(&format!("study-provenance:{id}")),
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    fn candidate(
        id: &str,
        study_id: &str,
        score: u16,
        state: MechanismState,
    ) -> MechanismCandidate {
        MechanismCandidate {
            mechanism_id: id.into(),
            study_id: study_id.into(),
            statement_digest: hash(&format!("statement:{id}")),
            evidence_order: vec![hash(&format!("evidence:{id}"))],
            provenance_digest: hash(&format!("provenance:{id}")),
            comparability_digest: hash("comparability"),
            support_score: score,
            state,
            omissions: vec![],
            uncertainty: vec![],
            negative_evidence: vec![],
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    fn request(candidates: Vec<MechanismCandidate>) -> MechanismQuestion {
        MechanismQuestion {
            request_id: "mechanism:control".into(),
            workflow_id: "workflow:mechanism-exploration".into(),
            objective_id: "objective:organoid".into(),
            required_modalities: vec!["imaging".into(), "omics".into()],
            studies: vec![study("study:a"), study("study:b")],
            candidates,
            replay_identity: hash("replay"),
            budget: 100,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            federation_allow: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn ranks_competing_supported_mechanisms_deterministically() {
        let receipt = operate_mechanism_control(&request(vec![
            candidate("mechanism:a", "study:a", 70, MechanismState::Supported),
            candidate("mechanism:b", "study:b", 90, MechanismState::Supported),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, MechanismDisposition::Ranked);
        assert_eq!(
            receipt.portfolio.ranked_order,
            vec!["mechanism:b", "mechanism:a"]
        );
        assert_eq!(receipt.portfolio.rank_score_order, vec![90, 70]);
        assert_eq!(receipt.digest(), receipt.digest());
    }

    #[test]
    fn contradiction_remains_blocked_and_negative() {
        let receipt = operate_mechanism_control(&request(vec![
            candidate("mechanism:a", "study:a", 70, MechanismState::Supported),
            candidate("mechanism:b", "study:b", 90, MechanismState::Contradicted),
        ]))
        .unwrap();
        assert_eq!(receipt.disposition, MechanismDisposition::Partial);
        assert_eq!(receipt.portfolio.blocked_order, vec!["mechanism:b"]);
        assert!(!receipt.negative_evidence.is_empty());
    }

    #[test]
    fn comparability_mismatch_is_not_admitted() {
        let mut input = request(vec![
            candidate("mechanism:a", "study:a", 70, MechanismState::Supported),
            candidate("mechanism:b", "study:b", 90, MechanismState::Supported),
        ]);
        input.candidates[1].comparability_digest = hash("different");
        let receipt = operate_mechanism_control(&input).unwrap();
        assert_eq!(receipt.disposition, MechanismDisposition::Partial);
        assert!(receipt
            .omissions
            .iter()
            .any(|item| item.contains("comparability")));
    }

    #[test]
    fn protected_closure_gap_is_unknown() {
        let mut input = request(vec![candidate(
            "mechanism:a",
            "study:a",
            70,
            MechanismState::Supported,
        )]);
        input.protected_closure = false;
        let receipt = operate_mechanism_control(&input).unwrap();
        assert_eq!(receipt.disposition, MechanismDisposition::Unknown);
        assert!(receipt
            .uncertainty
            .iter()
            .any(|item| item.contains("protected-closure")));
    }

    #[test]
    fn duplicate_candidate_is_rejected() {
        let result = operate_mechanism_control(&request(vec![
            candidate("mechanism:a", "study:a", 70, MechanismState::Supported),
            candidate("mechanism:a", "study:b", 90, MechanismState::Supported),
        ]));
        assert!(result.is_err());
    }
}
