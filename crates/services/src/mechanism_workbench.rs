//! Prospective high-throughput mechanism-exploration research workbench.
//!
//! Atlas feature: `AFA-services-P08-F19`.
//!
//! This service owns batch orchestration and release evidence, not a hidden scientific oracle.
//! Callers submit typed, locally retained candidate evidence; the workbench ranks candidates
//! deterministically, admits only reproducible multi-study/multimodal support, and leaves every
//! unknown, contradicted, unmeasured, budget-blocked, or policy-denied candidate in the report.

use bioprism_foundation::{
    AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference, EvidenceState,
    ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-services-P08-F19";
pub const CONTRACT_VERSION: &str = "services-prospective-mechanism-workbench/1.0";
pub const MAX_CANDIDATES: usize = 4096;
pub const MAX_RESULTS: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CandidateState {
    Supported,
    Unknown,
    Contradicted,
    Unmeasured,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismCandidate {
    pub candidate_id: String,
    pub mechanism_id: String,
    pub scope: String,
    pub study_ids: Vec<String>,
    pub modality_ids: Vec<String>,
    pub support_milli: u16,
    pub novelty_milli: u16,
    pub state: CandidateState,
    pub artifact_digest: ContentHash,
    pub evidence_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub budget_cost: u64,
    pub raw_data_local: bool,
    pub reproducible: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismWorkbenchRequest {
    pub request_id: String,
    pub workflow_id: String,
    pub objective_id: String,
    pub target_schema: String,
    pub scope: String,
    pub max_results: usize,
    pub candidates: Vec<MechanismCandidate>,
    pub replay_identity: ContentHash,
    pub budget: u64,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismWorkbenchDisposition {
    Qualified,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismWorkbenchReport {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub objective_id: String,
    pub target_schema: String,
    pub scope: String,
    pub disposition: MechanismWorkbenchDisposition,
    pub ranked_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub mechanism_order: Vec<String>,
    pub study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub score_order: Vec<u16>,
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
pub enum MechanismWorkbenchError {
    #[error("invalid mechanism workbench request: {0}")]
    Invalid(String),
    #[error("mechanism workbench artifact failed: {0}")]
    Artifact(String),
    #[error("mechanism workbench serialization failed: {0}")]
    Serialization(String),
}

impl MechanismWorkbenchReport {
    pub fn validate(&self) -> Result<(), MechanismWorkbenchError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.objective_id.trim().is_empty()
            || self.target_schema.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.ranked_order.is_empty()
        {
            return Err(MechanismWorkbenchError::Invalid(
                "identity, ranking, locality, or boundary is incomplete".into(),
            ));
        }
        for values in [
            &self.ranked_order,
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
                return Err(MechanismWorkbenchError::Invalid(
                    "mechanism workbench ordering is not canonical".into(),
                ));
            }
        }
        if self.score_order.len() != self.ranked_order.len()
            || self
                .admitted_order
                .iter()
                .any(|id| !self.ranked_order.contains(id))
            || self
                .blocked_order
                .iter()
                .any(|id| !self.ranked_order.contains(id))
            || self
                .unknown_order
                .iter()
                .any(|id| !self.ranked_order.contains(id))
        {
            return Err(MechanismWorkbenchError::Invalid(
                "ranking, score, or disposition linkage is incomplete".into(),
            ));
        }
        for values in [
            &self.artifact_order,
            &self.evidence_order,
            &self.provenance_order,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(MechanismWorkbenchError::Invalid(
                    "mechanism workbench digest ordering is not canonical".into(),
                ));
            }
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| MechanismWorkbenchError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, MechanismWorkbenchError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| MechanismWorkbenchError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| MechanismWorkbenchError::Serialization(error.to_string()))
    }
}

pub fn mechanism_workbench_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: "0.1.0".into(),
        owner_crate: "services".into(),
        consumers: [
            "mechanism-exploration lead".into(),
            "experiment-design service".into(),
            "research workbench operator".into(),
        ]
        .into(),
        behavior: "ranks prospective mechanism candidates across high-throughput batches and emits a typed evidence-bearing workbench report with explicit support, uncertainty, contradiction, omission, and budget states".into(),
        value: "turns a large mechanism portfolio into a reproducible researcher-facing shortlist without converting incomplete evidence into a conclusion".into(),
        inputs: vec![TypedPort {
            name: "mechanism_workbench_request".into(),
            schema: "MechanismWorkbenchRequest@1".into(),
            required: true,
        }],
        outputs: vec![TypedPort {
            name: "mechanism_workbench_report".into(),
            schema: "MechanismWorkbenchReport@1".into(),
            required: true,
        }],
        effects: [Effect::ReadLocalData, Effect::WriteLocalArtifact, Effect::ExecuteLocalComputation].into(),
        permissions: ["view:authorized-research-state".into(), "write:local-workbench-artifact".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference {
            source_id: "w3c-prov-o".into(),
            state: EvidenceState::Supported,
            locator: Some("https://www.w3.org/TR/prov-o/".into()),
        }],
        authority_requirements: Vec::new(),
        autonomy_tier: AutonomyTier::A1,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn run_mechanism_workbench(
    request: &MechanismWorkbenchRequest,
) -> Result<MechanismWorkbenchReport, MechanismWorkbenchError> {
    validate_request(request)?;
    let mut candidates = request.candidates.clone();
    candidates.sort_by(|left, right| {
        right
            .support_milli
            .cmp(&left.support_milli)
            .then(right.novelty_milli.cmp(&left.novelty_milli))
            .then(left.candidate_id.cmp(&right.candidate_id))
    });
    let ranked_order = candidates
        .iter()
        .map(|candidate| candidate.candidate_id.clone())
        .collect::<Vec<_>>();
    let mut admitted = Vec::new();
    let mut blocked = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut mechanisms = BTreeSet::new();
    let mut studies = BTreeSet::new();
    let mut modalities = BTreeSet::new();
    let mut scores = Vec::with_capacity(candidates.len());
    let mut artifacts = BTreeSet::new();
    let mut evidence = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut spent = 0_u64;
    for candidate in &candidates {
        scores.push(candidate.support_milli);
        let budget_ok = candidate.budget_cost <= request.budget.saturating_sub(spent);
        let complete = candidate.state == CandidateState::Supported
            && candidate.scope == request.scope
            && candidate.study_ids.len() >= 2
            && candidate.modality_ids.len() >= 2
            && candidate.raw_data_local
            && candidate.reproducible
            && budget_ok;
        let admitted_now = request.policy_allow
            && request.protected_closure
            && request.signed_approval
            && request.raw_data_local
            && complete
            && admitted.len() < request.max_results;
        if admitted_now {
            spent = spent.saturating_add(candidate.budget_cost);
            admitted.push(candidate.candidate_id.clone());
            mechanisms.insert(candidate.mechanism_id.clone());
            studies.extend(candidate.study_ids.iter().cloned());
            modalities.extend(candidate.modality_ids.iter().cloned());
            artifacts.insert(candidate.artifact_digest.clone());
            evidence.insert(candidate.evidence_digest.clone());
            provenance.insert(candidate.provenance_digest.clone());
        } else {
            blocked.insert(candidate.candidate_id.clone());
            if candidate.state == CandidateState::Unknown
                || candidate.state == CandidateState::Unmeasured
            {
                unknown.insert(candidate.candidate_id.clone());
                uncertainty.insert(
                    format!(
                        "candidate:{}:state-{:?}-not-admitted",
                        candidate.candidate_id, candidate.state
                    )
                    .to_ascii_lowercase(),
                );
            }
            if candidate.state == CandidateState::Contradicted {
                negative.insert(format!(
                    "candidate:{}:contradicted-negative-evidence",
                    candidate.candidate_id
                ));
            }
            if candidate.scope != request.scope {
                omissions.insert(format!(
                    "candidate:{}:scope-mismatch",
                    candidate.candidate_id
                ));
            }
            if candidate.study_ids.len() < 2 {
                omissions.insert(format!(
                    "candidate:{}:independent-study-floor",
                    candidate.candidate_id
                ));
            }
            if candidate.modality_ids.len() < 2 {
                omissions.insert(format!(
                    "candidate:{}:multimodal-floor",
                    candidate.candidate_id
                ));
            }
            if !candidate.raw_data_local || !request.raw_data_local {
                negative.insert(format!(
                    "candidate:{}:raw-data-locality-failed",
                    candidate.candidate_id
                ));
            }
            if !candidate.reproducible {
                uncertainty.insert(format!(
                    "candidate:{}:replay-not-reproducible",
                    candidate.candidate_id
                ));
            }
            if !budget_ok {
                omissions.insert(format!(
                    "candidate:{}:budget-exhausted",
                    candidate.candidate_id
                ));
            }
            if admitted.len() >= request.max_results {
                omissions.insert(format!("candidate:{}:result-limit", candidate.candidate_id));
            }
        }
    }
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.signed_approval {
        omissions.insert("request:signed-approval-required".into());
    }
    let disposition = if !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.raw_data_local
    {
        MechanismWorkbenchDisposition::Blocked
    } else if admitted.is_empty() {
        MechanismWorkbenchDisposition::Unknown
    } else if blocked.is_empty()
        && omissions.is_empty()
        && uncertainty.is_empty()
        && negative.is_empty()
    {
        MechanismWorkbenchDisposition::Qualified
    } else {
        MechanismWorkbenchDisposition::Partial
    };
    let mut checks: Vec<String> = vec![
        "support and novelty ranking is deterministic with candidate-id tie break".into(),
        "scope, independent-study, multimodal, reproducibility, locality, policy, approval, and budget gates are explicit".into(),
        "unknown, unmeasured, contradicted, omitted, and negative candidates remain unresolved".into(),
        "high-throughput execution emits a local content-addressed report rather than a clinical decision".into(),
    ];
    checks.sort();
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "workflow_id": request.workflow_id,
        "objective_id": request.objective_id,
        "target_schema": request.target_schema,
        "scope": request.scope,
        "disposition": disposition,
        "ranked_order": ranked_order,
        "admitted_order": admitted,
        "blocked_order": blocked,
        "unknown_order": unknown,
        "score_order": scores,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": negative,
        "replay_identity": request.replay_identity,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("mechanism-workbench-report:{}", request.request_id),
        "application/vnd.aurora.mechanism-workbench-report+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| MechanismWorkbenchError::Artifact(error.to_string()))?;
    let effect_receipts = if admitted.is_empty() {
        vec!["block:mechanism-workbench-release".into()]
    } else {
        vec![format!(
            "write:local-mechanism-workbench:{}",
            request.request_id
        )]
    };
    let report = MechanismWorkbenchReport {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        objective_id: request.objective_id.clone(),
        target_schema: request.target_schema.clone(),
        scope: request.scope.clone(),
        disposition,
        ranked_order,
        admitted_order: admitted,
        blocked_order: blocked.into_iter().collect(),
        unknown_order: unknown.into_iter().collect(),
        mechanism_order: mechanisms.into_iter().collect(),
        study_order: studies.into_iter().collect(),
        modality_order: modalities.into_iter().collect(),
        score_order: scores,
        artifact_order: artifacts.into_iter().collect(),
        evidence_order: evidence.into_iter().collect(),
        provenance_order: provenance.into_iter().collect(),
        omissions: omissions.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        replay_identity: request.replay_identity.clone(),
        effect_receipts,
        artifact,
        raw_data_local: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    report.validate()?;
    Ok(report)
}

fn validate_request(request: &MechanismWorkbenchRequest) -> Result<(), MechanismWorkbenchError> {
    if request.request_id.trim().is_empty()
        || request.workflow_id.trim().is_empty()
        || request.objective_id.trim().is_empty()
        || request.target_schema.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.max_results == 0
        || request.max_results > MAX_RESULTS
        || request.candidates.is_empty()
        || request.candidates.len() > MAX_CANDIDATES
        || request.budget == 0
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(MechanismWorkbenchError::Invalid(
            "request identity, scope, result limit, candidates, budget, or boundary is incomplete"
                .into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for candidate in &request.candidates {
        if candidate.candidate_id.trim().is_empty()
            || candidate.mechanism_id.trim().is_empty()
            || candidate.scope.trim().is_empty()
            || candidate.study_ids.is_empty()
            || candidate.modality_ids.is_empty()
            || candidate.support_milli > 1000
            || candidate.novelty_milli > 1000
            || candidate.boundary != PRECLINICAL_BOUNDARY
            || !ids.insert(candidate.candidate_id.clone())
        {
            return Err(MechanismWorkbenchError::Invalid(format!(
                "candidate {} is invalid or duplicated",
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

    fn candidate(id: &str, support_milli: u16, state: CandidateState) -> MechanismCandidate {
        MechanismCandidate {
            candidate_id: id.into(),
            mechanism_id: format!("mechanism:{id}"),
            scope: "organoid:neural".into(),
            study_ids: vec!["study:imaging".into(), "study:omics".into()],
            modality_ids: vec!["imaging".into(), "omics".into()],
            support_milli,
            novelty_milli: 700,
            state,
            artifact_digest: hash(&format!("artifact:{id}")),
            evidence_digest: hash(&format!("evidence:{id}")),
            provenance_digest: hash(&format!("provenance:{id}")),
            replay_identity: hash(&format!("replay:{id}")),
            budget_cost: 10,
            raw_data_local: true,
            reproducible: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    fn request(candidates: Vec<MechanismCandidate>) -> MechanismWorkbenchRequest {
        MechanismWorkbenchRequest {
            request_id: "request:mechanisms".into(),
            workflow_id: "workflow:mechanism-batch".into(),
            objective_id: "objective:organoid".into(),
            target_schema: "mechanism-workbench/1".into(),
            scope: "organoid:neural".into(),
            max_results: 4,
            candidates,
            replay_identity: hash("replay:batch"),
            budget: 100,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn manifest_is_typed_a1_and_local() {
        let manifest = mechanism_workbench_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A1);
        assert!(manifest.effects.contains(&Effect::WriteLocalArtifact));
    }

    #[test]
    fn ranks_and_admits_supported_candidates() {
        let report = run_mechanism_workbench(&request(vec![
            candidate("candidate:b", 700, CandidateState::Supported),
            candidate("candidate:a", 900, CandidateState::Supported),
        ]))
        .unwrap();
        assert_eq!(report.disposition, MechanismWorkbenchDisposition::Qualified);
        assert_eq!(report.ranked_order, vec!["candidate:a", "candidate:b"]);
        assert_eq!(report.admitted_order, report.ranked_order);
        assert_eq!(report.digest().unwrap(), report.digest().unwrap());
    }

    #[test]
    fn unknown_and_contradicted_candidates_remain_explicit() {
        let report = run_mechanism_workbench(&request(vec![
            candidate("candidate:a", 900, CandidateState::Supported),
            candidate("candidate:b", 800, CandidateState::Unknown),
            candidate("candidate:c", 700, CandidateState::Contradicted),
        ]))
        .unwrap();
        assert_eq!(report.disposition, MechanismWorkbenchDisposition::Partial);
        assert!(report.unknown_order.contains(&"candidate:b".into()));
        assert!(report
            .negative_evidence
            .iter()
            .any(|item| item.contains("candidate:c")));
    }

    #[test]
    fn policy_denial_blocks_batch_release() {
        let mut input = request(vec![candidate(
            "candidate:a",
            900,
            CandidateState::Supported,
        )]);
        input.policy_allow = false;
        let report = run_mechanism_workbench(&input).unwrap();
        assert_eq!(report.disposition, MechanismWorkbenchDisposition::Blocked);
        assert_eq!(
            report.effect_receipts,
            vec!["block:mechanism-workbench-release"]
        );
    }

    #[test]
    fn missing_multimodal_floor_is_omitted() {
        let mut input = request(vec![candidate(
            "candidate:a",
            900,
            CandidateState::Supported,
        )]);
        input.candidates[0].modality_ids = vec!["imaging".into()];
        let report = run_mechanism_workbench(&input).unwrap();
        assert_eq!(report.disposition, MechanismWorkbenchDisposition::Unknown);
        assert!(report
            .omissions
            .iter()
            .any(|item| item.contains("multimodal-floor")));
    }

    #[test]
    fn duplicate_candidates_are_rejected() {
        let result = run_mechanism_workbench(&request(vec![
            candidate("candidate:a", 900, CandidateState::Supported),
            candidate("candidate:a", 800, CandidateState::Supported),
        ]));
        assert!(result.is_err());
    }
}
