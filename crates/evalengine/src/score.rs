//! The score model: outcome and justification, deliberately not the same axis.
//!
//! Blueprint 07.05 asks for partial credit "derived from meaningful state progress, satisfied
//! subconstraints, or rubric levels — not arbitrary fractional assignment", and 07.01 lists the
//! failure this is aimed at: "a superficially successful run receives credit despite violating
//! the intended task semantics."
//!
//! # Why two axes
//!
//! A single `passed: bool` cannot express the most common way an agent evaluation lies to you: the
//! run reached the right answer for a reason the evidence does not support. Collapsing that into
//! `true` inflates the headline; collapsing it into `false` throws away a real result and makes the
//! benchmark look harder than it is. So [`Outcome`] and [`Justification`] are recorded separately
//! and the pair is mapped to a [`Conclusion`] in which "right answer, unsupported reason" is
//! [`Conclusion::UnsupportedPass`] — its own category, neither of the two lies.
//!
//! # How inflation is actually prevented
//!
//! Not by a magic number. [`Credit`] carries two fields that are aggregated separately:
//! `full_pass`, which only [`Conclusion::Pass`] ever sets, and `fraction`, which carries partial
//! progress. A pack's pass rate therefore cannot be raised by unsupported passes at all, whatever
//! the fraction says. The numeric ceilings in [`CreditPolicy`] are a second, weaker line of
//! defence, and they are declared policy constants rather than measurements — see that type.
//!
//! # Unknown is not zero
//!
//! [`RubricProgress::fraction`] returns `None` when nothing about the rubric is known, rather than
//! `0.0`. This follows the treatment in `bioprism-section`'s omission manifest: a group nobody
//! analysed is `Unknown`, never `Zero`. A rubric whose constraints were all unevaluated has an
//! unknown fraction; averaging it in as failure is a policy choice a caller may make explicitly
//! through [`CreditPolicy::unknown_credit`], and never happens silently.
//!
//! # Not implemented here
//!
//! Rubric *authoring*, rubric levels beyond satisfied/violated/unknown, and any notion of who
//! judged a constraint. This module takes a rubric's outcome as given; producing it is the job of
//! the evaluators in blueprint 07.02–07.04, which live outside this crate.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

use crate::error::EvalError;

/// What the run produced, judged against the task's intended end state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Outcome {
    /// The intended end state was reached.
    Correct,
    /// Some declared subgoals were reached and others were not.
    Partial,
    /// The intended end state was not reached.
    Incorrect,
    /// The agent declined. Abstention is a behaviour to measure, not a failure to average in.
    Abstained,
    /// The evaluator could not tell: artifact missing, evaluator errored, output redacted.
    Unknown,
}

/// Whether the run's stated reasoning is supported by the same evidence that decided the outcome.
///
/// The distinction between [`Justification::Absent`] and [`Justification::Unexamined`] is the one
/// that carries weight: absent means the run offered no reason and somebody checked, unexamined
/// means nobody looked. Only the first is a property of the run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Justification {
    /// The stated reasoning is entailed by the evidence.
    Supported,
    /// The stated reasoning is not entailed by the evidence, though nothing contradicts it.
    Unsupported,
    /// The evidence contradicts the stated reasoning. The run was right by accident and its
    /// account of why is wrong.
    Contradicted,
    /// The run offered no reasoning, and that was checked.
    Absent,
    /// Nobody examined the reasoning. Never counts as support.
    Unexamined,
}

impl Justification {
    /// Whether this justification can carry a full pass.
    pub fn supports_full_pass(self) -> bool {
        matches!(self, Justification::Supported)
    }

    /// Whether the reasoning was actually looked at.
    pub fn was_examined(self) -> bool {
        !matches!(self, Justification::Unexamined)
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Justification::Supported => "supported",
            Justification::Unsupported => "unsupported",
            Justification::Contradicted => "contradicted",
            Justification::Absent => "absent",
            Justification::Unexamined => "unexamined",
        }
    }
}

/// Whether one named subconstraint of a task was met.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Satisfaction {
    Satisfied,
    Violated,
    /// Not evaluated. Contributes to neither numerator nor denominator.
    Unknown,
}

/// One named, weighted subconstraint. Partial credit is only ever a sum over these.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Constraint {
    pub name: String,
    /// Relative importance. Integer so that a rubric's denominator is exact.
    pub weight: u32,
    pub satisfaction: Satisfaction,
}

