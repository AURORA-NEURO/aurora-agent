//! Anytime evaluation curves — blueprint 42.22.
//!
//! 42.22 asks to "display capability estimates, uncertainty, cost, safety coverage, and stopping
//! state as evaluations accumulate". The interesting failure is in the word *anytime*: a curve
//! read at 40% coverage looks exactly like a curve read at 100%, because a line on a chart has no
//! way to say which strata it never touched.
//!
//! The remedy here is to make incompleteness a **field of the value** rather than metadata beside
//! it. There is no `f64` accessor on a curve. Reading a number means constructing an
//! [`ObservedRate`], which carries `over_strata` and `of_strata` inside itself, prints them in its
//! own `Display`, and answers [`ObservedRate::is_over_all_strata`]. A caller can still ignore the
//! denominator, but not without holding it.
//!
//! Uncovered strata are then **named, not counted** — [`CurveFinding::UncoveredStratum`] carries
//! the stratum and how many eligible units it holds — and the lens's own
//! [`Coverage`](crate::Coverage) is the curve's coverage, so a truncated evaluation produces a
//! partial lens answer for the same reason and through the same type as a half-loaded view in
//! 42.30.
//!
//! # A plan is required
//!
//! [`AnytimeCurve::compile`] refuses an evaluation with no planned strata. Without a plan there
//! is no denominator, so every point is trivially "complete" and the module's entire content
//! evaporates. It also refuses a point in a stratum that was never planned, because then the
//! numerator is not drawn from the denominator being reported.
//!
//! # Not implemented
//!
//! **No uncertainty interval.** 42.22 lists uncertainty; computing an interval needs per-trial
//! outcomes retained past aggregation and a clustering unit, which `bioprism-atlas` documents as
//! belonging to the statistics layer for the same reason. **No stopping rule.** The lens records
//! the stopping state it is handed and checks it against coverage; deciding *when* to stop is a
//! sequential-analysis question 42.22 does not specify. **No cost model.**

use crate::error::LensError;
use crate::grammar::{
    Coverage, EvidenceRequirement, Lens, LensDeclaration, LensId, LensOutcome, PendingRegion,
    Refusal, RefusalReason,
};
use crate::nonvisual::{Cell, Witness};
use bioprism_scope::ScopeKey;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

/// A named slice of the evaluation population, with how many units it holds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stratum {
    pub name: String,
    pub eligible: usize,
}

impl Stratum {
    pub fn new(name: impl Into<String>, eligible: usize) -> Self {
        Stratum {
            name: name.into(),
            eligible,
        }
    }
}

/// Trials run in one stratum, and how many passed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CurvePoint {
    pub stratum: String,
    pub trials: usize,
    pub passes: usize,
}

impl CurvePoint {
    pub fn new(stratum: impl Into<String>, trials: usize, passes: usize) -> Self {
        CurvePoint {
            stratum: stratum.into(),
            trials,
            passes,
        }
    }
}

/// Why the evaluation is where it is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "stopping", rename_all = "snake_case")]
pub enum StoppingState {
    /// Still accumulating.
    Running,
    /// Halted because a budget ran out. Says nothing about whether the estimate is settled.
    StoppedAtBudget { budget: String },
    /// Halted because a declared criterion was met.
    StoppedAtCriterion { criterion: String },
}

impl StoppingState {
    pub fn as_str(&self) -> &'static str {
        match self {
            StoppingState::Running => "running",
            StoppingState::StoppedAtBudget { .. } => "stopped_at_budget",
            StoppingState::StoppedAtCriterion { .. } => "stopped_at_criterion",
        }
    }

    pub fn is_stopped(&self) -> bool {
        !matches!(self, StoppingState::Running)
    }
}

/// An evaluation in progress.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnytimeEvaluation {
    pub evaluation: String,
    /// Every stratum the evaluation intends to cover. The denominator.
    pub planned: Vec<Stratum>,
    pub points: Vec<CurvePoint>,
    pub stopping: StoppingState,
    /// The minimum number of trials a stratum needs before its contribution means anything.
    #[serde(default)]
    pub min_trials_per_stratum: usize,
}

