//! Protocol adapters, their quality grades, and the loss report that must accompany a projection.
//!
//! Blueprint 23.24.
//!
//! # The one thing this module is for
//!
//! 23.24 opens by saying the point of adapters is to avoid "a proprietary island while refusing to
//! collapse richer semantics into transport primitives **without a loss report**". Everything here
//! follows from taking that clause literally: [`project`] returns a [`Projection`] that *contains*
//! its [`LossReport`], there is no constructor for a `Projection` without one, and the report
//! distinguishes a surface the adapter refused to carry from one it was never asked about.
//!
//! # Two ladders that §23 never relates
//!
//! 23.01 gives a six-rung adapter ladder by *transport mechanism* — `bioprism_fabric::AdapterRung`,
//! opaque text through native SDK. 23.24 gives a six-grade ladder by *semantic surface carried*,
//! G0 through G5. They range over the same adapters and the blueprint never states the relation, so
//! this module keeps 23.24's vocabulary ([`CarriedSurface`]) separate from 23.01's
//! (`bioprism_fabric::SemanticFeature`) and supplies an explicit partial bridge, [`surface_of`],
//! together with the two sets the bridge cannot cross: [`UNBRIDGED_SURFACES`] and
//! [`UNBRIDGED_FEATURES`].
//!
//! The bridge being partial is the interesting result rather than a defect of the bridge.
//! [`grade_of_stack_adapter`] shows it: an adapter declared purely in 23.01's vocabulary cannot be
//! graded above [`Grade::G1`], because G2 requires trace correlation and 23.01's feature list has no
//! member that can express it. A publication claiming G3 for an adapter whose only declaration is a
//! `bioprism_fabric::Adapter` is claiming something its declaration cannot support.
//!
//! # Not implemented
//!
//! No transport, and no protocol bindings of any kind. Nothing here speaks A2A, MCP, JSON-RPC,
//! OpenTelemetry OTLP or CloudEvents over the wire; [`CloudEventEnvelope`] is a value with the
//! eight fields 23.24's table names and no encoder. 23.24's *framework adapters* section, which
//! asks that graph and crew frameworks be mapped onto participants and role bindings, is not
//! implemented at all: it describes instrumenting a third-party scheduler this workspace does not
//! have. Its eight target concepts are recorded in [`FRAMEWORK_MAPPING_TARGETS`] and nothing
//! consumes them, which is the same treatment `bioprism_fabric::stack` gives 23.01's own tables.

use bioprism_fabric::flow::{Labelling, Sensitivity};
use bioprism_fabric::stack::{Adapter, SemanticFeature};
use bioprism_weave::{AuthorityError, AuthorityTable, Capability};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// The semantic surface 23.24's grade ladder ranges over.
///
/// Read off the grade table verbatim: each grade line names one or more surfaces, and this is their
/// union. It is deliberately *not* `bioprism_fabric::SemanticFeature`, which is 23.01's list for
/// 23.01's ladder; see [`surface_of`] for how far the two can be reconciled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CarriedSurface {
    /// G1: "structured messages and artifacts".
    StructuredMessages,
    Artifacts,
    /// G2: "task lifecycle and trace correlation".
    TaskLifecycle,
    TraceCorrelation,
    /// G3: "typed acts, schemas, and effects".
    TypedActs,
    Schemas,
    Effects,
    /// G4: "commitments, authority, and context capsules".
    Commitments,
    Authority,
    ContextCapsules,
    /// G5: "continuations, fork/join, and full replay hooks".
    Continuations,
    ForkJoin,
    ReplayHooks,
}

impl CarriedSurface {
    /// Every surface, in grade order.
    ///
    /// There is no `OpaqueTransport` member. G0 is "opaque transport only", which is the *absence*
    /// of every semantic surface rather than the presence of one more, so [`Grade::G0`] requires
    /// nothing and every adapter meets it. Modelling transport as a surface would make G0
    /// unreachable for an adapter that genuinely only moves bytes.
    pub const ALL: [CarriedSurface; 13] = [
        CarriedSurface::StructuredMessages,
        CarriedSurface::Artifacts,
        CarriedSurface::TaskLifecycle,
        CarriedSurface::TraceCorrelation,
        CarriedSurface::TypedActs,
        CarriedSurface::Schemas,
        CarriedSurface::Effects,
        CarriedSurface::Commitments,
        CarriedSurface::Authority,
        CarriedSurface::ContextCapsules,
        CarriedSurface::Continuations,
        CarriedSurface::ForkJoin,
        CarriedSurface::ReplayHooks,
    ];
}

