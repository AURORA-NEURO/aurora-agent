//! Explicit multidimensional acquisition costs and exact bounded planning.
//!
//! Blueprint 43.14 names cost as a vector rather than a single price. A token-heavy literature
//! query, a slow assay, a privacy-sensitive join, and an expert review are not interchangeable
//! merely because a caller can assign them the same number. This module keeps seven dimensions in
//! the input and budget, then requires a separately supplied weight vector only for the objective
//! used to compare feasible policies.
//!
//! [`adaptive_policy_with_cost_vectors`] is the vector counterpart to
//! [`crate::adaptive_policy`]. It enumerates the same finite policy space, rejects any branch that
//! exceeds any component of the remaining budget, and reports expected cost as both the full
//! vector and its explicit scalarization. It therefore never turns an infeasible specimen or
//! privacy budget into an apparently affordable plan by hiding it inside a weighted sum.

use crate::decision::{Belief, DecisionProblem, LOSS_EPSILON};
use crate::error::EpistemicError;
use crate::evidence::Acquisition;
use serde::{Deserialize, Serialize};

/// Names and order of the cost vector dimensions on every wire surface.
pub const COST_DIMENSIONS: [&str; 7] = [
    "tokens",
    "compute_ms",
    "latency_ms",
    "money_usd",
    "privacy_loss",
    "specimen_units",
    "expert_minutes",
];

/// A non-negative resource vector. Dimensions are intentionally not unit-erased.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "CostVectorWire", into = "CostVectorWire")]
pub struct CostVector {
    tokens: f64,
    compute_ms: f64,
    latency_ms: f64,
    money_usd: f64,
    privacy_loss: f64,
    specimen_units: f64,
    expert_minutes: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
struct CostVectorWire {
    tokens: f64,
    compute_ms: f64,
    latency_ms: f64,
    money_usd: f64,
    privacy_loss: f64,
    specimen_units: f64,
    expert_minutes: f64,
}

impl TryFrom<CostVectorWire> for CostVector {
    type Error = String;

    fn try_from(value: CostVectorWire) -> Result<Self, Self::Error> {
        CostVector::new(
            value.tokens,
            value.compute_ms,
            value.latency_ms,
            value.money_usd,
            value.privacy_loss,
            value.specimen_units,
            value.expert_minutes,
        )
        .map_err(|error| error.to_string())
    }
}

impl From<CostVector> for CostVectorWire {
    fn from(value: CostVector) -> Self {
        CostVectorWire {
            tokens: value.tokens,
            compute_ms: value.compute_ms,
            latency_ms: value.latency_ms,
            money_usd: value.money_usd,
            privacy_loss: value.privacy_loss,
            specimen_units: value.specimen_units,
            expert_minutes: value.expert_minutes,
        }
    }
}

impl CostVector {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        tokens: f64,
        compute_ms: f64,
        latency_ms: f64,
        money_usd: f64,
        privacy_loss: f64,
        specimen_units: f64,
        expert_minutes: f64,
    ) -> Result<Self, EpistemicError> {
        let values = [
            ("tokens", tokens),
            ("compute_ms", compute_ms),
            ("latency_ms", latency_ms),
            ("money_usd", money_usd),
            ("privacy_loss", privacy_loss),
            ("specimen_units", specimen_units),
            ("expert_minutes", expert_minutes),
        ];
        for (dimension, value) in values {
            if !value.is_finite() || value < 0.0 {
                return Err(EpistemicError::InadmissibleCost {
                    item: dimension.into(),
                    value,
                });
            }
        }
        Ok(CostVector {
            tokens,
            compute_ms,
            latency_ms,
            money_usd,
            privacy_loss,
            specimen_units,
            expert_minutes,
        })
    }

    pub const fn zero() -> Self {
        CostVector {
            tokens: 0.0,
            compute_ms: 0.0,
            latency_ms: 0.0,
            money_usd: 0.0,
            privacy_loss: 0.0,
            specimen_units: 0.0,
            expert_minutes: 0.0,
        }
    }

