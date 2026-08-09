//! The BioCapability Atlas.
//!
//! Implements blueprint section 33 (BioCapability Atlas and Metrics), the two core specifications
//! it rests on — 03.09 (capability ontology) and 03.10 (failure taxonomy) — and the claim ladder
//! of 43.40.
//!
//! Section 33 opens by refusing the thing everyone wants: "AURORA BioPRISM does not produce a
//! universal 'biology intelligence' number. It produces a capability atlas ... The primary public
//! object is a scorecard plus failure atlas, not a rank." This crate is that object, and the
//! refusal is enforced by types rather than by policy documents.
//!
//! # The distinction the crate exists to hold
//!
//! A capability with no evidence is **unmeasured**. A capability whose trials all failed is
//! **measured and poor**. These are not two ends of one scale; they are different kinds of
//! statement, and collapsing them is how an evaluation becomes a marketing artifact — the
//! unmeasured capability quietly renders as a low bar on a heatmap, and a reader concludes
//! "weak here" when the truth is "nobody looked".
//!
//! The mechanism is the absence of a value, not a lint:
//!
//! - [`Measurement`] cannot be constructed or deserialized with an empty denominator.
//! - [`CapabilityCell::score`] returns `Option<f64>`. There is no `score_or_zero`.
//! - [`CapabilityCell::render`] hands a renderer either a [`CellRendering::Score`] or a
//!   [`CellRendering::Hole`], and the hole arm carries no number to draw.
//! - An unmeasured cell serializes as `{"state":"unmeasured","reason":...}` — the JSON has no
//!   score key at all.
//! - [`composite`] returns `Result` and refuses while any weighted capability is a hole.
//!
//! This is the same zero-versus-unknown distinction `bioprism-section` makes in its omission
//! manifest, where `InfluenceClass::Zero` means "provably cannot move the decision" and
//! `InfluenceClass::Unknown` means "nobody checked". [`UnmeasuredReason::influence_class`] maps
//! atlas holes onto that vocabulary, and [`CoverageReport::omission_manifest`] emits a real
//! [`bioprism_section::OmissionManifest`], so both crates gate on one predicate.
//!
//! # Shape
//!
//! - [`ontology`] — the versioned capability hierarchy of 03.09, its relations, and the refusal
//!   of an ontology in which a capability is its own ancestor.
//! - [`failure`] — the closed failure taxonomy of 03.10: mechanisms, axes, an unflattened causal
//!   chain, and label distributions that keep reviewer disagreement instead of resolving it.
//! - [`evidence`] — evidence records, [`Measurement`], and [`CapabilityCell`].
//! - [`atlas`] — the fold from records to cells, including oracle-authority resolution.
//! - [`claim`] — the ladder of 43.40 and [`permitted_claim`].
//! - [`report`] — coverage, holes, debt, and the composite gate.
//!
//! # What this crate does not do
//!
//! - **No metric estimators.** Section 33's modules 02–17 define Regret, Calibration, Cost,
//!   Latency, Robustness and the rest. The atlas indexes measurements; it does not compute them.
//! - **No confidence intervals.** 33.01 requires intervals clustered at the highest dependency
//!   level. [`Measurement::effective_size`] reports the clustering unit honestly, but the
//!   estimator needs per-trial outcomes retained past aggregation and belongs in the statistics
//!   layer. Reporting an interval this crate cannot justify would be the exact failure it exists
//!   to prevent.
//! - **No ontology reprojection.** 03.09 allows historical results to be mapped onto a new
//!   version by explicit transformation; no transformation language is specified, so a version
//!   mismatch is refused rather than guessed.
//! - **No claim cards, no signed bundles, no provenance replay.** 43.40 and 33.19 require them
//!   for release; they are release-process artifacts, not atlas invariants.
//! - **No rendering.** 33.01 lists ten public-card panels. This crate emits their input.

pub mod atlas;
pub mod claim;
pub mod error;
pub mod evidence;
pub mod failure;
pub mod ontology;
pub mod report;

pub use atlas::{Atlas, AtlasBuilder, Inconsistency};
pub use claim::{
    assess_claim, license_claim, permitted_claim, ClaimAssessment, ClaimConstraint, ClaimLicence,
    ClaimTier,
};
pub use error::AtlasError;
pub use evidence::{
    CapabilityCell, CellRendering, EvidenceRecord, EvidenceTier, Measurement, MeasurementDepth,
    MeasurementFields, OracleTier, TrialOutcome, UnmeasuredReason,
};
pub use failure::{
    CausalChain, Detectability, EvidenceStatus, FailureAxes, FailureLabel, FailureMechanism,
    FailureRecord, FailureStage, Inducement, LabelDistribution, LabelWeight, Reversibility,
    Severity,
};
pub use ontology::{
    CapabilityDimension, CapabilityFamily, CapabilityId, CapabilityNode, CapabilityOntology,
    Relation, RelationKind,
};
pub use report::{
    composite, Composite, CoverageDebt, CoverageReport, DepthCount, FamilyCoverage, Hole,
    MeasuredEntry, StageCount, WeightingPolicy,
};

/// The ontology schema this crate was written against.
///
/// Not the same thing as an ontology's own `version`: this is the shape of the vocabulary, that
/// is the content of one hierarchy.
pub const ONTOLOGY_SCHEMA_VERSION: &str = "bioprism-capability-ontology/0.1";

/// The failure-taxonomy schema. Bumped when a mechanism is added to or removed from
/// [`FailureMechanism::CLOSED_SET`], because 03.10's usefulness depends on the set being closed
/// and comparably so across releases.
pub const FAILURE_TAXONOMY_VERSION: &str = "bioprism-failure-taxonomy/0.1";
