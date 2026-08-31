//! Token-efficient biological inference: the parts of blueprint section 39 that are about keeping
//! a compact context *honest* rather than about producing one.
//!
//! Section 39's thesis, stated in 39.01 and enforced by `bioprism-obligation`, is that **a smaller
//! prompt is not a success if it increases overclaiming**. `bioprism-obligation` owns the core of
//! that: the decision obligation graph (39.06), the non-compressible invariants (39.05), the token
//! budget controller (39.16), and the omission ledger and sufficiency certificate (39.17).
//! `bioprism-section` owns the Decision Section and the influence vocabulary; `bioprism-fiber` is
//! the compiler. This crate takes the modules that sit *around* a compile — proving one did not
//! silently degrade, knowing when one has gone stale, attributing a saving to a component, handing
//! one to a peer, and contracting what a summary discards.
//!
//! # What is here, and why these five carry the crate
//!
//! - [`fixture`] — **golden context fixtures (39.21)**. A pinned compile, set-valued because more
//!   than one projection is often legal, and a drift report that names *what* moved. A fixture that
//!   only says "digest differs" tells a developer nothing they can act on, so the report is a typed
//!   list of drifts each naming a node, an obligation, a slot or an omission group.
//! - [`staleness`] — **staleness, TTL and recomputation (39.18)**. Three states, not two.
//!   [`staleness::Currency`] has no `is_fresh`, and *"I could not check"* is
//!   [`staleness::Currency::Undetermined`], a third answer rather than a synonym for fresh. There
//!   is no clock in this workspace, so currency is judged against a supplied epoch or a world
//!   digest, never wall time. This follows `bioprism-hubapi`, which took the same position for
//!   registry mirrors, for the same reason: an offline system that defaults to "current" launders a
//!   missing observation into a guarantee.
//! - [`ablation`] — **ablations and experimental design (39.23)**. A contrast that varied more than
//!   one component is refused, not scored. `bioprism-evalengine` applies that rule when *scoring* a
//!   matched fork; this applies it to the *design*, so a confounded experiment is refused before it
//!   produces a number somebody wants to believe.
//! - [`projection`] — **multi-agent context projection (39.11)**. What a peer receives is a
//!   projection of what you compiled, and it carries its own omission record. `bioprism-weave`'s
//!   capsule already does the transport; the question here is what a projection *costs* and what it
//!   *drops*, and whether every drop is accounted for.
//! - [`summary`] — **summarisation contracts (39.13)**. A summary declares what it preserves and
//!   what it discards. A summary with no declared loss is not a summary, it is a claim, and
//!   [`summary::Discarded`] makes that unrepresentable: losslessness requires an argument.
//!
//! Four more modules are implemented at the size their specifications warrant: [`compiler`] for the
//! API and CLI surface of 39.20, [`recovery`] for the failure taxonomy of 39.24, [`literature`] for
//! the claim context of 39.14, and [`temporal`] for the OncoWorld temporal firewall of 39.15.
//! [`context`] holds the compiled-context value they all share.
//!
//! # Prose modules, named rather than counted
//!
//! Two of section 39's modules are not implementable and are not claimed as implemented:
//!
//! - **39.01, the token economy thesis.** It has no build contract, no inputs or outputs, and no
//!   invariants section. It is an argument, an optimisation objective in LaTeX, and two lists
//!   ("what should be compressed", "what must not be compressed away"). The second list *is*
//!   encoded here — as [`context::NodeKind::is_protected_class`] — but the module is prose and
//!   citing it as built would be inflating a coverage number.
//! - **39.25, the implementation plan.** It has the standard build-contract skeleton, but its
//!   content is an eight-week delivery schedule and its "invariants" are process commitments about
//!   shipping order. One clause of it is a runtime invariant — *"full-context baseline remains
//!   available"* — and that one is enforced, in
//!   [`ablation::AblationDesign::validate`]. The rest is a plan.
//!
//! # Every token number in this crate is an estimate
//!
//! There is no tokenizer here and there cannot be one offline. Counts come from
//! [`bioprism_obligation::TokenEstimate`], which carries the
//! [`bioprism_obligation::EstimationMethod`] that produced them, and totals over mixed estimators
//! degrade to [`bioprism_obligation::EstimationMethod::Mixed`] because a sum measured with two
//! rulers is nobody's number. `bioprism-docgraph` set this precedent and this crate does not weaken
//! it: [`fixture::ContextDrift::EstimatorChanged`] is a critical drift precisely because a token
//! comparison across estimators is not a comparison at all.
//!
//! # Where section 39 assumes a measurement this crate cannot make
//!
//! Several of the section's operational metrics require running a model, a tokenizer, or an oracle.
//! Those are modelled as *declared inputs a caller supplies* and never as values this crate
//! computes: [`ablation::ArmOutcome::validity`] is an outcome somebody else scored,
//! [`recovery::ContextFailure`] is a diagnosis somebody else reached, and
//! [`staleness::WorldObservation`] is an observation somebody else made. Nothing here fabricates
//! one, and every type that holds one says whose it is.

