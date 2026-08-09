//! Small worlds a soundness claim can be checked on exhaustively.
//!
//! A bound nobody checked against brute force is a number, not a guarantee. Checking against
//! brute force means enumerating a perturbation space, which means the worlds have to be small —
//! so they are generated here, deterministically, in four structural families chosen to exercise
//! the *refusals* as much as the successes.
//!
//! | Family | Structure | What it tests |
//! |---|---|---|
//! | [`Family::Chain`] | prior, row-stochastic transitions, one free variable at the far end | the contraction method's only supported class; bounds should be tight near the target and loose far from it |
//! | [`Family::Star`] | many arms into one free centre | chain detection must refuse on degree, and the structural bound must still hold |
//! | [`Family::Tree`] | a branching path to the free root | chain detection must refuse on branching even though a path to the target exists |
//! | [`Family::Triangle`] | three variables, three pairwise factors | chain detection must refuse on the cycle, where the coefficient argument genuinely does not generalise |
//! | [`Family::WeakCycle`] | a cycle of any length with near-uniform potentials | the class [`crate::gibbs`] accepts and [`crate::contraction`] refuses: cyclic, and weakly enough coupled that Dobrushin's uniqueness condition holds |
//!
//! The families are structural, not biological. They carry the all-positive random potentials the
//! soundness argument needs and nothing about them is evidence about anything. That is stated on
//! every generated region's assumption list, in the same spirit as
//! [`bioprism_backends::QueryRegion::with_uniform_tables`].
//!
//! Entries are drawn strictly inside `[0.05, 1.0)` rather than from `[0, 1)`. A zero entry sends
//! the removal ratio range to `[0, ·]` and the bound to the vacuous `1.0`, which is sound but
//! tests nothing; the zero-entry case is covered by its own fixture instead, so the generated
//! family measures tightness rather than repeatedly confirming that `1.0` is an upper bound.

use crate::error::InfluenceError;
use crate::rng::SplitMix64;
use bioprism_backends::{QueryRegion, RegionFactor};

const MIN_ENTRY: f64 = 0.05;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    Chain,
    Star,
    Tree,
    Triangle,
    WeakCycle,
}

impl Family {
    pub fn as_str(self) -> &'static str {
        match self {
            Family::Chain => "chain",
            Family::Star => "star",
            Family::Tree => "tree",
            Family::Triangle => "triangle",
            Family::WeakCycle => "weak_cycle",
        }
    }
}

/// A generated region's recipe. Two identical specs always produce identical regions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SmallWorldSpec {
    pub family: Family,
    /// Chain length, arm count, or branch depth depending on the family. Ignored by `Triangle`.
    pub size: usize,
    pub cardinality: usize,
    pub seed: u64,
}

impl SmallWorldSpec {
    pub fn label(&self) -> String {
        format!(
            "{}/{}x{}/seed{}",
            self.family.as_str(),
            self.size,
            self.cardinality,
            self.seed
        )
    }
}

fn rejected(source: bioprism_backends::RegionError) -> InfluenceError {
    InfluenceError::PerturbedRegionRejected {
        message: source.to_string(),
    }
}

fn positive_table(rng: &mut SplitMix64, entries: usize) -> Vec<f64> {
    (0..entries).map(|_| rng.between(MIN_ENTRY, 1.0)).collect()
}

/// A table whose every row over the last variable sums to one.
fn stochastic_table(rng: &mut SplitMix64, rows: usize, columns: usize) -> Vec<f64> {
    let mut table = Vec::with_capacity(rows * columns);
    for _ in 0..rows {
        let row = positive_table(rng, columns);
        let mass: f64 = row.iter().sum();
        table.extend(row.into_iter().map(|value| value / mass));
    }
    table
}

const ASSUMPTION: &str =
    "potentials are pseudo-random positives from a seeded SplitMix64; this region is a structural fixture and carries no evidential content";

