//! Biology data standards: the vocabulary that makes comparability checkable.
//!
//! Implements blueprint section 28 (Biology Data and Standards) as a *typed standards* layer,
//! and protected class 3 of 39.05 — "units, coordinate system, genome/reference build,
//! orientation, and scale" — as types rather than as documentation.
//!
//! # What this crate is for
//!
//! `bioprism-adapter` owns ingestion: readers, semantic-loss reporting, conformance. This crate
//! owns the vocabulary those adapters must respect. Nothing here parses a file. The division is
//! the one 28.00 draws when it says PRISM Bio "does not invent a universal biological file
//! format" but provides "a common evaluation and lineage layer over modality-native standards":
//! the adapter reads the DICOM, this crate says what its coordinates mean.
//!
//! The single question the crate exists to answer is whether two measurements may be compared.
//! [`comparability::comparable`] returns `Ok(())` or a typed reason naming the first blocking
//! dimension. Everything else — [`unit`], [`frame`], [`reference`], [`ontology`] — supplies the
//! declarations that question needs.
//!
//! # The rule that shapes the design
//!
//! Two measurements in *unstated* frames are not comparable. Not "probably fine", not
//! "comparable with a warning" — blocked, with [`Incomparability::UnstatedFrame`]. The same
//! holds for unstated genome builds. This is the failure that hides best: two records that both
//! omit their frame look identical in exactly the way two records that agree on their frame do,
//! so silence reads as agreement. `bioprism-bioeval` refuses undeclared frames on the same
//! grounds, and [`frame::FrameBinding`] deliberately has no `Default`, because defaulting to RAS
//! would convert "nobody wrote it down" into "it is RAS".
//!
//! Where a declaration *is* present but differs, the crate does not paper over it either. Every
//! conversion produces a [`unit::ConversionRecord`]; every cross-build lift is a
//! [`reference::BuildTransport`] carrying a [`bioprism_scope::LossLedger`], and a transport that
//! claims to have lost nothing is rejected the way `bioprism-scope` rejects one. 28.00 lists
//! "changing reference genome coordinates" among the forbidden silent behaviours; a lift is
//! allowed, a silent reinterpretation is not.
//!
//! # Relationship to `bioprism-scope`
//!
//! This crate extends an existing vocabulary rather than building a parallel one.
//! [`bioprism_scope::ScopeClass::Coordinate`] already covers `frame`, `orientation`, `space`,
//! `units` and `genome_build`; [`bioprism_scope::ScopeClass::Ontology`] covers ontology and
//! classifier version. [`Incomparability::blocking_class`] maps every refusal back into that
//! taxonomy, and [`reference::BuildTransport::to_scope_mapping`] renders a build lift as a
//! genuine [`bioprism_scope::ScopeMapping`] over the `genome_build` dimension.
//!
//! # What is deliberately not implemented
//!
//! - **No UCUM parser and no unit ontology.** [`unit::Unit::parse`] is a lookup into a closed
//!   table of the SI-prefixed and clinical units neuro-oncology uses. Section 28 never names a
//!   unit system, so a general grammar would be this crate's invention wearing the blueprint's
//!   authority.
//! - **No ontology downloads and no subsumption reasoning.** [`ontology::MappingPrecision`]
//!   records the relation a curator asserted. Without the ontology loaded, inferring that one
//!   term subsumes another would be fabrication.
//! - **No chain files and no liftover arithmetic.** A [`reference::BuildTransport`] declares and
//!   audits a lift; the lifted coordinate comes from whichever tool performed it.
//! - **No affine matrices, no registration, no atlas volumes.** [`frame`] decides whether two
//!   positions share a frame and refuses when they do not. Moving a point between frames needs a
//!   transform this crate does not have.
//! - **No file parsing.** That is `bioprism-adapter`.
//!
//! # Example
//!
//! ```
//! use bioprism_standards::{comparable, Incomparability, Measurement, Position, Quantity, Unit};
//!
//! let mm = Unit::parse("mm")?;
//! let left = Measurement::located("centroid, site A", Position::unstated([62.0, -18.0, 31.0], mm.clone()));
//! let right = Measurement::located("centroid, site B", Position::unstated([62.0, -18.0, 31.0], mm));
//!
//! let refusal = comparable(&left, &right).unwrap_err();
//! assert!(matches!(refusal, Incomparability::UnstatedFrame { .. }));
//! # Ok::<(), bioprism_standards::UnitError>(())
//! ```

