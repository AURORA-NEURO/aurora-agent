//! Uncertainty kinds and reference standards.
//!
//! Implements blueprint 25.12. Its first invariant is the whole point of the module:
//! **"uncertainty type is never reduced to one generic confidence."**
//!
//! A single `confidence: 0.82` is the most common lossy compression in applied biology, and it
//! is lossy in a way that changes decisions. Aleatoric uncertainty says *collect more of the
//! same and it will not help*. Epistemic uncertainty says *it will*. Mapping uncertainty says
//! *the number is fine and the label is wrong*. Distribution-shift uncertainty says *this model
//! has never seen anything like this case*. Averaging them produces a number that answers none
//! of those questions while looking like it answers all of them.
//!
//! So [`UncertaintyBudget`] holds at most one component per [`UncertaintyKind`] and exposes no
//! total. [`UncertaintyBudget::pool`] exists precisely so that the refusal is a value a caller
//! can receive: pooling within a kind is arithmetic, and pooling across kinds returns
//! [`crate::UncertaintyError::CrossKindPooling`].
//!
//! The second invariant, **"expert disagreement remains visible"**, is enforced at adjudication:
//! an [`AdjudicationRecord`] that does not carry every reviewer who dissented from its outcome
//! is rejected. Adjudication is allowed to *decide*; it is not allowed to make the
//! disagreement disappear.
//!
//! The third, **"calibration is contextual"**, is enforced with the scope order:
//! a [`CalibrationCurve`] applies only where the query scope refines the scope it was fitted
//! in. That reuses [`bioprism_scope::ScopeKey::refines`] rather than introducing a second,
//! weaker notion of "close enough".
//!
//! # Not implemented
//!
//! 25.12 lists "proper-scoring compatibility" under validation. Nothing here computes a Brier
//! or log score; the module checks that a distribution *could* be scored (normalised, on a
//! declared support) and leaves scoring to the oracle layer. Reviewer independence is recorded
//! as a declared flag, not inferred — inferring it needs a study design this IR does not see.

use crate::error::UncertaintyError;
use bioprism_scope::ScopeKey;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// Tolerance for the categorical normalisation check.
///
/// Distributions arrive from adapters that rounded to three or four decimals, so an exact test
/// would reject sound inputs; a loose one would admit a distribution that is missing a class.
const NORMALISATION_TOLERANCE: f64 = 1e-6;

/// The distinct kinds of not-knowing. These are not points on one axis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UncertaintyKind {
    /// Irreducible variation in the system. More data of the same kind does not shrink it.
    Aleatoric,
    /// Ignorance about parameters or structure. More data does shrink it.
    Epistemic,
    /// Noise the instrument added, described by an [`crate::lens::ErrorModel`].
    Measurement,
    /// Ambiguity in translating a source term into an ontology term.
    Mapping,
    /// Disagreement between qualified humans reading the same material.
    Expert,
    /// The gap between the population a model was fitted on and the case in front of it.
    DistributionShift,
}

impl fmt::Display for UncertaintyKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            UncertaintyKind::Aleatoric => "aleatoric",
            UncertaintyKind::Epistemic => "epistemic",
            UncertaintyKind::Measurement => "measurement",
            UncertaintyKind::Mapping => "mapping",
            UncertaintyKind::Expert => "expert",
            UncertaintyKind::DistributionShift => "distribution-shift",
        };
        f.write_str(name)
    }
}

/// One reader's call on one item.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReviewerAssessment {
    pub reviewer: String,
    pub label: String,
    pub confidence: Option<f64>,
    /// Whether this reader worked without seeing the others' calls.
    ///
    /// Declared, never inferred. Three readers in a room produce one opinion, and a record
    /// that does not say which situation it was cannot be pooled honestly.
    pub independent: bool,
}

