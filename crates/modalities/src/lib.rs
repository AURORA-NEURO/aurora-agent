//! The modality layer: what each assay family measures, what it cannot, and when two of them may
//! be compared.
//!
//! Implements the seventeen modality modules of blueprint section 28 that no other crate owns:
//! 28.02 epigenomics, 28.03 bulk transcriptomics, 28.04 single-cell and multiome, 28.05 spatial
//! omics, 28.06 proteomics, 28.07 metabolomics and flux, 28.08 CRISPR and functional screens,
//! 28.09 protein structure, 28.10 pharmacology and PK, 28.11 microbiome, 28.12 microscopy and
//! high-content imaging, 28.14 digital pathology, 28.15 clinical research and EHR, 28.16 trials and
//! real-world evidence, 28.17 literature and knowledge bases, 28.18 model organisms and
//! cross-species, and 28.20 neuro-oncology connectors.
//!
//! # The question the crate exists to answer
//!
//! Seventeen modalities all produce numbers, and the easy mistake is treating them as
//! interchangeable observations of one underlying truth. They are not. Bulk transcriptomics
//! measures a population average; single-cell measures a distribution; spatial measures a
//! distribution *with location*. A bulk value is recoverable from single-cell data and the reverse
//! is not, and that asymmetry is in the type system here rather than in a comment:
//!
//! * [`descriptor::ModalityDescriptor`] states, per resolution axis, whether an assay resolves it.
//! * [`support::supports`] refuses a claim the modality cannot carry, naming the missing axis and —
//!   where section 28 already lists the mistake — the blueprint's own label for it.
//! * [`transport::ModalityTransport`] moves a value between resolutions with a
//!   [`bioprism_scope::LossLedger`] it writes itself, and
//!   [`transport::ModalityTransport::invert`] refuses to undo an aggregation.
//! * [`comparability::comparable_across`] adds the modality dimension to
//!   [`bioprism_standards::comparable`] rather than paralleling it.
//! * [`literature`] models the step from "a paper says this" to "this holds here", and makes the
//!   unbound form unusable as evidence.
//!
//! # Where the boundaries are
//!
//! `bioprism-standards` owns the *typed standards* layer — units with dimensional analysis,
//! coordinate frames, reference builds, ontology binding, and the comparability check that names
//! the first blocking dimension. This crate reuses all of it and builds no second unit system and
//! no second comparability check: [`comparability::comparable_across`] delegates to
//! [`bioprism_standards::comparable_under`] once its own three checks pass, and
//! [`error::CrossModalIncomparability::Standards`] carries the standards layer's verdict unchanged.
//!
//! `bioprism-adapter` owns ingestion and semantic-loss reporting. `bioprism-oncoworlds` owns
//! section 30's domain depth, and this crate extends its rule rather than restating it: a marker
//! absent from a specimen is not absent from the tumour, and here, a value absent from a
//! modality's resolution is not a value of zero.
//!
//! # The four states of a resolution axis
//!
//! [`descriptor::ResolutionStatus`] distinguishes `Resolved`, `Unresolved`, `Imputed` and
//! `Undeclared`, and the crate never collapses them. AGENTS.md's rule that "zero influence is not
//! unknown influence" is the same rule: an assay that cannot see cells, a descriptor that did not
//! say, and a cell axis that a deconvolution invented are three different epistemic states, and
//! [`support::supports`] returns a different refusal for each.
//!
//! The `Imputed` state is where the crate's sharpest distinction lives. 28.03 lists deconvolution
//! among its *benchmark decisions* and "composition — bulk changes are interpreted as
//! cell-intrinsic regulation" among its *characteristic failure modes*, which are the same
//! operation used for two different purposes. So an imputed cell axis supports a composition claim
//! and is refused for a cell-intrinsic one, because the second would use structure the reference
//! panel supplied as evidence for that structure.
//!
//! # What is deliberately not implemented
//!
//! - **No assay data and no file parsing.** Nothing here reads a FASTQ, an H5AD, an OME-TIFF or a
//!   FHIR bundle. That is `bioprism-adapter`'s job, and section 28 is explicit that PRISM Bio
//!   "does not invent a universal biological file format".
//! - **No pipelines, no estimators, no statistics.** [`transport::ModalityTransport::apply`]
//!   rewrites a descriptor's declarations; it computes no mean, fits no reference panel and
//!   imputes no value. `supports` returning `Ok` means the modality is the right kind of
//!   instrument, never that a study was adequately powered or that a claim is true.
//! - **No invented constants.** There is not one detection limit, coverage floor, false-discovery
//!   rate, purity threshold or confidence cutoff anywhere in this crate. Section 28 states none,
//!   and `bioprism-oncoworlds` set the precedent of pushing every missing number back to the
//!   caller: [`descriptor::ModalityDescriptor::caller_supplied_constants`] records seventeen such
//!   obligations by name rather than filling any of them in.
//! - **No entailment checking.** 28.17's "claim drift" needs a restatement compared against a
//!   source span. [`literature::BoundClaim::source_text`] retains the span; the comparison is not
//!   made here.
//! - **Forty-one of section 28's eighty-five failure modes are unchecked.** All eighty-five are
//!   transcribed. Twenty-eight are checked by [`support::supports`], sixteen are checked somewhere
//!   else and say where via [`descriptor::FailureTrigger::CheckedElsewhere`], and the remaining
//!   forty-one carry [`descriptor::FailureTrigger::NotMechanised`] with a `reason` naming what
//!   would be needed — usually something this crate does not hold, such as a count matrix, a peak
//!   set, a guide-to-gene map or a model lineage graph. A listed-but-unchecked failure mode is a
//!   limitation; a silently absent one would read as coverage. The three counts are pinned by a
//!   test, so the claim cannot drift away from the code.
//!
//! # Example
//!
//! ```
//! use bioprism_modalities::{supports, ClaimKind, Modality, Unsupported};
//!
//! let refusal = supports(Modality::BulkTranscriptomics, ClaimKind::CellIntrinsicChange)
//!     .unwrap_err();
//!
//! assert_eq!(refusal.named_module(), Some("28.03"));
//! assert!(matches!(
//!     refusal.root(),
//!     Unsupported::MissingResolution { .. }
//! ));
//! assert!(supports(Modality::SingleCell, ClaimKind::CellIntrinsicChange).is_ok());
//! ```

pub mod catalog;
pub mod comparability;
pub mod descriptor;
pub mod error;
pub mod literature;
pub mod support;
pub mod transport;

pub use catalog::{descriptor, resolution_of, substitution_failure_mode};
pub use comparability::{
    comparable_across, comparable_across_under, report, CrossModalReport, CrossModalVerdict,
    ModalMeasurement,
};
pub use descriptor::{
    EvidenceDesign, FailureMode, FailureTrigger, Measurand, Modality, ModalityDescriptor,
    Resolution, ResolutionStatus,
};
pub use error::{
    BindingRefusal, CrossModalIncomparability, ModalityError, TransportRefusal, Unsupported,
};
pub use literature::{
    cites, BoundClaim, EvaluationHorizon, EvidenceTier, LiteratureClaim, RetractionStatus,
    SourceProvenance,
};
pub use support::{
    analysis_unit, independent_unit, supports, supports_descriptor, supported_claims,
    ClaimKind, ClaimRequirements, ImputedAxisPolicy, MeasurandRequirement,
};
pub use transport::{Fidelity, ModalityTransport, TransportChain, TransportKind};
