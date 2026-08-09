//! Research CI — blueprint 34.20.
//!
//! 34.20 asks to *"continuously check scientific claims, data lineage, benchmark regressions, and
//! architecture changes in research repositories"* and lists six capabilities. Its metrics are
//! about a pipeline (PR defect detection, false alert rate, CI cost) but its capabilities are the
//! only place in section 34 where the blueprint states things a result must be *true of*. That
//! makes it the one module here whose detailed design is unambiguously a set of predicates, so
//! this module is those predicates as functions and nothing else.
//!
//! There is no workflow file, no YAML, no runner, no scheduler and no reporter. A check is
//! [`Check::run`] over a [`ResultUnderReview`]; wiring it to a pull request is a deployment's job
//! and would be untestable here.
//!
//! # Three outcomes, and undetermined is not pass
//!
//! [`CheckOutcome`] has [`CheckOutcome::Pass`], [`CheckOutcome::Fail`] and
//! [`CheckOutcome::Undetermined`]. The third is the whole reason the type is not a `bool`: a check
//! that had nothing to look at has not been satisfied, and a CI system that reports it green will
//! be trusted for exactly as long as it takes somebody to delete the inputs.
//! [`CiReport::publishability`] therefore blocks on undetermined checks and lists them separately
//! from failures, because the two have different remedies — a failure means fix the result, an
//! undetermined means produce the evidence.
//!
//! # One check fails on absence rather than going undetermined
//!
//! [`Check::NonClaimDeclared`] fails when a result declares nothing it does not establish. That is
//! deliberate and it is the only asymmetry in the module: for every other check, no observation
//! means nobody looked; for this one, no observation *is* the defect. `bioprism-hub` already
//! requires a submission to carry a [`NonClaim`](bioprism_hub::NonClaim), and this is the same rule
//! applied one layer earlier, before the thing is submitted at all.
//!
//! # Figure reproduction is digest equality, deliberately
//!
//! 34.20 wants "figure reproduction" and says nothing about tolerance. A tolerance for a
//! floating-point plot is a real need and inventing one here would silently define what counts as
//! the same figure. [`Check::FigureReproduces`] compares content digests, so a figure that differs
//! in the last bit fails. A deployment that needs tolerance should render to a canonical form
//! first and hash that — which puts the tolerance where somebody can read it.
//!
//! # Not implemented
//!
//! No repository access, no git, no diffing, no PR annotation, no cost model, no false-alert rate.
//! No claim extraction: [`Observation::Claim`] is a claim somebody already identified, because
//! finding claims in prose is a language problem and a wrong answer would be a CI system that
//! silently checks the wrong sentences.

use crate::card::ProvenanceRung;
use crate::connector::Egress;
use crate::error::CiError;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// A fact CI observed about the artifact under review.
///
/// An open list of facts rather than a fixed struct with eight fields, because the absence of a
/// kind of observation is itself meaningful: a result with no [`Observation::Figure`] has not
/// passed figure reproduction, it has not been asked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "observation", rename_all = "snake_case")]
pub enum Observation {
    /// A claim the result makes, and the immutable result object it resolves to.
    ///
    /// `None` is section 34's trust requirement violated: "every rendered score resolves to
    /// immutable result objects", and a claim that resolves to nothing is the failure that rule
    /// exists to catch.
    Claim {
        id: String,
        resolves_to: Option<ContentHash>,
    },
    /// A named cohort split and the member ids in it.
    Split {
        name: String,
        members: BTreeSet<String>,
    },
    /// A published figure, its declared digest, and the digest of a recomputation if one was run.
    Figure {
        name: String,
        declared: ContentHash,
        recomputed: Option<ContentHash>,
    },
    /// A decision cell in the regression suite.
    Cell {
        id: String,
        previously_passed: bool,
        passes_now: Option<bool>,
    },
    /// A dependency of the environment the result was produced in.
    Dependency { name: String, pinned: Option<bool> },
    /// An export the result performed, and what the connector permitted.
    EgressEvent {
        connector: String,
        permitted: Egress,
        requested: Egress,
    },
    /// A statement of what the result does not establish.
    NonClaim { statement: String },
    /// A world the result was produced against, and the provenance rung its card declared.
    WorldReference {
        world: String,
        rung: Option<ProvenanceRung>,
    },
}

