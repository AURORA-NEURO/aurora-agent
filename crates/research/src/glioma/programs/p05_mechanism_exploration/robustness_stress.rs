//! Adversarial robustness stress surfaces for preclinical glioma mechanisms.
//!
//! Mechanism rankings are often sensitive to measurement error, missing model systems, and
//! plausible alternative interpretations. This feature evaluates bounded stress scenarios and
//! makes rank reversals, unmeasured omissions, and brittle evidence visible before the next action
//! is selected.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P05-F07";
pub const OUTPUT_SCHEMA: &str = "GliomaMechanismRobustnessStress1@2";
pub const MAX_MECHANISMS: usize = 512;
pub const MAX_SCENARIOS: usize = 1_024;
pub const MAX_ABS_DELTA_MILLI: i32 = 1_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismStressCandidate {
    pub mechanism_id: String,
    pub baseline_score_milli: u16,
    pub baseline_uncertainty_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismStressAdjustment {
    pub mechanism_id: String,
    pub delta_milli: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismStressScenario {
    pub scenario_id: String,
    pub weight_milli: u16,
    pub adjustment_order: Vec<MechanismStressAdjustment>,
    pub omitted_mechanism_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismRobustnessStressRequest {
    pub objective: String,
    pub candidates: Vec<MechanismStressCandidate>,
    pub scenarios: Vec<MechanismStressScenario>,
    pub min_stability_milli: u16,
    pub max_mechanisms: usize,
    pub max_scenarios: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "availability", rename_all = "snake_case")]
pub enum MechanismStressScenarioScore {
    Available {
        scenario_id: String,
        score_milli: u16,
        rank: u16,
    },
    Omitted {
        scenario_id: String,
    },
}

impl MechanismStressScenarioScore {
    fn scenario_id(&self) -> &str {
        match self {
            MechanismStressScenarioScore::Available { scenario_id, .. }
            | MechanismStressScenarioScore::Omitted { scenario_id } => scenario_id,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum MechanismRobustnessEvaluation {
    Measured {
        available_scenarios: usize,
        worst_score_milli: u16,
        best_score_milli: u16,
        weighted_score_milli: u16,
        rank_reversal_count: u16,
        stability_milli: u16,
    },
    Unmeasured {
        omitted_scenarios: usize,
    },
}

impl MechanismRobustnessEvaluation {
    fn available_scenarios(&self) -> usize {
        match self {
            MechanismRobustnessEvaluation::Measured {
                available_scenarios,
                ..
            } => *available_scenarios,
            MechanismRobustnessEvaluation::Unmeasured { .. } => 0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismRobustnessRecord {
    pub mechanism_id: String,
    pub baseline_score_milli: u16,
    pub baseline_rank: u16,
    /// Measured aggregates and the unmeasured case cannot be combined or imputed.
    pub evaluation: MechanismRobustnessEvaluation,
    pub scenario_order: Vec<String>,
    pub scenario_scores: Vec<MechanismStressScenarioScore>,
    pub negative_evidence_order: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MechanismRobustnessStressDisposition {
    Ready,
    Partial,
    Blocked,
}

fn record_is_invalid(
    record: &MechanismRobustnessRecord,
    scenario_order: &[String],
    mechanism_count: usize,
) -> bool {
    let available_scores = record
        .scenario_scores
        .iter()
        .filter(|score| matches!(score, MechanismStressScenarioScore::Available { .. }))
        .count();
    let scenarios_invalid = record.scenario_scores.len() != scenario_order.len()
        || record
            .scenario_scores
            .iter()
            .zip(scenario_order)
            .any(|(score, expected)| {
                score.scenario_id() != expected
                    || match score {
                        MechanismStressScenarioScore::Available {
                            score_milli, rank, ..
                        } => *score_milli > 1_000 || *rank == 0 || *rank as usize > mechanism_count,
                        MechanismStressScenarioScore::Omitted { .. } => false,
                    }
            });
    let evaluation_invalid = match &record.evaluation {
        MechanismRobustnessEvaluation::Measured {
            available_scenarios,
            worst_score_milli,
            best_score_milli,
            weighted_score_milli,
            rank_reversal_count,
            stability_milli,
        } => {
            if *available_scenarios == 0
                || *available_scenarios != available_scores
                || *available_scenarios > scenario_order.len()
                || *worst_score_milli > 1_000
                || *best_score_milli > 1_000
                || *weighted_score_milli > 1_000
                || *rank_reversal_count as usize > *available_scenarios
                || *stability_milli > 1_000
            {
                true
            } else {
                let expected = ((*available_scenarios as u32 - u32::from(*rank_reversal_count))
                    * 1_000)
                    / *available_scenarios as u32;
                *stability_milli != expected as u16
            }
        }
        MechanismRobustnessEvaluation::Unmeasured { omitted_scenarios } => {
            available_scores != 0 || *omitted_scenarios != scenario_order.len()
        }
    };
    record.mechanism_id.trim().is_empty()
        || record.baseline_score_milli > 1_000
        || record.baseline_rank == 0
        || record.baseline_rank as usize > mechanism_count
        || record.scenario_order != scenario_order
        || scenarios_invalid
        || evaluation_invalid
        || !canonical(&record.negative_evidence_order)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismRobustnessStress {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub scenario_order: Vec<String>,
    pub mechanism_order: Vec<String>,
    pub records: Vec<MechanismRobustnessRecord>,
    pub baseline_frontier_order: Vec<String>,
    pub fragile_order: Vec<String>,
    pub omitted_mechanism_order: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub disposition: MechanismRobustnessStressDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum MechanismRobustnessStressError {
    #[error("mechanism robustness stress request is invalid: {0}")]
    InvalidRequest(String),
    #[error("mechanism robustness stress scenario is invalid: {0}")]
    InvalidScenario(String),
    #[error("mechanism robustness stress output is invalid: {0}")]
    InvalidOutput(String),
    #[error("mechanism robustness stress digest failed: {0}")]
    Digest(String),
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn unique_nonempty(values: &[String]) -> bool {
    values.iter().all(|value| !value.trim().is_empty())
        && values.iter().collect::<BTreeSet<_>>().len() == values.len()
}

fn digest_input(output: &MechanismRobustnessStress) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "scenario_order": output.scenario_order,
        "mechanism_order": output.mechanism_order,
        "records": output.records,
        "baseline_frontier_order": output.baseline_frontier_order,
        "fragile_order": output.fragile_order,
        "omitted_mechanism_order": output.omitted_mechanism_order,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "disposition": output.disposition,
    })
}

fn clamp_score(value: i32) -> u16 {
    value.clamp(0, 1_000) as u16
}

fn ranking(scores: &BTreeMap<String, u16>) -> Vec<String> {
    let mut entries = scores.iter().collect::<Vec<_>>();
    entries.sort_by(|left, right| right.1.cmp(left.1).then_with(|| left.0.cmp(right.0)));
    entries.into_iter().map(|(id, _)| id.clone()).collect()
}

impl MechanismRobustnessStress {
    pub fn validate(&self) -> Result<(), MechanismRobustnessStressError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !unique_nonempty(&self.scenario_order)
            || self.scenario_order.len() < 2
            || !canonical(&self.mechanism_order)
            || self.records.len() != self.mechanism_order.len()
            || self
                .records
                .windows(2)
                .any(|pair| pair[0].mechanism_id >= pair[1].mechanism_id)
            || !unique_nonempty(&self.baseline_frontier_order)
            || !unique_nonempty(&self.fragile_order)
            || !canonical(&self.omitted_mechanism_order)
            || !canonical(&self.negative_evidence)
            || !canonical(&self.uncertainty)
            || self.records.iter().any(|record| {
                record_is_invalid(record, &self.scenario_order, self.mechanism_order.len())
            })
            || self.digest.as_str().len() != 64
        {
            return Err(MechanismRobustnessStressError::InvalidOutput(
                "identity, scenario alignment, ranking, score bounds, or digest shape is invalid"
                    .into(),
            ));
        }
        let ids = self
            .records
            .iter()
            .map(|record| record.mechanism_id.clone())
            .collect::<BTreeSet<_>>();
        if ids
            != self
                .mechanism_order
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>()
            || self
                .baseline_frontier_order
                .iter()
                .any(|id| !ids.contains(id))
            || self.fragile_order.iter().any(|id| !ids.contains(id))
            || self
                .omitted_mechanism_order
                .iter()
                .any(|id| !ids.contains(id))
        {
            return Err(MechanismRobustnessStressError::InvalidOutput(
                "mechanism partitions do not reconcile with records".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| MechanismRobustnessStressError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(MechanismRobustnessStressError::Digest(
                "mechanism robustness stress digest does not match canonical content".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &MechanismRobustnessStressRequest,
) -> Result<(), MechanismRobustnessStressError> {
    if request.objective.trim().is_empty()
        || request.candidates.is_empty()
        || request.candidates.len() > MAX_MECHANISMS
        || request.max_mechanisms == 0
        || request.max_mechanisms > MAX_MECHANISMS
        || request.candidates.len() > request.max_mechanisms
        || request.scenarios.len() < 2
        || request.scenarios.len() > MAX_SCENARIOS
        || request.max_scenarios < 2
        || request.max_scenarios > MAX_SCENARIOS
        || request.scenarios.len() > request.max_scenarios
        || request.min_stability_milli > 1_000
    {
        return Err(MechanismRobustnessStressError::InvalidRequest(
            "objective, bounded candidates, at least two bounded stress scenarios, and stability threshold are required".into(),
        ));
    }
    let mut candidate_ids = BTreeSet::new();
    for candidate in &request.candidates {
        if candidate.mechanism_id.trim().is_empty()
            || candidate.baseline_uncertainty_milli == 0
            || candidate.baseline_score_milli > 1_000
            || candidate.baseline_uncertainty_milli > 1_000
            || !candidate_ids.insert(candidate.mechanism_id.clone())
        {
            return Err(MechanismRobustnessStressError::InvalidRequest(
                "candidates require unique ids and bounded baseline scores/uncertainty".into(),
            ));
        }
    }
    let mut scenario_ids = BTreeSet::new();
    let mut total_weight = 0_u32;
    for scenario in &request.scenarios {
        if scenario.scenario_id.trim().is_empty()
            || !scenario_ids.insert(scenario.scenario_id.clone())
            || scenario.weight_milli == 0
            || scenario
                .adjustment_order
                .windows(2)
                .any(|pair| pair[0].mechanism_id >= pair[1].mechanism_id)
            || scenario
                .omitted_mechanism_order
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || scenario
                .omitted_mechanism_order
                .iter()
                .any(|id| !candidate_ids.contains(id))
        {
            return Err(MechanismRobustnessStressError::InvalidScenario(
                "scenarios require unique ids, positive weights, canonical adjustment/omission order, and known mechanisms".into(),
            ));
        }
        let mut adjustment_ids = BTreeSet::new();
        for adjustment in &scenario.adjustment_order {
            if !candidate_ids.contains(&adjustment.mechanism_id)
                || !adjustment_ids.insert(adjustment.mechanism_id.clone())
                || adjustment.delta_milli.unsigned_abs() > MAX_ABS_DELTA_MILLI as u32
            {
                return Err(MechanismRobustnessStressError::InvalidScenario(
                    "scenario adjustments require unique known mechanisms and bounded deltas"
                        .into(),
                ));
            }
        }
        total_weight = total_weight.saturating_add(u32::from(scenario.weight_milli));
    }
    if total_weight != 1_000 {
        return Err(MechanismRobustnessStressError::InvalidRequest(
            "stress scenario weights must sum to 1000 milli".into(),
        ));
    }
    Ok(())
}

/// Evaluate mechanism ranking stability under bounded perturbation and omission scenarios.
pub fn stress_glioma_mechanism_robustness(
    request: &MechanismRobustnessStressRequest,
) -> Result<MechanismRobustnessStress, MechanismRobustnessStressError> {
    validate_request(request)?;
    let candidate_map = request
        .candidates
        .iter()
        .map(|candidate| (candidate.mechanism_id.clone(), candidate))
        .collect::<BTreeMap<_, _>>();
    let baseline_scores = candidate_map
        .iter()
        .map(|(id, candidate)| (id.clone(), candidate.baseline_score_milli))
        .collect::<BTreeMap<_, _>>();
    let baseline_frontier_order = ranking(&baseline_scores);
    let baseline_rank_by_id = baseline_frontier_order
        .iter()
        .enumerate()
        .map(|(index, id)| (id.clone(), (index + 1) as u16))
        .collect::<BTreeMap<_, _>>();
    let scenario_order = request
        .scenarios
        .iter()
        .map(|scenario| scenario.scenario_id.clone())
        .collect::<Vec<_>>();
    let mut records = Vec::new();
    let mut fragile = BTreeSet::new();
    let mut omitted = BTreeSet::new();
    let mut negative = BTreeSet::new();
    let mut uncertainty = BTreeSet::new();
    for candidate in &request.candidates {
        let mut scenario_scores = Vec::new();
        let mut weighted_sum = 0_u64;
        let mut weight_total = 0_u64;
        let mut worst = 1_000_u16;
        let mut best = 0_u16;
        let mut reversals = 0_u16;
        let mut available_scenarios = 0_usize;
        for scenario in &request.scenarios {
            let available = !scenario
                .omitted_mechanism_order
                .binary_search(&candidate.mechanism_id)
                .is_ok();
            if !available {
                omitted.insert(candidate.mechanism_id.clone());
                uncertainty.insert(format!(
                    "mechanism:{}:omitted-in:{}",
                    candidate.mechanism_id, scenario.scenario_id
                ));
            }
            let mut scores = BTreeMap::new();
            for other in &request.candidates {
                if !scenario
                    .omitted_mechanism_order
                    .binary_search(&other.mechanism_id)
                    .is_ok()
                {
                    let delta = scenario
                        .adjustment_order
                        .iter()
                        .find(|adjustment| adjustment.mechanism_id == other.mechanism_id)
                        .map(|adjustment| adjustment.delta_milli)
                        .unwrap_or(0);
                    scores.insert(
                        other.mechanism_id.clone(),
                        clamp_score(i32::from(other.baseline_score_milli) + delta),
                    );
                }
            }
            let scenario_ranking = ranking(&scores);
            let rank = scenario_ranking
                .iter()
                .position(|id| id == &candidate.mechanism_id)
                .map(|index| (index + 1) as u16);
            if available {
                let Some(rank) = rank else {
                    return Err(MechanismRobustnessStressError::InvalidOutput(
                        "an available candidate is missing from its scenario ranking".into(),
                    ));
                };
                let delta = scenario
                    .adjustment_order
                    .iter()
                    .find(|adjustment| adjustment.mechanism_id == candidate.mechanism_id)
                    .map(|adjustment| adjustment.delta_milli)
                    .unwrap_or(0);
                let score = clamp_score(i32::from(candidate.baseline_score_milli) + delta);
                if rank > baseline_rank_by_id[&candidate.mechanism_id] {
                    reversals = reversals.saturating_add(1);
                }
                available_scenarios += 1;
                worst = worst.min(score);
                best = best.max(score);
                weighted_sum = weighted_sum.saturating_add(
                    u64::from(score).saturating_mul(u64::from(scenario.weight_milli)),
                );
                weight_total = weight_total.saturating_add(u64::from(scenario.weight_milli));
                scenario_scores.push(MechanismStressScenarioScore::Available {
                    scenario_id: scenario.scenario_id.clone(),
                    score_milli: score,
                    rank,
                });
            } else {
                scenario_scores.push(MechanismStressScenarioScore::Omitted {
                    scenario_id: scenario.scenario_id.clone(),
                });
            }
        }
        let (evaluation, stability) = if available_scenarios == 0 {
            (
                MechanismRobustnessEvaluation::Unmeasured {
                    omitted_scenarios: request.scenarios.len(),
                },
                None,
            )
        } else {
            let Some(weighted_score) = weighted_sum.checked_div(weight_total) else {
                return Err(MechanismRobustnessStressError::InvalidOutput(
                    "a measured mechanism has no positive scenario weight".into(),
                ));
            };
            let stability = ((available_scenarios as u32 - u32::from(reversals)) * 1_000)
                / available_scenarios as u32;
            (
                MechanismRobustnessEvaluation::Measured {
                    available_scenarios,
                    worst_score_milli: worst,
                    best_score_milli: best,
                    weighted_score_milli: weighted_score as u16,
                    rank_reversal_count: reversals,
                    stability_milli: stability as u16,
                },
                Some(stability as u16),
            )
        };
        let mut negative_order = Vec::new();
        if stability.is_some_and(|value| value < request.min_stability_milli) {
            let evidence = format!("mechanism:{}:rank-fragility", candidate.mechanism_id);
            negative.insert(evidence.clone());
            fragile.insert(candidate.mechanism_id.clone());
            negative_order.push(evidence);
        }
        records.push(MechanismRobustnessRecord {
            mechanism_id: candidate.mechanism_id.clone(),
            baseline_score_milli: candidate.baseline_score_milli,
            baseline_rank: baseline_rank_by_id[&candidate.mechanism_id],
            evaluation,
            scenario_order: scenario_order.clone(),
            scenario_scores,
            negative_evidence_order: negative_order,
        });
    }
    records.sort_by(|left, right| left.mechanism_id.cmp(&right.mechanism_id));
    let measured = records
        .iter()
        .filter(|record| record.evaluation.available_scenarios() > 0)
        .count();
    let disposition = if measured == 0 || fragile.len() == measured {
        MechanismRobustnessStressDisposition::Blocked
    } else if !fragile.is_empty() || !omitted.is_empty() || measured < records.len() {
        MechanismRobustnessStressDisposition::Partial
    } else {
        MechanismRobustnessStressDisposition::Ready
    };
    let mut output = MechanismRobustnessStress {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        scenario_order,
        mechanism_order: records
            .iter()
            .map(|record| record.mechanism_id.clone())
            .collect(),
        records,
        baseline_frontier_order,
        fragile_order: fragile.into_iter().collect(),
        omitted_mechanism_order: omitted.into_iter().collect(),
        negative_evidence: negative.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"placeholder"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| MechanismRobustnessStressError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> MechanismRobustnessStressRequest {
        MechanismRobustnessStressRequest {
            objective: "stress invasion mechanism ranking".into(),
            candidates: vec![
                MechanismStressCandidate {
                    mechanism_id: "near".into(),
                    baseline_score_milli: 800,
                    baseline_uncertainty_milli: 100,
                },
                MechanismStressCandidate {
                    mechanism_id: "far".into(),
                    baseline_score_milli: 700,
                    baseline_uncertainty_milli: 100,
                },
            ],
            scenarios: vec![
                MechanismStressScenario {
                    scenario_id: "nominal".into(),
                    weight_milli: 500,
                    adjustment_order: vec![],
                    omitted_mechanism_order: vec![],
                },
                MechanismStressScenario {
                    scenario_id: "adversarial".into(),
                    weight_milli: 500,
                    adjustment_order: vec![MechanismStressAdjustment {
                        mechanism_id: "near".into(),
                        delta_milli: -250,
                    }],
                    omitted_mechanism_order: vec![],
                },
            ],
            min_stability_milli: 750,
            max_mechanisms: 8,
            max_scenarios: 8,
        }
    }

    #[test]
    fn stress_surface_detects_rank_reversal_and_fragility() {
        let output = stress_glioma_mechanism_robustness(&request()).unwrap();
        assert_eq!(
            output.disposition,
            MechanismRobustnessStressDisposition::Partial
        );
        assert_eq!(output.fragile_order, vec!["near"]);
        assert_eq!(
            output
                .records
                .iter()
                .find(|record| record.mechanism_id == "near")
                .unwrap()
                .evaluation,
            MechanismRobustnessEvaluation::Measured {
                available_scenarios: 2,
                worst_score_milli: 550,
                best_score_milli: 800,
                weighted_score_milli: 675,
                rank_reversal_count: 1,
                stability_milli: 500,
            }
        );
        output.validate().unwrap();
    }

    #[test]
    fn omission_scenarios_are_excluded_from_the_measured_stability_denominator() {
        let mut request = request();
        request.scenarios[0].weight_milli = 300;
        request.scenarios[1].weight_milli = 300;
        request.scenarios.push(MechanismStressScenario {
            scenario_id: "drop-near".into(),
            weight_milli: 400,
            adjustment_order: vec![],
            omitted_mechanism_order: vec!["near".into()],
        });

        let output = stress_glioma_mechanism_robustness(&request).unwrap();
        let near = output
            .records
            .iter()
            .find(|record| record.mechanism_id == "near")
            .unwrap();
        assert!(matches!(
            &near.evaluation,
            MechanismRobustnessEvaluation::Measured {
                available_scenarios: 2,
                rank_reversal_count: 1,
                stability_milli: 500,
                worst_score_milli: 550,
                best_score_milli: 800,
                weighted_score_milli: 675,
            }
        ));
        assert!(matches!(
            &near.scenario_scores[2],
            MechanismStressScenarioScore::Omitted { .. }
        ));
        assert_eq!(
            output.disposition,
            MechanismRobustnessStressDisposition::Partial
        );
        output.validate().unwrap();
    }

    #[test]
    fn a_mechanism_omitted_from_every_scenario_has_no_fabricated_aggregate_scores() {
        let mut request = request();
        for scenario in &mut request.scenarios {
            scenario.omitted_mechanism_order = vec!["far".into(), "near".into()];
        }

        let output = stress_glioma_mechanism_robustness(&request).unwrap();
        assert_eq!(
            output.disposition,
            MechanismRobustnessStressDisposition::Blocked
        );
        for record in &output.records {
            assert!(matches!(
                &record.evaluation,
                MechanismRobustnessEvaluation::Unmeasured {
                    omitted_scenarios: 2
                }
            ));
            assert!(
                record
                    .scenario_scores
                    .iter()
                    .all(|score| matches!(score, MechanismStressScenarioScore::Omitted { .. }))
            );
        }
        let wire = serde_json::to_value(&output).unwrap();
        assert_eq!(wire["output_schema"], OUTPUT_SCHEMA);
        assert_eq!(wire["records"][0]["evaluation"]["status"], "unmeasured");
        assert!(
            wire["records"][0]["evaluation"]
                .get("worst_score_milli")
                .is_none()
        );
        assert_eq!(
            wire["records"][0]["scenario_scores"][0]["availability"],
            "omitted"
        );
        let round_trip: MechanismRobustnessStress = serde_json::from_value(wire).unwrap();
        round_trip.validate().unwrap();

        let mut inconsistent = output.clone();
        inconsistent.records[0].evaluation = MechanismRobustnessEvaluation::Unmeasured {
            omitted_scenarios: 1,
        };
        assert!(matches!(
            inconsistent.validate(),
            Err(MechanismRobustnessStressError::InvalidOutput(_))
        ));
        output.validate().unwrap();
    }

    #[test]
    fn stress_surface_requires_weighted_scenarios() {
        let mut invalid = request();
        invalid.scenarios[0].weight_milli = 400;
        let error = stress_glioma_mechanism_robustness(&invalid).unwrap_err();
        assert!(error.to_string().contains("sum to 1000"));
    }
}