    pub fn tokens(&self) -> f64 {
        self.tokens
    }

    pub fn compute_ms(&self) -> f64 {
        self.compute_ms
    }

    pub fn latency_ms(&self) -> f64 {
        self.latency_ms
    }

    pub fn money_usd(&self) -> f64 {
        self.money_usd
    }

    pub fn privacy_loss(&self) -> f64 {
        self.privacy_loss
    }

    pub fn specimen_units(&self) -> f64 {
        self.specimen_units
    }

    pub fn expert_minutes(&self) -> f64 {
        self.expert_minutes
    }

    pub fn components(&self) -> [f64; 7] {
        [
            self.tokens,
            self.compute_ms,
            self.latency_ms,
            self.money_usd,
            self.privacy_loss,
            self.specimen_units,
            self.expert_minutes,
        ]
    }

    pub fn checked_add(self, other: Self) -> Result<Self, EpistemicError> {
        let values = self
            .components()
            .into_iter()
            .zip(other.components())
            .map(|(left, right)| left + right)
            .collect::<Vec<_>>();
        CostVector::new(
            values[0], values[1], values[2], values[3], values[4], values[5], values[6],
        )
    }

    pub fn scale(self, probability: f64) -> Result<Self, EpistemicError> {
        if !probability.is_finite() || probability < 0.0 {
            return Err(EpistemicError::InadmissibleCost {
                item: "probability-scaled cost".into(),
                value: probability,
            });
        }
        let values = self.components().map(|value| value * probability);
        CostVector::new(
            values[0], values[1], values[2], values[3], values[4], values[5], values[6],
        )
    }

    /// Whether every component fits inside `budget`, with a numerical reconciliation slack.
    pub fn fits_within(&self, budget: &Self) -> bool {
        self.components()
            .into_iter()
            .zip(budget.components())
            .all(|(value, limit)| value <= limit + LOSS_EPSILON)
    }

    pub fn remaining_after(self, spent: Self) -> Result<Self, EpistemicError> {
        let values = self
            .components()
            .into_iter()
            .zip(spent.components())
            .map(|(limit, value)| (limit - value).max(0.0))
            .collect::<Vec<_>>();
        CostVector::new(
            values[0], values[1], values[2], values[3], values[4], values[5], values[6],
        )
    }
}

/// Explicit weights for the scalar objective used to compare otherwise feasible vectors.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "CostWeightsWire", into = "CostWeightsWire")]
pub struct CostWeights {
    tokens: f64,
    compute_ms: f64,
    latency_ms: f64,
    money_usd: f64,
    privacy_loss: f64,
    specimen_units: f64,
    expert_minutes: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
struct CostWeightsWire {
    tokens: f64,
    compute_ms: f64,
    latency_ms: f64,
    money_usd: f64,
    privacy_loss: f64,
    specimen_units: f64,
    expert_minutes: f64,
}

impl TryFrom<CostWeightsWire> for CostWeights {
    type Error = String;

