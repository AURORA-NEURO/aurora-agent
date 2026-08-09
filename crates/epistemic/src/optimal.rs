//! The optimum by brute force, and the approximation ratio that comes out against greedy.
//!
//! Blueprint 43.14's evaluation program leads with "optimality gap on small exact instances", and
//! its implementation artifacts list an "exact small-instance benchmark". This is that benchmark.
//! It exists for the reason `bioprism-influence`'s brute-force suite exists: a guarantee nobody
//! checked against ground truth is a number, not a guarantee.
//!
//! ## What is measured
//!
//! For each generated instance, the true constrained optimum by enumeration, the greedy selection,
//! and their ratio. The reported statistic is the **minimum** ratio over the family, not the mean:
//! an approximation factor is a worst-case claim, and averaging is how a worst case gets hidden.
//!
//! ## The measurement, on the seeds the suite runs
//!
//! | Family | Instances | Passed the check | Worst ratio | Mean | Below `1 − 1/e` |
//! |---|---|---|---|---|---|
//! | [`coverage_family`], seed `0x5EED1234`, ground 9, `k = 3` | 40 | 40 | **0.909** | 0.990 | 0 |
//! | [`regret_family`], seed `0x9E3779B9`, ground 8, `k = 2` | 60 | **17** | **0.000** | 0.950 | 3 |
//!
//! Three things in that table are worth more than the headline.
//!
//! **The worst coverage instance is 0.909, not 0.632.** The `1 − 1/e` bound is tight only on
//! adversarial constructions; on random coverage instances greedy is very good, and the guarantee
//! is a floor nobody came near. Reporting the floor as though it were the expected performance
//! would understate greedy, and reporting 0.909 as though it were a guarantee would overstate it.
//!
//! **17 of 60 random decision instances *are* monotone submodular.** Regret reduction is not
//! submodular *in general*, which is a different and weaker claim than "regret reduction is never
//! submodular". That difference is the entire argument for checking per instance rather than
//! deciding per objective: a selector that assumed non-submodularity would decline a guarantee it
//! could have had 28% of the time, and one that assumed submodularity would quote a factor it did
//! not have 72% of the time.
//!
//! **43 of the 61 regret instances are degenerate** — no subset within the cardinality bound
//! reduces regret at all, because the evidence never moves the Bayes action off the prior's
//! choice. Those score `1.0` by the convention below and they are what pulls the mean to 0.950. A
//! mean over this family is close to meaningless; the minimum is the statistic.
//!
//! [`complementary_instance`] is the extreme case, built by hand to reproduce 43.14's own worked
//! micro-example, and it is where the `0.000` comes from: every first-step marginal is zero, so
//! greedy takes nothing while a two-element budget reaches the optimum exactly.
//!
//! ## Ratio when the optimum is zero
//!
//! Reported as `1.0`. Both selectors achieved everything achievable, and a `0/0` recorded as `0.0`
//! would drag a minimum down with an instance that discriminates nothing. Instances in that state
//! are counted separately in [`RatioMeasurement::degenerate`] so the figure can be read without
//! them.

use crate::decision::{Belief, DecisionProblem};
use crate::error::EpistemicError;
use crate::evidence::{EvidenceItem, EvidencePool};
use crate::greedy::{greedy, Constraint};
use crate::objective::{Coverage, RegretReduction, SetFunction, Tabulated};
use crate::rng::SplitMix64;
use crate::submodularity;
use crate::theorem::ONE_MINUS_ONE_OVER_E;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Largest ground set [`brute_force_optimum`] will enumerate. `2^20` subsets.
pub const MAX_BRUTE_FORCE_GROUND: usize = 20;

