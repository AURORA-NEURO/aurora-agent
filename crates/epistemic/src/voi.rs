//! Value of information: what an evidence action is worth before you take it.
//!
//! Blueprint 43.50 gives the definition in utilities; this is the same quantity in losses:
//!
//! ```text
//! VOI(e) = bayes_risk(π) − E_y[ bayes_risk(π | y) ] − cost(e)
//! ```
//!
//! The first term is what you expect to lose deciding now. The second is what you expect to lose
//! deciding after seeing `e`'s result, averaged over the results you might see. Their difference
//! is what the observation buys; subtracting the cost is what makes it an acquisition decision
//! rather than a curiosity.
//!
//! ## The one thing that is provable here
//!
//! **Gross value of information is never negative.** `bayes_risk` is a minimum of finitely many
//! functions that are linear in the belief, hence concave in it, and the posterior belief averages
//! back to the prior. Jensen then gives `E_y[bayes_risk(π|y)] ≤ bayes_risk(π)`. Information cannot
//! hurt a decider who is free to ignore it.
//!
//! *Net* value routinely is negative, and that is not a violation — it is the entire content of
//! 43.50's "VOI includes specimen, privacy, and human burden". [`ValueOfInformation`] keeps the two
//! numbers apart in the type so a caller cannot report the gross figure as the case for acquiring.
//!
//! The suite checks non-negativity against a seeded family rather than citing the argument above,
//! for the reason `bioprism-influence` gives about its own bounds: an argument nobody ran is a
//! claim, not a guarantee.
//!
//! ## The thing that is emphatically not provable
//!
//! Value of information is **not submodular in general**, and not even for well-behaved decision
//! problems. Two assays can each be worth nothing alone and everything together — 43.14's own
//! worked micro-example is exactly this, tumour purity and copy number. [`joint_value`] prices
//! bundles for that reason, and [`crate::submodularity`] finds the violation by exhaustion rather
//! than taking anyone's word for it.
//!
//! ## What is not modelled
//!
//! Sequential and adaptive acquisition. A real planner chooses the second assay after seeing the
//! first result, and its value is an expectation over adaptive policies, not over joint outcomes
//! of a fixed set. [`joint_value`] prices the *non-adaptive* bundle, which is a lower bound on the
//! adaptive value. 43.15 (information-directed acquisition) is the module that would own the
//! adaptive case and this crate does not implement it.

use crate::decision::{Belief, DecisionProblem};
use crate::error::EpistemicError;
use crate::evidence::Acquisition;
use serde::{Deserialize, Serialize};

/// The largest joint outcome space [`joint_value`] will enumerate.
pub const MAX_JOINT_OUTCOMES: u64 = 1 << 16;

/// What an evidence action is worth, before and after its cost.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ValueOfInformation {
    /// Expected reduction in Bayes risk. Provably non-negative.
    pub gross: f64,
    /// Token, compute, latency, privacy, specimen and expert burden, as declared.
    pub cost: f64,
    /// `gross − cost`. Negative means the acquisition is not worth taking.
    pub net: f64,
    /// Marginal probability of each outcome under the current belief, in declaration order.
    pub outcome_probabilities: Vec<f64>,
    /// The action taken if nothing further is acquired.
    pub action_without: usize,
    /// The action taken after each outcome, in declaration order.
    pub action_after: Vec<usize>,
}

impl ValueOfInformation {
    /// Whether the acquisition can change what is done.
    ///
    /// An action-invariant acquisition has gross value exactly zero, and 43.50's failure mode "no
    /// feasible informative action yields abstention" is decided on this rather than on whether
    /// the number rounds to zero.
    pub fn changes_the_action(&self) -> bool {
        self.action_after.iter().any(|a| *a != self.action_without)
    }

    pub fn worth_acquiring(&self) -> bool {
        self.net > 0.0
    }
}

/// Prices one evidence action against the current belief.
pub fn value_of_information(
    problem: &DecisionProblem,
    belief: &Belief,
    acquisition: &Acquisition,
) -> Result<ValueOfInformation, EpistemicError> {
    belief.check_against(problem)?;
    let before = problem.bayes_risk(belief);
    let action_without = problem.bayes_action(belief);

    let mut probabilities = Vec::with_capacity(acquisition.outcomes().len());
    let mut actions = Vec::with_capacity(acquisition.outcomes().len());
    let mut expected_after = 0.0f64;

    for outcome in acquisition.outcomes() {
        let joint: Vec<f64> = (0..problem.model_count())
            .map(|m| belief.mass(m) * outcome.likelihood(m))
            .collect();
        let probability: f64 = joint.iter().sum();
        probabilities.push(probability);
        if probability <= 0.0 {
            actions.push(action_without);
            continue;
        }
        let posterior = Belief::new(joint)?;
        actions.push(problem.bayes_action(&posterior));
        expected_after += probability * problem.bayes_risk(&posterior);
    }

    let gross = before - expected_after;
    Ok(ValueOfInformation {
        gross,
        cost: acquisition.cost,
        net: gross - acquisition.cost,
        outcome_probabilities: probabilities,
        action_without,
        action_after: actions,
    })
}

