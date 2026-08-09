//! Procedural generation and mutation scheduling.
//!
//! Blueprint 35.09: "choose which validated transformations to instantiate rather than expanding
//! every combinatorial possibility." The combinatorial expansion is the thing to avoid, and not
//! because it is expensive — because it is *cheap*, and cheap expansion is what turns a benchmark
//! into a paraphrase pile.
//!
//! ## The schedule reports its own ceiling before anything is generated
//!
//! [`GenerationSchedule::class_ceiling`] counts the distinct `(parent decision, mutation family)`
//! pairs the schedule can possibly produce. A schedule of a million entries with a ceiling of four
//! hundred is visible as such *before* a single instance is generated, which is the cheapest
//! possible moment to find out. Coverage-first ordering means the budget buys new pairs before it
//! buys repeat parameterizations.
//!
//! ## Hidden seeds
//!
//! 35.09 requires a "hidden seed policy" and 35.11 requires "secret generator seeds": the seed for
//! a hidden-tier family must not travel with the release, or the hidden tier is regenerable by
//! anyone. [`HiddenSeed`] holds a private `u64`, serializes to its **digest**, and implements no
//! `Deserialize` — a seed cannot be reconstructed from a published artefact, which is the property
//! the policy actually needs. The digest still lets an auditor confirm afterwards that the seed
//! used was the seed committed to.
//!
//! ## Not implemented
//!
//! No difficulty targeting and no novelty model. 35.09 lists both; both need a measured difficulty
//! signal from executed runs, which is `bioprism-adaptive`'s data and not available at scheduling
//! time in this crate. Compatibility is an explicit per-parent set rather than an inferred matrix.

use bioprism_worldgen::rng::SplitMix64;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize, Serializer};
use std::collections::{BTreeMap, BTreeSet};

/// A generator seed that can be published as a commitment but never as a value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HiddenSeed(u64);

impl HiddenSeed {
    pub fn new(seed: u64) -> Self {
        HiddenSeed(seed)
    }

    /// The publishable commitment to this seed.
    pub fn digest(&self) -> String {
        ContentHash::of_bytes(&self.0.to_be_bytes()).as_str().to_string()
    }

    fn value(&self) -> u64 {
        self.0
    }
}

impl Serialize for HiddenSeed {
    /// Emits the digest, never the seed. There is no `Deserialize`, by design.
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.digest())
    }
}

/// How many instances a scheduling pass may commit to generating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct MutationBudget {
    pub instances: usize,
}

/// What a parent world admits: its decisions, and the families it is compatible with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ParentEligibility {
    pub parent: String,
    pub decisions: Vec<String>,
    /// 35.09's "mutation compatibility". A family that cannot preserve a decision's semantics on
    /// this parent is excluded here rather than rejected after generation.
    pub compatible_families: BTreeSet<String>,
}

/// One scheduled generation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScheduledMutation {
    pub parent: String,
    pub decision: String,
    pub family: String,
    /// Index of this parameterization within its `(decision, family)` pair. Anything above 0 adds
    /// instances without adding an equivalence class.
    pub parameterization: usize,
}

/// A generation plan, with the ceiling it can reach.
///
/// `Serialize` only. A schedule cannot round-trip through JSON because [`HiddenSeed`] does not
/// deserialize — which is the hidden-seed policy working as intended: a published schedule is an
/// audit artefact, not a recipe for regenerating the hidden tier.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GenerationSchedule {
    pub entries: Vec<ScheduledMutation>,
    /// Seed commitment. The seed itself is not here and cannot be.
    pub seed_commitment: HiddenSeed,
    /// Distinct `(decision, family)` pairs the schedule covers.
    pub covered_pairs: usize,
    /// Entries beyond the first for some pair — instance count without class count.
    pub repeat_parameterizations: usize,
}

impl GenerationSchedule {
    /// The most equivalence classes this schedule can possibly yield.
    pub fn class_ceiling(&self) -> usize {
        self.covered_pairs
    }

    /// Nominal entries over the class ceiling.
    pub fn projected_inflation(&self) -> f64 {
        if self.covered_pairs == 0 {
            0.0
        } else {
            self.entries.len() as f64 / self.covered_pairs as f64
        }
    }
}

/// Builds a coverage-first schedule under `budget`.
///
/// Pass one takes each `(parent, decision, family)` triple once, in an order deterministically
/// permuted by `seed` so that a truncated budget does not systematically favour whichever parent
/// sorts first. Pass two onward adds parameterizations, which raise the instance count and not the
/// ceiling — and the returned schedule says so.
pub fn schedule(
    parents: &[ParentEligibility],
    budget: MutationBudget,
    seed: HiddenSeed,
) -> GenerationSchedule {
    let mut triples: Vec<(String, String, String)> = Vec::new();
    for parent in parents {
        for decision in &parent.decisions {
            for family in &parent.compatible_families {
                triples.push((parent.parent.clone(), decision.clone(), family.clone()));
            }
        }
    }
    triples.sort();

    let mut rng = SplitMix64::new(seed.value());
    for index in (1..triples.len()).rev() {
        let swap = rng.below(index + 1);
        triples.swap(index, swap);
    }

    let mut entries = Vec::new();
    let mut used: BTreeMap<(String, String), usize> = BTreeMap::new();
    let mut repeat_parameterizations = 0usize;
    let mut round = 0usize;
    while entries.len() < budget.instances && !triples.is_empty() {
        for (parent, decision, family) in &triples {
            if entries.len() >= budget.instances {
                break;
            }
            let key = (decision.clone(), family.clone());
            let parameterization = *used.get(&key).unwrap_or(&0);
            if parameterization > 0 {
                repeat_parameterizations += 1;
            }
            used.insert(key, parameterization + 1);
            entries.push(ScheduledMutation {
                parent: parent.clone(),
                decision: decision.clone(),
                family: family.clone(),
                parameterization,
            });
        }
        round += 1;
        if round > budget.instances {
            break;
        }
    }

    GenerationSchedule {
        covered_pairs: used.len(),
        entries,
        seed_commitment: seed,
        repeat_parameterizations,
    }
}
