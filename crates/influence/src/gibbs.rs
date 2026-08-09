//! The loopy case: Dobrushin's comparison theorem, and the ascending chain it produces.
//!
//! [`crate::contraction`] handles a simple path and refuses everything else, and its refusal on
//! cycles is not laziness — the ergodic coefficient of a cycle is not the product of its edges, so
//! there is no drop-in generalisation of that argument. This module is the argument that does
//! generalise, and it is the reason 43.11's widening is not decoration here: the quantity it
//! computes is an infinite series whose partial sums form an ascending chain that no finite number
//! of joins reaches.
//!
//! ## The theorem
//!
//! Let `μ` be the Gibbs measure of a positive factor graph, with single-site conditionals
//! `γ_i(· | x_{-i}) ∝ ∏_{f ∋ i} φ_f`. The Dobrushin interdependence matrix is
//!
//! ```text
//!     C_ij  =  max { TV( γ_i(·|x), γ_i(·|y) ) : x, y agree off site j },      C_ii = 0
//! ```
//!
//! and for a second measure `ν` differing from `μ` in its specification,
//!
//! ```text
//!     b_i   =  max_x TV( γ_i(·|x), γ̃_i(·|x) ).
//! ```
//!
//! Under Dobrushin's uniqueness condition `‖C‖_∞ = max_i Σ_j C_ij < 1`, the comparison theorem
//! bounds the difference of expectations of any function by the oscillations of that function
//! against `D = Σ_{n ≥ 0} Cⁿ`:
//!
//! ```text
//!     |μ f − ν f|  ≤  Σ_{i,j} δ_j(f) · D_ji · b_i .
//! ```
//!
//! An indicator of a set depending only on site `v` has `δ_v ≤ 1` and `δ_j = 0` elsewhere, and
//! total variation is the supremum over such sets, so for the marginal at the query's free
//! variable
//!
//! ```text
//!     TV( μ_v , ν_v )  ≤  Σ_i D_vi · b_i  =  u_v ,   where   u = b + C·u .
//! ```
//!
//! Several free variables give `Σ_{v free} u_v`, since each coordinate of an indicator over the
//! joint free scope still has oscillation at most one. That is loose and it is sound.
//!
//! ## Two places this implementation is deliberately conservative
//!
//! **Which measure's `C`.** The theorem is stated for the interdependence matrix of one of the two
//! measures. `D = Σ Cⁿ` is entrywise monotone in `C`, so this module computes `C(μ)` and `C(ν)` and
//! uses their entrywise maximum, which dominates either and therefore dominates whichever the
//! statement needs. That costs precision and removes a question about which convention is in force.
//!
//! **What the perturbation vector measures.** For [`crate::Perturbation::Removal`] `b_i` is computed
//! exactly, by evaluating both conditionals at every neighbourhood configuration. For a stated
//! multiplicative range it is the lemma of [`crate::ratio`] applied per site: reweighting `φ_g`
//! inside `[lo, hi]` reweights `γ_i` as a function of `x_i` inside the same interval, so
//! `b_i ≤ (√hi − √lo)/(√hi + √lo)`. Sound, and looser than the removal case.
//!
//! ## The hypotheses, stated so they can be refused
//!
//! A region qualifies when, and only when:
//!
//! 1. it has at least one factor and at least one free variable;
//! 2. every factor carries a potential — checked through
//!    [`crate::domains::SupportDomain`], the domain whose whole job is this precondition;
//! 3. every entry of every potential is strictly positive, so the single-site conditionals exist
//!    everywhere;
//! 4. no site's neighbourhood needs more than [`MAX_CONDITIONAL_CONFIGURATIONS`] configurations to
//!    enumerate;
//! 5. `‖C‖_∞ < 1`.
//!
//! Anything else returns [`crate::UnknownReason::RegionOutsideMethodClass`] naming the clause that
//! failed, in the same style and for the same reason as [`crate::contraction::detect`]: a method
//! that approximated its way past a hypothesis would produce a number that is not a bound.

use crate::domains::ratio_interval::RatioInterval;
use crate::domains::support::{certainly_positive, Support};
use crate::error::{InfluenceError, UnknownReason};
use crate::measure::total_variation_of_rows;
use crate::perturbation::Perturbation;
use bioprism_backends::QueryRegion;
use std::collections::BTreeMap;

