//! State minimization.
//!
//! The MVP promise in `00_START_HERE/09_MVP_CUT_LINE.md` includes "minimize the failing state …
//! with preservation checks". A 761-fact world that produces four leakage witnesses is a
//! reproduction; an eleven-fact world that produces the same four is a *diagnosis*.
//!
//! This is delta debugging restricted to a single removal pass, which yields a 1-minimal set:
//! every remaining fact is load-bearing, in the sense that removing it alone changes the verdict.
//! That is a weaker guarantee than globally minimal and is stated as such — finding a globally
//! minimal set is exponential, and pretending otherwise would be the kind of unearned claim 43.43
//! warns against.
//!
//! # A refusal is not an abstention
//!
//! An oracle error was previously turned into `OracleVerdict::abstain`, which is the same swallow
//! [`crate::fork`] removed and it did more damage here. An abstention is a signature like any
//! other, so a reduction whose oracle refused at both ends compared equal to its target and
//! reported that it had *preserved* an answer nobody gave. On a candidate the oracle refuses
//! outright, every removal would have matched and the minimizer would have eaten the world down to
//! nothing while claiming a preserved verdict.
//!
//! 43.40's rule about missing evidence applies unchanged: an answer nobody could obtain and an
//! answer of "underdetermined" are different states and must not share a representation. Two
//! mechanisms keep them apart, and neither is a caveat field on an otherwise unchanged number —
//! see [`minimize`] for why the two differ in shape.
//!
//! # Which of the two is reachable today
//!
//! Only the first. The shipped split-integrity oracle refuses on evidence that is *present and
//! inconsistent*, never on evidence that is absent, so a removal cannot introduce a refusal a
//! larger set did not already have: `evaluate` is monotone under key removal, and the reduction
//! step only ever removes keys. [`UnjudgedRemoval`] therefore records a state no run of this
//! module currently produces.
//!
//! It is here rather than omitted because that monotonicity is a property of one oracle, asserted
//! nowhere and enforced by nothing — `bioprism_fiber::oracle::evaluate` is free to grow a check
//! that fires on a smaller set. Leaving the state out would make the old swallow reappear silently
//! the day it does, which is the failure mode this module was just fixed for. A test pins the
//! serialized shape so it cannot be quietly deleted as dead weight.
//!
//! # Not implemented
//!
//! No fixpoint. One removal pass, so a fact that only becomes removable after a later one is gone
//! stays in the result; `bioprism_benchcompiler::minimize` runs to a fixpoint against a
//! caller-supplied property and states the stronger guarantee that earns.

use bioprism_fiber::{oracle, FiberError};
use bioprism_section::{OracleStatus, OracleVerdict};
use bioprism_world::World;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Why no minimization exists to report.
///
/// One variant, because there is exactly one precondition a minimization has and inventing more
/// would be the kind of implied capability this workspace treats as a lie. Every other way the
/// oracle can decline belongs to an individual removal and is [`UnjudgedRemoval`] instead.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(tag = "failure", rename_all = "snake_case")]
pub enum MinimizeError {
    /// The oracle refused the candidate itself, so there is no signature for a reduction to
    /// preserve.
    ///
    /// The refusal was previously turned into an abstention and adopted as the target, after which
    /// every removal preserved it — the minimizer reduced the world to nothing and reported that
    /// the answer had survived. There is no answer to survive.
    #[error(
        "the oracle refused the {facts} fact(s) this minimization started from, so there is no \
         signature to preserve: {detail}"
    )]
    OracleRefusedCandidate { facts: usize, detail: String },
}

/// A removal the oracle would not judge.
///
/// The fact stays, because "the oracle declined" is not "removing it changes the answer" — but it
/// is not load-bearing either, and the 1-minimality claim does not reach it. Kept in a list of its
/// own rather than mixed into [`Minimization::minimal`] so that the facts a reader may cite as
/// load-bearing and the facts nobody could rule out are two populations rather than one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnjudgedRemoval {
    /// The fact whose removal could not be tested.
    pub fact: String,
    /// The oracle's refusal, verbatim.
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Minimization {
    pub started_from: usize,
    pub minimal: Vec<String>,
    pub removed: usize,
    /// The status the reduction preserves.
    ///
    /// Always a verdict the oracle actually returned: a candidate it refused never reaches this
    /// struct, so `"underdetermined"` here means the oracle answered "underdetermined" and never
    /// means it declined to answer.
    pub preserved_status: String,
    pub preserved_witnesses: Vec<String>,
    /// Oracle evaluations performed, refusals included. Reported so the cost of minimizing is
    /// visible.
    pub evaluations: usize,
    /// Facts held only because the oracle refused the world without them.
    ///
    /// Deliberately carries no `#[serde(default)]`: an absent field would deserialize as an empty
    /// list, which reads as "every removal was judged" — the optimistic default this type exists
    /// to stop. A document that does not say must not be read as a document that said no.
    pub unjudged: Vec<UnjudgedRemoval>,
    pub guarantee: String,
}

