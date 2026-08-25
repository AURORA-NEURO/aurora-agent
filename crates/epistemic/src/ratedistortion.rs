//! Causal decision rate–distortion: blueprint 43.50, and the part of 43.12 it generalises.
//!
//! `bioprism-examples` records `rate_distortion_optimisation` as a blocked claim, and names the
//! obstacle: *"the same missing decision_loss field: there is no loss to trade distortion against"*.
//! This module supplies the calculus with the loss supplied by the caller. Legacy FIBER wire
//! forms cannot supply one; `fiber-query/0.3` supplies the 43.10 loss table, while the additional
//! posterior and evidence-pool bindings required for this module remain a future wire contract.
//!
//! ## The two numbers
//!
//! **Rate** is what a context costs: the summed cost of the retained evidence.
//!
//! **Distortion** is what acting on it costs. Concretely: compress to a context `Z`, take the
//! action that is optimal *given only `Z`*, and evaluate that action against the belief you would
//! have held with everything. The excess loss is the distortion:
//!
//! ```text
//! D(Z) = risk(π_full, a*(Z)) − risk(π_full, a*(full))
//! ```
//!
//! This is decision regret, not information loss, and the difference is the module's whole point.
//! Two contexts that lose the same number of bits can have distortions of zero and of everything,
//! because bits that no permitted action depends on are free to drop. That is the sense in which
//! optimising a context is action-aware rather than predictive.
//!
//! ## Why distortion is not monotone, and why that is the interesting fact
//!
//! Dropping evidence can *lower* distortion. A partially informative item can pull the posterior
//! toward the wrong action, and a context omitting it lands closer to the full-evidence decision
//! than one including it. [`the_frontier`](frontier) therefore enumerates rather than descends,
//! and [`crate::greedy`](mod@crate::greedy) cannot claim a submodular guarantee for regret reduction — the
//! non-monotonicity here and the failed submodularity check there are the same fact seen twice.
//!
//! ## Bayes and minimax, and what identification means here
//!
//! [`DistortionCriterion::BayesRegret`] needs a prior worth having. When there is none —
//! 43.50's "compatible-model set" case — [`DistortionCriterion::MinimaxRegret`] measures the worst
//! regret over the surviving models with no distribution over them at all.
//!
//! [`Identification`] classifies whether model uncertainty reaches the decision. **It is not
//! causal identification.** This crate implements no do-calculus, no back-door or front-door
//! criterion, and no graph surgery; it cannot tell you whether an effect is estimable from
//! observational data. What it can tell you is whether the models still standing after the
//! evidence disagree about what to do, which is the question the *context compiler* has to answer,
//! and 43.50's own runtime contract asks for exactly that under the row "Decision sufficiency:
//! regret bound relative to model set and action policy". The naming overlap is a trap and is
//! called out here rather than left for a reader to fall into.

use crate::decision::{Belief, DecisionProblem, LOSS_EPSILON};
use crate::error::EpistemicError;
use crate::evidence::EvidencePool;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// The largest evidence pool [`frontier`] and [`minimal_sufficient_context`] will enumerate.
///
/// `2^16` subsets. Beyond it these functions refuse. A frontier computed by a heuristic search is
/// a set of upper bounds on the true frontier, and reporting it under the same type would let a
/// caller read "the minimum distortion at this rate" off a number that is not one.
pub const MAX_ENUMERATED_SUBSETS: u64 = 1 << 16;

/// How the loss of a compressed context is measured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DistortionCriterion {
    /// Excess expected loss under the full-evidence posterior. Needs a prior.
    BayesRegret,
    /// Worst-case regret over the compatible-model set, with no distribution over it.
    MinimaxRegret,
}

/// What one candidate context costs and what acting on it costs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextEvaluation {
    /// Evidence retained, by pool index.
    pub retained: Vec<usize>,
    /// Summed cost of the retained evidence.
    pub rate: f64,
    /// Excess loss of acting on this context, under the chosen criterion.
    pub distortion: f64,
    /// The action a decider holding only this context would take.
    pub action: usize,
    /// The action a decider holding everything would take.
    pub reference_action: usize,
    /// Models still carrying mass under this context, at the caller's floor.
    pub compatible: Vec<usize>,
}

