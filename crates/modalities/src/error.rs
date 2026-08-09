//! Typed refusals for the modality layer.
//!
//! Every type here answers one question badly asked elsewhere: "can I use this number for that?"
//! Blueprint 28.00 forbids silent coercion, and the modality layer's version of coercion is
//! quieter than the unit layer's — nothing about a bulk expression value *looks* wrong when it is
//! used to argue about one cell. So the refusals carry the missing thing by name, and where the
//! refusal corresponds to a failure mode section 28 already lists, it carries that failure mode's
//! blueprint id and label rather than a paraphrase.
//!
//! The three-way split matters. [`Unsupported`] is about a single modality and a single claim.
//! [`TransportRefusal`] is about moving a value between modalities. [`CrossModalIncomparability`]
//! is about two values side by side. A caller that conflates them ends up asking whether two
//! numbers are equal when the real question was whether either number means what they think.

use crate::descriptor::{Measurand, Modality, Resolution, ResolutionStatus};
use crate::support::ClaimKind;
use bioprism_scope::ScopeClass;
use bioprism_standards::Incomparability;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Why a modality cannot support a claim.
///
/// The four resolution-related variants are deliberately distinct rather than one
/// `MissingResolution`. AGENTS.md's rule that "zero influence is not unknown influence" applies
/// here verbatim: an assay that demonstrably cannot see single cells, an assay whose descriptor
/// never said, and a value where the cell axis was *invented* by a deconvolution are three
/// different epistemic states, and collapsing them would let the weakest masquerade as the
/// strongest.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(tag = "unsupported", rename_all = "snake_case")]
pub enum Unsupported {
    /// The assay does not resolve an axis the claim is about.
    ///
    /// The canonical case: bulk transcriptomics and a cell-level claim. 28.03 names this
    /// "composition — bulk changes are interpreted as cell-intrinsic regulation".
    #[error("{modality} does not resolve {axis}, which a {claim} claim is about")]
    MissingResolution {
        modality: Modality,
        claim: ClaimKind,
        axis: Resolution,
    },

    /// The descriptor never said whether the assay resolves that axis.
    ///
    /// Not the same failure as [`Unsupported::MissingResolution`], and it is not more permissive
    /// either — the fix is upstream, in the descriptor, rather than in the choice of assay.
    #[error("{modality} has not declared whether it resolves {axis}; a {claim} claim needs it stated")]
    UndeclaredResolution {
        modality: Modality,
        claim: ClaimKind,
        axis: Resolution,
    },

    /// The axis exists in the value only because a transport put it there.
    ///
    /// A deconvolved bulk sample has per-cell-type numbers, and they are estimates conditioned on
    /// a reference panel. Using them to argue about cell-level biology is circular: the cell-level
    /// structure came from the reference, not from the specimen.
    #[error("{axis} in {modality} was imputed by {imputed_by}, not measured; a {claim} claim may not rest on it")]
    ImputedResolution {
        modality: Modality,
        claim: ClaimKind,
        axis: Resolution,
        imputed_by: String,
    },

    /// The modality measures a different quantity than the claim is about.
    ///
    /// 28.06's "RNA-protein equivalence" failure mode in typed form: transcript abundance is not
    /// protein activity, and no amount of resolution fixes that.
    #[error("{modality} measures {measured}, but a {claim} claim is about {required}")]
    WrongMeasurand {
        modality: Modality,
        claim: ClaimKind,
        measured: Measurand,
        required: String,
    },

    /// The claim needs an intervention and the modality is observational.
    #[error("a {claim} claim requires an interventional design; {modality} is observational")]
    ObservationalOnly { modality: Modality, claim: ClaimKind },

    /// The modality carries both designs and this dataset did not say which.
    ///
    /// 28.16 mixes randomised trial arms with real-world comparator cohorts under one heading, so
    /// the design is a property of the record, not of the modality. The blueprint supplies no rule
    /// for guessing, so the requirement goes back to the caller.
    #[error("{modality} carries both interventional and observational records; declare the design of this dataset before making a {claim} claim")]
    DesignNotDeclared { modality: Modality, claim: ClaimKind },

    /// Values were counted at one axis and treated as independent replicates at another.
    ///
    /// Not a claim the modality cannot make — it is an arithmetic mistake made while making a
    /// claim it can. 28.03, 28.04, 28.12 and 28.14 each list a version of it.
    #[error("{modality} was counted at {counted}, but its independent unit is {independent}")]
    PseudoReplication {
        modality: Modality,
        counted: Resolution,
        independent: Resolution,
    },

    /// The refusal corresponds to a failure mode section 28 already names.
    ///
    /// Wrapping rather than replacing the inner refusal keeps both readings available: the
    /// mechanical reason (an axis is missing) and the blueprint's own name for the mistake.
    #[error("{inner} — {module} names this failure mode: {label}")]
    NamedFailureMode {
        module: String,
        label: String,
        statement: String,
        inner: Box<Unsupported>,
    },
}

