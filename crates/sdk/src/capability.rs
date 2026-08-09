//! What a plugin says it is, and at what precedence.
//!
//! Blueprint 40.16 takes `capability schemas` as an input and emits a `capability card`; 43.35
//! requires capabilities to be "discoverable operations, schemas, permissions, and conformance
//! evidence"; 11.12 lists thirteen plugin kinds.
//!
//! # Four kinds, not thirteen
//!
//! 11.12's list runs from trace adapter to report renderer. [`CapabilityKind`] carries five, and
//! the criterion is not ambition but arithmetic: a kind is here only if this workspace already has
//! an extension point for it, so that a registered plugin has something to be registered *as*.
//!
//! | kind | the extension point it fills |
//! |---|---|
//! | [`CapabilityKind::Adapter`] | `bioprism_adapter::Adapter` |
//! | [`CapabilityKind::Oracle`] | `bioprism_oracle::Oracle` |
//! | [`CapabilityKind::ContextStrategy`] | `bioprism_baseline::ContextStrategy` |
//! | [`CapabilityKind::MutationOperator`] | `bioprism_mutation::Mutation`, which is data rather than a trait |
//! | [`CapabilityKind::QueryBackend`] | `bioprism_backends::QueryBackend` |
//!
//! Adding `report renderer` before a renderer interface exists would let a plugin register for a
//! job nothing can dispatch, and the registry would report a provider the host cannot call.
//!
//! # Why this crate names those traits without depending on them
//!
//! `bioprism-sdk` depends on `bioprism-ids` and `bioprism-section` and nothing else in the
//! workspace. It is the layer *above* the extension points, not a sixth one, and a dependency edge
//! to each of the five would make the registration layer un-buildable whenever any one of them
//! churns — and would drag every plugin host into linking all five. The consequence is honest and
//! worth stating: this crate cannot check that a plugin claiming
//! [`CapabilityKind::Adapter`] actually implements `Adapter`. It checks the declaration. The trait
//! bound is the host's job at the point of dispatch, where the concrete type is in scope.

use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

/// The extension points a plugin can register for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityKind {
    /// Normalises a source format into facts plus a mandatory loss audit.
    Adapter,
    /// Judges evidence and states what it can and cannot establish.
    Oracle,
    /// Selects context for a decision query, entered as one competitor among baselines.
    ContextStrategy,
    /// Produces a mutated world with a declared metamorphic relation.
    MutationOperator,
    /// Executes a compiled plan against a store.
    QueryBackend,
}

impl CapabilityKind {
    pub const ALL: [CapabilityKind; 5] = [
        CapabilityKind::Adapter,
        CapabilityKind::Oracle,
        CapabilityKind::ContextStrategy,
        CapabilityKind::MutationOperator,
        CapabilityKind::QueryBackend,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            CapabilityKind::Adapter => "adapter",
            CapabilityKind::Oracle => "oracle",
            CapabilityKind::ContextStrategy => "context-strategy",
            CapabilityKind::MutationOperator => "mutation-operator",
            CapabilityKind::QueryBackend => "query-backend",
        }
    }

    /// Whether a plugin of this kind has to carry a semantic-loss declaration.
    ///
    /// Only the adapter does, and for the reason `bioprism-adapter` gives: an adapter is the only
    /// one of the five that *converts* between representations, so it is the only one that can
    /// silently drop something on the way in. An oracle that cannot judge some evidence returns
    /// `NotEvaluable`, which is a value, not a loss.
    pub fn requires_loss_declaration(self) -> bool {
        matches!(self, CapabilityKind::Adapter)
    }
}

impl fmt::Display for CapabilityKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Precedence among plugins offering the same kind. Higher wins.
///
/// A newtype rather than a bare `u32` so that the conflict rule has something to be about, and so
/// that `Priority::DEFAULT` is a value a plugin author can be pointed at. Two plugins at the same
/// priority for the same kind is a registration error, not a tiebreak: see
/// [`crate::registry`] for why an alphabetical tiebreak would be worse than a refusal.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Priority(pub u32);

impl Priority {
    /// Where a plugin lands if its author does not think about precedence.
    pub const DEFAULT: Priority = Priority(100);
    /// The reference implementation's slot. Left low so a third party can outrank it deliberately.
    pub const BUILTIN: Priority = Priority(10);

    pub fn value(self) -> u32 {
        self.0
    }
}

impl fmt::Display for Priority {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// One capability claim: a kind, a precedence, and the concrete subjects it handles.
///
/// `subjects` is what makes resolution useful rather than nominal. An adapter capability whose
/// subjects are `text/csv` and `application/x-cbioportal` can be resolved *for a format*, so a
/// host with three adapters does not have to ask each in turn and take the first that does not
/// error — which is the "try it and see" pattern 43.35 rules out for versions and which is no
/// better for formats.
///
/// An empty subject set means "all subjects of this kind", which is the right default for a
/// context strategy (it takes any query) and the wrong one for an adapter. Nothing forces the
/// distinction, because a genuinely universal adapter is conceivable; [`Capability::is_universal`]
/// makes the claim visible instead.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Capability {
    pub kind: CapabilityKind,
    pub priority: Priority,
    pub subjects: BTreeSet<String>,
}

impl Capability {
    pub fn new(kind: CapabilityKind) -> Self {
        Capability {
            kind,
            priority: Priority::DEFAULT,
            subjects: BTreeSet::new(),
        }
    }

    pub fn at(mut self, priority: Priority) -> Self {
        self.priority = priority;
        self
    }

    pub fn handling<I, S>(mut self, subjects: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        self.subjects.extend(subjects.into_iter().map(Into::into));
        self
    }

    /// Whether this capability claims every subject of its kind.
    pub fn is_universal(&self) -> bool {
        self.subjects.is_empty()
    }

    /// Whether this capability covers `subject`. A universal capability covers everything.
    pub fn handles(&self, subject: &str) -> bool {
        self.is_universal() || self.subjects.contains(subject)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_adapter_kind_owes_a_loss_declaration() {
        let owing: Vec<CapabilityKind> = CapabilityKind::ALL
            .into_iter()
            .filter(|kind| kind.requires_loss_declaration())
            .collect();
        assert_eq!(owing, vec![CapabilityKind::Adapter]);
    }

    #[test]
    fn a_capability_with_no_subjects_claims_all_of_them_and_says_so() {
        let capability = Capability::new(CapabilityKind::ContextStrategy);
        assert!(capability.is_universal());
        assert!(capability.handles("anything"));
    }

    #[test]
    fn a_subject_list_narrows_what_the_capability_answers_for() {
        let capability = Capability::new(CapabilityKind::Adapter).handling(["text/csv"]);
        assert!(capability.handles("text/csv"));
        assert!(!capability.handles("application/dicom"));
        assert!(!capability.is_universal());
    }

    #[test]
    fn the_builtin_priority_is_below_the_default_so_a_third_party_can_outrank_it() {
        assert!(Priority::BUILTIN < Priority::DEFAULT);
    }
}