/// 23.24's adapter quality grades, weakest first so `>=` reads as "at least this good".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Grade {
    /// "opaque transport only".
    G0,
    /// "structured messages and artifacts".
    G1,
    /// "task lifecycle and trace correlation".
    G2,
    /// "typed acts, schemas, and effects".
    G3,
    /// "commitments, authority, and context capsules".
    G4,
    /// "continuations, fork/join, and full replay hooks".
    G5,
}

impl Grade {
    pub const ALL: [Grade; 6] = [
        Grade::G0,
        Grade::G1,
        Grade::G2,
        Grade::G3,
        Grade::G4,
        Grade::G5,
    ];

    /// The surfaces this grade *adds* over the one below it.
    pub fn introduces(self) -> &'static [CarriedSurface] {
        match self {
            Grade::G0 => &[],
            Grade::G1 => &[
                CarriedSurface::StructuredMessages,
                CarriedSurface::Artifacts,
            ],
            Grade::G2 => &[CarriedSurface::TaskLifecycle, CarriedSurface::TraceCorrelation],
            Grade::G3 => &[
                CarriedSurface::TypedActs,
                CarriedSurface::Schemas,
                CarriedSurface::Effects,
            ],
            Grade::G4 => &[
                CarriedSurface::Commitments,
                CarriedSurface::Authority,
                CarriedSurface::ContextCapsules,
            ],
            Grade::G5 => &[
                CarriedSurface::Continuations,
                CarriedSurface::ForkJoin,
                CarriedSurface::ReplayHooks,
            ],
        }
    }

    /// Everything this grade requires, cumulatively.
    ///
    /// **Cumulativity is this crate's reading.** 23.24 prints six lines and never says a grade
    /// subsumes the ones below it. The alternative reading — six unrelated labels — makes the
    /// ordering in the table meaningless and makes "published results state the grade used"
    /// uninformative, so cumulative it is, said out loud rather than assumed.
    pub fn requires(self) -> BTreeSet<CarriedSurface> {
        Grade::ALL
            .iter()
            .filter(|g| **g <= self)
            .flat_map(|g| g.introduces().iter().copied())
            .collect()
    }
}

/// An adapter's declaration in 23.24's own vocabulary.
///
/// `carries` is what the adapter asserts it transports without loss. There is no "approximates"
/// field here on purpose: 23.01 already owns the approximated/unsupported distinction on
/// `bioprism_fabric::Adapter`, and an approximation is a loss, so for grading purposes it is
/// simply not carried.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterProfile {
    pub protocol: Protocol,
    pub name: String,
    pub carries: BTreeSet<CarriedSurface>,
}

/// The protocols 23.24 names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Protocol {
    A2A,
    Mcp,
    OpenTelemetry,
    CloudEvents,
    Cli,
    Framework,
}

impl AdapterProfile {
    pub fn new(protocol: Protocol, name: impl Into<String>) -> Self {
        AdapterProfile {
            protocol,
            name: name.into(),
            carries: BTreeSet::new(),
        }
    }

    pub fn carrying(mut self, surface: CarriedSurface) -> Self {
        self.carries.insert(surface);
        self
    }

    /// The highest grade every one of whose requirements this adapter carries.
    ///
    /// Grades do not skip: an adapter carrying G5's surfaces but not G2's trace correlation is
    /// G1, because [`Grade::requires`] is cumulative. That is the point of the ladder.
    pub fn grade(&self) -> Grade {
        Grade::ALL
            .iter()
            .copied()
            .rfind(|g| g.requires().is_subset(&self.carries))
            .unwrap_or(Grade::G0)
    }

    /// What the adapter would have to add to reach `target`.
    pub fn shortfall(&self, target: Grade) -> BTreeSet<CarriedSurface> {
        target
            .requires()
            .difference(&self.carries)
            .copied()
            .collect()
    }
}