/// The best feasible subset containing `protected`, by enumeration.
pub fn brute_force_optimum<F: SetFunction + ?Sized>(
    function: &F,
    constraint: &Constraint,
    protected: &BTreeSet<usize>,
) -> Result<(BTreeSet<usize>, f64), EpistemicError> {
    let ground = function.ground_size();
    if ground > MAX_BRUTE_FORCE_GROUND {
        return Err(EpistemicError::ExhaustiveCapExceeded {
            ground,
            needed: 1u64 << ground.min(63),
            cap: 1u64 << MAX_BRUTE_FORCE_GROUND,
        });
    }
    let mut best: Option<(BTreeSet<usize>, f64)> = None;
    for mask in 0..(1usize << ground) {
        let subset: BTreeSet<usize> = (0..ground).filter(|i| (mask >> i) & 1 == 1).collect();
        if !protected.is_subset(&subset) {
            continue;
        }
        if let Some(k) = constraint.cardinality {
            if subset.len() > k {
                continue;
            }
        }
        if let Some(budget) = constraint.budget {
            if constraint.cost_of(&subset) > budget + crate::greedy::MARGINAL_EPSILON {
                continue;
            }
        }
        let value = function.value(&subset)?;
        if best
            .as_ref()
            .is_none_or(|(_, current)| value > *current + crate::greedy::MARGINAL_EPSILON)
        {
            best = Some((subset, value));
        }
    }
    best.ok_or(EpistemicError::UnconstrainedSelection)
}

/// The instance on which greedy did worst.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct WorstInstance {
    pub index: usize,
    pub ratio: f64,
    pub greedy_value: f64,
    pub optimal_value: f64,
    pub greedy_set: Vec<usize>,
    pub optimal_set: Vec<usize>,
    pub cardinality: usize,
}

/// What greedy attained against the true optimum, over a family of instances.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RatioMeasurement {
    pub family: String,
    pub instances: usize,
    /// The worst-case statistic. This is the number an approximation factor has to beat.
    pub min_ratio: f64,
    pub mean_ratio: f64,
    /// Instances where the optimum was zero, so the ratio carries no information.
    pub degenerate: usize,
    /// Instances landing strictly below `1 − 1/e`.
    pub below_theoretical: usize,
    /// Instances whose objective passed the exhaustive monotone-submodular check.
    pub passed_submodularity: usize,
    pub worst: Option<WorstInstance>,
}

impl RatioMeasurement {
    /// Whether every instance cleared the `1 − 1/e` line.
    pub fn respects_theoretical_factor(&self) -> bool {
        self.below_theoretical == 0
    }
}

/// Runs greedy against brute force over a family of tabulated objectives.
pub fn measure_ratio(
    family: impl Into<String>,
    instances: &[(Tabulated, Constraint)],
) -> Result<RatioMeasurement, EpistemicError> {
    let mut min_ratio = f64::INFINITY;
    let mut total = 0.0f64;
    let mut degenerate = 0usize;
    let mut below = 0usize;
    let mut passed = 0usize;
    let mut worst: Option<WorstInstance> = None;
    let protected = BTreeSet::new();

    for (index, (function, constraint)) in instances.iter().enumerate() {
        let report = submodularity::check(function)?;
        if report.monotone_submodular() {
            passed += 1;
        }
        let selection = greedy(function, constraint, &protected, Some(&report))?;
        let (optimal_set, optimal_value) = brute_force_optimum(function, constraint, &protected)?;

        let ratio = if optimal_value.abs() <= crate::greedy::MARGINAL_EPSILON {
            degenerate += 1;
            1.0
        } else {
            selection.value / optimal_value
        };
        total += ratio;
        if ratio < ONE_MINUS_ONE_OVER_E {
            below += 1;
        }
        if ratio < min_ratio {
            min_ratio = ratio;
            worst = Some(WorstInstance {
                index,
                ratio,
                greedy_value: selection.value,
                optimal_value,
                greedy_set: selection.chosen.clone(),
                optimal_set: optimal_set.iter().copied().collect(),
                cardinality: constraint.cardinality.unwrap_or(0),
            });
        }
    }

    Ok(RatioMeasurement {
        family: family.into(),
        instances: instances.len(),
        min_ratio: if instances.is_empty() { 1.0 } else { min_ratio },
        mean_ratio: if instances.is_empty() {
            1.0
        } else {
            total / instances.len() as f64
        },
        degenerate,
        below_theoretical: below,
        passed_submodularity: passed,
        worst,
    })
}

