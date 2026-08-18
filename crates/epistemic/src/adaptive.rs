//! Exact finite-horizon adaptive acquisition policies.
//!
//! [`crate::voi`] prices one action or a fixed bundle before any result is known. This module
//! closes the next decision boundary: after an outcome, the planner may choose a different
//! remaining acquisition, stop, or abstain from spending the remaining budget. The policy is
//! still a caller-declared calculation, not an execution engine. It assumes conditional
//! independence of each acquisition's outcomes given the compatible model and never claims that
//! a candidate was run.
//!
//! The planner minimizes expected terminal Bayes risk plus declared acquisition cost. It
//! enumerates every reachable branch and every affordable unused acquisition, with explicit caps
//! on acquisition count, horizon, and evaluated policy states. A cap is a refusal, never a
//! sampled policy wearing an optimality label.

use crate::decision::{Belief, DecisionProblem, LOSS_EPSILON};
use crate::error::EpistemicError;
use crate::evidence::Acquisition;
use serde::{Deserialize, Serialize};

/// Maximum number of distinct acquisitions in an exact policy state mask.
pub const MAX_ADAPTIVE_ACQUISITIONS: usize = 16;
/// Maximum number of acquisition decisions on any policy path.
pub const MAX_ADAPTIVE_STEPS: usize = 16;
/// Maximum number of reachable state evaluations, including rejected alternatives.
pub const MAX_ADAPTIVE_POLICY_NODES: usize = 65_536;

/// One result branch in a selected adaptive policy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdaptiveOutcome {
    /// The acquisition-declared result label.
    pub label: String,
    /// Probability of this result under the belief at the parent node.
    pub probability: f64,
    /// Posterior model masses after conditioning on this result.
    pub posterior: Vec<f64>,
    /// The next policy decision. Zero-probability branches carry a deterministic stop node.
    pub next: Box<AdaptiveNode>,
}

/// A node in an exact adaptive policy tree.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum AdaptiveNode {
    /// Stop and take the named action under the current posterior.
    Stop { action: usize, risk: f64 },
    /// Acquire one unused action, then branch on every declared outcome.
    Acquire {
        acquisition: usize,
        id: String,
        cost: f64,
        /// Expected terminal objective from this node, including all downstream costs.
        expected_total: f64,
        expected_terminal_risk: f64,
        expected_acquisition_cost: f64,
        outcomes: Vec<AdaptiveOutcome>,
    },
}

/// The exact adaptive policy and its objective decomposition.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AdaptivePolicy {
    /// Expected terminal Bayes risk plus every declared cost paid on the path.
    pub expected_total: f64,
    /// Expected terminal Bayes risk alone.
    pub expected_terminal_risk: f64,
    /// Expected declared acquisition cost alone.
    pub expected_acquisition_cost: f64,
    /// Number of state evaluations used while comparing policy alternatives.
    pub nodes_evaluated: usize,
    /// Maximum number of acquisitions on the selected policy path.
    pub selected_depth: usize,
    /// Root decision of the selected policy.
    pub root: AdaptiveNode,
}

#[derive(Debug, Clone)]
struct Evaluation {
    total: f64,
    terminal_risk: f64,
    acquisition_cost: f64,
    depth: usize,
    node: AdaptiveNode,
}

/// Compute an exact adaptive acquisition policy under an explicit budget and finite horizon.
///
/// Each acquisition may be used at most once. Outcome likelihoods are treated as conditionally
/// independent across acquisitions given the model, the same assumption used by
/// [`crate::voi::joint_value`]. The planner is conservative on ties: it stops rather than paying
/// a cost for an outcome-equivalent policy.
pub fn adaptive_policy(
    problem: &DecisionProblem,
    belief: &Belief,
    acquisitions: &[Acquisition],
    budget: f64,
    max_steps: usize,
) -> Result<AdaptivePolicy, EpistemicError> {
    problem.validate()?;
    belief.check_against(problem)?;
    if !budget.is_finite() || budget < 0.0 {
        return Err(EpistemicError::InadmissibleAdaptiveBudget { value: budget });
    }
    if max_steps > MAX_ADAPTIVE_STEPS {
        return Err(EpistemicError::AdaptiveStepLimit {
            steps: max_steps,
            cap: MAX_ADAPTIVE_STEPS,
        });
    }
    if acquisitions.is_empty() {
        return Err(EpistemicError::OutcomelessAcquisition {
            action: "adaptive policy".to_string(),
        });
    }
    if acquisitions.len() > MAX_ADAPTIVE_ACQUISITIONS {
        return Err(EpistemicError::AdaptiveAcquisitionCapExceeded {
            acquisitions: acquisitions.len(),
            cap: MAX_ADAPTIVE_ACQUISITIONS,
        });
    }
    let ids: Vec<String> = acquisitions.iter().map(|item| item.id.clone()).collect();
    crate::unique(&ids, "adaptive acquisitions")?;
    for acquisition in acquisitions {
        acquisition.check_against(problem)?;
    }

    let mask = (1u32 << acquisitions.len()) - 1;
    let mut nodes_evaluated = 0usize;
    let selected = evaluate(
        problem,
        belief,
        acquisitions,
        mask,
        budget,
        max_steps,
        0,
        &mut nodes_evaluated,
    )?;
    Ok(AdaptivePolicy {
        expected_total: selected.total,
        expected_terminal_risk: selected.terminal_risk,
        expected_acquisition_cost: selected.acquisition_cost,
        nodes_evaluated,
        selected_depth: selected.depth,
        root: selected.node,
    })
}