/// The partial bridge from 23.01's feature vocabulary to 23.24's surface vocabulary.
///
/// **Not in the blueprint.** 23.01 and 23.24 give two lists for the same objects and never relate
/// them. Six pairs are unambiguous; the rest are not, and returning `None` is how this module
/// refuses to guess. See [`UNBRIDGED_FEATURES`] for the features with no surface and
/// [`UNBRIDGED_SURFACES`] for the surfaces no feature can express.
pub fn surface_of(feature: SemanticFeature) -> Option<CarriedSurface> {
    match feature {
        SemanticFeature::Messages => Some(CarriedSurface::StructuredMessages),
        SemanticFeature::Artifacts => Some(CarriedSurface::Artifacts),
        SemanticFeature::TaskLifecycle => Some(CarriedSurface::TaskLifecycle),
        SemanticFeature::Commitments => Some(CarriedSurface::Commitments),
        SemanticFeature::AuthorityDelegation => Some(CarriedSurface::Authority),
        SemanticFeature::ContinuationTransfer => Some(CarriedSurface::Continuations),
        SemanticFeature::Discovery
        | SemanticFeature::EpistemicStateDelta
        | SemanticFeature::SecurityLabels
        | SemanticFeature::MessageOrdering
        | SemanticFeature::ClaimVersusVerifiedFact => None,
    }
}

/// 23.01 features that no 23.24 grade mentions. A 23.01 declaration can say more than a grade can.
pub const UNBRIDGED_FEATURES: [SemanticFeature; 5] = [
    SemanticFeature::Discovery,
    SemanticFeature::EpistemicStateDelta,
    SemanticFeature::SecurityLabels,
    SemanticFeature::MessageOrdering,
    SemanticFeature::ClaimVersusVerifiedFact,
];

/// 23.24 surfaces that no 23.01 feature can express. A grade can say more than a declaration can.
pub const UNBRIDGED_SURFACES: [CarriedSurface; 7] = [
    CarriedSurface::TraceCorrelation,
    CarriedSurface::TypedActs,
    CarriedSurface::Schemas,
    CarriedSurface::Effects,
    CarriedSurface::ContextCapsules,
    CarriedSurface::ForkJoin,
    CarriedSurface::ReplayHooks,
];

/// A grade derived from a 23.01 adapter declaration, and the ceiling that derivation sits under.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GradeAssessment {
    pub grade: Grade,
    /// The best grade *any* 23.01 declaration could reach, however complete.
    pub ceiling: Grade,
    /// Surfaces above the ceiling that the declaration has no vocabulary for.
    pub unexpressible: BTreeSet<CarriedSurface>,
}

/// Grade a 23.01 adapter declaration, honestly.
///
/// The result is capped at [`Grade::G1`] no matter how much the adapter declares, because
/// [`Grade::G2`] needs [`CarriedSurface::TraceCorrelation`] and 23.01's `SemanticFeature` has no
/// member that maps to it. An adapter that really does correlate traces has to say so in 23.24's
/// vocabulary — via [`AdapterProfile`] — and that is a claim about the adapter rather than an
/// artefact of this function.
pub fn grade_of_stack_adapter(adapter: &Adapter) -> GradeAssessment {
    let carries: BTreeSet<CarriedSurface> = adapter
        .supported
        .iter()
        .filter_map(|f| surface_of(*f))
        .collect();
    let unexpressible: BTreeSet<CarriedSurface> = UNBRIDGED_SURFACES.into_iter().collect();
    let ceiling = Grade::ALL
        .iter()
        .copied()
        .rfind(|g| g.requires().iter().all(|s| !unexpressible.contains(s)))
        .unwrap_or(Grade::G0);
    let grade = Grade::ALL
        .iter()
        .copied()
        .rfind(|g| *g <= ceiling && g.requires().is_subset(&carries))
        .unwrap_or(Grade::G0);
    GradeAssessment {
        grade,
        ceiling,
        unexpressible,
    }
}

/// What an adapter was asked to carry and what it could not.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LossReport {
    pub adapter: String,
    pub grade: Grade,
    /// Requested and carried.
    pub preserved: BTreeSet<CarriedSurface>,
    /// Requested and refused. 23.24: these "travel through a namespaced extension or referenced
    /// artifact where both sides support them" — this module records the loss, it does not route.
    pub dropped: BTreeSet<CarriedSurface>,
}

