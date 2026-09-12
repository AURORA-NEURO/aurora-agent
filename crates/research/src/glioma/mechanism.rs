//! Competing-mechanism exploration for glioma research programs.
//!
//! A mechanism portfolio is a product output: it binds candidate mechanisms to observed artifact
//! ids, predicted discriminators, and model-system coverage.  It is not a truth oracle.  Ranking
//! is a deterministic triage for what to investigate next, while disconfirming evidence and
//! missing coverage remain visible in the result.

use super::super::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P05-F01";
pub const OUTPUT_SCHEMA: &str = "GliomaMechanismPortfolio1@1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismCandidate {
    pub mechanism_id: String,
    pub statement: String,
    pub supporting_evidence_order: Vec<String>,
    pub disconfirming_evidence_order: Vec<String>,
    pub predicted_modalities: BTreeSet<GliomaModality>,
    pub predicted_model_systems: BTreeSet<GliomaModelSystem>,
    pub support_milli: u16,
    pub reproducibility_milli: u16,
    pub discriminating_action_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismRequest {
    pub objective: String,
    pub required_modalities: BTreeSet<GliomaModality>,
    pub required_model_systems: BTreeSet<GliomaModelSystem>,
    pub max_candidates: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismRanking {
    pub mechanism_id: String,
    pub score_milli: u16,
    pub modality_coverage_milli: u16,
    pub model_coverage_milli: u16,
    pub unresolved_order: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismDisposition {
    Qualified,
    Partial,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismPortfolio {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub ranking: Vec<MechanismRanking>,
    pub selected_order: Vec<String>,
    pub unresolved_order: Vec<String>,
    pub contradicted_order: Vec<String>,
    pub discriminating_action_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: MechanismDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MechanismError {
    #[error("mechanism request is invalid: {0}")]
    InvalidRequest(String),
    #[error("mechanism candidate is invalid: {0}")]
    InvalidCandidate(String),
    #[error("mechanism portfolio is invalid: {0}")]
    InvalidOutput(String),
    #[error("mechanism digest failed: {0}")]
    Digest(String),
}

fn coverage<T: Ord + Copy>(required: &BTreeSet<T>, predicted: &BTreeSet<T>) -> u16 {
    if required.is_empty() {
        return 1_000;
    }
    ((required.intersection(predicted).count() * 1_000) / required.len()) as u16
}

fn digest_input(portfolio: &MechanismPortfolio) -> serde_json::Value {
    serde_json::json!({
        "feature_id": portfolio.feature_id,
        "output_schema": portfolio.output_schema,
        "objective": portfolio.objective,
        "ranking": portfolio.ranking,
        "selected_order": portfolio.selected_order,
        "unresolved_order": portfolio.unresolved_order,
        "contradicted_order": portfolio.contradicted_order,
        "discriminating_action_order": portfolio.discriminating_action_order,
        "negative_evidence": portfolio.negative_evidence,
        "uncertainty": portfolio.uncertainty,
        "disposition": portfolio.disposition,
    })
}

impl MechanismPortfolio {
    pub fn validate(&self) -> Result<(), MechanismError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.ranking.iter().any(|item| {
                item.score_milli > 1_000
                    || item.modality_coverage_milli > 1_000
                    || item.model_coverage_milli > 1_000
            })
            || self
                .unresolved_order
                .windows(2)
                .any(|pair| pair[0] > pair[1])
            || self
                .contradicted_order
                .windows(2)
                .any(|pair| pair[0] > pair[1])
            || self
                .discriminating_action_order
                .windows(2)
                .any(|pair| pair[0] > pair[1])
            || self
                .negative_evidence
                .windows(2)
                .any(|pair| pair[0] > pair[1])
            || self.uncertainty.windows(2).any(|pair| pair[0] > pair[1])
        {
            return Err(MechanismError::InvalidOutput(
                "identity, score bounds, or ordering is invalid".into(),
            ));
        }
        let ranking_order = self
            .ranking
            .iter()
            .map(|item| item.mechanism_id.clone())
            .collect::<Vec<_>>();
        if self.ranking.windows(2).any(|pair| {
            pair[0].score_milli < pair[1].score_milli
                || (pair[0].score_milli == pair[1].score_milli
                    && pair[0].mechanism_id > pair[1].mechanism_id)
        }) || self.selected_order
            != ranking_order
                .iter()
                .take(self.selected_order.len())
                .cloned()
                .collect::<Vec<_>>()
        {
            return Err(MechanismError::InvalidOutput(
                "ranking and selected order do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|e| MechanismError::Digest(e.to_string()))?;
        if expected != self.digest {
            return Err(MechanismError::InvalidOutput(
                "digest is not bound to the mechanism portfolio".into(),
            ));
        }
        Ok(())
    }
}

pub fn explore_mechanisms(
    request: &MechanismRequest,
    candidates: &[MechanismCandidate],
) -> Result<MechanismPortfolio, MechanismError> {
    if request.objective.trim().is_empty() || request.max_candidates == 0 {
        return Err(MechanismError::InvalidRequest(
            "objective and candidate bound are required".into(),
        ));
    }
    let mut ids = BTreeSet::new();
    let mut ranked = Vec::with_capacity(candidates.len());
    for candidate in candidates {
        if candidate.mechanism_id.trim().is_empty()
            || candidate.statement.trim().is_empty()
            || candidate.support_milli > 1_000
            || candidate.reproducibility_milli > 1_000
            || candidate
                .supporting_evidence_order
                .windows(2)
                .any(|pair| pair[0] > pair[1])
            || candidate
                .disconfirming_evidence_order
                .windows(2)
                .any(|pair| pair[0] > pair[1])
            || candidate
                .discriminating_action_order
                .windows(2)
                .any(|pair| pair[0] > pair[1])
            || !ids.insert(candidate.mechanism_id.clone())
        {
            return Err(MechanismError::InvalidCandidate(
                "identity, scores, evidence ordering, or uniqueness is invalid".into(),
            ));
        }
        let modality_coverage_milli = coverage(
            &request.required_modalities,
            &candidate.predicted_modalities,
        );
        let model_coverage_milli = coverage(
            &request.required_model_systems,
            &candidate.predicted_model_systems,
        );
        let contradiction_penalty = if candidate.disconfirming_evidence_order.is_empty() {
            0
        } else {
            150
        };
        let score_milli = ((candidate.support_milli as u32 * 45
            + candidate.reproducibility_milli as u32 * 25
            + modality_coverage_milli as u32 * 20
            + model_coverage_milli as u32 * 10)
            / 100)
            .saturating_sub(contradiction_penalty)
            .min(1_000) as u16;
        let mut unresolved_order = Vec::new();
        if modality_coverage_milli < 1_000 {
            unresolved_order.push("required-modality-coverage-incomplete".into());
        }
        if model_coverage_milli < 1_000 {
            unresolved_order.push("required-model-coverage-incomplete".into());
        }
        if !candidate.disconfirming_evidence_order.is_empty() {
            unresolved_order.push("disconfirming-evidence-present".into());
        }
        ranked.push((
            score_milli,
            candidate.mechanism_id.clone(),
            MechanismRanking {
                mechanism_id: candidate.mechanism_id.clone(),
                score_milli,
                modality_coverage_milli,
                model_coverage_milli,
                unresolved_order,
            },
            candidate.disconfirming_evidence_order.is_empty(),
            candidate.discriminating_action_order.clone(),
        ));
    }
    ranked.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
    let selected = ranked
        .iter()
        .take(request.max_candidates)
        .map(|item| item.1.clone())
        .collect::<Vec<_>>();
    let selected_set = selected.iter().cloned().collect::<BTreeSet<_>>();
    let unresolved = ranked
        .iter()
        .filter(|item| !selected_set.contains(&item.1) || !item.2.unresolved_order.is_empty())
        .map(|item| item.1.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let contradicted = ranked
        .iter()
        .filter(|item| !item.3)
        .map(|item| item.1.clone())
        .collect::<Vec<_>>();
    let mut actions = BTreeSet::new();
    for item in &ranked {
        actions.extend(item.4.iter().cloned());
    }
    let mut negative = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    if candidates.is_empty() {
        negative.insert("no-mechanism-candidates-provided".into());
    }
    if !contradicted.is_empty() {
        negative.insert("disconfirming-evidence-attached-to-candidate".into());
    }
    if !unresolved.is_empty() {
        uncertainty.insert("mechanism-coverage-or-identifiability-incomplete".into());
    }
    let disposition = if selected.is_empty() {
        MechanismDisposition::Unresolved
    } else if !unresolved.is_empty() || !contradicted.is_empty() {
        MechanismDisposition::Partial
    } else {
        MechanismDisposition::Qualified
    };
    let mut portfolio = MechanismPortfolio {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        ranking: ranked.into_iter().map(|item| item.2).collect(),
        selected_order: selected,
        unresolved_order: unresolved,
        contradicted_order: contradicted,
        discriminating_action_order: actions.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_value(&serde_json::json!({}))
            .map_err(|e| MechanismError::Digest(e.to_string()))?,
    };
    portfolio.digest = ContentHash::of_value(&digest_input(&portfolio))
        .map_err(|e| MechanismError::Digest(e.to_string()))?;
    portfolio.validate()?;
    Ok(portfolio)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn candidate(id: &str, modality: GliomaModality) -> MechanismCandidate {
        MechanismCandidate {
            mechanism_id: id.into(),
            statement: format!("mechanism-{id}"),
            supporting_evidence_order: vec![format!("evidence-{id}")],
            disconfirming_evidence_order: Vec::new(),
            predicted_modalities: BTreeSet::from([modality]),
            predicted_model_systems: BTreeSet::from([GliomaModelSystem::Organoid]),
            support_milli: 800,
            reproducibility_milli: 700,
            discriminating_action_order: vec![format!("assay-{id}")],
        }
    }

    #[test]
    fn mechanism_portfolio_ranks_candidates_and_retains_disconfirming_evidence() {
        let request = MechanismRequest {
            objective: "map invasion mechanism".into(),
            required_modalities: BTreeSet::from([
                GliomaModality::Genomics,
                GliomaModality::Imaging,
            ]),
            required_model_systems: BTreeSet::from([GliomaModelSystem::Organoid]),
            max_candidates: 2,
        };
        let mut first = candidate("a", GliomaModality::Genomics);
        first.disconfirming_evidence_order = vec!["negative-a".into()];
        let second = candidate("b", GliomaModality::Imaging);
        let portfolio = explore_mechanisms(&request, &[first, second]).unwrap();
        assert_eq!(portfolio.selected_order, vec!["b", "a"]);
        assert_eq!(portfolio.contradicted_order, vec!["a"]);
        assert_eq!(portfolio.disposition, MechanismDisposition::Partial);
        portfolio.validate().unwrap();
    }

    #[test]
    fn no_candidates_is_unresolved() {
        let request = MechanismRequest {
            objective: "map".into(),
            required_modalities: BTreeSet::new(),
            required_model_systems: BTreeSet::new(),
            max_candidates: 1,
        };
        let portfolio = explore_mechanisms(&request, &[]).unwrap();
        assert_eq!(portfolio.disposition, MechanismDisposition::Unresolved);
        assert!(portfolio
            .negative_evidence
            .iter()
            .any(|item| item.contains("no-mechanism")));
    }
}
