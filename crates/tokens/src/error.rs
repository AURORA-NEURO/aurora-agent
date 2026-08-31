//! Typed failures, one enum per module concern.
//!
//! Every module of section 39 carries the same clause: *"a failure must be emitted as a typed event
//! with the module ID … and whether the failure invalidates only the current projection or the
//! underlying result."* This crate cannot emit events — there is no bus here — but it can make each
//! declared failure class a distinct variant with the operands a caller would need to build such an
//! event, which is the half that belongs in a library.
//!
//! Errors are split by concern rather than pooled. A caller reconciling a golden fixture should not
//! have to match on ablation confounding, and no `From` conversion exists between these enums,
//! because a staleness fault must not be able to arrive at a caller wearing a fixture's clothes.
//!
//! # The refusals are the interesting variants
//!
//! Several variants exist to make a *silently plausible* outcome impossible rather than to report
//! an accident: [`AblationError`]'s confound refusals, [`SummaryError::LossNotDeclared`],
//! [`ProjectionError::HoldoutWouldBeProjected`], [`StalenessError::CurrencyUndetermined`]. Each is
//! documented with what would otherwise have happened.

use thiserror::Error;

/// Failures of golden context fixtures (39.21).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FixtureError {
    #[error(
        "fixture `{0}` accepts no projections; a golden with an empty accepted set can never pass"
    )]
    NoAcceptedProjections(String),

    #[error("fixture `{fixture}` lists expectation `{expectation}` twice")]
    DuplicateExpectation {
        fixture: String,
        expectation: String,
    },

    #[error(
        "fixture `{fixture}` expects node `{node}`, which the world marks as an evaluator holdout; \
         a golden that pins hidden truth teaches the compiler to emit it"
    )]
    HoldoutLeakedIntoFixture { fixture: String, node: String },

    #[error(
        "fixture `{fixture}` pins a rendering digest for node `{node}` but records no semantic \
         expectation for it; that is a prose snapshot wearing a fixture's name"
    )]
    RenderingPinnedWithoutSemantics { fixture: String, node: String },

    #[error("fixture bundle `{bundle}` has no fixture covering `{requirement}`")]
    BundleCoverageGap { bundle: String, requirement: String },

    #[error("fixture `{0}` could not be content-addressed: {1}")]
    NotAddressable(String, String),
}

/// Failures of staleness, TTL, invalidation and recomputation (39.18).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum StalenessError {
    #[error(
        "context `{context}` is {detail}, and the reuse policy does not accept an unverifiable \
         currency; \"I could not check\" is not \"it is current\""
    )]
    CurrencyUndetermined { context: String, detail: String },

    #[error("context `{context}` is expired against {axis}: {detail}")]
    Expired {
        context: String,
        axis: String,
        detail: String,
    },

    #[error(
        "context `{context}` declares no validity at all, so nothing can ever mark it stale; \
         a cache entry that cannot expire is not fresh, it is unaccountable"
    )]
    NoDeclaredValidity { context: String },

    #[error("provenance edge from `{from}` names dependency `{to}`, which is not in the graph")]
    DanglingProvenanceEdge { from: String, to: String },

    #[error("provenance cycle reachable from `{0}`; invalidation would not terminate")]
    ProvenanceCycle(String),

    #[error(
        "invalidating {changed} node(s) would recompute {fan_out} of {total}, past the ceiling of \
         {ceiling}; this is an invalidation storm, not a targeted refresh"
    )]
    InvalidationFanOutExceeded {
        changed: usize,
        fan_out: usize,
        total: usize,
        ceiling: usize,
    },
}

