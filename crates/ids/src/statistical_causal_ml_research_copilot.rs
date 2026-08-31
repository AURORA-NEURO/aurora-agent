//! Local single-study statistical, causal, and ML analysis research copilot
//! (`AFA-ids-P13-F09`).
//!
//! The copilot compiles caller-supplied, preclinical analysis candidates into a
//! deterministic qualified-analysis plan.  It is deliberately a bounded
//! planning capability: it never fits a model, moves raw observations,
//! publishes a scientific conclusion, or makes a clinical decision.  Every
//! omission, uncertainty, negative result, and authorization failure is
//! retained in the returned receipt so a researcher can replay the decision.

use crate::ContentHash;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "AFA-ids-P13-F09";
pub const CONTRACT_VERSION: &str =
    "ids-local-single-study-statistical-causal-ml-research-copilot/1.0";
pub const INPUT_SCHEMA: &str = "AnalysisCopilotRequest7@1";
pub const OUTPUT_SCHEMA: &str = "QualifiedAnalysisResult10@1";
pub const CONTENT_TYPE: &str = "application/vnd.aurora.qualified-analysis-result-10+json";
pub const PRECLINICAL_BOUNDARY: &str = "preclinical-research-only; no human-subject or clinical-source data; no diagnosis, treatment, triage, enrollment, or clinical decisions";
pub const MAX_CANDIDATES: usize = 4096;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnalysisEvidenceState {
    Proven,
    Supported,
    Unknown,
    Unmeasured,
    Contradicted,
}

