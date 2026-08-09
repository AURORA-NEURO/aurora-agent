//! The semantic-loss declaration an adapter plugin has to carry.
//!
//! `bioprism-adapter` already commits to this idea: an adapter's manifest declares every loss kind
//! it may emit, and conformance fails an adapter that emits one it did not declare. This module is
//! the *registration-time* half of the same commitment — the registry refuses to register an
//! adapter plugin that never made the declaration at all (40.16 takes
//! `fixtures/conformance declaration` as an input; 23.49 requires adapters to report semantic
//! loss).
//!
//! # Why the kinds are strings here
//!
//! `bioprism_adapter::LossKind` is a closed enum of eight variants and it is the real vocabulary.
//! This crate stores the declaration as strings anyway, and does not depend on the adapter crate.
//! Two reasons, in order of weight:
//!
//! 1. **Coupling.** Every plugin host would otherwise link the adapter crate to register an
//!    *oracle*. The registration layer sits above all five extension points and should not import
//!    any of them.
//! 2. **Version skew.** A plugin compiled against an older adapter crate can still declare a kind
//!    this build's enum has since renamed. Refusing to *register* it because the enum moved would
//!    be a worse failure than recording the string and letting conformance — which does link the
//!    adapter crate — decide.
//!
//! The cost is stated rather than hidden: [`KNOWN_LOSS_KINDS`] is a copy, and a copy can drift.
//! [`SemanticLossDeclaration::unrecognised_kinds`] surfaces drift as an observation, never as a
//! registration failure, and [`crate::conformance_note`] says who is supposed to check it properly.

use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// The wire spellings of `bioprism_adapter::LossKind`, as of the version this crate was written
/// against.
///
/// Advisory. Nothing here rejects a kind outside this list; see the module docs.
pub const KNOWN_LOSS_KINDS: [&str; 8] = [
    "unmapped_column",
    "unpreserved_unit",
    "coordinate_frame_not_carried",
    "precision_reduced",
    "provenance_unavailable",
    "ontology_term_unmapped",
    "type_undetermined",
    "content_uninterpreted",
];

/// What an adapter plugin says about what it drops.
///
/// # Why this is a struct with a flag and not an enum
///
/// `enum { Lossless, Lossy(kinds) }` would make "lossless *and* drops units" unrepresentable in
/// Rust, which is the shape this workspace normally prefers. It is the wrong shape here, because a
/// manifest is a wire document: the contradictory state arrives as JSON from outside the process,
/// and an enum would turn it into a serde error naming a field. The registry needs to answer "this
/// plugin claims lossless ingestion while declaring that it drops units" — a sentence about the
/// declaration, not about the parser. So the contradiction is representable and
/// [`SemanticLossDeclaration::is_contradictory`] is the check that finds it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SemanticLossDeclaration {
    /// The adapter claims nothing is dropped. A strong claim; conformance is what tests it.
    pub lossless: bool,
    /// Every loss kind this adapter may emit.
    pub kinds: BTreeSet<String>,
    /// Digest of the loss report produced on the adapter's own conformance fixture, if the plugin
    /// ran one. Present so a later audit can tell whether the declaration was made against the
    /// same bytes the conformance evidence covers.
    pub audit_digest: Option<ContentHash>,
    pub notes: Vec<String>,
}

impl SemanticLossDeclaration {
    /// Claims that ingestion drops nothing.
    pub fn lossless() -> Self {
        SemanticLossDeclaration {
            lossless: true,
            ..SemanticLossDeclaration::default()
        }
    }

    /// Declares the loss surface: the kinds this adapter may emit.
    pub fn admitting<I, S>(kinds: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        SemanticLossDeclaration {
            lossless: false,
            kinds: kinds.into_iter().map(Into::into).collect(),
            ..SemanticLossDeclaration::default()
        }
    }

    pub fn with_audit_digest(mut self, digest: ContentHash) -> Self {
        self.audit_digest = Some(digest);
        self
    }

    pub fn noting(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    /// Says nothing at all: neither a lossless claim nor a loss surface.
    ///
    /// This is the state that has to be refused for adapters. An adapter with an empty declaration
    /// is indistinguishable from one that never looked, and `bioprism-adapter`'s module docs are
    /// explicit that the conflation "always resolves in the adapter author's favour".
    pub fn is_silent(&self) -> bool {
        !self.lossless && self.kinds.is_empty()
    }

    pub fn is_contradictory(&self) -> bool {
        self.lossless && !self.kinds.is_empty()
    }

    /// Declared kinds that are not in [`KNOWN_LOSS_KINDS`], sorted.
    ///
    /// An observation for an operator, not an error: see the module docs on version skew.
    pub fn unrecognised_kinds(&self) -> Vec<&str> {
        self.kinds
            .iter()
            .map(String::as_str)
            .filter(|kind| !KNOWN_LOSS_KINDS.contains(kind))
            .collect()
    }

    /// The kinds, rendered for an error message.
    pub fn describe_kinds(&self) -> String {
        if self.kinds.is_empty() {
            "none".to_string()
        } else {
            self.kinds
                .iter()
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join(", ")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_declaration_that_says_nothing_is_distinguishable_from_a_lossless_claim() {
        assert!(SemanticLossDeclaration::default().is_silent());
        assert!(!SemanticLossDeclaration::lossless().is_silent());
    }

    #[test]
    fn claiming_lossless_while_declaring_kinds_is_detectable_not_merely_unrepresentable() {
        let json = r#"{"lossless":true,"kinds":["unpreserved_unit"],"notes":[]}"#;
        let declaration: SemanticLossDeclaration =
            serde_json::from_str(json).expect("a wire document may contradict itself");
        assert!(declaration.is_contradictory());
        assert!(declaration.describe_kinds().contains("unpreserved_unit"));
    }

    #[test]
    fn an_unknown_loss_kind_is_reported_but_does_not_invalidate_the_declaration() {
        let declaration =
            SemanticLossDeclaration::admitting(["unmapped_column", "chromatin_state_dropped"]);
        assert_eq!(
            declaration.unrecognised_kinds(),
            vec!["chromatin_state_dropped"]
        );
        assert!(!declaration.is_silent());
        assert!(!declaration.is_contradictory());
    }

    #[test]
    fn every_known_kind_is_recognised_by_the_vocabulary_copy() {
        let declaration = SemanticLossDeclaration::admitting(KNOWN_LOSS_KINDS);
        assert!(declaration.unrecognised_kinds().is_empty());
        assert_eq!(declaration.kinds.len(), KNOWN_LOSS_KINDS.len());
    }
}