const METHOD: &str = "abstract_interpretation";
const HANDLES: &str = "a strictly positive factor graph of any shape, cycles included, whose Dobrushin interdependence matrix has maximum row sum below one";

/// The largest neighbourhood configuration space a single site's conditionals will be enumerated
/// over.
///
/// Refuses rather than sampling, for the reason [`crate::MAX_PERTURBATION_VERTICES`] gives: an
/// analysis that silently degrades to a sample is weakest exactly where it matters.
pub const MAX_CONDITIONAL_CONFIGURATIONS: usize = 4096;

fn outside(detail: impl Into<String>) -> UnknownReason {
    UnknownReason::RegionOutsideMethodClass {
        method: METHOD.to_string(),
        handles: HANDLES.to_string(),
        detail: detail.into(),
    }
}

/// A factor graph's potentials, either as declared or with one factor perturbed.
struct Specification<'a> {
    region: &'a QueryRegion,
    tables: Vec<Vec<f64>>,
}

impl<'a> Specification<'a> {
    /// The region's own potentials, refusing a factor that carries none or is not strictly
    /// positive.
    fn declared(region: &'a QueryRegion) -> Result<Self, UnknownReason> {
        let mut tables = Vec::with_capacity(region.factors().len());
        for factor in region.factors() {
            let Some(table) = factor.table() else {
                return Err(UnknownReason::NoFactorTable {
                    factor: factor.id().to_string(),
                });
            };
            if !certainly_positive(&Support::of_table(table)) {
                return Err(outside(format!(
                    "factor {:?} has a zero or non-finite entry; a single-site conditional of a factor graph with a forbidden assignment is not defined everywhere",
                    factor.id()
                )));
            }
            tables.push(table.to_vec());
        }
        Ok(Specification { region, tables })
    }

    /// The same potentials with every named factor replaced by the all-ones factor.
    fn with_factors_removed(&self, factor_ids: &[String]) -> Result<Self, InfluenceError> {
        let mut tables = self.tables.clone();
        for factor_id in factor_ids {
            let position = self
                .region
                .factors()
                .iter()
                .position(|factor| factor.id() == *factor_id)
                .ok_or_else(|| InfluenceError::UnknownFactor {
                    region: self.region.label().to_string(),
                    factor: factor_id.clone(),
                })?;
            tables[position] = vec![1.0; self.tables[position].len()];
        }
        Ok(Specification {
            region: self.region,
            tables,
        })
    }

    fn entry(&self, factor_position: usize, assignment: &BTreeMap<&str, usize>) -> f64 {
        let factor = &self.region.factors()[factor_position];
        let mut index = 0usize;
        for name in factor.scope() {
            let card = self
                .region
                .cardinality_of(name)
                .expect("a factor's scope variables are region variables");
            index = index * card + assignment[name.as_str()];
        }
        self.tables[factor_position][index]
    }

    /// `γ_site(· | assignment)`, normalised, or `None` when the conditional has no mass.
    fn conditional<'k>(
        &self,
        site: &'k str,
        touching: &[usize],
        assignment: &mut BTreeMap<&'k str, usize>,
        cardinality: usize,
    ) -> Option<Vec<f64>> {
        let mut row = Vec::with_capacity(cardinality);
        for value in 0..cardinality {
            assignment.insert(site, value);
            let mut product = 1.0f64;
            for position in touching {
                product *= self.entry(*position, assignment);
            }
            row.push(product);
        }
        let mass: f64 = row.iter().sum();
        if !mass.is_finite() || mass <= 0.0 {
            return None;
        }
        Some(row.into_iter().map(|value| value / mass).collect())
    }
}

/// The linear system the comparison theorem produces: `u = b + C·u`.
#[derive(Debug, Clone, PartialEq)]
pub struct ComparisonSystem {
    sites: Vec<String>,
    interdependence: Vec<Vec<f64>>,
    perturbation: Vec<f64>,
    contraction: f64,
    free_positions: Vec<usize>,
}

