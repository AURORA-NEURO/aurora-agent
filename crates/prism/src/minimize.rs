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
pub fn minimize(
    world: &World,
    candidate: &BTreeSet<String>,
) -> Result<Minimization, MinimizeError> {
    let reference =
        verdict_for(world, candidate).map_err(|error| MinimizeError::OracleRefusedCandidate {
            facts: candidate.len(),
            detail: error.to_string(),
        })?;
    let target = signature(&reference);
    let mut kept = candidate.clone();
    let mut evaluations = 1usize;
    let mut unjudged: Vec<UnjudgedRemoval> = Vec::new();

    let ordered: Vec<String> = candidate.iter().cloned().collect();
    for id in ordered {
        let mut attempt = kept.clone();
        if !attempt.remove(&id) {
            continue;
        }
        evaluations += 1;
        match verdict_for(world, &attempt) {
            Ok(verdict) if signature(&verdict) == target => kept = attempt,
            Ok(_) => {}
            Err(error) => unjudged.push(UnjudgedRemoval {
                fact: id,
                detail: error.to_string(),
            }),
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
