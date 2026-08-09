//! A re-execution comparison oracle (31.03).
//!
//! 31.03's worked case is exact: "A notebook reruns but its reported confidence interval excludes
//! the recomputed estimate; the workflow passes execution and fails result consistency." That is
//! [`Recheck::IntervalCovers`]. The numeric-tolerance case is [`Recheck::Numeric`].
//!
//! # What this is not
//!
//! It does **not** reconstruct an environment or execute a workflow. 31.03's first required
//! function is "reconstruct environment and execute workflow", and 40.21 requires oracle code to
//! run isolated; neither is possible from a library with no process boundary, no container
//! runtime, and no filesystem contract. This oracle consumes a recomputation that some *other*
//! component performed and placed in the evidence, and compares it to what was reported.
//!
//! That limitation is why it sits at [`EvidenceTier::Execution`] rather than
//! [`EvidenceTier::Deterministic`]: the comparison is exact, but its inputs came from an execution
//! this crate did not witness. Calling it deterministic would be claiming to have watched a run
//! that happened somewhere else.
//!
//! Also not implemented: seed stability and platform sensitivity (31.03's "separate stochastic
//! variation from drift"). Distinguishing a tolerance failure caused by nondeterminism from one
//! caused by drift needs repeated runs, which needs the execution harness that is absent here.

use crate::error::OracleError;
use crate::evidence::Evidence;
use crate::judgement::{Confidence, Finding, Judgement, Position};
use crate::ladder::EvidenceTier;
use crate::manifest::{OracleId, OracleManifest, OracleRef, OracleVersion};
use crate::oracle::Oracle;
use crate::plane::Plane;
use crate::time::ValidityWindow;

/// One comparison between what was reported and what was recomputed.
#[derive(Debug, Clone, PartialEq)]
pub enum Recheck {
    /// A reported number must match its recomputation within a tolerance.
    Numeric {
        reported: String,
        recomputed: String,
        tolerance: f64,
    },
    /// A reported interval must contain the recomputed point estimate.
    IntervalCovers {
        estimate: String,
        low: String,
        high: String,
    },
}

impl Recheck {
    fn name(&self) -> String {
        match self {
            Recheck::Numeric {
                reported,
                recomputed,
                ..
            } => format!("numeric({reported} vs {recomputed})"),
            Recheck::IntervalCovers {
                estimate,
                low,
                high,
            } => format!("interval_covers({estimate} in [{low}, {high}])"),
        }
    }
}

/// Compares reported outputs against recomputations supplied in the evidence.
pub struct ReexecutionOracle {
    manifest: OracleManifest,
    rechecks: Vec<Recheck>,
}

impl ReexecutionOracle {
    /// Builds a re-execution oracle at [`EvidenceTier::Execution`], establishing only
    /// [`Plane::Analytical`].
    ///
    /// The manifest disclaims the biological plane explicitly, which is 31.00's thesis in one
    /// line: a pipeline that reproduces a number "can pass computational validity while failing
    /// biological validity".
    pub fn new(
        id: impl Into<String>,
        version: OracleVersion,
        validity: ValidityWindow,
    ) -> Result<Self, OracleError> {
        let manifest = OracleManifest::new(
            OracleRef::new(OracleId::parse(id)?, version),
            EvidenceTier::Execution,
            [Plane::Analytical],
            [],
            validity,
        )?
        .disclaiming_the_rest()
        .with_failure_mode(
            "compares a recomputation it did not perform; a recomputation produced by the same \
             defective code as the report will agree with it",
        )
        .with_failure_mode(
            "cannot separate stochastic variation from drift, because it sees one run",
        );

        Ok(ReexecutionOracle {
            manifest,
            rechecks: Vec::new(),
        })
    }

    pub fn check(mut self, recheck: Recheck) -> Self {
        self.rechecks.push(recheck);
        self
    }

    pub fn manifest_mut(&mut self) -> &mut OracleManifest {
        &mut self.manifest
    }

    fn evaluate_one(&self, evidence: &Evidence, recheck: &Recheck) -> Option<Finding> {
        match recheck {
            Recheck::Numeric {
                reported,
                recomputed,
                tolerance,
            } => {
                let (Some(left), Some(right)) =
                    (evidence.number(reported), evidence.number(recomputed))
                else {
                    return Some(Finding::NotApplicable {
                        check: recheck.name(),
                        reason: "the reported value or its recomputation is absent".to_string(),
                    });
                };
                if (left - right).abs() <= *tolerance {
                    None
                } else {
                    Some(Finding::NumericDivergence {
                        reported: reported.clone(),
                        recomputed: recomputed.clone(),
                        reported_value: left,
                        recomputed_value: right,
                        tolerance: *tolerance,
                    })
                }
            }
            Recheck::IntervalCovers {
                estimate,
                low,
                high,
            } => {
                let (Some(point), Some(lower), Some(upper)) = (
                    evidence.number(estimate),
                    evidence.number(low),
                    evidence.number(high),
                ) else {
                    return Some(Finding::NotApplicable {
                        check: recheck.name(),
                        reason: "the estimate or an interval bound is absent".to_string(),
                    });
                };
                if point >= lower && point <= upper {
                    None
                } else {
                    Some(Finding::PropertyViolated {
                        property: recheck.name(),
                        pointer: estimate.clone(),
                        detail: format!(
                            "the recomputed estimate {point} lies outside the reported interval \
                             [{lower}, {upper}]"
                        ),
                    })
                }
            }
        }
    }
}

impl Oracle for ReexecutionOracle {
    fn manifest(&self) -> &OracleManifest {
        &self.manifest
    }

    fn evaluate(&self, evidence: &Evidence) -> Result<Judgement, OracleError> {
        let findings: Vec<Finding> = self
            .rechecks
            .iter()
            .filter_map(|recheck| self.evaluate_one(evidence, recheck))
            .collect();

        let violated = findings.iter().any(Finding::is_violation);
        let position = if violated {
            Position::Contradicted
        } else if findings.len() == self.rechecks.len() {
            Position::NotEvaluable
        } else {
            Position::Supported
        };

        Ok(
            Judgement::from_manifest(&self.manifest, &evidence.at, position, Confidence::CERTAIN)
                .with_findings(findings)
                .with_rationale(format!(
                    "compared {} reported output(s) against supplied recomputations",
                    self.rechecks.len()
                )),
        )
    }
}