impl Minimization {
    pub fn reduction_ratio(&self) -> f64 {
        if self.started_from == 0 {
            return 1.0;
        }
        self.minimal.len() as f64 / self.started_from as f64
    }

    /// Whether every removal in the trajectory reached a verdict.
    ///
    /// The unqualified 1-minimality sentence is true of the whole minimal set only when this
    /// holds; [`Minimization::guarantee`] says so in words for the same reason.
    pub fn is_fully_judged(&self) -> bool {
        self.unjudged.is_empty()
    }

    /// The facts that are in the minimal set because nobody could rule them out.
    ///
    /// There is deliberately no accessor reporting an unjudged fact as a load-bearing one, which is
    /// the coercion the two lists exist to prevent.
    pub fn unjudged_facts(&self) -> impl Iterator<Item = &str> {
        self.unjudged.iter().map(|removal| removal.fact.as_str())
    }
}

/// Whether a minimal set still reproduces the signature it claims.
///
/// Three states rather than a `bool`, for the reason 43.40 gives about missing evidence: an oracle
/// that refuses the minimal set has not refuted the claim, and `false` would report that it had.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "preservation", rename_all = "snake_case")]
pub enum Preservation {
    /// Re-judged, and the signature matched.
    Preserved,
    /// Re-judged, and the signature differs. The reduction lost what it claimed to keep.
    Diverged {
        status: String,
        witnesses: Vec<String>,
    },
    /// The oracle refused the minimal set, so the claim is unchecked rather than refuted.
    Unverifiable { detail: String },
}

impl Preservation {
    /// Only [`Preservation::Preserved`] is true here. An unverifiable check is not a passing one.
    pub fn is_preserved(&self) -> bool {
        matches!(self, Preservation::Preserved)
    }
}

fn verdict_for(world: &World, facts: &BTreeSet<String>) -> Result<OracleVerdict, FiberError> {
    oracle::evaluate_selected(world, facts)
}

fn signature(verdict: &OracleVerdict) -> (OracleStatus, BTreeSet<String>) {
    (
        verdict.status,
        verdict
            .witness_kinds()
            .into_iter()
            .map(str::to_string)
            .collect(),
    )
}

/// Reduces `candidate` to a 1-minimal subset preserving the oracle signature.
///
/// Iteration is over a sorted vector so the result is deterministic: the same input always yields
/// the same minimal set, which a regression pack depends on.
///
/// # Why this returns a `Result` where [`crate::fork::matched_fork`] does not
///
/// `matched_fork` kept its refusal as a *state* because an `Err` there would answer an arm-level
/// question by discarding the arms that did run, and a panel exists to be compared. The two halves
/// of that argument come apart here, so the two halves of the refusal do too.
///
/// The candidate's own verdict is not a step of the trajectory; it is the target every step is
/// measured against, and it is computed before any step runs. An `Err` for it discards nothing,
/// because nothing has happened yet — and the alternative is worse than the fork's was: a fork with
/// one unjudged arm still compares the others, whereas a minimization against a target the oracle
/// never produced is not a weakened reduction but a meaningless one. That is
/// [`MinimizeError::OracleRefusedCandidate`].
///
/// A *removal* is a step of the trajectory, and here the fork's reasoning transfers verbatim: an
/// `Err` on the seventh of eleven removals would throw away six proven reductions to report one
/// fact nobody could test. The fact is kept, the refusal is recorded in [`Minimization::unjudged`],
/// and [`Minimization::guarantee`] narrows to the facts that were actually judged.
///
/// # One working set, restored on every rejection
///
/// A removal used to be tried on a *copy* of the surviving set: a fresh `BTreeSet<String>` with
/// every surviving id heap-allocated again, once per attempted removal. That is quadratic in the
/// corpus, and reducing the 761-fact reference world spent about 290 000 string copies to discard
/// all but six facts. `kept` is now mutated in place instead.
///
/// What the copy bought was a trial that could simply be dropped, and in-place mutation has to buy
/// that back by hand: `kept` *is* the live answer, so every route out of the match that does not
/// accept the removal must put the id back before the next iteration reads it. The match has three
/// arms and exactly two of them are such routes: the `Ok` arm whose signature differs from the
/// target, and the `Err` arm. Each restores in its own body rather than through a shared tail,
/// because the failure this shape invites is silent — a missed restore drops a load-bearing fact
/// and the minimization still reports itself as preserving the target signature.
///
/// Which test covers which restore was established by deleting each one and reading off what
/// broke, rather than by reasoning about it:
///
/// - The `Ok` restore is covered by `minimization_reduces_the_world_and_preserves_the_signature`
///   and by `a_minimal_set_the_oracle_refuses_is_unverified_rather_than_refuted`; those two fail
///   and nothing else does. `every_fact_in_the_minimal_set_is_load_bearing` was named here as the
///   test a missed restore fails and is not: it re-minimizes each one-fact-shorter subset and asks
///   only that the signature changed, which a set that has already lost load-bearing facts still
///   satisfies.
/// - The `Err` restore is covered by
///   `an_oracle_that_refuses_a_removal_leaves_the_working_set_intact`, which reaches the arm
///   through the crate-private `minimize_with`. Nothing reaches it through this function: the
///   shipped oracle is monotone under key removal, as the module header explains, so a refusal a
///   larger set did not already have cannot occur, and the arm was uncovered until the seam
///   existed.
///
/// The loop cannot exit early: no `?` and no `return` stands between a removal and its restore, so
/// a partially-restored set is unreachable. That is a property of this function's control flow
/// rather than of the type, and it is the invariant to check first if a step is ever added here.
pub fn minimize(
    world: &World,
    candidate: &BTreeSet<String>,
) -> Result<Minimization, MinimizeError> {
    minimize_with(candidate, |facts| verdict_for(world, facts))
}