impl LossReport {
    /// Whether the projection lost nothing that was asked for.
    ///
    /// Named for what it asserts. There is no `is_ok`, because "nothing was requested" and "nothing
    /// was lost" are the same value here and a reader should have to notice that.
    pub fn lossless_for_request(&self) -> bool {
        self.dropped.is_empty()
    }
}

/// A projection of a Weave surface through an adapter, inseparable from its loss report.
///
/// Both fields are public and the type has no constructor other than [`project`], so there is no
/// way to obtain the carried set without the report that says what is missing from it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Projection {
    pub carried: BTreeSet<CarriedSurface>,
    pub loss: LossReport,
}

/// Project a requested surface through an adapter.
///
/// This is 23.24's central rule as a function signature: there is no variant that returns the
/// carried set alone.
pub fn project(adapter: &AdapterProfile, requested: &BTreeSet<CarriedSurface>) -> Projection {
    let carried: BTreeSet<CarriedSurface> =
        requested.intersection(&adapter.carries).copied().collect();
    let dropped: BTreeSet<CarriedSurface> =
        requested.difference(&adapter.carries).copied().collect();
    Projection {
        loss: LossReport {
            adapter: adapter.name.clone(),
            grade: adapter.grade(),
            preserved: carried.clone(),
            dropped,
        },
        carried,
    }
}

/// 23.24's CloudEvents envelope, field for field.
///
/// The blueprint's table is eight rows and this is eight fields. `data` is deliberately
/// [`Payload`] rather than a JSON value, because 23.24's rule about it is a rule about whether the
/// content is present or referenced.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CloudEventEnvelope {
    pub id: String,
    pub source: String,
    pub subject: String,
    /// "namespaced Weave act/event type".
    pub event_type: String,
    /// 23.24 wants an event timestamp. This crate has no clock, so a caller supplies one or does
    /// not; `None` is a missing timestamp and never a substituted one.
    pub time: Option<String>,
    pub dataschema: String,
    pub datacontenttype: String,
    pub data: Payload,
}

/// Whether the payload travels inline or by reference.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "payload")]
pub enum Payload {
    /// The typed payload itself, in the envelope.
    Inline { body: String },
    /// A content-addressed reference. 23.24's escape for anything that must not be in the clear.
    Referenced { hash: String },
    /// Encrypted in place; the envelope carries ciphertext it cannot read.
    Encrypted { ciphertext_hash: String },
}

impl Payload {
    fn is_clear(&self) -> bool {
        matches!(self, Payload::Inline { .. })
    }
}

/// Why an envelope was refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, thiserror::Error)]
#[serde(rename_all = "snake_case", tag = "refusal")]
pub enum EnvelopeRefusal {
    /// 23.24: "Sensitive content remains encrypted or referenced, not placed in clear metadata."
    #[error("{sensitivity:?} content cannot travel inline in a CloudEvents envelope")]
    SensitiveInClearMetadata { sensitivity: Sensitivity },

    /// Inherited from `bioprism_fabric::flow`: an unlabelled value is not a public one, so it
    /// cannot be shown to be safe to place in the clear.
    #[error("payload labelling is unknown, so clear placement cannot be justified")]
    LabellingUnknown,

    /// 23.24 requires a namespaced act/event type; a bare word could collide with a core act.
    #[error("event type {0} is not namespaced")]
    UnnamespacedType(String),
}

/// Build a CloudEvents envelope, enforcing 23.24's clear-metadata rule.
///
/// `Public` and `Internal` content may travel inline. `Confidential` and `Restricted` may not, and
/// neither may content whose labelling was never recorded — the last case following
/// `bioprism_fabric::flow::Labelling::Unlabelled`, which refuses rather than defaulting to public.
pub fn build_envelope(
    envelope: CloudEventEnvelope,
    labelling: &Labelling,
) -> Result<CloudEventEnvelope, EnvelopeRefusal> {
    if !envelope.event_type.contains('.') && !envelope.event_type.contains(':') {
        return Err(EnvelopeRefusal::UnnamespacedType(envelope.event_type));
    }
    if envelope.data.is_clear() {
        match labelling {
            Labelling::Unlabelled => return Err(EnvelopeRefusal::LabellingUnknown),
            Labelling::Labelled(label) if label.sensitivity >= Sensitivity::Confidential => {
                return Err(EnvelopeRefusal::SensitiveInClearMetadata {
                    sensitivity: label.sensitivity,
                })
            }
            Labelling::Labelled(_) => {}
        }
    }
    Ok(envelope)
}