/// Failures of ablation and experimental design (39.23).
///
/// The refusals mirror the rule `bioprism-evalengine` applies when *scoring* a matched fork: an
/// arm pair that varied more than one component supports no statement about either. These fire
/// earlier, on the design, so a confounded experiment can be refused before it is run rather than
/// after it has produced a number somebody wants to believe.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AblationError {
    #[error("ablation invariant violated: {detail}")]
    InvariantViolation { detail: String },

    #[error("ablation design `{0}` declares fewer than two arms")]
    TooFewArms(String),

    #[error("ablation design `{design}` declares arm `{arm}` twice")]
    DuplicateArm { design: String, arm: String },

    #[error(
        "design `{design}` declares no full-context reference arm; without one, a token saving has \
         nothing to be a saving against"
    )]
    NoFullContextBaseline { design: String },

    #[error(
        "design `{design}` predeclares no outcome or margin; an ablation scored against a margin \
         chosen after seeing the result is not an ablation"
    )]
    NoPreDeclaration { design: String },

    #[error(
        "design `{design}` reports a cost comparison for `{contrast}` with no validity outcome; \
             39.22 forbids compression ratio as a criterion on its own"
    )]
    CostReportedWithoutValidity { design: String, contrast: String },

    #[error("contrast `{contrast}` names arm `{arm}`, which the design does not declare")]
    UnknownArm { contrast: String, arm: String },
}

/// Failures of multi-agent context projection (39.11).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProjectionError {
    #[error(
        "projecting to role `{role}` would include node `{node}`, which is evaluator holdout state; \
         no policy makes this legal"
    )]
    HoldoutWouldBeProjected { role: String, node: String },

    #[error(
        "projecting to role `{role}` would include node `{node}`, private to role `{owner}`, and \
         the policy grants no access to that peer's state"
    )]
    PeerPrivateWouldLeak {
        role: String,
        node: String,
        owner: String,
    },

    #[error(
        "projection to role `{role}` drops contradiction node `{node}` while the policy declares \
         contradiction edges mandatory"
    )]
    MandatoryContradictionDropped { role: String, node: String },

    #[error(
        "projection to role `{role}` drops {dropped} node(s) but its omission record accounts for \
         {accounted}; an unrecorded drop is exactly the defect the ledger exists to catch"
    )]
    DropNotAccountedFor {
        role: String,
        dropped: usize,
        accounted: usize,
    },

    #[error(
        "projection to role `{role}` claims sufficiency `{projected_status}` while the source \
         context was `{source_status}`; a projection cannot know more than what it was projected \
         from"
    )]
    SufficiencyStrengthenedByProjection {
        role: String,
        source_status: String,
        projected_status: String,
    },

    #[error("projection to role `{0}` could not be content-addressed: {1}")]
    NotAddressable(String, String),
}

/// Failures of table, matrix, image and sequence summarization (39.13).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SummaryError {
    #[error(
        "summary `{0}` declares a discard list with no entries; a summary that claims to lose \
         nothing must say why it is lossless, and one that loses something must say what"
    )]
    LossNotDeclared(String),

    #[error("summary `{0}` claims to be lossless without an argument for why")]
    LosslessWithoutArgument(String),

    #[error(
        "summary `{summary}` spans sources with different identity `{left}` and `{right}`; \
         summarizing across identities produces a number that is about nothing"
    )]
    IncompatibleIdentity {
        summary: String,
        left: String,
        right: String,
    },

    #[error(
        "summary `{summary}` spans coordinate systems `{left}` and `{right}`; positions from two \
         reference frames do not aggregate"
    )]
    IncompatibleCoordinateSystem {
        summary: String,
        left: String,
        right: String,
    },

    #[error("summary `{summary}` spans units `{left}` and `{right}`")]
    IncompatibleUnits {
        summary: String,
        left: String,
        right: String,
    },

    #[error(
        "the obligation `{obligation}` depends on tails or rare states, and summary `{summary}` \
         preserves no distribution shape; a mean would hide the state being decided on"
    )]
    TailSensitiveWithoutShape { summary: String, obligation: String },

    #[error("summary `{summary}` of a {modality} artifact preserves no {required}")]
    RequiredAspectMissing {
        summary: String,
        modality: String,
        required: String,
    },

    #[error("summary `{summary}` names no source locator, so nothing can be expanded back")]
    NoSourceLocator { summary: String },

    #[error("summary `{summary}` has no source artifacts")]
    NoSources { summary: String },
}

