//! Evidence that has been observed, and evidence actions that have not.
//!
//! The distinction runs through blueprint 43.50 and it is the reason this module has two types
//! instead of one.
//!
//! An [`EvidenceItem`] has already been observed. Its value is known; what is uncertain is which
//! model generated it. Including it in a context conditions the belief deterministically. This is
//! the object rate–distortion compresses: a context is a *subset of already-available evidence*,
//! and the question is which ones can be dropped.
//!
//! An [`Acquisition`] has not been performed. Its outcome is a random variable, and its worth is
//! an expectation over outcomes the caller has not seen. This is the object value of information
//! prices, and it is why [`crate::voi`] takes an `Acquisition` and [`crate::ratedistortion`] takes
//! an `EvidencePool`. Conflating them produces the most common error in this area: valuing an
//! observation you already made, which is always worth exactly zero more than having made it.
//!
//! ## Likelihoods, not scores
//!
//! Both types carry a likelihood profile over the problem's models. That is a stronger demand than
//! a relevance score, and it is deliberate: 43.14 requires that utility include "decision
//! relevance, not embedding similarity alone", and a likelihood is the only thing that composes
//! into a posterior. A caller who has only a similarity score does not have an input to this
//! crate, and there is no constructor that pretends otherwise.
//!
//! Likelihoods are *unnormalised* across items — `ℓ_f(m) = P(observed value of f | m)` is a
//! density or a probability depending on the item, and only its ratio across models is used. What
//! is refused is an item whose likelihood is zero under every model: conditioning on it
//! annihilates the posterior, which means the item contradicts every model in the set. That is a
//! finding about the model set, not an observation to fold in, so it is an error.

use crate::decision::{Belief, DecisionProblem};
use crate::error::EpistemicError;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// One already-observed piece of evidence.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceItem {
    pub id: String,
    /// What including this item costs the context: tokens, facts, latency, specimen, or burden.
    ///
    /// 43.14 specifies cost as a *vector*. This is a scalar, because the selection procedures here
    /// need a single order and the crate does not implement the multi-objective portfolio that
    /// would consume a vector. The scalarisation is the caller's; see [`crate::NOT_IMPLEMENTED`].
    pub cost: f64,
    likelihood: Vec<f64>,
}

impl EvidenceItem {
    pub fn new(
        id: impl Into<String>,
        cost: f64,
        likelihood: Vec<f64>,
    ) -> Result<Self, EpistemicError> {
        let id = id.into();
        if !cost.is_finite() || cost < 0.0 {
            return Err(EpistemicError::InadmissibleCost {
                item: id,
                value: cost,
            });
        }
        let mut any_positive = false;
        for (model, value) in likelihood.iter().enumerate() {
            if !value.is_finite() || *value < 0.0 {
                return Err(EpistemicError::InadmissibleLikelihood {
                    item: id,
                    model,
                    value: *value,
                });
            }
            if *value > 0.0 {
                any_positive = true;
            }
        }
        if !any_positive {
            return Err(EpistemicError::AnnihilatingEvidence { item: id });
        }
        Ok(EvidenceItem {
            id,
            cost,
            likelihood,
        })
    }

    /// Re-checks intrinsic invariants after a value arrived through derived serde deserialisation.
    pub fn validate(&self) -> Result<(), EpistemicError> {
        if !self.cost.is_finite() || self.cost < 0.0 {
            return Err(EpistemicError::InadmissibleCost {
                item: self.id.clone(),
                value: self.cost,
            });
        }
        let mut any_positive = false;
        for (model, value) in self.likelihood.iter().enumerate() {
            if !value.is_finite() || *value < 0.0 {
                return Err(EpistemicError::InadmissibleLikelihood {
                    item: self.id.clone(),
                    model,
                    value: *value,
                });
            }
            any_positive |= *value > 0.0;
        }
        if !any_positive {
            return Err(EpistemicError::AnnihilatingEvidence {
                item: self.id.clone(),
            });
        }
        Ok(())
    }

    /// An item whose likelihood is the same under every model.
    ///
    /// Such an item cannot move any posterior and therefore cannot change any decision. Its true
    /// value is exactly zero; the *computed* value is zero to within about one unit in the last
    /// place of the Bayes risk, because the expectation is summed in floating point. The suite
    /// states that tolerance rather than hiding it, and decides action-invariance on
    /// [`crate::voi::ValueOfInformation::changes_the_action`], which is an integer comparison and
    /// therefore exact.
    pub fn uninformative(
        id: impl Into<String>,
        cost: f64,
        models: usize,
    ) -> Result<Self, EpistemicError> {
        EvidenceItem::new(id, cost, vec![1.0; models])
    }