/// Builds the region a spec names.
pub fn generate(spec: &SmallWorldSpec) -> Result<QueryRegion, InfluenceError> {
    match spec.family {
        Family::Chain => chain(spec),
        Family::Star => star(spec),
        Family::Tree => tree(spec),
        Family::Triangle => triangle(spec),
        Family::WeakCycle => weak_cycle(spec),
    }
}

/// The maximum relative deviation from one in a [`Family::WeakCycle`] potential.
///
/// Chosen so the Dobrushin interdependence matrix of the generated cycles has maximum row sum
/// comfortably below one, which is the hypothesis [`crate::gibbs`] checks and refuses on. A larger
/// coupling would generate regions the method declines, which the [`Family::Triangle`] fixtures
/// already supply; the point of this family is to exercise the accepted case.
pub const WEAK_CYCLE_COUPLING: f64 = 0.2;

/// A cycle whose factors barely couple their endpoints.
///
/// Cyclic, so [`crate::contraction::detect`] refuses it — the coefficient of a cycle is not the
/// product of its edges. Weakly coupled, so the comparison theorem of [`crate::gibbs`] applies and
/// its infinite series converges. Between them these two facts are the reason 43.11's widening is
/// load-bearing in this crate rather than decorative.
fn weak_cycle(spec: &SmallWorldSpec) -> Result<QueryRegion, InfluenceError> {
    let mut rng = SplitMix64::new(spec.seed);
    let length = spec.size.clamp(3, 5);
    let card = spec.cardinality.max(2);

    let names: Vec<String> = (0..length).map(|index| format!("c{index}")).collect();
    let mut builder = QueryRegion::builder(spec.label());
    for name in &names {
        builder = builder.observed_variable(name, card);
    }
    for index in 0..length {
        let table: Vec<f64> = (0..card * card)
            .map(|_| rng.between(1.0 - WEAK_CYCLE_COUPLING, 1.0 + WEAK_CYCLE_COUPLING))
            .collect();
        builder = builder.factor(RegionFactor::with_table(
            format!("f.c{index}"),
            vec![names[index].clone(), names[(index + 1) % length].clone()],
            table,
        ));
    }
    builder
        .free(names[0].clone())
        .assumption(ASSUMPTION)
        .build()
        .map_err(rejected)
}

fn chain(spec: &SmallWorldSpec) -> Result<QueryRegion, InfluenceError> {
    let mut rng = SplitMix64::new(spec.seed);
    let length = spec.size.max(1);
    let card = spec.cardinality.max(2);

    let names: Vec<String> = (0..=length).map(|index| format!("v{index}")).collect();
    let mut builder = QueryRegion::builder(spec.label());
    for name in &names {
        builder = builder.observed_variable(name, card);
    }
    let prior = stochastic_table(&mut rng, 1, card);
    builder = builder.factor(RegionFactor::with_table("f.prior", vec![names[0].clone()], prior));
    for index in 0..length {
        let table = stochastic_table(&mut rng, card, card);
        builder = builder.factor(RegionFactor::with_table(
            format!("f.t{index}"),
            vec![names[index].clone(), names[index + 1].clone()],
            table,
        ));
    }
    builder
        .free(names[length].clone())
        .assumption(ASSUMPTION)
        .build()
        .map_err(rejected)
}

fn star(spec: &SmallWorldSpec) -> Result<QueryRegion, InfluenceError> {
    let mut rng = SplitMix64::new(spec.seed);
    let arms = spec.size.max(1);
    let card = spec.cardinality.max(2);

    let mut builder = QueryRegion::builder(spec.label()).observed_variable("centre", card);
    for index in 0..arms {
        builder = builder.observed_variable(format!("arm{index}"), card);
    }
    for index in 0..arms {
        builder = builder.factor(RegionFactor::with_table(
            format!("f.prior{index}"),
            vec![format!("arm{index}")],
            positive_table(&mut rng, card),
        ));
        builder = builder.factor(RegionFactor::with_table(
            format!("f.arm{index}"),
            vec![format!("arm{index}"), "centre".to_string()],
            positive_table(&mut rng, card * card),
        ));
    }
    builder
        .free("centre")
        .assumption(ASSUMPTION)
        .build()
        .map_err(rejected)
}

