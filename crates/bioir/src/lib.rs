//! Biological IR: the contracts that make measured biology replayable.
//!
//! Implements blueprint section 25 (Biological IR and Language), specifically modules
//! 25.04 (specimen and aliquot lineage), 25.05 (AssayLens), 25.11 (biological evidence
//! object), 25.12 (uncertainty and reference standards) and 25.13 (cohort, eligibility and
//! split). The closure constraint of 39.05 (non-compressible biological invariants) is
//! reified in [`invariants`].
//!
//! The organising idea shared by all five modules is that a biological number is never
//! self-describing. `0.7` is meaningless without the material it came from, the lens that
//! produced it, the population it was selected into, and the kinds of uncertainty that were
//! and were not accounted for. Every type here exists to keep one of those four contexts
//! attached to the number, and the errors exist to refuse operations that would silently
//! detach it.
//!
//! # What this crate deliberately does not do
//!
//! Every file in blueprint section 25 carries status `Specification Draft` and is written as
//! prose: a purpose, a list of primary object names, a table whose only column value is
//! "Required", and three or four one-line invariants. It is a design intent, not a frozen
//! contract. Where the prose under-specifies, this crate implements the invariant that is
//! actually stated and documents the gap rather than inventing a schema and presenting it as
//! spec. Concretely, none of the following are implemented because the blueprint does not
//! define them:
//!
//! - **A unit ontology.** 25.04 lists "unit conversion" under validation and never names a
//!   unit system. [`Quantity`] therefore compares only within an identical unit string and
//!   raises [`error::LineageError::UnitMismatch`] otherwise. Refusing is honest; guessing that
//!   `mL` converts to `uL` by a factor this crate invented is not.
//! - **An entity ontology.** 25.03 owns ontology binding; material types, sites, analytes and
//!   entity terms are carried here as opaque strings so that local terminology survives, per
//!   the 25.03 requirement to bind identifiers "without erasing local terminology".
//! - **Locator resolution.** 25.11 requires "native coordinates remain resolvable" but no
//!   artifact-shape contract exists to resolve against, so [`evidence::Locator`] is checked
//!   for internal well-formedness only.
//! - **Canonical JSON Schema and cross-language fixtures.** 25.23 owns conformance; the
//!   content hashes computed here are over `serde_json` canonical bytes from
//!   [`bioprism_ids::ContentHash`] and have not been reconciled against a reference
//!   implementation.
//!
//! # Shared vocabulary
//!
//! Validity context is [`bioprism_scope::ScopeKey`], instants are
//! [`bioprism_scope::Timestamp`], and artifact identity is [`bioprism_ids::ContentHash`].
//! This crate defines no parallel versions of those.

pub mod cohort;
pub mod error;
pub mod evidence;
pub mod ids;
pub mod invariants;
pub mod knowledge;
pub mod lens;
pub mod lineage;
pub mod laboratory_control_plane;
pub mod quantity;
pub mod uncertainty;

pub use error::{CohortError, EvidenceError, LensError, LineageError, UncertaintyError};
pub use ids::{CohortId, EvidenceId, LensId, ObservationId, SpecimenId, SubjectId};
pub use quantity::Quantity;

pub use cohort::{
    ChronologicalBoundary, CohortAssembly, CohortDefinition, EligibilityRule, Estimand, Fold,
    GroupingKey, LeakageFinding, Observation, Predicate, RepeatedMeasures, RuleEffect, SplitPlan,
    SplitUnit, TimeAnchor, Truth, UnitOfAnalysis,
};
pub use evidence::{
    AccessPolicy, Derivation, EvidenceIssue, EvidenceLedger, EvidenceObject, Locator,
    MeasurementContext, Modality, Provenance, QualityAssertion, Relation, Stance,
};
pub use invariants::{ContextProjection, ProtectedClass, RetentionReport};
pub use lens::{
    AssayLens, Calibration, CalibrationKind, ComparabilityRule, ErrorModel, Identifiability,
    Incomparability, LensCatalog, MaterialRequirement, Measurement, MeasurementScale,
    MeasurementTarget, MissingnessClass, ProcessingStep, ProtocolChain, QcContract, QcMetric,
    QcOutcome, Reading,
};
pub use lineage::{
    CollectionEvent, ConsumptionEvent, IdentityAssertion, IdentityConfidence, LeakageRisk,
    LineageGraph, LineageIssue, Origin, ProcessKind, Specimen,
};
pub use uncertainty::{
    AdjudicationRecord, CalibrationBin, CalibrationCurve, Representation, ReviewerAssessment,
    ReviewerDistribution, UncertaintyBudget, UncertaintyComponent, UncertaintyKind,
};

pub use knowledge::{
    EvidenceReferenceView, EvidenceSynthesis, KnowledgeCompiler, KnowledgeError,
    ScopedRetrievalQuery, FEATURE_CONTRACT_VERSION, FEATURE_ID,
};

pub use laboratory_control_plane::{
    laboratory_control_plane_manifest, preflight_instrument_action,
    InstrumentActionReceipt, InstrumentActionRequest, InstrumentCapability,
    LaboratoryControlError, CONTRACT_VERSION as LABORATORY_CONTROL_CONTRACT_VERSION,
    FEATURE_ID as LABORATORY_CONTROL_FEATURE_ID,
};
