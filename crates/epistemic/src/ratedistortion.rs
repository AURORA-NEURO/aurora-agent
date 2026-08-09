//! Causal decision rate–distortion: blueprint 43.50, and the part of 43.12 it generalises.
//!
//! `bioprism-examples` records `rate_distortion_optimisation` as a blocked claim, and names the
//! obstacle: *"the same missing decision_loss field: there is no loss to trade distortion against"*.
//! This module supplies the calculus with the loss supplied by the caller. [`crate::gap`] states
//! what `fiber-query/0.1` would have to carry for the compiler to reach it without one.
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
    prior.check_against(problem)?;
    pool.check_against(problem)?;

    let full = pool.full_posterior(prior)?;
    let reference_action = problem.bayes_action(&full);
    let compressed = pool.posterior(prior, subset)?;
    let compatible = full.support_above(compatibility_floor);

    let (action, distortion) = match criterion {
        DistortionCriterion::BayesRegret => {
            let action = problem.bayes_action(&compressed);
            (action, problem.regret(&full, action))
        }
        DistortionCriterion::MinimaxRegret => {
            let compressed_compatible = compressed.support_above(compatibility_floor);
            let action = problem
                .minimax_action(&compressed_compatible)
                .unwrap_or_else(|| problem.bayes_action(&compressed));
            (action, problem.minimax_regret(&compatible, action))
        }
    };

    Ok(ContextEvaluation {
        retained: subset.iter().copied().collect(),
        rate: pool.rate(subset)?,
        distortion,
        action,
        reference_action,
        compatible,
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

    let mut candidates: Vec<FrontierPoint> = Vec::new();
    for mask in 0..needed {
        let subset: BTreeSet<usize> = (0..n).filter(|i| (mask >> i) & 1 == 1).collect();
        let evaluation = evaluate_context(
            problem,
            prior,
            pool,
            &subset,
            criterion,
            compatibility_floor,
        )?;
        candidates.push(FrontierPoint {
            rate: evaluation.rate,
            distortion: evaluation.distortion,
            retained: evaluation.retained,
        });
    }

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

    Ok(Frontier {
        points,
        criterion,
        evaluated: needed as usize,
    })
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
    PointIdentified { action: usize, compatible: Vec<usize> },
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
