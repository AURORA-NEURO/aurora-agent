//! Multiparty choreography: the part of the fabric the microkernel refuses to do.
//!
//! `bioprism-weave` is a trusted computing base and says so: it records typed acts, commitments,
//! grants and challenges, and it decides nothing about what is true. *"Claims and their challenges
//! both stay in the ledger; adjudication is somebody else's job."* This crate is that job, and the
//! four others that sit next to it — deciding what a well-formed multi-party protocol is
//! (23.06), whether a given one can stall (23.45), how a dispute is settled without the kernel
//! having to know any science (23.18), when a set of votes is actually a quorum (23.19), and what
//! a compensated transaction did and did not undo (23.21, 23.22).
//!
//! It never redefines a kernel concept. Acts, ledgers, capsules, authority grants and budgets are
//! weave's (23.05, 23.07, 23.08, 23.09, 23.13, 23.16, 23.49); this crate reads them
//! ([`adjudicate::open_challenges`] pairs a ledger challenge with the claim it targets) and adds
//! only the layer above.
//!
//! # The one rule shared by all five modules
//!
//! Each module has a state it can reach that means *"the procedure ran and did not settle this"*,
//! and in each case that state is structurally distinct from success:
//!
//! - [`check::Verdict::Inconclusive`] is not [`check::Verdict::Holds`]. No deadlock found within
//!   the bound is not the absence of deadlock.
//! - [`adjudicate::Ruling::Unresolved`] names the evidence that is missing. It is not a coin flip
//!   and there is deliberately no rule that defaults to the higher-authority party.
//! - [`quorum::QuorumOutcome::IndependenceUnverified`] is not a quorum. Five jurors reading one
//!   source are one vote, and [`quorum::QuorumOutcome::Reached`] carries the
//!   [`quorum::CheckedIndependence`] that justified the count, so the distinction survives
//!   serialisation.
//! - [`saga::SagaOutcome::CompensatedWithResidue`] is not a rollback. It lists what compensation
//!   could not undo.
//!
//! # What is deliberately absent
//!
//! No network, no transport, no real participants, no scheduler, no clock and no randomness.
//! Every function here is deterministic, which is what lets a counterexample trace, an
//! adjudication record and a saga outcome be compared across runs and across machines. The state
//! exploration in [`check`] is bounded and says so in its result type. Nothing in this crate
//! performs an effect; [`saga`] coordinates effects described by a caller-supplied outcome script,
//! which is how 23.22's chaos scenarios become reproducible tests instead of flaky ones.

pub mod adjudicate;
pub mod check;
pub mod protocol_execution_integrity_support;
pub mod local_protocol_execution_integrity_inference;
pub mod multimodal_protocol_execution_integrity_inference;
pub mod throughput_protocol_execution_integrity_inference;
pub mod federated_continual_protocol_execution_integrity_inference;
pub mod local_protocol_execution_integrity_contract_model;
pub mod multimodal_protocol_execution_integrity_contract_model;
pub mod throughput_protocol_execution_integrity_contract_model;
pub mod federated_continual_protocol_execution_integrity_contract_model;
pub mod local_protocol_execution_integrity_research_copilot;
pub mod multimodal_protocol_execution_integrity_research_copilot;
pub mod throughput_protocol_execution_integrity_research_copilot;
pub mod federated_continual_protocol_execution_integrity_research_copilot;
pub mod local_protocol_execution_integrity_workflow_fabric;
pub mod multimodal_protocol_execution_integrity_workflow_fabric;
pub mod throughput_protocol_execution_integrity_workflow_fabric;
pub mod federated_continual_protocol_execution_integrity_workflow_fabric;
pub mod quorum;
pub mod retrieval_synthesis_workbench;
pub mod saga;
pub mod session;

pub use adjudicate::{
    open_challenges, AdjudicationRecord, Assurance, Dispute, DisputeKind, Dissent, Evidence,
    EvidenceKind, EvidenceRequirement, OpenChallenge, Position, Procedure, Rule, RuleCondition,
    RuleOutcome, Ruling, RulingScope, Side,
};
pub use protocol_execution_integrity_support::{
    execute as execute_protocol_execution_integrity, manifest as protocol_execution_integrity_manifest,
    ProtocolExecutionArtifact4, ProtocolExecutionCard7, ProtocolExecutionIntegrityError,
    ProtocolExecutionRequest4, ProtocolStep4,
};
pub use check::{
    BoundHit, ChannelState, Counterexample, Event, ExplorationBound, ModelCheckReport, Property,
    RoleState, RoleStatus, StateSummary, System, TraceStep, Verdict,
};
pub use quorum::{
    CheckedIndependence, Cluster, Correlation, CorrelationReason, EvidenceBasis, Finding, Juror,
    JurorDissent, Jury, Question, QuestionKind, QuorumOutcome, QuorumRule, Vote,
};
pub use retrieval_synthesis_workbench::{
    capability_manifest as retrieval_workbench_capability_manifest,
    render as render_retrieval_workbench, EvidenceSynthesis, RetrievalCandidate,
    ScopedRetrievalQuery, WorkbenchError, CONTRACT_VERSION as RETRIEVAL_WORKBENCH_CONTRACT_VERSION,
    FEATURE_ID as RETRIEVAL_WORKBENCH_FEATURE_ID, INPUT_SCHEMA as RETRIEVAL_WORKBENCH_INPUT_SCHEMA,
    OUTPUT_SCHEMA as RETRIEVAL_WORKBENCH_OUTPUT_SCHEMA,
};
pub use saga::{
    ActionClass, Compensation, CompensationReport, Failure, FailurePolicy, Recovery,
    RecoveryLadder, Remediation, Residual, Restoration, Saga, SagaError, SagaOutcome, Step,
    StepOutcome, StepReport,
};
pub use session::{
    GlobalBranch, GlobalType, Label, LocalBranch, LocalType, ProtocolError, Role, WellFormedGlobal,
    UNFOLD_LIMIT,
};