impl ContextEvaluation {
    /// Whether this context changes the decision relative to holding everything.
    ///
    /// Distortion can be zero while the action differs, when two actions tie under the full
    /// posterior. Both facts are reported because they answer different questions: a certificate
    /// consumer cares about the loss, an auditor replaying a trace cares about the action.
    pub fn action_preserved(&self) -> bool {
        self.action == self.reference_action
    }
}

/// Evaluates one candidate context.
///
/// `compatibility_floor` is the posterior mass below which a model is treated as ruled out. It is
/// the caller's because 43.50 puts the compatible-model set in the runtime contract; a library
/// default here would decide, invisibly, which models a minimax is taken over.
pub fn evaluate_context(
    problem: &DecisionProblem,
    prior: &Belief,
    pool: &EvidencePool,
    subset: &BTreeSet<usize>,
    criterion: DistortionCriterion,
    compatibility_floor: f64,
) -> Result<ContextEvaluation, EpistemicError> {
    let full = FullEvidence::resolve(problem, prior, pool, compatibility_floor)?;
    let (action, distortion) = decide_and_measure(
        problem,
        prior,
        pool,
        subset,
        criterion,
        compatibility_floor,
        &full,
    )?;

    Ok(ContextEvaluation {
        retained: subset.iter().copied().collect(),
        rate: pool.rate(subset)?,
        distortion,
        action,
        reference_action: full.reference_action,
        compatible: full.compatible,
    })
}

/// What every candidate context is measured *against*: the belief, decision and compatible set a
/// decider holding all the evidence would hold.
///
/// None of the three depends on the subset, and the validations that gate them do not either.
/// [`frontier`] resolves them once rather than `2^n` times; [`evaluate_context`] resolves them for
/// its single subset. Both then run [`decide_and_measure`], so there is one evaluation body and
/// the two entry points cannot drift.
struct FullEvidence {
    posterior: Belief,
    reference_action: usize,
    compatible: Vec<usize>,
}

impl FullEvidence {
    fn resolve(
        problem: &DecisionProblem,
        prior: &Belief,
        pool: &EvidencePool,
        compatibility_floor: f64,
    ) -> Result<Self, EpistemicError> {
        prior.check_against(problem)?;
        pool.check_against(problem)?;

        let posterior = pool.full_posterior(prior)?;
        let reference_action = problem.bayes_action(&posterior);
        let compatible = posterior.support_above(compatibility_floor);
        Ok(FullEvidence {
            posterior,
            reference_action,
            compatible,
        })
    }
}

/// The action a decider holding only `subset` takes, and what that costs against `full`.
fn decide_and_measure(
    problem: &DecisionProblem,
    prior: &Belief,
    pool: &EvidencePool,
    subset: &BTreeSet<usize>,
    criterion: DistortionCriterion,
    compatibility_floor: f64,
    full: &FullEvidence,
) -> Result<(usize, f64), EpistemicError> {
    let compressed = pool.posterior(prior, subset)?;

    Ok(match criterion {
        DistortionCriterion::BayesRegret => {
            let action = problem.bayes_action(&compressed);
            (action, problem.regret(&full.posterior, action))
        }
        DistortionCriterion::MinimaxRegret => {
            let compressed_compatible = compressed.support_above(compatibility_floor);
            let action = problem
                .minimax_action(&compressed_compatible)
                .unwrap_or_else(|| problem.bayes_action(&compressed));
            (action, problem.minimax_regret(&full.compatible, action))
        }
    })
}

/// One Pareto-optimal point of the rate–distortion frontier.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FrontierPoint {
    pub rate: f64,
    pub distortion: f64,
    pub retained: Vec<usize>,
}

/// The achievable rate–distortion frontier of an evidence pool, by exhaustion.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Frontier {
    pub points: Vec<FrontierPoint>,
    pub criterion: DistortionCriterion,
    /// Subsets evaluated. Equal to `2^|pool|`, because the search is exhaustive or it refuses.
    pub evaluated: usize,
}

