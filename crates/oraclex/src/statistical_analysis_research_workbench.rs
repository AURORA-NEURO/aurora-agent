//! Federated-continual statistical/causal/ML analysis workbench (`AFA-oraclex-P13-F20`).
//!
//! This is an admission and interpretation surface, not a model executor. It qualifies
//! caller-supplied analysis attestations and keeps identification gaps, uncertainty, omissions,
//! negative results, and policy failures visible in a deterministic artifact.

use bioprism_foundation::{
    AuthorityRequirement, AutonomyTier, CapabilityManifest, Determinism, Effect, EvidenceReference,
    EvidenceState, ResearchSurface, TypedPort, TypedResearchArtifact, PRECLINICAL_BOUNDARY,
    RESEARCH_CONTRACT_SCHEMA_VERSION,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-oraclex-P13-F20";
pub const CONTRACT_VERSION: &str =
    "oraclex-federated-continual-statistical-causal-ml-analysis-research-workbench/1.0";
pub const INPUT_SCHEMA: &str = "AnalysisQuestion4@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedAnalysisResult5@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.oraclex-qualified-analysis-result-5+json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisCandidate5 {
    pub candidate_id: String,
    pub study_id: String,
    pub modality: String,
    pub model_id: String,
    pub estimand: String,
    pub evidence_state: EvidenceState,
    pub input_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub identification_supported: bool,
    pub comparability_supported: bool,
    pub quality_supported: bool,
    pub signed: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub negative_result: bool,
    pub omission_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisQuestion4 {
    pub schema_version: String,
    pub request_id: String,
    pub researcher: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub required_candidate_order: Vec<String>,
    pub required_study_order: Vec<String>,
    pub required_modality_order: Vec<String>,
    pub required_model_order: Vec<String>,
    pub candidates: Vec<AnalysisCandidate5>,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub adversarial_event_order: Vec<String>,
    pub budget_units: u64,
    pub boundary: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisDisposition {
    Qualified,
    Unresolved,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualifiedAnalysisResult5 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub researcher: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub disposition: AnalysisDisposition,
    pub candidate_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub missing_candidate_order: Vec<String>,
    pub study_order: Vec<String>,
    pub selected_study_order: Vec<String>,
    pub unresolved_study_order: Vec<String>,
    pub blocked_study_order: Vec<String>,
    pub missing_study_order: Vec<String>,
    pub modality_order: Vec<String>,
    pub selected_modality_order: Vec<String>,
    pub unresolved_modality_order: Vec<String>,
    pub blocked_modality_order: Vec<String>,
    pub missing_modality_order: Vec<String>,
    pub model_order: Vec<String>,
    pub selected_model_order: Vec<String>,
    pub unresolved_model_order: Vec<String>,
    pub blocked_model_order: Vec<String>,
    pub missing_model_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub contradiction_order: Vec<String>,
    pub adversarial_event_order: Vec<String>,
    pub budget_used_units: u64,
    pub replay_identity: ContentHash,
    pub provenance_digest: ContentHash,
    pub analysis_digest: ContentHash,
    pub artifact: TypedResearchArtifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum StatisticalAnalysisWorkbenchError {
    #[error("invalid Oraclex statistical analysis request or receipt: {0}")]
    Invalid(String),
    #[error("Oraclex statistical analysis artifact failed: {0}")]
    Artifact(String),
}

fn invalid(message: impl Into<String>) -> StatisticalAnalysisWorkbenchError {
    StatisticalAnalysisWorkbenchError::Invalid(message.into())
}
fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}
fn valid_digest(value: &ContentHash) -> bool {
    value.as_str().len() == 64
}
fn partition(universe: &[String], parts: &[&[String]]) -> bool {
    let expected = universe.iter().cloned().collect::<BTreeSet<_>>();
    let flat = parts
        .iter()
        .flat_map(|part| part.iter().cloned())
        .collect::<Vec<_>>();
    expected.len() == universe.len()
        && flat.len() == expected.len()
        && flat.iter().cloned().collect::<BTreeSet<_>>() == expected
}

pub fn statistical_analysis_research_workbench_manifest() -> CapabilityManifest {
    CapabilityManifest { schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(), capability_id: FEATURE_ID.into(), version: CONTRACT_VERSION.into(), owner_crate: "oraclex".into(), consumers: ["preclinical neuroscientist".into(), "analysis workbench operator".into(), "federated research verifier".into()].into(), behavior: "qualify federated continual statistical, causal, and ML analysis attestations with identification, comparability, quality, evidence, provenance, replay, and locality gates without executing models".into(), value: "makes analytical readiness and uncertainty auditable before computation while retaining negative and omitted evidence".into(), inputs: vec![TypedPort { name: "analysis_question".into(), schema: INPUT_SCHEMA.into(), required: true }], outputs: vec![TypedPort { name: "qualified_analysis_result".into(), schema: OUTPUT_SCHEMA.into(), required: true }], effects: [Effect::ExecuteLocalComputation, Effect::WriteLocalArtifact].into(), permissions: ["analyze:declared-local-portfolio".into()].into(), determinism: Determinism::ByteStable, evidence: vec![EvidenceReference { source_id: "w3c-prov-o".into(), state: EvidenceState::Supported, locator: Some("https://www.w3.org/TR/prov-o/".into()) }, EvidenceReference { source_id: "ro-crate-1.3".into(), state: EvidenceState::Supported, locator: Some("https://www.researchobject.org/ro-crate/specification.html".into()) }], authority_requirements: vec![AuthorityRequirement { role: "analysis workbench operator".into(), reason: "analysis admission consumes governed local attestations and requires explicit authority".into() }], autonomy_tier: AutonomyTier::A1, surfaces: [ResearchSurface::Ui, ResearchSurface::Cli, ResearchSurface::Api, ResearchSurface::Sdk, ResearchSurface::McpTool, ResearchSurface::Protocol, ResearchSurface::Policy, ResearchSurface::Operator].into(), boundary: PRECLINICAL_BOUNDARY.into() }
}

impl QualifiedAnalysisResult5 {
    pub fn validate(&self) -> Result<(), StatisticalAnalysisWorkbenchError> {
        if self.schema_version != RESEARCH_CONTRACT_SCHEMA_VERSION
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.content_type != CONTENT_TYPE
            || !self.raw_data_local
            || !self.aggregate_only
            || self.request_id.trim().is_empty()
            || self.researcher.trim().is_empty()
            || self.purpose.trim().is_empty()
            || self.semantic_profile.trim().is_empty()
            || self.candidate_order.is_empty()
            || self.study_order.is_empty()
            || self.modality_order.is_empty()
            || self.model_order.is_empty()
            || self.effect_receipts.is_empty()
        {
            return Err(invalid(
                "analysis identity, closure, locality, or effects are incomplete",
            ));
        }
        for values in [
            &self.candidate_order,
            &self.selected_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.missing_candidate_order,
            &self.study_order,
            &self.selected_study_order,
            &self.unresolved_study_order,
            &self.blocked_study_order,
            &self.missing_study_order,
            &self.modality_order,
            &self.selected_modality_order,
            &self.unresolved_modality_order,
            &self.blocked_modality_order,
            &self.missing_modality_order,
            &self.model_order,
            &self.selected_model_order,
            &self.unresolved_model_order,
            &self.blocked_model_order,
            &self.missing_model_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.contradiction_order,
            &self.adversarial_event_order,
            &self.effect_receipts,
        ] {
            if !canonical(values) {
                return Err(invalid("analysis ordering is not canonical"));
            }
        }
        let mut candidates = self.candidate_order.clone();
        candidates.extend(self.missing_candidate_order.iter().cloned());
        candidates.sort();
        if !partition(
            &candidates,
            &[
                &self.selected_order,
                &self.unresolved_order,
                &self.blocked_order,
                &self.missing_candidate_order,
            ],
        ) || !partition(
            &self.study_order,
            &[
                &self.selected_study_order,
                &self.unresolved_study_order,
                &self.blocked_study_order,
                &self.missing_study_order,
            ],
        ) || !partition(
            &self.modality_order,
            &[
                &self.selected_modality_order,
                &self.unresolved_modality_order,
                &self.blocked_modality_order,
                &self.missing_modality_order,
            ],
        ) || !partition(
            &self.model_order,
            &[
                &self.selected_model_order,
                &self.unresolved_model_order,
                &self.blocked_model_order,
                &self.missing_model_order,
            ],
        ) {
            return Err(invalid("analysis states do not partition"));
        }
        if !valid_digest(&self.replay_identity)
            || !valid_digest(&self.provenance_digest)
            || !valid_digest(&self.analysis_digest)
            || self.artifact.content_hash != self.analysis_digest
        {
            return Err(StatisticalAnalysisWorkbenchError::Artifact(
                "analysis digest or artifact hash is inconsistent".into(),
            ));
        }
        if self.effect_receipts.iter().any(|effect| {
            effect != "block:unsafe-release" && !effect.starts_with("analyze:local-portfolio:")
        }) {
            return Err(invalid("analysis effect is outside local portfolio gate"));
        }
        if self.disposition == AnalysisDisposition::Qualified
            && self.effect_receipts != [format!("analyze:local-portfolio:{}", self.request_id)]
        {
            return Err(invalid("qualified analysis effect is invalid"));
        }
        if self.disposition != AnalysisDisposition::Qualified
            && self.effect_receipts != ["block:unsafe-release".to_string()]
        {
            return Err(invalid("non-qualified analysis must block"));
        }
        self.artifact
            .validate_metadata()
            .map_err(|error| StatisticalAnalysisWorkbenchError::Artifact(error.to_string()))
    }
    pub fn digest(&self) -> Result<ContentHash, StatisticalAnalysisWorkbenchError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|e| StatisticalAnalysisWorkbenchError::Artifact(e.to_string()))?,
        )
        .map_err(|e| StatisticalAnalysisWorkbenchError::Artifact(e.to_string()))
    }
}

