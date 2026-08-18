//! Decision-equivalence quotienting: blueprint 43.10.
//!
//! A context compiler may retain several compatible models that are different as descriptions but
//! indistinguishable for the decision it is actually permitted to make.  43.10 asks for the
//! quotient by that relation rather than paying for, or reasoning over, distinctions that cannot
//! change an allowed decision.  The relation is not a biological equivalence, a causal
//! equivalence, or a statement that two models make the same prediction outside the supplied
//! loss table.
//!
//! This implementation makes the missing contract explicit.  The caller supplies the decision
//! problem and the permitted action names.  For each model, the quotient records the loss of every
//! permitted action relative to the best permitted loss for that model.  Two models are in one
//! class exactly when those finite relative-loss profiles have identical IEEE-754 bit patterns.
//! Subtracting the model's best loss makes the quotient invariant to an additive model-specific
//! baseline, which cannot change action ordering, regret, or any Bayes action over the permitted
//! actions.  Exact bit equality keeps the relation transitive and replayable; an epsilon-based
//! "almost equal" relation would not be an equivalence relation and could merge A with B and B
//! with C while refusing to merge A with C.
//!
//! The quotient therefore preserves permitted-action ordering and decision regret profiles.  It
//! does not preserve absolute expected loss, forbidden actions, causal semantics, model
//! likelihoods, or the truth of any scientific claim. The versioned `fiber-query/0.3` boundary
//! now carries the exact action/loss table needed to invoke this pass from `bioprism-fiber`; the
//! older wire forms continue to refuse that missing contract rather than inventing one. The
//! compatible-model posterior and evidence-pool bindings required by rate-distortion remain a
//! separate, deliberately deferred boundary.

use crate::decision::{DecisionProblem, LOSS_EPSILON};
use crate::error::EpistemicError;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The schema of the in-process/MCP quotient projection.
pub const DECISION_QUOTIENT_SCHEMA_VERSION: &str = "bioprism-epistemic-decision-quotient/0.1";

/// The equivalence relation implemented by [`quotient`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EquivalenceBasis {
    /// Equality of the permitted-action loss vector after subtracting its model-local minimum.
    PermittedLossDifferenceProfile,
}

impl EquivalenceBasis {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PermittedLossDifferenceProfile => "permitted_loss_difference_profile",
        }
    }
}

/// One class in the decision quotient.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecisionEquivalenceClass {
    /// Stable zero-based class index in canonical profile order.
    pub class_index: usize,
    /// Lexically first model in the class. It is a label, not a privileged model.
    pub representative_model: String,
    /// All model identifiers in lexical order.
    pub members: Vec<String>,
    /// Each permitted action's loss minus the best permitted loss for this model.
    pub loss_differences: BTreeMap<String, f64>,
    /// Actions tied for the model-local minimum under the library's explicit tie tolerance.
    pub preferred_actions: Vec<String>,
}

/// The deterministic quotient of a decision problem by its permitted action contract.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DecisionEquivalenceQuotient {
    pub schema_version: String,
    pub basis: EquivalenceBasis,
    /// Canonical lexical order, because action order is not semantic.
    pub permitted_actions: Vec<String>,
    pub original_model_count: usize,
    pub quotient_model_count: usize,
    pub merged_model_count: usize,
    /// Model identity remains visible; a class number without this map would be lossy.
    pub model_to_class: BTreeMap<String, usize>,
    pub classes: Vec<DecisionEquivalenceClass>,
}

struct EquivalenceGroup {
    profile: BTreeMap<String, f64>,
    preferred_actions: Vec<String>,
    members: Vec<String>,
}

impl DecisionEquivalenceQuotient {
    pub fn class_for_model(&self, model: &str) -> Option<usize> {
        self.model_to_class.get(model).copied()
    }

    pub fn compression_fraction(&self) -> f64 {
        if self.original_model_count == 0 {
            return 0.0;
        }
        self.quotient_model_count as f64 / self.original_model_count as f64
    }

    /// Whether the quotient actually removed at least one model description.
    pub fn compressed(&self) -> bool {
        self.quotient_model_count < self.original_model_count
    }
}