/// Prices a *bundle* of evidence actions taken together, non-adaptively.
///
/// Exists because 43.14's fallback for detected complementarity is to "evaluate the pair as a
/// bundle instead of pruning both", and a bundle cannot be priced by summing its members: the
/// whole reason to bundle is that the sum is wrong.
///
/// Enumerates the cartesian product of outcomes, treating the actions as conditionally independent
/// given the model. That independence is an assumption of this function, not a theorem, and it is
/// the assumption a caller with correlated assays would have to reconsider.
pub fn joint_value(
    problem: &DecisionProblem,
    belief: &Belief,
    acquisitions: &[Acquisition],
) -> Result<ValueOfInformation, EpistemicError> {
    belief.check_against(problem)?;
    if acquisitions.is_empty() {
        return Ok(ValueOfInformation {
            gross: 0.0,
            cost: 0.0,
            net: 0.0,
            outcome_probabilities: Vec::new(),
            action_without: problem.bayes_action(belief),
            action_after: Vec::new(),
        });
    }

    let mut total: u64 = 1;
    for acquisition in acquisitions {
        total = total.saturating_mul(acquisition.outcomes().len() as u64);
        if total > MAX_JOINT_OUTCOMES {
            return Err(EpistemicError::ExhaustiveCapExceeded {
                ground: acquisitions.len(),
                needed: total,
                cap: MAX_JOINT_OUTCOMES,
            });
        }
    }

    let before = problem.bayes_risk(belief);
    let action_without = problem.bayes_action(belief);
    let mut probabilities = Vec::with_capacity(total as usize);
    let mut actions = Vec::with_capacity(total as usize);
    let mut expected_after = 0.0f64;

    for index in 0..total {
        let mut remaining = index;
        let mut joint: Vec<f64> = belief.masses().to_vec();
        for acquisition in acquisitions {
            let width = acquisition.outcomes().len() as u64;
            let choice = (remaining % width) as usize;
            remaining /= width;
            let outcome = &acquisition.outcomes()[choice];
            for (model, value) in joint.iter_mut().enumerate() {
                *value *= outcome.likelihood(model);
            }
        }
        let probability: f64 = joint.iter().sum();
        probabilities.push(probability);
        if probability <= 0.0 {
            actions.push(action_without);
            continue;
        }
        let posterior = Belief::new(joint)?;
        actions.push(problem.bayes_action(&posterior));
        expected_after += probability * problem.bayes_risk(&posterior);
    }

    let cost: f64 = acquisitions.iter().map(|a| a.cost).sum();
    let gross = before - expected_after;
    Ok(ValueOfInformation {
        gross,
        cost,
        net: gross - cost,
        outcome_probabilities: probabilities,
        action_without,
        action_after: actions,
    })
}

/// How far a bundle's value departs from the sum of its members' values.
///
/// Positive means complementarity — the pair is worth more than its parts, which is exactly the
/// condition under which a greedy selector prunes both and 43.14 requires switching planner.
/// Negative means redundancy, the diminishing-returns case greedy is built for.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Complementarity {
    pub joint_gross: f64,
    pub sum_of_singletons: f64,
    /// `joint_gross − sum_of_singletons`.
    pub excess: f64,
}

impl Complementarity {
    /// Whether the bundle is worth strictly more than its members priced separately.
    pub fn is_complementary(&self) -> bool {
        self.excess > crate::decision::LOSS_EPSILON
    }
}

/// Measures complementarity of a bundle against the current belief.
pub fn complementarity(
    problem: &DecisionProblem,
    belief: &Belief,
    acquisitions: &[Acquisition],
) -> Result<Complementarity, EpistemicError> {
    let joint = joint_value(problem, belief, acquisitions)?;
    let mut sum = 0.0;
    for acquisition in acquisitions {
        sum += value_of_information(problem, belief, acquisition)?.gross;
    }
    Ok(Complementarity {
        joint_gross: joint.gross,
        sum_of_singletons: sum,
        excess: joint.gross - sum,
    })
}