/// A binary tree of depth `size`, rooted at the free variable.
///
/// Node `0` is the root; node `n`'s children are `2n + 1` and `2n + 2`. Leaves carry priors, and
/// every parent-child pair carries a factor.
fn tree(spec: &SmallWorldSpec) -> Result<QueryRegion, InfluenceError> {
    let mut rng = SplitMix64::new(spec.seed);
    let depth = spec.size.clamp(1, 3);
    let card = spec.cardinality.max(2);
    let nodes = (1usize << (depth + 1)) - 1;

    let mut builder = QueryRegion::builder(spec.label());
    for node in 0..nodes {
        builder = builder.observed_variable(format!("n{node}"), card);
    }
    for node in 1..nodes {
        let parent = (node - 1) / 2;
        builder = builder.factor(RegionFactor::with_table(
            format!("f.edge{node}"),
            vec![format!("n{node}"), format!("n{parent}")],
            positive_table(&mut rng, card * card),
        ));
    }
    let first_leaf = (1usize << depth) - 1;
    for node in first_leaf..nodes {
        builder = builder.factor(RegionFactor::with_table(
            format!("f.leaf{node}"),
            vec![format!("n{node}")],
            positive_table(&mut rng, card),
        ));
    }
    builder
        .free("n0")
        .assumption(ASSUMPTION)
        .build()
        .map_err(rejected)
}

fn triangle(spec: &SmallWorldSpec) -> Result<QueryRegion, InfluenceError> {
    let mut rng = SplitMix64::new(spec.seed);
    let card = spec.cardinality.max(2);
    QueryRegion::builder(spec.label())
        .observed_variable("a", card)
        .observed_variable("b", card)
        .observed_variable("c", card)
        .factor(RegionFactor::with_table(
            "f.ab",
            vec!["a".to_string(), "b".to_string()],
            positive_table(&mut rng, card * card),
        ))
        .factor(RegionFactor::with_table(
            "f.bc",
            vec!["b".to_string(), "c".to_string()],
            positive_table(&mut rng, card * card),
        ))
        .factor(RegionFactor::with_table(
            "f.ac",
            vec!["a".to_string(), "c".to_string()],
            positive_table(&mut rng, card * card),
        ))
        .free("c")
        .assumption(ASSUMPTION)
        .build()
        .map_err(rejected)
}

/// The generated family the soundness suite runs over.
///
/// Deterministic and modest: forty regions, every one small enough that the perturbation
/// space of a single factor can be enumerated to its vertices without sampling. Growing the family
/// is a matter of adding seeds; growing the *regions* is not, and the cap in
/// [`crate::bruteforce`] refuses rather than silently switching to a sample.
pub fn family() -> Vec<SmallWorldSpec> {
    let mut specs = Vec::new();
    for seed in [0x5EED_0001u64, 0x5EED_0002, 0x5EED_0003, 0x5EED_0004] {
        for cardinality in [2usize, 3] {
            specs.push(SmallWorldSpec {
                family: Family::Chain,
                size: 3,
                cardinality,
                seed,
            });
            specs.push(SmallWorldSpec {
                family: Family::Chain,
                size: 5,
                cardinality,
                seed: seed ^ 0x11,
            });
            specs.push(SmallWorldSpec {
                family: Family::Star,
                size: 3,
                cardinality,
                seed,
            });
            specs.push(SmallWorldSpec {
                family: Family::Tree,
                size: 2,
                cardinality,
                seed,
            });
            specs.push(SmallWorldSpec {
                family: Family::Triangle,
                size: 0,
                cardinality,
                seed,
            });
            specs.push(SmallWorldSpec {
                family: Family::WeakCycle,
                size: 3,
                cardinality,
                seed,
            });
            specs.push(SmallWorldSpec {
                family: Family::WeakCycle,
                size: 4,
                cardinality,
                seed: seed ^ 0x22,
            });
        }
    }
    specs
}
