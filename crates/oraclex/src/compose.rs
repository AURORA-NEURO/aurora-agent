//! Composition, interaction, and minimization (32.21).
//!
//! Two failure modes, and they pull in opposite directions.
//!
//! **Composition can cancel.** 32.21's first failure risk is "two mutations cancel". A pair of
//! transformations each declared non-invariant, whose composition leaves the required conclusion
//! exactly where it started, has produced a case that looks stressed and tests nothing.
//! [`interaction`] compares the composed relation against the components and reports the cancellation
//! as a contradiction with both components named.
//!
//! **Minimization can destroy the thing it shrank.** The other three risks — "shrinking removes
//! biological plausibility", "minimal case exposes hidden label cue", "order changes correct answer" —
//! are all one shape: a smaller case that no longer fails for the original reason. [`minimality`]
//! checks the three properties that make a shrink admissible: the minimal set is a subset of the
//! original, it still reproduces, and the mechanism is preserved. Failing the first is a contradiction
//! because it is a fact about two sets; failing the third is unresolved when nobody recorded the
//! mechanism, because "we did not check" is not "it was destroyed".
//!
//! # Order
//!
//! [`Composition`] records whether its steps commute. Non-commuting steps in an unordered pack are a
//! declaration error, not a runtime one, so [`order_declared`] answers from the declaration alone.
//!
//! # Not implemented
//!
//! No delta debugging. 32.21's operators include "delta debugging over artifacts and state", which is
//! a search over candidate subsets driven by re-running the failing pipeline. This crate has no
//! pipeline to re-run; it checks the *result* of somebody else's search, which is the part that is
//! currently unchecked.

use std::collections::BTreeSet;

use bioprism_oracle::EvidenceTier;
use serde::{Deserialize, Serialize};

use crate::program::ExpectedRelation;
use crate::verdict::{Determination, Missing, Unresolved, Witness};

/// A sequence of transformations applied to one parent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Composition {
    pub id: String,
    /// Step ids in the order they were applied.
    pub steps: Vec<String>,
    /// Whether the author declared that these steps may be applied in any order.
    pub commutes: bool,
    /// Whether the pack that holds this composition preserves the order.
    pub order_preserved_in_pack: bool,
}

impl Composition {
    pub fn new(id: impl Into<String>, steps: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Composition {
            id: id.into(),
            steps: steps.into_iter().map(Into::into).collect(),
            commutes: false,
            order_preserved_in_pack: true,
        }
    }

    pub fn commuting(mut self, commutes: bool) -> Self {
        self.commutes = commutes;
        self
    }

    pub fn in_unordered_pack(mut self) -> Self {
        self.order_preserved_in_pack = false;
        self
    }
}

/// Whether the pack can honour this composition's ordering requirements.
pub fn order_declared(composition: &Composition) -> Determination {
    if composition.steps.len() < 2 {
        return Determination::not_evaluable(
            "a composition of fewer than two steps has no order to declare",
        );
    }
    if composition.commutes || composition.order_preserved_in_pack {
        return Determination::supported(
            EvidenceTier::Deterministic,
            if composition.commutes {
                format!("{} declares its steps commutative", composition.id)
            } else {
                format!("{} is held in an order-preserving pack", composition.id)
            },
        );
    }
    Determination::contradicted(
        EvidenceTier::Deterministic,
        Witness::RelationViolated {
            relation: format!("{} is reproducible", composition.id),
            expected: "commutative steps, or a pack that preserves their order".to_string(),
            observed: format!(
                "{} non-commuting steps in an unordered pack",
                composition.steps.len()
            ),
        },
    )
}

/// How two component relations combine, against what the composition actually did.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Interaction {
    pub left: ExpectedRelation,
    pub right: ExpectedRelation,
    /// What the composed transformation was observed to do to the required conclusion.
    pub observed: ExpectedRelation,
    /// Whether the author declared this interaction in advance (§32's "interaction declarations").
    pub declared: bool,
}

/// Whether a composed pair behaves as its components imply.
///
/// One case is decidable outright and it is the one that matters: two non-invariant components whose
/// composition is invariant have cancelled. Every other combination depends on the specific
/// transformations, so an undeclared interaction comes back unresolved naming the declaration as the
/// gap rather than being waved through.
pub fn interaction(interaction: &Interaction) -> Determination {
    let components_move =
        !interaction.left.is_invariant() && !interaction.right.is_invariant();
    if components_move && interaction.observed.is_invariant() {
        return Determination::contradicted(
            EvidenceTier::Deterministic,
            Witness::RelationViolated {
                relation: "composition of two non-invariant transformations".to_string(),
                expected: format!(
                    "a conclusion moved by {} then {}",
                    interaction.left.as_str(),
                    interaction.right.as_str()
                ),
                observed: "the composed case is invariant; the two transformations cancelled"
                    .to_string(),
            },
        );
    }
    if !interaction.declared {
        return Determination::unresolved(
            "a declared interaction between the two transformations",
            format!(
                "{} composed with {} was observed as {} and nobody predicted it",
                interaction.left.as_str(),
                interaction.right.as_str(),
                interaction.observed.as_str()
            ),
        );
    }
    Determination::supported(
        EvidenceTier::Property,
        format!(
            "declared interaction of {} and {} produced {}",
            interaction.left.as_str(),
            interaction.right.as_str(),
            interaction.observed.as_str()
        ),
    )
}

