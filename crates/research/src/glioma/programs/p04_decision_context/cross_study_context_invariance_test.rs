//! Cross-study context invariance testing for `GAF-GLIOMA-P04-F26`.
//!
//! A context difference report says where studies differ; it does not say whether a declared
//! research rule survives those differences. This feature executes a bounded, typed predicate
//! rule over study-local context metadata, applies declared nuisance-field perturbations, and
//! repeats the rule under leave-one-study-out partitions. It records flips, score ranges,
//! unsupported strata, independent-group floors, and concrete counterexamples. The rule is a
//! preclinical transportability diagnostic, not a biological or clinical conclusion.

use super::cross_study_context_diff::{StudyContextField, StudyContextSpec};
use bioprism_foundation::PRECLINICAL_BOUNDARY;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use thiserror::Error;

pub const FEATURE_ID: &str = "GAF-GLIOMA-P04-F26";
pub const OUTPUT_SCHEMA: &str = "GliomaCrossStudyContextInvariance1@1";
pub const MAX_STUDIES: usize = 128;
pub const MAX_PREDICATES: usize = 128;
pub const MAX_PERTURBATIONS: usize = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InvariancePredicateOperator {
    Equal,
    NotEqual,
    Present,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvariancePredicate {
    pub field_id: String,
    pub operator: InvariancePredicateOperator,
    pub expected_value: Option<String>,
    pub weight_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextDecisionRule {
    pub rule_id: String,
    pub predicates: Vec<InvariancePredicate>,
    pub threshold_milli: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextNuisancePerturbation {
    pub perturbation_id: String,
    pub field_id: String,
    pub replacement_value: Option<String>,
    pub measured: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvarianceStudyInput {
    pub study: StudyContextSpec,
    pub independent_group: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossStudyContextInvarianceRequest {
    pub objective: String,
    pub rule: ContextDecisionRule,
    pub studies: Vec<InvarianceStudyInput>,
    pub nuisance_field_order: Vec<String>,
    pub perturbations: Vec<ContextNuisancePerturbation>,
    pub minimum_independent_groups: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InvarianceEvaluation {
    pub study_id: String,
    pub independent_group: String,
    pub score_milli: u16,
    pub decision: bool,
    pub supported: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextInvarianceFieldResult {
    pub field_id: String,
    pub baseline_score_min_milli: u16,
    pub baseline_score_max_milli: u16,
    pub perturbed_score_min_milli: Option<u16>,
    pub perturbed_score_max_milli: Option<u16>,
    pub baseline_positive_study_order: Vec<String>,
    pub perturbation_flip_order: Vec<String>,
    pub leave_one_out_flip_order: Vec<String>,
    pub unsupported_study_order: Vec<String>,
    pub counterexample_order: Vec<String>,
    pub invariant: bool,
    pub context_sensitive: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextInvarianceDisposition {
    Invariant,
    ContextSensitive,
    Unsupported,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CrossStudyContextInvarianceReport {
    pub feature_id: String,
    pub output_schema: String,
    pub boundary: String,
    pub objective: String,
    pub rule_id: String,
    pub study_order: Vec<String>,
    pub independent_group: BTreeMap<String, String>,
    pub baseline_evaluations: Vec<InvarianceEvaluation>,
    pub leave_one_out_order: Vec<String>,
    pub fields: Vec<ContextInvarianceFieldResult>,
    pub invariant_field_order: Vec<String>,
    pub context_sensitive_field_order: Vec<String>,
    pub unsupported_field_order: Vec<String>,
    pub counterexample_order: Vec<String>,
    pub disposition: ContextInvarianceDisposition,
    pub next_action: String,
    pub digest: ContentHash,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CrossStudyContextInvarianceError {
    #[error("context invariance request is invalid: {0}")]
    InvalidRequest(String),
    #[error("context invariance output is invalid: {0}")]
    InvalidOutput(String),
    #[error("context invariance digest failed: {0}")]
    Digest(String),
}

fn bounded_text(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty() && trimmed.len() <= 512 && !trimmed.chars().any(char::is_control)
}

fn canonical<T: Ord>(values: &[T]) -> bool {
    values.windows(2).all(|window| window[0] < window[1])
}

fn digest_input(report: &CrossStudyContextInvarianceReport) -> serde_json::Value {
    serde_json::json!({
        "feature_id": report.feature_id,
        "output_schema": report.output_schema,
        "boundary": report.boundary,
        "objective": report.objective,
        "rule_id": report.rule_id,
        "study_order": report.study_order,
        "independent_group": report.independent_group,
        "baseline_evaluations": report.baseline_evaluations,
        "leave_one_out_order": report.leave_one_out_order,
        "fields": report.fields,
        "invariant_field_order": report.invariant_field_order,
        "context_sensitive_field_order": report.context_sensitive_field_order,
        "unsupported_field_order": report.unsupported_field_order,
        "counterexample_order": report.counterexample_order,
        "disposition": report.disposition,
        "next_action": report.next_action,
    })
}

impl CrossStudyContextInvarianceReport {
    pub fn validate(&self) -> Result<(), CrossStudyContextInvarianceError> {
        if self.feature_id != FEATURE_ID
            || self.output_schema != OUTPUT_SCHEMA
            || self.boundary != PRECLINICAL_BOUNDARY
            || !bounded_text(&self.objective)
            || !bounded_text(&self.rule_id)
            || self.study_order.len() < 2
            || !canonical(&self.study_order)
            || self.independent_group.keys().cloned().collect::<Vec<_>>() != self.study_order
            || !canonical(&self.leave_one_out_order)
            || self.fields.iter().any(|field| {
                !bounded_text(&field.field_id)
                    || !canonical(&field.baseline_positive_study_order)
                    || !canonical(&field.perturbation_flip_order)
                    || !canonical(&field.leave_one_out_flip_order)
                    || !canonical(&field.unsupported_study_order)
                    || !canonical(&field.counterexample_order)
            })
            || !canonical(&self.invariant_field_order)
            || !canonical(&self.context_sensitive_field_order)
            || !canonical(&self.unsupported_field_order)
            || !canonical(&self.counterexample_order)
            || self.digest.as_str().len() != 64
        {
            return Err(CrossStudyContextInvarianceError::InvalidOutput(
                "boundary, study ordering, field partitions, or digest is invalid".into(),
            ));
        }
        let expected = ContentHash::of_value(&digest_input(self))
            .map_err(|error| CrossStudyContextInvarianceError::Digest(error.to_string()))?;
        if expected != self.digest {
            return Err(CrossStudyContextInvarianceError::InvalidOutput(
                "context invariance digest is not content-addressed".into(),
            ));
        }
        Ok(())
    }
}

fn validate_request(
    request: &CrossStudyContextInvarianceRequest,
) -> Result<(), CrossStudyContextInvarianceError> {
    if !bounded_text(&request.objective)
        || request.studies.len() < 2
        || request.studies.len() > MAX_STUDIES
        || request.rule.predicates.is_empty()
        || request.rule.predicates.len() > MAX_PREDICATES
        || !bounded_text(&request.rule.rule_id)
        || request.rule.threshold_milli == 0
        || request.rule.threshold_milli > 1_000
        || request.minimum_independent_groups == 0
        || request.perturbations.len() > MAX_PERTURBATIONS
        || request.nuisance_field_order.is_empty()
        || !canonical(&request.nuisance_field_order)
    {
        return Err(CrossStudyContextInvarianceError::InvalidRequest(
            "objective, bounded studies, rule predicates, nuisance fields, group floor, and perturbation limits are required".into(),
        ));
    }
    let mut study_ids = BTreeSet::new();
    let mut groups = BTreeSet::new();
    for input in &request.studies {
        if !bounded_text(&input.study.study_id)
            || !bounded_text(&input.study.context_version)
            || !bounded_text(&input.independent_group)
            || !study_ids.insert(input.study.study_id.clone())
        {
            return Err(CrossStudyContextInvarianceError::InvalidRequest(
                "study and independent-group identities must be unique and bounded".into(),
            ));
        }
        groups.insert(input.independent_group.clone());
        let mut field_ids = BTreeSet::new();
        for field in &input.study.fields {
            if !bounded_text(&field.field_id)
                || !field_ids.insert(field.field_id.clone())
                || field
                    .value
                    .as_ref()
                    .is_some_and(|value| !bounded_text(value))
                || field.unit.as_ref().is_some_and(|unit| !bounded_text(unit))
                || (field.measured
                    && field
                        .value
                        .as_ref()
                        .is_none_or(|value| value.trim().is_empty()))
                || field
                    .source_digest
                    .as_ref()
                    .is_some_and(|digest| digest.as_str().len() != 64)
            {
                return Err(CrossStudyContextInvarianceError::InvalidRequest(
                    "study fields must be unique, bounded, and explicitly measured or unmeasured"
                        .into(),
                ));
            }
        }
    }
    if groups.len() < request.minimum_independent_groups {
        return Err(CrossStudyContextInvarianceError::InvalidRequest(
            "independent-group floor exceeds supplied groups".into(),
        ));
    }
    let nuisance = request
        .nuisance_field_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut predicate_ids = BTreeSet::new();
    for predicate in &request.rule.predicates {
        if !bounded_text(&predicate.field_id)
            || !predicate_ids.insert(predicate.field_id.clone())
            || predicate.weight_milli == 0
            || predicate.weight_milli > 1_000
            || matches!(
                predicate.operator,
                InvariancePredicateOperator::Equal | InvariancePredicateOperator::NotEqual
            ) != predicate.expected_value.is_some()
            || predicate
                .expected_value
                .as_ref()
                .is_some_and(|value| !bounded_text(value))
        {
            return Err(CrossStudyContextInvarianceError::InvalidRequest(
                "rule predicates require unique fields, positive weights, and operator-matched expected values".into(),
            ));
        }
    }
    if request
        .nuisance_field_order
        .iter()
        .any(|field| !bounded_text(field))
        || request.perturbations.iter().any(|perturbation| {
            !bounded_text(&perturbation.perturbation_id)
                || !bounded_text(&perturbation.field_id)
                || !nuisance.contains(&perturbation.field_id)
                || perturbation
                    .replacement_value
                    .as_ref()
                    .is_some_and(|value| !bounded_text(value))
        })
    {
        return Err(CrossStudyContextInvarianceError::InvalidRequest(
            "perturbations must bind to declared nuisance fields and remain bounded".into(),
        ));
    }
    Ok(())
}

fn field_map(study: &StudyContextSpec) -> BTreeMap<String, StudyContextField> {
    study
        .fields
        .iter()
        .cloned()
        .map(|field| (field.field_id.clone(), field))
        .collect()
}

fn evaluate_rule(
    rule: &ContextDecisionRule,
    study: &InvarianceStudyInput,
    override_field: Option<(&str, Option<&String>, bool)>,
) -> InvarianceEvaluation {
    let fields = field_map(&study.study);
    let mut score = 0_u32;
    let mut supported = true;
    for predicate in &rule.predicates {
        let (value, measured) =
            if let Some((field_id, replacement, override_measured)) = override_field {
                if field_id == predicate.field_id {
                    (replacement.cloned(), override_measured)
                } else {
                    fields
                        .get(&predicate.field_id)
                        .map(|field| (field.value.clone(), field.measured))
                        .unwrap_or((None, false))
                }
            } else {
                fields
                    .get(&predicate.field_id)
                    .map(|field| (field.value.clone(), field.measured))
                    .unwrap_or((None, false))
            };
        let predicate_supported = match predicate.operator {
            InvariancePredicateOperator::Present | InvariancePredicateOperator::Missing => true,
            InvariancePredicateOperator::Equal | InvariancePredicateOperator::NotEqual => measured,
        };
        supported &= predicate_supported;
        let matches = match predicate.operator {
            InvariancePredicateOperator::Equal => measured && value == predicate.expected_value,
            InvariancePredicateOperator::NotEqual => measured && value != predicate.expected_value,
            InvariancePredicateOperator::Present => measured && value.is_some(),
            InvariancePredicateOperator::Missing => !measured || value.is_none(),
        };
        if matches {
            score = score.saturating_add(u32::from(predicate.weight_milli));
        }
    }
    InvarianceEvaluation {
        study_id: study.study.study_id.clone(),
        independent_group: study.independent_group.clone(),
        score_milli: score.min(1_000) as u16,
        decision: score >= u32::from(rule.threshold_milli),
        supported,
    }
}

fn majority(evaluations: &[InvarianceEvaluation]) -> Option<bool> {
    let supported = evaluations
        .iter()
        .filter(|evaluation| evaluation.supported)
        .collect::<Vec<_>>();
    if supported.is_empty() {
        return None;
    }
    let positives = supported
        .iter()
        .filter(|evaluation| evaluation.decision)
        .count();
    if positives * 2 == supported.len() {
        None
    } else {
        Some(positives * 2 > supported.len())
    }
}

fn min_max(evaluations: &[InvarianceEvaluation]) -> (u16, u16) {
    let scores = evaluations
        .iter()
        .map(|evaluation| evaluation.score_milli)
        .collect::<Vec<_>>();
    (
        scores.iter().copied().min().unwrap_or(0),
        scores.iter().copied().max().unwrap_or(0),
    )
}

/// Evaluate a typed decision rule under nuisance perturbations and leave-one-study-out splits.
pub fn test_glioma_cross_study_context_invariance(
    request: &CrossStudyContextInvarianceRequest,
) -> Result<CrossStudyContextInvarianceReport, CrossStudyContextInvarianceError> {
    validate_request(request)?;
    let mut studies = request.studies.clone();
    studies.sort_by(|left, right| left.study.study_id.cmp(&right.study.study_id));
    let baseline_evaluations = studies
        .iter()
        .map(|study| evaluate_rule(&request.rule, study, None))
        .collect::<Vec<_>>();
    let leave_one_out_order = studies
        .iter()
        .map(|study| format!("leave-one-out:{}", study.study.study_id))
        .collect::<Vec<_>>();
    let mut invariant_fields = Vec::new();
    let mut sensitive_fields = Vec::new();
    let mut unsupported_fields = Vec::new();
    let mut counterexamples = BTreeSet::new();
    let mut field_results = Vec::new();
    for field_id in &request.nuisance_field_order {
        let field_evaluations = baseline_evaluations
            .iter()
            .filter(|evaluation| evaluation.supported)
            .filter(|evaluation| {
                studies
                    .iter()
                    .find(|study| study.study.study_id == evaluation.study_id)
                    .is_some_and(|study| field_map(&study.study).contains_key(field_id))
            })
            .cloned()
            .collect::<Vec<_>>();
        let (baseline_min, baseline_max) = min_max(&field_evaluations);
        let field_independent_groups = field_evaluations
            .iter()
            .map(|evaluation| evaluation.independent_group.clone())
            .collect::<BTreeSet<_>>();
        let field_baseline_majority = majority(&field_evaluations);
        let baseline_positive_study_order = field_evaluations
            .iter()
            .filter(|evaluation| evaluation.decision)
            .map(|evaluation| evaluation.study_id.clone())
            .collect::<Vec<_>>();
        let unsupported_study_order = baseline_evaluations
            .iter()
            .filter(|evaluation| !evaluation.supported)
            .map(|evaluation| evaluation.study_id.clone())
            .collect::<Vec<_>>();
        let mut perturbation_flips = BTreeSet::new();
        let mut leave_one_out_flips = BTreeSet::new();
        let mut perturbed_evaluations = Vec::new();
        for perturbation in request
            .perturbations
            .iter()
            .filter(|perturbation| perturbation.field_id == *field_id)
        {
            for study in &studies {
                let evaluation = evaluate_rule(
                    &request.rule,
                    study,
                    Some((
                        field_id,
                        perturbation.replacement_value.as_ref(),
                        perturbation.measured,
                    )),
                );
                if let Some(baseline) = baseline_evaluations
                    .iter()
                    .find(|baseline| baseline.study_id == evaluation.study_id)
                {
                    if baseline.supported
                        && evaluation.supported
                        && baseline.decision != evaluation.decision
                    {
                        let label = format!(
                            "{}|{}|{}",
                            perturbation.perturbation_id, study.study.study_id, evaluation.decision
                        );
                        perturbation_flips.insert(label.clone());
                        counterexamples.insert(format!("{}:perturbation:{label}", field_id));
                    }
                }
                perturbed_evaluations.push(evaluation);
            }
        }
        let supported_perturbed_evaluations = perturbed_evaluations
            .iter()
            .filter(|evaluation| evaluation.supported)
            .cloned()
            .collect::<Vec<_>>();
        let (perturbed_min, perturbed_max) = if supported_perturbed_evaluations.is_empty() {
            (None, None)
        } else {
            let (minimum, maximum) = min_max(&supported_perturbed_evaluations);
            (Some(minimum), Some(maximum))
        };
        for omitted in &studies {
            let retained = field_evaluations
                .iter()
                .filter(|evaluation| evaluation.study_id != omitted.study.study_id)
                .cloned()
                .collect::<Vec<_>>();
            let retained_groups = retained
                .iter()
                .filter(|evaluation| evaluation.supported)
                .map(|evaluation| evaluation.independent_group.clone())
                .collect::<BTreeSet<_>>();
            if retained_groups.len() < request.minimum_independent_groups {
                continue;
            }
            let held_out_majority = majority(&retained);
            if field_baseline_majority != held_out_majority {
                let label = format!("{}|{}", field_id, omitted.study.study_id);
                leave_one_out_flips.insert(label.clone());
                counterexamples.insert(format!("{}:leave-one-out:{label}", field_id));
            }
        }
        let has_unsupported = !unsupported_study_order.is_empty()
            || field_independent_groups.len() < request.minimum_independent_groups
            || (perturbed_evaluations.is_empty() && leave_one_out_flips.is_empty());
        let context_sensitive = !perturbation_flips.is_empty() || !leave_one_out_flips.is_empty();
        let invariant = !has_unsupported && !context_sensitive;
        if context_sensitive {
            sensitive_fields.push(field_id.clone());
        } else if has_unsupported {
            unsupported_fields.push(field_id.clone());
        } else {
            invariant_fields.push(field_id.clone());
        }
        field_results.push(ContextInvarianceFieldResult {
            field_id: field_id.clone(),
            baseline_score_min_milli: baseline_min,
            baseline_score_max_milli: baseline_max,
            perturbed_score_min_milli: perturbed_min,
            perturbed_score_max_milli: perturbed_max,
            baseline_positive_study_order,
            perturbation_flip_order: perturbation_flips.into_iter().collect(),
            leave_one_out_flip_order: leave_one_out_flips.into_iter().collect(),
            unsupported_study_order,
            counterexample_order: counterexamples
                .iter()
                .filter(|entry| entry.starts_with(&format!("{}:", field_id)))
                .cloned()
                .collect(),
            invariant,
            context_sensitive,
        });
    }
    let disposition = if !sensitive_fields.is_empty() {
        ContextInvarianceDisposition::ContextSensitive
    } else if !unsupported_fields.is_empty() {
        ContextInvarianceDisposition::Unsupported
    } else {
        ContextInvarianceDisposition::Invariant
    };
    let next_action = match disposition {
        ContextInvarianceDisposition::Invariant => "retain the rule for the declared context range and validate it prospectively on an independent study".into(),
        ContextInvarianceDisposition::ContextSensitive => "stratify the rule by the reported nuisance field and route each counterexample to replication or redesign".into(),
        ContextInvarianceDisposition::Unsupported => "acquire the missing context or independent group before labeling the rule invariant".into(),
    };
    let mut report = CrossStudyContextInvarianceReport {
        feature_id: FEATURE_ID.into(),
        output_schema: OUTPUT_SCHEMA.into(),
        boundary: PRECLINICAL_BOUNDARY.into(),
        objective: request.objective.clone(),
        rule_id: request.rule.rule_id.clone(),
        study_order: studies
            .iter()
            .map(|study| study.study.study_id.clone())
            .collect(),
        independent_group: studies
            .iter()
            .map(|study| {
                (
                    study.study.study_id.clone(),
                    study.independent_group.clone(),
                )
            })
            .collect(),
        baseline_evaluations,
        leave_one_out_order,
        fields: field_results,
        invariant_field_order: invariant_fields,
        context_sensitive_field_order: sensitive_fields,
        unsupported_field_order: unsupported_fields,
        counterexample_order: counterexamples.into_iter().collect(),
        disposition,
        next_action,
        digest: ContentHash::of_bytes(b"unsealed-glioma-cross-study-context-invariance"),
    };
    report.digest = ContentHash::of_value(&digest_input(&report))
        .map_err(|error| CrossStudyContextInvarianceError::Digest(error.to_string()))?;
    report.validate()?;
    Ok(report)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glioma::programs::p04_decision_context::cross_study_context_diff::{
        ContextFieldDomain, StudyContextField,
    };

    fn hash(seed: &str) -> ContentHash {
        ContentHash::of_bytes(seed.as_bytes())
    }

    fn field(id: &str, value: &str, measured: bool) -> StudyContextField {
        StudyContextField {
            field_id: id.into(),
            domain: ContextFieldDomain::Model,
            value: measured.then(|| value.into()),
            unit: None,
            measured,
            source_digest: Some(hash(id)),
        }
    }

    fn study(id: &str, group: &str, model: &str, measured: bool) -> InvarianceStudyInput {
        InvarianceStudyInput {
            study: StudyContextSpec {
                study_id: id.into(),
                context_version: "ctx-1".into(),
                fields: vec![
                    field("model", model, measured),
                    field("assay", "imaging", true),
                ],
            },
            independent_group: group.into(),
        }
    }

    fn request(
        studies: Vec<InvarianceStudyInput>,
        perturbations: Vec<ContextNuisancePerturbation>,
    ) -> CrossStudyContextInvarianceRequest {
        CrossStudyContextInvarianceRequest {
            objective: "transport invasion rule".into(),
            rule: ContextDecisionRule {
                rule_id: "organoid-rule".into(),
                predicates: vec![InvariancePredicate {
                    field_id: "model".into(),
                    operator: InvariancePredicateOperator::Equal,
                    expected_value: Some("organoid".into()),
                    weight_milli: 1_000,
                }],
                threshold_milli: 700,
            },
            studies,
            nuisance_field_order: vec!["model".into()],
            perturbations,
            minimum_independent_groups: 2,
        }
    }

    #[test]
    fn injected_nuisance_perturbation_is_a_context_sensitive_counterexample() {
        let output = test_glioma_cross_study_context_invariance(&request(
            vec![
                study("a", "g1", "organoid", true),
                study("b", "g2", "organoid", true),
            ],
            vec![ContextNuisancePerturbation {
                perturbation_id: "xenograft-shift".into(),
                field_id: "model".into(),
                replacement_value: Some("xenograft".into()),
                measured: true,
            }],
        ))
        .expect("invariance report");
        assert_eq!(
            output.disposition,
            ContextInvarianceDisposition::ContextSensitive
        );
        assert!(output.fields[0].context_sensitive);
        assert!(!output.counterexample_order.is_empty());
    }

    #[test]
    fn no_declared_perturbation_is_unsupported_not_invariant() {
        let output = test_glioma_cross_study_context_invariance(&request(
            vec![
                study("a", "g1", "organoid", true),
                study("b", "g2", "organoid", true),
            ],
            Vec::new(),
        ))
        .expect("invariance report");
        assert_eq!(
            output.disposition,
            ContextInvarianceDisposition::Unsupported
        );
        assert_eq!(output.unsupported_field_order, vec!["model"]);
    }

    #[test]
    fn missing_measurement_is_explicitly_unsupported() {
        let output = test_glioma_cross_study_context_invariance(&request(
            vec![
                study("a", "g1", "organoid", true),
                study("b", "g2", "organoid", false),
            ],
            vec![ContextNuisancePerturbation {
                perturbation_id: "keep".into(),
                field_id: "model".into(),
                replacement_value: Some("organoid".into()),
                measured: true,
            }],
        ))
        .expect("invariance report");
        assert_eq!(
            output.disposition,
            ContextInvarianceDisposition::Unsupported
        );
        assert_eq!(output.fields[0].unsupported_study_order, vec!["b"]);
    }

    #[test]
    fn leave_one_study_out_flip_is_retained() {
        let mut studies = vec![
            study("a", "g1", "organoid", true),
            study("b", "g2", "organoid", true),
            study("c", "g3", "xenograft", true),
        ];
        let mut request = request(
            studies.clone(),
            vec![ContextNuisancePerturbation {
                perturbation_id: "keep".into(),
                field_id: "model".into(),
                replacement_value: Some("organoid".into()),
                measured: true,
            }],
        );
        request.rule.threshold_milli = 500;
        let output =
            test_glioma_cross_study_context_invariance(&request).expect("invariance report");
        assert!(!output.fields[0].leave_one_out_flip_order.is_empty());
        studies.reverse();
        let permuted =
            test_glioma_cross_study_context_invariance(&CrossStudyContextInvarianceRequest {
                studies,
                ..request
            })
            .expect("permuted report");
        assert_eq!(output.digest, permuted.digest);
    }

    #[test]
    fn permutation_of_studies_does_not_change_digest() {
        let left = request(
            vec![
                study("a", "g1", "organoid", true),
                study("b", "g2", "organoid", true),
            ],
            vec![ContextNuisancePerturbation {
                perturbation_id: "x".into(),
                field_id: "model".into(),
                replacement_value: Some("xenograft".into()),
                measured: true,
            }],
        );
        let right = CrossStudyContextInvarianceRequest {
            studies: left.studies.iter().cloned().rev().collect(),
            ..left.clone()
        };
        assert_eq!(
            test_glioma_cross_study_context_invariance(&left)
                .unwrap()
                .digest,
            test_glioma_cross_study_context_invariance(&right)
                .unwrap()
                .digest
        );
    }
}
