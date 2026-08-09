//! The typed refusals of this crate.
//!
//! Every variant here is a place where §36 asks for something the crate can check and the caller
//! asked for the opposite. Errors that belong to a sibling are *carried*, not restated:
//! [`BioethicsError::Onco`] and [`BioethicsError::Safety`] wrap the boundary and release-gate
//! refusals of `bioprism-onco` and `bioprism-safety` so that a caller sees the originating crate's
//! own sentence rather than a paraphrase of it.

use crate::safeguard::{BlueprintModule, ControlSurface};
use bioprism_onco::OncoError;
use bioprism_safety::SafetyError;
use thiserror::Error;

/// What this crate refuses to do.
#[derive(Debug, Error, PartialEq)]
pub enum BioethicsError {
    /// A control whose effect requires a runtime, a network or a person was marked enforced.
    ///
    /// `bioprism-safety` holds the same line for threats. A library of plain Rust types can make a
    /// statement about itself unbreakable; it cannot make a statement about the world outside the
    /// process unbreakable, and a register that blurred the two would be decorative.
    #[error(
        "safeguard {safeguard:?} guards a perimeter, and a perimeter control cannot be enforced by \
         a single-process library; record it as declared"
    )]
    PerimeterCannotBeEnforced {
        safeguard: String,
        module: BlueprintModule,
        surface: ControlSurface,
    },

    /// A caller treated a declared safeguard as if something applied it.
    #[error(
        "safeguard {safeguard:?} is declared in {declared_in} and applied by nothing in this \
         process; a declaration is not a control and must not be relied on"
    )]
    UnenforcedReliance {
        safeguard: String,
        declared_in: String,
    },

    /// An enforced safeguard was decoded from bytes.
    ///
    /// Enforcement here means "no value of the relevant type exists", which is a property of the
    /// compiled crate. A JSON document asserting it is asserting something the document cannot
    /// know, so decoding fails rather than silently downgrading the record to declared — a silent
    /// downgrade would be a second, quieter lie.
    #[error(
        "safeguard {safeguard:?} claims enforcement in a serialized document; enforcement is a \
         property of this crate's types and does not travel in bytes"
    )]
    EnforcementNotTransportable { safeguard: String },

    /// A serialized safeguard used an enforcement word the register does not know.
    #[error("safeguard {safeguard:?} declares unknown enforcement state {state:?}")]
    UnknownEnforcementState { safeguard: String, state: String },

    /// A plan containing a physical step was asked for a disposition without both of the human
    /// approvals 36.10 requires.
    #[error(
        "plan {plan:?} contains {physical_steps} step(s) that act on the world and is missing \
         {missing}; 36.10 requires both a human approval and an institutional safety review before \
         a physical step may leave this workspace as a referral"
    )]
    PhysicalStepUnauthorised {
        plan: String,
        physical_steps: usize,
        missing: String,
    },

    /// An approval or safety-review record was filed with an empty attributable field.
    ///
    /// An anonymous approval documents nothing, which is `bioprism-safety`'s rule for residual-risk
    /// acceptance applied to 36.10's "human approval".
    #[error("authorisation for plan {plan:?} left {field} empty; an unattributed approval records nothing")]
    UnattributedAuthorisation { plan: String, field: String },

    /// A dual-use release referral was requested for a task whose misuse surfaces nobody assessed.
    ///
    /// 36.11's purpose sentence is "evaluate and release biological capabilities according to
    /// plausible misuse, not only benchmark performance". Unassessed is not none.
    #[error(
        "task {subject:?} has no misuse-surface assessment; an unassessed task is not a task with \
         no misuse surface, and 36.11 releases on assessed misuse rather than on benchmark score"
    )]
    MisuseSurfacesUnassessed { subject: String },

    /// A release referral was requested for a risk assessment carrying no sensitive category.
    ///
    /// 13.26's six sensitive categories and 36.11's six misuse surfaces are two different lists and
    /// the blueprint nowhere relates them. This crate refuses to invent the correspondence, so the
    /// caller states both and this variant fires when only one arrived.
    #[error(
        "task {subject:?} has an assessed misuse surface but its risk assessment names no \
         sensitive category; the two lists are not derivable from one another"
    )]
    SensitiveCategoryUnstated { subject: String },

    /// A misuse-surface assessment and a risk assessment named different subjects.
    #[error("misuse assessment names {release:?} and the risk assessment names {assessment:?}")]
    AssessmentSubjectMismatch { release: String, assessment: String },

    /// A representation finding was attributed across strata whose resource context does not match.
    #[error(
        "finding {finding:?} spans strata that differ on {unmatched}; 36.13 names site resources, \
         instrument availability and follow-up as scope axes, so a difference across them is not \
         attributable to any other axis"
    )]
    ResourceContextUnmatched { finding: String, unmatched: String },

    /// A stratum was recorded twice on the same axis.
    #[error("stratum {label:?} is recorded twice on axis {axis}")]
    DuplicateStratum { axis: String, label: String },

    /// A module was submitted for verification with evidence 36.21 requires and the dossier lacks.
    #[error(
        "module {subject:?} cannot be recorded verified: {missing} absent from the dossier, and an \
         absent evidence kind is not a satisfied one"
    )]
    UnmetValidationEvidence { subject: String, missing: String },

    /// The independent reproduction of 36.21 named the module's own author.
    ///
    /// 36.21 says "independent reproduction" and never states an independence criterion. The only
    /// criterion applied here is structural non-identity, which is the same weak criterion
    /// `bioprism-stewardship`'s reviewer separation uses, and it is weak on purpose: a stronger one
    /// would be invented.
    #[error(
        "module {subject:?} names {actor:?} as both author and independent reproducer; the only \
         independence criterion this crate can check is that the two names differ"
    )]
    ReproducerIsAuthor { subject: String, actor: String },

    /// An institutional determination was transcribed with a missing attributable field.
    #[error(
        "institutional determination for study {study:?} left {field} empty; a determination with \
         no body and no reference is not a determination"
    )]
    IncompleteInstitutionalDetermination { study: String, field: String },

    /// A study declared a purpose its consent does not permit.
    ///
    /// The consent model is `bioprism-policy`'s. This variant carries policy's refusal sentence
    /// rather than re-deciding the question.
    #[error("study {study:?} declares purpose {purpose} which its consent refuses: {refusal}")]
    PurposeOutsideConsent {
        study: String,
        purpose: String,
        refusal: String,
    },

    /// A research boundary refusal from `bioprism-onco`.
    #[error(transparent)]
    Onco(#[from] OncoError),

    /// A refusal from `bioprism-safety`'s dual-use release gate or withholding rule.
    #[error(transparent)]
    Safety(#[from] SafetyError),
}