impl Constraint {
    pub fn new(name: impl Into<String>, weight: u32, satisfaction: Satisfaction) -> Self {
        Constraint {
            name: name.into(),
            weight,
            satisfaction,
        }
    }
}

/// A set of named subconstraints. The only admissible source of a fractional score.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rubric {
    pub constraints: Vec<Constraint>,
}

impl Rubric {
    pub fn new(constraints: Vec<Constraint>) -> Result<Self, EvalError> {
        let mut seen = BTreeSet::new();
        for constraint in &constraints {
            if !seen.insert(constraint.name.as_str()) {
                return Err(EvalError::DuplicateConstraint {
                    name: constraint.name.clone(),
                });
            }
        }
        Ok(Rubric { constraints })
    }

    /// Weight totals by satisfaction class.
    pub fn progress(&self) -> RubricProgress {
        let mut progress = RubricProgress::default();
        for constraint in &self.constraints {
            match constraint.satisfaction {
                Satisfaction::Satisfied => progress.satisfied_weight += u64::from(constraint.weight),
                Satisfaction::Violated => progress.violated_weight += u64::from(constraint.weight),
                Satisfaction::Unknown => progress.unknown_weight += u64::from(constraint.weight),
            }
        }
        progress
    }

    pub fn names(&self) -> Vec<&str> {
        self.constraints.iter().map(|c| c.name.as_str()).collect()
    }
}

/// Weight accounting for a rubric, with the unknown share kept out of the denominator.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RubricProgress {
    pub satisfied_weight: u64,
    pub violated_weight: u64,
    pub unknown_weight: u64,
}

impl RubricProgress {
    /// Weight that was actually evaluated.
    pub fn known_weight(&self) -> u64 {
        self.satisfied_weight + self.violated_weight
    }

    pub fn total_weight(&self) -> u64 {
        self.known_weight() + self.unknown_weight
    }

    /// Satisfied share **of what was checked**.
    ///
    /// `None` when nothing was checked. That is the honest answer: a rubric nobody evaluated has
    /// an unknown fraction, not a fraction of zero.
    pub fn fraction(&self) -> Option<f64> {
        let known = self.known_weight();
        if known == 0 {
            return None;
        }
        Some(self.satisfied_weight as f64 / known as f64)
    }

    /// Share of the rubric's declared weight that was never evaluated. Reported alongside
    /// [`RubricProgress::fraction`] so a fraction computed over 5% of the rubric is visibly that.
    pub fn unknown_share(&self) -> Option<f64> {
        let total = self.total_weight();
        if total == 0 {
            return None;
        }
        Some(self.unknown_weight as f64 / total as f64)
    }

    pub fn is_fully_known(&self) -> bool {
        self.unknown_weight == 0 && self.total_weight() > 0
    }
}

/// A check whose failure removes success regardless of any other evidence.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VetoKind {
    Safety,
    Permission,
    DataLeakage,
    /// The run interfered with its own grader. Blueprint 07.10's headline case.
    GraderTampering,
    /// Schema, checksum or provenance integrity failed.
    Integrity,
}

impl VetoKind {
    pub fn as_str(self) -> &'static str {
        match self {
            VetoKind::Safety => "safety",
            VetoKind::Permission => "permission",
            VetoKind::DataLeakage => "data_leakage",
            VetoKind::GraderTampering => "grader_tampering",
            VetoKind::Integrity => "integrity",
        }
    }
}

/// An individually visible veto. Vetoes are never summed into a weighted average; a report lists
/// them, and any aggregate that would hide one refuses instead.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Veto {
    pub kind: VetoKind,
    pub detail: String,
    /// The check that raised it, kept so the veto can be re-run.
    pub evaluator: String,
}

impl Veto {
    pub fn new(kind: VetoKind, evaluator: impl Into<String>, detail: impl Into<String>) -> Self {
        Veto {
            kind,
            evaluator: evaluator.into(),
            detail: detail.into(),
        }
    }
}