impl Unsupported {
    /// The mechanical refusal underneath any [`Unsupported::NamedFailureMode`] wrapper.
    pub fn root(&self) -> &Unsupported {
        match self {
            Unsupported::NamedFailureMode { inner, .. } => inner.root(),
            other => other,
        }
    }

    /// The blueprint module that names this refusal as a failure mode, when one does.
    pub fn named_module(&self) -> Option<&str> {
        match self {
            Unsupported::NamedFailureMode { module, .. } => Some(module.as_str()),
            _ => None,
        }
    }

    /// True when the block is a missing declaration rather than a stated limitation.
    ///
    /// Mirrors [`bioprism_standards::Incomparability::is_silence`], and for the same reason: a
    /// stated limitation is a fact about the assay, while silence is a fact about the metadata and
    /// is usually fixable without running anything.
    pub fn is_silence(&self) -> bool {
        matches!(
            self.root(),
            Unsupported::UndeclaredResolution { .. } | Unsupported::DesignNotDeclared { .. }
        )
    }
}

/// Why a cross-modality transport was refused or cannot be inverted.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(tag = "transport_refusal", rename_all = "snake_case")]
pub enum TransportRefusal {
    /// An aggregation was declared but the target resolves an axis the source does not.
    ///
    /// Aggregation only ever removes resolution. A declaration that it added some is not an
    /// optimistic aggregation, it is a different operation wearing aggregation's exactness.
    #[error("aggregation from {from} to {to} would have to create {axis}; aggregation only removes resolution")]
    AggregationWouldAddResolution {
        from: Modality,
        to: Modality,
        axis: Resolution,
    },

    /// A resolution-creating transport with no stated basis.
    ///
    /// Deconvolution needs a reference panel; imputation needs a model. 28.03 lists deconvolution
    /// among its benchmark decisions precisely because the answer depends on that input, so a
    /// transport that will not name it is not auditable.
    #[error("{kind} from {from} to {to} creates {axis} but names no basis for it")]
    UnstatedBasis {
        from: Modality,
        to: Modality,
        kind: String,
        axis: Resolution,
    },

    /// A cross-modality transport claiming to have lost nothing.
    ///
    /// The same rule `bioprism-scope` applies to [`bioprism_scope::MappingKind::Transport`] and
    /// `bioprism-standards` applies to a build lift. Moving a measurement between modalities is
    /// never free; a ledger asserting otherwise is a claim no method can support.
    #[error("transport {from} -> {to} declares no loss; crossing modalities is never free")]
    UndeclaredLoss { from: Modality, to: Modality },

    /// The transport is not invertible, so the round trip does not recover the input.
    ///
    /// Returned by [`crate::transport::ModalityTransport::invert`] rather than by construction:
    /// building an aggregation is fine, claiming to undo one is not.
    #[error("{kind} from {from} to {to} is not invertible: {because}")]
    NotInvertible {
        from: Modality,
        to: Modality,
        kind: String,
        because: String,
    },

    /// The source value does not have the axis the transport says it consumes.
    #[error("{kind} consumes {axis} from {from}, but {from} does not resolve it")]
    SourceLacksAxis {
        from: Modality,
        kind: String,
        axis: Resolution,
    },
}

/// Why two measurements from different modalities may not be compared.
///
/// Deliberately *wraps* [`bioprism_standards::Incomparability`] rather than repeating it. The
/// standards crate already decides units, frames, reference builds and ontology binding, and a
/// second implementation of those checks would drift. What this type adds is the dimension
/// standards has no view on: whether the two numbers are numbers about the same kind of thing.
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(tag = "blocked_by", rename_all = "snake_case")]
pub enum CrossModalIncomparability {
    /// The two measurements are of different quantities.
    ///
    /// The flagship case is RNA-seq and proteomics "on the same gene". Same ontology term, same
    /// unit family, entirely different quantity — 28.06 lists treating one as the other as a
    /// characteristic failure mode.
    #[error("{left} measures {left_measurand} and {right} measures {right_measurand}: {note}")]
    MeasurandMismatch {
        left: Modality,
        right: Modality,
        left_measurand: Measurand,
        right_measurand: Measurand,
        note: String,
    },

    /// The two measurements summarise different populations of things.
    ///
    /// A population average and a per-cell value are not two estimates of one number. Comparing
    /// them requires an explicit aggregation of the second, which is a transport with a ledger.
    #[error("{left} reports at {left_axis} and {right} reports at {right_axis}; aggregate explicitly before comparing")]
    ResolutionMismatch {
        left: Modality,
        right: Modality,
        left_axis: String,
        right_axis: String,
    },

    /// One side's resolution was imputed and the comparison is about that resolution.
    #[error("{side} carries {axis} imputed by {imputed_by}; comparing it against a measured value would read an estimate as an observation")]
    ImputedAgainstMeasured {
        side: Modality,
        axis: Resolution,
        imputed_by: String,
    },

    /// One side declares nothing about the axis it reports at.
    #[error("{side} has not declared {axis}; silence is not agreement")]
    UndeclaredAxis { side: Modality, axis: Resolution },