impl Frontier {
    /// The lowest distortion achievable at rate at most `rate`.
    ///
    /// `None` when no context is that cheap — which happens only for a negative `rate`, since the
    /// empty context always has rate zero.
    pub fn best_at_rate(&self, rate: f64) -> Option<&FrontierPoint> {
        self.points
            .iter()
            .filter(|p| p.rate <= rate + LOSS_EPSILON)
            .min_by(|a, b| a.distortion.total_cmp(&b.distortion))
    }

    /// The cheapest context whose distortion is at most `epsilon`.
    pub fn cheapest_within(&self, epsilon: f64) -> Option<&FrontierPoint> {
        self.points
            .iter()
            .filter(|p| p.distortion <= epsilon + LOSS_EPSILON)
            .min_by(|a, b| a.rate.total_cmp(&b.rate))
    }
}

/// Enumerates every context and keeps the Pareto-optimal ones.
///
/// Exhaustive by construction: a frontier is a claim about the *minimum* distortion at each rate,
/// and a search that stopped early would report an upper bound wearing the same type.
pub fn frontier(
    problem: &DecisionProblem,
    prior: &Belief,
    pool: &EvidencePool,
    criterion: DistortionCriterion,
    compatibility_floor: f64,
) -> Result<Frontier, EpistemicError> {
    let n = pool.len();
    let needed = 1u64
        .checked_shl(n as u32)
        .ok_or(EpistemicError::ExhaustiveCapExceeded {
            ground: n,
            needed: u64::MAX,
            cap: MAX_ENUMERATED_SUBSETS,
        })?;
    if needed > MAX_ENUMERATED_SUBSETS {
        return Err(EpistemicError::ExhaustiveCapExceeded {
            ground: n,
            needed,
            cap: MAX_ENUMERATED_SUBSETS,
        });
    }

    let full = FullEvidence::resolve(problem, prior, pool, compatibility_floor)?;
    let mut candidates: Vec<FrontierPoint> = Vec::with_capacity(needed as usize);
    for mask in 0..needed {
        let subset: BTreeSet<usize> = (0..n).filter(|i| (mask >> i) & 1 == 1).collect();
        let (_, distortion) = decide_and_measure(
            problem,
            prior,
            pool,
            &subset,
            criterion,
            compatibility_floor,
            &full,
        )?;
        candidates.push(FrontierPoint {
            rate: pool.rate(&subset)?,
            distortion,
            retained: subset.into_iter().collect(),
        });
    }

    Ok(Frontier {
        points: pareto_optimal(candidates),
        criterion,
        evaluated: needed as usize,
    })
}