/// What an evaluator concluded about a result.
///
/// Nine variants rather than a boolean, because each one implies a different action from whoever
/// reads the report. `Disputed` means resolve the evaluators; `Unknown` means fix the pipeline;
/// `UnsupportedPass` means the task or the oracle is under-specified; `Vetoed` means stop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Conclusion {
    /// Right outcome, supported reasoning, no veto. The only full pass.
    Pass,
    /// Right outcome, reasoning not supported by the evidence, or no reasoning offered.
    UnsupportedPass,
    /// Right outcome, reasoning contradicted by the evidence.
    ContradictedPass,
    /// Some declared subconstraints were met.
    PartialCredit,
    /// The intended end state was not reached and no subconstraint progress was recorded.
    Fail,
    /// A veto fired. Preserved separately from `Fail` so a safety stop is never read as a
    /// capability limit.
    Vetoed,
    /// Evaluators at the deciding tier disagreed. Not a tiebreak — a work item.
    Disputed,
    /// Right outcome, and nobody checked the reasoning.
    JustificationUnexamined,
    /// The evaluator could not reach a conclusion.
    Unknown,
    /// The agent declined to answer.
    Abstained,
}

impl Conclusion {
    /// Whether this is the unqualified pass. Exactly one variant returns `true`, and every
    /// pass-rate aggregate in this crate is defined over this predicate.
    pub fn is_full_pass(self) -> bool {
        matches!(self, Conclusion::Pass)
    }

    /// Whether the outcome was right, whatever the reasoning was.
    ///
    /// Deliberately separate from [`Conclusion::is_full_pass`]. A report that shows both numbers
    /// makes the gap between "got it right" and "got it right for a supportable reason" the first
    /// thing a reader sees, which is the point of the two-axis model.
    pub fn outcome_was_correct(self) -> bool {
        matches!(
            self,
            Conclusion::Pass
                | Conclusion::UnsupportedPass
                | Conclusion::ContradictedPass
                | Conclusion::JustificationUnexamined
        )
    }

    /// Whether this conclusion says nothing about capability, and so must be excluded from a
    /// denominator rather than counted as a failure.
    pub fn is_uninformative(self) -> bool {
        matches!(
            self,
            Conclusion::Unknown | Conclusion::Disputed | Conclusion::JustificationUnexamined
        )
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Conclusion::Pass => "pass",
            Conclusion::UnsupportedPass => "unsupported_pass",
            Conclusion::ContradictedPass => "contradicted_pass",
            Conclusion::PartialCredit => "partial_credit",
            Conclusion::Fail => "fail",
            Conclusion::Vetoed => "vetoed",
            Conclusion::Disputed => "disputed",
            Conclusion::JustificationUnexamined => "justification_unexamined",
            Conclusion::Unknown => "unknown",
            Conclusion::Abstained => "abstained",
        }
    }
}

/// How a numeric credit was arrived at, so that no fraction in a report is unexplained.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CreditBasis {
    /// A full pass: the value is 1.0 by definition of the conclusion, not by measurement.
    FullPass,
    /// Derived from satisfied rubric weight.
    Rubric {
        satisfied_weight: u64,
        known_weight: u64,
    },
    /// Ceiling applied because the justification did not support the outcome.
    Capped {
        raw: String,
        ceiling: String,
        reason: String,
    },
    /// No rubric and no full pass: the credit is genuinely unknown.
    NoBasis,
    /// A veto fired; the credit is zero by policy, not by measurement.
    Vetoed,
    /// A declared policy assigned this value to an unknown.
    DeclaredForUnknown { declared_by: String },
}

/// A credit, kept as two fields on purpose.
///
/// `full_pass` drives pass rates and `fraction` drives partial-credit means; they are reported
/// side by side and never collapsed into one number here. A run that got the right answer for the
/// wrong reason moves the second and cannot move the first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Credit {
    /// `None` means unknown. It does not mean zero, and it must not be averaged as zero unless a
    /// policy says so in [`CreditPolicy::unknown_credit`].
    pub fraction: Option<f64>,
    pub full_pass: bool,
    pub basis: CreditBasis,
}

impl Credit {
    /// The invariant this whole module exists to hold: only a full pass reaches 1.0.
    pub fn respects_full_pass_ceiling(&self) -> bool {
        match self.fraction {
            Some(value) if value >= 1.0 => self.full_pass,
            _ => true,
        }
    }
}

/// What to do with an unknown when a number is unavoidable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UnknownCredit {
    /// Leave it unknown and let the aggregate report it separately. The default.
    Leave,
    /// Count it as zero, on the record, attributed to whoever decided that.
    DeclaredZero { declared_by: String },
}