/// A family of weighted-coverage instances. Monotone submodular by construction.
///
/// `ground` items each cover a random subset of `obligations`, with random positive weights. The
/// cardinality bound is roughly a third of the ground set, which is where greedy has room to be
/// wrong without the constraint trivially forcing the optimum.
pub fn coverage_family(
    seed: u64,
    count: usize,
    ground: usize,
    obligations: usize,
) -> Result<Vec<(Tabulated, Constraint)>, EpistemicError> {
    let mut rng = SplitMix64::new(seed);
    let mut out = Vec::with_capacity(count);
    for instance in 0..count {
        let covers: Vec<BTreeSet<usize>> = (0..ground)
            .map(|_| {
                (0..obligations)
                    .filter(|_| rng.unit() < 0.4)
                    .collect::<BTreeSet<usize>>()
            })
            .collect();
        let weights: Vec<f64> = (0..obligations).map(|_| rng.between(0.5, 4.0)).collect();
        let function = Coverage::new(format!("generated-{instance}"), covers, weights)?;
        let k = (ground / 3).max(1);
        out.push((
            Tabulated::of(&function)?,
            Constraint::cardinality(k, ground)?,
        ));
    }
    Ok(out)
}

/// A family of decision problems priced by regret reduction. Neither monotone nor submodular.
///
/// Each instance draws a loss matrix and a pool of evidence with random per-model likelihoods, so
/// items vary from decisive to actively misleading. This is the realistic case: nobody selecting
/// context for a decision is optimising set coverage.
pub fn regret_family(
    seed: u64,
    count: usize,
    ground: usize,
    models: usize,
    actions: usize,
) -> Result<Vec<RegretInstance>, EpistemicError> {
    let mut rng = SplitMix64::new(seed);
    let mut out = Vec::with_capacity(count);
    for _ in 0..count {
        let loss: Vec<f64> = (0..actions * models).map(|_| rng.between(0.0, 1.0)).collect();
        let problem = DecisionProblem::new(
            (0..actions).map(|a| format!("a{a}")).collect(),
            (0..models).map(|m| format!("m{m}")).collect(),
            loss,
        )?;
        let prior = Belief::new((0..models).map(|_| rng.between(0.2, 1.0)).collect())?;
        let items: Vec<EvidenceItem> = (0..ground)
            .map(|index| {
                let likelihood: Vec<f64> =
                    (0..models).map(|_| rng.between(0.05, 1.0)).collect();
                EvidenceItem::new(format!("e{index}"), 1.0, likelihood)
            })
            .collect::<Result<_, _>>()?;
        let pool = EvidencePool::new(items)?;
        out.push(RegretInstance {
            problem,
            prior,
            pool,
            cardinality: (ground / 3).max(1),
        });
    }
    Ok(out)
}

/// One generated decision instance, held together so the borrows in [`RegretReduction`] have an owner.
#[derive(Debug, Clone)]
pub struct RegretInstance {
    pub problem: DecisionProblem,
    pub prior: Belief,
    pub pool: EvidencePool,
    pub cardinality: usize,
}

impl RegretInstance {
    /// Tabulates the regret-reduction objective over this instance.
    pub fn tabulate(&self) -> Result<(Tabulated, Constraint), EpistemicError> {
        let function = RegretReduction::new(&self.problem, &self.prior, &self.pool)?;
        Ok((
            Tabulated::of(&function)?,
            Constraint::cardinality(self.cardinality, self.pool.len())?,
        ))
    }
}