impl ReviewerAssessment {
    pub fn new(reviewer: impl Into<String>, label: impl Into<String>, independent: bool) -> Self {
        ReviewerAssessment {
            reviewer: reviewer.into(),
            label: label.into(),
            confidence: None,
            independent,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReviewerDistribution {
    pub assessments: Vec<ReviewerAssessment>,
}

impl ReviewerDistribution {
    pub fn new(assessments: Vec<ReviewerAssessment>) -> Self {
        ReviewerDistribution { assessments }
    }

    /// How many readers chose each label. The disagreement itself, not a summary of it.
    pub fn label_counts(&self) -> BTreeMap<String, usize> {
        let mut counts = BTreeMap::new();
        for assessment in &self.assessments {
            *counts.entry(assessment.label.clone()).or_insert(0) += 1;
        }
        counts
    }

    pub fn is_unanimous(&self) -> bool {
        self.label_counts().len() <= 1
    }

    /// Readers whose call differs from `outcome`.
    pub fn dissenters(&self, outcome: &str) -> Vec<&ReviewerAssessment> {
        self.assessments
            .iter()
            .filter(|assessment| assessment.label != outcome)
            .collect()
    }

    pub fn validate(&self, subject: &str) -> Result<(), UncertaintyError> {
        if self.assessments.is_empty() {
            return Err(UncertaintyError::NoReviewers {
                subject: subject.to_string(),
            });
        }
        for assessment in &self.assessments {
            if let Some(confidence) = assessment.confidence {
                if !(0.0..=1.0).contains(&confidence) {
                    return Err(UncertaintyError::ProbabilityOutOfRange {
                        subject: subject.to_string(),
                        label: assessment.reviewer.clone(),
                        value: confidence.to_string(),
                    });
                }
            }
        }
        Ok(())
    }
}

/// A decision that resolves a reviewer disagreement without deleting it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdjudicationRecord {
    pub adjudicator: String,
    pub outcome: String,
    /// How the outcome was reached: "majority", "senior review", "consensus meeting".
    pub method: String,
    /// Every reviewer whose call differed from `outcome`, carried forward verbatim.
    pub dissent: Vec<ReviewerAssessment>,
}

impl AdjudicationRecord {
    /// Checks that no dissenting reader was dropped on the way to the outcome.
    ///
    /// This is the enforcement point for "expert disagreement remains visible". A record that
    /// silently kept only the concurring readers would reconcile perfectly and misrepresent
    /// the evidence, so the check is against the original distribution, not against itself.
    pub fn validate(&self, panel: &ReviewerDistribution) -> Result<(), UncertaintyError> {
        let carried: BTreeSet<&str> = self
            .dissent
            .iter()
            .map(|assessment| assessment.reviewer.as_str())
            .collect();
        for dissenter in panel.dissenters(&self.outcome) {
            if !carried.contains(dissenter.reviewer.as_str()) {
                return Err(UncertaintyError::DissentErased {
                    adjudicator: self.adjudicator.clone(),
                    reviewer: dissenter.reviewer.clone(),
                });
            }
        }
        Ok(())
    }
}

/// How one uncertainty component is written down.
///
/// The variants are not interchangeable encodings of one thing. An interval and a reviewer
/// panel are different objects, and the enum keeps them different so that a consumer must
/// decide what to do with each instead of receiving a pre-flattened number.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "representation", rename_all = "snake_case")]
pub enum Representation {
    /// A coverage interval. `coverage` is the nominal probability, e.g. 0.95.
    Interval {
        lower: f64,
        upper: f64,
        coverage: f64,
    },
    /// A probability over a discrete support. Must normalise.
    Categorical { probabilities: BTreeMap<String, f64> },
    /// A standard error on the point estimate.
    StandardError { value: f64 },
    /// The raw panel, not a summary of it.
    Panel { distribution: ReviewerDistribution },
    /// One source term with several defensible ontology targets.
    MappingAmbiguity {
        source_term: String,
        ontology_version: String,
        candidates: BTreeMap<String, f64>,
    },
    /// A named shift diagnostic and its value against a named reference cohort.
    ShiftDiagnostic {
        statistic: String,
        value: f64,
        reference_cohort: String,
    },
    /// The case could not be assessed at all, with the reason 25.12 requires.
    Ungradable { reason: String },
}

impl Representation {
    fn validate(&self, subject: &str) -> Result<(), UncertaintyError> {
        match self {
            Representation::Interval {
                lower,
                upper,
                coverage,
            } => {
                if lower > upper {
                    return Err(UncertaintyError::InvertedInterval {
                        subject: subject.to_string(),
                        lower: lower.to_string(),
                        upper: upper.to_string(),
                    });
                }
                if *coverage <= 0.0 || *coverage > 1.0 {
                    return Err(UncertaintyError::InvalidCoverage {
                        subject: subject.to_string(),
                        coverage: coverage.to_string(),
                    });
                }
                Ok(())
            }
            Representation::Categorical { probabilities } => {
                check_distribution(subject, probabilities)
            }
            Representation::MappingAmbiguity { candidates, .. } => {
                check_distribution(subject, candidates)
            }
            Representation::Panel { distribution } => distribution.validate(subject),
            Representation::StandardError { .. }
            | Representation::ShiftDiagnostic { .. }
            | Representation::Ungradable { .. } => Ok(()),
        }
    }
}

fn check_distribution(
    subject: &str,
    probabilities: &BTreeMap<String, f64>,
) -> Result<(), UncertaintyError> {
    let mut total = 0.0;
    for (label, probability) in probabilities {
        if !(0.0..=1.0).contains(probability) {
            return Err(UncertaintyError::ProbabilityOutOfRange {
                subject: subject.to_string(),
                label: label.clone(),
                value: probability.to_string(),
            });
        }
        total += probability;
    }
    if (total - 1.0).abs() > NORMALISATION_TOLERANCE {
        return Err(UncertaintyError::UnnormalizedDistribution {
            subject: subject.to_string(),
            sum: total.to_string(),
        });
    }
    Ok(())
}

/// One kind of uncertainty, written down one way, with the reason it is there.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UncertaintyComponent {
    pub kind: UncertaintyKind,
    pub representation: Representation,
    /// What the numbers range over: the outcome, a segmentation boundary, a class label.
    pub support: String,
    /// Where this component came from. A budget with unexplained components cannot be audited.
    pub rationale: String,
}

impl UncertaintyComponent {
    pub fn new(
        kind: UncertaintyKind,
        representation: Representation,
        support: impl Into<String>,
    ) -> Self {
        UncertaintyComponent {
            kind,
            representation,
            support: support.into(),
            rationale: String::new(),
        }
    }

