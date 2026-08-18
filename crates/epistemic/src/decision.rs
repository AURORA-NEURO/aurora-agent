//! The decision problem the whole crate is defined against.
//!
//! Blueprint 43.50 writes the decision contract as a set of compatible models `M`, permitted
//! actions `A`, and a utility `U(a, M)`. Blueprint 43.02 writes the query as
//! `q = ⟨Y, A, t, ω, ℓ, B, ε, R⟩`, where `ℓ` is the decision loss. Everything downstream — the
//! distortion of a compressed context, the value of an evidence action, whether a rebase preserved
//! a decision — is a statement about `A` and `ℓ`.
//!
//! The legacy `fiber-query/0.1` and `fiber-query/0.2` forms carry neither. The executable
//! `fiber-query/0.3` boundary owns the wire conversion and passes this kernel an explicit problem.
//! Every entry point that needs a problem still takes it from the caller rather than reading a
//! default. There is no `DecisionProblem::default()`, and that is deliberate: a default loss is a
//! decision made by whoever wrote the library, silently, on behalf of a caller who never saw it.
//!
//! ## Loss, not utility
//!
//! 43.50 is written in utilities and maximisation; this module is written in losses and
//! minimisation. They are the same object under negation, and one direction had to be picked so
//! that "zero is the best case" holds everywhere — regret, distortion and value of information all
//! read more clearly against a floor of zero. [`DecisionProblem::from_utilities`] converts.
//!
//! ## Ties
//!
//! [`DecisionProblem::bayes_action`] breaks ties by lowest action index. An arbitrary but fixed
//! rule is required for replay: a selection that depends on hash iteration order produces a
//! different certificate on a different run, and this workspace hashes certificates.

use crate::error::EpistemicError;
use serde::{Deserialize, Serialize};

/// Numeric slack used when comparing two losses for equality.
///
/// Exposed because a caller comparing decisions across two runs needs the same tolerance the
/// library used, and because a hidden epsilon is a hidden decision.
pub const LOSS_EPSILON: f64 = 1e-12;

/// Actions, models, and the loss of taking one under the other.
///
/// Row-major over actions: entry `(a, m)` lives at `a * models.len() + m`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecisionProblem {
    actions: Vec<String>,
    models: Vec<String>,
    loss: Vec<f64>,
}

impl DecisionProblem {
    /// Builds a problem from a loss matrix, rejecting every shape and value it cannot use.
    ///
    /// Losses may be negative — a loss is an arbitrary affine scale, and shifting it does not
    /// change any argmin — but they must be finite. A `NaN` loss has no minimiser, and an infinite
    /// one makes every regret involving it either `inf` or `NaN`, so both are refused at the
    /// boundary rather than propagated into a frontier.
    pub fn new(
        actions: Vec<String>,
        models: Vec<String>,
        loss: Vec<f64>,
    ) -> Result<Self, EpistemicError> {
        if actions.is_empty() || models.is_empty() {
            return Err(EpistemicError::EmptyDecisionProblem {
                actions: actions.len(),
                models: models.len(),
            });
        }
        let want = actions.len() * models.len();
        if loss.len() != want {
            return Err(EpistemicError::LossMatrixShape {
                got: loss.len(),
                want,
                actions: actions.len(),
                models: models.len(),
            });
        }
        crate::unique(&actions, "actions")?;
        crate::unique(&models, "models")?;
        for (index, value) in loss.iter().enumerate() {
            if !value.is_finite() {
                return Err(EpistemicError::NonFiniteLoss {
                    action: index / models.len(),
                    model: index % models.len(),
                    value: *value,
                });
            }
        }
        Ok(DecisionProblem {
            actions,
            models,
            loss,
        })
    }

    /// Builds a problem from the utility matrix 43.50 is written in, by negation.
    pub fn from_utilities(
        actions: Vec<String>,
        models: Vec<String>,
        utility: Vec<f64>,
    ) -> Result<Self, EpistemicError> {
        let loss = utility.iter().map(|u| -u).collect();
        DecisionProblem::new(actions, models, loss)
    }

    pub fn actions(&self) -> &[String] {
        &self.actions
    }

    pub fn models(&self) -> &[String] {
        &self.models
    }