/// A shrink of a failing case, and what the shrinker checked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Minimization {
    /// Every element of the original failing case.
    pub original: BTreeSet<String>,
    /// The elements the shrinker kept.
    pub minimal: BTreeSet<String>,
    /// Whether the minimal case still fails.
    pub still_reproduces: bool,
    /// The mechanism the original failure ran through, if anyone recorded it.
    pub mechanism: Option<String>,
    /// Whether the minimal case fails through that same mechanism. `None` when the mechanism was
    /// never recorded, and therefore could not be checked.
    pub mechanism_preserved: Option<bool>,
    /// Whether the shrink introduced a cue the original did not have — 32.21's "minimal case exposes
    /// hidden label cue". `None` when nobody looked.
    pub introduces_cue: Option<bool>,
}

impl Minimization {
    pub fn new(
        original: impl IntoIterator<Item = impl Into<String>>,
        minimal: impl IntoIterator<Item = impl Into<String>>,
        still_reproduces: bool,
    ) -> Self {
        Minimization {
            original: original.into_iter().map(Into::into).collect(),
            minimal: minimal.into_iter().map(Into::into).collect(),
            still_reproduces,
            mechanism: None,
            mechanism_preserved: None,
            introduces_cue: None,
        }
    }

    pub fn through(mut self, mechanism: impl Into<String>, preserved: bool) -> Self {
        self.mechanism = Some(mechanism.into());
        self.mechanism_preserved = Some(preserved);
        self
    }

    pub fn cue_checked(mut self, introduces: bool) -> Self {
        self.introduces_cue = Some(introduces);
        self
    }

    /// How much smaller the minimal case is.
    pub fn removed(&self) -> BTreeSet<&str> {
        self.original
            .difference(&self.minimal)
            .map(String::as_str)
            .collect()
    }
}

/// Whether a shrink may replace the case it came from.
///
/// Order of checks is deliberate. Subset containment is a fact about two sets and fails as a
/// contradiction. Reproduction is a fact the shrinker recorded and fails as a contradiction.
/// Mechanism preservation and cue introduction are things that may simply not have been checked, and
/// an unchecked property is unresolved naming the check — never assumed to have passed because the
/// case got smaller.
pub fn minimality(minimization: &Minimization) -> Determination {
    if minimization.minimal.is_empty() {
        return Determination::not_evaluable("the minimal case is empty");
    }
    let added: Vec<&str> = minimization
        .minimal
        .difference(&minimization.original)
        .map(String::as_str)
        .collect();
    if let Some(element) = added.first() {
        return Determination::contradicted(
            EvidenceTier::Deterministic,
            Witness::RelationViolated {
                relation: "the minimal case is a subset of the original".to_string(),
                expected: "no new elements".to_string(),
                observed: format!("'{element}' appears in the shrink and not in the original"),
            },
        );
    }
    if !minimization.still_reproduces {
        return Determination::contradicted(
            EvidenceTier::Deterministic,
            Witness::RelationViolated {
                relation: "the minimal case still fails".to_string(),
                expected: "reproduction of the original failure".to_string(),
                observed: format!(
                    "the shrink removed {:?} and stopped failing",
                    minimization.removed()
                ),
            },
        );
    }
    if minimization.mechanism_preserved == Some(false) {
        return Determination::contradicted(
            EvidenceTier::Property,
            Witness::RelationViolated {
                relation: "the minimal case fails through the original mechanism".to_string(),
                expected: minimization
                    .mechanism
                    .clone()
                    .unwrap_or_else(|| "the recorded mechanism".to_string()),
                observed: "the shrink fails for a different reason".to_string(),
            },
        );
    }
    if minimization.introduces_cue == Some(true) {
        return Determination::contradicted(
            EvidenceTier::Property,
            Witness::RelationViolated {
                relation: "the minimal case introduces no cue the original lacked".to_string(),
                expected: "a shrink that removes only irrelevant elements".to_string(),
                observed: "the shrink exposes a label cue".to_string(),
            },
        );
    }

    let mut gaps: Vec<Missing> = Vec::new();
    if minimization.mechanism_preserved.is_none() {
        gaps.push(Missing::new(
            "a check that the shrink fails through the original mechanism",
            "a smaller case that fails for a new reason is a different bug wearing the old name",
        ));
    }
    if minimization.introduces_cue.is_none() {
        gaps.push(Missing::new(
            "a check for a label cue introduced by shrinking",
            "removing context can leave the answer readable from what remains",
        ));
    }
    match Unresolved::new(gaps) {
        Ok(unresolved) => Determination::Unresolved(unresolved),
        Err(_) => Determination::supported(
            EvidenceTier::Property,
            format!(
                "the shrink removed {:?}, still fails through '{}', and introduces no cue",
                minimization.removed(),
                minimization
                    .mechanism
                    .clone()
                    .unwrap_or_else(|| "the recorded mechanism".to_string())
            ),
        ),
    }
}
