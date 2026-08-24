//! The lens grammar and the typed lens catalogue.
//!
//! Implements blueprint section 42 (Graph-Native Evaluation Hub and UI), specifically the layer
//! beneath the interface: **what a lens is, what it can be asked, and what it must say when it
//! cannot answer**.
//!
//! # This is not a UI crate
//!
//! There is no renderer here. No layout, no coordinates, no colour, no HTML, no widgets, no
//! keyboard model, no focus order, no viewport, no streaming transport, no deep links, no saved
//! views, no interaction model of any kind. Nothing in this crate draws anything, and no type is
//! named as though it does. Section 42 describes an interface across 31 modules; what is
//! *checkable* in those 31 modules — and therefore what belongs in a Rust workspace with no
//! browser — is the answerable-question layer, which is what this crate is.
//!
//! `bioprism-graph` already implements the projection half of sections 41 and 42 under the
//! constraint of 43.01: it turns a compiled Decision Section into typed graph, hypergraph,
//! timeline and table projections, seals each to the provenance it came from, and refuses to emit
//! one that has lost an unresolved obligation. See [`catalogue::DISCHARGED_BY_GRAPH`] for the
//! module-by-module split. The short version: `graph` answers *what does this evidence look like*,
//! this crate answers *what question is this a view of, and did anyone actually check*.
//!
//! # The invariant this crate exists for
//!
//! 42.27, Accessibility and Non-Visual Navigation, asks for "table, outline, keyboard,
//! screen-reader, text, and API equivalents for every core graph task". Written as prose, that is
//! a review checklist, and review checklists are how accessibility is lost: the picture ships and
//! the table becomes a follow-up that describes the picture.
//!
//! Here it is a bound. [`Lens`] requires `type Witness: Witness`, and [`Witness`] has no method
//! that returns anything drawable — only named columns, typed [`Cell`]s, and a sentence. **A lens
//! that can only produce a rendering has no type to satisfy that bound and does not compile.**
//! Every one of the 31 modules' "nonvisual path has feature parity" acceptance gates is then a
//! consequence of the type rather than a thing to remember.
//!
//! # Three outcomes, and two of them are not the same
//!
//! Every lens result is [`LensOutcome::Answered`], [`LensOutcome::Refused`], or
//! [`LensOutcome::EvidenceAbsent`]. The last two are never merged, for the reason
//! `bioprism-section` never merges `InfluenceClass::Zero` with `InfluenceClass::Unknown`: "this
//! lens will not tell you" and "nobody gathered what it would need" have different remedies.
//!
//! The same refusal runs all the way down:
//!
//! | Distinction | Where it is made unrepresentable |
//! |---|---|
//! | measured absence vs. nobody measured | [`Observation`] has no blank; [`Missingness`] has no catch-all |
//! | a check that passed vs. one that never ran | [`leakage::LeakageFinding::CheckNotRunnable`], and [`leakage::leakage_status`] never returns `Valid` beside one |
//! | an assumption satisfied vs. unexamined | [`transport::AssumptionStatus::Unexamined`], and [`transport::transport_status`] returns `Underdetermined` |
//! | a finished sweep vs. a truncated one | [`Coverage`] has private fields and its partial constructor refuses an unnamed remainder |
//! | a rate over all strata vs. over some | [`anytime::ObservedRate`] carries its own strata denominator and prints it |
//! | a lens that passed vs. one nobody ran | [`gate::GateBlock::LensNotRun`] is its own variant |
//! | a cost of zero vs. a cost nobody measured | [`Recorded<usize>`] on a [`profile::FactEntry`] |
//!
//! # Shape
//!
//! - [`grammar`] — 42.01. [`Lens`], [`LensDeclaration`], [`LensOutcome`], [`Coverage`], and
//!   [`run`], which is the only way to obtain a sealed [`LensReport`].
//! - [`nonvisual`] — 42.27. The [`Witness`] trait and the [`Cell`] that has no blank.
//! - [`missingness`] — 42.13's core. [`Observation`], [`Missingness`], [`Recorded`].
//! - [`leakage`], [`claim`], [`transport`], [`qc`], [`profile`], [`anytime`] — the six lenses.
//! - [`gate`] — 42.30 and 42.31, as an executable predicate over a set of reports.
//! - [`catalogue`] — the six declarations, plus a named account of the twenty-five §42 modules
//!   that are not implemented here and why.
//!
//! # Six lenses, not thirty-one stubs
//!
//! Section 42's 31 module files are 93.6% byte-identical scaffolding — each differs from 42.01 in
//! five lines. Implementing them one-to-one would produce thirty-one structurally identical types
//! distinguished by a string, which is `AGENTS.md`'s "instance count is not benchmark count"
//! error with a module id on it. The shared contract is implemented once; the six modules whose
//! distinguishing sentence implies a *checkable* semantics get a lens. See [`catalogue`].
//!
//! # Determinism
//!
//! No clock, no ambient state, no hash-map iteration order in any output. A [`LensReport`] carries
//! a receipt that is a content hash of its own canonical body, so the same lens over the same
//! evidence produces the same receipt in any process. Reports are `Serialize`-only with private
//! fields and one constructor, so a claim that a lens answered cannot be forged.