    /// Re-check invariants after a value arrived through derived serde deserialisation.
    ///
    /// The constructor enforces these conditions for ordinary callers, but serde can construct a
    /// public value without invoking it. Decision-relative analyses call this gate before any
    /// indexed matrix access so malformed wire data becomes a typed refusal instead of a panic.
    pub fn validate(&self) -> Result<(), EpistemicError> {
        if self.actions.is_empty() || self.models.is_empty() {
            return Err(EpistemicError::EmptyDecisionProblem {
                actions: self.actions.len(),
                models: self.models.len(),
            });
        }
        let want = self.actions.len() * self.models.len();
        if self.loss.len() != want {
            return Err(EpistemicError::LossMatrixShape {
                got: self.loss.len(),
                want,
                actions: self.actions.len(),
                models: self.models.len(),
            });
        }
        crate::unique(&self.actions, "actions")?;
        crate::unique(&self.models, "models")?;
        for (index, value) in self.loss.iter().enumerate() {
            if !value.is_finite() {
                return Err(EpistemicError::NonFiniteLoss {
                    action: index / self.models.len(),
                    model: index % self.models.len(),
                    value: *value,
                });
            }
        }
        Ok(())
    }

    pub fn action_count(&self) -> usize {
        self.actions.len()
    }

    pub fn model_count(&self) -> usize {
        self.models.len()
    }

    /// Loss of action `a` under model `m`.
    pub fn loss(&self, action: usize, model: usize) -> f64 {
        self.loss[action * self.models.len() + model]
    }

    pub fn action_index(&self, name: &str) -> Result<usize, EpistemicError> {
        self.actions.iter().position(|a| a == name).ok_or_else(|| {
            EpistemicError::UnknownIdentifier {
                collection: "actions".to_string(),
                id: name.to_string(),
            }
        })
    }

    pub fn model_index(&self, name: &str) -> Result<usize, EpistemicError> {
        self.models.iter().position(|m| m == name).ok_or_else(|| {
            EpistemicError::UnknownIdentifier {
                collection: "models".to_string(),
                id: name.to_string(),
            }
        })
    }

    /// Expected loss of `action` under `belief`.
    pub fn risk(&self, belief: &Belief, action: usize) -> f64 {
        (0..self.models.len())
            .map(|m| belief.mass(m) * self.loss(action, m))
            .sum()
    }

    /// The action minimising expected loss, ties broken by lowest index.
    pub fn bayes_action(&self, belief: &Belief) -> usize {
        let mut best = 0usize;
        let mut best_risk = self.risk(belief, 0);
        for action in 1..self.actions.len() {
            let risk = self.risk(belief, action);
            if risk < best_risk - LOSS_EPSILON {
                best = action;
                best_risk = risk;
            }
        }
        best
    }

    /// The expected loss of the best action under `belief`.
    pub fn bayes_risk(&self, belief: &Belief) -> f64 {
        self.risk(belief, self.bayes_action(belief))
    }

    /// Expected loss of `action` under `belief`, minus the best achievable expected loss.
    ///
    /// This is the quantity every distortion in [`crate::ratedistortion`] is measured in. It is
    /// non-negative by construction, and the suite checks that rather than trusting the
    /// construction.
    pub fn regret(&self, belief: &Belief, action: usize) -> f64 {
        (self.risk(belief, action) - self.bayes_risk(belief)).max(0.0)
    }

    /// The worst regret `action` incurs over `compatible`, each model taken as if certain.
    ///
    /// This is the `sup` form of 43.50's decision-sufficiency condition. It is the criterion to
    /// use when the compatible-model set is a *set* rather than a distribution — that is, when
    /// identification failed and there is no honest prior over the remaining models.
    pub fn minimax_regret(&self, compatible: &[usize], action: usize) -> f64 {
        compatible
            .iter()
            .map(|&m| {
                let best = (0..self.actions.len())
                    .map(|a| self.loss(a, m))
                    .fold(f64::INFINITY, f64::min);
                self.loss(action, m) - best
            })
            .fold(0.0f64, f64::max)
    }