/// The Pareto-optimal candidates, ordered by rate then distortion then retained set.
///
/// `q` dominates `p` when it costs no more and distorts no more, each within [`LOSS_EPSILON`], and
/// beats `p` by more than that tolerance on at least one of the two. Domination is tested against
/// *every* candidate rather than only the survivors, which matters: the tolerance makes the
/// relation non-transitive, so a chain where each link dominates the one before it but the last
/// does not dominate the first answers differently under the two readings.
///
/// Two monotone sweeps over a sorted list rather than the pairwise scan this replaces. `p` is
/// dominated exactly when either
///
/// - some `q` strictly cheaper than tolerance has distortion within tolerance of `p`'s, or
/// - some `q` no more expensive than tolerance has distortion strictly below tolerance of `p`'s,
///
/// and each of those two candidate sets is a prefix of the rate-sorted list whose boundary only
/// ever moves forward. [`frontier`] enumerates up to [`MAX_ENUMERATED_SUBSETS`] candidates, where
/// the pairwise form is four billion comparisons.
fn pareto_optimal(mut candidates: Vec<FrontierPoint>) -> Vec<FrontierPoint> {
    candidates.sort_by(|a, b| {
        a.rate
            .total_cmp(&b.rate)
            .then(a.distortion.total_cmp(&b.distortion))
            .then(a.retained.cmp(&b.retained))
    });

    let mut cheaper_than = 0usize;
    let mut least_distortion_cheaper = f64::INFINITY;
    let mut no_dearer_than = 0usize;
    let mut least_distortion_no_dearer = f64::INFINITY;
    let mut points: Vec<FrontierPoint> = Vec::new();

    for index in 0..candidates.len() {
        let point = &candidates[index];
        while cheaper_than < candidates.len()
            && candidates[cheaper_than].rate < point.rate - LOSS_EPSILON
        {
            least_distortion_cheaper =
                least_distortion_cheaper.min(candidates[cheaper_than].distortion);
            cheaper_than += 1;
        }
        while no_dearer_than < candidates.len()
            && candidates[no_dearer_than].rate <= point.rate + LOSS_EPSILON
        {
            least_distortion_no_dearer =
                least_distortion_no_dearer.min(candidates[no_dearer_than].distortion);
            no_dearer_than += 1;
        }

        let beaten_on_rate =
            cheaper_than > 0 && least_distortion_cheaper <= point.distortion + LOSS_EPSILON;
        let beaten_on_distortion =
            no_dearer_than > 0 && least_distortion_no_dearer < point.distortion - LOSS_EPSILON;
        if !beaten_on_rate && !beaten_on_distortion {
            points.push(point.clone());
        }
    }

    points.dedup_by(|a, b| {
        (a.rate - b.rate).abs() <= LOSS_EPSILON
            && (a.distortion - b.distortion).abs() <= LOSS_EPSILON
    });
    points
}

/// Whether residual model uncertainty reaches the decision.
///
/// Read the module docs before using the name: this is *decision* identification, not causal
/// identification. A `PointIdentified` verdict says the compatible models agree on what to do; it
/// says nothing about whether any causal effect is estimable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "status")]
pub enum Identification {
    /// Every compatible model prefers the same action, so the residual uncertainty is decision-irrelevant.
    PointIdentified {
        action: usize,
        compatible: Vec<usize>,
    },
    /// Compatible models disagree, but the minimax action's worst regret is within tolerance.
    SetIdentifiedWithinTolerance {
        action: usize,
        minimax_regret: f64,
        compatible: Vec<usize>,
    },
    /// Compatible models disagree by more than tolerance even with all evidence in hand.
    /// No context is decision-sufficient, so the honest output is abstention.
    NonIdentified {
        minimax_regret: f64,
        tolerance: f64,
        compatible: Vec<usize>,
    },
}

impl Identification {
    pub fn supports_a_decision(&self) -> bool {
        !matches!(self, Identification::NonIdentified { .. })
    }
}

/// Classifies the decision after all available evidence has been folded in.
pub fn identification(
    problem: &DecisionProblem,
    prior: &Belief,
    pool: &EvidencePool,
    tolerance: f64,
    compatibility_floor: f64,
) -> Result<Identification, EpistemicError> {
    if !tolerance.is_finite() || tolerance < 0.0 {
        return Err(EpistemicError::InadmissibleTolerance { value: tolerance });
    }
    let full = pool.full_posterior(prior)?;
    let compatible = full.support_above(compatibility_floor);
    if problem.actions_agree(&compatible) {
        return Ok(Identification::PointIdentified {
            action: problem.bayes_action(&full),
            compatible,
        });
    }
    let action = problem
        .minimax_action(&compatible)
        .unwrap_or_else(|| problem.bayes_action(&full));
    let regret = problem.minimax_regret(&compatible, action);
    if regret <= tolerance + LOSS_EPSILON {
        Ok(Identification::SetIdentifiedWithinTolerance {
            action,
            minimax_regret: regret,
            compatible,
        })
    } else {
        Ok(Identification::NonIdentified {
            minimax_regret: regret,
            tolerance,
            compatible,
        })
    }
}

