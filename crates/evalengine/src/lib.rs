//! The evaluation engine: scoring, attribution and the capability posterior.
//!
//! Implements blueprint section 07 (Evaluation Engine) and the scoring half of section 06
//! (Benchmark Compiler) — the part that consumes what the compiler produced rather than the part
//! that produces it.
//!
//! # What this crate is for
//!
//! Blueprint 00.01 says the output of PRISM is "a capability posterior rather than a single
//! score", and that a matched fork isolates *which component* explains a difference. Both claims
//! are easy to state and easy to quietly violate, so this crate is organised around making the
//! violations impossible to reach rather than merely discouraged:
//!
//! - [`ladder`] composes tiered evidence strongest-first, so a judge has no code path by which to
//!   raise a deterministic conclusion (07.01);
//! - [`score`] keeps outcome and justification on separate axes, so a right answer for the wrong
//!   reason is its own category rather than a pass or a fail (07.05);
//! - [`attribution`] refuses to attribute a difference to any component when more than one
//!   component varied (07.07);
//! - [`cluster`] reports an effective sample size beside every aggregate, so instances descended
//!   from one parent cannot be counted as independent evidence (07.06);
//! - [`posterior`] returns a capability vector, and its `overall()` is a `Result` that refuses
//!   unless declared coverage floors were met (07.05, 07.12).
//!
//! # What this crate does not do
//!
//! It does not *produce* judgements. The evidence ladder over oracle verdicts lives in
//! `bioprism-oracle`; running evaluators, sandboxing untrusted benchmark code, segmenting
//! trajectories and synthesising oracles all live elsewhere. This crate takes tiered
//! [`ladder::Contribution`]s as given and answers what may be concluded, attributed and published
//! from them.
//!
//! It also does not implement the parts of section 07 that need machinery this crate deliberately
//! has no access to: cost and latency accounting (07.08), calibration curves and reliability
//! diagrams (07.06), benchmark-health drift detection (07.11), and the human resolution workflow
//! that a disputed result should enter (07.01). Where those are load-bearing for an invariant here,
//! the invariant is stated and the measurement is left to the caller.

pub mod analysis_qualification;
pub mod attribution;
pub mod bridge;
pub mod cluster;
pub mod error;
pub mod evaluation_observability;
pub mod federated_evaluation;
pub mod federated_protocol_simulation_copilot;
pub mod local_mechanism_exploration_assurance;
pub mod ladder;
pub mod multimodal_replication;
pub mod posterior;
pub mod replication;
pub mod research_release;
pub mod score;