    pub fn because(mut self, rationale: impl Into<String>) -> Self {
        self.rationale = rationale.into();
        self
    }

    pub fn validate(&self, subject: &str) -> Result<(), UncertaintyError> {
        self.representation.validate(subject)
    }
}

/// Everything a claim says about what it does not know, kept separated by kind.
///
/// There is deliberately no `total()`, no `confidence()` and no `Ord`. Any of the three would
/// be an invitation to sort claims by a scalar that does not exist.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct UncertaintyBudget {
    components: BTreeMap<UncertaintyKind, UncertaintyComponent>,
}

impl UncertaintyBudget {
    pub fn new() -> Self {
        UncertaintyBudget {
            components: BTreeMap::new(),
        }
    }

    /// Adds a component, refusing a second component of the same kind.
    ///
    /// Two aleatoric components would have to be combined to be used, and combining them is a
    /// modelling decision the author should make explicitly with [`UncertaintyBudget::pool`].
    pub fn declare(
        &mut self,
        subject: &str,
        component: UncertaintyComponent,
    ) -> Result<(), UncertaintyError> {
        component.validate(subject)?;
        if self.components.contains_key(&component.kind) {
            return Err(UncertaintyError::DuplicateKind {
                subject: subject.to_string(),
                kind: component.kind,
            });
        }
        self.components.insert(component.kind, component);
        Ok(())
    }

    pub fn component(&self, kind: UncertaintyKind) -> Option<&UncertaintyComponent> {
        self.components.get(&kind)
    }

    pub fn kinds(&self) -> BTreeSet<UncertaintyKind> {
        self.components.keys().copied().collect()
    }

    pub fn is_empty(&self) -> bool {
        self.components.is_empty()
    }

    pub fn len(&self) -> usize {
        self.components.len()
    }

    /// Checks that the budget accounts for every kind a decision declared it needs.
    ///
    /// Accounting for a kind does not mean the uncertainty is small. A component that says
    /// "distribution shift: this case is outside the fitted range" is accounted for; the
    /// absence of any distribution-shift component is what this refuses.
    pub fn accounts_for(
        &self,
        subject: &str,
        required: &[UncertaintyKind],
    ) -> Result<(), UncertaintyError> {
        for kind in required {
            if !self.components.contains_key(kind) {
                return Err(UncertaintyError::UnaccountedKind {
                    subject: subject.to_string(),
                    kind: *kind,
                });
            }
        }
        Ok(())
    }