pub mod anytime;
pub mod catalogue;
pub mod claim;
pub mod error;
pub mod federated_assurance;
pub mod gate;
pub mod grammar;
pub mod leakage;
pub mod missingness;
pub mod nonvisual;
pub mod profile;
pub mod qc;
pub mod transport;

pub use anytime::{
    AnytimeCurve, AnytimeCurveLens, AnytimeEvaluation, CurveFinding, CurvePoint, ObservedRate,
    StoppingState, Stratum,
};
pub use catalogue::{
    catalogue, catalogue_ids, DISCHARGED_BY_GRAPH, IMPLEMENTED_HERE, NOT_IMPLEMENTED,
};
pub use claim::{ClaimDossier, ClaimEvidenceLens, ClaimFinding, ClaimRecord, EvidenceItem};
pub use error::LensError;
pub use federated_assurance::{
    assure_federated_lens, FederatedLensAssuranceReceipt, FederatedLensAssuranceRequest,
    FederatedLensDisposition, FederatedLensError,
    CONTRACT_VERSION as FEDERATED_LENS_ASSURANCE_CONTRACT_VERSION,
    FEATURE_ID as FEDERATED_LENS_ASSURANCE_FEATURE_ID,
};
pub use gate::{GateBlock, GateOutcome, ReleaseGate};
pub use grammar::{
    run, AbsentRequirement, Completeness, Coverage, EvidenceGap, EvidenceRequirement, Lens,
    LensDeclaration, LensId, LensOutcome, LensReport, PendingRegion, Refusal, RefusalReason,
    ReportOutcome, ScopePrecondition,
};
pub use leakage::{
    leakage_status, CohortLeakageLens, CohortSplit, LeakageFinding, PreprocessingStep,
    SubjectRecord,
};
pub use missingness::{
    Attempt, CensorDirection, Measured, Missingness, MissingnessClass, MissingnessSummary,
    Observation, Recorded, UnattemptedReason,
};
pub use nonvisual::{Cell, Witness, WitnessRow};

/// The Decision Section layers of 43.25, re-exported because [`profile::FactEntry`] is typed by
/// them and a caller should not have to depend on `bioprism-section` to name one.
pub use bioprism_section::Layer;
pub use profile::{ContextProfile, ContextTokenProfileLens, FactEntry, ProfileFinding};
pub use qc::{AssayPanel, AssayQcMissingnessLens, PanelCell, QcDirection, QcFinding, QcMetric};
pub use transport::{
    transport_status, Assumption, AssumptionRecord, AssumptionStatus, CausalTransportLens,
    IdentificationStrategy, StratumSupport, StudyDesign, TransportFinding, TransportQuestion,
};

/// The schema version of a [`LensReport`] body.
///
/// The receipt is a hash over that body, so changing the body's shape changes every receipt.
/// Bumped when a field is added to or removed from the report, not when a lens changes what it
/// finds.
pub const LENS_REPORT_SCHEMA_VERSION: &str = "bioprism-lens-report/0.1";

/// The number of modules in blueprint section 42.
pub const SECTION_42_MODULE_COUNT: usize = 31;