/// Compute the exact decision-equivalence quotient for `permitted_actions`.
///
/// The action set is treated as a set and canonicalised lexically.  An empty set is refused: the
/// empty decision boundary would make every model vacuously equivalent and would be a fabricated
/// compression rather than a useful result.  The decision problem is revalidated even when it
/// arrived through serde, because derived deserialisation can construct a value without passing
/// through `DecisionProblem::new`.
pub fn quotient(
    problem: &DecisionProblem,
    permitted_actions: &[String],
) -> Result<DecisionEquivalenceQuotient, EpistemicError> {
    problem.validate()?;
    if permitted_actions.is_empty() {
        return Err(EpistemicError::EmptyPermittedActionSet);
    }

    let mut canonical_actions = BTreeSet::new();
    for action in permitted_actions {
        if !canonical_actions.insert(action.clone()) {
            return Err(EpistemicError::DuplicateIdentifier {
                collection: "permitted actions".to_string(),
                id: action.clone(),
            });
        }
        problem.action_index(action)?;
    }
    let canonical_actions: Vec<String> = canonical_actions.into_iter().collect();
    let action_indices: Vec<usize> = canonical_actions
        .iter()
        .map(|action| problem.action_index(action))
        .collect::<Result<_, _>>()?;

    let mut grouped: BTreeMap<Vec<u64>, EquivalenceGroup> = BTreeMap::new();
    for (model_index, model) in problem.models().iter().enumerate() {
        let losses: Vec<f64> = action_indices
            .iter()
            .map(|&action| problem.loss(action, model_index))
            .collect();
        let best = losses.iter().copied().fold(f64::INFINITY, f64::min);
        let differences: Vec<f64> = losses
            .iter()
            .map(|loss| canonical_zero(*loss - best))
            .collect();
        let profile_key = differences.iter().map(|value| value.to_bits()).collect();
        let profile = canonical_actions
            .iter()
            .cloned()
            .zip(differences.iter().copied())
            .collect::<BTreeMap<_, _>>();
        let preferred_actions = canonical_actions
            .iter()
            .zip(losses.iter())
            .filter(|(_, loss)| **loss <= best + LOSS_EPSILON)
            .map(|(action, _)| action.clone())
            .collect::<Vec<_>>();
        grouped
            .entry(profile_key)
            .and_modify(|group| group.members.push(model.clone()))
            .or_insert(EquivalenceGroup {
                profile,
                preferred_actions,
                members: vec![model.clone()],
            });
    }

    let mut classes = Vec::with_capacity(grouped.len());
    let mut model_to_class = BTreeMap::new();
    for (class_index, (_, group)) in grouped.into_iter().enumerate() {
        let EquivalenceGroup {
            profile,
            preferred_actions,
            mut members,
        } = group;
        members.sort();
        for model in &members {
            model_to_class.insert(model.clone(), class_index);
        }
        classes.push(DecisionEquivalenceClass {
            class_index,
            representative_model: members[0].clone(),
            members,
            loss_differences: profile,
            preferred_actions,
        });
    }

    let original_model_count = problem.model_count();
    let quotient_model_count = classes.len();
    Ok(DecisionEquivalenceQuotient {
        schema_version: DECISION_QUOTIENT_SCHEMA_VERSION.to_string(),
        basis: EquivalenceBasis::PermittedLossDifferenceProfile,
        permitted_actions: canonical_actions,
        original_model_count,
        quotient_model_count,
        merged_model_count: original_model_count.saturating_sub(quotient_model_count),
        model_to_class,
        classes,
    })
}

fn canonical_zero(value: f64) -> f64 {
    if value == 0.0 {
        0.0
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn problem(models: &[&str], loss: &[f64]) -> DecisionProblem {
        DecisionProblem::new(
            vec!["accept".into(), "defer".into(), "reject".into()],
            models.iter().map(|model| (*model).to_string()).collect(),
            loss.to_vec(),
        )
        .unwrap()
    }

    #[test]
    fn additive_model_baselines_are_decision_equivalent() {
        let problem = problem(
            &["m-a", "m-b", "m-c"],
            &[
                0.0, 7.0, 0.0, // accept
                4.0, 11.0, 5.0, // defer
                8.0, 15.0, 8.0, // reject
            ],
        );
        let result = quotient(
            &problem,
            &["reject".into(), "accept".into(), "defer".into()],
        )
        .unwrap();
        assert_eq!(result.permitted_actions, ["accept", "defer", "reject"]);
        assert_eq!(result.original_model_count, 3);
        assert_eq!(result.quotient_model_count, 2);
        assert_eq!(result.merged_model_count, 1);
        assert_eq!(result.class_for_model("m-a"), result.class_for_model("m-b"));
        assert_ne!(result.class_for_model("m-a"), result.class_for_model("m-c"));
        assert_eq!(result.classes[0].loss_differences["accept"], 0.0);
    }

    #[test]
    fn forbidden_action_differences_are_not_part_of_the_quotient() {
        let problem = problem(
            &["m-a", "m-b"],
            &[
                0.0, 0.0, // accept
                4.0, 4.0, // defer
                8.0, 800.0, // reject differs only in forbidden reject
            ],
        );
        let permitted = quotient(&problem, &["accept".into(), "defer".into()]).unwrap();
        assert_eq!(permitted.quotient_model_count, 1);
        let all = quotient(
            &problem,
            &["accept".into(), "defer".into(), "reject".into()],
        )
        .unwrap();
        assert_eq!(all.quotient_model_count, 2);
    }

    #[test]
    fn ties_are_preserved_and_class_order_is_input_order_independent() {
        let first = problem(&["m-a", "m-b"], &[0.0, 3.0, 0.0, 3.0, 4.0, 9.0]);
        let second = problem(&["m-b", "m-a"], &[3.0, 0.0, 3.0, 0.0, 9.0, 4.0]);
        let left = quotient(&first, &["reject".into(), "accept".into(), "defer".into()]).unwrap();
        let right = quotient(&second, &["defer".into(), "accept".into(), "reject".into()]).unwrap();
        assert_eq!(left.classes, right.classes);
        assert_eq!(left.classes[0].preferred_actions, ["accept", "defer"]);
        assert_eq!(left.model_to_class, right.model_to_class);
    }

    #[test]
    fn missing_or_duplicate_permitted_actions_refuse() {
        let problem = problem(&["m"], &[0.0, 1.0, 2.0]);
        assert_eq!(
            quotient(&problem, &[]).unwrap_err(),
            EpistemicError::EmptyPermittedActionSet
        );
        assert!(matches!(
            quotient(&problem, &["accept".into(), "accept".into()]),
            Err(EpistemicError::DuplicateIdentifier { .. })
        ));
        assert!(matches!(
            quotient(&problem, &["unknown".into()]),
            Err(EpistemicError::UnknownIdentifier { .. })
        ));
    }
}