/// Everything CI observed, plus the name of what it looked at.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct ResultUnderReview {
    pub subject: String,
    pub observations: Vec<Observation>,
}

impl ResultUnderReview {
    pub fn of(subject: impl Into<String>) -> ResultUnderReview {
        ResultUnderReview {
            subject: subject.into(),
            observations: Vec::new(),
        }
    }

    pub fn observing(mut self, observation: Observation) -> ResultUnderReview {
        self.observations.push(observation);
        self
    }
}

/// The outcome of one check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum CheckOutcome {
    Pass,
    Fail { why: String },
    /// Nothing to check. Not a pass, and blocks publication just as a failure does — with a
    /// different remedy.
    Undetermined { why: String },
}

impl CheckOutcome {
    pub fn is_pass(&self) -> bool {
        matches!(self, CheckOutcome::Pass)
    }

    fn fail(why: impl Into<String>) -> CheckOutcome {
        CheckOutcome::Fail { why: why.into() }
    }

    fn undetermined(why: impl Into<String>) -> CheckOutcome {
        CheckOutcome::Undetermined { why: why.into() }
    }
}

/// The checks 34.20 names, one variant each.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Check {
    /// 34.20's "claim-to-result drift".
    ClaimResolvesToResult,
    /// 34.20's "cohort and split invariants". Only the invariant that can be checked without a
    /// cohort model: two named splits must not share a member.
    CohortSplitsAreDisjoint,
    /// 34.20's "figure reproduction", as digest equality.
    FigureReproduces,
    /// 34.20's "Decision Cell regression suite".
    DecisionCellRegression,
    /// 34.20's "dependency and environment changes".
    EnvironmentPinned,
    /// 34.20's "safety and data-policy checks", restricted to the half that is decidable from a
    /// record: an export that exceeded what its connector permitted.
    DataPolicyRespected,
    /// Not in 34.20's list, and included because every other module in section 34 asks for it: a
    /// result must say what it does not establish.
    NonClaimDeclared,
    /// The consequence of [`crate::card`]: a result produced against a world whose card declares no
    /// provenance rung cannot be interpreted.
    ProvenanceRungDeclared,
}

impl Check {
    pub const ALL: [Check; 8] = [
        Check::ClaimResolvesToResult,
        Check::CohortSplitsAreDisjoint,
        Check::FigureReproduces,
        Check::DecisionCellRegression,
        Check::EnvironmentPinned,
        Check::DataPolicyRespected,
        Check::NonClaimDeclared,
        Check::ProvenanceRungDeclared,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Check::ClaimResolvesToResult => "claim resolves to result",
            Check::CohortSplitsAreDisjoint => "cohort splits are disjoint",
            Check::FigureReproduces => "figure reproduces",
            Check::DecisionCellRegression => "decision cell regression",
            Check::EnvironmentPinned => "environment pinned",
            Check::DataPolicyRespected => "data policy respected",
            Check::NonClaimDeclared => "non-claim declared",
            Check::ProvenanceRungDeclared => "provenance rung declared",
        }
    }

    /// Run this check. Pure, total, and deterministic in the order of `result.observations`.
    pub fn run(self, result: &ResultUnderReview) -> CheckOutcome {
        match self {
            Check::ClaimResolvesToResult => claims_resolve(result),
            Check::CohortSplitsAreDisjoint => splits_disjoint(result),
            Check::FigureReproduces => figures_reproduce(result),
            Check::DecisionCellRegression => cells_still_pass(result),
            Check::EnvironmentPinned => environment_pinned(result),
            Check::DataPolicyRespected => data_policy_respected(result),
            Check::NonClaimDeclared => non_claim_declared(result),
            Check::ProvenanceRungDeclared => provenance_rung_declared(result),
        }
    }
}