/// An MCP exposure: what a server makes discoverable.
///
/// 23.24: "MCP exposure grants discoverability, not permission." That rule is enforced by absence.
/// This type has no field that can hold a grant, no method that returns one, and no way to reach
/// `bioprism_weave::AuthorityTable`. [`authorize_mcp_call`] takes the exposure *and* a grant
/// identifier and consults the table regardless of what the exposure says.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct McpExposure {
    pub tool: String,
    pub required_capability: Capability,
}

impl McpExposure {
    pub fn new(tool: impl Into<String>, required_capability: Capability) -> Self {
        McpExposure {
            tool: tool.into(),
            required_capability,
        }
    }
}

/// Check a call against the authority table, whatever the exposure advertises.
///
/// The exposure contributes exactly one thing: which capability the call needs. Everything else is
/// the kernel's decision, which is what makes the rule true rather than merely stated.
pub fn authorize_mcp_call(
    exposure: &McpExposure,
    table: &AuthorityTable,
    grant_id: &str,
) -> Result<(), AuthorityError> {
    table.check(grant_id, exposure.required_capability.clone())
}

/// The seven things 23.24's CLI adapter "must capture".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CliCaptureItem {
    CommandAndVersion,
    Environment,
    StdoutStderrDistinction,
    ToolLikeEvents,
    FileDeltas,
    ExitStatus,
    /// "uncertainty in semantic extraction" — the item that makes the list honest rather than
    /// merely complete.
    ExtractionUncertainty,
}

impl CliCaptureItem {
    pub const ALL: [CliCaptureItem; 7] = [
        CliCaptureItem::CommandAndVersion,
        CliCaptureItem::Environment,
        CliCaptureItem::StdoutStderrDistinction,
        CliCaptureItem::ToolLikeEvents,
        CliCaptureItem::FileDeltas,
        CliCaptureItem::ExitStatus,
        CliCaptureItem::ExtractionUncertainty,
    ];
}

/// How confident the CLI adapter is that it understood what it read.
///
/// Not a probability and not an `Option<f64>`. 23.24 asks the adapter to capture *uncertainty in
/// semantic extraction*, and a scalar invites averaging it away; a reason string cannot be averaged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "extraction")]
pub enum Extraction {
    /// The adapter parsed a structured mode the tool documents.
    Structured,
    /// The adapter inferred structure from free text, and says how.
    Inferred { basis: String },
    /// The adapter could not tell. Distinct from `Inferred`, which is a guess with a stated basis.
    Undetermined { reason: String },
}

impl Extraction {
    /// Whether a downstream typed act may be minted from this extraction without a human in the
    /// loop. Inference is allowed; not knowing is not.
    pub fn admits_typed_act(&self) -> bool {
        !matches!(self, Extraction::Undetermined { .. })
    }
}

/// A CLI adapter's capture record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CliCapture {
    pub captured: BTreeSet<CliCaptureItem>,
    pub extraction: Extraction,
}

impl CliCapture {
    pub fn new(extraction: Extraction) -> Self {
        CliCapture {
            captured: BTreeSet::new(),
            extraction,
        }
    }

    pub fn capturing(mut self, item: CliCaptureItem) -> Self {
        self.captured.insert(item);
        self
    }

    /// Which of the seven required items are absent.
    pub fn missing(&self) -> BTreeSet<CliCaptureItem> {
        CliCaptureItem::ALL
            .into_iter()
            .filter(|item| !self.captured.contains(item))
            .collect()
    }
}

/// 23.24's twelve conformance-matrix dimensions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConformanceDimension {
    Discovery,
    SchemaNegotiation,
    Streaming,
    Cancellation,
    Retries,
    Idempotency,
    Authentication,
    EffectEnforcement,
    SecurityLabels,
    TraceCorrelation,
    ArtifactIntegrity,
    SemanticLoss,
}