impl ComparisonSystem {
    /// Region variables in the order the matrix is indexed by.
    pub fn sites(&self) -> &[String] {
        &self.sites
    }

    /// `C`, the entrywise maximum of the two measures' Dobrushin interdependence matrices.
    pub fn interdependence(&self) -> &[Vec<f64>] {
        &self.interdependence
    }

    /// `b`, the per-site displacement the perturbation applies to a single-site conditional.
    pub fn perturbation(&self) -> &[f64] {
        &self.perturbation
    }

    /// `‖C‖_∞`. Below one by construction, since the system is not returned otherwise.
    pub fn contraction(&self) -> f64 {
        self.contraction
    }

    /// Positions in [`ComparisonSystem::sites`] of the query's free variables.
    pub fn free_positions(&self) -> &[usize] {
        &self.free_positions
    }

    /// One application of `u ↦ b + C·u`, on plain reals.
    ///
    /// The abstract counterpart lives in [`mod@crate::interpret`]; this is the concrete operation the
    /// abstract one must over-approximate, and having it here in one line is what lets the suite
    /// state that relationship as a test rather than as a claim.
    pub fn apply(&self, state: &[f64]) -> Vec<f64> {
        self.interdependence
            .iter()
            .zip(&self.perturbation)
            .map(|(row, base)| {
                base + row
                    .iter()
                    .zip(state)
                    .map(|(coefficient, value)| coefficient * value)
                    .sum::<f64>()
            })
            .collect()
    }

    /// The exact solution of `(I − C)u = b` by Gaussian elimination with partial pivoting.
    ///
    /// Not used by the analysis, which computes a post-fixpoint through the domain machinery. It is
    /// here so the suite can measure what widening gave away and what narrowing took back against
    /// the number they are approximations of, rather than against each other.
    pub fn exact_solution(&self) -> Option<Vec<f64>> {
        let size = self.sites.len();
        let mut matrix: Vec<Vec<f64>> = (0..size)
            .map(|row| {
                let mut line: Vec<f64> = (0..size)
                    .map(|column| {
                        let identity = if row == column { 1.0 } else { 0.0 };
                        identity - self.interdependence[row][column]
                    })
                    .collect();
                line.push(self.perturbation[row]);
                line
            })
            .collect();

        for pivot in 0..size {
            let (best, magnitude) = (pivot..size).fold((pivot, 0.0f64), |(best, magnitude), row| {
                let candidate = matrix[row][pivot].abs();
                if candidate > magnitude {
                    (row, candidate)
                } else {
                    (best, magnitude)
                }
            });
            if magnitude < 1e-12 {
                return None;
            }
            matrix.swap(pivot, best);
            let divisor = matrix[pivot][pivot];
            for value in matrix[pivot][pivot..=size].iter_mut() {
                *value /= divisor;
            }
            let pivot_row = matrix[pivot].clone();
            for (index, row) in matrix.iter_mut().enumerate() {
                if index == pivot {
                    continue;
                }
                let factor = row[pivot];
                if factor == 0.0 {
                    continue;
                }
                for (value, base) in row[pivot..=size]
                    .iter_mut()
                    .zip(&pivot_row[pivot..=size])
                {
                    *value -= factor * base;
                }
            }
        }
        Some(matrix.iter().map(|row| row[size]).collect())
    }
}

fn neighbourhood(region: &QueryRegion, site: &str) -> (Vec<usize>, Vec<String>) {
    let mut touching = Vec::new();
    let mut neighbours: Vec<String> = Vec::new();
    for (position, factor) in region.factors().iter().enumerate() {
        if !factor.scope().iter().any(|name| name == site) {
            continue;
        }
        touching.push(position);
        for name in factor.scope() {
            if name != site && !neighbours.contains(name) {
                neighbours.push(name.clone());
            }
        }
    }
    neighbours.sort();
    (touching, neighbours)
}