/// Declared ceilings for credit that is not a full pass.
///
/// The two numbers are **policy constants, not measurements**. Nothing discovers that an
/// unsupported pass is worth 0.5 of a supported one; a pack declares it. What is structural, and
/// what the tests check, is only that both are strictly below 1.0 and that
/// [`Credit::full_pass`] stays false. A pack that disagrees with the values overrides them; a pack
/// that sets one to 1.0 is rejected by [`CreditPolicy::validate`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CreditPolicy {
    pub unsupported_ceiling: f64,
    pub contradicted_ceiling: f64,
    pub unknown_credit: UnknownCredit,
}

impl Default for CreditPolicy {
    fn default() -> Self {
        CreditPolicy {
            unsupported_ceiling: 0.5,
            contradicted_ceiling: 0.25,
            unknown_credit: UnknownCredit::Leave,
        }
    }
}

impl CreditPolicy {
    /// Whether the ceilings still leave a full pass strictly better than anything else.
    pub fn validate(&self) -> bool {
        (0.0..1.0).contains(&self.unsupported_ceiling)
            && (0.0..1.0).contains(&self.contradicted_ceiling)
    }
}

/// One evaluator's reading of one result: the two axes, the rubric, and any vetoes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ResultScore {
    pub outcome: Outcome,
    pub justification: Justification,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rubric: Option<Rubric>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub vetoes: Vec<Veto>,
}

impl ResultScore {
    pub fn new(outcome: Outcome, justification: Justification) -> Self {
        ResultScore {
            outcome,
            justification,
            rubric: None,
            vetoes: Vec::new(),
        }
    }

    pub fn with_rubric(mut self, rubric: Rubric) -> Self {
        self.rubric = Some(rubric);
        self
    }

    pub fn with_veto(mut self, veto: Veto) -> Self {
        self.vetoes.push(veto);
        self
    }

    pub fn progress(&self) -> RubricProgress {
        self.rubric.as_ref().map(Rubric::progress).unwrap_or_default()
    }

    /// Map the two axes onto one conclusion.
    ///
    /// Vetoes are checked first and unconditionally: blueprint 07.01 lets safety, permission,
    /// leakage, tampering and integrity checks veto a success score, and a veto that could be
    /// outvoted by a rubric would not be a veto.
    pub fn conclusion(&self) -> Conclusion {
        if !self.vetoes.is_empty() {
            return Conclusion::Vetoed;
        }
        match self.outcome {
            Outcome::Unknown => Conclusion::Unknown,
            Outcome::Abstained => Conclusion::Abstained,
            Outcome::Correct => match self.justification {
                Justification::Supported => Conclusion::Pass,
                Justification::Unsupported | Justification::Absent => Conclusion::UnsupportedPass,
                Justification::Contradicted => Conclusion::ContradictedPass,
                Justification::Unexamined => Conclusion::JustificationUnexamined,
            },
            Outcome::Partial => Conclusion::PartialCredit,
            Outcome::Incorrect => match self.progress().satisfied_weight {
                0 => Conclusion::Fail,
                _ => Conclusion::PartialCredit,
            },
        }
    }

    /// The numeric credit, if one is defensible.
    pub fn credit(&self, policy: &CreditPolicy) -> Credit {
        credit_for(self.conclusion(), self.progress(), policy)
    }
}

/// Shared credit derivation, used both by [`ResultScore`] and by composed results in
/// [`crate::ladder`].
pub fn credit_for(
    conclusion: Conclusion,
    progress: RubricProgress,
    policy: &CreditPolicy,
) -> Credit {
    let rubric_basis = |progress: RubricProgress| CreditBasis::Rubric {
        satisfied_weight: progress.satisfied_weight,
        known_weight: progress.known_weight(),
    };

    match conclusion {
        Conclusion::Pass => Credit {
            fraction: Some(1.0),
            full_pass: true,
            basis: CreditBasis::FullPass,
        },
        Conclusion::UnsupportedPass => capped(progress, policy.unsupported_ceiling, UNSUPPORTED_REASON, rubric_basis),
        Conclusion::ContradictedPass => capped(progress, policy.contradicted_ceiling, CONTRADICTED_REASON, rubric_basis),
        Conclusion::PartialCredit => match progress.fraction() {
            Some(fraction) => Credit {
                fraction: Some(fraction.min(NEAR_FULL_CEILING)),
                full_pass: false,
                basis: rubric_basis(progress),
            },
            None => Credit {
                fraction: None,
                full_pass: false,
                basis: CreditBasis::NoBasis,
            },
        },
        Conclusion::Fail => Credit {
            fraction: Some(0.0),
            full_pass: false,
            basis: rubric_basis(progress),
        },
        Conclusion::Vetoed => Credit {
            fraction: Some(0.0),
            full_pass: false,
            basis: CreditBasis::Vetoed,
        },
        Conclusion::Abstained
        | Conclusion::Disputed
        | Conclusion::JustificationUnexamined
        | Conclusion::Unknown => match &policy.unknown_credit {
            UnknownCredit::Leave => Credit {
                fraction: None,
                full_pass: false,
                basis: CreditBasis::NoBasis,
            },
            UnknownCredit::DeclaredZero { declared_by } => Credit {
                fraction: Some(0.0),
                full_pass: false,
                basis: CreditBasis::DeclaredForUnknown {
                    declared_by: declared_by.clone(),
                },
            },
        },
    }
}

