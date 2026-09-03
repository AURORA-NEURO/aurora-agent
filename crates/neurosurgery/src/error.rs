//! Typed refusals and malformed-input errors.

use crate::{RequestUse, ToolCapability};
use thiserror::Error;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum NeurosurgeryError {
    #[error("unsupported schema {found:?}; expected {expected:?}")]
    UnsupportedSchema {
        found: String,
        expected: &'static str,
    },
    #[error("{field} must be non-empty")]
    EmptyField { field: &'static str },
    #[error("{field} contains a control character")]
    ControlCharacter { field: &'static str },
    #[error("{field} exceeds the {max}-byte safety bound")]
    FieldTooLong { field: &'static str, max: usize },
    #[error("{field} contains {found} items, over the {max}-item safety bound")]
    TooMany {
        field: &'static str,
        found: usize,
        max: usize,
    },
    #[error(
        "request contains direct-identifier field(s): {fields:?}; research output is de-identified"
    )]
    DirectIdentifiers { fields: Vec<String> },
    #[error("request use {use_case:?} ({description}) is outside the research-only boundary")]
    ClinicalUseRefused {
        use_case: RequestUse,
        description: &'static str,
    },
    #[error("requested tool {tool:?} is not available for this local agent")]
    UnknownTool { tool: ToolCapability },
    #[error("requested tool {tool:?} was listed more than once")]
    DuplicateTool { tool: ToolCapability },
    #[error("evidence record {id:?} must have a non-empty citation and title")]
    InvalidEvidence { id: String },
    #[error("request could not be digested: {0}")]
    Digest(String),
    #[error("request JSON is invalid: {0}")]
    Json(String),
    #[error("real glioma data bundle rejected: {reason}")]
    RealDataRejected { reason: String },
    #[error("glioma molecular panel rejected: {reason}")]
    GliomaPanelRejected { reason: String },
    #[error("temporal observation metadata rejected: {reason}")]
    TemporalRejected { reason: String },
    #[error("real-data routing is only implemented for glioma, not {specialty:?}")]
    RealDataSpecialtyUnsupported { specialty: crate::Specialty },
    #[error("neurosurgical session rejected: {reason}")]
    SessionRejected { reason: String },
}
