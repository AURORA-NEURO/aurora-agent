//! The set functions evidence selection is run against: blueprint 43.14.
//!
//! 43.14 lists four candidate utilities — "obligation coverage, hypothesis elimination, expected
//! entropy reduction, or protected witness coverage" — and does not say which of them are
//! submodular. Three of them are implemented here, plus the one a context compiler actually wants,
//! and the interesting result is that they do not agree.
//!
//! | Objective | Normalised | Monotone | Submodular |
//! |---|---|---|---|
//! | [`Coverage`] | yes, by construction | yes, provably | yes, provably |
//! | [`HypothesisElimination`] | yes | yes | yes — it is a coverage function over the eliminated set |
//! | [`ExpectedRiskReduction`] | yes | yes, by the value-of-information argument | **no** |
//! | [`RegretReduction`] | yes | **no** | **no** |
//!
//! The proofs for the first two are the standard ones and are stated in the type docs. They are
//! also *checked*, by [`crate::submodularity::check`], on every instance the suite generates —
//! because a proof about the mathematical object is not a proof about this code, and the gap
//! between them is exactly where an off-by-one in a coverage index lives.
//!
//! The last two fail, and their failure is the finding. [`RegretReduction`] is the objective a
//! context compiler would reach for: it measures how much of the decision loss a context recovers.
//! It is not monotone — adding a partially informative item can move the posterior toward the
//! wrong action — so monotone greedy is not applicable to it at all, and the `1 − 1/e` factor is
//! not merely unproven for it, it is unavailable.
//!
//! ## Tabulation
//!
//! [`Tabulated`] evaluates a set function on every subset once and answers from a table. The
//! exhaustive checks in [`crate::submodularity`] make `O(n · 3^n)` queries; without tabulation
//! that is `O(n · 3^n)` posterior computations and the check becomes too slow to run on every
//! instance, which is the same as not running it.

use crate::decision::{Belief, DecisionProblem};
use crate::error::EpistemicError;
use crate::evidence::{Acquisition, EvidencePool};
use std::collections::BTreeSet;

/// A normalised set function over a finite ground set indexed `0..ground_size`.
///
/// `value` returns a `Result` rather than an `f64` because two of the implementations condition a
/// belief, and a subset that annihilates the posterior is a real input with no value — not a
/// value of zero.
pub trait SetFunction {
    /// A stable name, reported alongside every guarantee so the reader knows what was checked.
    fn name(&self) -> String;

    fn ground_size(&self) -> usize;

    fn value(&self, subset: &BTreeSet<usize>) -> Result<f64, EpistemicError>;

    /// `F(S ∪ {e}) − F(S)`.
    fn marginal(&self, subset: &BTreeSet<usize>, element: usize) -> Result<f64, EpistemicError> {
        if element >= self.ground_size() {
            return Err(EpistemicError::ElementOutOfRange {
                element,
                ground: self.ground_size(),
            });
        }
        let base = self.value(subset)?;
        let mut extended = subset.clone();
        extended.insert(element);
        Ok(self.value(&extended)? - base)
    }
}

/// Weighted set coverage: `F(S) = Σ_{o ∈ ⋃_{e∈S} covers(e)} w(o)`.
///
/// Monotone and submodular for non-negative weights, by the standard argument: a ground element
/// contributes the weight of the obligations it newly covers, and the set of obligations left
/// uncovered shrinks as the selection grows, so the contribution cannot grow. This is the shape of
/// 43.14's "obligation coverage" and "protected witness coverage" candidates.
#[derive(Debug, Clone, PartialEq)]
pub struct Coverage {
    label: String,
    covers: Vec<BTreeSet<usize>>,
    weights: Vec<f64>,
}

impl Coverage {
    pub fn new(
        label: impl Into<String>,
        covers: Vec<BTreeSet<usize>>,
        weights: Vec<f64>,
    ) -> Result<Self, EpistemicError> {
        for set in &covers {
            for &obligation in set {
                if obligation >= weights.len() {
                    return Err(EpistemicError::ElementOutOfRange {
                        element: obligation,
                        ground: weights.len(),
                    });
                }
            }
        }
        for (index, weight) in weights.iter().enumerate() {
            if !weight.is_finite() || *weight < 0.0 {
                return Err(EpistemicError::InadmissibleCost {
                    item: format!("obligation {index}"),
                    value: *weight,
                });
            }
        }
        Ok(Coverage {
            label: label.into(),
            covers,
            weights,
        })
    }