/// Failures of the context compiler API surface (39.20).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CompilerApiError {
    #[error(
        "comparison `{comparison}` varies {varied:?} in addition to the context policy; 39.20 \
         permits comparison mode to change only the declared context policy"
    )]
    ComparisonVariesMoreThanPolicy {
        comparison: String,
        varied: Vec<String>,
    },

    #[error("comparison `{comparison}` varies nothing; both requests name policy `{policy}`")]
    ComparisonVariesNothing { comparison: String, policy: String },

    #[error(
        "the requested envelope of {envelope} tokens is below the mandatory closure estimate of \
         {mandatory}; mandatory invariants are not tradeable for tokens"
    )]
    EnvelopeBelowMandatoryClosure { envelope: usize, mandatory: usize },

    #[error("a dry run may not resolve restricted node `{0}`; it returns handles or nothing")]
    DryRunTouchedRestrictedData(String),

    #[error("request could not be content-addressed: {0}")]
    NotAddressable(String),
}

/// Failures of context failure recovery (39.24).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RecoveryError {
    #[error(
        "the proposed escalation for failure `{failure}` would expose holdout node `{node}`; \
         insufficiency is not solved by revealing the evaluator's answer"
    )]
    WouldExposeHoldout { failure: String, node: String },

    #[error("selective escalation for failure `{0}` names no nodes; that is a whole-context recompile wearing a cheaper name")]
    SelectiveEscalationNamesNoNodes(String),

    #[error(
        "a whole-context recompile for failure `{0}` states no reason why selective escalation was \
         insufficient"
    )]
    WholeRecompileUnjustified(String),

    #[error("recovery for failure `{0}` discards the failed artifact, leaving nothing to audit")]
    FailedArtifactNotPreserved(String),
}

/// Failures of literature claim context (39.14).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LiteratureError {
    #[error(
        "claim `{claim}` is supported only by citation edges; a citation records that one paper \
         mentioned another, not that it found the same thing"
    )]
    CitationCountedAsSupport { claim: String },

    #[error("claim `{claim}` names no population qualifier")]
    PopulationMissing { claim: String },

    #[error("claim `{claim}` names no assay or method qualifier")]
    AssayMissing { claim: String },

    #[error(
        "source `{source_id}` for claim `{claim}` became available at epoch {available} which is \
         after the visibility cutoff {cutoff}"
    )]
    PostCutoffSourceIncluded {
        claim: String,
        source_id: String,
        available: u64,
        cutoff: u64,
    },

    #[error("claim `{claim}` names no source locator")]
    LocatorMissing { claim: String },
}

/// Failures of OncoWorld temporal context (39.15).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TemporalError {
    #[error("timeline event `{event}` belongs to subject `{found}`, but this firewall is scoped to `{expected}`")]
    SubjectMismatch {
        event: String,
        expected: String,
        found: String,
    },

    #[error("{field} identity `{value}` is empty or contains a control character")]
    InvalidIdentity { field: &'static str, value: String },

    #[error(
        "event `{event}` occurred at epoch {occurred} which is after the decision epoch {decision}; \
         admitting it would let follow-up decide the decision it followed"
    )]
    FutureLeak {
        event: String,
        occurred: u64,
        decision: u64,
    },

    #[error(
        "imaging event `{event}` carries no treatment or steroid exposure context; an imaging \
         interpretation detached from what the patient was on is not interpretable"
    )]
    ImagingWithoutClinicalContext { event: String },

    #[error("reclassification of `{subject}` at epoch {epoch} would overwrite the historical wording recorded at epoch {original}")]
    HistoricalDiagnosisOverwritten {
        subject: String,
        epoch: u64,
        original: u64,
    },

    #[error(
        "timeline for `{subject}` records two events at epoch {epoch} with the same id `{event}`"
    )]
    DuplicateEvent {
        subject: String,
        epoch: u64,
        event: String,
    },

    #[error(
        "the retrospective plane was read while compiling a capsule for decision epoch {0}; later \
         evidence attaches to the oracle plane only"
    )]
    RetrospectivePlaneRead(u64),
}