impl fmt::Display for Check {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

fn claims_resolve(result: &ResultUnderReview) -> CheckOutcome {
    let mut seen = false;
    for observation in &result.observations {
        if let Observation::Claim { id, resolves_to } = observation {
            seen = true;
            if resolves_to.is_none() {
                return CheckOutcome::fail(format!("claim '{id}' resolves to no result object"));
            }
        }
    }
    if seen {
        CheckOutcome::Pass
    } else {
        CheckOutcome::undetermined("no claims were identified in the result")
    }
}

fn splits_disjoint(result: &ResultUnderReview) -> CheckOutcome {
    let splits: BTreeMap<&str, &BTreeSet<String>> = result
        .observations
        .iter()
        .filter_map(|o| match o {
            Observation::Split { name, members } => Some((name.as_str(), members)),
            _ => None,
        })
        .collect();
    if splits.len() < 2 {
        return CheckOutcome::undetermined(
            "fewer than two named splits were observed, so disjointness is not decidable",
        );
    }
    let names: Vec<&&str> = splits.keys().collect();
    for (i, left) in names.iter().enumerate() {
        for right in names.iter().skip(i + 1) {
            let shared = splits[**left].intersection(splits[**right]).next();
            if let Some(member) = shared {
                return CheckOutcome::fail(format!(
                    "splits '{left}' and '{right}' both contain '{member}'"
                ));
            }
        }
    }
    CheckOutcome::Pass
}

fn figures_reproduce(result: &ResultUnderReview) -> CheckOutcome {
    let mut seen = false;
    for observation in &result.observations {
        if let Observation::Figure {
            name,
            declared,
            recomputed,
        } = observation
        {
            seen = true;
            match recomputed {
                None => {
                    return CheckOutcome::undetermined(format!(
                        "figure '{name}' was never recomputed"
                    ))
                }
                Some(actual) if actual != declared => {
                    return CheckOutcome::fail(format!(
                        "figure '{name}' recomputed to {} but was published as {}",
                        actual.as_str(),
                        declared.as_str()
                    ))
                }
                Some(_) => {}
            }
        }
    }
    if seen {
        CheckOutcome::Pass
    } else {
        CheckOutcome::undetermined("no figures were observed")
    }
}

fn cells_still_pass(result: &ResultUnderReview) -> CheckOutcome {
    let mut seen = false;
    for observation in &result.observations {
        if let Observation::Cell {
            id,
            previously_passed,
            passes_now,
        } = observation
        {
            seen = true;
            match passes_now {
                None => {
                    return CheckOutcome::undetermined(format!("decision cell '{id}' was not run"))
                }
                Some(false) if *previously_passed => {
                    return CheckOutcome::fail(format!("decision cell '{id}' regressed"))
                }
                Some(_) => {}
            }
        }
    }
    if seen {
        CheckOutcome::Pass
    } else {
        CheckOutcome::undetermined("no decision cells were observed")
    }
}

fn environment_pinned(result: &ResultUnderReview) -> CheckOutcome {
    let mut seen = false;
    for observation in &result.observations {
        if let Observation::Dependency { name, pinned } = observation {
            seen = true;
            match pinned {
                None => {
                    return CheckOutcome::undetermined(format!(
                        "dependency '{name}' was not inspected"
                    ))
                }
                Some(false) => {
                    return CheckOutcome::fail(format!(
                        "dependency '{name}' is not pinned to an exact version"
                    ))
                }
                Some(true) => {}
            }
        }
    }
    if seen {
        CheckOutcome::Pass
    } else {
        CheckOutcome::undetermined("no dependencies were observed")
    }
}

fn data_policy_respected(result: &ResultUnderReview) -> CheckOutcome {
    let mut seen = false;
    for observation in &result.observations {
        if let Observation::EgressEvent {
            connector,
            permitted,
            requested,
        } = observation
        {
            seen = true;
            if !permitted.permits(*requested) {
                return CheckOutcome::fail(format!(
                    "connector '{connector}' permits {permitted} but {requested} left it"
                ));
            }
        }
    }
    if seen {
        CheckOutcome::Pass
    } else {
        CheckOutcome::undetermined("no egress events were observed")
    }
}

fn non_claim_declared(result: &ResultUnderReview) -> CheckOutcome {
    let declared = result.observations.iter().any(|o| match o {
        Observation::NonClaim { statement } => !statement.trim().is_empty(),
        _ => false,
    });
    if declared {
        CheckOutcome::Pass
    } else {
        CheckOutcome::fail("the result states nothing that it does not establish")
    }
}

fn provenance_rung_declared(result: &ResultUnderReview) -> CheckOutcome {
    let mut seen = false;
    for observation in &result.observations {
        if let Observation::WorldReference { world, rung } = observation {
            seen = true;
            if rung.is_none() {
                return CheckOutcome::fail(format!(
                    "world '{world}' has no declared provenance rung"
                ));
            }
        }
    }
    if seen {
        CheckOutcome::Pass
    } else {
        CheckOutcome::undetermined("the result references no world")
    }
}

/// Whether a report clears the bar for publication.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "publishability", rename_all = "snake_case")]
pub enum Publishability {
    Publishable,
    /// Kept apart because the remedies differ: a failure means fix the result, an undetermined
    /// means produce the missing evidence.
    Blocked {
        failed: Vec<Check>,
        undetermined: Vec<Check>,
    },
}

/// The outcome of a suite.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CiReport {
    pub subject: String,
    pub outcomes: BTreeMap<Check, CheckOutcome>,
}