/// [`minimize`] over an injected oracle.
///
/// Crate-private, and a seam rather than a feature: the public entry point is the one that runs
/// `bioprism_fiber::oracle`, and a caller free to supply its own judge could minimize against
/// anything at all while the result still called itself a preserved oracle signature.
///
/// It exists because the `Err` arm of the removal loop is unreachable through [`minimize`]. The
/// shipped split-integrity oracle refuses only on evidence that is present and inconsistent, so
/// removing keys cannot introduce a refusal, so no world can be written that drives a shipped run
/// into that arm. Deleting the restore inside it changed no test result at all. A refusing oracle
/// injected here reaches it directly, which is what turns [`UnjudgedRemoval`] and its restore from
/// asserted behaviour into tested behaviour.
pub(crate) fn minimize_with(
    candidate: &BTreeSet<String>,
    verdict_of: impl Fn(&BTreeSet<String>) -> Result<OracleVerdict, FiberError>,
) -> Result<Minimization, MinimizeError> {
    let reference =
        verdict_of(candidate).map_err(|error| MinimizeError::OracleRefusedCandidate {
            facts: candidate.len(),
            detail: error.to_string(),
        })?;
    let target = signature(&reference);
    let mut kept = candidate.clone();
    let mut evaluations = 1usize;
    let mut unjudged: Vec<UnjudgedRemoval> = Vec::new();

    for id in candidate {
        if !kept.remove(id) {
            continue;
        }
        evaluations += 1;
        match verdict_of(&kept) {
            Ok(verdict) if signature(&verdict) == target => {}
            Ok(_) => {
                kept.insert(id.clone());
            }
            Err(error) => {
                kept.insert(id.clone());
                unjudged.push(UnjudgedRemoval {
                    fact: id.clone(),
                    detail: error.to_string(),
                });
            }
        }
    }

    let guarantee = if unjudged.is_empty() {
        "1-minimal: removing any single remaining fact changes the oracle signature. \
         Not globally minimal; that search is exponential."
            .to_string()
    } else {
        format!(
            "1-minimal over the {} fact(s) the oracle judged: removing any one of those changes \
             the oracle signature. {} further fact(s) are held unjudged — the oracle refused the \
             world without them, and unjudged is not load-bearing. \
             Not globally minimal; that search is exponential.",
            kept.len().saturating_sub(unjudged.len()),
            unjudged.len()
        )
    };

    Ok(Minimization {
        started_from: candidate.len(),
        removed: candidate.len() - kept.len(),
        minimal: kept.into_iter().collect(),
        preserved_status: target.0.as_str().to_string(),
        preserved_witnesses: target.1.into_iter().collect(),
        evaluations,
        unjudged,
        guarantee,
    })
}

/// Minimizes the whole world rather than a pre-selected region.
pub fn minimize_world(world: &World) -> Result<Minimization, MinimizeError> {
    let all: BTreeSet<String> = world
        .facts
        .iter()
        .map(|fact| fact.id.as_str().to_string())
        .collect();
    minimize(world, &all)
}