    fn try_from(value: CostWeightsWire) -> Result<Self, Self::Error> {
        CostWeights::new(
            value.tokens,
            value.compute_ms,
            value.latency_ms,
            value.money_usd,
            value.privacy_loss,
            value.specimen_units,
            value.expert_minutes,
        )
        .map_err(|error| error.to_string())
    }
}

impl From<CostWeights> for CostWeightsWire {
    fn from(value: CostWeights) -> Self {
        CostWeightsWire {
            tokens: value.tokens,
            compute_ms: value.compute_ms,
            latency_ms: value.latency_ms,
            money_usd: value.money_usd,
            privacy_loss: value.privacy_loss,
            specimen_units: value.specimen_units,
            expert_minutes: value.expert_minutes,
        }
    }
}

impl CostWeights {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        tokens: f64,
        compute_ms: f64,
        latency_ms: f64,
        money_usd: f64,
        privacy_loss: f64,
        specimen_units: f64,
        expert_minutes: f64,
    ) -> Result<Self, EpistemicError> {
        let values = [
            ("tokens", tokens),
            ("compute_ms", compute_ms),
            ("latency_ms", latency_ms),
            ("money_usd", money_usd),
            ("privacy_loss", privacy_loss),
            ("specimen_units", specimen_units),
            ("expert_minutes", expert_minutes),
        ];
        for (dimension, value) in values {
            if !value.is_finite() || value < 0.0 {
                return Err(EpistemicError::InadmissibleCost {
                    item: format!("weight/{dimension}"),
                    value,
                });
            }
        }
        if values.iter().all(|(_, value)| *value == 0.0) {
            return Err(EpistemicError::InadmissibleCost {
                item: "cost weights".into(),
                value: 0.0,
            });
        }
        Ok(CostWeights {
            tokens,
            compute_ms,
            latency_ms,
            money_usd,
            privacy_loss,
            specimen_units,
            expert_minutes,
        })
    }

    pub fn components(&self) -> [f64; 7] {
        [
            self.tokens,
            self.compute_ms,
            self.latency_ms,
            self.money_usd,
            self.privacy_loss,
            self.specimen_units,
            self.expert_minutes,
        ]
    }

    pub fn scalarize(&self, cost: CostVector) -> f64 {
        self.components()
            .into_iter()
            .zip(cost.components())
            .map(|(weight, value)| weight * value)
            .sum()
    }
}

/// An acquisition with an auditable vector cost. Its likelihood partition remains the ordinary
/// [`Acquisition`] contract, so existing epistemic callers can migrate one action at a time.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CostedAcquisition {
    pub acquisition: Acquisition,
    pub cost: CostVector,
}

impl CostedAcquisition {
    pub fn new(acquisition: Acquisition, cost: CostVector) -> Self {
        CostedAcquisition { acquisition, cost }
    }
}

/// A branch in a vector-budget adaptive policy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CostedAdaptiveOutcome {
    pub label: String,
    pub probability: f64,
    pub posterior: Vec<f64>,
    pub next: Box<CostedAdaptiveNode>,
}

/// A node in a vector-budget adaptive policy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum CostedAdaptiveNode {
    Stop {
        action: usize,
        risk: f64,
    },
    Acquire {
        acquisition: usize,
        id: String,
        cost: CostVector,
        scalarized_cost: f64,
        expected_total: f64,
        expected_terminal_risk: f64,
        expected_acquisition_cost: CostVector,
        outcomes: Vec<CostedAdaptiveOutcome>,
    },
}

/// Exact finite-horizon policy output under vector feasibility and explicit scalarization.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CostedAdaptivePolicy {
    pub expected_total: f64,
    pub expected_terminal_risk: f64,
    pub expected_acquisition_cost: CostVector,
    pub expected_scalarized_cost: f64,
    pub nodes_evaluated: usize,
    pub selected_depth: usize,
    pub budget: CostVector,
    pub weights: CostWeights,
    pub root: CostedAdaptiveNode,
}

