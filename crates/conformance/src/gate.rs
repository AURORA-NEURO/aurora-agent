//! Release gates.
//!
//! Implements the gate of blueprint 43.40 — "this module can move from specification to reference
//! implementation only when its schema is versioned, deterministic fixtures exist, failure
//! behavior is typed, provenance is replayable, and an equal-engineering baseline demonstrates
//! whether the claimed optimization is real" — together with the parts of 40.44's release
//! checklist a conformance run can actually evidence.
//!
//! Two design commitments.
//!
//! **The decision is never a boolean.** [`assess`] returns [`ReleaseDecision::Blocked`] carrying
//! the specific gates that are unmet and why. A gate function that returns `false` transfers the
//! whole diagnostic burden to whoever reads the CI log, and in practice that means the gate gets
//! disabled rather than satisfied.
//!
//! **The gates are noncompensatory.** 43.40 says so for safety and protected-completeness, and
//! the structure here extends it to all of them: there is no score, no weighting and no way for a
//! strong result on one gate to offset a weak one on another. Every unmet gate blocks.
//!
//! What a passing decision does *not* mean: 40.44 also requires an SBOM, security scan,
//! signatures, a clean-machine demo, a rollback plan and a named release owner. None of those are
//! observable from a conformance report and none are checked here. This assesses the evidence a
//! suite run produces; it is one input to a release decision, not the decision.

use crate::case::Layer;
use crate::pyramid::PyramidBalance;
use crate::suite::SuiteReport;
use serde::{Deserialize, Serialize};
use std::fmt;

/// The comparator required by 43.40's last clause.
///
/// Declared by the caller and taken on trust: nothing in this crate re-runs the comparison or
/// checks that the baseline was competently engineered. The gate therefore establishes that a
/// baseline was *named*, not that the optimisation claim survived it — the distinction matters,
/// because "we have a baseline" is exactly the claim an under-engineered baseline supports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BaselineReference {
    pub name: String,
    /// How the comparator selects context — the thing that has to be equally engineered.
    pub method: String,
    /// Where the comparison result lives.
    pub evidence: String,
    /// Whether the comparator reached the same decision on the same inputs. A baseline that
    /// drops the decisive evidence is not a comparator, it is a smaller wrong answer.
    pub verdict_preserving: bool,
    pub notes: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReleaseGate {
    /// Golden fixtures exist and a golden-layer requirement passed against them (43.40, 40.33).
    DeterministicFixturesExist,
    /// No fixture drifted from its declared digest (40.31 failure mode "fixture drift").
    NoFixtureDrift,
    /// Some requirement established that a refusal carries a typed kind (43.40, 40.36).
    FailureBehaviourIsTyped,
    /// Some requirement recomputed a published digest and some requirement established that a
    /// certificate binds the artifact it describes (43.40, 43.26).
    ProvenanceIsReplayable,
    /// An equal-engineering comparator is declared (43.40).
    EqualEngineeringBaselineExists,
    /// Every `must` requirement passed (40.44 "all P0 quality gates green").
    RequiredRequirementsPass,
    /// No case was retried to green (40.31 invariant 4).
    NoFlakyRetryToGreen,
    /// The suite is a pyramid rather than a single layer (40.31).
    TestPyramidIsBalanced,
    /// The suite carries a content digest, so "passed suite 0.1.0" names one set of cases
    /// (40.32 invariant 2).
    SuiteIsVersionedByDigest,
}

impl ReleaseGate {
    pub const ALL: [ReleaseGate; 9] = [
        ReleaseGate::DeterministicFixturesExist,
        ReleaseGate::NoFixtureDrift,
        ReleaseGate::FailureBehaviourIsTyped,
        ReleaseGate::ProvenanceIsReplayable,
        ReleaseGate::EqualEngineeringBaselineExists,
        ReleaseGate::RequiredRequirementsPass,
        ReleaseGate::NoFlakyRetryToGreen,
        ReleaseGate::TestPyramidIsBalanced,
        ReleaseGate::SuiteIsVersionedByDigest,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            ReleaseGate::DeterministicFixturesExist => "deterministic_fixtures_exist",
            ReleaseGate::NoFixtureDrift => "no_fixture_drift",
            ReleaseGate::FailureBehaviourIsTyped => "failure_behaviour_is_typed",
            ReleaseGate::ProvenanceIsReplayable => "provenance_is_replayable",
            ReleaseGate::EqualEngineeringBaselineExists => "equal_engineering_baseline_exists",
            ReleaseGate::RequiredRequirementsPass => "required_requirements_pass",
            ReleaseGate::NoFlakyRetryToGreen => "no_flaky_retry_to_green",
            ReleaseGate::TestPyramidIsBalanced => "test_pyramid_is_balanced",
            ReleaseGate::SuiteIsVersionedByDigest => "suite_is_versioned_by_digest",
        }
    }

    /// The blueprint clause this gate enforces.
    pub fn source(self) -> &'static str {
        match self {
            ReleaseGate::DeterministicFixturesExist => "43.40 release gate; 40.33",
            ReleaseGate::NoFixtureDrift => "40.31 failure semantics",
            ReleaseGate::FailureBehaviourIsTyped => "43.40 release gate; 40.36",
            ReleaseGate::ProvenanceIsReplayable => "43.40 release gate; 43.26",
            ReleaseGate::EqualEngineeringBaselineExists => "43.40 release gate",
            ReleaseGate::RequiredRequirementsPass => "40.44 required checklist",
            ReleaseGate::NoFlakyRetryToGreen => "40.31 invariant 4",
            ReleaseGate::TestPyramidIsBalanced => "40.31",
            ReleaseGate::SuiteIsVersionedByDigest => "40.32 invariant 2",
        }
    }
}

