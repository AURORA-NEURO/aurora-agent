//! Provider-neutral neurosurgical research agent.
//!
//! This crate is a domain extension for the AURORA autonomous kernel. It provides a structured,
//! deterministic intake and a read-only tool router for glioma, cranial-base, craniofacial,
//! encephalocele, spina-bifida and Chiari-malformation research. It does not call a model,
//! access a network, read patient files, or execute a clinical action. A caller can therefore use
//! it with a local model, a rules engine, or a human-only workflow without adding a provider key.
//!
//! The central contract is intentionally conservative:
//!
//! * a request declares its purpose and any direct-identifier fields; individualized diagnosis,
//!   treatment, triage, urgent alerting and intervention planning are refused before tools run;
//! * observations and literature are carried as separate, typed inputs, with `NotCollected` and
//!   `Uninterpretable` remaining distinct from a measured finding;
//! * every plan starts with a safety and case-integrity tool, routes to specialty tools, and ends
//!   in a human-review hold; all built-in tools are read-only;
//! * evidence gaps are emitted explicitly, never converted into a low score or a reassuring
//!   default; the response digest binds the exact request bytes to the plan and report.
//! * a stateless session API can execute the route one tool at a time with chained, digest-bound
//!   checkpoints that a caller can persist and resume without server-side memory.
//! * the intake-mission seam can derive bounded local bundle filters from matched specialty
//!   vocabulary, while keeping the original free-text question out of returned reports.
//!
//! No clinical conclusion is represented by the output types. Hypotheses are labelled as
//! research hypotheses and every report carries a non-clinical-use notice. This is deliberate:
//! the repository's `bioprism-onco` boundary is research-only and the new surface must not create
//! an accidental path around it.
//!
//! # What is not implemented
//!
//! * No diagnosis, prognosis, treatment recommendation, triage decision or procedural
//!   instruction is produced.
//! * No imaging, pathology, genomics, operative note or EHR-content parser is included. Callers
//!   may submit a real, de-identified `CaseAssetManifest`, sanitized FHIR `Bundle` metadata, or
//!   standard DICOM JSON metadata import that records the existence, provenance and content
//!   digests of those assets. These paths never open asset bytes, extract identifiers, or
//!   interpret a scan/report; callers still supply summaries and provenance-bearing evidence
//!   records to the appropriate research tools.
//! * No network, database, model invocation, credential handling, scheduler, notification,
//!   device control, operating-room integration or autonomous physical effect exists here. The
//!   real-data snapshot is supplied by an explicit external refresh script; the Rust core remains
//!   offline.
//! * No clinical guideline is embedded and no numeric threshold is invented. Evidence tiers and
//!   specialty routes describe what must be checked, not what a clinician should do.
//!
//! The wire schema is intentionally small and versioned so that a deployment can wrap this crate
//! behind MCP, HTTP, a local command, or another provider-neutral adapter.

mod agent;
mod case_asset_manifest;
mod case_asset_review_disposition;
mod case_dicom;
mod case_dicom_workflow;
mod case_fhir;
mod catalogue;
mod error;
mod evidence_acquisition;
mod evidence_audit;
mod evidence_graph;
mod evidence_program;
mod evidence_synthesis;
mod glioma;
mod glioma_molecular_map;
mod intake;
mod literature_link;
mod mission_audit;
mod model;
mod public_literature;
mod public_literature_draft_audit;
mod public_literature_integrity;
mod public_literature_matrix;
mod public_literature_portfolio;
mod public_literature_reasoning_context;
mod public_literature_refresh;
mod public_literature_review_queue;
mod public_literature_workbench;
mod real_data;
mod real_data_autonomous_workflow;
mod real_data_cohort_landscape;
mod real_data_coverage;
mod real_data_diff;
mod real_data_draft_audit;
mod real_data_evidence_packet;
mod real_data_freshness;
mod real_data_molecular_coverage;
mod real_data_reasoning_context;
mod real_data_reconciliation;
mod real_data_refresh;
mod real_data_review_disposition;
mod real_data_review_queue;
mod real_data_trial_landscape;
mod research_brief;
mod research_plan;
mod specialty_evidence_map;
mod temporal;

pub use agent::NeurosurgicalAgent;
pub use case_asset_manifest::*;
pub use case_asset_review_disposition::*;
pub use case_dicom::*;
pub use case_dicom_workflow::*;
pub use case_fhir::*;
pub use catalogue::{required_capabilities, tool_catalogue};
pub use error::NeurosurgeryError;
pub use evidence_acquisition::{
    advance_with_case_assets_and_dispositions, compile_with_case_assets_and_dispositions,
    finish_with_case_assets_and_dispositions, start_with_case_assets_and_dispositions,
    EvidenceAcquisitionAdvanceResult, EvidenceAcquisitionEvent, EvidenceAcquisitionExecutionReport,
    EvidenceAcquisitionExecutionStep, EvidenceAcquisitionQuery, EvidenceAcquisitionReport,
    EvidenceAcquisitionSession, EvidenceAcquisitionSessionStatus, EvidenceAcquisitionSourceQuery,
    EvidenceAcquisitionStartResult, EvidenceAcquisitionStep, EvidenceAcquisitionStepStatus,
    EvidenceAcquisitionTrigger, EVIDENCE_ACQUISITION_EXECUTION_SCHEMA_VERSION,
    EVIDENCE_ACQUISITION_SCHEMA_VERSION, EVIDENCE_ACQUISITION_SESSION_SCHEMA_VERSION,
    MAX_EVIDENCE_ACQUISITION_ADVANCE_STEPS, MAX_EVIDENCE_ACQUISITION_REFERENCES,
    MAX_EVIDENCE_ACQUISITION_STEPS,
};
pub use evidence_audit::*;
pub use evidence_graph::*;
pub use evidence_program::*;
pub use evidence_synthesis::*;
pub use glioma::*;
pub use glioma_molecular_map::*;
pub use intake::*;
pub use literature_link::*;
pub use mission_audit::*;
pub use model::*;
pub use public_literature::*;
pub use public_literature_draft_audit::*;
pub use public_literature_integrity::*;
pub use public_literature_matrix::*;
pub use public_literature_portfolio::*;
pub use public_literature_reasoning_context::*;
pub use public_literature_refresh::*;
pub use public_literature_review_queue::*;
pub use public_literature_workbench::*;
pub use real_data::*;
pub use real_data_autonomous_workflow::*;
pub use real_data_cohort_landscape::*;
pub use real_data_coverage::*;
pub use real_data_diff::*;
pub use real_data_draft_audit::*;
pub use real_data_evidence_packet::*;
pub use real_data_freshness::*;
pub use real_data_molecular_coverage::*;
pub use real_data_reasoning_context::*;
pub use real_data_reconciliation::*;
pub use real_data_refresh::*;
pub use real_data_review_disposition::*;
pub use real_data_review_queue::*;
pub use real_data_trial_landscape::*;
pub use research_brief::*;
pub use research_plan::*;
pub use specialty_evidence_map::*;
pub use temporal::*;

/// Version of the JSON contract emitted by this crate.
pub const NEUROSURGERY_SCHEMA_VERSION: &str = "bioprism-neurosurgery/0.1";

/// Maximum number of read-only route steps a one-call autonomous session may execute.
/// Callers that need a larger workflow must persist and resume checkpoints explicitly.
pub const MAX_SESSION_STEPS: usize = 256;