/// Exact vector-feasible adaptive planning under the same finite caps as the scalar planner.
pub fn adaptive_policy_with_cost_vectors(
    problem: &DecisionProblem,
    belief: &Belief,
    acquisitions: &[CostedAcquisition],
    budget: CostVector,
    weights: CostWeights,
    max_steps: usize,
) -> Result<CostedAdaptivePolicy, EpistemicError> {
    problem.validate()?;
    belief.check_against(problem)?;
    if max_steps > 16 {
        return Err(EpistemicError::AdaptiveStepLimit {
            steps: max_steps,
            cap: 16,
        });
    }
    if acquisitions.is_empty() || acquisitions.len() > 16 {
        return Err(EpistemicError::AdaptiveAcquisitionCapExceeded {
            acquisitions: acquisitions.len(),
            cap: 16,
        });
    }
    let ids: Vec<String> = acquisitions
        .iter()
        .map(|item| item.acquisition.id.clone())
        .collect();
    crate::unique(&ids, "vector-cost adaptive acquisitions")?;
    for item in acquisitions {
        item.acquisition.check_against(problem)?;
    }
    let mut nodes_evaluated = 0;
    let selected = evaluate(
        problem,
        belief,
        acquisitions,
        (1u32 << acquisitions.len()) - 1,
        budget,
        weights,
        max_steps,
        0,
        &mut nodes_evaluated,
    )?;
    Ok(CostedAdaptivePolicy {
        expected_total: selected.total,
        expected_terminal_risk: selected.terminal_risk,
        expected_acquisition_cost: selected.acquisition_cost,
        expected_scalarized_cost: selected.scalarized_cost,
        nodes_evaluated,
        selected_depth: selected.depth,
        budget,
        weights,
        root: selected.node,
    })
}

#[derive(Debug, Clone)]
struct Evaluation {
    total: f64,
    terminal_risk: f64,
    acquisition_cost: CostVector,
    scalarized_cost: f64,
    depth: usize,
    node: CostedAdaptiveNode,
}

