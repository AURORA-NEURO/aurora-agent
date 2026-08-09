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

use bioprism_fiber::oracle;
use bioprism_section::{OracleStatus, OracleVerdict};
use bioprism_world::World;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Minimization {
    pub started_from: usize,
    pub minimal: Vec<String>,
    pub removed: usize,
    pub preserved_status: String,
    pub preserved_witnesses: Vec<String>,
    /// Oracle evaluations performed. Reported so the cost of minimizing is visible.
    pub evaluations: usize,
    pub guarantee: String,
}

impl Minimization {
    pub fn reduction_ratio(&self) -> f64 {
        if self.started_from == 0 {
            return 1.0;
        }
        self.minimal.len() as f64 / self.started_from as f64
    }
}

fn verdict_for(world: &World, facts: &BTreeSet<String>) -> OracleVerdict {
    let values: BTreeMap<String, Value> = facts
        .iter()
        .filter_map(|id| world.fact(id))
        .map(|fact| (fact.provides.as_str().to_string(), fact.value.clone()))
        .collect();
    oracle::evaluate(&values).unwrap_or_else(|_| OracleVerdict::abstain(oracle::ORACLE_KIND, vec![]))
}

fn signature(verdict: &OracleVerdict) -> (OracleStatus, BTreeSet<String>) {
    (
        verdict.status,
        verdict.witness_kinds().into_iter().map(str::to_string).collect(),
    )
}

/// Reduces `candidate` to a 1-minimal subset preserving the oracle signature.
///
/// Iteration is over a sorted vector so the result is deterministic: the same input always yields
/// the same minimal set, which a regression pack depends on.
pub fn minimize(world: &World, candidate: &BTreeSet<String>) -> Minimization {
    let target = signature(&verdict_for(world, candidate));
    let mut kept = candidate.clone();
    let mut evaluations = 1usize;

    let ordered: Vec<String> = candidate.iter().cloned().collect();
    for id in ordered {
        let mut attempt = kept.clone();
        if !attempt.remove(&id) {
            continue;
        }
        evaluations += 1;
        if signature(&verdict_for(world, &attempt)) == target {
            kept = attempt;
        }
    }

    Minimization {
        started_from: candidate.len(),
        removed: candidate.len() - kept.len(),
        minimal: kept.into_iter().collect(),
        preserved_status: target.0.as_str().to_string(),
        preserved_witnesses: target.1.into_iter().collect(),
        evaluations,
        guarantee: "1-minimal: removing any single remaining fact changes the oracle signature. \
                    Not globally minimal; that search is exponential."
            .to_string(),
    }
}

/// Minimizes the whole world rather than a pre-selected region.
pub fn minimize_world(world: &World) -> Minimization {
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
/// advancement on exactly this kind of unverified reduction.
pub fn preserves(world: &World, minimization: &Minimization) -> bool {
    let facts: BTreeSet<String> = minimization.minimal.iter().cloned().collect();
    let verdict = verdict_for(world, &facts);
    let observed: BTreeSet<String> = verdict
        .witness_kinds()
        .into_iter()
        .map(str::to_string)
        .collect();
    let expected: BTreeSet<String> = minimization.preserved_witnesses.iter().cloned().collect();
    verdict.status.as_str() == minimization.preserved_status && observed == expected
}