    /// Uniform weights, the plain cardinality-coverage case.
    pub fn uniform(
        label: impl Into<String>,
        covers: Vec<BTreeSet<usize>>,
        obligations: usize,
    ) -> Result<Self, EpistemicError> {
        Coverage::new(label, covers, vec![1.0; obligations])
    }
}

impl SetFunction for Coverage {
    fn name(&self) -> String {
        format!("coverage/{}", self.label)
    }

    fn ground_size(&self) -> usize {
        self.covers.len()
    }

    fn value(&self, subset: &BTreeSet<usize>) -> Result<f64, EpistemicError> {
        let mut covered: BTreeSet<usize> = BTreeSet::new();
        for &element in subset {
            let set = self
                .covers
                .get(element)
                .ok_or(EpistemicError::ElementOutOfRange {
                    element,
                    ground: self.covers.len(),
                })?;
            covered.extend(set.iter().copied());
        }
        Ok(covered.iter().map(|o| self.weights[*o]).sum())
    }
}

/// Hypothesis elimination: `F(S)` is the number of models `S` jointly rules out.
///
/// 43.14's second candidate utility. A coverage function in disguise — the "obligations" are the
/// models and each evidence item covers the ones it excludes — so it inherits monotonicity and
/// submodularity. It is kept as its own type rather than a constructor for [`Coverage`] because
/// the two answer different questions and a reader tracing a selection back to a blueprint clause
/// should not have to decode a coverage index into a model index.
#[derive(Debug, Clone, PartialEq)]
pub struct HypothesisElimination {
    inner: Coverage,
}

impl HypothesisElimination {
    /// Builds from an evidence pool: item `e` eliminates model `m` when `ℓ_e(m) = 0`.
    pub fn from_pool(pool: &EvidencePool, models: usize) -> Result<Self, EpistemicError> {
        let covers = pool
            .items()
            .iter()
            .map(|item| {
                (0..models)
                    .filter(|&m| item.likelihood(m) == 0.0)
                    .collect::<BTreeSet<usize>>()
            })
            .collect();
        Ok(HypothesisElimination {
            inner: Coverage::uniform("hypothesis-elimination", covers, models)?,
        })
    }
}

impl SetFunction for HypothesisElimination {
    fn name(&self) -> String {
        "hypothesis-elimination".to_string()
    }

    fn ground_size(&self) -> usize {
        self.inner.ground_size()
    }

    fn value(&self, subset: &BTreeSet<usize>) -> Result<f64, EpistemicError> {
        self.inner.value(subset)
    }
}

/// Decision-regret reduction: `F(S) = D(∅) − D(S)`, with `D` the distortion of [`crate::ratedistortion`].
///
/// The objective a context compiler wants, and the one the guarantee does not cover. It is
/// normalised — `F(∅) = 0` by construction — and it is neither monotone nor submodular, for the
/// same underlying reason: a context can be made *worse* by adding evidence, when the added item
/// moves the posterior toward an action the full evidence does not support.
///
/// This is not a defect of the implementation. It is a property of decision regret, and 43.14's
/// normative decision is written for exactly this case: "use greedy or lazy-greedy evidence
/// selection only when the utility exhibits validated diminishing returns; otherwise switch to
/// adaptive, non-submodular, or exact planning."
#[derive(Debug, Clone)]
pub struct RegretReduction<'a> {
    problem: &'a DecisionProblem,
    prior: &'a Belief,
    pool: &'a EvidencePool,
    baseline: f64,
    full: Belief,
}

impl<'a> RegretReduction<'a> {
    pub fn new(
        problem: &'a DecisionProblem,
        prior: &'a Belief,
        pool: &'a EvidencePool,
    ) -> Result<Self, EpistemicError> {
        prior.check_against(problem)?;
        pool.check_against(problem)?;
        let full = pool.full_posterior(prior)?;
        let empty = pool.posterior(prior, &BTreeSet::new())?;
        let baseline = problem.regret(&full, problem.bayes_action(&empty));
        Ok(RegretReduction {
            problem,
            prior,
            pool,
            baseline,
            full,
        })
    }

    /// The distortion of the empty context: how bad deciding with nothing is.
    pub fn baseline(&self) -> f64 {
        self.baseline
    }
}

impl SetFunction for RegretReduction<'_> {
    fn name(&self) -> String {
        "regret-reduction".to_string()
    }

    fn ground_size(&self) -> usize {
        self.pool.len()
    }

    fn value(&self, subset: &BTreeSet<usize>) -> Result<f64, EpistemicError> {
        let posterior = self.pool.posterior(self.prior, subset)?;
        let action = self.problem.bayes_action(&posterior);
        Ok(self.baseline - self.problem.regret(&self.full, action))
    }
}