pub fn qualify_statistical_analysis(
    request: &AnalysisQuestion4,
) -> Result<QualifiedAnalysisResult5, StatisticalAnalysisWorkbenchError> {
    if request.schema_version != INPUT_SCHEMA
        || request.request_id.trim().is_empty()
        || request.researcher.trim().is_empty()
        || request.purpose.trim().is_empty()
        || request.semantic_profile.trim().is_empty()
        || request.required_candidate_order.is_empty()
        || request.required_study_order.is_empty()
        || request.required_modality_order.is_empty()
        || request.required_model_order.is_empty()
        || !canonical(&request.required_candidate_order)
        || !canonical(&request.required_study_order)
        || !canonical(&request.required_modality_order)
        || !canonical(&request.required_model_order)
        || !canonical(&request.adversarial_event_order)
        || !valid_digest(&request.replay_identity)
        || request.budget_units == 0
        || !request.raw_data_local
        || !request.aggregate_only
        || request.boundary != PRECLINICAL_BOUNDARY
        || request.candidates.is_empty()
    {
        return Err(invalid(
            "analysis request identity, closure, replay, budget, locality, or boundary is invalid",
        ));
    }
    let mut rows = request.candidates.clone();
    rows.sort_by(|left, right| {
        let rank = |state: EvidenceState| match state {
            EvidenceState::Proven => 0,
            EvidenceState::Supported => 1,
            EvidenceState::Speculative => 2,
            EvidenceState::Unknown => 3,
            EvidenceState::Contradicted => 4,
        };
        (rank(left.evidence_state), left.candidate_id.clone())
            .cmp(&(rank(right.evidence_state), right.candidate_id.clone()))
    });
    let candidate_order = rows
        .iter()
        .map(|row| row.candidate_id.clone())
        .collect::<Vec<_>>();
    if candidate_order.iter().any(|id| id.trim().is_empty())
        || candidate_order.windows(2).any(|pair| pair[0] == pair[1])
    {
        return Err(invalid("analysis candidates must have unique identifiers"));
    }
    let mut selected = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut omission = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut contradiction = BTreeSet::new();
    let mut budget_used = 0_u64;
    for row in &rows {
        budget_used = budget_used
            .checked_add(1)
            .ok_or_else(|| invalid("analysis budget overflow"))?;
        omission.extend(
            row.omission_order
                .iter()
                .map(|item| format!("{}:{}", row.candidate_id, item)),
        );
        if row.negative_result {
            negative.insert(row.candidate_id.clone());
        }
        if row.evidence_state == EvidenceState::Contradicted {
            contradiction.insert(row.candidate_id.clone());
        }
        let hard = !row.identification_supported
            || !row.comparability_supported
            || !row.quality_supported
            || !row.signed
            || !row.raw_data_local
            || !row.aggregate_only
            || row.evidence_state == EvidenceState::Contradicted;
        let soft = row.replay_identity != request.replay_identity
            || matches!(
                row.evidence_state,
                EvidenceState::Unknown | EvidenceState::Speculative
            )
            || !row.omission_order.is_empty();
        if hard {
            blocked.insert(row.candidate_id.clone());
        } else if soft {
            unresolved.insert(row.candidate_id.clone());
            uncertainty.insert(format!("{}:readiness-or-replay", row.candidate_id));
        } else {
            selected.insert(row.candidate_id.clone());
        }
    }
    let required = request
        .required_candidate_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let missing = required
        .difference(&candidate_order.iter().cloned().collect())
        .cloned()
        .collect::<BTreeSet<_>>();
    omission.extend(
        missing
            .iter()
            .map(|id| format!("request:missing-candidate:{}", id)),
    );
    let mut studies = request
        .required_study_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut modalities = request
        .required_modality_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut models = request
        .required_model_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    for row in &rows {
        studies.insert(row.study_id.clone());
        modalities.insert(row.modality.clone());
        models.insert(row.model_id.clone());
    }
    let groups = |axis: u8,
                  universe: &BTreeSet<String>|
     -> (
        BTreeSet<String>,
        BTreeSet<String>,
        BTreeSet<String>,
        BTreeSet<String>,
    ) {
        let key = |id: &String, set: &BTreeSet<String>| {
            rows.iter().any(|row| {
                set.contains(&row.candidate_id)
                    && match axis {
                        0 => &row.study_id,
                        1 => &row.modality,
                        _ => &row.model_id,
                    } == id
            })
        };
        let a = universe
            .iter()
            .filter(|id| key(id, &selected))
            .cloned()
            .collect::<BTreeSet<_>>();
        let b = universe
            .iter()
            .filter(|id| !a.contains(*id) && key(id, &unresolved))
            .cloned()
            .collect::<BTreeSet<_>>();
        let c = universe
            .iter()
            .filter(|id| !a.contains(*id) && !b.contains(*id) && key(id, &blocked))
            .cloned()
            .collect::<BTreeSet<_>>();
        let d = universe
            .difference(&a)
            .filter(|id| !b.contains(*id) && !c.contains(*id))
            .cloned()
            .collect::<BTreeSet<_>>();
        (a, b, c, d)
    };
    let (ss, us, bs, ms) = groups(0, &studies);
    let (sm, um, bm, mm) = groups(1, &modalities);
    let (sx, ux, bx, mx) = groups(2, &models);
    let global_block = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.raw_data_local
        || !request.aggregate_only
        || !request.adversarial_event_order.is_empty()
        || budget_used > request.budget_units;
    if global_block {
        blocked.extend(candidate_order.iter().cloned());
        selected.clear();
        unresolved.clear();
        omission.insert("control:policy-closure-approval-locality-or-budget-blocked".into());
    }
    uncertainty.extend(
        request
            .adversarial_event_order
            .iter()
            .map(|event| format!("adversarial:{}", event)),
    );
    let selected_order = selected.into_iter().collect::<Vec<_>>();
    let unresolved_order = unresolved.into_iter().collect::<Vec<_>>();
    let blocked_order = blocked.into_iter().collect::<Vec<_>>();
    let missing_order = missing.into_iter().collect::<Vec<_>>();
    let disposition = if global_block || !blocked_order.is_empty() || !missing_order.is_empty() {
        AnalysisDisposition::Blocked
    } else if selected_order.is_empty()
        || !unresolved_order.is_empty()
        || !us.is_empty()
        || !um.is_empty()
        || !ux.is_empty()
    {
        AnalysisDisposition::Unresolved
    } else {
        AnalysisDisposition::Qualified
    };
    let effects = if disposition == AnalysisDisposition::Qualified {
        vec![format!("analyze:local-portfolio:{}", request.request_id)]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let provenance = ContentHash::of_bytes(
        rows.iter()
            .map(|row| row.provenance_digest.to_string())
            .collect::<Vec<_>>()
            .join("|")
            .as_bytes(),
    );
    let payload = json!({"schema_version":RESEARCH_CONTRACT_SCHEMA_VERSION,"contract_version":CONTRACT_VERSION,"feature_id":FEATURE_ID,"request_id":request.request_id,"researcher":request.researcher,"purpose":request.purpose,"semantic_profile":request.semantic_profile,"disposition":disposition,"candidate_order":candidate_order,"selected_order":selected_order,"unresolved_order":unresolved_order,"blocked_order":blocked_order,"missing_candidate_order":missing_order,"study_order":studies,"selected_study_order":ss,"unresolved_study_order":us,"blocked_study_order":bs,"missing_study_order":ms,"modality_order":modalities,"selected_modality_order":sm,"unresolved_modality_order":um,"blocked_modality_order":bm,"missing_modality_order":mm,"model_order":models,"selected_model_order":sx,"unresolved_model_order":ux,"blocked_model_order":bx,"missing_model_order":mx,"omission_order":omission,"uncertainty_order":uncertainty,"negative_evidence_order":negative,"contradiction_order":contradiction,"adversarial_event_order":request.adversarial_event_order,"budget_used_units":budget_used,"replay_identity":request.replay_identity,"provenance_digest":provenance,"effect_receipts":effects,"raw_data_local":true,"aggregate_only":true,"boundary":PRECLINICAL_BOUNDARY});
    let artifact = TypedResearchArtifact::from_payload(
        format!("oraclex-analysis-result-5:{}", request.request_id),
        CONTENT_TYPE,
        &payload,
        Vec::new(),
        Vec::new(),
    )
    .map_err(|error| StatisticalAnalysisWorkbenchError::Artifact(error.to_string()))?;
    let analysis_digest = artifact.content_hash.clone();
    let result = QualifiedAnalysisResult5 {
        schema_version: RESEARCH_CONTRACT_SCHEMA_VERSION.into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        researcher: request.researcher.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        disposition,
        candidate_order: payload["candidate_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        selected_order: payload["selected_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        unresolved_order: payload["unresolved_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        blocked_order: payload["blocked_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        missing_candidate_order: payload["missing_candidate_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        study_order: studies.into_iter().collect(),
        selected_study_order: ss.into_iter().collect(),
        unresolved_study_order: us.into_iter().collect(),
        blocked_study_order: bs.into_iter().collect(),
        missing_study_order: ms.into_iter().collect(),
        modality_order: modalities.into_iter().collect(),
        selected_modality_order: sm.into_iter().collect(),
        unresolved_modality_order: um.into_iter().collect(),
        blocked_modality_order: bm.into_iter().collect(),
        missing_modality_order: mm.into_iter().collect(),
        model_order: models.into_iter().collect(),
        selected_model_order: sx.into_iter().collect(),
        unresolved_model_order: ux.into_iter().collect(),
        blocked_model_order: bx.into_iter().collect(),
        missing_model_order: mx.into_iter().collect(),
        omission_order: omission.into_iter().collect(),
        uncertainty_order: uncertainty.into_iter().collect(),
        negative_evidence_order: negative.into_iter().collect(),
        contradiction_order: contradiction.into_iter().collect(),
        adversarial_event_order: request.adversarial_event_order.clone(),
        budget_used_units: budget_used,
        replay_identity: request.replay_identity.clone(),
        provenance_digest: provenance,
        analysis_digest,
        artifact,
        effect_receipts: effects,
        raw_data_local: true,
        aggregate_only: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    result.validate()?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn h(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }
    fn candidate(id: &str, state: EvidenceState) -> AnalysisCandidate5 {
        AnalysisCandidate5 {
            candidate_id: id.into(),
            study_id: format!("study:{id}"),
            modality: "imaging".into(),
            model_id: format!("model:{id}"),
            estimand: "effect".into(),
            evidence_state: state,
            input_digest: h(id),
            provenance_digest: h(&format!("p:{id}")),
            replay_identity: h("replay"),
            identification_supported: true,
            comparability_supported: true,
            quality_supported: true,
            signed: true,
            raw_data_local: true,
            aggregate_only: true,
            negative_result: false,
            omission_order: vec![],
        }
    }
    fn request(candidates: Vec<AnalysisCandidate5>) -> AnalysisQuestion4 {
        AnalysisQuestion4 {
            schema_version: INPUT_SCHEMA.into(),
            request_id: "req".into(),
            researcher: "researcher".into(),
            purpose: "analysis".into(),
            semantic_profile: "imaging-omics".into(),
            required_candidate_order: vec!["a".into()],
            required_study_order: vec!["study:a".into()],
            required_modality_order: vec!["imaging".into()],
            required_model_order: vec!["model:a".into()],
            candidates,
            replay_identity: h("replay"),
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            aggregate_only: true,
            adversarial_event_order: vec![],
            budget_units: 10,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }
    #[test]
    fn manifest_is_a1() {
        assert_eq!(
            statistical_analysis_research_workbench_manifest().autonomy_tier,
            AutonomyTier::A1
        )
    }
    #[test]
    fn qualified() {
        assert_eq!(
            qualify_statistical_analysis(&request(vec![candidate("a", EvidenceState::Supported)]))
                .unwrap()
                .disposition,
            AnalysisDisposition::Qualified
        )
    }
    #[test]
    fn unknown_unresolved() {
        assert_eq!(
            qualify_statistical_analysis(&request(vec![candidate("a", EvidenceState::Unknown)]))
                .unwrap()
                .disposition,
            AnalysisDisposition::Unresolved
        )
    }
    #[test]
    fn contradiction_blocks() {
        assert_eq!(
            qualify_statistical_analysis(&request(vec![candidate(
                "a",
                EvidenceState::Contradicted
            )]))
            .unwrap()
            .disposition,
            AnalysisDisposition::Blocked
        )
    }
    #[test]
    fn negative_retained() {
        let mut c = candidate("a", EvidenceState::Supported);
        c.negative_result = true;
        assert!(!qualify_statistical_analysis(&request(vec![c]))
            .unwrap()
            .negative_evidence_order
            .is_empty())
    }
}