impl CiReport {
    /// Run a suite. Refuses an empty suite and a repeated check.
    pub fn run(
        subject: impl Into<String>,
        checks: impl IntoIterator<Item = Check>,
        result: &ResultUnderReview,
    ) -> Result<CiReport, CiError> {
        let mut outcomes = BTreeMap::new();
        let mut any = false;
        for check in checks {
            any = true;
            if outcomes.contains_key(&check) {
                return Err(CiError::DuplicateCheck {
                    check: check.to_string(),
                });
            }
            outcomes.insert(check, check.run(result));
        }
        if !any {
            return Err(CiError::EmptySuite);
        }
        Ok(CiReport {
            subject: subject.into(),
            outcomes,
        })
    }

    /// The full suite.
    pub fn full(subject: impl Into<String>, result: &ResultUnderReview) -> CiReport {
        CiReport::run(subject, Check::ALL, result).expect("Check::ALL is non-empty and distinct")
    }

    pub fn publishability(&self) -> Publishability {
        let failed: Vec<Check> = self
            .outcomes
            .iter()
            .filter(|(_, o)| matches!(o, CheckOutcome::Fail { .. }))
            .map(|(c, _)| *c)
            .collect();
        let undetermined: Vec<Check> = self
            .outcomes
            .iter()
            .filter(|(_, o)| matches!(o, CheckOutcome::Undetermined { .. }))
            .map(|(c, _)| *c)
            .collect();
        if failed.is_empty() && undetermined.is_empty() {
            Publishability::Publishable
        } else {
            Publishability::Blocked {
                failed,
                undetermined,
            }
        }
    }

