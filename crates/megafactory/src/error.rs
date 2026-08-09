//! Typed errors, one enum per covered module.
//!
//! Split by module rather than unified because the six modules share no failure vocabulary: a
//! widened release mode and a stale fence token are not variants of one thing, and a single enum
//! would force every caller to match arms that its call can never produce.

use thiserror::Error;

/// Failures while authoring or reviewing an observed-data world (35.02).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AuthoringError {
    #[error(
        "world `{world}` names parent `{parent}`, which was not supplied to the lineage check"
    )]
    UnknownParent { world: String, parent: String },
    #[error(
        "world `{world}` requests release mode `{requested}` but its parent `{parent}` is released \
         as `{parent_mode}`; descendants inherit access and privacy restrictions and may not widen \
         them"
    )]
    ReleaseModeWidened {
        world: String,
        parent: String,
        parent_mode: &'static str,
        requested: &'static str,
    },
    #[error("world `{world}` records the same artifact id `{artifact}` twice")]
    DuplicateArtifact { world: String, artifact: String },
    #[error("world `{world}` records the same latent question `{question}` twice")]
    DuplicateLatentQuestion { world: String, question: String },
    #[error("lineage for world `{0}` contains a cycle")]
    LineageCycle(String),
}

/// Failures while constructing or auditing a semi-synthetic world (35.03).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InsertionError {
    #[error("insertion names background `{0}`, which is not in the panel")]
    UnknownBackground(String),
    #[error("background `{0}` appears twice in the panel")]
    DuplicateBackground(String),
    #[error(
        "background `{0}` carries two insertions; a background hosts at most one latent state"
    )]
    DoubleInsertion(String),
    #[error(
        "insertion into background `{background}` asserts latent onset outside the background's \
         own observation interval; the world would claim a state at a time the background does not \
         cover"
    )]
    OnsetOutsideBackground { background: String },
    #[error("a detector call names background `{0}`, which is not in the panel")]
    UnknownDetectorCall(String),
}

/// Failures in the mechanistic simulation and assay-twin factory (35.04).
#[derive(Debug, Clone, PartialEq, Error)]
pub enum TwinError {
    #[error(
        "model `{0}` states no known misspecification; a twin that claims none is asserting it is \
         the system rather than a model of it"
    )]
    NoStatedMisspecification(String),
    #[error("model `{model}` has no compartments")]
    NoCompartments { model: String },
    #[error(
        "model `{model}` supplies a {rows}x{cols} rate matrix for {compartments} compartments"
    )]
    RateShape {
        model: String,
        rows: usize,
        cols: usize,
        compartments: usize,
    },
    #[error("model `{model}` supplies a non-finite rate at ({row}, {col})")]
    NonFiniteRate {
        model: String,
        row: usize,
        col: usize,
    },
    #[error("initial state has {given} entries for model `{model}`'s {expected} compartments")]
    InitialStateShape {
        model: String,
        given: usize,
        expected: usize,
    },
    #[error("model `{model}` has no compartment named `{compartment}`")]
    UnknownCompartment { model: String, compartment: String },
    #[error(
        "out-of-model robustness needs at least one alternative model; a discrepancy probe over a \
         single model measures the model against itself"
    )]
    NoAlternatives,
    #[error(
        "alternative model `{alternative}` has compartments {alternative_compartments:?}, which \
         differ from reference model `{reference}`'s {reference_compartments:?}; the two \
         counterfactuals are not comparable"
    )]
    IncomparableAlternative {
        reference: String,
        alternative: String,
        reference_compartments: Vec<String>,
        alternative_compartments: Vec<String>,
    },
}

/// Failures in trajectory capture, redaction, and completeness accounting (35.06).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CaptureError {
    #[error("span sequence {seq} occurs twice in session `{session}`")]
    DuplicateSequence { session: String, seq: u64 },
    #[error("span sequence {seq} follows {previous} in session `{session}`; spans are appended in sequence order")]
    OutOfOrder {
        session: String,
        seq: u64,
        previous: u64,
    },
    #[error(
        "session `{session}` is gapped at {missing:?} and cannot be compiled into decision \
         boundaries; a boundary inferred across a gap is a guess about events nobody recorded"
    )]
    GappedSession { session: String, missing: Vec<u64> },
    #[error("redaction policy names field `{0}`, which no span carries")]
    RedactionTargetAbsent(String),
}

/// Failures in boundary detection and agreement measurement (35.07).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BoundaryError {
    #[error("boundary sequence {seq} occurs twice in `{set}`")]
    DuplicateBoundary { set: String, seq: u64 },
    #[error(
        "the reference boundary set names no annotator; an agreement figure against an anonymous \
         reference measures agreement with nobody"
    )]
    UnattributedReference,
    #[error("reference boundary at {seq} is outside the session's span range {first}..={last}")]
    BoundaryOutsideSession { seq: u64, first: u64, last: u64 },
    #[error("cell span {start}..{end} is empty")]
    EmptyCellSpan { start: u64, end: u64 },
}

/// Failures in placement, fencing, and execution accounting (35.13).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PlacementError {
    #[error(
        "worker `{worker}` does not declare resource class `{class}`; capability is declared, not \
         inferred"
    )]
    ClassNotDeclared { worker: String, class: String },
    #[error(
        "worker `{worker}` is unattested and the work carries access tier `{tier}`; an unattested \
         worker may not execute restricted work"
    )]
    UnattestedWorker { worker: String, tier: &'static str },
    #[error(
        "job `{job}`'s inputs live in `{data_locale}` at the enclave tier and worker `{worker}` is \
         in `{worker_locale}`; enclave data does not leave its locale"
    )]
    EnclaveTransfer {
        job: String,
        worker: String,
        data_locale: String,
        worker_locale: String,
    },
    #[error(
        "worker `{worker}` sits in trust domain `{domain}`, which also hosts the oracle judging \
         job `{job}`; oracle independence is preserved in distributed and federated execution"
    )]
    OracleDomainCollision {
        worker: String,
        domain: String,
        job: String,
    },
    #[error("fence {presented} for job `{job}` is stale; the registry has issued {current}")]
    StaleFence {
        job: String,
        presented: u64,
        current: u64,
    },
    #[error("no fence has been issued for job `{0}`")]
    NoFenceIssued(String),
    #[error("item `{0}` committed as executed but is not in the enumerated corpus")]
    ExecutedItemNotEnumerated(String),
    #[error("effective size could not be measured over the executed subset: {0}")]
    ExecutedSubset(String),
}