    /// One side reports at an axis it states it does not resolve.
    ///
    /// Distinct from [`CrossModalIncomparability::UndeclaredAxis`] and not a silence: the
    /// descriptor said the assay cannot see that axis, and the measurement claims to be indexed by
    /// it anyway. That is a malformed measurement rather than a missing declaration.
    #[error("{side} reports at {axis} but declares that it does not resolve it")]
    UnreportableAxis { side: Modality, axis: Resolution },

    /// The standards layer blocked it first.
    #[error(transparent)]
    Standards(#[from] Incomparability),
}

impl CrossModalIncomparability {
    /// The scope dimension class this refusal belongs to.
    ///
    /// Measurand and resolution are properties of the assay, so they land in
    /// [`ScopeClass::Specimen`] — the class `bioprism-scope` gives to `assay` and `platform` —
    /// while a delegated refusal keeps whatever class the standards layer assigned it. Reusing
    /// that taxonomy instead of minting a "modality" class keeps one vocabulary for the question
    /// "which dimension blocked this".
    pub fn blocking_class(&self) -> ScopeClass {
        match self {
            CrossModalIncomparability::MeasurandMismatch { .. }
            | CrossModalIncomparability::ResolutionMismatch { .. }
            | CrossModalIncomparability::ImputedAgainstMeasured { .. }
            | CrossModalIncomparability::UndeclaredAxis { .. }
            | CrossModalIncomparability::UnreportableAxis { .. } => ScopeClass::Specimen,
            CrossModalIncomparability::Standards(inner) => inner.blocking_class(),
        }
    }

    /// True when the block is a missing declaration rather than a stated disagreement.
    pub fn is_silence(&self) -> bool {
        match self {
            CrossModalIncomparability::UndeclaredAxis { .. } => true,
            CrossModalIncomparability::Standards(inner) => inner.is_silence(),
            _ => false,
        }
    }
}

/// Why a literature claim could not be bound to a scope (28.17).
#[derive(Debug, Clone, PartialEq, Eq, Error, Serialize, Deserialize)]
#[serde(tag = "binding_refusal", rename_all = "snake_case")]
pub enum BindingRefusal {
    /// A review or guideline was offered as direct evidence.
    ///
    /// 28.17 calls this "citation laundering: a review citation is used as if it were direct
    /// evidence". The binding is not refused outright — a review is real evidence about the state
    /// of a field — but it may not be bound at [`crate::literature::EvidenceTier::Primary`].
    #[error("{identifier} is a {tier} source and may not be bound as primary evidence")]
    CitationLaundering { identifier: String, tier: String },

    /// The source did not state the population it studied.
    ///
    /// Without it, 28.17's "population mismatch" is unfalsifiable: there is nothing to compare the
    /// target scope against, so every generalisation looks admissible.
    #[error("{identifier} states no study population; a claim with no population cannot be checked against a target scope")]
    UnstatedPopulation { identifier: String },

    /// The target scope is not inside the population the source studied.
    #[error("{identifier} studied {population}, which the target scope does not refine")]
    PopulationMismatch {
        identifier: String,
        population: String,
    },

    /// The source postdates the horizon the task is being evaluated at.
    ///
    /// 28.17 names "temporal leakage: later discoveries are used in historical rediscovery", and
    /// 28.16 repeats it for registry status. A horizon is only meaningful if crossing it is an
    /// error rather than a warning.
    #[error("{identifier} was published at {published} but the evaluation horizon is {horizon}")]
    TemporalLeakage {
        identifier: String,
        published: String,
        horizon: String,
    },

    /// The source is retracted or has a stated concern.
    #[error("{identifier} is {status}; binding it as evidence needs an explicit warrant")]
    RetractedSource { identifier: String, status: String },
}

/// The crate-level error, for callers that would rather match one type.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ModalityError {
    #[error(transparent)]
    Unsupported(#[from] Unsupported),
    #[error(transparent)]
    Transport(#[from] TransportRefusal),
    #[error(transparent)]
    Incomparable(#[from] CrossModalIncomparability),
    #[error(transparent)]
    Binding(#[from] BindingRefusal),

    /// A descriptor declared an axis both resolved and unresolved.
    #[error("descriptor for {modality} declares {axis} twice with different statuses")]
    ContradictoryDescriptor {
        modality: Modality,
        axis: Resolution,
    },

    /// Canonical encoding failed, which in practice means a non-finite value reached a digest.
    #[error("could not canonically encode {subject}: {detail}")]
    Encoding { subject: String, detail: String },
}

/// A helper for the descriptor builder, kept beside the errors it produces.
pub(crate) fn contradictory(
    modality: Modality,
    axis: Resolution,
    existing: &ResolutionStatus,
    incoming: &ResolutionStatus,
) -> Option<ModalityError> {
    if existing == incoming {
        None
    } else {
        Some(ModalityError::ContradictoryDescriptor { modality, axis })
    }
}