/// A typed, pre-registered analysis strategy supplied by an institutional
/// analysis service.  It is a declaration, not a promise that the strategy
/// has already been executed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisCandidate8 {
    pub candidate_id: String,
    pub study_id: String,
    pub estimand: String,
    pub method_family: String,
    pub feature_ids: Vec<String>,
    pub input_schema: String,
    pub output_schema: String,
    pub artifact_digest: ContentHash,
    pub provenance_digest: ContentHash,
    pub replay_identity: ContentHash,
    pub estimated_units: u64,
    pub sample_size: u64,
    pub missingness_milli: i64,
    pub uncertainty_milli: i64,
    pub effect_milli: i64,
    pub robustness_milli: i64,
    pub evidence_state: AnalysisEvidenceState,
    pub deterministic: bool,
    pub local_only: bool,
    pub permitted: bool,
    pub signed: bool,
    pub protected_closure: bool,
    pub negative_result: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnalysisCopilotRequest7 {
    pub request_id: String,
    pub study_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub model_portfolio_version: String,
    pub candidates: Vec<AnalysisCandidate8>,
    pub checkpoint: u64,
    pub max_budget_units: u64,
    pub minimum_candidate_quorum: usize,
    pub minimum_sample_size: u64,
    pub maximum_missingness_milli: i64,
    pub minimum_robustness_milli: i64,
    pub replay_identity: ContentHash,
    pub policy_allow: bool,
    pub protected_closure: bool,
    pub signed_approval: bool,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualifiedAnalysisResult10Artifact {
    pub artifact_id: String,
    pub content_type: String,
    pub content_hash: ContentHash,
    pub semantic_loss: Vec<String>,
    pub provenance_digests: Vec<ContentHash>,
    pub boundary: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualifiedAnalysisResult10 {
    pub schema_version: String,
    pub contract_version: String,
    pub feature_id: String,
    pub request_id: String,
    pub study_id: String,
    pub requester: String,
    pub purpose: String,
    pub semantic_profile: String,
    pub model_portfolio_version: String,
    pub checkpoint: u64,
    pub disposition: String,
    pub candidate_order: Vec<String>,
    pub selected_order: Vec<String>,
    pub fallback_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub blocked_order: Vec<String>,
    pub missing_study_order: Vec<String>,
    pub underpowered_order: Vec<String>,
    pub high_missingness_order: Vec<String>,
    pub non_robust_order: Vec<String>,
    pub omission_order: Vec<String>,
    pub uncertainty_order: Vec<String>,
    pub negative_evidence_order: Vec<String>,
    pub score_order: Vec<i64>,
    pub sample_size_order: Vec<u64>,
    pub total_units: u64,
    pub replay_identity: ContentHash,
    pub analysis_digest: ContentHash,
    pub artifact: QualifiedAnalysisResult10Artifact,
    pub effect_receipts: Vec<String>,
    pub raw_data_local: bool,
    pub aggregate_only: bool,
    pub boundary: String,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum StatisticalCausalMlError {
    #[error("invalid statistical, causal, and ML analysis request: {0}")]
    Invalid(String),
    #[error("qualified analysis artifact failed: {0}")]
    Artifact(String),
}

pub fn statistical_causal_ml_manifest() -> serde_json::Value {
    json!({
        "schema_version": "aurora-research-contract/1.0",
        "capability_id": FEATURE_ID,
        "version": CONTRACT_VERSION,
        "owner_crate": "ids",
        "consumers": ["computational biologist", "biostatistician", "downstream AURORA crate maintainer", "research workbench operator"],
        "behavior": "compiles typed local preclinical analysis candidates into a deterministic, replayable qualified-analysis plan",
        "value": "selects a reproducible method portfolio while exposing power, missingness, robustness, uncertainty, provenance, and policy gates before model execution",
        "input_schema": INPUT_SCHEMA,
        "output_schema": OUTPUT_SCHEMA,
        "effects": ["manage:local-capability", "exchange:permitted-summaries"],
        "permissions": ["read:local-analysis-manifests"],
        "autonomy_tier": "A1",
        "boundary": PRECLINICAL_BOUNDARY
    })
}

impl QualifiedAnalysisResult10 {
    pub fn validate(&self) -> Result<(), StatisticalCausalMlError> {
        if self.schema_version != "aurora-research-contract/1.0"
            || self.contract_version != CONTRACT_VERSION
            || self.feature_id != FEATURE_ID
            || self.boundary != PRECLINICAL_BOUNDARY
            || !self.raw_data_local
            || !self.aggregate_only
            || !all_nonempty([
                &self.request_id,
                &self.study_id,
                &self.requester,
                &self.purpose,
                &self.semantic_profile,
                &self.model_portfolio_version,
            ])
            || self.checkpoint == 0
            || self.candidate_order.is_empty()
            || self.effect_receipts.is_empty()
            || !["qualified", "unresolved", "blocked"].contains(&self.disposition.as_str())
        {
            return Err(StatisticalCausalMlError::Invalid(
                "analysis identity, checkpoint, locality, candidates, or effects are incomplete"
                    .into(),
            ));
        }
        for values in [
            &self.candidate_order,
            &self.selected_order,
            &self.fallback_order,
            &self.unresolved_order,
            &self.blocked_order,
            &self.missing_study_order,
            &self.underpowered_order,
            &self.high_missingness_order,
            &self.non_robust_order,
            &self.omission_order,
            &self.uncertainty_order,
            &self.negative_evidence_order,
            &self.effect_receipts,
        ] {
            if values.windows(2).any(|w| w[0] >= w[1]) {
                return Err(StatisticalCausalMlError::Invalid(
                    "analysis ordering is not canonical".into(),
                ));
            }
        }
        let candidates = BTreeSet::from_iter(self.candidate_order.iter().cloned());
        if candidates.len() != self.candidate_order.len() {
            return Err(StatisticalCausalMlError::Invalid(
                "analysis candidate ids are not unique".into(),
            ));
        }
        let parts = self
            .selected_order
            .iter()
            .chain(&self.fallback_order)
            .chain(&self.unresolved_order)
            .chain(&self.blocked_order)
            .cloned()
            .collect::<Vec<_>>();
        let part_set = BTreeSet::from_iter(parts.iter().cloned());
        if part_set != candidates || part_set.len() != parts.len() {
            return Err(StatisticalCausalMlError::Invalid(
                "analysis candidate states do not partition".into(),
            ));
        }
        if self.selected_order.len() + self.fallback_order.len() != self.score_order.len()
            || self.score_order.len() != self.sample_size_order.len()
        {
            return Err(StatisticalCausalMlError::Artifact(
                "analysis score and sample-size cardinality is inconsistent".into(),
            ));
        }
        if self.artifact.content_type != CONTENT_TYPE
            || self.artifact.boundary != PRECLINICAL_BOUNDARY
            || self.artifact.content_hash != self.analysis_digest
            || self
                .artifact
                .provenance_digests
                .iter()
                .any(|d| d.as_str().len() != 64)
        {
            return Err(StatisticalCausalMlError::Artifact(
                "analysis artifact metadata or digest is inconsistent".into(),
            ));
        }
        if self.effect_receipts.iter().any(|e| {
            !e.starts_with("exchange:permitted-summaries:")
                && !e.starts_with("manage:local-capability:")
                && e != "block:unsafe-release"
        }) {
            return Err(StatisticalCausalMlError::Invalid(
                "effect is outside the governed analysis gate".into(),
            ));
        }
        Ok(())
    }

    pub fn digest(&self) -> Result<ContentHash, StatisticalCausalMlError> {
        self.validate()?;
        ContentHash::of_value(
            &serde_json::to_value(self)
                .map_err(|e| StatisticalCausalMlError::Artifact(e.to_string()))?,
        )
        .map_err(|e| StatisticalCausalMlError::Artifact(e.to_string()))
    }
}

fn all_nonempty<const N: usize>(values: [&String; N]) -> bool {
    values.iter().all(|v| !v.trim().is_empty())
}

fn valid_metric(value: i64) -> bool {
    (0..=1_000).contains(&value)
}

fn score_candidate(candidate: &AnalysisCandidate8) -> i64 {
    // Integer-only scoring makes the ordering byte-stable across Rust,
    // Python, and TypeScript.  Uncertainty and missingness are penalties;
    // absolute effect and sample support are positive evidence, not a claim
    // of biological or clinical efficacy.
    candidate
        .robustness_milli
        .saturating_mul(4)
        .saturating_add((1_000 - candidate.uncertainty_milli).saturating_mul(2))
        .saturating_add((1_000 - candidate.missingness_milli).max(0))
        .saturating_add(candidate.effect_milli.abs().min(1_000))
        .saturating_add(candidate.sample_size.min(1_000) as i64)
}

pub fn compile_statistical_causal_ml(
    request: &AnalysisCopilotRequest7,
) -> Result<QualifiedAnalysisResult10, StatisticalCausalMlError> {
    validate_request(request)?;
    let mut candidates = request.candidates.clone();
    candidates.sort_by(|a, b| a.candidate_id.cmp(&b.candidate_id));
    let candidate_order = candidates
        .iter()
        .map(|c| c.candidate_id.clone())
        .collect::<Vec<_>>();
    let by_id = candidates
        .iter()
        .map(|c| (c.candidate_id.clone(), c))
        .collect::<BTreeMap<_, _>>();
    let mut selected = BTreeSet::new();
    let mut fallback = BTreeSet::new();
    let mut unresolved = BTreeSet::new();
    let mut blocked = BTreeSet::new();
    let mut missing_study = BTreeSet::new();
    let mut underpowered = BTreeSet::new();
    let mut high_missingness = BTreeSet::new();
    let mut non_robust = BTreeSet::new();
    let mut omissions = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut eligible = Vec::<(i64, String)>::new();
    let mut total_units = 0u64;

    for candidate in &candidates {
        let id = candidate.candidate_id.clone();
        total_units = total_units.saturating_add(candidate.estimated_units);
        if candidate.negative_result {
            negative.insert(format!("{id}:negative-result"));
        }
        if candidate.study_id != request.study_id {
            missing_study.insert(id.clone());
            blocked.insert(id);
            continue;
        }
        if !candidate.local_only || !candidate.protected_closure {
            blocked.insert(id);
            if !candidate.local_only {
                omissions.insert(format!("{}:raw-data-not-local", candidate.candidate_id));
            }
            if !candidate.protected_closure {
                uncertainty.insert(format!(
                    "{}:protected-closure-incomplete",
                    candidate.candidate_id
                ));
            }
            continue;
        }
        if candidate.evidence_state == AnalysisEvidenceState::Contradicted {
            blocked.insert(id.clone());
            negative.insert(format!("{id}:contradicted"));
            continue;
        }
        if candidate.replay_identity != request.replay_identity
            || !candidate.deterministic
            || !candidate.permitted
            || !candidate.signed
        {
            unresolved.insert(id.clone());
            omissions.insert(format!("{id}:replay-or-authorization"));
            continue;
        }
        if !matches!(
            candidate.evidence_state,
            AnalysisEvidenceState::Proven | AnalysisEvidenceState::Supported
        ) {
            unresolved.insert(id.clone());
            uncertainty.insert(format!("{id}:evidence-state"));
            continue;
        }
        let mut disqualified = false;
        if candidate.sample_size < request.minimum_sample_size {
            underpowered.insert(id.clone());
            unresolved.insert(id.clone());
            disqualified = true;
        }
        if candidate.missingness_milli > request.maximum_missingness_milli {
            high_missingness.insert(id.clone());
            unresolved.insert(id.clone());
            disqualified = true;
        }
        if candidate.robustness_milli < request.minimum_robustness_milli {
            non_robust.insert(id.clone());
            unresolved.insert(id.clone());
            disqualified = true;
        }
        if disqualified {
            uncertainty.insert(format!("{id}:acceptance-threshold"));
        } else {
            eligible.push((score_candidate(candidate), id));
        }
    }
    eligible.sort_by(|(sa, ia), (sb, ib)| sb.cmp(sa).then_with(|| ia.cmp(ib)));
    if let Some((_, id)) = eligible.first() {
        selected.insert(id.clone());
        for (_, other) in eligible.iter().skip(1) {
            fallback.insert(other.clone());
        }
    }
    if total_units > request.max_budget_units {
        omissions.insert(format!("request:budget-exceeded:{total_units}"));
    }
    if eligible.len() < request.minimum_candidate_quorum {
        uncertainty.insert("candidate:minimum-quorum-unmet".into());
    }
    let global_block = !request.policy_allow
        || !request.protected_closure
        || !request.signed_approval
        || !request.raw_data_local
        || !request.aggregate_only;
    if !request.policy_allow {
        negative.insert("request:policy-denied".into());
    }
    if !request.protected_closure {
        uncertainty.insert("request:protected-closure-incomplete".into());
    }
    if !request.signed_approval {
        uncertainty.insert("request:signed-approval-missing".into());
    }
    if global_block {
        blocked.extend(candidate_order.iter().cloned());
        selected.clear();
        fallback.clear();
        unresolved.clear();
        omissions.insert("request:analysis-not-authorized".into());
    }
    let disposition = if global_block || selected.is_empty() && !blocked.is_empty() {
        "blocked"
    } else if selected.is_empty()
        || total_units > request.max_budget_units
        || eligible.len() < request.minimum_candidate_quorum
    {
        "unresolved"
    } else {
        "qualified"
    };
    if disposition != "qualified" {
        omissions.insert("request:analysis-not-release-ready".into());
    }

    let selected_order = selected.iter().cloned().collect::<Vec<_>>();
    let fallback_order = fallback.iter().cloned().collect::<Vec<_>>();
    let unresolved_order = unresolved.iter().cloned().collect::<Vec<_>>();
    let blocked_order = blocked.iter().cloned().collect::<Vec<_>>();
    let score_map = eligible.into_iter().collect::<BTreeMap<_, _>>();
    let score_order = selected_order
        .iter()
        .chain(&fallback_order)
        .map(|id| {
            score_map
                .iter()
                .find_map(|(score, candidate_id)| (candidate_id == id).then_some(*score))
                .unwrap_or_default()
        })
        .collect::<Vec<_>>();
    let sample_size_order = selected_order
        .iter()
        .chain(&fallback_order)
        .map(|id| by_id[id].sample_size)
        .collect::<Vec<_>>();
    let payload = json!({
        "schema_version": "aurora-research-contract/1.0",
        "contract_version": CONTRACT_VERSION,
        "feature_id": FEATURE_ID,
        "request_id": request.request_id,
        "study_id": request.study_id,
        "requester": request.requester,
        "purpose": request.purpose,
        "semantic_profile": request.semantic_profile,
        "model_portfolio_version": request.model_portfolio_version,
        "checkpoint": request.checkpoint,
        "disposition": disposition,
        "candidate_order": candidate_order,
        "selected_order": selected_order,
        "fallback_order": fallback_order,
        "unresolved_order": unresolved_order,
        "blocked_order": blocked_order,
        "missing_study_order": missing_study,
        "underpowered_order": underpowered,
        "high_missingness_order": high_missingness,
        "non_robust_order": non_robust,
        "omission_order": omissions,
        "uncertainty_order": uncertainty,
        "negative_evidence_order": negative,
        "score_order": score_order,
        "sample_size_order": sample_size_order,
        "total_units": total_units,
        "replay_identity": request.replay_identity,
        "boundary": PRECLINICAL_BOUNDARY
    });
    let digest = ContentHash::of_value(&payload)
        .map_err(|e| StatisticalCausalMlError::Artifact(e.to_string()))?;
    let artifact = QualifiedAnalysisResult10Artifact {
        artifact_id: format!("qualified-analysis-result-10:{}", request.request_id),
        content_type: CONTENT_TYPE.into(),
        content_hash: digest.clone(),
        semantic_loss: omissions.iter().cloned().collect(),
        provenance_digests: candidates
            .iter()
            .map(|c| c.provenance_digest.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    let effect_receipts = if disposition == "qualified" {
        vec![
            format!("exchange:permitted-summaries:{}", request.request_id),
            format!("manage:local-capability:{}", request.request_id),
        ]
    } else {
        vec!["block:unsafe-release".into()]
    };
    let receipt = QualifiedAnalysisResult10 {
        schema_version: "aurora-research-contract/1.0".into(),
        contract_version: CONTRACT_VERSION.into(),
        feature_id: FEATURE_ID.into(),
        request_id: request.request_id.clone(),
        study_id: request.study_id.clone(),
        requester: request.requester.clone(),
        purpose: request.purpose.clone(),
        semantic_profile: request.semantic_profile.clone(),
        model_portfolio_version: request.model_portfolio_version.clone(),
        checkpoint: request.checkpoint,
        disposition: disposition.into(),
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
        fallback_order: payload["fallback_order"]
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
        missing_study_order: payload["missing_study_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        underpowered_order: payload["underpowered_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        high_missingness_order: payload["high_missingness_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        non_robust_order: payload["non_robust_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        omission_order: payload["omission_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        uncertainty_order: payload["uncertainty_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        negative_evidence_order: payload["negative_evidence_order"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_str().unwrap().into())
            .collect(),
        score_order,
        sample_size_order,
        total_units,
        replay_identity: request.replay_identity.clone(),
        analysis_digest: digest,
        artifact,
        effect_receipts,
        raw_data_local: true,
        aggregate_only: true,
        boundary: PRECLINICAL_BOUNDARY.into(),
    };
    receipt.validate()?;
    Ok(receipt)
}

fn validate_request(request: &AnalysisCopilotRequest7) -> Result<(), StatisticalCausalMlError> {
    if !all_nonempty([
        &request.request_id,
        &request.study_id,
        &request.requester,
        &request.purpose,
        &request.semantic_profile,
        &request.model_portfolio_version,
    ]) || request.candidates.is_empty()
        || request.candidates.len() > MAX_CANDIDATES
        || request.checkpoint == 0
        || request.max_budget_units == 0
        || request.minimum_candidate_quorum == 0
        || request.minimum_candidate_quorum > request.candidates.len()
        || request.replay_identity.as_str().len() != 64
        || request.maximum_missingness_milli < 0
        || request.maximum_missingness_milli > 1_000
        || request.minimum_robustness_milli < 0
        || request.minimum_robustness_milli > 1_000
        || request.boundary != PRECLINICAL_BOUNDARY
        || !request.raw_data_local
        || !request.aggregate_only
    {
        return Err(StatisticalCausalMlError::Invalid(
            "request identity, bounds, candidates, thresholds, replay, locality, or boundary is invalid".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    for candidate in &request.candidates {
        if !all_nonempty([
            &candidate.candidate_id,
            &candidate.study_id,
            &candidate.estimand,
            &candidate.method_family,
            &candidate.input_schema,
            &candidate.output_schema,
        ]) || !ids.insert(candidate.candidate_id.clone())
            || candidate.feature_ids.is_empty()
            || candidate.feature_ids.windows(2).any(|w| w[0] >= w[1])
            || candidate.feature_ids.iter().any(|f| f.trim().is_empty())
            || candidate.artifact_digest.as_str().len() != 64
            || candidate.provenance_digest.as_str().len() != 64
            || candidate.replay_identity.as_str().len() != 64
            || candidate.estimated_units == 0
            || candidate.sample_size == 0
            || !valid_metric(candidate.missingness_milli)
            || !valid_metric(candidate.uncertainty_milli)
            || !valid_metric(candidate.robustness_milli)
        {
            return Err(StatisticalCausalMlError::Invalid(
                "candidate identity, feature contract, bounds, or digest is invalid".into(),
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn h(value: &str) -> ContentHash {
        ContentHash::of_bytes(value.as_bytes())
    }

    fn candidate(id: &str, score: i64) -> AnalysisCandidate8 {
        AnalysisCandidate8 {
            candidate_id: id.into(),
            study_id: "study:one".into(),
            estimand: "effect:exposure-outcome".into(),
            method_family: "doubly-robust".into(),
            feature_ids: vec!["feature:age".into(), "feature:signal".into()],
            input_schema: "AnnData@0.10".into(),
            output_schema: "EffectEstimate@1".into(),
            artifact_digest: h(id),
            provenance_digest: h("provenance"),
            replay_identity: h("replay"),
            estimated_units: 10,
            sample_size: 100,
            missingness_milli: 50,
            uncertainty_milli: 100,
            effect_milli: score,
            robustness_milli: 900,
            evidence_state: AnalysisEvidenceState::Supported,
            deterministic: true,
            local_only: true,
            permitted: true,
            signed: true,
            protected_closure: true,
            negative_result: false,
        }
    }

    fn request() -> AnalysisCopilotRequest7 {
        AnalysisCopilotRequest7 {
            request_id: "request:analysis".into(),
            study_id: "study:one".into(),
            requester: "computational-biologist".into(),
            purpose: "preclinical-mechanism-analysis".into(),
            semantic_profile: "neuro:analysis:v1".into(),
            model_portfolio_version: "portfolio:2026.1".into(),
            candidates: vec![candidate("analysis:a", 100), candidate("analysis:b", 50)],
            checkpoint: 2,
            max_budget_units: 100,
            minimum_candidate_quorum: 1,
            minimum_sample_size: 50,
            maximum_missingness_milli: 200,
            minimum_robustness_milli: 700,
            replay_identity: h("replay"),
            policy_allow: true,
            protected_closure: true,
            signed_approval: true,
            raw_data_local: true,
            aggregate_only: true,
            boundary: PRECLINICAL_BOUNDARY.into(),
        }
    }

    #[test]
    fn manifest_is_a1() {
        assert_eq!(statistical_causal_ml_manifest()["autonomy_tier"], "A1");
    }

    #[test]
    fn nominal_selects_deterministic_candidate_and_fallback() {
        let report = compile_statistical_causal_ml(&request()).unwrap();
        assert_eq!(report.disposition, "qualified");
        assert_eq!(report.selected_order, vec!["analysis:a"]);
        assert_eq!(report.fallback_order, vec!["analysis:b"]);
        assert_eq!(report.digest().unwrap(), report.digest().unwrap());
    }

    #[test]
    fn underpowered_candidate_is_unresolved() {
        let mut request = request();
        request.candidates[0].sample_size = 10;
        request.candidates[1].sample_size = 10;
        let report = compile_statistical_causal_ml(&request).unwrap();
        assert_eq!(report.disposition, "unresolved");
        assert_eq!(report.underpowered_order.len(), 2);
    }

    #[test]
    fn missing_study_is_blocked() {
        let mut request = request();
        request.candidates[0].study_id = "study:other".into();
        let report = compile_statistical_causal_ml(&request).unwrap();
        assert!(report.blocked_order.contains(&"analysis:a".into()));
        assert!(report.missing_study_order.contains(&"analysis:a".into()));
    }

    #[test]
    fn policy_denial_blocks_every_candidate() {
        let mut request = request();
        request.policy_allow = false;
        let report = compile_statistical_causal_ml(&request).unwrap();
        assert_eq!(report.disposition, "blocked");
        assert!(report.selected_order.is_empty());
        assert_eq!(report.blocked_order.len(), 2);
        assert_eq!(report.effect_receipts, vec!["block:unsafe-release"]);
    }

    #[test]
    fn contradiction_is_negative_and_blocked() {
        let mut request = request();
        request.candidates[0].evidence_state = AnalysisEvidenceState::Contradicted;
        let report = compile_statistical_causal_ml(&request).unwrap();
        assert!(report
            .negative_evidence_order
            .iter()
            .any(|x| x.contains("contradicted")));
        assert!(report.blocked_order.contains(&"analysis:a".into()));
    }

    #[test]
    fn high_missingness_is_explicit() {
        let mut request = request();
        request.candidates[0].missingness_milli = 900;
        let report = compile_statistical_causal_ml(&request).unwrap();
        assert!(report.high_missingness_order.contains(&"analysis:a".into()));
        assert_eq!(report.disposition, "qualified");
        assert_eq!(report.selected_order, vec!["analysis:b"]);
    }
}