/// Expected reduction in Bayes risk from a set of evidence *actions*.
///
/// Monotone — this is the value-of-information non-negativity argument applied to a growing set —
/// and not submodular. The pairing is the useful one: monotonicity alone buys nothing, because the
/// `1 − 1/e` factor needs both conditions, and an objective that passes one check and fails the
/// other is precisely the case a selector would get wrong by checking only the easy one.
#[derive(Debug, Clone)]
pub struct ExpectedRiskReduction<'a> {
    problem: &'a DecisionProblem,
    belief: &'a Belief,
    acquisitions: &'a [Acquisition],
}

impl<'a> ExpectedRiskReduction<'a> {
    pub fn new(
        problem: &'a DecisionProblem,
        belief: &'a Belief,
        acquisitions: &'a [Acquisition],
    ) -> Result<Self, EpistemicError> {
        belief.check_against(problem)?;
        Ok(ExpectedRiskReduction {
            problem,
            belief,
            acquisitions,
        })
    }
}

impl SetFunction for ExpectedRiskReduction<'_> {
    fn name(&self) -> String {
        "expected-risk-reduction".to_string()
    }

    fn ground_size(&self) -> usize {
        self.acquisitions.len()
    }

    fn value(&self, subset: &BTreeSet<usize>) -> Result<f64, EpistemicError> {
        let selected: Vec<Acquisition> = subset
            .iter()
            .map(|&index| {
                self.acquisitions
                    .get(index)
                    .cloned()
                    .ok_or(EpistemicError::ElementOutOfRange {
                        element: index,
                        ground: self.acquisitions.len(),
                    })
            })
            .collect::<Result<_, _>>()?;
        Ok(crate::voi::joint_value(self.problem, self.belief, &selected)?.gross)
    }
}

/// A set function evaluated once on every subset and answered from a table.
///
/// The cap is not a performance guard, it is a correctness one: [`crate::submodularity::check`]
/// enumerates `O(n · 3^n)` triples, and a check that degraded to sampling above some size would be
/// weakest exactly on the instances where a violation is hardest to spot by inspection.
#[derive(Debug, Clone, PartialEq)]
pub struct Tabulated {
    label: String,
    ground: usize,
    values: Vec<f64>,
}

/// The largest ground set [`Tabulated`] will build a table for. `2^18` entries.
pub const MAX_TABULATED_GROUND: usize = 18;

impl Tabulated {
    pub fn of<F: SetFunction + ?Sized>(function: &F) -> Result<Self, EpistemicError> {
        let ground = function.ground_size();
        if ground > MAX_TABULATED_GROUND {
            return Err(EpistemicError::ExhaustiveCapExceeded {
                ground,
                needed: 1u64 << ground.min(63),
                cap: 1u64 << MAX_TABULATED_GROUND,
            });
        }
        let total = 1usize << ground;
        let mut values = Vec::with_capacity(total);
        for mask in 0..total {
            let subset: BTreeSet<usize> = (0..ground).filter(|i| (mask >> i) & 1 == 1).collect();
            values.push(function.value(&subset)?);
        }
        Ok(Tabulated {
            label: function.name(),
            ground,
            values,
        })
    }

    /// Value of the subset denoted by a bitmask. Panics only on a mask outside the table, which
    /// is unreachable from the public API.
    pub fn at(&self, mask: usize) -> f64 {
        self.values[mask]
    }

    pub fn ground(&self) -> usize {
        self.ground
    }

    /// `F(mask ∪ {e}) − F(mask)`, with `e` required to be outside `mask`.
    pub fn gain(&self, mask: usize, element: usize) -> f64 {
        self.values[mask | (1 << element)] - self.values[mask]
    }
}

impl SetFunction for Tabulated {
    fn name(&self) -> String {
        self.label.clone()
    }

    fn ground_size(&self) -> usize {
        self.ground
    }

    fn value(&self, subset: &BTreeSet<usize>) -> Result<f64, EpistemicError> {
        let mut mask = 0usize;
        for &element in subset {
            if element >= self.ground {
                return Err(EpistemicError::ElementOutOfRange {
                    element,
                    ground: self.ground,
                });
            }
            mask |= 1 << element;
        }
        Ok(self.values[mask])
    }
}