/// Every conditional of one site, indexed by a mixed-radix encoding of its neighbourhood.
fn conditionals_of_site<'k>(
    specification: &Specification<'_>,
    site: &'k str,
    touching: &[usize],
    neighbours: &'k [String],
    radices: &[usize],
    configurations: usize,
) -> Result<Vec<Vec<f64>>, UnknownReason> {
    let cardinality = specification
        .region
        .cardinality_of(site)
        .expect("sites are region variables");
    let mut rows = Vec::with_capacity(configurations);
    for encoded in 0..configurations {
        let mut assignment: BTreeMap<&str, usize> = BTreeMap::new();
        let mut remaining = encoded;
        for (name, radix) in neighbours.iter().zip(radices).rev() {
            assignment.insert(name.as_str(), remaining % radix);
            remaining /= radix;
        }
        assignment.insert(site, 0);
        let row = specification
            .conditional(site, touching, &mut assignment, cardinality)
            .ok_or_else(|| {
                outside(format!(
                    "the single-site conditional at {site:?} has no mass under some neighbourhood configuration"
                ))
            })?;
        rows.push(row);
    }
    Ok(rows)
}

/// `C` for one specification.
fn interdependence_matrix(
    specification: &Specification<'_>,
    sites: &[String],
) -> Result<Vec<Vec<f64>>, UnknownReason> {
    let index_of: BTreeMap<&str, usize> = sites
        .iter()
        .enumerate()
        .map(|(position, name)| (name.as_str(), position))
        .collect();
    let mut matrix = vec![vec![0.0f64; sites.len()]; sites.len()];

    for (row, site) in sites.iter().enumerate() {
        let (touching, neighbours) = neighbourhood(specification.region, site);
        let radices: Vec<usize> = neighbours
            .iter()
            .map(|name| {
                specification
                    .region
                    .cardinality_of(name)
                    .expect("neighbours are region variables")
            })
            .collect();
        let mut configurations = 1usize;
        for radix in &radices {
            configurations = configurations.saturating_mul(*radix);
        }
        if configurations > MAX_CONDITIONAL_CONFIGURATIONS {
            return Err(outside(format!(
                "site {site:?} has a neighbourhood of {configurations} configurations, above the cap of {MAX_CONDITIONAL_CONFIGURATIONS}; enumerating a sample instead would produce a coefficient that is not a maximum"
            )));
        }
        let rows = conditionals_of_site(
            specification,
            site,
            &touching,
            &neighbours,
            &radices,
            configurations,
        )?;

        let mut strides = vec![1usize; neighbours.len()];
        for position in (0..neighbours.len().saturating_sub(1)).rev() {
            strides[position] = strides[position + 1] * radices[position + 1];
        }

        for (position, neighbour) in neighbours.iter().enumerate() {
            let column = index_of[neighbour.as_str()];
            let mut worst = 0.0f64;
            for encoded in 0..configurations {
                let current = (encoded / strides[position]) % radices[position];
                for alternative in (current + 1)..radices[position] {
                    let shifted =
                        encoded + (alternative - current) * strides[position];
                    worst = worst.max(total_variation_of_rows(&rows[encoded], &rows[shifted]));
                }
            }
            matrix[row][column] = matrix[row][column].max(worst);
        }
    }
    Ok(matrix)
}

/// `b` for a removal, computed exactly from the two specifications' conditionals.
fn removal_displacement(
    declared: &Specification<'_>,
    perturbed: &Specification<'_>,
    sites: &[String],
    scope: &[String],
) -> Result<Vec<f64>, UnknownReason> {
    let mut vector = vec![0.0f64; sites.len()];
    for (position, site) in sites.iter().enumerate() {
        if !scope.iter().any(|name| name == site) {
            continue;
        }
        let (touching, neighbours) = neighbourhood(declared.region, site);
        let radices: Vec<usize> = neighbours
            .iter()
            .map(|name| {
                declared
                    .region
                    .cardinality_of(name)
                    .expect("neighbours are region variables")
            })
            .collect();
        let mut configurations = 1usize;
        for radix in &radices {
            configurations = configurations.saturating_mul(*radix);
        }
        if configurations > MAX_CONDITIONAL_CONFIGURATIONS {
            return Err(outside(format!(
                "site {site:?} has a neighbourhood of {configurations} configurations, above the cap of {MAX_CONDITIONAL_CONFIGURATIONS}"
            )));
        }
        let before = conditionals_of_site(
            declared,
            site,
            &touching,
            &neighbours,
            &radices,
            configurations,
        )?;
        let after = conditionals_of_site(
            perturbed,
            site,
            &touching,
            &neighbours,
            &radices,
            configurations,
        )?;
        vector[position] = before
            .iter()
            .zip(&after)
            .map(|(left, right)| total_variation_of_rows(left, right))
            .fold(0.0f64, f64::max);
    }
    Ok(vector)
}