    pub fn likelihood(&self, model: usize) -> f64 {
        self.likelihood.get(model).copied().unwrap_or(0.0)
    }

    pub fn likelihoods(&self) -> &[f64] {
        &self.likelihood
    }
}

/// The evidence available to a query, in a fixed order.
///
/// Order is part of the identity: selection procedures break ties by index, so two pools with the
/// same members in a different order are different inputs and may produce different (equally
/// valid) selections. Sorting silently would hide that.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidencePool {
    items: Vec<EvidenceItem>,
}

impl EvidencePool {
    pub fn new(items: Vec<EvidenceItem>) -> Result<Self, EpistemicError> {
        let ids: Vec<String> = items.iter().map(|i| i.id.clone()).collect();
        crate::unique(&ids, "evidence pool")?;
        let pool = EvidencePool { items };
        pool.validate()?;
        Ok(pool)
    }

    /// Re-checks pool identity and item invariants after derived serde deserialisation.
    pub fn validate(&self) -> Result<(), EpistemicError> {
        let ids: Vec<String> = self.items.iter().map(|item| item.id.clone()).collect();
        crate::unique(&ids, "evidence pool")?;
        for item in &self.items {
            item.validate()?;
        }
        Ok(())
    }

    /// Checks every item's likelihood profile has one entry per model of `problem`.
    pub fn check_against(&self, problem: &DecisionProblem) -> Result<(), EpistemicError> {
        problem.validate()?;
        self.validate()?;
        for item in &self.items {
            if item.likelihood.len() != problem.model_count() {
                return Err(EpistemicError::LikelihoodShape {
                    item: item.id.clone(),
                    got: item.likelihood.len(),
                    models: problem.model_count(),
                });
            }
        }
        Ok(())
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn items(&self) -> &[EvidenceItem] {
        &self.items
    }

    pub fn get(&self, index: usize) -> Result<&EvidenceItem, EpistemicError> {
        self.items
            .get(index)
            .ok_or(EpistemicError::ElementOutOfRange {
                element: index,
                ground: self.items.len(),
            })
    }

    pub fn index_of(&self, id: &str) -> Result<usize, EpistemicError> {
        self.items.iter().position(|i| i.id == id).ok_or_else(|| {
            EpistemicError::UnknownIdentifier {
                collection: "evidence pool".to_string(),
                id: id.to_string(),
            }
        })
    }

    /// Total cost of a subset. This is the *rate* in rate–distortion.
    pub fn rate(&self, subset: &BTreeSet<usize>) -> Result<f64, EpistemicError> {
        self.validate()?;
        let mut total = 0.0;
        for &index in subset {
            total += self.get(index)?.cost;
        }
        Ok(total)
    }

    /// The posterior after conditioning `prior` on exactly the items in `subset`.
    ///
    /// Returns [`EpistemicError::DegenerateBelief`] when the retained items jointly rule out every
    /// model. That is a real outcome — a context can be internally contradictory with the model
    /// set — and it is surfaced rather than smoothed, because smoothing it would let a
    /// contradiction compile into a confident answer.
    pub fn posterior(
        &self,
        prior: &Belief,
        subset: &BTreeSet<usize>,
    ) -> Result<Belief, EpistemicError> {
        prior.validate()?;
        self.validate()?;
        let mut mass: Vec<f64> = prior.masses().to_vec();
        for &index in subset {
            let item = self.get(index)?;
            if item.likelihood.len() != prior.len() {
                return Err(EpistemicError::LikelihoodShape {
                    item: item.id.clone(),
                    got: item.likelihood.len(),
                    models: prior.len(),
                });
            }
            for (model, value) in mass.iter_mut().enumerate() {
                *value *= item.likelihood(model);
            }
        }
        Belief::new(mass)
    }

    /// The posterior after conditioning on every item. The reference against which a compressed
    /// context's distortion is measured.
    pub fn full_posterior(&self, prior: &Belief) -> Result<Belief, EpistemicError> {
        self.posterior(prior, &(0..self.items.len()).collect())
    }

    pub fn everything(&self) -> BTreeSet<usize> {
        (0..self.items.len()).collect()
    }
}

/// One possible result of an evidence action that has not been performed.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Outcome {
    pub label: String,
    likelihood: Vec<f64>,
}

impl Outcome {
    pub fn new(label: impl Into<String>, likelihood: Vec<f64>) -> Self {
        Outcome {
            label: label.into(),
            likelihood,
        }
    }

    pub fn likelihood(&self, model: usize) -> f64 {
        self.likelihood.get(model).copied().unwrap_or(0.0)
    }