impl ConformanceDimension {
    pub const ALL: [ConformanceDimension; 12] = [
        ConformanceDimension::Discovery,
        ConformanceDimension::SchemaNegotiation,
        ConformanceDimension::Streaming,
        ConformanceDimension::Cancellation,
        ConformanceDimension::Retries,
        ConformanceDimension::Idempotency,
        ConformanceDimension::Authentication,
        ConformanceDimension::EffectEnforcement,
        ConformanceDimension::SecurityLabels,
        ConformanceDimension::TraceCorrelation,
        ConformanceDimension::ArtifactIntegrity,
        ConformanceDimension::SemanticLoss,
    ];
}

/// The outcome of one matrix cell.
///
/// There is no default. A dimension absent from the matrix reads as [`CellOutcome::NotTested`] via
/// [`ConformanceMatrix::outcome`], which is not a pass and is not a failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CellOutcome {
    Passed,
    Failed,
    NotTested,
}

/// 23.24: "Every adapter is tested across: ..." — the matrix that claim is made from.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConformanceMatrix {
    cells: BTreeMap<ConformanceDimension, CellOutcome>,
}

impl ConformanceMatrix {
    pub fn new() -> Self {
        ConformanceMatrix::default()
    }

    pub fn recording(mut self, dimension: ConformanceDimension, outcome: CellOutcome) -> Self {
        self.cells.insert(dimension, outcome);
        self
    }

    pub fn outcome(&self, dimension: ConformanceDimension) -> CellOutcome {
        self.cells
            .get(&dimension)
            .copied()
            .unwrap_or(CellOutcome::NotTested)
    }

    pub fn untested(&self) -> BTreeSet<ConformanceDimension> {
        ConformanceDimension::ALL
            .into_iter()
            .filter(|d| self.outcome(*d) == CellOutcome::NotTested)
            .collect()
    }

    pub fn failed(&self) -> BTreeSet<ConformanceDimension> {
        ConformanceDimension::ALL
            .into_iter()
            .filter(|d| self.outcome(*d) == CellOutcome::Failed)
            .collect()
    }
}

/// 23.24: "Published results state the grade used."
///
/// The grade is a required constructor argument, so a result without one does not exist. The
/// matrix travels with it, because a grade claimed with eleven of twelve dimensions untested is a
/// different claim from the same grade fully exercised, and the reader is entitled to tell them
/// apart.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublishedResult {
    pub adapter: String,
    pub grade: Grade,
    pub matrix: ConformanceMatrix,
}

impl PublishedResult {
    pub fn new(adapter: &AdapterProfile, matrix: ConformanceMatrix) -> Self {
        PublishedResult {
            adapter: adapter.name.clone(),
            grade: adapter.grade(),
            matrix,
        }
    }

    /// Whether every dimension was exercised and none failed.
    ///
    /// Untested and failed are reported separately by [`ConformanceMatrix::untested`] and
    /// [`ConformanceMatrix::failed`]; this predicate collapses them and exists only so a caller
    /// can ask the easy question explicitly.
    pub fn fully_exercised(&self) -> bool {
        self.matrix.untested().is_empty() && self.matrix.failed().is_empty()
    }
}

/// 23.24's framework-adapter target concepts, recorded and unconsumed.
///
/// Nothing in this workspace instruments a third-party graph or crew framework, so mapping onto
/// these is not implemented; the list is here so a reader can see it was read.
pub const FRAMEWORK_MAPPING_TARGETS: [&str; 8] = [
    "participants",
    "role bindings",
    "messages",
    "state nodes",
    "tool calls",
    "handoffs",
    "checkpoints",
    "evaluators",
];

/// The OpenTelemetry span-and-event sources 23.24 lists, recorded and unconsumed.
///
/// 23.24 also observes that "the canonical Weave event is richer than an OTel span". This crate
/// emits nothing, so the observation is recorded rather than tested.
pub const OTEL_SPAN_SOURCES: [&str; 11] = [
    "thread and molecule invocation",
    "role binding",
    "model generation",
    "context projection",
    "communicative acts",
    "tool calls",
    "commitment transitions",
    "forks and joins",
    "verifier results",
    "topology changes",
    "budget events",
];
