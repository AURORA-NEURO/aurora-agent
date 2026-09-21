//! Adversarial robustness stress surfaces for preclinical glioma mechanisms.
//!
//! Mechanism rankings are often sensitive to measurement error, missing model systems, and
//! plausible alternative interpretations.  This feature evaluates bounded stress scenarios and
//! makes rank reversals and brittle evidence visible before the next action is selected.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P05-F07";
pub const OUTPUT_SCHEMA: &str = "GliomaMechanismRobustnessStress1@1";
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
pub struct MechanismStressScenarioScore {
    pub scenario_id: String,
    pub score_milli: u16,
    pub rank: u16,
    pub available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MechanismRobustnessRecord {
    pub mechanism_id: String,
    pub baseline_score_milli: u16,
    pub worst_score_milli: u16,
    pub best_score_milli: u16,
    pub weighted_score_milli: u16,
    pub baseline_rank: u16,
    pub rank_reversal_count: u16,
    pub stability_milli: u16,
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
                record.mechanism_id.trim().is_empty()
                    || record.baseline_score_milli > 1_000
                    || record.worst_score_milli > 1_000
                    || record.best_score_milli > 1_000
                    || record.weighted_score_milli > 1_000
                    || record.baseline_rank == 0
                    || record.scenario_order != self.scenario_order
                    || record.scenario_scores.len() != self.scenario_order.len()
                    || !canonical(&record.negative_evidence_order)
                    || record.scenario_scores.iter().any(|score| {
                        score.rank == 0 || score.rank as usize > self.mechanism_order.len()
                    })
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
        for scenario in &request.scenarios {
            let available = !scenario
                .omitted_mechanism_order
                .binary_search(&candidate.mechanism_id)
                .is_ok();
            let score = if available {
                let delta = scenario
                    .adjustment_order
                    .iter()
                    .find(|adjustment| adjustment.mechanism_id == candidate.mechanism_id)
                    .map(|adjustment| adjustment.delta_milli)
                    .unwrap_or(0);
                clamp_score(i32::from(candidate.baseline_score_milli) + delta)
            } else {
                omitted.insert(candidate.mechanism_id.clone());
                uncertainty.insert(format!(
                    "mechanism:{}:omitted-in:{}",
                    candidate.mechanism_id, scenario.scenario_id
                ));
                0
            };
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
            let rank = ranking(&scores)
                .iter()
                .position(|id| id == &candidate.mechanism_id)
                .map(|index| (index + 1) as u16)
                .unwrap_or(candidate_map.len() as u16 + 1);
            if available && rank > baseline_rank_by_id[&candidate.mechanism_id] {
                reversals = reversals.saturating_add(1);
            }
            if available {
                worst = worst.min(score);
                best = best.max(score);
                weighted_sum = weighted_sum.saturating_add(
                    u64::from(score).saturating_mul(u64::from(scenario.weight_milli)),
                );
                weight_total = weight_total.saturating_add(u64::from(scenario.weight_milli));
            }
            scenario_scores.push(MechanismStressScenarioScore {
                scenario_id: scenario.scenario_id.clone(),
                score_milli: score,
                rank,
                available,
            });
        }
        let stability = ((u32::from(request.scenarios.len() as u16 - reversals) * 1_000)
            / request.scenarios.len() as u32) as u16;
        let weighted_score = if weight_total == 0 {
            0
        } else {
            (weighted_sum / weight_total) as u16
        };
        let mut negative_order = Vec::new();
        if stability < request.min_stability_milli {
            let evidence = format!("mechanism:{}:rank-fragility", candidate.mechanism_id);
            negative.insert(evidence.clone());
            fragile.insert(candidate.mechanism_id.clone());
            negative_order.push(evidence);
        }
        records.push(MechanismRobustnessRecord {
            mechanism_id: candidate.mechanism_id.clone(),
            baseline_score_milli: candidate.baseline_score_milli,
            worst_score_milli: worst,
            best_score_milli: best,
            weighted_score_milli: weighted_score,
            baseline_rank: baseline_rank_by_id[&candidate.mechanism_id],
            rank_reversal_count: reversals,
            stability_milli: stability,
            scenario_order: scenario_order.clone(),
            scenario_scores,
            negative_evidence_order: negative_order,
        });
    }
    records.sort_by(|left, right| left.mechanism_id.cmp(&right.mechanism_id));
    let disposition = if fragile.len() == records.len() {
        MechanismRobustnessStressDisposition::Blocked
    } else if !fragile.is_empty() || !omitted.is_empty() {
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
                .rank_reversal_count,
            1
        );
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