impl fmt::Display for ReleaseGate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A gate that did not pass, with the reason and the evidence that shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnmetGate {
    pub gate: ReleaseGate,
    pub because: String,
    /// Concrete pointers — case ids, drift lines, imbalance findings — so a reader can act
    /// without re-running anything.
    pub evidence: Vec<String>,
}

impl fmt::Display for UnmetGate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} [{}]: {}",
            self.gate,
            self.gate.source(),
            self.because
        )?;
        for item in &self.evidence {
            write!(f, "\n    - {item}")?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "decision", rename_all = "snake_case")]
pub enum ReleaseDecision {
    Release {
        suite_id: String,
        suite_version: String,
        suite_digest: String,
        implementation: String,
        gates: Vec<ReleaseGate>,
    },
    Blocked {
        suite_id: String,
        suite_version: String,
        met: Vec<ReleaseGate>,
        unmet: Vec<UnmetGate>,
    },
}

impl ReleaseDecision {
    pub fn is_release(&self) -> bool {
        matches!(self, ReleaseDecision::Release { .. })
    }

    pub fn unmet(&self) -> &[UnmetGate] {
        match self {
            ReleaseDecision::Release { .. } => &[],
            ReleaseDecision::Blocked { unmet, .. } => unmet,
        }
    }

    pub fn blocks_on(&self, gate: ReleaseGate) -> bool {
        self.unmet().iter().any(|u| u.gate == gate)
    }

    pub fn explain(&self) -> String {
        match self {
            ReleaseDecision::Release {
                suite_id,
                suite_version,
                implementation,
                gates,
                ..
            } => format!(
                "release permitted for {implementation} against {suite_id} {suite_version}: {} gates met",
                gates.len()
            ),
            ReleaseDecision::Blocked {
                suite_id,
                suite_version,
                unmet,
                ..
            } => {
                let mut lines = vec![format!(
                    "release blocked for {suite_id} {suite_version}: {} gate(s) unmet",
                    unmet.len()
                )];
                lines.extend(unmet.iter().map(UnmetGate::to_string));
                lines.join("\n")
            }
        }
    }
}

/// Evaluates every gate against a conformance run.
///
/// Gates are evaluated independently and all failures are reported together, for the same reason
/// the runner does not stop at the first failing case: a release that is blocked on four gates
/// should not take four cycles to discover that.
pub fn assess(report: &SuiteReport) -> ReleaseDecision {
    let mut met = Vec::new();
    let mut unmet = Vec::new();

    for gate in ReleaseGate::ALL {
        match evaluate(gate, report) {
            None => met.push(gate),
            Some(reason) => unmet.push(reason),
        }
    }

    if unmet.is_empty() {
        ReleaseDecision::Release {
            suite_id: report.suite_id.clone(),
            suite_version: report.suite_version.clone(),
            suite_digest: report.suite_digest.clone(),
            implementation: format!(
                "{} {}",
                report.implementation.name, report.implementation.version
            ),
            gates: met,
        }
    } else {
        ReleaseDecision::Blocked {
            suite_id: report.suite_id.clone(),
            suite_version: report.suite_version.clone(),
            met,
            unmet,
        }
    }
}