impl AnytimeEvaluation {
    pub fn new(evaluation: impl Into<String>, planned: Vec<Stratum>) -> Self {
        AnytimeEvaluation {
            evaluation: evaluation.into(),
            planned,
            points: Vec::new(),
            stopping: StoppingState::Running,
            min_trials_per_stratum: 0,
        }
    }

    pub fn with_points(mut self, points: Vec<CurvePoint>) -> Self {
        self.points = points;
        self
    }

    pub fn stopping(mut self, stopping: StoppingState) -> Self {
        self.stopping = stopping;
        self
    }

    pub fn requiring(mut self, min_trials_per_stratum: usize) -> Self {
        self.min_trials_per_stratum = min_trials_per_stratum;
        self
    }
}

/// A pass rate that carries its own denominator of strata.
///
/// This type exists so that the incompleteness of an anytime estimate travels *inside* the number
/// rather than beside it. `Display` prints the coverage, so even a log line that formats one of
/// these cannot lose it, and there is no `From<ObservedRate> for f64`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct ObservedRate {
    rate: f64,
    trials: usize,
    over_strata: usize,
    of_strata: usize,
}

impl ObservedRate {
    /// The rate over the strata that were actually visited. Never the population rate unless
    /// [`ObservedRate::is_over_all_strata`] is true.
    pub fn rate(&self) -> f64 {
        self.rate
    }

    pub fn trials(&self) -> usize {
        self.trials
    }

    pub fn over_strata(&self) -> usize {
        self.over_strata
    }

    pub fn of_strata(&self) -> usize {
        self.of_strata
    }

    pub fn is_over_all_strata(&self) -> bool {
        self.over_strata == self.of_strata
    }
}

impl fmt::Display for ObservedRate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{:.4} over {} of {} strata ({} trials)",
            self.rate, self.over_strata, self.of_strata, self.trials
        )
    }
}

/// A validated anytime curve.
///
/// Private fields, one fallible constructor. There is no accessor that returns a bare rate: see
/// [`AnytimeCurve::observed_rate`], which returns an [`ObservedRate`].
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct AnytimeCurve {
    evaluation: String,
    planned: Vec<Stratum>,
    points: Vec<CurvePoint>,
    stopping: StoppingState,
}

impl AnytimeCurve {
    /// Validate an evaluation into a curve.
    ///
    /// Refuses an empty plan (no denominator) and any point whose stratum is not in the plan (a
    /// numerator drawn from outside its own denominator).
    pub fn compile(evaluation: &AnytimeEvaluation) -> Result<Self, LensError> {
        if evaluation.planned.is_empty() {
            return Err(LensError::NoPlannedStrata {
                evaluation: evaluation.evaluation.clone(),
            });
        }
        let planned_names: BTreeSet<&str> =
            evaluation.planned.iter().map(|s| s.name.as_str()).collect();
        for point in &evaluation.points {
            if !planned_names.contains(point.stratum.as_str()) {
                return Err(LensError::UnplannedStratum {
                    evaluation: evaluation.evaluation.clone(),
                    stratum: point.stratum.clone(),
                });
            }
        }
        Ok(AnytimeCurve {
            evaluation: evaluation.evaluation.clone(),
            planned: evaluation.planned.clone(),
            points: evaluation.points.clone(),
            stopping: evaluation.stopping.clone(),
        })
    }

    pub fn evaluation(&self) -> &str {
        &self.evaluation
    }

    pub fn stopping(&self) -> &StoppingState {
        &self.stopping
    }

    /// Strata with at least one trial.
    pub fn covered_strata(&self) -> BTreeSet<&str> {
        self.points
            .iter()
            .filter(|p| p.trials > 0)
            .map(|p| p.stratum.as_str())
            .collect()
    }

    /// Strata the plan named and the curve never reached.
    pub fn uncovered_strata(&self) -> Vec<&Stratum> {
        let covered = self.covered_strata();
        self.planned
            .iter()
            .filter(|s| !covered.contains(s.name.as_str()))
            .collect()
    }