fn evaluate(
    problem: &DecisionProblem,
    belief: &Belief,
    acquisitions: &[CostedAcquisition],
    remaining: u32,
    budget: CostVector,
    weights: CostWeights,
    steps_left: usize,
    depth: usize,
    nodes_evaluated: &mut usize,
) -> Result<Evaluation, EpistemicError> {
    *nodes_evaluated =
        nodes_evaluated
            .checked_add(1)
            .ok_or(EpistemicError::AdaptivePolicyCapExceeded {
                nodes: usize::MAX,
                cap: 65_536,
            })?;
    if *nodes_evaluated > 65_536 {
        return Err(EpistemicError::AdaptivePolicyCapExceeded {
            nodes: *nodes_evaluated,
            cap: 65_536,
        });
    }
    let stop_action = problem.bayes_action(belief);
    let stop_risk = problem.bayes_risk(belief);
    let mut best = Evaluation {
        total: stop_risk,
        terminal_risk: stop_risk,
        acquisition_cost: CostVector::zero(),
        scalarized_cost: 0.0,
        depth,
        node: CostedAdaptiveNode::Stop {
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
        if !acquisition.cost.fits_within(&budget) {
            continue;
        }
        let next_budget = budget.remaining_after(acquisition.cost)?;
        let scalarized_cost = weights.scalarize(acquisition.cost);
        let mut expected_total = scalarized_cost;
        let mut expected_terminal_risk = 0.0;
        let mut expected_acquisition_cost = acquisition.cost;
        let mut outcomes = Vec::with_capacity(acquisition.acquisition.outcomes().len());
        let mut selected_depth = depth + 1;
        for outcome in acquisition.acquisition.outcomes() {
            let joint: Vec<f64> = (0..problem.model_count())
                .map(|model| belief.mass(model) * outcome.likelihood(model))
                .collect();
            let probability: f64 = joint.iter().sum();
            if probability <= 0.0 {
                outcomes.push(CostedAdaptiveOutcome {
                    label: outcome.label.clone(),
                    probability: 0.0,
                    posterior: belief.masses().to_vec(),
                    next: Box::new(CostedAdaptiveNode::Stop {
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
                next_budget,
                weights,
                steps_left - 1,
                depth + 1,
                nodes_evaluated,
            )?;
            expected_total += probability * child.total;
            expected_terminal_risk += probability * child.terminal_risk;
            expected_acquisition_cost = expected_acquisition_cost
                .checked_add(child.acquisition_cost.scale(probability)?)?;
            selected_depth = selected_depth.max(child.depth);
            outcomes.push(CostedAdaptiveOutcome {
                label: outcome.label.clone(),
                probability,
                posterior: posterior.masses().to_vec(),
                next: Box::new(child.node),
            });
        }
        let candidate = Evaluation {
            total: expected_total,
            terminal_risk: expected_terminal_risk,
            scalarized_cost: weights.scalarize(expected_acquisition_cost),
            acquisition_cost: expected_acquisition_cost,
            depth: selected_depth,
            node: CostedAdaptiveNode::Acquire {
                acquisition: index,
                id: acquisition.acquisition.id.clone(),
                cost: acquisition.cost,
                scalarized_cost,
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

/// Converts a scalar policy into a zero-dimensional compatibility report for migration tooling.
///
/// This does not claim that a scalar policy satisfies a vector budget; callers must use
/// [`adaptive_policy_with_cost_vectors`] when component feasibility matters. The function exists
/// to make that distinction explicit in code instead of silently attaching invented dimensions.
pub fn scalar_policy_cost_note() -> &'static str {
    "scalar adaptive policies carry no component-wise feasibility claim; use vector-cost planning for multidimensional budgets"
}

#[cfg(test)]
mod tests {
    use super::*;

    fn problem() -> DecisionProblem {
        DecisionProblem::new(
            vec!["choose-m0".into(), "choose-m1".into()],
            vec!["m0".into(), "m1".into()],
            vec![0.0, 1.0, 1.0, 0.0],
        )
        .unwrap()
    }

    fn weights() -> CostWeights {
        CostWeights::new(1.0, 0.001, 0.001, 10.0, 100.0, 5.0, 0.5).unwrap()
    }

    #[test]
    fn vectors_keep_dimensions_separate_and_round_trip_through_serde() {
        let vector = CostVector::new(10.0, 2.0, 4.0, 0.5, 0.1, 1.0, 30.0).unwrap();
        let raw = serde_json::to_string(&vector).unwrap();
        let decoded: CostVector = serde_json::from_str(&raw).unwrap();
        assert_eq!(decoded, vector);
        assert_eq!(vector.components().len(), COST_DIMENSIONS.len());
        assert!(!CostVector::new(10.0, 2.0, 4.0, 0.5, 0.1, 2.0, 30.0)
            .unwrap()
            .fits_within(&vector));
    }

    #[test]
    fn vector_budget_refuses_a_policy_that_only_scalarization_would_allow() {
        let acquisition = CostedAcquisition::new(
            Acquisition::binary("screen", 0.01, vec![0.9, 0.2]).unwrap(),
            CostVector::new(1.0, 1.0, 100.0, 0.0, 0.0, 0.0, 0.0).unwrap(),
        );
        let policy = adaptive_policy_with_cost_vectors(
            &problem(),
            &Belief::new(vec![0.9, 0.1]).unwrap(),
            &[acquisition],
            CostVector::new(100.0, 100.0, 10.0, 100.0, 100.0, 100.0, 100.0).unwrap(),
            weights(),
            1,
        )
        .unwrap();
        assert!(matches!(policy.root, CostedAdaptiveNode::Stop { .. }));
    }

    #[test]
    fn vector_policy_reports_expected_vector_and_auditable_scalarization() {
        let acquisition = CostedAcquisition::new(
            Acquisition::binary("screen", 0.01, vec![0.9, 0.2]).unwrap(),
            CostVector::new(2.0, 3.0, 4.0, 0.5, 0.1, 0.0, 5.0).unwrap(),
        );
        let weights = weights();
        let policy = adaptive_policy_with_cost_vectors(
            &problem(),
            &Belief::new(vec![0.5, 0.5]).unwrap(),
            &[acquisition],
            CostVector::new(100.0, 100.0, 100.0, 100.0, 100.0, 100.0, 100.0).unwrap(),
            weights,
            1,
        )
        .unwrap();
        assert_eq!(
            policy.expected_scalarized_cost,
            weights.scalarize(policy.expected_acquisition_cost)
        );
        assert!(policy.expected_total.is_finite());
    }
}