    /// A stable, sorted rendering, for a log that a human reads.
    pub fn lines(&self) -> Vec<String> {
        self.outcomes
            .iter()
            .map(|(check, outcome)| match outcome {
                CheckOutcome::Pass => format!("pass          {check}"),
                CheckOutcome::Fail { why } => format!("FAIL          {check}: {why}"),
                CheckOutcome::Undetermined { why } => format!("undetermined  {check}: {why}"),
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hash(seed: &str) -> ContentHash {
        ContentHash::of_bytes(seed.as_bytes())
    }

    fn split(name: &str, members: &[&str]) -> Observation {
        Observation::Split {
            name: name.to_string(),
            members: members.iter().map(|m| m.to_string()).collect(),
        }
    }

    #[test]
    fn a_claim_that_resolves_to_nothing_fails_and_names_the_claim() {
        let result = ResultUnderReview::of("r").observing(Observation::Claim {
            id: "auc-improves".to_string(),
            resolves_to: None,
        });
        let outcome = Check::ClaimResolvesToResult.run(&result);
        assert!(matches!(outcome, CheckOutcome::Fail { ref why } if why.contains("auc-improves")));
    }

    #[test]
    fn a_result_with_no_identified_claims_is_undetermined_not_passing() {
        let result = ResultUnderReview::of("r");
        assert!(matches!(
            Check::ClaimResolvesToResult.run(&result),
            CheckOutcome::Undetermined { .. }
        ));
    }

    #[test]
    fn overlapping_splits_fail_and_name_the_shared_member() {
        let result = ResultUnderReview::of("r")
            .observing(split("train", &["p1", "p2"]))
            .observing(split("test", &["p2", "p3"]));
        let outcome = Check::CohortSplitsAreDisjoint.run(&result);
        assert!(matches!(outcome, CheckOutcome::Fail { ref why } if why.contains("p2")));
    }

    #[test]
    fn disjoint_splits_pass() {
        let result = ResultUnderReview::of("r")
            .observing(split("train", &["p1"]))
            .observing(split("test", &["p2"]));
        assert!(Check::CohortSplitsAreDisjoint.run(&result).is_pass());
    }

    #[test]
    fn one_split_alone_cannot_decide_disjointness() {
        let result = ResultUnderReview::of("r").observing(split("train", &["p1"]));
        assert!(matches!(
            Check::CohortSplitsAreDisjoint.run(&result),
            CheckOutcome::Undetermined { .. }
        ));
    }

    #[test]
    fn a_figure_that_was_never_recomputed_is_undetermined_not_reproduced() {
        let result = ResultUnderReview::of("r").observing(Observation::Figure {
            name: "fig2".to_string(),
            declared: hash("a"),
            recomputed: None,
        });
        assert!(matches!(
            Check::FigureReproduces.run(&result),
            CheckOutcome::Undetermined { .. }
        ));
    }

    #[test]
    fn a_figure_that_differs_by_one_bit_fails() {
        let result = ResultUnderReview::of("r").observing(Observation::Figure {
            name: "fig2".to_string(),
            declared: hash("a"),
            recomputed: Some(hash("b")),
        });
        assert!(matches!(
            Check::FigureReproduces.run(&result),
            CheckOutcome::Fail { .. }
        ));
    }

    #[test]
    fn a_previously_passing_cell_that_now_fails_is_a_regression() {
        let result = ResultUnderReview::of("r").observing(Observation::Cell {
            id: "cell-7".to_string(),
            previously_passed: true,
            passes_now: Some(false),
        });
        assert!(matches!(
            Check::DecisionCellRegression.run(&result),
            CheckOutcome::Fail { .. }
        ));
    }

    #[test]
    fn a_cell_that_never_passed_and_still_does_not_is_not_a_regression() {
        let result = ResultUnderReview::of("r").observing(Observation::Cell {
            id: "cell-7".to_string(),
            previously_passed: false,
            passes_now: Some(false),
        });
        assert!(Check::DecisionCellRegression.run(&result).is_pass());
    }

    #[test]
    fn an_unpinned_dependency_fails_and_names_it() {
        let result = ResultUnderReview::of("r").observing(Observation::Dependency {
            name: "serde".to_string(),
            pinned: Some(false),
        });
        assert!(matches!(
            Check::EnvironmentPinned.run(&result),
            CheckOutcome::Fail { ref why } if why.contains("serde")
        ));
    }

    #[test]
    fn a_dependency_nobody_inspected_is_undetermined() {
        let result = ResultUnderReview::of("r").observing(Observation::Dependency {
            name: "serde".to_string(),
            pinned: None,
        });
        assert!(matches!(
            Check::EnvironmentPinned.run(&result),
            CheckOutcome::Undetermined { .. }
        ));
    }

    #[test]
    fn an_export_that_exceeded_what_the_connector_permitted_fails() {
        let result = ResultUnderReview::of("r").observing(Observation::EgressEvent {
            connector: "site-a".to_string(),
            permitted: Egress::AggregateOnly,
            requested: Egress::RecordLevel,
        });
        assert!(matches!(
            Check::DataPolicyRespected.run(&result),
            CheckOutcome::Fail { .. }
        ));
    }

    #[test]
    fn a_result_that_declares_nothing_it_does_not_establish_fails_rather_than_going_undetermined() {
        let result = ResultUnderReview::of("r");
        assert!(matches!(
            Check::NonClaimDeclared.run(&result),
            CheckOutcome::Fail { .. }
        ));
    }

    #[test]
    fn a_blank_non_claim_does_not_discharge_the_check() {
        let result = ResultUnderReview::of("r").observing(Observation::NonClaim {
            statement: "   ".to_string(),
        });
        assert!(matches!(
            Check::NonClaimDeclared.run(&result),
            CheckOutcome::Fail { .. }
        ));
    }

    #[test]
    fn a_world_without_a_declared_rung_fails() {
        let result = ResultUnderReview::of("r").observing(Observation::WorldReference {
            world: "w1".to_string(),
            rung: None,
        });
        assert!(matches!(
            Check::ProvenanceRungDeclared.run(&result),
            CheckOutcome::Fail { .. }
        ));
    }

    #[test]
    fn a_world_with_a_declared_rung_passes() {
        let result = ResultUnderReview::of("r").observing(Observation::WorldReference {
            world: "w1".to_string(),
            rung: Some(ProvenanceRung::SemiSynthetic),
        });
        assert!(Check::ProvenanceRungDeclared.run(&result).is_pass());
    }

    #[test]
    fn an_undetermined_check_blocks_publication_and_is_listed_apart_from_failures() {
        let result = ResultUnderReview::of("r").observing(Observation::NonClaim {
            statement: "no patient-level validity".to_string(),
        });
        let report = CiReport::full("r", &result);
        match report.publishability() {
            Publishability::Blocked {
                failed,
                undetermined,
            } => {
                assert!(failed.is_empty(), "no check should have failed: {failed:?}");
                assert!(undetermined.contains(&Check::FigureReproduces));
                assert!(!undetermined.contains(&Check::NonClaimDeclared));
            }
            other => panic!("expected blocked, got {other:?}"),
        }
    }

    #[test]
    fn a_result_that_satisfies_every_check_is_publishable() {
        let result = ResultUnderReview::of("r")
            .observing(Observation::Claim {
                id: "c1".to_string(),
                resolves_to: Some(hash("res")),
            })
            .observing(split("train", &["p1"]))
            .observing(split("test", &["p2"]))
            .observing(Observation::Figure {
                name: "fig1".to_string(),
                declared: hash("f"),
                recomputed: Some(hash("f")),
            })
            .observing(Observation::Cell {
                id: "cell-1".to_string(),
                previously_passed: true,
                passes_now: Some(true),
            })
            .observing(Observation::Dependency {
                name: "serde".to_string(),
                pinned: Some(true),
            })
            .observing(Observation::EgressEvent {
                connector: "site-a".to_string(),
                permitted: Egress::AggregateOnly,
                requested: Egress::AggregateOnly,
            })
            .observing(Observation::NonClaim {
                statement: "establishes nothing about a patient".to_string(),
            })
            .observing(Observation::WorldReference {
                world: "w1".to_string(),
                rung: Some(ProvenanceRung::Observed),
            });
        assert_eq!(
            CiReport::full("r", &result).publishability(),
            Publishability::Publishable
        );
    }

    #[test]
    fn an_empty_suite_is_refused_because_it_would_pass_everything() {
        let result = ResultUnderReview::of("r");
        assert_eq!(
            CiReport::run("r", Vec::<Check>::new(), &result).unwrap_err(),
            CiError::EmptySuite
        );
    }

    #[test]
    fn a_repeated_check_is_refused() {
        let result = ResultUnderReview::of("r");
        assert!(matches!(
            CiReport::run(
                "r",
                [Check::NonClaimDeclared, Check::NonClaimDeclared],
                &result
            ),
            Err(CiError::DuplicateCheck { .. })
        ));
    }

    #[test]
    fn a_report_renders_deterministically_in_check_order() {
        let result = ResultUnderReview::of("r");
        let first = CiReport::full("r", &result).lines();
        let second = CiReport::full("r", &result).lines();
        assert_eq!(first, second);
        assert_eq!(first.len(), Check::ALL.len());
    }
}
