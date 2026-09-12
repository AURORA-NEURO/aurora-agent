//! Stratified causal adjustment for preclinical glioma assays.
//!
//! This feature estimates a declared treatment contrast after adjustment for a caller-supplied
//! preclinical confounder stratum (for example, molecular subtype, batch, or organoid donor
//! class). It aggregates repeated observations to unit means, requires overlap in both arms,
//! weights eligible strata by their pooled unit population, and computes leave-one-stratum-out
//! influence bounds. All arithmetic is integer based and the result is a research interpretation,
//! never a clinical effect or treatment recommendation.

use crate::glioma_engine::{GliomaModelSystem, LocalArtifactRef};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P10-F11";
pub const OUTPUT_SCHEMA: &str = "GliomaStratifiedCausalAdjustment1@1";
pub const MAX_OBSERVATIONS: usize = 65_536;
pub const MAX_STRATA: usize = 4_096;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StratifiedCausalRequest {
    pub objective: String,
    pub control_arm: String,
    pub treatment_arm: String,
    pub model_system: GliomaModelSystem,
    pub min_units_per_arm_per_stratum: usize,
    pub min_eligible_strata: usize,
    pub effect_threshold_milli: u64,
    pub max_stratum_imbalance_milli: u16,
    pub max_leave_one_stratum_shift_milli: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StratifiedObservation {
    pub observation_id: String,
    pub unit_id: String,
    pub stratum_id: String,
    pub arm_id: String,
    pub model_system: GliomaModelSystem,
    pub batch_id: String,
    pub outcome_milli: i64,
    pub artifact: LocalArtifactRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CausalStratumSummary {
    pub stratum_id: String,
    pub control_unit_order: Vec<String>,
    pub treatment_unit_order: Vec<String>,
    pub control_mean_milli: Option<i64>,
    pub treatment_mean_milli: Option<i64>,
    pub effect_milli: Option<i64>,
    pub imbalance_milli: Option<u16>,
    pub pooled_units: usize,
    pub eligible: bool,
    pub exclusion_reason: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StratifiedCausalActionKind {
    ReleaseQualifiedContrast,
    ReplicateUnbalancedStrata,
    AddMissingStratumCoverage,
    PublishNegativeContrast,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StratifiedCausalDisposition {
    Qualified,
    Negative,
    Unresolved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StratifiedCausalAdjustment {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub control_arm: String,
    pub treatment_arm: String,
    pub model_system: GliomaModelSystem,
    pub stratum_order: Vec<String>,
    pub eligible_stratum_order: Vec<String>,
    pub excluded_stratum_order: Vec<String>,
    pub summaries: Vec<CausalStratumSummary>,
    pub eligible_unit_count: usize,
    pub adjusted_effect_milli: i64,
    pub interval_low_milli: i64,
    pub interval_high_milli: i64,
    pub uncertainty_milli: u64,
    pub max_stratum_imbalance_milli: u16,
    pub stratum_effect_range_milli: u64,
    pub negative_evidence: Vec<String>,
    pub uncertainty: Vec<String>,
    pub next_action: StratifiedCausalActionKind,
    pub disposition: StratifiedCausalDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum StratifiedCausalError {
    #[error("stratified causal request is invalid: {0}")]
    InvalidRequest(String),
    #[error("stratified causal observation is invalid: {0}")]
    InvalidObservation(String),
    #[error("stratified causal output is invalid: {0}")]
    InvalidOutput(String),
    #[error("stratified causal digest failed: {0}")]
    Digest(String),
}

fn ordered_unique(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn mean(values: &[i64]) -> i64 {
    let mean = values.iter().map(|value| i128::from(*value)).sum::<i128>() / values.len() as i128;
    mean.clamp(i128::from(i64::MIN), i128::from(i64::MAX)) as i64
}

fn imbalance(control_count: usize, treatment_count: usize) -> u16 {
    let difference = control_count.abs_diff(treatment_count) as i128;
    let pooled = control_count.saturating_add(treatment_count).max(1) as i128;
    (difference * 1_000 / pooled).clamp(0, 1_000) as u16
}

fn digest_input(output: &StratifiedCausalAdjustment) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "control_arm": output.control_arm,
        "treatment_arm": output.treatment_arm,
        "model_system": output.model_system,
        "stratum_order": output.stratum_order,
        "eligible_stratum_order": output.eligible_stratum_order,
        "excluded_stratum_order": output.excluded_stratum_order,
        "summaries": output.summaries,
        "eligible_unit_count": output.eligible_unit_count,
        "adjusted_effect_milli": output.adjusted_effect_milli,
        "interval_low_milli": output.interval_low_milli,
        "interval_high_milli": output.interval_high_milli,
        "uncertainty_milli": output.uncertainty_milli,
        "max_stratum_imbalance_milli": output.max_stratum_imbalance_milli,
        "stratum_effect_range_milli": output.stratum_effect_range_milli,
        "negative_evidence": output.negative_evidence,
        "uncertainty": output.uncertainty,
        "next_action": output.next_action,
        "disposition": output.disposition,
    })
}

impl StratifiedCausalAdjustment {
    pub fn validate(&self) -> Result<(), StratifiedCausalError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || self.control_arm.trim().is_empty()
            || self.treatment_arm.trim().is_empty()
            || self.control_arm == self.treatment_arm
            || !ordered_unique(&self.stratum_order)
            || !ordered_unique(&self.eligible_stratum_order)
            || !ordered_unique(&self.excluded_stratum_order)
            || !ordered_unique(&self.negative_evidence)
            || !ordered_unique(&self.uncertainty)
            || self.interval_low_milli > self.interval_high_milli
            || self.max_stratum_imbalance_milli > 1_000
            || self
                .summaries
                .windows(2)
                .any(|pair| pair[0].stratum_id >= pair[1].stratum_id)
            || self.summaries.iter().any(|summary| {
                summary.stratum_id.trim().is_empty()
                    || !ordered_unique(&summary.control_unit_order)
                    || !ordered_unique(&summary.treatment_unit_order)
                    || summary.control_mean_milli.is_none() != summary.control_unit_order.is_empty()
                    || summary.treatment_mean_milli.is_none()
                        != summary.treatment_unit_order.is_empty()
                    || summary.imbalance_milli.is_some_and(|value| value > 1_000)
                    || (summary.eligible && summary.effect_milli.is_none())
                    || (!summary.eligible && summary.exclusion_reason.is_none())
            })
        {
            return Err(StratifiedCausalError::InvalidOutput(
                "identity, ordering, arm partition, interval, or summary bounds are invalid".into(),
            ));
        }
        let ids = self
            .summaries
            .iter()
            .map(|summary| summary.stratum_id.clone())
            .collect::<BTreeSet<_>>();
        let eligible = self
            .summaries
            .iter()
            .filter(|summary| summary.eligible)
            .map(|summary| summary.stratum_id.clone())
            .collect::<Vec<_>>();
        let excluded = self
            .summaries
            .iter()
            .filter(|summary| !summary.eligible)
            .map(|summary| summary.stratum_id.clone())
            .collect::<Vec<_>>();
        if ids.len() != self.summaries.len()
            || self.stratum_order != ids.iter().cloned().collect::<Vec<_>>()
            || self.eligible_stratum_order != eligible
            || self.excluded_stratum_order != excluded
            || self.eligible_unit_count
                != self
                    .summaries
                    .iter()
                    .filter(|summary| summary.eligible)
                    .map(|summary| summary.pooled_units)
                    .sum::<usize>()
        {
            return Err(StratifiedCausalError::InvalidOutput(
                "stratum partitions or eligible unit count do not reconcile".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| StratifiedCausalError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(StratifiedCausalError::InvalidOutput(
                "digest is not bound to stratified causal adjustment".into(),
            ));
        }
        Ok(())
    }
}

pub fn analyze_stratified_causal_adjustment(
    request: &StratifiedCausalRequest,
    observations: &[StratifiedObservation],
) -> Result<StratifiedCausalAdjustment, StratifiedCausalError> {
    if request.objective.trim().is_empty()
        || request.control_arm.trim().is_empty()
        || request.treatment_arm.trim().is_empty()
        || request.control_arm == request.treatment_arm
        || request.min_units_per_arm_per_stratum == 0
        || request.min_eligible_strata == 0
        || request.effect_threshold_milli > i64::MAX as u64
        || request.max_stratum_imbalance_milli > 1_000
        || request.max_leave_one_stratum_shift_milli > i64::MAX as u64
        || observations.is_empty()
        || observations.len() > MAX_OBSERVATIONS
    {
        return Err(StratifiedCausalError::InvalidRequest(
            "objective, distinct arms, positive floors, overlap bounds, and bounded observations are required".into(),
        ));
    }
    let mut observation_ids = BTreeSet::new();
    let mut unit_strata = BTreeMap::<String, String>::new();
    let mut unit_arms = BTreeMap::<String, String>::new();
    let mut grouped = BTreeMap::<String, BTreeMap<String, BTreeMap<String, Vec<i64>>>>::new();
    for observation in observations {
        if observation.observation_id.trim().is_empty()
            || observation.unit_id.trim().is_empty()
            || observation.stratum_id.trim().is_empty()
            || observation.arm_id != request.control_arm
                && observation.arm_id != request.treatment_arm
            || observation.model_system != request.model_system
            || observation.batch_id.trim().is_empty()
            || observation.artifact.validate().is_err()
            || !observation.artifact.local_only
            || observation.artifact.contains_human_data
            || observation.artifact.contains_direct_identifiers
            || !observation_ids.insert(observation.observation_id.clone())
        {
            return Err(StratifiedCausalError::InvalidObservation(
                "observation identity, arm/model binding, batch, local artifact, privacy, or uniqueness is invalid".into(),
            ));
        }
        if let Some(previous_stratum) = unit_strata.get(&observation.unit_id) {
            if previous_stratum != &observation.stratum_id {
                return Err(StratifiedCausalError::InvalidObservation(
                    "a unit cannot move between confounder strata".into(),
                ));
            }
        } else {
            unit_strata.insert(observation.unit_id.clone(), observation.stratum_id.clone());
        }
        if let Some(previous_arm) = unit_arms.get(&observation.unit_id) {
            if previous_arm != &observation.arm_id {
                return Err(StratifiedCausalError::InvalidObservation(
                    "a unit cannot contribute observations to both causal arms".into(),
                ));
            }
        } else {
            unit_arms.insert(observation.unit_id.clone(), observation.arm_id.clone());
        }
        grouped
            .entry(observation.stratum_id.clone())
            .or_default()
            .entry(observation.arm_id.clone())
            .or_default()
            .entry(observation.unit_id.clone())
            .or_default()
            .push(observation.outcome_milli);
    }
    if grouped.len() > MAX_STRATA {
        return Err(StratifiedCausalError::InvalidObservation(
            "stratum bound exceeded".into(),
        ));
    }
    let mut summaries = Vec::with_capacity(grouped.len());
    for (stratum_id, arms) in grouped {
        let control_units = arms
            .get(&request.control_arm)
            .into_iter()
            .flat_map(|units| units.iter())
            .map(|(unit_id, values)| (unit_id.clone(), mean(values)))
            .collect::<BTreeMap<_, _>>();
        let treatment_units = arms
            .get(&request.treatment_arm)
            .into_iter()
            .flat_map(|units| units.iter())
            .map(|(unit_id, values)| (unit_id.clone(), mean(values)))
            .collect::<BTreeMap<_, _>>();
        let control_order = control_units.keys().cloned().collect::<Vec<_>>();
        let treatment_order = treatment_units.keys().cloned().collect::<Vec<_>>();
        let control_mean = (!control_units.is_empty())
            .then(|| mean(&control_units.values().copied().collect::<Vec<_>>()));
        let treatment_mean = (!treatment_units.is_empty())
            .then(|| mean(&treatment_units.values().copied().collect::<Vec<_>>()));
        let eligible = control_units.len() >= request.min_units_per_arm_per_stratum
            && treatment_units.len() >= request.min_units_per_arm_per_stratum;
        let effect = eligible.then(|| treatment_mean.unwrap() - control_mean.unwrap());
        let pooled_units = control_units.len().saturating_add(treatment_units.len());
        let reason = if eligible {
            None
        } else if control_units.is_empty() || treatment_units.is_empty() {
            Some("positivity-missing-control-or-treatment".into())
        } else {
            Some("stratum-replicate-floor-not-met".into())
        };
        summaries.push(CausalStratumSummary {
            stratum_id,
            control_unit_order: control_order,
            treatment_unit_order: treatment_order,
            control_mean_milli: control_mean,
            treatment_mean_milli: treatment_mean,
            effect_milli: effect,
            imbalance_milli: (eligible)
                .then(|| imbalance(control_units.len(), treatment_units.len())),
            pooled_units,
            eligible,
            exclusion_reason: reason,
        });
    }
    summaries.sort_by(|left, right| left.stratum_id.cmp(&right.stratum_id));
    let eligible_summary = summaries
        .iter()
        .filter(|summary| summary.eligible)
        .collect::<Vec<_>>();
    let eligible_stratum_order = eligible_summary
        .iter()
        .map(|summary| summary.stratum_id.clone())
        .collect::<Vec<_>>();
    let excluded_stratum_order = summaries
        .iter()
        .filter(|summary| !summary.eligible)
        .map(|summary| summary.stratum_id.clone())
        .collect::<Vec<_>>();
    let eligible_unit_count = eligible_summary
        .iter()
        .map(|summary| summary.pooled_units)
        .sum::<usize>();
    let adjusted_effect = if eligible_summary.is_empty() {
        0
    } else {
        let numerator = eligible_summary
            .iter()
            .map(|summary| i128::from(summary.effect_milli.unwrap()) * summary.pooled_units as i128)
            .sum::<i128>();
        (numerator / eligible_unit_count.max(1) as i128) as i64
    };
    let mut leave_one_out = Vec::new();
    if eligible_summary.len() > 1 {
        for omitted in &eligible_summary {
            let included = eligible_summary
                .iter()
                .filter(|summary| summary.stratum_id != omitted.stratum_id)
                .collect::<Vec<_>>();
            let total_weight = included
                .iter()
                .map(|summary| summary.pooled_units)
                .sum::<usize>();
            let numerator = included
                .iter()
                .map(|summary| {
                    i128::from(summary.effect_milli.unwrap()) * summary.pooled_units as i128
                })
                .sum::<i128>();
            leave_one_out.push((numerator / total_weight.max(1) as i128) as i64);
        }
    } else {
        leave_one_out.push(adjusted_effect);
    }
    let interval_low = *leave_one_out.iter().min().unwrap_or(&adjusted_effect);
    let interval_high = *leave_one_out.iter().max().unwrap_or(&adjusted_effect);
    let uncertainty_milli = (adjusted_effect - interval_low)
        .unsigned_abs()
        .max((interval_high - adjusted_effect).unsigned_abs());
    let max_imbalance = eligible_summary
        .iter()
        .filter_map(|summary| summary.imbalance_milli)
        .max()
        .unwrap_or(1_000);
    let effects = eligible_summary
        .iter()
        .filter_map(|summary| summary.effect_milli)
        .collect::<Vec<_>>();
    let range = effects
        .iter()
        .min()
        .zip(effects.iter().max())
        .map(|(low, high)| (*high - *low).unsigned_abs())
        .unwrap_or(0);
    let mut negative_evidence = BTreeSet::new();
    if !eligible_summary.is_empty()
        && adjusted_effect.unsigned_abs() < request.effect_threshold_milli
    {
        negative_evidence.insert("adjusted-effect-below-declared-threshold".into());
    }
    if max_imbalance > request.max_stratum_imbalance_milli {
        negative_evidence.insert("stratum-overlap-exceeds-declared-bound".into());
    }
    let mut uncertainty = BTreeSet::new();
    if eligible_summary.len() < request.min_eligible_strata {
        uncertainty.insert("eligible-stratum-floor-not-met".into());
    }
    if !excluded_stratum_order.is_empty() {
        uncertainty.insert("excluded-strata-retained-for-coverage-audit".into());
    }
    if uncertainty_milli > request.max_leave_one_stratum_shift_milli {
        uncertainty.insert("leave-one-stratum-influence-exceeds-bound".into());
    }
    let qualified = eligible_summary.len() >= request.min_eligible_strata
        && adjusted_effect.unsigned_abs() >= request.effect_threshold_milli
        && max_imbalance <= request.max_stratum_imbalance_milli
        && uncertainty_milli <= request.max_leave_one_stratum_shift_milli;
    let disposition = if eligible_summary.len() < request.min_eligible_strata {
        StratifiedCausalDisposition::Unresolved
    } else if qualified {
        StratifiedCausalDisposition::Qualified
    } else {
        StratifiedCausalDisposition::Negative
    };
    let next_action = if eligible_summary.len() < request.min_eligible_strata {
        StratifiedCausalActionKind::AddMissingStratumCoverage
    } else if max_imbalance > request.max_stratum_imbalance_milli
        || uncertainty_milli > request.max_leave_one_stratum_shift_milli
    {
        StratifiedCausalActionKind::ReplicateUnbalancedStrata
    } else if qualified {
        StratifiedCausalActionKind::ReleaseQualifiedContrast
    } else {
        StratifiedCausalActionKind::PublishNegativeContrast
    };
    let mut output = StratifiedCausalAdjustment {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        control_arm: request.control_arm.clone(),
        treatment_arm: request.treatment_arm.clone(),
        model_system: request.model_system,
        stratum_order: summaries
            .iter()
            .map(|summary| summary.stratum_id.clone())
            .collect(),
        eligible_stratum_order,
        excluded_stratum_order,
        summaries,
        eligible_unit_count,
        adjusted_effect_milli: adjusted_effect,
        interval_low_milli: interval_low,
        interval_high_milli: interval_high,
        uncertainty_milli,
        max_stratum_imbalance_milli: max_imbalance,
        stratum_effect_range_milli: range,
        negative_evidence: negative_evidence.into_iter().collect(),
        uncertainty: uncertainty.into_iter().collect(),
        next_action,
        disposition,
        digest: ContentHash::of_value(&serde_json::json!({}))
            .map_err(|error| StratifiedCausalError::Digest(error.to_string()))?,
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| StratifiedCausalError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn artifact(id: &str) -> LocalArtifactRef {
        LocalArtifactRef {
            artifact_id: format!("artifact-{id}"),
            content_hash: ContentHash::of_value(&serde_json::json!({"id": id})).unwrap(),
            content_type: "application/vnd.aurora.glioma-stratified-observation+json".into(),
            local_only: true,
            contains_human_data: false,
            contains_direct_identifiers: false,
        }
    }

    fn request() -> StratifiedCausalRequest {
        StratifiedCausalRequest {
            objective: "adjust invasion effect by molecular stratum".into(),
            control_arm: "control".into(),
            treatment_arm: "treated".into(),
            model_system: GliomaModelSystem::Organoid,
            min_units_per_arm_per_stratum: 2,
            min_eligible_strata: 2,
            effect_threshold_milli: 100,
            max_stratum_imbalance_milli: 400,
            max_leave_one_stratum_shift_milli: 80,
        }
    }

    fn observation(
        id: &str,
        unit: &str,
        stratum: &str,
        arm: &str,
        outcome: i64,
    ) -> StratifiedObservation {
        StratifiedObservation {
            observation_id: id.into(),
            unit_id: unit.into(),
            stratum_id: stratum.into(),
            arm_id: arm.into(),
            model_system: GliomaModelSystem::Organoid,
            batch_id: format!("batch-{id}"),
            outcome_milli: outcome,
            artifact: artifact(id),
        }
    }

    #[test]
    fn balanced_strata_produce_adjusted_qualified_effect() {
        let observations = vec![
            observation("a-c1", "c1", "low", "control", 100),
            observation("a-c2", "c2", "low", "control", 110),
            observation("a-t1", "t1", "low", "treated", 260),
            observation("a-t2", "t2", "low", "treated", 270),
            observation("b-c1", "c3", "high", "control", 200),
            observation("b-c2", "c4", "high", "control", 210),
            observation("b-t1", "t3", "high", "treated", 340),
            observation("b-t2", "t4", "high", "treated", 350),
        ];
        let output = analyze_stratified_causal_adjustment(&request(), &observations).unwrap();
        assert_eq!(output.disposition, StratifiedCausalDisposition::Qualified);
        assert_eq!(output.adjusted_effect_milli, 150);
        assert_eq!(output.max_stratum_imbalance_milli, 0);
        assert_eq!(
            output.next_action,
            StratifiedCausalActionKind::ReleaseQualifiedContrast
        );
        output.validate().unwrap();
    }

    #[test]
    fn missing_overlap_is_unresolved_and_preserved() {
        let mut request = request();
        request.min_eligible_strata = 2;
        let observations = vec![
            observation("a-c1", "c1", "low", "control", 100),
            observation("a-c2", "c2", "low", "control", 110),
            observation("a-t1", "t1", "low", "treated", 260),
            observation("a-t2", "t2", "low", "treated", 270),
            observation("b-c1", "c3", "high", "control", 200),
            observation("b-c2", "c4", "high", "control", 210),
        ];
        let output = analyze_stratified_causal_adjustment(&request, &observations).unwrap();
        assert_eq!(output.disposition, StratifiedCausalDisposition::Unresolved);
        assert_eq!(
            output.next_action,
            StratifiedCausalActionKind::AddMissingStratumCoverage
        );
        assert!(output.excluded_stratum_order.contains(&"high".to_string()));
        assert!(output
            .uncertainty
            .contains(&"eligible-stratum-floor-not-met".to_string()));
    }

    #[test]
    fn repeated_unit_measurements_are_collapsed_and_replay_stable() {
        let mut observations = vec![
            observation("a-c1-1", "c1", "low", "control", 100),
            observation("a-c1-2", "c1", "low", "control", 120),
            observation("a-c2", "c2", "low", "control", 110),
            observation("a-t1", "t1", "low", "treated", 260),
            observation("a-t2", "t2", "low", "treated", 270),
            observation("b-c1", "c3", "high", "control", 200),
            observation("b-c2", "c4", "high", "control", 210),
            observation("b-t1", "t3", "high", "treated", 340),
            observation("b-t2", "t4", "high", "treated", 350),
        ];
        observations.reverse();
        let first = analyze_stratified_causal_adjustment(&request(), &observations).unwrap();
        let second = analyze_stratified_causal_adjustment(&request(), &observations).unwrap();
        assert_eq!(first, second);
        let low = first
            .summaries
            .iter()
            .find(|summary| summary.stratum_id == "low")
            .unwrap();
        assert_eq!(low.control_unit_order.len(), 2);
        assert_eq!(low.control_mean_milli, Some(110));
    }

    #[test]
    fn unit_cannot_cross_causal_arms() {
        let observations = vec![
            observation("a-c1", "shared", "low", "control", 100),
            observation("a-t1", "shared", "low", "treated", 260),
        ];
        let error = analyze_stratified_causal_adjustment(&request(), &observations).unwrap_err();
        assert!(error.to_string().contains("both causal arms"));
    }
}