    /// The pass rate, wrapped in its own denominator of strata.
    ///
    /// Zero trials yield a rate of zero with `trials == 0`, which a reader must interpret through
    /// [`ObservedRate::trials`] rather than as a measured failure. This is the one place the
    /// module is weaker than [`crate::missingness`]: a rate is a float and a float has no hole.
    /// The trial count beside it is what carries the distinction.
    pub fn observed_rate(&self) -> ObservedRate {
        let trials: usize = self.points.iter().map(|p| p.trials).sum();
        let passes: usize = self.points.iter().map(|p| p.passes).sum();
        let rate = if trials == 0 {
            0.0
        } else {
            passes as f64 / trials as f64
        };
        ObservedRate {
            rate,
            trials,
            over_strata: self.covered_strata().len(),
            of_strata: self.planned.len(),
        }
    }
}

/// What the anytime curve lens found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CurveFinding {
    /// A planned stratum with no trials. The named hole in the curve.
    UncoveredStratum { stratum: String, eligible: usize },
    /// A stratum with fewer trials than the evaluation itself declared necessary.
    UnderpoweredStratum {
        stratum: String,
        trials: usize,
        required: usize,
    },
    /// The evaluation stopped while planned strata remained untouched.
    StoppedBeforeCoverage {
        stopping: String,
        uncovered: Vec<String>,
    },
    /// A point reporting more trials than its stratum has eligible units. The curve and the plan
    /// disagree about the population.
    ImpossiblePoint {
        stratum: String,
        trials: usize,
        eligible: usize,
    },
    /// More passes than trials.
    ImpossiblePasses {
        stratum: String,
        trials: usize,
        passes: usize,
    },
}

impl Witness for CurveFinding {
    fn kind(&self) -> &'static str {
        match self {
            CurveFinding::UncoveredStratum { .. } => "uncovered_stratum",
            CurveFinding::UnderpoweredStratum { .. } => "underpowered_stratum",
            CurveFinding::StoppedBeforeCoverage { .. } => "stopped_before_coverage",
            CurveFinding::ImpossiblePoint { .. } => "impossible_point",
            CurveFinding::ImpossiblePasses { .. } => "impossible_passes",
        }
    }

    fn columns(&self) -> &'static [&'static str] {
        match self {
            CurveFinding::UncoveredStratum { .. } => &["stratum", "eligible", "trials"],
            CurveFinding::UnderpoweredStratum { .. } => &["stratum", "trials", "required"],
            CurveFinding::StoppedBeforeCoverage { .. } => &["stopping", "uncovered_strata"],
            CurveFinding::ImpossiblePoint { .. } => &["stratum", "trials", "eligible"],
            CurveFinding::ImpossiblePasses { .. } => &["stratum", "trials", "passes"],
        }
    }

    fn cells(&self) -> Vec<Cell> {
        match self {
            CurveFinding::UncoveredStratum { stratum, eligible } => vec![
                Cell::text(stratum.clone()),
                Cell::count(*eligible),
                Cell::count(0),
            ],
            CurveFinding::UnderpoweredStratum {
                stratum,
                trials,
                required,
            } => vec![
                Cell::text(stratum.clone()),
                Cell::count(*trials),
                Cell::count(*required),
            ],
            CurveFinding::StoppedBeforeCoverage {
                stopping,
                uncovered,
            } => vec![
                Cell::text(stopping.clone()),
                Cell::text(uncovered.join(", ")),
            ],
            CurveFinding::ImpossiblePoint {
                stratum,
                trials,
                eligible,
            } => vec![
                Cell::text(stratum.clone()),
                Cell::count(*trials),
                Cell::count(*eligible),
            ],
            CurveFinding::ImpossiblePasses {
                stratum,
                trials,
                passes,
            } => vec![
                Cell::text(stratum.clone()),
                Cell::count(*trials),
                Cell::count(*passes),
            ],
        }
    }

    fn sentence(&self) -> String {
        match self {
            CurveFinding::UncoveredStratum { stratum, eligible } => format!(
                "stratum `{stratum}` holds {eligible} eligible unit(s) and contributed no trial; \
                 the curve says nothing about it"
            ),
            CurveFinding::UnderpoweredStratum {
                stratum,
                trials,
                required,
            } => format!(
                "stratum `{stratum}` contributed {trials} trial(s) against the {required} this \
                 evaluation declared necessary"
            ),
            CurveFinding::StoppedBeforeCoverage {
                stopping,
                uncovered,
            } => format!(
                "the evaluation is {stopping} with {} planned stratum/strata never reached: {}",
                uncovered.len(),
                uncovered.join(", ")
            ),
            CurveFinding::ImpossiblePoint {
                stratum,
                trials,
                eligible,
            } => format!(
                "stratum `{stratum}` reports {trials} trial(s) over {eligible} eligible unit(s)"
            ),
            CurveFinding::ImpossiblePasses {
                stratum,
                trials,
                passes,
            } => format!("stratum `{stratum}` reports {passes} pass(es) from {trials} trial(s)"),
        }
    }
}

