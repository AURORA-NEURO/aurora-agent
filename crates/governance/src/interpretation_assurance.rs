//! Prospective high-throughput interpretation and visualization assurance.
//!
//! Atlas feature: `AFA-governance-P14-F27`.
//!
//! Governance certifies the boundary around an interpretation; it does not manufacture a
//! scientific explanation. Every candidate carries result, visualization, evidence, provenance,
//! baseline, uncertainty, and competing-explanation metadata. Only supported, closure-complete
//! candidates cross the assurance gate, while unknown, contradicted, unmeasured, and omitted
//! interpretations remain visible in the report.

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

pub const FEATURE_ID: &str = "AFA-governance-P14-F27";
pub const CONTRACT_VERSION: &str = "governance-interpretation-assurance/1.0";
pub const MAX_CANDIDATES: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterpretationState {
    Supported,
    Unknown,
    Contradicted,
    Unmeasured,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterpretationCandidate {
    pub interpretation_id: String,
    pub scope: String,
    pub result_ids: Vec<String>,
    pub visualization_ids: Vec<String>,
    pub support_milli: u16,
    pub state: InterpretationState,
    pub semantic_digest: ContentHash,
    pub artifact_digest: ContentHash,
    pub evidence_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub baseline_digest: Option<ContentHash>,
    pub replay_identity: ContentHash,
    pub competing_explanations: Vec<String>,
    pub omissions: Vec<String>,
    pub uncertainty: Vec<String>,
    pub raw_data_local: bool,
    pub reproducible: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterpretationAssuranceRequest {
    pub request_id: String,
    pub workflow_id: String,
    pub objective_id: String,
    pub scope: String,
    pub minimum_support_milli: u16,
    pub max_results: usize,
    pub candidates: Vec<InterpretationCandidate>,
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
pub enum InterpretationDisposition {
    Qualified,
    Partial,
    Unknown,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterpretationAssuranceReport {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub workflow_id: String,
    pub objective_id: String,
    pub scope: String,
    pub disposition: InterpretationDisposition,
    pub ranked_order: Vec<String>,
    pub admitted_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub unknown_order: Vec<String>,
    pub result_order: Vec<String>,
    pub visualization_order: Vec<String>,
    pub support_order: Vec<u16>,
    pub semantic_order: Vec<ContentHash>,
    pub artifact_order: Vec<ContentHash>,
    pub evidence_order: Vec<ContentHash>,
    pub provenance_order: Vec<ContentHash>,
    pub baseline_order: Vec<ContentHash>,
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
pub enum InterpretationAssuranceError {
    #[error("invalid interpretation assurance request: {0}")]
    Invalid(String),
    #[error("interpretation assurance artifact failed: {0}")]
    Artifact(String),
    #[error("interpretation assurance serialization failed: {0}")]
    Serialization(String),
}

impl InterpretationAssuranceReport {
    pub fn validate(&self) -> Result<(), InterpretationAssuranceError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || self.request_id.trim().is_empty()
            || self.workflow_id.trim().is_empty()
            || self.objective_id.trim().is_empty()
            || self.scope.trim().is_empty()
            || self.ranked_order.is_empty()
        {
            return Err(InterpretationAssuranceError::Invalid(
                "identity, ranking, locality, or boundary is incomplete".into(),
            ));
        }
        if self.support_order.len() != self.ranked_order.len()
            || self
                .admitted_order
                .iter()
                .any(|value| !self.ranked_order.contains(value))
            || self
                .blocked_order
                .iter()
                .any(|value| !self.ranked_order.contains(value))
            || self
                .unknown_order
                .iter()
                .any(|value| !self.ranked_order.contains(value))
        {
            return Err(InterpretationAssuranceError::Invalid(
                "support or disposition linkage is incomplete".into(),
            ));
        }
        for values in [
            &self.ranked_order,
            &self.admitted_order,
            &self.blocked_order,
            &self.unknown_order,
            &self.result_order,
            &self.visualization_order,
            &self.omissions,
            &self.uncertainty,
            &self.negative_evidence,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(InterpretationAssuranceError::Invalid(
                    "interpretation assurance ordering is not canonical".into(),
                ));
            }
        }
        for values in [
            &self.semantic_order,
            &self.artifact_order,
            &self.evidence_order,
            &self.provenance_order,
            &self.baseline_order,
        ] {
            if values.windows(2).any(|pair| pair[0] >= pair[1]) {
                return Err(InterpretationAssuranceError::Invalid(
                    "interpretation assurance digest ordering is not canonical".into(),
                ));
            }
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| InterpretationAssuranceError::Artifact(error.to_string()))
    }

    pub fn digest(&self) -> Result<ContentHash, InterpretationAssuranceError> {
        self.validate()?;
        let value = serde_json::to_value(self)
            .map_err(|error| InterpretationAssuranceError::Serialization(error.to_string()))?;
        ContentHash::of_value(&value)
            .map_err(|error| InterpretationAssuranceError::Serialization(error.to_string()))
    }
}

pub fn interpretation_assurance_manifest() -> CapabilityManifest {
    CapabilityManifest {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        capability_id: FEATURE_ID.into(),
        version: "0.1.0".into(),
        owner_crate: "governance".into(),
        consumers: [
            "research interpretation lead".into(),
            "visualization reviewer".into(),
            "publication release steward".into(),
        ]
        .into(),
        behavior: "assures prospective interpretation and visualization candidates against evidence, provenance, baseline, replay, omission, competing-explanation, policy, and locality gates without upgrading uncertainty into a conclusion".into(),
        value: "provides a high-throughput, researcher-facing interpretation release gate with reproducible negative and unknown evidence".into(),
        inputs: vec![TypedPort {
            name: "interpretation_assurance_request".into(),
            schema: "InterpretationAssuranceRequest@1".into(),
            required: true,
        }],
        outputs: vec![TypedPort {
            name: "interpretation_assurance_report".into(),
            schema: "InterpretationAssuranceReport@1".into(),
            required: true,
        }],
        effects: [Effect::ReadLocalData, Effect::WriteLocalArtifact, Effect::ExecuteLocalComputation].into(),
        permissions: ["review:research-interpretation".into(), "write:local-interpretation-report".into()].into(),
        determinism: Determinism::ByteStable,
        evidence: vec![EvidenceReference {
            source_id: "w3c-prov-o".into(),
            state: EvidenceState::Supported,
            locator: Some("https://www.w3.org/TR/prov-o/".into()),
        }],
        authority_requirements: Vec::new(),
        autonomy_tier: AutonomyTier::A1,
        surfaces: [ResearchSurface::Ui, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::Cli, ResearchSurface::McpTool, ResearchSurface::Policy, ResearchSurface::Operator].into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    }
}

pub fn assure_interpretations(
    request: &InterpretationAssuranceRequest,
) -> Result<InterpretationAssuranceReport, InterpretationAssuranceError> {
    validate_request(request)?;
    let mut candidates = request.candidates.clone();
    candidates.sort_by(|left, right| {
        right
            .support_milli
            .cmp(&left.support_milli)
            .then(left.interpretation_id.cmp(&right.interpretation_id))
    });
    let ranked_order = candidates
        .iter()
        .map(|candidate| candidate.interpretation_id.clone())
        .collect::<Vec<_>>();
    let mut admitted = Vec::new();
    let mut blocked = BTreeSet::new();
    let mut unknown = BTreeSet::new();
    let mut results = BTreeSet::new();
    let mut visualizations = BTreeSet::new();
    let mut semantics = BTreeSet::new();
    let mut artifacts = BTreeSet::new();
    let mut evidence = BTreeSet::new();
    let mut provenance = BTreeSet::new();
    let mut baselines = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut spent = 0_u64;
    for candidate in &candidates {
        let budget_cost = (candidate.interpretation_id.len()
            + candidate.result_ids.len()
            + candidate.visualization_ids.len()) as u64
            + 1;
        let budget_ok = budget_cost <= request.budget.saturating_sub(spent);
        let complete = candidate.state == InterpretationState::Supported
            && candidate.scope == request.scope
            && candidate.support_milli >= request.minimum_support_milli
            && candidate.baseline_digest.is_some()
            && !candidate.result_ids.is_empty()
            && !candidate.visualization_ids.is_empty()
            && !candidate.competing_explanations.is_empty()
            && candidate.omissions.is_empty()
            && candidate.uncertainty.is_empty()
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
            spent = spent.saturating_add(budget_cost);
            admitted.push(candidate.interpretation_id.clone());
            results.extend(candidate.result_ids.iter().cloned());
            visualizations.extend(candidate.visualization_ids.iter().cloned());
            semantics.insert(candidate.semantic_digest.clone());
            artifacts.insert(candidate.artifact_digest.clone());
            evidence.insert(candidate.evidence_digest.clone());
            provenance.insert(candidate.provenance_digest.clone());
            if let Some(baseline) = &candidate.baseline_digest {
                baselines.insert(baseline.clone());
            }
        } else {
            blocked.insert(candidate.interpretation_id.clone());
            if matches!(
                candidate.state,
                InterpretationState::Unknown | InterpretationState::Unmeasured
            ) {
                unknown.insert(candidate.interpretation_id.clone());
                uncertainty.insert(
                    format!(
                        "interpretation:{}:state-{:?}-not-admitted",
                        candidate.interpretation_id, candidate.state
                    )
                    .to_ascii_lowercase(),
                );
            }
            if candidate.state == InterpretationState::Contradicted {
                negative.insert(format!(
                    "interpretation:{}:contradicted-negative-evidence",
                    candidate.interpretation_id
                ));
            }
            if candidate.scope != request.scope {
                omissions.insert(format!(
                    "interpretation:{}:scope-mismatch",
                    candidate.interpretation_id
                ));
            }
            if candidate.support_milli < request.minimum_support_milli {
                uncertainty.insert(format!(
                    "interpretation:{}:support-below-threshold",
                    candidate.interpretation_id
                ));
            }
            if candidate.baseline_digest.is_none() {
                omissions.insert(format!(
                    "interpretation:{}:baseline-missing",
                    candidate.interpretation_id
                ));
            }
            if candidate.result_ids.is_empty() {
                omissions.insert(format!(
                    "interpretation:{}:result-missing",
                    candidate.interpretation_id
                ));
            }
            if candidate.visualization_ids.is_empty() {
                omissions.insert(format!(
                    "interpretation:{}:visualization-missing",
                    candidate.interpretation_id
                ));
            }
            if candidate.competing_explanations.is_empty() {
                uncertainty.insert(format!(
                    "interpretation:{}:competing-explanations-missing",
                    candidate.interpretation_id
                ));
            }
            if !candidate.omissions.is_empty() {
                uncertainty.insert(format!(
                    "interpretation:{}:protected-closure-incomplete",
                    candidate.interpretation_id
                ));
            }
            if !candidate.raw_data_local || !request.raw_data_local {
                negative.insert(format!(
                    "interpretation:{}:raw-data-locality-failed",
                    candidate.interpretation_id
                ));
            }
            if !candidate.reproducible {
                uncertainty.insert(format!(
                    "interpretation:{}:replay-not-reproducible",
                    candidate.interpretation_id
                ));
            }
            if !budget_ok {
                omissions.insert(format!(
                    "interpretation:{}:budget-exhausted",
                    candidate.interpretation_id
                ));
            }
            if admitted.len() >= request.max_results {
                omissions.insert(format!(
                    "interpretation:{}:result-limit",
                    candidate.interpretation_id
                ));
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
        InterpretationDisposition::Blocked
    } else if admitted.is_empty() {
        InterpretationDisposition::Unknown
    } else if blocked.is_empty()
        && omissions.is_empty()
        && uncertainty.is_empty()
        && negative.is_empty()
    {
        InterpretationDisposition::Qualified
    } else {
        InterpretationDisposition::Partial
    };
    let mut checks: Vec<String> = vec![
        "support ranking is deterministic with interpretation-id tie break".into(),
        "baseline, result, visualization, competing-explanation, evidence, provenance, replay, locality, policy, approval, and budget gates are explicit".into(),
        "unknown, unmeasured, contradicted, omitted, and negative interpretations remain unresolved".into(),
        "the report is a governance assurance artifact and never a clinical or diagnostic decision".into(),
    ];
    checks.sort();
    let payload = json!({
        "schema_version": RESEARCH_CONTRACT_SCHEMA_VERSION,
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "workflow_id": request.workflow_id,
        "objective_id": request.objective_id,
        "scope": request.scope,
        "disposition": disposition,
        "ranked_order": ranked_order,
        "admitted_order": admitted,
        "blocked_order": blocked,
        "unknown_order": unknown,
        "omissions": omissions,
        "uncertainty": uncertainty,
        "negative_evidence": negative,
        "replay_identity": request.replay_identity,
        "boundary": PRECLINICAL_BOUNDARY,
    });
    let artifact = TypedResearchArtifact::from_payload(
        format!("interpretation-assurance-report:{}", request.request_id),
        "application/vnd.aurora.interpretation-assurance+json",
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| InterpretationAssuranceError::Artifact(error.to_string()))?;
    let effect_receipts = if admitted.is_empty() {
        vec!["block:interpretation-assurance-release".into()]
    } else {
        vec![format!(
            "evaluate:interpretation-assurance:{}",
            request.request_id
        )]
    };
    let report = InterpretationAssuranceReport {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        workflow_id: request.workflow_id.clone(),
        objective_id: request.objective_id.clone(),
        scope: request.scope.clone(),
        disposition,
        ranked_order,
        admitted_order: admitted,
        blocked_order: blocked.into_iter().collect(),
        unknown_order: unknown.into_iter().collect(),
        result_order: results.into_iter().collect(),
        visualization_order: visualizations.into_iter().collect(),
        support_order: candidates
            .iter()
            .map(|candidate| candidate.support_milli)
            .collect(),
        semantic_order: semantics.into_iter().collect(),
        artifact_order: artifacts.into_iter().collect(),
        evidence_order: evidence.into_iter().collect(),
        provenance_order: provenance.into_iter().collect(),
        baseline_order: baselines.into_iter().collect(),
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

fn validate_request(
    request: &InterpretationAssuranceRequest,
) -> Result<(), InterpretationAssuranceError> {
    if request.request_id.trim().is_empty()
        || request.workflow_id.trim().is_empty()
        || request.objective_id.trim().is_empty()
        || request.scope.trim().is_empty()
        || request.minimum_support_milli > 1000
        || request.max_results == 0
        || request.candidates.is_empty()
        || request.candidates.len() > MAX_CANDIDATES
        || request.budget == 0
        || request.boundary != PRECLINICAL_BOUNDARY
    {
        return Err(InterpretationAssuranceError::Invalid("request identity, threshold, result limit, candidates, budget, or boundary is incomplete".into()));
    }
    let mut ids = BTreeSet::new();
    for candidate in &request.candidates {
        if candidate.interpretation_id.trim().is_empty()
            || candidate.scope.trim().is_empty()
            || candidate.result_ids.is_empty() && candidate.visualization_ids.is_empty()
            || candidate.support_milli > 1000
            || candidate.boundary != PRECLINICAL_BOUNDARY
            || !ids.insert(candidate.interpretation_id.clone())
        {
            return Err(InterpretationAssuranceError::Invalid(format!(
                "interpretation {} is invalid or duplicated",
                candidate.interpretation_id
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
        support_milli: u16,
        state: InterpretationState,
    ) -> InterpretationCandidate {
        InterpretationCandidate {
            interpretation_id: id.into(),
            scope: "organoid:neural".into(),
            result_ids: vec![format!("result:{id}")],
            visualization_ids: vec![format!("view:{id}")],
            support_milli,
            state,
            semantic_digest: hash(&format!("semantic:{id}")),
            artifact_digest: hash(&format!("artifact:{id}")),
            evidence_digest: hash(&format!("evidence:{id}")),
            provenance_digest: hash(&format!("provenance:{id}")),
            baseline_digest: Some(hash("baseline")),
            replay_identity: hash(&format!("replay:{id}")),
            competing_explanations: vec!["alternative:one".into()],
            omissions: vec![],
            uncertainty: vec![],
            raw_data_local: true,
            reproducible: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    fn request(candidates: Vec<InterpretationCandidate>) -> InterpretationAssuranceRequest {
        InterpretationAssuranceRequest {
            request_id: "request:interpretation".into(),
            workflow_id: "workflow:visualization".into(),
            objective_id: "objective:organoid".into(),
            scope: "organoid:neural".into(),
            minimum_support_milli: 700,
            max_results: 4,
            candidates,
            replay_identity: hash("replay"),
            budget: 1000,
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_typed_a1_and_nonclinical() {
        let manifest = interpretation_assurance_manifest();
        manifest.validate().unwrap();
        assert_eq!(manifest.autonomy_tier, AutonomyTier::A1);
    }
    #[test]
    fn supported_interpretations_are_qualified() {
        let report = assure_interpretations(&request(vec![
            candidate("interpretation:b", 800, InterpretationState::Supported),
            candidate("interpretation:a", 900, InterpretationState::Supported),
        ]))
        .unwrap();
        assert_eq!(report.disposition, InterpretationDisposition::Qualified);
        assert_eq!(
            report.ranked_order,
            vec!["interpretation:a", "interpretation:b"]
        );
        assert_eq!(report.digest().unwrap(), report.digest().unwrap());
    }
    #[test]
    fn unknown_and_contradicted_interpretations_remain_visible() {
        let report = assure_interpretations(&request(vec![
            candidate("interpretation:a", 900, InterpretationState::Supported),
            candidate("interpretation:b", 800, InterpretationState::Unknown),
            candidate("interpretation:c", 700, InterpretationState::Contradicted),
        ]))
        .unwrap();
        assert_eq!(report.disposition, InterpretationDisposition::Partial);
        assert!(report.unknown_order.contains(&"interpretation:b".into()));
        assert!(report
            .negative_evidence
            .iter()
            .any(|item| item.contains("interpretation:c")));
    }
    #[test]
    fn missing_baseline_is_omitted() {
        let mut input = request(vec![candidate(
            "interpretation:a",
            900,
            InterpretationState::Supported,
        )]);
        input.candidates[0].baseline_digest = None;
        let report = assure_interpretations(&input).unwrap();
        assert_eq!(report.disposition, InterpretationDisposition::Unknown);
        assert!(report
            .omissions
            .iter()
            .any(|item| item.contains("baseline-missing")));
    }
    #[test]
    fn policy_denial_blocks_release() {
        let mut input = request(vec![candidate(
            "interpretation:a",
            900,
            InterpretationState::Supported,
        )]);
        input.policy_allow = false;
        let report = assure_interpretations(&input).unwrap();
        assert_eq!(report.disposition, InterpretationDisposition::Blocked);
        assert_eq!(
            report.effect_receipts,
            vec!["block:interpretation-assurance-release"]
        );
    }
    #[test]
    fn duplicate_interpretations_are_rejected() {
        let result = assure_interpretations(&request(vec![
            candidate("interpretation:a", 900, InterpretationState::Supported),
            candidate("interpretation:a", 800, InterpretationState::Supported),
        ]));
        assert!(result.is_err());
    }
}