pub mod ablation;
pub mod compression_integrity_support;
pub mod local_compression_integrity_inference;
pub mod multimodal_compression_integrity_inference;
pub mod throughput_compression_integrity_inference;
pub mod federated_continual_compression_integrity_inference;
pub mod local_compression_integrity_contract_model;
pub mod multimodal_compression_integrity_contract_model;
pub mod throughput_compression_integrity_contract_model;
pub mod federated_continual_compression_integrity_contract_model;
pub mod local_compression_integrity_research_copilot;
pub mod multimodal_compression_integrity_research_copilot;
pub mod throughput_compression_integrity_research_copilot;
pub mod federated_continual_compression_integrity_research_copilot;
pub mod local_compression_integrity_workflow_fabric;
pub mod multimodal_compression_integrity_workflow_fabric;
pub mod throughput_compression_integrity_workflow_fabric;
pub mod federated_continual_compression_integrity_workflow_fabric;
pub mod compiler;
pub mod context;
pub mod error;
pub mod fixture;
pub mod literature;
pub mod projection;
pub mod recovery;
pub mod staleness;
pub mod summary;
pub mod temporal;

pub use ablation::{
    AblationArm, AblationDesign, AblationReport, ArmOutcome, AttributionClaim, ComponentSetting,
    ConfoundReason, Contrast, ContrastReport, ContrastVerdict, ContextComponent, LearnedSelector,
    PreDeclaration, ValidityOutcome,
};
pub use compression_integrity_support::{qualify as qualify_compression_integrity, manifest as compression_integrity_manifest, CompressionClaim4, CompressionIntegrityArtifact4, CompressionIntegrityCard7, CompressionIntegrityError, CompressionIntegrityRequest4, BOUNDARY as COMPRESSION_INTEGRITY_BOUNDARY, CONTENT_TYPE as COMPRESSION_INTEGRITY_CONTENT_TYPE};
pub use local_compression_integrity_inference::*;
pub use multimodal_compression_integrity_inference::*;
pub use throughput_compression_integrity_inference::*;
pub use federated_continual_compression_integrity_inference::*;
pub use local_compression_integrity_contract_model::*;
pub use multimodal_compression_integrity_contract_model::*;
pub use throughput_compression_integrity_contract_model::*;
pub use federated_continual_compression_integrity_contract_model::*;
pub use local_compression_integrity_research_copilot::*;
pub use multimodal_compression_integrity_research_copilot::*;
pub use throughput_compression_integrity_research_copilot::*;
pub use federated_continual_compression_integrity_research_copilot::*;
pub use local_compression_integrity_workflow_fabric::*;
pub use multimodal_compression_integrity_workflow_fabric::*;
pub use throughput_compression_integrity_workflow_fabric::*;
pub use federated_continual_compression_integrity_workflow_fabric::*;
pub use compiler::{
    compare, plan, ComparisonMode, ContextPlan, ContextRequest, PlanCandidate, PolicyComparison,
    ResolutionDepth, TokenEnvelope,
};
pub use context::{
    estimates_are_comparable, CompiledContext, ContextNode, NodeKind, Visibility,
};
pub use error::{
    AblationError, CompilerApiError, FixtureError, LiteratureError, ProjectionError, RecoveryError,
    StalenessError, SummaryError, TemporalError,
};
pub use fixture::{
    band_source, pin_single, ContextDrift, ContextExpectation, ContextFixture, CoverageGap,
    DriftSeverity, ExpectationComparison, FixtureBundle, FixtureClass, FixtureEntry, FixtureReport,
    FixtureVerdict, NodeExpectation, TokenBand,
};
pub use literature::{
    CitationRelation, ClaimCluster, ClaimQualifiers, EvidenceDepth, LiteratureClaim, SourceRecord,
    SupportSummary,
};
pub use projection::{
    project, validate_projection, DroppedNode, PeerProjection, ProjectionCost, ProjectionOmission,
    ProjectionPolicy,
};
pub use recovery::{
    ContextFailure, Diagnosis, Escalation, FailureStreak, PolicyAction, RecoveryRecord,
    ResolutionChange,
};
pub use staleness::{
    plan_recomputation, BiologicalValidity, CacheValidity, ContextEpoch, Currency,
    DependencyChange, FanOutCeiling, ImpactReport, InvalidationGraph, MissingObservation,
    ProvenanceEdge, RecomputationPlan, RecomputationUnit, RecomputeReason, ReusePolicy,
    ValidityAxis, ValidityDeclaration, WorldDigest, WorldObservation,
};
pub use summary::{
    Discarded, DiscardedAspect, Modality, PreservedAspect, Recovery, SourceArtifact,
    SummaryContract, SummaryObligation,
};
pub use temporal::{
    ClinicalContext, DecisionEpoch, DiagnosisHistory, DiagnosisRecord, EventKind, ResponseState,
    RetrospectivePlane, TemporalFirewall, TimelineEvent,
};