/// Blueprint 42.22.
#[derive(Debug, Clone, Copy, Default)]
pub struct AnytimeCurveLens;

impl AnytimeCurveLens {
    pub const ID: &'static str = "anytime_curve";
}

impl Lens for AnytimeCurveLens {
    type Evidence = AnytimeEvaluation;
    type Witness = CurveFinding;

    fn declaration(&self) -> LensDeclaration {
        LensDeclaration::new(
            LensId::new(Self::ID),
            "42.22",
            "what has this evaluation actually covered so far, and which planned strata has it \
             not reached?",
            vec![
                EvidenceRequirement::new(
                    "evaluation.planned",
                    "every stratum the evaluation intends to cover",
                ),
                EvidenceRequirement::new("evaluation.points", "trials and passes per stratum"),
                EvidenceRequirement::new("evaluation.stopping", "the stopping state"),
            ],
            Vec::new(),
            vec![RefusalReason::NoAnswerableFormulation],
        )
        .expect("42.22 declaration is well formed")
    }

    fn answer(
        &self,
        _scope: &ScopeKey,
        evaluation: &AnytimeEvaluation,
    ) -> LensOutcome<CurveFinding> {
        let curve = match AnytimeCurve::compile(evaluation) {
            Ok(curve) => curve,
            Err(error) => {
                return LensOutcome::Refused(Refusal::new(
                    RefusalReason::NoAnswerableFormulation,
                    error.to_string(),
                ))
            }
        };

        let mut findings = Vec::new();
        let uncovered = curve.uncovered_strata();
        for stratum in &uncovered {
            findings.push(CurveFinding::UncoveredStratum {
                stratum: stratum.name.clone(),
                eligible: stratum.eligible,
            });
        }

        for point in &evaluation.points {
            if point.passes > point.trials {
                findings.push(CurveFinding::ImpossiblePasses {
                    stratum: point.stratum.clone(),
                    trials: point.trials,
                    passes: point.passes,
                });
            }
            if let Some(planned) = evaluation.planned.iter().find(|s| s.name == point.stratum) {
                if point.trials > planned.eligible {
                    findings.push(CurveFinding::ImpossiblePoint {
                        stratum: point.stratum.clone(),
                        trials: point.trials,
                        eligible: planned.eligible,
                    });
                }
                if point.trials > 0 && point.trials < evaluation.min_trials_per_stratum {
                    findings.push(CurveFinding::UnderpoweredStratum {
                        stratum: point.stratum.clone(),
                        trials: point.trials,
                        required: evaluation.min_trials_per_stratum,
                    });
                }
            }
        }

        if evaluation.stopping.is_stopped() && !uncovered.is_empty() {
            findings.push(CurveFinding::StoppedBeforeCoverage {
                stopping: evaluation.stopping.as_str().to_string(),
                uncovered: uncovered.iter().map(|s| s.name.clone()).collect(),
            });
        }

        let examined = curve.covered_strata().len();
        let eligible = evaluation.planned.len();
        let coverage = if uncovered.is_empty() {
            Coverage::complete(AnytimeCurveLens::ID, examined, eligible)
        } else {
            Coverage::partial(
                AnytimeCurveLens::ID,
                examined,
                eligible,
                uncovered
                    .iter()
                    .map(|s| {
                        PendingRegion::new(
                            s.name.clone(),
                            format!("{} eligible unit(s), no trial yet", s.eligible),
                        )
                    })
                    .collect(),
            )
        };
        match coverage {
            Ok(coverage) => LensOutcome::Answered {
                witnesses: findings,
                coverage,
            },
            Err(error) => LensOutcome::Refused(Refusal::new(
                RefusalReason::NoAnswerableFormulation,
                error.to_string(),
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::grammar::{run, Completeness};

    fn planned() -> Vec<Stratum> {
        vec![
            Stratum::new("imaging", 100),
            Stratum::new("molecular", 80),
            Stratum::new("pathology", 60),
        ]
    }

    #[test]
    fn a_curve_with_no_plan_has_no_denominator_and_is_refused() {
        let evaluation = AnytimeEvaluation::new("eval-1", Vec::new());
        let err = AnytimeCurve::compile(&evaluation).unwrap_err();
        assert!(matches!(err, LensError::NoPlannedStrata { .. }));
        let report = run(&AnytimeCurveLens, &ScopeKey::new(), &evaluation).unwrap();
        assert_eq!(report.outcome().as_str(), "refused");
    }

    #[test]
    fn a_point_in_an_unplanned_stratum_is_refused_rather_than_absorbed() {
        let evaluation = AnytimeEvaluation::new("eval-1", planned())
            .with_points(vec![CurvePoint::new("genomics", 10, 8)]);
        let err = AnytimeCurve::compile(&evaluation).unwrap_err();
        assert!(matches!(err, LensError::UnplannedStratum { .. }));
    }

    #[test]
    fn a_truncated_curve_carries_its_incompleteness_inside_the_rate() {
        let evaluation = AnytimeEvaluation::new("eval-1", planned())
            .with_points(vec![CurvePoint::new("imaging", 40, 30)]);
        let curve = AnytimeCurve::compile(&evaluation).unwrap();
        let rate = curve.observed_rate();
        assert_eq!(rate.over_strata(), 1);
        assert_eq!(rate.of_strata(), 3);
        assert!(!rate.is_over_all_strata());
        assert!(rate.to_string().contains("1 of 3 strata"));
    }

    #[test]
    fn a_finished_curve_reports_a_rate_over_all_strata() {
        let evaluation = AnytimeEvaluation::new("eval-1", planned()).with_points(vec![
            CurvePoint::new("imaging", 40, 30),
            CurvePoint::new("molecular", 20, 10),
            CurvePoint::new("pathology", 10, 10),
        ]);
        let curve = AnytimeCurve::compile(&evaluation).unwrap();
        assert!(curve.observed_rate().is_over_all_strata());
        assert!(curve.uncovered_strata().is_empty());
    }

    #[test]
    fn an_uncovered_stratum_is_named_with_its_eligible_count_not_merely_counted() {
        let evaluation = AnytimeEvaluation::new("eval-1", planned())
            .with_points(vec![CurvePoint::new("imaging", 40, 30)]);
        let report = run(&AnytimeCurveLens, &ScopeKey::new(), &evaluation).unwrap();
        let rows: Vec<&str> = report
            .witnesses()
            .iter()
            .filter(|r| r.kind == "uncovered_stratum")
            .map(|r| r.sentence.as_str())
            .collect();
        assert_eq!(rows.len(), 2);
        assert!(rows
            .iter()
            .any(|s| s.contains("molecular") && s.contains("80")));
        assert!(rows
            .iter()
            .any(|s| s.contains("pathology") && s.contains("60")));
    }

    #[test]
    fn a_partial_curve_produces_a_partial_lens_answer() {
        let evaluation = AnytimeEvaluation::new("eval-1", planned())
            .with_points(vec![CurvePoint::new("imaging", 40, 30)]);
        let report = run(&AnytimeCurveLens, &ScopeKey::new(), &evaluation).unwrap();
        assert_eq!(
            report.completeness(),
            Completeness::Partial {
                examined: 1,
                eligible: 3
            }
        );
    }

    #[test]
    fn a_complete_curve_produces_a_complete_lens_answer_with_no_findings() {
        let evaluation = AnytimeEvaluation::new("eval-1", planned()).with_points(vec![
            CurvePoint::new("imaging", 40, 30),
            CurvePoint::new("molecular", 20, 10),
            CurvePoint::new("pathology", 10, 10),
        ]);
        let report = run(&AnytimeCurveLens, &ScopeKey::new(), &evaluation).unwrap();
        assert!(report.completeness().is_complete());
        assert!(report.witnesses().is_empty());
    }

    #[test]
    fn stopping_with_strata_untouched_is_itself_a_finding() {
        let evaluation = AnytimeEvaluation::new("eval-1", planned())
            .with_points(vec![CurvePoint::new("imaging", 40, 30)])
            .stopping(StoppingState::StoppedAtBudget {
                budget: "gpu-hours".into(),
            });
        let report = run(&AnytimeCurveLens, &ScopeKey::new(), &evaluation).unwrap();
        let row = report
            .witnesses()
            .iter()
            .find(|r| r.kind == "stopped_before_coverage")
            .expect("stopping-before-coverage reported");
        assert!(row.sentence.contains("molecular"));
        assert!(row.sentence.contains("pathology"));
    }

    #[test]
    fn a_stratum_below_its_declared_minimum_is_underpowered_not_covered_silently() {
        let evaluation = AnytimeEvaluation::new("eval-1", planned())
            .with_points(vec![
                CurvePoint::new("imaging", 40, 30),
                CurvePoint::new("molecular", 2, 2),
                CurvePoint::new("pathology", 30, 20),
            ])
            .requiring(10);
        let report = run(&AnytimeCurveLens, &ScopeKey::new(), &evaluation).unwrap();
        let row = report
            .witnesses()
            .iter()
            .find(|r| r.kind == "underpowered_stratum")
            .expect("underpowered stratum reported");
        assert!(row.sentence.contains("molecular"));
        assert!(row.sentence.contains("2 trial"));
    }

    #[test]
    fn more_trials_than_eligible_units_is_reported_as_impossible() {
        let evaluation = AnytimeEvaluation::new("eval-1", planned()).with_points(vec![
            CurvePoint::new("imaging", 400, 300),
            CurvePoint::new("molecular", 1, 1),
            CurvePoint::new("pathology", 1, 1),
        ]);
        let report = run(&AnytimeCurveLens, &ScopeKey::new(), &evaluation).unwrap();
        assert!(report
            .witnesses()
            .iter()
            .any(|r| r.kind == "impossible_point"));
    }

    #[test]
    fn more_passes_than_trials_is_reported_as_impossible() {
        let evaluation = AnytimeEvaluation::new("eval-1", planned()).with_points(vec![
            CurvePoint::new("imaging", 10, 30),
            CurvePoint::new("molecular", 1, 1),
            CurvePoint::new("pathology", 1, 1),
        ]);
        let report = run(&AnytimeCurveLens, &ScopeKey::new(), &evaluation).unwrap();
        assert!(report
            .witnesses()
            .iter()
            .any(|r| r.kind == "impossible_passes"));
    }

    #[test]
    fn a_zero_trial_rate_is_distinguishable_from_a_measured_zero_rate() {
        let untried = AnytimeCurve::compile(&AnytimeEvaluation::new("a", planned()))
            .unwrap()
            .observed_rate();
        let all_failed =
            AnytimeCurve::compile(&AnytimeEvaluation::new("b", planned()).with_points(vec![
                CurvePoint::new("imaging", 40, 0),
                CurvePoint::new("molecular", 40, 0),
                CurvePoint::new("pathology", 40, 0),
            ]))
            .unwrap()
            .observed_rate();
        assert_eq!(untried.rate(), all_failed.rate());
        assert_ne!(untried.trials(), all_failed.trials());
        assert_ne!(untried.to_string(), all_failed.to_string());
    }
}