fn evaluate(gate: ReleaseGate, report: &SuiteReport) -> Option<UnmetGate> {
    match gate {
        ReleaseGate::DeterministicFixturesExist => {
            if report.fixture_count == 0 {
                return Some(UnmetGate {
                    gate,
                    because: "the run loaded no fixtures, so nothing was compared against a frozen expected output".to_string(),
                    evidence: vec![format!("fixture manifest {}", report.fixture_manifest_id)],
                });
            }
            if !report.covers_layer(Layer::Golden) {
                return Some(UnmetGate {
                    gate,
                    because: "no golden-layer requirement passed, so no output is pinned to a frozen expectation".to_string(),
                    evidence: vec![format!(
                        "{} fixtures loaded, 0 passing golden cases",
                        report.fixture_count
                    )],
                });
            }
            if !report.covers("recompilation_is_byte_identical") {
                return Some(UnmetGate {
                    gate,
                    because: "no requirement established that recompiling the same inputs is byte-identical; a fixture that matches once is not evidence of determinism".to_string(),
                    evidence: vec!["expectation kind recompilation_is_byte_identical was never exercised by a passing case".to_string()],
                });
            }
            None
        }

        ReleaseGate::NoFixtureDrift => {
            if report.fixture_drift.is_empty() {
                None
            } else {
                Some(UnmetGate {
                    gate,
                    because: "at least one fixture no longer matches its declared digest; every verdict in this run was produced against unknown inputs".to_string(),
                    evidence: report
                        .fixture_drift
                        .iter()
                        .map(ToString::to_string)
                        .collect(),
                })
            }
        }

        ReleaseGate::FailureBehaviourIsTyped => {
            if report.covers("fails_with") {
                None
            } else {
                Some(UnmetGate {
                    gate,
                    because: "no passing requirement asserted a typed refusal, so nothing establishes that failures are classified rather than thrown".to_string(),
                    evidence: vec![
                        "expectation kind fails_with was never exercised by a passing case"
                            .to_string(),
                    ],
                })
            }
        }

        ReleaseGate::ProvenanceIsReplayable => {
            let mut missing = Vec::new();
            if !report.covers("embedded_digest_verifies") {
                missing.push(
                    "no passing requirement recomputed a published digest (embedded_digest_verifies)"
                        .to_string(),
                );
            }
            if !report.covers("digest_binds") {
                missing.push(
                    "no passing requirement checked that a certificate binds the artifact it describes (digest_binds)"
                        .to_string(),
                );
            }
            if missing.is_empty() {
                None
            } else {
                Some(UnmetGate {
                    gate,
                    because: "a receipt nobody can recompute is an assertion, not provenance"
                        .to_string(),
                    evidence: missing,
                })
            }
        }

        ReleaseGate::EqualEngineeringBaselineExists => match &report.baseline {
            None => Some(UnmetGate {
                gate,
                because: "no equal-engineering comparator is declared, so the run cannot show whether the claimed optimisation is real".to_string(),
                evidence: vec!["43.40: an equal-engineering baseline must demonstrate whether the claimed optimization is real".to_string()],
            }),
            Some(baseline) if !baseline.verdict_preserving => Some(UnmetGate {
                gate,
                because: format!(
                    "the declared baseline {:?} does not preserve the decision, so it is not a comparator",
                    baseline.name
                ),
                evidence: vec![format!("{} via {}", baseline.evidence, baseline.method)],
            }),
            Some(_) => None,
        },

        ReleaseGate::RequiredRequirementsPass => {
            let unmet_cases = report.unmet_requirements();
            if unmet_cases.is_empty() {
                None
            } else {
                Some(UnmetGate {
                    gate,
                    because: format!(
                        "{} mandatory requirement(s) did not pass",
                        unmet_cases.len()
                    ),
                    evidence: unmet_cases
                        .iter()
                        .map(|case| {
                            format!(
                                "{} ({}) is {}",
                                case.case_id,
                                case.enforces.join(", "),
                                case.outcome.as_str()
                            )
                        })
                        .collect(),
                })
            }
        }

        ReleaseGate::NoFlakyRetryToGreen => {
            if report.retried_to_green.is_empty() {
                None
            } else {
                Some(UnmetGate {
                    gate,
                    because: "cases were re-run after failing and then reported green; 40.31 forbids retrying a flaky test to green".to_string(),
                    evidence: report.retried_to_green.clone(),
                })
            }
        }

        ReleaseGate::TestPyramidIsBalanced => match report.pyramid().balance() {
            PyramidBalance::Balanced => None,
            PyramidBalance::Unbalanced { findings } => Some(UnmetGate {
                gate,
                because: "the suite is not distributed across the layers of 40.31, so a green run does not mean what it appears to".to_string(),
                evidence: findings.iter().map(ToString::to_string).collect(),
            }),
        },

        ReleaseGate::SuiteIsVersionedByDigest => {
            if report.suite_digest.len() == 64 {
                None
            } else {
                Some(UnmetGate {
                    gate,
                    because: "the report carries no suite content digest, so the version string is unverifiable".to_string(),
                    evidence: vec![format!("suite_digest {:?}", report.suite_digest)],
                })
            }
        }
    }
}