pub mod comparability;
pub mod error;
pub mod frame;
pub mod mechanism_exploration_inference_engine;
pub mod ontology;
pub mod reference;
pub mod unit;
pub mod migration_integrity_support;
pub mod local_migration_integrity_inference;
pub mod multimodal_migration_integrity_inference;
pub mod throughput_migration_integrity_inference;
pub mod federated_continual_migration_integrity_inference;
pub mod local_migration_integrity_contract_model;
pub mod multimodal_migration_integrity_contract_model;
pub mod throughput_migration_integrity_contract_model;
pub mod federated_continual_migration_integrity_contract_model;
pub mod local_migration_integrity_research_copilot;
pub mod multimodal_migration_integrity_research_copilot;
pub mod throughput_migration_integrity_research_copilot;
pub mod federated_continual_migration_integrity_research_copilot;
pub mod local_migration_integrity_workflow_fabric;
pub mod multimodal_migration_integrity_workflow_fabric;
pub mod throughput_migration_integrity_workflow_fabric;
pub mod federated_continual_migration_integrity_workflow_fabric;

pub use comparability::{
    comparable, comparable_under, reconcile, report, ComparabilityPolicy, ComparabilityReport,
    Measurement, Observable, Verdict, CHECK_ORDER,
};
pub use error::{
    FrameError, Incomparability, OntologyError, ReferenceError, StandardsError, UnitError,
};
pub use frame::{
    AnatomicalAxis, AxisDirection, CoordinateSpace, Extent, Frame, FrameBinding, Handedness,
    Orientation, Position,
};
pub use mechanism_exploration_inference_engine::{
    infer_standards_mechanisms,
    standards_mechanism_exploration_inference_manifest,
    StandardsMechanismCandidate6,
    StandardsMechanismInferenceArtifact8,
    StandardsMechanismInferenceError,
    StandardsMechanismInferenceReceipt8,
    StandardsMechanismPeer5,
    StandardsMechanismQuestion6,
    CONTENT_TYPE as STANDARDS_MECHANISM_INFERENCE_CONTENT_TYPE,
    CONTRACT_VERSION as STANDARDS_MECHANISM_INFERENCE_CONTRACT_VERSION,
    FEATURE_ID as STANDARDS_MECHANISM_INFERENCE_FEATURE_ID,
    INPUT_SCHEMA as STANDARDS_MECHANISM_INFERENCE_INPUT_SCHEMA,
    OUTPUT_SCHEMA as STANDARDS_MECHANISM_INFERENCE_OUTPUT_SCHEMA,
};
pub use ontology::{Binding, MappingPrecision, OntologyId, TermBinding, TermCatalog};
pub use reference::{
    BuildBinding, BuildTransport, CoordinateConvention, GenomeBuild, GenomicInterval,
    GenomicPosition, LiftOutcome, LiftRecord, ReferenceSpace,
};
pub use unit::{
    BaseDimension, ConversionRecord, Converted, Dimension, Exactness, Quantity, Unit, UnitKind,
};
pub use migration_integrity_support::*;
pub use local_migration_integrity_inference::*;
pub use multimodal_migration_integrity_inference::*;
pub use throughput_migration_integrity_inference::*;
pub use federated_continual_migration_integrity_inference::*;
pub use local_migration_integrity_contract_model::*;
pub use multimodal_migration_integrity_contract_model::*;
pub use throughput_migration_integrity_contract_model::*;
pub use federated_continual_migration_integrity_contract_model::*;
pub use local_migration_integrity_research_copilot::*;
pub use multimodal_migration_integrity_research_copilot::*;
pub use throughput_migration_integrity_research_copilot::*;
pub use federated_continual_migration_integrity_research_copilot::*;
pub use local_migration_integrity_workflow_fabric::*;
pub use multimodal_migration_integrity_workflow_fabric::*;
pub use throughput_migration_integrity_workflow_fabric::*;
pub use federated_continual_migration_integrity_workflow_fabric::*;