const UNSUPPORTED_REASON: &str =
    "outcome correct but the stated reasoning is not entailed by the evidence";
const CONTRADICTED_REASON: &str =
    "outcome correct but the evidence contradicts the stated reasoning";

/// The most a rubric-derived partial credit may reach.
///
/// A rubric can report every declared subconstraint satisfied while the task's end state was only
/// partially reached — the rubric is a proxy, not the goal. Capping just below 1.0 keeps a partial
/// result from tying a full pass on the fraction axis as well as on the pass axis.
const NEAR_FULL_CEILING: f64 = 0.99;

fn capped(
    progress: RubricProgress,
    ceiling: f64,
    reason: &'static str,
    rubric_basis: impl Fn(RubricProgress) -> CreditBasis,
) -> Credit {
    match progress.fraction() {
        Some(raw) if raw <= ceiling => Credit {
            fraction: Some(raw),
            full_pass: false,
            basis: rubric_basis(progress),
        },
        Some(raw) => Credit {
            fraction: Some(ceiling),
            full_pass: false,
            basis: CreditBasis::Capped {
                raw: format!("{raw:.4}"),
                ceiling: format!("{ceiling:.4}"),
                reason: reason.to_string(),
            },
        },
        None => Credit {
            fraction: Some(ceiling),
            full_pass: false,
            basis: CreditBasis::Capped {
                raw: "unknown".to_string(),
                ceiling: format!("{ceiling:.4}"),
                reason: reason.to_string(),
            },
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rubric(pairs: &[(&str, u32, Satisfaction)]) -> Rubric {
        Rubric::new(
            pairs
                .iter()
                .map(|(name, weight, sat)| Constraint::new(*name, *weight, *sat))
                .collect(),
        )
        .expect("distinct names")
    }

    #[test]
    fn a_correct_outcome_with_an_unsupported_justification_is_not_a_full_pass() {
        let score = ResultScore::new(Outcome::Correct, Justification::Unsupported);
        assert_eq!(score.conclusion(), Conclusion::UnsupportedPass);
        assert!(!score.conclusion().is_full_pass());
        assert!(score.conclusion().outcome_was_correct());
    }

    #[test]
    fn an_unsupported_pass_cannot_reach_full_credit_even_with_a_perfect_rubric() {
        let policy = CreditPolicy::default();
        let score = ResultScore::new(Outcome::Correct, Justification::Unsupported).with_rubric(
            rubric(&[
                ("plan", 1, Satisfaction::Satisfied),
                ("cite", 1, Satisfaction::Satisfied),
            ]),
        );
        let credit = score.credit(&policy);
        assert!(!credit.full_pass);
        assert_eq!(credit.fraction, Some(policy.unsupported_ceiling));
        assert!(credit.respects_full_pass_ceiling());
    }

    #[test]
    fn a_contradicted_pass_scores_below_an_unsupported_one() {
        let policy = CreditPolicy::default();
        let unsupported = ResultScore::new(Outcome::Correct, Justification::Unsupported)
            .with_rubric(rubric(&[("a", 1, Satisfaction::Satisfied)]))
            .credit(&policy);
        let contradicted = ResultScore::new(Outcome::Correct, Justification::Contradicted)
            .with_rubric(rubric(&[("a", 1, Satisfaction::Satisfied)]))
            .credit(&policy);
        assert!(contradicted.fraction < unsupported.fraction);
    }

    #[test]
    fn an_unexamined_justification_is_distinguished_from_an_absent_one() {
        let unexamined = ResultScore::new(Outcome::Correct, Justification::Unexamined);
        let absent = ResultScore::new(Outcome::Correct, Justification::Absent);
        assert_eq!(unexamined.conclusion(), Conclusion::JustificationUnexamined);
        assert_eq!(absent.conclusion(), Conclusion::UnsupportedPass);
        assert!(unexamined.conclusion().is_uninformative());
        assert!(!absent.conclusion().is_uninformative());
    }

    #[test]
    fn an_unevaluated_rubric_has_an_unknown_fraction_rather_than_zero() {
        let progress = rubric(&[
            ("a", 3, Satisfaction::Unknown),
            ("b", 1, Satisfaction::Unknown),
        ])
        .progress();
        assert_eq!(progress.fraction(), None);
        assert_eq!(progress.unknown_share(), Some(1.0));
        assert!(!progress.is_fully_known());
    }

    #[test]
    fn unknown_constraints_leave_the_partial_credit_denominator() {
        let progress = rubric(&[
            ("a", 1, Satisfaction::Satisfied),
            ("b", 1, Satisfaction::Violated),
            ("c", 8, Satisfaction::Unknown),
        ])
        .progress();
        assert_eq!(progress.fraction(), Some(0.5));
        assert_eq!(progress.unknown_share(), Some(0.8));
    }

    #[test]
    fn a_veto_overrides_a_correct_and_supported_outcome() {
        let score = ResultScore::new(Outcome::Correct, Justification::Supported)
            .with_rubric(rubric(&[("a", 1, Satisfaction::Satisfied)]))
            .with_veto(Veto::new(
                VetoKind::GraderTampering,
                "tamper-check",
                "test file rewritten mid-run",
            ));
        assert_eq!(score.conclusion(), Conclusion::Vetoed);
        assert!(!score.credit(&CreditPolicy::default()).full_pass);
    }

    #[test]
    fn a_rubric_with_a_repeated_constraint_name_is_rejected() {
        let err = Rubric::new(vec![
            Constraint::new("plan", 1, Satisfaction::Satisfied),
            Constraint::new("plan", 1, Satisfaction::Violated),
        ])
        .unwrap_err();
        assert_eq!(
            err,
            EvalError::DuplicateConstraint {
                name: "plan".to_string()
            }
        );
    }

    #[test]
    fn an_incorrect_outcome_with_subconstraint_progress_earns_partial_not_zero() {
        let score = ResultScore::new(Outcome::Incorrect, Justification::Supported).with_rubric(
            rubric(&[
                ("setup", 1, Satisfaction::Satisfied),
                ("finish", 3, Satisfaction::Violated),
            ]),
        );
        assert_eq!(score.conclusion(), Conclusion::PartialCredit);
        assert_eq!(score.credit(&CreditPolicy::default()).fraction, Some(0.25));
    }

    #[test]
    fn the_default_credit_policy_keeps_a_full_pass_strictly_best() {
        assert!(CreditPolicy::default().validate());
        let broken = CreditPolicy {
            unsupported_ceiling: 1.0,
            ..CreditPolicy::default()
        };
        assert!(!broken.validate());
    }

    #[test]
    fn unknown_is_only_counted_as_zero_when_a_named_policy_says_so() {
        let score = ResultScore::new(Outcome::Unknown, Justification::Unexamined);
        assert_eq!(score.credit(&CreditPolicy::default()).fraction, None);

        let declared = CreditPolicy {
            unknown_credit: UnknownCredit::DeclaredZero {
                declared_by: "release-gate-v3".to_string(),
            },
            ..CreditPolicy::default()
        };
        let credit = score.credit(&declared);
        assert_eq!(credit.fraction, Some(0.0));
        assert!(matches!(
            credit.basis,
            CreditBasis::DeclaredForUnknown { .. }
        ));
    }

    #[test]
    fn score_records_round_trip_through_json() {
        let score = ResultScore::new(Outcome::Partial, Justification::Supported)
            .with_rubric(rubric(&[("a", 2, Satisfaction::Satisfied)]));
        let text = serde_json::to_string(&score).expect("serialize");
        let back: ResultScore = serde_json::from_str(&text).expect("deserialize");
        assert_eq!(score, back);
    }
}