    /// The action minimising worst-case regret over `compatible`, ties broken by lowest index.
    ///
    /// Returns `None` when `compatible` is empty: a minimax over nothing is not zero, it is
    /// undefined, and returning action 0 would be an answer to a question nobody could ask.
    pub fn minimax_action(&self, compatible: &[usize]) -> Option<usize> {
        if compatible.is_empty() {
            return None;
        }
        let mut best = 0usize;
        let mut best_regret = self.minimax_regret(compatible, 0);
        for action in 1..self.actions.len() {
            let regret = self.minimax_regret(compatible, action);
            if regret < best_regret - LOSS_EPSILON {
                best = action;
                best_regret = regret;
            }
        }
        Some(best)
    }

    /// Whether every model in `compatible` prefers the same action.
    ///
    /// This is the decision-relative sense in which a question can be "identified": not that a
    /// causal effect is point-identified, but that the residual model uncertainty does not reach
    /// the decision. See [`crate::ratedistortion::Identification`] for why the distinction is
    /// stated so loudly.
    pub fn actions_agree(&self, compatible: &[usize]) -> bool {
        let mut chosen: Option<usize> = None;
        for &m in compatible {
            let best = (0..self.actions.len())
                .map(|a| (a, self.loss(a, m)))
                .fold((0usize, f64::INFINITY), |acc, (a, l)| {
                    if l < acc.1 - LOSS_EPSILON {
                        (a, l)
                    } else {
                        acc
                    }
                })
                .0;
            match chosen {
                None => chosen = Some(best),
                Some(previous) if previous != best => return false,
                Some(_) => {}
            }
        }
        true
    }
}

/// A normalised distribution over the models of a [`DecisionProblem`].
///
/// Private mass vector with one gated constructor, so a belief that does not sum to one cannot be
/// constructed. `AGENTS.md` calls for invariants in the type system where they can go there, and
/// an unnormalised belief silently rescales every risk computed from it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Belief {
    mass: Vec<f64>,
}

impl Belief {
    /// Normalises `mass`, rejecting negative, non-finite, and all-zero inputs.
    pub fn new(mass: Vec<f64>) -> Result<Self, EpistemicError> {
        for (model, value) in mass.iter().enumerate() {
            if !value.is_finite() || *value < 0.0 {
                return Err(EpistemicError::InadmissibleBeliefMass {
                    model,
                    value: *value,
                });
            }
        }
        let total: f64 = mass.iter().sum();
        if total <= 0.0 || !total.is_finite() {
            return Err(EpistemicError::DegenerateBelief { mass: total });
        }
        Ok(Belief {
            mass: mass.into_iter().map(|v| v / total).collect(),
        })
    }

    /// The uniform belief over `models` models.
    pub fn uniform(models: usize) -> Result<Self, EpistemicError> {
        Belief::new(vec![1.0; models])
    }

    /// All mass on one model.
    pub fn point(models: usize, model: usize) -> Result<Self, EpistemicError> {
        if model >= models {
            return Err(EpistemicError::ElementOutOfRange {
                element: model,
                ground: models,
            });
        }
        let mut mass = vec![0.0; models];
        mass[model] = 1.0;
        Belief::new(mass)
    }

    pub fn mass(&self, model: usize) -> f64 {
        self.mass.get(model).copied().unwrap_or(0.0)
    }

    pub fn len(&self) -> usize {
        self.mass.len()
    }

    pub fn is_empty(&self) -> bool {
        self.mass.is_empty()
    }

    pub fn masses(&self) -> &[f64] {
        &self.mass
    }

    /// Models carrying at least `floor` posterior mass, in index order.
    ///
    /// This is how a compatible-model set is read off a posterior. `floor` is the caller's, not
    /// the library's: what counts as "still compatible" is a modelling decision, and 43.50 puts
    /// the compatible-model set in the runtime contract rather than deriving it.
    pub fn support_above(&self, floor: f64) -> Vec<usize> {
        (0..self.mass.len())
            .filter(|&m| self.mass[m] >= floor)
            .collect()
    }

    /// Checks the belief is over the same model set as `problem`.
    pub fn check_against(&self, problem: &DecisionProblem) -> Result<(), EpistemicError> {
        if self.mass.len() != problem.model_count() {
            return Err(EpistemicError::BeliefShape {
                models: problem.model_count(),
                got: self.mass.len(),
            });
        }
        Ok(())
    }
}