pub use analysis_qualification::{
    analysis_qualification_manifest, qualify_analysis, AnalysisCandidate,
    AnalysisQualificationError, AnalysisQualificationRequest, AnalysisQuestion,
    IdentificationStatus, QualificationVerdict, QualifiedAnalysisResult,
    FEATURE_CONTRACT_VERSION as ANALYSIS_QUALIFICATION_FEATURE_VERSION,
    FEATURE_ID as ANALYSIS_QUALIFICATION_FEATURE_ID,
};
pub use attribution::{
    attribute, ArmSpec, Attribution, AttributionClaim, AttributionReport, ComponentEffect,
    EffectDirection, MatchedFork, RefusalReason,
};
pub use bridge::{contribution_from_verdict, digest, Provenance};
pub use cluster::{ClusteredEstimate, ClusteredSample, IccEstimate};
pub use error::EvalError;
pub use evaluation_observability::{
    compile_evaluation_card, evaluation_observability_manifest, CapabilityRunObservation,
    EvaluationCardReceipt, EvaluationCardRequest, EvaluationObservabilityError,
    FEATURE_CONTRACT_VERSION as EVALUATION_OBSERVABILITY_FEATURE_VERSION,
    FEATURE_ID as EVALUATION_OBSERVABILITY_FEATURE_ID,
};
pub use federated_evaluation::{
    evaluate_federated_evaluation, federated_evaluation_manifest, FederatedEvaluationDisposition,
    FederatedEvaluationError, FederatedEvaluationReceipt, FederatedEvaluationRequest,
    FederatedEvaluationSite, FederatedEvaluationSiteDisposition, FederatedEvaluationSiteEntry,
    FEATURE_ID as FEDERATED_EVALUATION_FEATURE_ID,
    FEATURE_VERSION as FEDERATED_EVALUATION_FEATURE_VERSION,
};
pub use federated_protocol_simulation_copilot::{
    assure_evalengine_protocol, assure_evalengine_protocol_json,
    evalengine_protocol_simulation_copilot_manifest, EvalenginePeerProtocolSummary,
    EvalengineProtocolDraft, EvalengineProtocolSimulationCopilotError,
    EvalengineProtocolSimulationReport,
    CONTRACT_VERSION as EVALENGINE_PROTOCOL_SIMULATION_COPILOT_CONTRACT_VERSION,
    FEATURE_ID as EVALENGINE_PROTOCOL_SIMULATION_COPILOT_FEATURE_ID,
    INPUT_SCHEMA as EVALENGINE_PROTOCOL_SIMULATION_COPILOT_INPUT_SCHEMA,
    OUTPUT_SCHEMA as EVALENGINE_PROTOCOL_SIMULATION_COPILOT_OUTPUT_SCHEMA,
};
pub use local_mechanism_exploration_assurance::{
    assure_evalengine_local_mechanism_exploration,
    evalengine_local_mechanism_exploration_assurance_manifest,
    EvalengineAssuranceDisposition, EvalengineCandidateState,
    EvalengineMechanismCandidate, EvalengineMechanismExplorationAssuranceError,
    EvalengineMechanismPortfolio7, EvalengineMechanismQuestion1,
    CONTRACT_VERSION as EVALENGINE_LOCAL_MECHANISM_EXPLORATION_CONTRACT_VERSION,
    FEATURE_ID as EVALENGINE_LOCAL_MECHANISM_EXPLORATION_FEATURE_ID,
};
pub use ladder::{
    compose, Contribution, Detail, Disagreement, EvidenceRef, ScoreTier, ScoredResult,
    SuppressedRaise, UnknownPolicy,
};
pub use multimodal_replication::{
    evaluate_multimodal_replication, multimodal_replication_manifest, ModalityReceipt,
    MultimodalReplicationDisposition, MultimodalReplicationError, MultimodalReplicationObservation,
    MultimodalReplicationPolicy, MultimodalReplicationReport, MultimodalReplicationRequest,
    MultimodalReplicationSummary, StudyComparability,
    FEATURE_ID as MULTIMODAL_REPLICATION_FEATURE_ID,
    FEATURE_VERSION as MULTIMODAL_REPLICATION_FEATURE_VERSION,
};
pub use posterior::{
    unprovenanced, CapabilityEstimate, CapabilityPosterior, CoverageFloor, Dominance, GateScalar,
    Observation, ReleaseGate,
};
pub use replication::{
    evaluate_replication, manifest as replication_manifest, ReplicationDisposition,
    ReplicationError, ReplicationObservation, ReplicationOutcome, ReplicationPolicy,
    ReplicationReport, ReplicationRequest, ReplicationSummary,
};
pub use research_release::{
    review_release, AdversarialCheck, ReleaseReview, ReleaseReviewPolicy, ReplicationEvidence,
};
pub use score::{
    credit_for, Conclusion, Constraint, Credit, CreditBasis, CreditPolicy, Justification, Outcome,
    ResultScore, Rubric, RubricProgress, Satisfaction, UnknownCredit, Veto, VetoKind,
};

/// Schema version for everything this crate serializes.
///
/// Blueprint 07.01's invariant: "changes to schemas and scoring semantics are versioned and cannot
/// retroactively rewrite already published results." Consumers pin this and refuse to merge
/// reports across a bump rather than silently reinterpreting old ones.
pub const EVALENGINE_SCHEMA_VERSION: &str = "07.0.1";