    /// Combines two components of the same kind, and refuses across kinds.
    ///
    /// The refusal is the feature. Callers reach for a single number at exactly this point, so
    /// this is where 25.12's first invariant has to be a compile-visible API rather than a
    /// paragraph in a doc comment. Within a kind, only intervals and standard errors combine,
    /// and they combine conservatively: interval union, and the larger standard error. Neither
    /// is a statistically optimal pooling rule, and neither pretends to be — a correct rule
    /// needs a correlation structure that this IR does not carry.
    pub fn pool(
        left: &UncertaintyComponent,
        right: &UncertaintyComponent,
    ) -> Result<UncertaintyComponent, UncertaintyError> {
        if left.kind != right.kind {
            return Err(UncertaintyError::CrossKindPooling {
                left: left.kind,
                right: right.kind,
            });
        }
        let representation = match (&left.representation, &right.representation) {
            (
                Representation::Interval {
                    lower: left_lower,
                    upper: left_upper,
                    coverage: left_coverage,
                },
                Representation::Interval {
                    lower: right_lower,
                    upper: right_upper,
                    coverage: right_coverage,
                },
            ) => Representation::Interval {
                lower: left_lower.min(*right_lower),
                upper: left_upper.max(*right_upper),
                coverage: left_coverage.min(*right_coverage),
            },
            (
                Representation::StandardError { value: left_value },
                Representation::StandardError { value: right_value },
            ) => Representation::StandardError {
                value: left_value.max(*right_value),
            },
            (left_form, right_form) => {
                return Err(UncertaintyError::RepresentationsNotCombinable {
                    kind: left.kind,
                    left: form_name(left_form).to_string(),
                    right: form_name(right_form).to_string(),
                })
            }
        };
        Ok(UncertaintyComponent {
            kind: left.kind,
            representation,
            support: left.support.clone(),
            rationale: format!("pooled: {} | {}", left.rationale, right.rationale),
        })
    }
}

/// One bin of a reliability diagram.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalibrationBin {
    pub predicted: f64,
    pub observed: f64,
    pub count: usize,
}

/// A reliability curve together with the scope it was fitted in.
///
/// The scope is not metadata. A model calibrated on 3T scans from one vendor is not calibrated
/// on 1.5T scans from another, and the only defensible statement is the one restricted to
/// where the fit was done.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CalibrationCurve {
    pub label: String,
    pub fitted_in: ScopeKey,
    pub bins: Vec<CalibrationBin>,
}

impl CalibrationCurve {
    /// Whether this curve may be applied in `query`.
    ///
    /// Uses the scope refinement order from [`bioprism_scope`]: the query must be at least as
    /// narrow as the fitting scope. Scopes that constrain disjoint dimensions are
    /// *incomparable*, not compatible, and this returns an error for them too — which is the
    /// right answer, because "we have no idea whether the calibration holds here" is not
    /// "it holds".
    pub fn applies_in(&self, query: &ScopeKey) -> Result<(), UncertaintyError> {
        if query.refines(&self.fitted_in) {
            Ok(())
        } else {
            Err(UncertaintyError::CalibrationOutOfContext {
                curve_scope: describe_scope(&self.fitted_in),
                query_scope: describe_scope(query),
            })
        }
    }

    /// Mean absolute gap between predicted and observed frequency, weighted by bin count.
    ///
    /// This is a summary of *this curve*, not a summary of uncertainty. It says nothing about
    /// any other kind in the budget and is not combinable with one.
    pub fn expected_calibration_error(&self) -> Option<f64> {
        let total: usize = self.bins.iter().map(|bin| bin.count).sum();
        if total == 0 {
            return None;
        }
        let weighted: f64 = self
            .bins
            .iter()
            .map(|bin| (bin.predicted - bin.observed).abs() * bin.count as f64)
            .sum();
        Some(weighted / total as f64)
    }
}

fn form_name(representation: &Representation) -> &'static str {
    match representation {
        Representation::Interval { .. } => "an interval",
        Representation::Categorical { .. } => "a categorical distribution",
        Representation::StandardError { .. } => "a standard error",
        Representation::Panel { .. } => "a reviewer panel",
        Representation::MappingAmbiguity { .. } => "a mapping ambiguity",
        Representation::ShiftDiagnostic { .. } => "a shift diagnostic",
        Representation::Ungradable { .. } => "an ungradable marker",
    }
}

fn describe_scope(scope: &ScopeKey) -> String {
    if scope.is_empty() {
        return "unscoped".to_string();
    }
    scope
        .iter()
        .map(|(dimension, value)| format!("{dimension}={}", value.describe()))
        .collect::<Vec<_>>()
        .join(",")
}
