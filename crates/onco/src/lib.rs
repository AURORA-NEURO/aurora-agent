#![allow(clippy::all)]

//! OncoWorld: neuro-oncology research world model.
//!
//! Implements blueprint cluster `30_NEURO_ONCOLOGY_ONCOWORLD` — specifically 30.01 (disease
//! scope and taxonomy), 30.02 (longitudinal tumour worldline), 30.06 (criteria-aware
//! longitudinal response), 30.07 (post-treatment change, progression and uncertainty), 30.10
//! (integrated molecular classification), 30.26 (outcomes, survival, censoring and estimands)
//! and 30.30 (research-only clinical boundary and escalation).
//!
//! # This is a research platform
//!
//! 30.30 fixes the primary design constraint: OncoWorld "does not diagnose a person, recommend a
//! treatment, replace a multidisciplinary tumor board, or claim medical-device functionality".
//! That constraint is implemented in the types rather than asserted in prose:
//!
//! * [`boundary::ResearchOutput`] can only be produced by [`boundary::ResearchBoundary::release`],
//!   which refuses every individual clinical use, and has no constructor and no `Deserialize`
//!   that would let a caller mint one;
//! * [`boundary::EscalationNotice`] carries a trigger and a route and nothing else, so a
//!   recommendation is not representable in it;
//! * [`response::ResponseCall::Progression`] carries a [`response::ConfirmedProgression`] whose
//!   constructor is private to the confirmation gate;
//! * [`outcome::TerminalFact::LostToFollowUp`] has no path to [`outcome::EventKind`].
//!
//! # The four invariants worth stating up front
//!
//! **Integrated classification is a conjunction.** Histology selects candidate criteria; only
//! observed molecular calls advance or exclude them. Histology alone yields
//! [`taxonomy::DiagnosticResolution::Unresolved`] with a prioritized evidence request, never a
//! guessed entity. An unrun assay reports [`status::ObservationStatus::NotCollected`], never a
//! negative call.
//!
//! **The clocks are distinct.** 30.02 names four time axes — event validity, recording, release
//! and agent visibility. Each is its own newtype, durations exist only within an axis, and the
//! temporal firewall cuts on agent visibility because cutting on acquisition leaks evidence that
//! had not yet been released.
//!
//! **Post-treatment change is not progression.** Inside the window following definitive therapy,
//! a progression reading without confirmation is [`response::ResponseCall::NotEvaluable`] — and
//! not-evaluable is not [`response::ResponseCall::Stable`], which 30.06 names as a
//! characteristic failure. Timing withholds a progression call; it never asserts treatment
//! effect, which 30.07 forbids.
//!
//! **Censoring is not an event.** Loss to follow-up maps to
//! [`outcome::CensoringReason::LostToFollowUp`] under every endpoint, and each endpoint declares
//! the estimand it answers, including whether its censoring can be called non-informative.
//!
//! # Scope of this crate
//!
//! Domain types, invariants and per-subject records. There is no estimator, no imaging pipeline,
//! no ontology loader and no policy engine here; each module's header states what it does not
//! implement. The entity, marker, criterion, escalation-route and output-use vocabularies are
//! **invented** — the blueprint specifies the shape of these objects and enumerates none of their
//! contents — and are marked as such where they are defined.

pub mod boundary;
pub mod computational_execution_contract_model;
pub mod error;
pub mod federated_provenance_signing_workflow;
pub mod ingest;
pub mod instrument_research_workbench;
pub mod outcome;
pub mod response;
pub mod status;
pub mod taxonomy;
pub mod worldline;

pub use boundary::{
    BoundaryDisposition, BoundaryRequest, ConsentBasis, EscalationNotice, EscalationRoute,
    EscalationTrigger, OutputUse, RequestContext, ResearchBoundary, ResearchOutput, TerminalAction,
};
pub use computational_execution_contract_model::{
    computational_execution_contract_manifest, model_computational_execution_contract,
    model_computational_execution_contract_json, validate_computational_execution_contract_json,
    ComputationalExecutionContractError, ExecutionNode1, ExecutionRun2, ResearchWorkflowSpec1,
    CONTRACT_VERSION as COMPUTATIONAL_EXECUTION_CONTRACT_VERSION,
    FEATURE_ID as COMPUTATIONAL_EXECUTION_FEATURE_ID,
    INPUT_SCHEMA as COMPUTATIONAL_EXECUTION_INPUT_SCHEMA,
    OUTPUT_SCHEMA as COMPUTATIONAL_EXECUTION_OUTPUT_SCHEMA,
};
pub use error::OncoError;
pub use federated_provenance_signing_workflow::{
    compile_federated_provenance_signing, federated_provenance_signing_manifest,
    OncoProvenanceObject6, ProvenanceSigningError, ProvenanceSigningRequest6,
    SignedProvenanceDisposition, SignedProvenanceWorkflow9,
    CONTENT_TYPE as FEDERATED_PROVENANCE_CONTENT_TYPE,
    CONTRACT_VERSION as FEDERATED_PROVENANCE_CONTRACT_VERSION,
    FEATURE_ID as FEDERATED_PROVENANCE_FEATURE_ID,
    INPUT_SCHEMA as FEDERATED_PROVENANCE_INPUT_SCHEMA,
    OUTPUT_SCHEMA as FEDERATED_PROVENANCE_OUTPUT_SCHEMA,
};
pub use instrument_research_workbench::{
    instrument_research_workbench_manifest, qualify_instrument_actions, OncoInstrumentAction5,
    OncoInstrumentDisposition, OncoInstrumentError, OncoInstrumentReceipt5, OncoInstrumentRequest6,
    CONTENT_TYPE as ONCO_INSTRUMENT_CONTENT_TYPE,
    CONTRACT_VERSION as ONCO_INSTRUMENT_CONTRACT_VERSION, FEATURE_ID as ONCO_INSTRUMENT_FEATURE_ID,
    INPUT_SCHEMA as ONCO_INSTRUMENT_INPUT_SCHEMA, OUTPUT_SCHEMA as ONCO_INSTRUMENT_OUTPUT_SCHEMA,
};
pub use outcome::{
    AnalysedFollowUp, AnalysisBias, AnalysisOutcome, CensoringAssumption, CensoringReason,
    DeathCause, EndpointKind, Estimand, EventKind, FollowUp, IntercurrentEvent,
    IntercurrentEventStrategy, Population, SummaryMeasure, TerminalFact,
};
pub use response::{
    assess, ChangeHypothesis, ConfirmatoryStudy, ConfirmedProgression, CriterionDivergence,
    DiscriminatingEvidence, HypothesisSet, NotEvaluableReason, NotExcludedHypothesis,
    ProgressionBasis, ProgressionEvidence, ResponseAssessment, ResponseCall, ResponseCategory,
    ResponseCriterion, ResponseRequest, ThresholdSensitivity, TreatmentContext, TreatmentModality,
};
pub use status::{ObservationStatus, Observed};
pub use taxonomy::{
    classify, CnsEntity, CnsGrade, DiagnosticResolution, EntityCriteria, EvidenceObligation,
    EvidenceRole, Histology, MarkerCall, MarkerObservation, MarkerPanel, MolecularMarker,
    Reclassification, SatisfiedEvidence, SourceDiagnosis,
};
pub use worldline::{
    AcquisitionTime, AvailabilityTime, ClinicalObservation, ClinicalTrend, Clocks, Compartment,
    DaysFromBaseline, DirectionOfChange, ImagingModality, ImagingObservation, Karnofsky,
    Observation, RecordTime, ReleaseTime, SubjectRef, TargetLesion, Timepoint, TumourWorldline,
};