/// Builds `C` and `b` for one perturbation, or names the hypothesis that failed.
///
/// The outer `Result` is a malformed request; the inner one is a well-formed question this method
/// is not entitled to answer.
pub fn comparison_system(
    region: &QueryRegion,
    factor_ids: &[String],
    perturbation: &Perturbation,
) -> Result<Result<ComparisonSystem, UnknownReason>, InfluenceError> {
    if region.factors().is_empty() {
        return Ok(Err(outside("the region has no factors")));
    }
    if factor_ids.is_empty() {
        return Ok(Err(outside(
            "no factor was named, so there is no perturbation to compare against",
        )));
    }
    if region.free_variables().is_empty() {
        return Ok(Err(outside(
            "the query has no free variables, so there is no marginal for a perturbation to move",
        )));
    }
    let mut scope: Vec<String> = Vec::new();
    for factor_id in factor_ids {
        let Some(target) = region
            .factors()
            .iter()
            .find(|factor| factor.id() == *factor_id)
        else {
            return Err(InfluenceError::UnknownFactor {
                region: region.label().to_string(),
                factor: factor_id.clone(),
            });
        };
        for name in target.scope() {
            if !scope.contains(name) {
                scope.push(name.clone());
            }
        }
    }

    let declared = match Specification::declared(region) {
        Ok(specification) => specification,
        Err(reason) => return Ok(Err(reason)),
    };
    let sites: Vec<String> = region.cardinality().keys().cloned().collect();

    let mut matrix = match interdependence_matrix(&declared, &sites) {
        Ok(matrix) => matrix,
        Err(reason) => return Ok(Err(reason)),
    };

    let vector = match perturbation {
        Perturbation::Removal => {
            let perturbed = declared.with_factors_removed(factor_ids)?;
            let perturbed_matrix = match interdependence_matrix(&perturbed, &sites) {
                Ok(matrix) => matrix,
                Err(reason) => return Ok(Err(reason)),
            };
            for (row, other) in matrix.iter_mut().zip(&perturbed_matrix) {
                for (entry, candidate) in row.iter_mut().zip(other) {
                    *entry = entry.max(*candidate);
                }
            }
            match removal_displacement(&declared, &perturbed, &sites, &scope) {
                Ok(vector) => vector,
                Err(reason) => return Ok(Err(reason)),
            }
        }
        Perturbation::MultiplicativeRange { range } => sites
            .iter()
            .map(|site| {
                let touching = factor_ids
                    .iter()
                    .filter(|factor_id| {
                        region
                            .factors()
                            .iter()
                            .find(|factor| factor.id() == factor_id.as_str())
                            .is_some_and(|factor| {
                                factor.scope().iter().any(|name| name == site)
                            })
                    })
                    .count();
                if touching == 0 {
                    return 0.0;
                }
                let exponent = touching as i32;
                RatioInterval::range(range.lo().powi(exponent), range.hi().powi(exponent))
                    .map_or(1.0, RatioInterval::total_variation_bound)
            })
            .collect(),
    };

    let contraction = matrix
        .iter()
        .map(|row| row.iter().sum::<f64>())
        .fold(0.0f64, f64::max);
    if !contraction.is_finite() || contraction >= 1.0 {
        return Ok(Err(outside(format!(
            "the Dobrushin interdependence matrix has maximum row sum {contraction}; the comparison theorem's uniqueness condition asks for less than one, and the series it sums does not converge without it"
        ))));
    }

    let free_positions = sites
        .iter()
        .enumerate()
        .filter(|(_, name)| region.free_variables().contains(name.as_str()))
        .map(|(position, _)| position)
        .collect();

    Ok(Ok(ComparisonSystem {
        sites,
        interdependence: matrix,
        perturbation: vector,
        contraction,
        free_positions,
    }))
}
