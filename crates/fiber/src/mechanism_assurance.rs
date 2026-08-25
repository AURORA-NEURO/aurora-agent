//! Multimodal mechanism-exploration assurance harness.
//!
//! Atlas feature: `AFA-fiber-P08-F26`.
//!
//! The harness verifies a caller-supplied candidate portfolio; it does not invent mechanisms.
//! Candidates are ranked deterministically, but admission requires scoped, comparable,
//! digest-complete evidence and provenance.  Unknown, unmeasured, contradicted, omitted, and
//! negative states are retained as witnesses instead of being collapsed into a score.

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState as FoundationEvidenceState, ResearchSurface, TypedPort, TypedResearchArtifact,
    PRECLINICAL_BOUNDARY, RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-fiber-P08-F26";
pub const CONTRACT_VERSION: &str = "fiber-mechanism-assurance/1.0";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateState {
    Supported,
    Unknown,
    Contradicted,
    Unmeasured,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AssuranceDisposition {
    Qualified,
    Conditional,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismCandidate {
    pub candidate_id: String,
    pub mechanism_id: String,
    pub scope: String,
    pub study_ids: Vec<String>,
    pub modality_ids: Vec<String>,
    pub support_milli: u16,
    pub state: CandidateState,
    pub artifact_digest: Option<ContentHash>,
    pub evidence_digest: Option<ContentHash>,
    pub provenance_digest: Option<ContentHash>,
    pub negative_result: bool,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismQuestion {
    pub question_id: String,
    pub workflow_id: String,
    pub scope: String,
    pub target_schema: String,
    pub candidates: Vec<MechanismCandidate>,
    pub required_candidate_ids: Vec<String>,
    pub max_admissions: usize,
    pub replay_identity: ContentHash,
    pub budget: u64,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismPortfolio {
    pub schema_version: String,
    pub feature_id: String,
    pub contract_version: String,
    pub question_id: String,
    pub workflow_id: String,
    pub target_schema: String,
    pub disposition: AssuranceDisposition,
    pub ranked_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub mechanism_order: Vec<String>,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub artifact_order: Vec<ContentHash>,
    pub evidence_order: Vec<ContentHash>,
    pub provenance_order: Vec<ContentHash>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub replay_identity: ContentHash,
    pub effect_receipts: Vec<String>,
    pub artifact: TypedResearchArtifact,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MechanismAssuranceError {
    #[error("invalid mechanism question: {0}")]
    Invalid(String),
    #[error("mechanism assurance contract failed: {0}")]
    Contract(String),
}

impl MechanismPortfolio {
    pub fn validate(&self) -> Result<(), MechanismAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.feature_id != FEATURE_ID
            || self.contract_version != CONTRACT_VERSION
            || self.question_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.target_schema.trim().is_empty()
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.ranked_order.is_empty()
            || (self.effect_receipts.is_empty()
                && self.disposition != AssuranceDisposition::Qualified)
        {
            return Err(MechanismAssuranceError::Contract(
                "mechanism portfolio identity, ranking, locality, effects, or boundary is incomplete".into(),
            ));
        }
        for values in [
            &self.admitted_order,
            &self.blocked_order,
            &self.unknown_order,
            &self.mechanism_order,
            &self.study_order,
            &self.modality_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(MechanismAssuranceError::Contract(
                    "mechanism portfolio ordering is not canonical".into(),
                ));
            }
        }
        for values in [
            &self.artifact_order,
            &self.evidence_order,
            &self.provenance_order,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(MechanismAssuranceError::Contract(
                    "mechanism portfolio digest ordering is not canonical".into(),
                ));
            }
        }
        if self
            .effect_receipts
            .iter()
            .any(|effect| effect != "block:unsafe-release")
        {
            return Err(MechanismAssuranceError::Contract(
                "mechanism assurance effect is outside the unsafe-release gate".into(),
            ));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| MechanismAssuranceError::Contract(error.to_string()))?;
        Ok(())
    }
}

pub fn capability_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: CONTRACT_VERSION.into(),
        owner_crate: "fiber".into(),
        consumers: ["downstream AURORA crate maintainer".into()].into(),
        behavior: "verifies and ranks caller-supplied multimodal mechanism candidates without inventing a mechanism or hiding unresolved evidence".into(),
        value: "prevents unsafe release of incomplete mechanism portfolios while retaining reproducible counterevidence".into(),
        inputs: vec![TypedPort {
            name: "mechanism_question".into(),
            schema: "MechanismQuestion2@1".into(),
            required: true,
        }],
        outputs: vec![TypedPort {
            name: "mechanism_portfolio".into(),
            schema: "MechanismPortfolio7@1".into(),
            required: true,
        }],
        effects: [Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(),
        permissions: ["evaluate:capability-runs".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference {
            source_id: "slsa-provenance-1.2".into(),
            state: FoundationEvidenceState::Supported,
            locator: Some("https://slsa.dev/spec/v1.2/provenance".into()),
        }],
        authority_requirements: Vec::new(),
        autonomy_tier: AutonomyTier::A1,
        surfaces: [ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Policy].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn assure(question: &MechanismQuestion) -> Result<MechanismPortfolio, MechanismAssuranceError> {
    validate_question(question)?;
    let mut candidates = question.candidates.clone();
    candidates.sort_by(|left, right| {
        right
            .support_milli
            .cmp(&left.support_milli)
            .then_with(|| left.mechanism_id.cmp(&right.mechanism_id))
            .then_with(|| left.candidate_id.cmp(&right.candidate_id))
    });
    let ranked_order = candidates
        .iter()
        .map(|candidate| candidate.candidate_id.clone())
        .collect::<Vec<_>>();
    let mut admitted = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut mechanisms = BTreeSet::new();
    let mut studies = BTreeSet::new();
    let mut modalities = BTreeSet::new();
    let mut artifacts = BTreeSet::new();
    let mut evidence = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut spent = 0_u64;
    for candidate in &candidates {
        let cost = candidate.candidate_id.len() as u64
            + candidate.mechanism_id.len() as u64
            + candidate.study_ids.len() as u64;
        if cost > question.budget.saturating_sub(spent) {
            blocked.insert(candidate.candidate_id.clone());
            omissions.insert(format!(
                "candidate:{}:budget-ceiling-exceeded",
                candidate.candidate_id
            ));
            continue;
        }
        if candidate.scope != question.scope {
            blocked.insert(candidate.candidate_id.clone());
            omissions.insert(format!(
                "candidate:{}:scope-mismatch",
                candidate.candidate_id
            ));
            continue;
        }
        match candidate.state {
            CandidateState::Contradicted => {
                blocked.insert(candidate.candidate_id.clone());
                negative.insert(format!(
                    "candidate:{}:contradicted-mechanism-evidence",
                    candidate.candidate_id
                ));
                continue;
            }
            CandidateState::Unknown | CandidateState::Unmeasured => {
                unknown.insert(candidate.candidate_id.clone());
                uncertainty.insert(
                    format!(
                        "candidate:{}:state-{:?}-not-admitted",
                        candidate.candidate_id, candidate.state
                    )
                    .to_ascii_lowercase(),
                );
                continue;
            }
            CandidateState::Supported => {}
        }
        if !candidate.omissions.is_empty() {
            unknown.insert(candidate.candidate_id.clone());
            omissions.extend(
                candidate
                    .omissions
                    .iter()
                    .map(|item| format!("candidate:{}:{item}", candidate.candidate_id)),
            );
            continue;
        }
        if !candidate.uncertainty.is_empty() {
            unknown.insert(candidate.candidate_id.clone());
            uncertainty.extend(
                candidate
                    .uncertainty
                    .iter()
                    .map(|item| format!("candidate:{}:{item}", candidate.candidate_id)),
            );
            continue;
        }
        let (Some(artifact_digest), Some(evidence_digest), Some(provenance_digest)) = (
            candidate.artifact_digest.clone(),
            candidate.evidence_digest.clone(),
            candidate.provenance_digest.clone(),
        ) else {
            unknown.insert(candidate.candidate_id.clone());
            omissions.insert(format!(
                "candidate:{}:artifact-evidence-or-provenance-digest-missing",
                candidate.candidate_id
            ));
            continue;
        };
        if admitted.len() >= question.max_admissions {
            blocked.insert(candidate.candidate_id.clone());
            omissions.insert(format!(
                "candidate:{}:max-admissions-ceiling",
                candidate.candidate_id
            ));
            continue;
        }
        admitted.insert(candidate.candidate_id.clone());
        mechanisms.insert(candidate.mechanism_id.clone());
        studies.extend(candidate.study_ids.iter().cloned());
        modalities.extend(candidate.modality_ids.iter().cloned());
        artifacts.insert(artifact_digest);
        evidence.insert(evidence_digest);
        provenance.insert(provenance_digest);
        spent = spent.saturating_add(cost);
        if candidate.negative_result {
            negative.insert(format!(
                "candidate:{}:negative-result-retained",
                candidate.candidate_id
            ));
        }
    }
    for required in &question.required_candidate_ids {
        if !admitted.contains(required) {
            omissions.insert(format!("candidate:{}:required-but-not-admitted", required));
        }
    }
    if !question.policy_allow {
        blocked.insert("question:policy-denied".into());
        negative.insert("question:policy-denied-no-portfolio-release".into());
    }
    if !question.protected_closure {
        unknown.insert("question:protected-closure-incomplete".into());
        uncertainty.insert("question:protected-closure-incomplete".into());
    }
    if !question.raw_data_local {
        blocked.insert("question:raw-data-locality-required".into());
        omissions.insert("question:raw-data-locality-required".into());
    }
    let admitted_order = admitted.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let unknown_order = unknown.into_iter().collect::<Vec<_>>();
    let mechanism_order = mechanisms.into_iter().collect::<Vec<_>>();
    let study_order = studies.into_iter().collect::<Vec<_>>();
    let modality_order = modalities.into_iter().collect::<Vec<_>>();
    let artifact_order = artifacts.into_iter().collect::<Vec<_>>();
    let evidence_order = evidence.into_iter().collect::<Vec<_>>();
    let provenance_order = provenance.into_iter().collect::<Vec<_>>();
    let omissions = omissions.into_iter().collect::<Vec<_>>();
    let uncertainty = uncertainty.into_iter().collect::<Vec<_>>();
    let negative_evidence = negative.into_iter().collect::<Vec<_>>();
    let disposition = if !question.policy_allow || !question.raw_data_local {
        AssuranceDisposition::Blocked
    } else if admitted_order.is_empty() {
        AssuranceDisposition::Unknown
    } else if !blocked_order.is_empty()
        || !unknown_order.is_empty()
        || !omissions.is_empty()
        || !uncertainty.is_empty()
        || !question.protected_closure
    {
        AssuranceDisposition::Conditional
    } else {
        AssuranceDisposition::Qualified
    };
    let effect_receipts = if disposition == AssuranceDisposition::Qualified {
        Vec::new()
    } else {
        vec!["block:unsafe-release".into()]
    };
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "feature_id": FEATURE_ID,
        "contract_version": CONTRACT_VERSION,
        "question_id": question.question_id,
        "workflow_id": question.workflow_id,
        "target_schema": question.target_schema,
        "disposition": disposition,
        "ranked_order": ranked_order,
        "admitted_order": admitted_order,
        "blocked_order": blocked_order,
        "unknown_order": unknown_order,
        "mechanism_order": mechanism_order,
        "study_order": study_order,
        "modality_order": modality_order,
        "artifact_order": artifact_order,
        "evidence_order": evidence_order,
        "provenance_order": provenance_order,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": negative_evidence,
        "replay_identity": question.replay_identity,
        "effect_receipts": effect_receipts,
        "raw_data_local": true,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("mechanism-portfolio:{}", question.question_id),
        "application/vnd.aurora.mechanism-portfolio+json",
        &payload,
        Vec::new(),
        evidence_order
            .iter()
            .map(|digest| bioprism_foundation::ProvenanceLink {
                source_id: digest.to_string(),
                relation: "mechanism-evidence".into(),
                digest: digest.clone(),
            })
            .collect(),
    )
    .map_err(|error| MechanismAssuranceError::Contract(error.to_string()))?;
    let portfolio = MechanismPortfolio {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        contract_version: CONTRACT_VERSION.into(),
        question_id: question.question_id.clone(),
        workflow_id: question.workflow_id.clone(),
        target_schema: question.target_schema.clone(),
        disposition,
        ranked_order,
        admitted_order,
        blocked_order,
        unknown_order,
        mechanism_order,
        study_order,
        modality_order,
        artifact_order,
        evidence_order,
        provenance_order,
        omissions,
        uncertainty,
        negative_evidence,
        replay_identity: question.replay_identity.clone(),
        effect_receipts,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    portfolio.validate()?;
    Ok(portfolio)
}

fn validate_question(question: &MechanismQuestion) -> Result<(), MechanismAssuranceError> {
    if question.question_id.trim().is_empty()
        || question.workflow_id.trim().is_empty()
        || question.scope.trim().is_empty()
        || question.target_schema.trim().is_empty()
        || question.candidates.is_empty()
        || question.max_admissions == 0
        || question.budget == 0
        || question.boundary != PRECLINICAL_BOUNDARY
        || question
            .required_candidate_ids
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    {
        return Err(MechanismAssuranceError::Invalid(
            "mechanism question identity, scope, candidates, closure, budget, or boundary is incomplete".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for candidate in &question.candidates {
        if candidate.candidate_id.trim().is_empty()
            || candidate.mechanism_id.trim().is_empty()
            || candidate.scope.trim().is_empty()
            || candidate.study_ids.is_empty()
            || candidate.study_ids.len() < 2
            || candidate.modality_ids.len() < 2
            || candidate.support_milli > 1_000
            || candidate.boundary != PRECLINICAL_BOUNDARY
            || !ids.insert(candidate.candidate_id.clone())
            || candidate
                .study_ids
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || candidate
                .modality_ids
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
        {
            return Err(MechanismAssuranceError::Invalid(format!(
                "candidate {} is invalid or duplicated",
                candidate.candidate_id
            )));
        }
    }
    if question
        .required_candidate_ids
        .iter()
        .any(|id| !ids.contains(id))
    {
        return Err(MechanismAssuranceError::Invalid(
            "required candidate closure references an unknown candidate".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(label: &str) -> ContentHash {
        ContentHash::of_bytes(label.as_bytes())
    }

    fn candidate(id: &str, state: CandidateState, negative_result: bool) -> MechanismCandidate {
        MechanismCandidate {
            candidate_id: id.into(),
            mechanism_id: format!("mechanism:{id}"),
            scope: "organoid:neural".into(),
            study_ids: vec!["study:imaging".into(), "study:omics".into()],
            modality_ids: vec!["imaging".into(), "omics".into()],
            support_milli: if id.ends_with('a') { 900 } else { 800 },
            state,
            artifact_digest: Some(hash(&format!("artifact:{id}"))),
            evidence_digest: Some(hash(&format!("evidence:{id}"))),
            provenance_digest: Some(hash(&format!("provenance:{id}"))),
            negative_result,
            omissions: vec![],
            uncertainty: vec![],
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    fn question(candidates: Vec<MechanismCandidate>) -> MechanismQuestion {
        MechanismQuestion {
            question_id: "question:mechanism".into(),
            workflow_id: "workflow:mechanism".into(),
            scope: "organoid:neural".into(),
            target_schema: "mechanism-portfolio/7".into(),
            candidates,
            required_candidate_ids: vec!["candidate:a".into(), "candidate:b".into()],
            max_admissions: 8,
            replay_identity: hash("replay"),
            budget: 10_000,
            policy_allow: true,
            protected_closure: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn manifest_is_typed_and_a1_deterministic() {
        let manifest = capability_manifest();
        assert!(manifest.validate().is_ok());
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A1);
    }

    #[test]
    fn supported_candidates_are_ranked_and_admitted() {
        let portfolio = assure(&question(vec![
            candidate("candidate:a", CandidateState::Supported, false),
            candidate("candidate:b", CandidateState::Supported, true),
        ]))
        .unwrap();
        assert_eq!(portfolio.disposition, AssuranceDisposition::Qualified);
        assert_eq!(portfolio.ranked_order, vec!["candidate:a", "candidate:b"]);
        assert_eq!(portfolio.admitted_order.len(), 2);
        assert!(portfolio
            .negative_evidence
            .iter()
            .any(|value| value.contains("negative-result")));
        assert!(portfolio.effect_receipts.is_empty());
    }

    #[test]
    fn unknown_candidate_is_retained_and_release_blocked() {
        let portfolio = assure(&question(vec![
            candidate("candidate:a", CandidateState::Supported, false),
            candidate("candidate:b", CandidateState::Unknown, false),
        ]))
        .unwrap();
        assert_eq!(portfolio.disposition, AssuranceDisposition::Conditional);
        assert!(portfolio.unknown_order.contains(&"candidate:b".into()));
        assert_eq!(portfolio.effect_receipts, vec!["block:unsafe-release"]);
    }

    #[test]
    fn contradiction_is_blocked_with_negative_evidence() {
        let portfolio = assure(&question(vec![
            candidate("candidate:a", CandidateState::Supported, false),
            candidate("candidate:b", CandidateState::Contradicted, false),
        ]))
        .unwrap();
        assert!(portfolio.blocked_order.contains(&"candidate:b".into()));
        assert!(portfolio
            .negative_evidence
            .iter()
            .any(|value| value.contains("contradicted")));
    }

    #[test]
    fn policy_denial_blocks_without_release() {
        let mut question = question(vec![
            candidate("candidate:a", CandidateState::Supported, false),
            candidate("candidate:b", CandidateState::Supported, false),
        ]);
        question.policy_allow = false;
        let portfolio = assure(&question).unwrap();
        assert_eq!(portfolio.disposition, AssuranceDisposition::Blocked);
        assert_eq!(portfolio.effect_receipts, vec!["block:unsafe-release"]);
    }

    #[test]
    fn duplicate_candidates_are_rejected() {
        let result = assure(&question(vec![
            candidate("candidate:a", CandidateState::Supported, false),
            candidate("candidate:a", CandidateState::Supported, false),
        ]));
        assert!(result.is_err());
    }
}