fn evaluate(
    problem: &DecisionProblem,
    belief: &Belief,
    acquisitions: &[Acquisition],
    remaining: u32,
    budget: f64,
    steps_left: usize,
    depth: usize,
    nodes_evaluated: &mut usize,
) -> Result<Evaluation, EpistemicError> {
    *nodes_evaluated =
        (*nodes_evaluated)
            .checked_add(1)
            .ok_or(EpistemicError::AdaptivePolicyCapExceeded {
                nodes: usize::MAX,
                cap: MAX_ADAPTIVE_POLICY_NODES,
            })?;
    if *nodes_evaluated > MAX_ADAPTIVE_POLICY_NODES {
        return Err(EpistemicError::AdaptivePolicyCapExceeded {
            nodes: *nodes_evaluated,
            cap: MAX_ADAPTIVE_POLICY_NODES,
        });
    }

    let stop_action = problem.bayes_action(belief);
    let stop_risk = problem.bayes_risk(belief);
    let mut best = Evaluation {
        total: stop_risk,
        terminal_risk: stop_risk,
        acquisition_cost: 0.0,
        depth,
        node: AdaptiveNode::Stop {
            action: stop_action,
            risk: stop_risk,
        },
    };
    if steps_left == 0 || remaining == 0 {
        return Ok(best);
    }

    for index in 0..acquisitions.len() {
        let bit = 1u32 << index;
        if remaining & bit == 0 {
            continue;
        }
        let acquisition = &acquisitions[index];
        if acquisition.cost > budget {
            continue;
        }

        let mut expected_total = acquisition.cost;
        let mut expected_terminal_risk = 0.0;
        let mut expected_acquisition_cost = acquisition.cost;
        let mut outcomes = Vec::with_capacity(acquisition.outcomes().len());
        let mut selected_depth = depth + 1;
        for outcome in acquisition.outcomes() {
            let joint: Vec<f64> = (0..problem.model_count())
                .map(|model| belief.mass(model) * outcome.likelihood(model))
                .collect();
            let probability: f64 = joint.iter().sum();
            if probability <= 0.0 {
                outcomes.push(AdaptiveOutcome {
                    label: outcome.label.clone(),
                    probability: 0.0,
                    posterior: belief.masses().to_vec(),
                    next: Box::new(AdaptiveNode::Stop {
                        action: stop_action,
                        risk: stop_risk,
                    }),
                });
                continue;
            }
            let posterior = Belief::new(joint)?;
            let child = evaluate(
                problem,
                &posterior,
                acquisitions,
                remaining ^ bit,
                (budget - acquisition.cost).max(0.0),
                steps_left - 1,
                depth + 1,
                nodes_evaluated,
            )?;
            expected_total += probability * child.total;
            expected_terminal_risk += probability * child.terminal_risk;
            expected_acquisition_cost += probability * child.acquisition_cost;
            selected_depth = selected_depth.max(child.depth);
            outcomes.push(AdaptiveOutcome {
                label: outcome.label.clone(),
                probability,
                posterior: posterior.masses().to_vec(),
                next: Box::new(child.node),
            });
        }
        let candidate = Evaluation {
            total: expected_total,
            terminal_risk: expected_terminal_risk,
            acquisition_cost: expected_acquisition_cost,
            depth: selected_depth,
            node: AdaptiveNode::Acquire {
                acquisition: index,
                id: acquisition.id.clone(),
                cost: acquisition.cost,
                expected_total,
                expected_terminal_risk,
                expected_acquisition_cost,
                outcomes,
            },
        };
        if candidate.total < best.total - LOSS_EPSILON {
            best = candidate;
        }
    }
    Ok(best)
}