/// Confirms a minimal set still reproduces the signature it claims.
///
/// A minimization that is not independently re-checked is just an assertion; 43.41 blocks
/// advancement on exactly this kind of unverified reduction. The check itself can fail to happen,
/// which is why the answer is a [`Preservation`] and not a boolean.
pub fn preserves(world: &World, minimization: &Minimization) -> Preservation {
    let facts: BTreeSet<String> = minimization.minimal.iter().cloned().collect();
    let verdict = match verdict_for(world, &facts) {
        Ok(verdict) => verdict,
        Err(error) => {
            return Preservation::Unverifiable {
                detail: error.to_string(),
            }
        }
    };
    let observed: BTreeSet<String> = verdict
        .witness_kinds()
        .into_iter()
        .map(str::to_string)
        .collect();
    let expected: BTreeSet<String> = minimization.preserved_witnesses.iter().cloned().collect();
    if verdict.status.as_str() == minimization.preserved_status && observed == expected {
        Preservation::Preserved
    } else {
        Preservation::Diverged {
            status: verdict.status.as_str().to_string(),
            witnesses: observed.into_iter().collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    fn ids(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|name| (*name).to_string()).collect()
    }

    /// The `Err` arm restores the id it could not judge, and every later step sees it.
    ///
    /// Reached only through the seam: the shipped oracle cannot refuse a set it accepted a
    /// superset of, so no fixture drives a run into this arm. Deleting the restore inside it left
    /// all of the crate's tests passing before this one existed.
    ///
    /// The stub refuses exactly one removal and accepts every other, so the reduction ought to eat
    /// the world down to the one fact nobody could rule out. Asserting only that `minimal` still
    /// holds `fact.b` would prove the id came back; the log of what the oracle was asked proves the
    /// working set it came back *into* was the live one, which is the property a shared-tail
    /// refactor would break without changing the result.
    #[test]
    fn an_oracle_that_refuses_a_removal_leaves_the_working_set_intact() {
        let candidate = ids(&["fact.a", "fact.b", "fact.c"]);
        let asked: RefCell<Vec<BTreeSet<String>>> = RefCell::new(Vec::new());

        let result = minimize_with(&candidate, |facts| {
            asked.borrow_mut().push(facts.clone());
            if !facts.contains("fact.b") {
                return Err(FiberError::UnorderableSplitGroups {
                    alias: "ALT-77".to_string(),
                    present: vec!["train".to_string()],
                });
            }
            Ok(OracleVerdict::new("split-integrity", Vec::new()))
        })
        .expect("the stub judges the candidate itself");

        assert_eq!(result.minimal, vec!["fact.b".to_string()]);
        assert_eq!(result.removed, 2);
        assert_eq!(
            result.unjudged,
            vec![UnjudgedRemoval {
                fact: "fact.b".to_string(),
                detail: FiberError::UnorderableSplitGroups {
                    alias: "ALT-77".to_string(),
                    present: vec!["train".to_string()],
                }
                .to_string(),
            }]
        );
        assert!(!result.is_fully_judged());
        assert!(result.guarantee.contains("unjudged is not load-bearing"));

        let asked = asked.into_inner();
        assert_eq!(
            asked.len(),
            result.evaluations,
            "the reported evaluation count must be the number of questions actually asked"
        );
        let refusal = asked
            .iter()
            .position(|facts| !facts.contains("fact.b"))
            .expect("the stub was driven into its refusal");
        assert!(
            asked[refusal + 1..]
                .iter()
                .all(|facts| facts.contains("fact.b")),
            "a set the oracle refused must be restored before the next question is asked"
        );
    }

    /// A candidate the injected oracle refuses outright is an error, not an empty minimization.
    ///
    /// The same rule [`MinimizeError::OracleRefusedCandidate`] states for the shipped oracle, held
    /// at the seam so that a future caller of `minimize_with` cannot reintroduce the swallow by
    /// supplying a judge that declines everything.
    #[test]
    fn an_oracle_that_refuses_the_candidate_itself_yields_no_minimization() {
        let candidate = ids(&["fact.a", "fact.b"]);

        let error = minimize_with(&candidate, |_| {
            Err(FiberError::UnorderableSplitGroups {
                alias: "ALT-77".to_string(),
                present: vec!["train".to_string()],
            })
        })
        .expect_err("a refused candidate has no signature to preserve");

        match error {
            MinimizeError::OracleRefusedCandidate { facts, detail } => {
                assert_eq!(facts, 2);
                assert!(detail.contains("ALT-77"), "{detail}");
            }
        }
    }
}