/// The outcome of asking for the smallest decision-sufficient context.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "outcome")]
pub enum Sufficiency {
    /// A context exists whose distortion is within tolerance, and this is the cheapest one.
    Sufficient {
        retained: Vec<usize>,
        rate: f64,
        distortion: f64,
        /// Rate of retaining everything, so a caller can read the saving without recomputing it.
        full_rate: f64,
    },
    /// Even the full context exceeds the tolerance. 43.50 requires abstention here rather than a
    /// point answer, and this variant is that abstention made representable.
    Abstain {
        reason: AbstentionReason,
        best_distortion: f64,
        tolerance: f64,
    },
}

/// Why no context was decision-sufficient.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AbstentionReason {
    /// The compatible models disagree by more than tolerance with all evidence in hand. No
    /// compression is at fault; the evidence does not determine the decision.
    NonIdentifiedUnderAllEvidence,
    /// A tolerance of zero was requested and no context, including the full one, attained it.
    /// Reachable only under a minimax criterion, where the full context has non-zero distortion.
    ToleranceUnattainable,
}

/// The cheapest context whose distortion is within `tolerance`, or an abstention.
///
/// This is the operation `bioprism-examples` calls `rate_distortion_optimisation`: "the smallest
/// context is chosen subject to a stated bound on decision loss".
///
/// ## When abstention is forced, and when it is not
///
/// The identification gate runs **only under [`DistortionCriterion::MinimaxRegret`]**, and the
/// asymmetry is deliberate rather than an oversight.
///
/// Choosing [`DistortionCriterion::BayesRegret`] *is* the assertion that the prior is trustworthy.
/// Under that assertion the full context always has distortion zero, so a sufficient context
/// always exists and forcing abstention because two models disagree would make the optimiser
/// unreachable — every decision worth compiling has models that disagree, or there would be
/// nothing to decide. The disagreement is still reported: [`identification`] is a separate call
/// and a caller may run it whatever criterion they optimise under.
///
/// Choosing [`DistortionCriterion::MinimaxRegret`] is the opposite assertion — that there is no
/// prior worth having over the compatible set. That is 43.50's non-identified case, and there
/// abstention is forced, because "the smallest context" is not an answer to a question whose
/// answer is unavailable at any size.
pub fn minimal_sufficient_context(
    problem: &DecisionProblem,
    prior: &Belief,
    pool: &EvidencePool,
    criterion: DistortionCriterion,
    tolerance: f64,
    compatibility_floor: f64,
) -> Result<Sufficiency, EpistemicError> {
    if !tolerance.is_finite() || tolerance < 0.0 {
        return Err(EpistemicError::InadmissibleTolerance { value: tolerance });
    }
    if criterion == DistortionCriterion::MinimaxRegret {
        let status = identification(problem, prior, pool, tolerance, compatibility_floor)?;
        if let Identification::NonIdentified { minimax_regret, .. } = status {
            return Ok(Sufficiency::Abstain {
                reason: AbstentionReason::NonIdentifiedUnderAllEvidence,
                best_distortion: minimax_regret,
                tolerance,
            });
        }
    }

    let front = frontier(problem, prior, pool, criterion, compatibility_floor)?;
    let full_rate = pool.rate(&pool.everything())?;
    match front.cheapest_within(tolerance) {
        Some(point) => Ok(Sufficiency::Sufficient {
            retained: point.retained.clone(),
            rate: point.rate,
            distortion: point.distortion,
            full_rate,
        }),
        None => Ok(Sufficiency::Abstain {
            reason: AbstentionReason::ToleranceUnattainable,
            best_distortion: front
                .points
                .iter()
                .map(|p| p.distortion)
                .fold(f64::INFINITY, f64::min),
            tolerance,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rng::SplitMix64;

    /// The pairwise dominance scan [`pareto_optimal`] replaces, transcribed unchanged.
    ///
    /// Kept as a test fixture rather than deleted: a sweep is only a valid rewrite of a rule if the
    /// rule it is a rewrite *of* is still executable and still disagrees when the sweep is wrong.
    fn pairwise_pareto(candidates: &[FrontierPoint]) -> Vec<FrontierPoint> {
        let mut points: Vec<FrontierPoint> = candidates
            .iter()
            .filter(|p| {
                !candidates.iter().any(|q| {
                    let cheaper_or_equal = q.rate <= p.rate + LOSS_EPSILON;
                    let better_or_equal = q.distortion <= p.distortion + LOSS_EPSILON;
                    let strictly_better = q.rate < p.rate - LOSS_EPSILON
                        || q.distortion < p.distortion - LOSS_EPSILON;
                    cheaper_or_equal && better_or_equal && strictly_better
                })
            })
            .cloned()
            .collect();
        points.sort_by(|a, b| {
            a.rate
                .total_cmp(&b.rate)
                .then(a.distortion.total_cmp(&b.distortion))
                .then(a.retained.cmp(&b.retained))
        });
        points.dedup_by(|a, b| {
            (a.rate - b.rate).abs() <= LOSS_EPSILON
                && (a.distortion - b.distortion).abs() <= LOSS_EPSILON
        });
        points
    }

    /// Coordinates drawn off a lattice whose spacing straddles [`LOSS_EPSILON`].
    ///
    /// Uniform floats would almost never place two candidates within the tolerance of each other,
    /// so the epsilon branches — the only part of the rule a sweep can plausibly get wrong — would
    /// never be exercised. Every gap here is one of: zero, half the tolerance, exactly the
    /// tolerance, twice it, and a gap far larger than it.
    fn lattice_value(rng: &mut SplitMix64, steps: usize) -> f64 {
        let base = rng.below(steps) as f64;
        let offsets = [
            0.0,
            LOSS_EPSILON / 2.0,
            LOSS_EPSILON,
            LOSS_EPSILON * 2.0,
            -LOSS_EPSILON / 2.0,
            -LOSS_EPSILON,
        ];
        base + offsets[rng.below(offsets.len())]
    }

    fn seeded_candidates(rng: &mut SplitMix64, count: usize, steps: usize) -> Vec<FrontierPoint> {
        (0..count)
            .map(|index| FrontierPoint {
                rate: lattice_value(rng, steps),
                distortion: lattice_value(rng, steps),
                retained: vec![index],
            })
            .collect()
    }

    #[test]
    fn the_sorted_sweep_keeps_exactly_what_the_pairwise_dominance_scan_keeps() {
        let mut rng = SplitMix64::new(0x5EED_0FA1_11E5);
        let mut examined = 0usize;
        for trial in 0..400 {
            let count = 1 + rng.below(40);
            let steps = 1 + rng.below(5);
            let candidates = seeded_candidates(&mut rng, count, steps);

            let expected = pairwise_pareto(&candidates);
            let actual = pareto_optimal(candidates.clone());

            assert_eq!(
                actual, expected,
                "trial {trial} over {count} candidates on a {steps}-step lattice diverged"
            );
            examined += count;
        }
        assert!(
            examined > 4_000,
            "only {examined} candidates were examined, too few to have hit the tolerance branches"
        );
    }

    #[test]
    fn a_domination_chain_that_the_tolerance_makes_non_transitive_is_resolved_pairwise() {
        let chain = vec![
            FrontierPoint {
                rate: 1.0,
                distortion: 1.0 + LOSS_EPSILON * 2.0,
                retained: vec![0],
            },
            FrontierPoint {
                rate: 1.0,
                distortion: 1.0 + LOSS_EPSILON,
                retained: vec![1],
            },
            FrontierPoint {
                rate: 1.0,
                distortion: 1.0,
                retained: vec![2],
            },
        ];
        assert_eq!(pareto_optimal(chain.clone()), pairwise_pareto(&chain));
    }

    #[test]
    fn a_single_candidate_dominates_nothing_including_itself() {
        let lone = vec![FrontierPoint {
            rate: 3.0,
            distortion: 0.25,
            retained: vec![0, 1],
        }];
        assert_eq!(pareto_optimal(lone.clone()), lone);
    }

    #[test]
    fn the_empty_candidate_set_yields_an_empty_frontier() {
        assert!(pareto_optimal(Vec::new()).is_empty());
    }
}