    pub fn likelihoods(&self) -> &[f64] {
        &self.likelihood
    }
}

/// An evidence action: an observation that could be made, at a cost.
///
/// The validity condition is that for each model the outcome likelihoods sum to one — the action
/// produces exactly one of its outcomes. A profile that does not sum to one is not a
/// normalisation nuisance: it means the caller has described something other than a complete
/// partition of results, and every expectation taken over it would be silently mis-weighted.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Acquisition {
    pub id: String,
    /// Token, compute, latency, privacy, specimen and expert burden, scalarised by the caller.
    pub cost: f64,
    outcomes: Vec<Outcome>,
}

impl Acquisition {
    pub fn new(
        id: impl Into<String>,
        cost: f64,
        outcomes: Vec<Outcome>,
        models: usize,
    ) -> Result<Self, EpistemicError> {
        let id = id.into();
        if !cost.is_finite() || cost < 0.0 {
            return Err(EpistemicError::InadmissibleCost {
                item: id,
                value: cost,
            });
        }
        if outcomes.is_empty() {
            return Err(EpistemicError::OutcomelessAcquisition { action: id });
        }
        for model in 0..models {
            let sum: f64 = outcomes.iter().map(|o| o.likelihood(model)).sum();
            for outcome in &outcomes {
                let value = outcome.likelihood(model);
                if !value.is_finite() || value < 0.0 {
                    return Err(EpistemicError::InadmissibleLikelihood {
                        item: format!("{id}/{}", outcome.label),
                        model,
                        value,
                    });
                }
            }
            if (sum - 1.0).abs() > 1e-9 {
                return Err(EpistemicError::ImproperAcquisition {
                    action: id,
                    model,
                    sum,
                });
            }
        }
        Ok(Acquisition { id, cost, outcomes })
    }

    /// An action whose outcome distribution is the same under every model.
    ///
    /// Its value of information is exactly zero, and the suite asserts exactly zero.
    pub fn uninformative(
        id: impl Into<String>,
        cost: f64,
        models: usize,
    ) -> Result<Self, EpistemicError> {
        Acquisition::new(
            id,
            cost,
            vec![
                Outcome::new("yes", vec![0.5; models]),
                Outcome::new("no", vec![0.5; models]),
            ],
            models,
        )
    }

    /// A binary test with per-model positive rates. `positive[m] = P(positive | model m)`.
    pub fn binary(
        id: impl Into<String>,
        cost: f64,
        positive: Vec<f64>,
    ) -> Result<Self, EpistemicError> {
        let models = positive.len();
        let negative: Vec<f64> = positive.iter().map(|p| 1.0 - p).collect();
        Acquisition::new(
            id,
            cost,
            vec![
                Outcome::new("positive", positive),
                Outcome::new("negative", negative),
            ],
            models,
        )
    }

    pub fn outcomes(&self) -> &[Outcome] {
        &self.outcomes
    }

    /// Re-check invariants after a value arrived through derived serde deserialisation.
    pub fn check_against(&self, problem: &DecisionProblem) -> Result<(), EpistemicError> {
        if !self.cost.is_finite() || self.cost < 0.0 {
            return Err(EpistemicError::InadmissibleCost {
                item: self.id.clone(),
                value: self.cost,
            });
        }
        if self.outcomes.is_empty() {
            return Err(EpistemicError::OutcomelessAcquisition {
                action: self.id.clone(),
            });
        }
        for outcome in &self.outcomes {
            if outcome.likelihoods().len() != problem.model_count() {
                return Err(EpistemicError::LikelihoodShape {
                    item: format!("{}/{}", self.id, outcome.label),
                    got: outcome.likelihoods().len(),
                    models: problem.model_count(),
                });
            }
        }
        for model in 0..problem.model_count() {
            let mut sum: f64 = 0.0;
            for outcome in &self.outcomes {
                let value = outcome.likelihoods().get(model).copied().ok_or_else(|| {
                    EpistemicError::LikelihoodShape {
                        item: format!("{}/{}", self.id, outcome.label),
                        got: outcome.likelihoods().len(),
                        models: problem.model_count(),
                    }
                })?;
                if !value.is_finite() || value < 0.0 {
                    return Err(EpistemicError::InadmissibleLikelihood {
                        item: format!("{}/{}", self.id, outcome.label),
                        model,
                        value,
                    });
                }
                sum += value;
            }
            if (sum - 1.0).abs() > 1e-9 {
                return Err(EpistemicError::ImproperAcquisition {
                    action: self.id.clone(),
                    model,
                    sum,
                });
            }
        }
        Ok(())
    }
}
