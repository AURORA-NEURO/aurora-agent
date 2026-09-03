//! Deterministic robustness analysis for preclinical glioma outcomes.
//!
//! A single two-arm result is not a research conclusion.  This module compiles an analysis into
//! a bounded omission battery (leave-one-batch-out and, optionally, leave-one-row-out), reruns
//! the declared estimand on every surviving subset, and reports whether the result is stable,
//! fragile, null, or unresolved.  The battery is deliberately conservative: an unresolved
//! omission is never counted as support, and a null result is published rather than upgraded by
//! the surrounding workflow.  It operates on local, de-identified preclinical rows only.

use crate::glioma::analysis::{
    analyze_preclinical_outcomes, AnalysisDataset, AnalysisDisposition, AnalysisRequest,
};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P09-F01";
pub const OUTPUT_SCHEMA: &str = "GliomaRobustnessSuite1@1";
pub const MAX_CASES: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RobustnessCaseKind {
    LeaveOneBatchOut,
    LeaveOneRowOut,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RobustnessRequest {
    pub objective: String,
    pub analysis: AnalysisRequest,
    pub max_cases: usize,
    pub include_row_jackknife: bool,
    pub min_eligible_cases: usize,
    pub min_stability_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RobustnessCase {
    pub case_id: String,
    pub kind: RobustnessCaseKind,
    pub omitted_rows: Vec<String>,
    pub omitted_batches: Vec<String>,
    pub effect_milli: i64,
    pub uncertainty_milli: u64,
    pub disposition: AnalysisDisposition,
    pub analysis_digest: ContentHash,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RobustnessDisposition {
    Stable,
    Fragile,
    Null,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RobustnessSuite {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub primary_digest: ContentHash,
    pub primary_effect_milli: i64,
    pub case_order: Vec<String>,
    pub cases: Vec<RobustnessCase>,
    pub eligible_case_count: usize,
    pub stable_case_count: usize,
    pub effect_low_milli: i64,
    pub effect_high_milli: i64,
    pub stability_milli: u16,
    pub direction_concordance_milli: u16,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: RobustnessDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum RobustnessError {
    #[error("robustness request is invalid: {0}")]
    InvalidRequest(String),
    #[error("robustness case is invalid: {0}")]
    InvalidCase(String),
    #[error("robustness output is invalid: {0}")]
    InvalidOutput(String),
    #[error("robustness digest failed: {0}")]
    Digest(String),
    #[error("robustness analysis failed: {0}")]
    Analysis(String),
}

fn sign(value: i64) -> i8 {
    if value > 0 {
        1
    } else if value < 0 {
        -1
    } else {
        0
    }
}

fn digest_input(suite: &RobustnessSuite) -> serde_json::Value {
    serde_json::json!({
        "feature_id": suite.feature_id,
        "output_schema": suite.output_schema,
        "objective": suite.objective,
        "primary_digest": suite.primary_digest,
        "primary_effect_milli": suite.primary_effect_milli,
        "case_order": suite.case_order,
        "cases": suite.cases,
        "eligible_case_count": suite.eligible_case_count,
        "stable_case_count": suite.stable_case_count,
        "effect_low_milli": suite.effect_low_milli,
        "effect_high_milli": suite.effect_high_milli,
        "stability_milli": suite.stability_milli,
        "direction_concordance_milli": suite.direction_concordance_milli,
        "negative_evidence": suite.negative_evidence,
        "uncertainty": suite.uncertainty,
        "disposition": suite.disposition,
    })
}

fn sorted_unique(values: impl IntoIterator<Item = String>) -> Vec<String> {
    values
        .into_iter()
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

impl RobustnessCase {
    pub fn validate(&self) -> Result<(), RobustnessError> {
        if self.case_id.trim().is_empty()
            || self.omitted_rows.windows(2).any(|pair| pair[0] >= pair[1])
            || self
                .omitted_batches
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self
                .negative_evidence
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.uncertainty.windows(2).any(|pair| pair[0] >= pair[1])
            || self.analysis_digest.as_str().len() != 64
            || match self.kind {
                RobustnessCaseKind::LeaveOneBatchOut => {
                    !self.omitted_rows.is_empty() || self.omitted_batches.len() != 1
                }
                RobustnessCaseKind::LeaveOneRowOut => {
                    self.omitted_rows.len() != 1 || !self.omitted_batches.is_empty()
                }
            }
        {
            return Err(RobustnessError::InvalidCase(
                "identity, omission, ordering, or digest shape is invalid".into(),
            ));
        }
        Ok(())
    }
}

impl RobustnessSuite {
    pub fn validate(&self) -> Result<(), RobustnessError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.primary_digest.as_str().len() != 64
            || self.case_order.len() != self.cases.len()
            || self.case_order.windows(2).any(|pair| pair[0] >= pair[1])
            || self.cases.iter().any(|case| case.validate().is_err())
            || self
                .cases
                .iter()
                .map(|case| &case.case_id)
                .collect::<Vec<_>>()
                != self.case_order.iter().collect::<Vec<_>>()
            || self.eligible_case_count > self.cases.len()
            || self.stable_case_count > self.eligible_case_count
            || self.stability_milli > 1_000
            || self.direction_concordance_milli > 1_000
            || self
                .negative_evidence
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.uncertainty.windows(2).any(|pair| pair[0] >= pair[1])
        {
            return Err(RobustnessError::InvalidOutput(
                "identity, case ordering, counts, bounds, or ordering is invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| RobustnessError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(RobustnessError::InvalidOutput(
                "digest is not bound to the robustness suite".into(),
            ));
        }
        Ok(())
    }
}

fn subset(
    dataset: &AnalysisDataset,
    omitted_rows: &BTreeSet<String>,
    omitted_batches: &BTreeSet<String>,
) -> AnalysisDataset {
    let mut subset = dataset.clone();
    subset.rows.retain(|row| {
        !omitted_rows.contains(&row.row_id) && !omitted_batches.contains(&row.batch_id)
    });
    subset
}

/// Run a bounded leave-one-batch/row-out robustness battery over one declared analysis.
pub fn assess_glioma_robustness(
    request: &RobustnessRequest,
    dataset: &AnalysisDataset,
) -> Result<RobustnessSuite, RobustnessError> {
    if request.objective.trim().is_empty()
        || request.max_cases == 0
        || request.max_cases > MAX_CASES
        || request.min_eligible_cases == 0
        || request.min_eligible_cases > request.max_cases
        || request.min_stability_milli > 1_000
        || request.analysis.objective.trim().is_empty()
        || request.analysis.effect_threshold_milli == 0
    {
        return Err(RobustnessError::InvalidRequest(
            "objective, bounded case budget, eligible-case floor, stability threshold, and effect threshold are required".into(),
        ));
    }
    let primary = analyze_preclinical_outcomes(&request.analysis, dataset)
        .map_err(|error| RobustnessError::Analysis(error.to_string()))?;

    let batch_ids = dataset
        .rows
        .iter()
        .map(|row| row.batch_id.clone())
        .collect::<BTreeSet<_>>();
    let row_ids = dataset
        .rows
        .iter()
        .map(|row| row.row_id.clone())
        .collect::<BTreeSet<_>>();
    let candidate_count = batch_ids.len()
        + if request.include_row_jackknife {
            row_ids.len()
        } else {
            0
        };
    let truncated = candidate_count > request.max_cases;
    let mut specs = Vec::<(String, RobustnessCaseKind, Vec<String>, Vec<String>)>::new();
    for batch in batch_ids {
        specs.push((
            format!("leave-one-batch-out:{batch}"),
            RobustnessCaseKind::LeaveOneBatchOut,
            Vec::new(),
            vec![batch],
        ));
    }
    if request.include_row_jackknife {
        for row in row_ids {
            specs.push((
                format!("leave-one-row-out:{row}"),
                RobustnessCaseKind::LeaveOneRowOut,
                vec![row],
                Vec::new(),
            ));
        }
    }
    specs.sort_by(|left, right| left.0.cmp(&right.0));
    specs.truncate(request.max_cases);

    let mut cases = Vec::with_capacity(specs.len());
    for (case_id, kind, omitted_rows, omitted_batches) in specs {
        let omitted_rows = sorted_unique(omitted_rows);
        let omitted_batches = sorted_unique(omitted_batches);
        let row_set = omitted_rows.iter().cloned().collect::<BTreeSet<_>>();
        let batch_set = omitted_batches.iter().cloned().collect::<BTreeSet<_>>();
        let result =
            analyze_preclinical_outcomes(&request.analysis, &subset(dataset, &row_set, &batch_set))
                .map_err(|error| RobustnessError::Analysis(error.to_string()))?;
        cases.push(RobustnessCase {
            case_id,
            kind,
            omitted_rows,
            omitted_batches,
            effect_milli: result.effect_milli,
            uncertainty_milli: result.uncertainty_milli,
            disposition: result.disposition,
            analysis_digest: result.digest,
            negative_evidence: result.negative_evidence,
            uncertainty: result.uncertainty,
        });
    }
    cases.sort_by(|left, right| left.case_id.cmp(&right.case_id));
    let case_order = cases
        .iter()
        .map(|case| case.case_id.clone())
        .collect::<Vec<_>>();
    let eligible_case_count = cases
        .iter()
        .filter(|case| !matches!(case.disposition, AnalysisDisposition::Unresolved))
        .count();
    let primary_sign = sign(primary.effect_milli);
    let stable_cases = cases
        .iter()
        .filter(|case| !matches!(case.disposition, AnalysisDisposition::Unresolved))
        .filter(|case| match primary.disposition {
            AnalysisDisposition::Qualified => {
                case.disposition == AnalysisDisposition::Qualified
                    && sign(case.effect_milli) == primary_sign
                    && case.effect_milli.unsigned_abs() >= request.analysis.effect_threshold_milli
            }
            AnalysisDisposition::Negative => {
                case.disposition == AnalysisDisposition::Negative
                    && case.effect_milli.unsigned_abs() < request.analysis.effect_threshold_milli
            }
            _ => false,
        })
        .count();
    let direction_matches = cases
        .iter()
        .filter(|case| !matches!(case.disposition, AnalysisDisposition::Unresolved))
        .filter(|case| sign(case.effect_milli) == primary_sign)
        .count();
    let stability_milli = (stable_cases * 1_000)
        .checked_div(eligible_case_count)
        .unwrap_or_default()
        .min(1_000) as u16;
    let direction_concordance_milli = (direction_matches * 1_000)
        .checked_div(eligible_case_count)
        .unwrap_or_default()
        .min(1_000) as u16;
    let (effect_low_milli, effect_high_milli) = cases
        .iter()
        .map(|case| case.effect_milli)
        .chain(std::iter::once(primary.effect_milli))
        .fold(
            (primary.effect_milli, primary.effect_milli),
            |(low, high), effect| (low.min(effect), high.max(effect)),
        );

    let mut negative = primary
        .negative_evidence
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut uncertainty = primary.uncertainty.iter().cloned().collect::<BTreeSet<_>>();
    if truncated {
        uncertainty.insert("robustness-case-budget-truncated".into());
    }
    if cases.is_empty() || eligible_case_count < request.min_eligible_cases {
        uncertainty.insert("minimum-eligible-robustness-cases-not-met".into());
    }
    if cases
        .iter()
        .any(|case| case.disposition == AnalysisDisposition::Unresolved)
    {
        uncertainty.insert("one-or-more-omission-cases-unresolved".into());
    }
    let disposition = if primary.disposition == AnalysisDisposition::Unresolved
        || eligible_case_count < request.min_eligible_cases
        || !cases.is_empty() && eligible_case_count != cases.len()
    {
        RobustnessDisposition::Unresolved
    } else if primary.disposition == AnalysisDisposition::Negative
        && stable_cases == eligible_case_count
    {
        RobustnessDisposition::Null
    } else if stability_milli >= request.min_stability_milli {
        RobustnessDisposition::Stable
    } else {
        negative.insert("declared-effect-not-stable-under-omission".into());
        RobustnessDisposition::Fragile
    };
    if matches!(disposition, RobustnessDisposition::Fragile) {
        negative.insert("robustness-direction-or-threshold-crossing".into());
    }
    let mut suite = RobustnessSuite {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        primary_digest: primary.digest,
        primary_effect_milli: primary.effect_milli,
        case_order,
        cases,
        eligible_case_count,
        stable_case_count: stable_cases,
        effect_low_milli,
        effect_high_milli,
        stability_milli,
        direction_concordance_milli,
        negative_evidence: negative.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_value(&serde_json::json!({}))
            .map_err(|error| RobustnessError::Digest(error.to_string()))?,
    };
    suite.digest = ContentHash::of_value(&digest_input(&suite))
        .map_err(|error| RobustnessError::Digest(error.to_string()))?;
    suite.validate()?;
    Ok(suite)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::analysis::AnalysisRow;
    use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};

    fn dataset(values: &[(String, String, i64)]) -> AnalysisDataset {
        AnalysisDataset {
            dataset_id: "robustness-dataset".into(),
            artifact: LocalArtifactRef {
                artifact_id: "artifact-robustness".into(),
                content_hash: ContentHash::of_value(&serde_json::json!({"dataset": "robustness"}))
                    .unwrap(),
                content_type: "application/vnd.aurora.glioma-analysis+json".into(),
                local_only: true,
                contains_human_data: false,
                contains_direct_identifiers: false,
            },
            rows: values
                .iter()
                .enumerate()
                .map(|(index, (arm, batch, outcome))| AnalysisRow {
                    row_id: format!("row-{index}"),
                    arm_id: arm.clone(),
                    model_system: GliomaModelSystem::Organoid,
                    batch_id: batch.clone(),
                    outcome_milli: *outcome,
                })
                .collect(),
        }
    }

    fn request(min_replicates: usize) -> RobustnessRequest {
        RobustnessRequest {
            objective: "stress-test a glioma invasion effect".into(),
            analysis: AnalysisRequest {
                objective: "estimate invasion effect".into(),
                control_arm: "control".into(),
                treatment_arm: "treated".into(),
                model_system: GliomaModelSystem::Organoid,
                min_replicates_per_arm: min_replicates,
                effect_threshold_milli: 100,
                alpha_milli: 50,
            },
            max_cases: 16,
            include_row_jackknife: false,
            min_eligible_cases: 2,
            min_stability_milli: 900,
        }
    }

    #[test]
    fn concordant_leave_one_batch_out_is_stable_and_replayable() {
        let data = dataset(&[
            ("control".into(), "b1".into(), 100),
            ("control".into(), "b2".into(), 105),
            ("control".into(), "b3".into(), 95),
            ("control".into(), "b4".into(), 102),
            ("treated".into(), "b5".into(), 300),
            ("treated".into(), "b6".into(), 305),
            ("treated".into(), "b7".into(), 295),
            ("treated".into(), "b8".into(), 301),
        ]);
        let first = assess_glioma_robustness(&request(3), &data).unwrap();
        let second = assess_glioma_robustness(&request(3), &data).unwrap();
        assert_eq!(first, second);
        assert_eq!(first.disposition, RobustnessDisposition::Stable);
        assert_eq!(first.stability_milli, 1_000);
        first.validate().unwrap();
    }

    #[test]
    fn omission_floor_is_unresolved_not_hidden_as_stability() {
        let data = dataset(&[
            ("control".into(), "b1".into(), 100),
            ("control".into(), "b2".into(), 100),
            ("control".into(), "b3".into(), 100),
            ("treated".into(), "b4".into(), 300),
            ("treated".into(), "b5".into(), 300),
            ("treated".into(), "b6".into(), 300),
        ]);
        let suite = assess_glioma_robustness(&request(3), &data).unwrap();
        assert_eq!(suite.disposition, RobustnessDisposition::Unresolved);
        assert!(suite
            .uncertainty
            .iter()
            .any(|item| item.contains("omission-cases")));
    }

    #[test]
    fn null_effect_is_published_as_null_when_omissions_agree() {
        let data = dataset(&[
            ("control".into(), "b1".into(), 100),
            ("control".into(), "b2".into(), 100),
            ("control".into(), "b3".into(), 100),
            ("control".into(), "b4".into(), 100),
            ("treated".into(), "b5".into(), 100),
            ("treated".into(), "b6".into(), 100),
            ("treated".into(), "b7".into(), 100),
            ("treated".into(), "b8".into(), 100),
        ]);
        let suite = assess_glioma_robustness(&request(3), &data).unwrap();
        assert_eq!(suite.disposition, RobustnessDisposition::Null);
        assert_eq!(suite.primary_effect_milli, 0);
    }
}
