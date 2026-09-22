//! Deterministic multi-factor contrast-panel design for preclinical glioma studies.
//!
//! This feature turns declared perturbation factors into a balanced factorial panel with named
//! estimands and interaction-coverage diagnostics. It is a design compiler, not a power oracle:
//! formal variance, assay-specific noise, and site-level calibration must still be supplied by a
//! downstream analysis and local validation workflow.

use crate::glioma_engine::{GliomaModality, GliomaModelSystem};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P06-F24";
pub const OUTPUT_SCHEMA: &str = "GliomaContrastDesign1@1";
pub const MAX_FACTORS: usize = 8;
pub const MAX_LEVELS_PER_FACTOR: usize = 8;
pub const MAX_CONDITIONS: usize = 4_096;
pub const MAX_INTERACTIONS: usize = 128;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContrastFactor {
    pub factor_id: String,
    pub label: String,
    pub level_order: Vec<String>,
    pub baseline_level: String,
    pub perturbation_kind: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContrastDesignRequest {
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub modality: GliomaModality,
    pub factors: Vec<ContrastFactor>,
    pub replicates_per_condition: u16,
    pub max_conditions: usize,
    pub max_total_units: u64,
    pub min_design_adequacy_milli: u16,
    pub required_interaction_order: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContrastCondition {
    pub condition_id: String,
    pub assignments: BTreeMap<String, String>,
    pub replicate_count: u16,
    pub is_control: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EstimandContrast {
    pub contrast_id: String,
    pub factor_id: String,
    pub level: String,
    pub positive_condition_order: Vec<String>,
    pub negative_condition_order: Vec<String>,
    pub estimand: String,
    pub balance_milli: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContrastDesignDisposition {
    Qualified,
    Partial,
    Underpowered,
    BudgetBlocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContrastDesign {
    pub feature_id: String,
    pub output_schema: String,
    pub objective: String,
    pub model_system: GliomaModelSystem,
    pub modality: GliomaModality,
    pub factor_order: Vec<String>,
    pub condition_order: Vec<String>,
    pub conditions: Vec<ContrastCondition>,
    pub contrast_order: Vec<String>,
    pub contrasts: Vec<EstimandContrast>,
    pub required_interaction_order: Vec<String>,
    pub covered_interaction_order: Vec<String>,
    pub missing_interaction_order: Vec<String>,
    pub design_adequacy_milli: u16,
    pub total_units: u64,
    pub limitations: Vec<String>,
    pub negative_evidence: Vec<String>,
    pub disposition: ContrastDesignDisposition,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ContrastDesignError {
    #[error("contrast design request is invalid: {0}")]
    InvalidRequest(String),
    #[error("contrast design input is invalid: {0}")]
    InvalidInput(String),
    #[error("contrast design output is invalid: {0}")]
    InvalidOutput(String),
    #[error("contrast design digest failed: {0}")]
    Digest(String),
}

fn canonical(values: &[String]) -> bool {
    values.windows(2).all(|pair| pair[0] < pair[1])
}

fn digest_input(output: &ContrastDesign) -> serde_json::Value {
    serde_json::json!({
        "feature_id": output.feature_id,
        "output_schema": output.output_schema,
        "objective": output.objective,
        "model_system": output.model_system,
        "modality": output.modality,
        "factor_order": output.factor_order,
        "condition_order": output.condition_order,
        "conditions": output.conditions,
        "contrast_order": output.contrast_order,
        "contrasts": output.contrasts,
        "required_interaction_order": output.required_interaction_order,
        "covered_interaction_order": output.covered_interaction_order,
        "missing_interaction_order": output.missing_interaction_order,
        "design_adequacy_milli": output.design_adequacy_milli,
        "total_units": output.total_units,
        "limitations": output.limitations,
        "negative_evidence": output.negative_evidence,
        "disposition": output.disposition,
    })
}

fn validate_request(request: &ContrastDesignRequest) -> Result<(), ContrastDesignError> {
    if request.objective.trim().is_empty()
        || request.factors.is_empty()
        || request.factors.len() > MAX_FACTORS
        || request.replicates_per_condition == 0
        || request.max_conditions == 0
        || request.max_conditions > MAX_CONDITIONS
        || request.max_total_units == 0
        || request.min_design_adequacy_milli > 1_000
        || request.required_interaction_order.len() > MAX_INTERACTIONS
        || !canonical(&request.required_interaction_order)
    {
        return Err(ContrastDesignError::InvalidRequest(
            "objective, factor, replicate, condition, budget, adequacy, or interaction bounds are invalid".into(),
        ));
    }
    Ok(())
}

fn validate_factors(factors: &[ContrastFactor]) -> Result<Vec<String>, ContrastDesignError> {
    let mut ids = BTreeSet::new();
    let mut order = Vec::with_capacity(factors.len());
    for factor in factors {
        if factor.factor_id.trim().is_empty()
            || factor.label.trim().is_empty()
            || factor.perturbation_kind.trim().is_empty()
            || factor.level_order.len() < 2
            || factor.level_order.len() > MAX_LEVELS_PER_FACTOR
            || !canonical(&factor.level_order)
            || factor
                .level_order
                .iter()
                .any(|level| level.trim().is_empty())
            || !factor.level_order.contains(&factor.baseline_level)
            || !ids.insert(factor.factor_id.clone())
        {
            return Err(ContrastDesignError::InvalidInput(
                "factors require unique ids, canonical levels, and a declared baseline".into(),
            ));
        }
        order.push(factor.factor_id.clone());
    }
    order.sort();
    Ok(order)
}

fn build_assignments(
    factors: &[ContrastFactor],
    index: usize,
    current: &mut BTreeMap<String, String>,
    output: &mut Vec<BTreeMap<String, String>>,
) {
    if index == factors.len() {
        output.push(current.clone());
        return;
    }
    let factor = &factors[index];
    for level in &factor.level_order {
        current.insert(factor.factor_id.clone(), level.clone());
        build_assignments(factors, index + 1, current, output);
    }
    current.remove(&factor.factor_id);
}

fn interaction_key(left: &str, right: &str) -> String {
    if left < right {
        format!("{left}×{right}")
    } else {
        format!("{right}×{left}")
    }
}

impl ContrastDesign {
    pub fn validate(&self) -> Result<(), ContrastDesignError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.objective.trim().is_empty()
            || !canonical(&self.factor_order)
            || !canonical(&self.condition_order)
            || !canonical(&self.contrast_order)
            || !canonical(&self.required_interaction_order)
            || !canonical(&self.covered_interaction_order)
            || !canonical(&self.missing_interaction_order)
            || !canonical(&self.limitations)
            || !canonical(&self.negative_evidence)
            || self.design_adequacy_milli > 1_000
            || self.conditions.len() != self.condition_order.len()
            || self.contrasts.len() != self.contrast_order.len()
            || self.conditions.iter().any(|condition| {
                condition.condition_id.trim().is_empty()
                    || condition.assignments.len() != self.factor_order.len()
                    || condition.replicate_count == 0
                    || condition
                        .assignments
                        .keys()
                        .any(|factor| !self.factor_order.contains(factor))
            })
            || self.contrasts.iter().any(|contrast| {
                contrast.contrast_id.trim().is_empty()
                    || contrast.factor_id.trim().is_empty()
                    || contrast.level.trim().is_empty()
                    || contrast.positive_condition_order.is_empty()
                    || contrast.negative_condition_order.is_empty()
                    || !canonical(&contrast.positive_condition_order)
                    || !canonical(&contrast.negative_condition_order)
                    || contrast.balance_milli > 1_000
                    || contrast.estimand.trim().is_empty()
            })
        {
            return Err(ContrastDesignError::InvalidOutput(
                "identity, ordering, condition coverage, or contrast bounds are invalid".into(),
            ));
        }
        let condition_ids = self
            .condition_order
            .iter()
            .cloned()
            .collect::<BTreeSet<_>>();
        let output_condition_ids = self
            .conditions
            .iter()
            .map(|condition| condition.condition_id.clone())
            .collect::<BTreeSet<_>>();
        let contrast_ids = self.contrast_order.iter().cloned().collect::<BTreeSet<_>>();
        let output_contrast_ids = self
            .contrasts
            .iter()
            .map(|contrast| contrast.contrast_id.clone())
            .collect::<BTreeSet<_>>();
        if condition_ids != output_condition_ids
            || contrast_ids != output_contrast_ids
            || self.contrasts.iter().any(|contrast| {
                contrast
                    .positive_condition_order
                    .iter()
                    .chain(contrast.negative_condition_order.iter())
                    .any(|condition| !condition_ids.contains(condition))
            })
        {
            return Err(ContrastDesignError::InvalidOutput(
                "condition or contrast references are not closed".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| ContrastDesignError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(ContrastDesignError::InvalidOutput(
                "contrast design digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

/// Compile a balanced factorial panel and named main-effect estimands from declared factors.
/// Budget and interaction gaps are returned as product states instead of being silently repaired.
pub fn design_glioma_contrast_panel(
    request: &ContrastDesignRequest,
) -> Result<ContrastDesign, ContrastDesignError> {
    validate_request(request)?;
    let factor_order = validate_factors(&request.factors)?;
    let factors = request.factors.to_vec();
    let mut assignments = Vec::new();
    build_assignments(&factors, 0, &mut BTreeMap::new(), &mut assignments);
    if assignments.len() > request.max_conditions {
        return Err(ContrastDesignError::InvalidInput(
            "factorial expansion exceeds the configured condition bound".into(),
        ));
    }
    let total_units = assignments.len() as u64 * u64::from(request.replicates_per_condition);
    let mut conditions = Vec::with_capacity(assignments.len());
    for (index, assignment) in assignments.into_iter().enumerate() {
        let is_control = request.factors.iter().all(|factor| {
            assignment
                .get(&factor.factor_id)
                .map(|level| level == &factor.baseline_level)
                .unwrap_or(false)
        });
        conditions.push(ContrastCondition {
            condition_id: format!("condition:{index:04}"),
            assignments: assignment,
            replicate_count: request.replicates_per_condition,
            is_control,
        });
    }
    let condition_order = conditions
        .iter()
        .map(|condition| condition.condition_id.clone())
        .collect::<Vec<_>>();
    let mut contrasts = Vec::new();
    for factor in &request.factors {
        for level in factor
            .level_order
            .iter()
            .filter(|level| *level != &factor.baseline_level)
        {
            let positive_condition_order = conditions
                .iter()
                .filter(|condition| condition.assignments.get(&factor.factor_id) == Some(level))
                .map(|condition| condition.condition_id.clone())
                .collect::<Vec<_>>();
            let negative_condition_order = conditions
                .iter()
                .filter(|condition| {
                    condition.assignments.get(&factor.factor_id) == Some(&factor.baseline_level)
                })
                .map(|condition| condition.condition_id.clone())
                .collect::<Vec<_>>();
            let balance_milli = if positive_condition_order.len() == negative_condition_order.len()
            {
                1_000
            } else {
                ((positive_condition_order
                    .len()
                    .min(negative_condition_order.len())
                    * 1_000)
                    / positive_condition_order
                        .len()
                        .max(negative_condition_order.len())) as u16
            };
            contrasts.push(EstimandContrast {
                contrast_id: format!("contrast:{}:{}", factor.factor_id, level),
                factor_id: factor.factor_id.clone(),
                level: level.clone(),
                positive_condition_order,
                negative_condition_order,
                estimand: format!(
                    "mean outcome({}) - mean outcome({})",
                    level, factor.baseline_level
                ),
                balance_milli,
            });
        }
    }
    contrasts.sort_by(|left, right| left.contrast_id.cmp(&right.contrast_id));
    let contrast_order = contrasts
        .iter()
        .map(|contrast| contrast.contrast_id.clone())
        .collect::<Vec<_>>();
    let baseline_by_factor = request
        .factors
        .iter()
        .map(|factor| (factor.factor_id.as_str(), factor.baseline_level.as_str()))
        .collect::<BTreeMap<_, _>>();
    let mut covered_interactions = BTreeSet::new();
    for left in &factor_order {
        for right in factor_order.iter().filter(|right| *right > left) {
            let key = interaction_key(left, right);
            let has_nonbaseline_pair = conditions.iter().any(|condition| {
                condition
                    .assignments
                    .get(left)
                    .zip(condition.assignments.get(right))
                    .is_some_and(|(left_level, right_level)| {
                        baseline_by_factor
                            .get(left.as_str())
                            .is_some_and(|baseline| left_level != baseline)
                            && baseline_by_factor
                                .get(right.as_str())
                                .is_some_and(|baseline| right_level != baseline)
                    })
            });
            if has_nonbaseline_pair {
                covered_interactions.insert(key);
            }
        }
    }
    let covered_interaction_order = covered_interactions.iter().cloned().collect::<Vec<_>>();
    let missing_interaction_order = request
        .required_interaction_order
        .iter()
        .filter(|interaction| !covered_interactions.contains(*interaction))
        .cloned()
        .collect::<Vec<_>>();
    let replicate_adequacy = (u32::from(request.replicates_per_condition).min(4) * 250) as u16;
    let balance_adequacy = contrasts
        .iter()
        .map(|contrast| u32::from(contrast.balance_milli))
        .min()
        .unwrap_or(0) as u16;
    let design_adequacy_milli = replicate_adequacy.min(balance_adequacy);
    let mut limitations = BTreeSet::from([
        "design adequacy is a bounded balance/replicate proxy, not a formal power calculation".to_string(),
        "assay variance, batch effects, and site calibration must be supplied by downstream analysis".to_string(),
    ]);
    let mut negative_evidence = BTreeSet::new();
    if !missing_interaction_order.is_empty() {
        negative_evidence.insert("required interaction coverage is incomplete".to_string());
    }
    if total_units > request.max_total_units {
        negative_evidence.insert(format!(
            "budget-blocked:total_units={}:max_total_units={}",
            total_units, request.max_total_units
        ));
    }
    let disposition = if total_units > request.max_total_units {
        ContrastDesignDisposition::BudgetBlocked
    } else if request.replicates_per_condition < 2
        || design_adequacy_milli < request.min_design_adequacy_milli
    {
        limitations
            .insert("replicate floor or balance floor is below the requested adequacy gate".into());
        ContrastDesignDisposition::Underpowered
    } else if !missing_interaction_order.is_empty() {
        ContrastDesignDisposition::Partial
    } else {
        ContrastDesignDisposition::Qualified
    };
    let mut output = ContrastDesign {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        objective: request.objective.clone(),
        model_system: request.model_system,
        modality: request.modality,
        factor_order,
        condition_order,
        conditions,
        contrast_order,
        contrasts,
        required_interaction_order: request.required_interaction_order.clone(),
        covered_interaction_order,
        missing_interaction_order,
        design_adequacy_milli,
        total_units,
        limitations: limitations.into_iter().collect(),
        negative_evidence: negative_evidence.into_iter().collect(),
        disposition,
        digest: ContentHash::of_bytes(b"unsealed-glioma-contrast-design"),
    };
    output.digest = ContentHash::of_value(&digest_input(&output))
        .map_err(|error| ContrastDesignError::Digest(error.to_string()))?;
    output.validate()?;
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bioprism_foundation::PRECLINICAL_BOUNDARY;

    fn request() -> ContrastDesignRequest {
        ContrastDesignRequest {
            objective: "test invasion perturbation contrast".into(),
            model_system: GliomaModelSystem::Organoid,
            modality: GliomaModality::FunctionalPerturbation,
            factors: vec![
                ContrastFactor {
                    factor_id: "drug".into(),
                    label: "drug perturbation".into(),
                    level_order: vec!["control".into(), "inhibitor".into()],
                    baseline_level: "control".into(),
                    perturbation_kind: "small_molecule".into(),
                },
                ContrastFactor {
                    factor_id: "matrix".into(),
                    label: "matrix context".into(),
                    level_order: vec!["control".into(), "stiff".into()],
                    baseline_level: "control".into(),
                    perturbation_kind: "microenvironment".into(),
                },
            ],
            replicates_per_condition: 3,
            max_conditions: 16,
            max_total_units: 32,
            min_design_adequacy_milli: 700,
            required_interaction_order: vec!["drug×matrix".into()],
        }
    }

    #[test]
    fn factorial_panel_is_balanced_and_replay_stable() {
        let output = design_glioma_contrast_panel(&request()).unwrap();
        let replay = design_glioma_contrast_panel(&request()).unwrap();
        assert_eq!(output, replay);
        assert_eq!(output.conditions.len(), 4);
        assert_eq!(output.contrasts.len(), 2);
        assert_eq!(output.disposition, ContrastDesignDisposition::Qualified);
        assert_eq!(output.design_adequacy_milli, 750);
    }

    #[test]
    fn budget_and_replicate_gates_remain_explicit() {
        let mut request = request();
        request.max_total_units = 2;
        let output = design_glioma_contrast_panel(&request).unwrap();
        assert_eq!(output.disposition, ContrastDesignDisposition::BudgetBlocked);
        assert!(!output.negative_evidence.is_empty());
        request.max_total_units = 32;
        request.replicates_per_condition = 1;
        let output = design_glioma_contrast_panel(&request).unwrap();
        assert_eq!(output.disposition, ContrastDesignDisposition::Underpowered);
    }

    #[test]
    fn preclinical_boundary_fixture_is_constant() {
        assert!(PRECLINICAL_BOUNDARY.contains("no diagnosis"));
        assert!(PRECLINICAL_BOUNDARY.contains("no human-subject"));
    }
}