/// 43.14's worked micro-example, built by hand.
///
/// > Tumor purity and copy-number evidence may each be weak alone but decisive together. Marginal
/// > gains increase after the first item, violating submodularity.
///
/// Four models encode the two-bit joint state a decider needs: purity high or low, copy number
/// gained or flat. Amplification may be called only under `purity_high_cn_gain`. `purity` and
/// `copy_number` each reveal one bit and neither is enough on its own — after either, the
/// posterior still favours not calling, so **each has a marginal gain of exactly zero from the
/// empty context**. Together they pin the model down and recover the entire decision loss.
///
/// The consequence for greedy is not a small gap, it is total: every first-step marginal is zero,
/// greedy declines to take a non-positive step, and it selects nothing. The measured ratio on this
/// instance is `0.0`, against an optimum that a two-element budget can reach exactly.
///
/// `expression` is a broad weakly informative item included so the ground set is not just the
/// decisive pair; it too has marginal zero, and its presence shows the failure is not an artefact
/// of a two-item pool.
pub fn complementary_instance() -> Result<RegretInstance, EpistemicError> {
    let models = vec![
        "purity_high_cn_gain".to_string(),
        "purity_high_cn_flat".to_string(),
        "purity_low_cn_gain".to_string(),
        "purity_low_cn_flat".to_string(),
    ];
    let actions = vec![
        "call_amplification".to_string(),
        "call_no_amplification".to_string(),
        "request_more_tissue".to_string(),
    ];
    let loss = vec![
        0.0, 1.0, 1.0, 1.0, //
        1.0, 0.0, 0.0, 0.0, //
        0.45, 0.45, 0.45, 0.45,
    ];
    let problem = DecisionProblem::new(actions, models, loss)?;
    let prior = Belief::new(vec![0.2, 0.3, 0.3, 0.2])?;

    let expression = EvidenceItem::new("expression", 1.0, vec![1.0, 0.9, 0.9, 0.8])?;
    let purity = EvidenceItem::new("purity", 1.0, vec![1.0, 1.0, 0.02, 0.02])?;
    let copy_number = EvidenceItem::new("copy_number", 1.0, vec![1.0, 0.02, 1.0, 0.02])?;

    let pool = EvidencePool::new(vec![expression, purity, copy_number])?;
    Ok(RegretInstance {
        problem,
        prior,
        pool,
        cardinality: 2,
    })
}

/// An instance where adding evidence makes the decision worse.
///
/// The monotonicity counterexample of 43.47's corpus. Three models, three actions, and one item
/// that points hard at `m1` while the rest of the evidence points at `m0`. Taking that item alone
/// moves the Bayes action to `a1`, which the full posterior does not support — so regret reduction
/// goes *negative* on a non-empty context.
///
/// Monotone greedy is not merely unguaranteed here, it is inapplicable: its correctness argument
/// assumes marginals are non-negative, and the whole point of this instance is that they are not.
pub fn misleading_instance() -> Result<RegretInstance, EpistemicError> {
    let models = vec!["m0".to_string(), "m1".to_string(), "m2".to_string()];
    let actions = vec!["a0".to_string(), "a1".to_string(), "a2".to_string()];
    let loss = vec![
        0.0, 1.0, 1.0, //
        1.0, 0.0, 1.0, //
        1.0, 1.0, 0.0,
    ];
    let problem = DecisionProblem::new(actions, models, loss)?;
    let prior = Belief::uniform(3)?;

    let misleading = EvidenceItem::new("misleading", 1.0, vec![0.1, 1.0, 0.1])?;
    let corroborating = EvidenceItem::new("corroborating", 1.0, vec![1.0, 0.05, 1.0])?;

    let pool = EvidencePool::new(vec![misleading, corroborating])?;
    Ok(RegretInstance {
        problem,
        prior,
        pool,
        cardinality: 1,
    })
}
