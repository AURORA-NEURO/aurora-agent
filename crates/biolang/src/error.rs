//! Typed failures for the IR family and for BioQL.
//!
//! Two rules shape this file.
//!
//! **A refusal names the dimension it refused on.** Blueprint 25.21's release gate is that "all
//! references, clocks, versions, and access labels are explicit"; an error that says "type error"
//! satisfies nothing. [`TypeError::Incomparable`] therefore carries the whole
//! [`Incomparability`] from `bioprism-standards` plus the dimension name and the
//! [`ScopeClass`] it belongs to, so a caller can route the failure without re-deriving it.
//!
//! **A missing declaration is its own kind of error.** `bioprism-standards` separates
//! [`Incomparability::is_silence`] from stated disagreement for the reason that silence reads as
//! agreement. The same split appears here as the family of `…NotDeclared` variants: a query with no
//! `labels` clause is not a permissive query, it is an under-specified one, and it is refused
//! rather than defaulted.

use crate::clock::Clock;
use crate::span::Span;
use bioprism_scope::ScopeClass;
use bioprism_standards::Incomparability;
use thiserror::Error;

/// Failures in canonical encoding, shared by every IR.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum IrError {
    /// The value could not be turned into canonical bytes.
    ///
    /// In practice this is a non-finite float reaching a digest. Refusing to hash a `NaN` is the
    /// same decision `bioprism-standards` makes: a placeholder digest would compare equal to some
    /// other pipeline's placeholder and manufacture agreement out of two failures.
    #[error("could not canonically encode {subject}: {detail}")]
    Encoding { subject: String, detail: String },

    /// An identifier field was empty or malformed.
    #[error("{field} is not a well-formed {kind} identifier: {detail}")]
    MalformedId {
        field: String,
        kind: String,
        detail: String,
    },
}

/// Validation failures for the BioWorld IR (25.01).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WorldError {
    /// 25.01: "Every asset has provenance and a resolvable locator."
    #[error("asset {asset:?} declares no {missing}; 25.01 requires provenance and a locator on every asset")]
    AssetUnderclared { asset: String, missing: String },

    /// 25.01: "Hidden labels are not reachable through participant tools."
    #[error("{item:?} is declared hidden and also exposed in the initial visible state")]
    HiddenItemVisible { item: String },

    /// A prohibited item that is also offered by the action catalog.
    #[error("action {action:?} produces {item:?}, which the world declares prohibited")]
    ProhibitedItemReachable { action: String, item: String },

    /// An oracle referenced by the world that the mesh does not contain.
    #[error("world cites oracle {oracle:?}, which is not in its declared mesh")]
    OracleNotInMesh { oracle: String },

    /// A version string this crate cannot read as `major.minor.patch`.
    #[error("{value:?} is not a semantic version; 25.01 requires world_id and semantic version")]
    MalformedVersion { value: String },
}

/// Validation failures for the BioState IR (25.02).
///
/// Not `Eq`: the resource variants carry `f64` amounts, and a bitwise-equality resource ledger would
/// be a worse lie than the missing trait.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum StateError {
    /// 25.02 validation, "clock-order checks": a record cannot precede the thing it records.
    #[error("state {state:?} was recorded at {record} but its event time is {event}; a record cannot precede its event")]
    RecordBeforeEvent {
        state: String,
        event: String,
        record: String,
    },

    /// 25.02: "A fork copies logical state but cannot duplicate affine resources."
    #[error("fork {child:?} consumed {consumed} of {resource:?} but its parent had already consumed {parent_consumed}; a fork cannot un-spend")]
    ForkUnspendsResource {
        child: String,
        resource: String,
        parent_consumed: f64,
        consumed: f64,
    },

    /// A child state that does not name its parent.
    #[error("state {child:?} claims to be a fork of {parent:?} but records a different parent")]
    ForkParentMismatch { child: String, parent: String },

    /// 25.02: "Biological and epistemic changes are never conflated."
    #[error("transition {label:?} changed the {plane} plane without declaring it")]
    UndeclaredPlaneChange { label: String, plane: String },

    /// The mirror failure: a declared change that did not happen.
    #[error("transition {label:?} declares a change to the {plane} plane whose hash did not change")]
    DeclaredPlaneUnchanged { label: String, plane: String },

    /// A resource amount that cannot survive canonical encoding.
    ///
    /// Not a pedantic check. `serde_json` turns a non-finite float into JSON `null` before
    /// `bioprism_ids`'s non-finite guard can refuse it, so a `NaN` amount would be hashed as an
    /// absent field and two runs that differed only there would agree on their digest. Refusing the
    /// state is the only place this crate can close that hole; see [`crate::canonical`].
    #[error("state {state:?} records a non-finite amount of {resource:?}; it cannot be canonically encoded")]
    NonFiniteAmount { state: String, resource: String },
}

/// Validation failures for the BioWorldline IR (25.09).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WorldlineError {
    /// States out of order on the worldline's own clock.
    #[error("state {state:?} has {clock} {at} but follows a state at {previous}")]
    OutOfOrder {
        state: String,
        clock: Clock,
        at: String,
        previous: String,
    },

    /// The invariant this IR exists for: one worldline, one scope.
    ///
    /// A worldline that mixes scopes is a chart assembled from two different patients, or two
    /// coordinate frames, or two genome builds, and every longitudinal quantity computed from it is
    /// a difference between incomparable things.
    #[error("state {state:?} is not within the worldline scope: dimension {dimension:?} is unbound or wider")]
    ScopeInterleaving { state: String, dimension: String },

    /// 25.09: "No event becomes visible before its reveal time."
    #[error("state {state:?} is gated by {gate:?} but appears at {at}, before the reveal at {reveal_at}")]
    PrematureReveal {
        state: String,
        gate: String,
        at: String,
        reveal_at: String,
    },

    /// 25.09: "Temporal corrections preserve prior recorded beliefs."
    #[error("revision {revision:?} supersedes {superseded:?}, which is no longer present on the worldline")]
    RevisionErasesHistory {
        revision: String,
        superseded: String,
    },

    /// A branch whose parent is not on this worldline.
    #[error("branch {branch:?} forks from {parent:?}, which is not a state on this worldline")]
    BranchParentMissing { branch: String, parent: String },

    /// Two worldlines whose scopes do not overlap at all.
    #[error("worldlines {left:?} and {right:?} have disjoint scopes: {reason}")]
    ScopesDisjoint {
        left: String,
        right: String,
        reason: String,
    },

    /// Two worldlines that bind the same dimension to values of different *kinds* — an exact value
    /// against a time window, say. `bioprism-scope` calls this a modelling error rather than a fact
    /// about the data, and it is reported separately from "no overlap" for that reason.
    #[error("worldlines {left:?} and {right:?} bind scope dimension {dimension:?} to different kinds of value")]
    ScopesDisagree {
        left: String,
        right: String,
        dimension: String,
    },
}

/// Validation failures for the Intervention and Action IR (25.06).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum InterventionError {
    /// 25.06: "Simulation is never labeled as real intervention."
    #[error("action {action:?} is a modeled perturbation but declares an effect on the {plane} plane")]
    SimulationClaimsRealEffect { action: String, plane: String },

    /// The mirror: a real-world action with no effect outside the artifact and knowledge planes.
    #[error("action {action:?} is declared a real-world effect but touches no material or biological plane")]
    RealEffectWithoutRealPlane { action: String },

    /// 25.06: "Irreversible effects require explicit authority."
    #[error("action {action:?} is irreversible and requires no authority")]
    IrreversibleWithoutAuthority { action: String },

    /// The lesson `bioprism-choreography` recorded: compensation is not rollback.
    #[error("action {action:?} claims compensation restores prior state; compensation leaves residue, which must be listed")]
    CompensationClaimsNoResidue { action: String },

    /// 25.06: "Action retries are idempotent or explicitly non-idempotent."
    #[error("action {action:?} consumes material irreversibly and cannot also be idempotent")]
    IrreversibleConsumptionCannotBeIdempotent { action: String },

    /// A precondition naming a plane the action never reads.
    #[error("action {action:?} has a precondition on the {plane} plane, which is not among its input state types")]
    PreconditionOffInputPlane { action: String, plane: String },
}

/// Validation failures for the Falsifiable Biological Contract IR (25.07).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FbcError {
    /// 25.07: "Every success state closes required obligations."
    #[error("terminal state {state:?} is a success but leaves obligation {obligation:?} open")]
    SuccessWithOpenObligation { state: String, obligation: String },

    /// 25.07: "Unsupported scope expansion invalidates the claim."
    #[error("claim scope is wider than the contract envelope on dimension {dimension:?}")]
    UnsupportedScopeExpansion { dimension: String },

    /// 25.07 validation, "obligation reachability".
    #[error("obligation {obligation:?} is discharged by no allowed action")]
    UnreachableObligation { obligation: String },

    /// 25.07 validation, "falsifier executable check".
    #[error("falsifier {falsifier:?} names oracle {oracle:?}, which is not in the contract's mesh")]
    FalsifierWithoutOracle { falsifier: String, oracle: String },

    /// A contract with no falsifier at all.
    #[error("contract {contract:?} declares no falsifier; a claim that nothing could refute is not falsifiable")]
    NoFalsifier { contract: String },
}

/// Validation failures for the Model, Pipeline and Agent IR (25.14).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SystemError {
    /// 25.14: "Published results pin every component."
    #[error("component {component:?} is not pinned: {detail}")]
    UnpinnedComponent { component: String, detail: String },

    /// 25.14: "Private prompts may be hashed but behavior contracts remain observable."
    #[error("component {component:?} hides its prompt and declares no observable behaviour contract")]
    HiddenBehaviourContract { component: String },

    /// A graph edge to a component the manifest does not declare.
    #[error("component graph references {component:?}, which the manifest does not declare")]
    DanglingComponent { component: String },

    /// A determinism claim contradicted by a declared source of nondeterminism.
    #[error("component {component:?} claims determinism but declares nondeterministic input {input:?}")]
    DeterminismContradicted { component: String, input: String },
}

/// Validation failures for the BioWeave Role and Act IR (25.15).
///
/// Not `Eq`, for the same reason as [`StateError`]: material amounts are `f64`.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum ActError {
    /// 25.15: "A claim act identifies evidence and scope."
    #[error("act {act:?} is a claim but identifies no {missing}")]
    ClaimWithout { act: String, missing: String },

    /// 25.15: "A specimen act obeys material conservation."
    #[error("act {act:?} reserves {requested} of specimen {specimen:?} but only {available} remains")]
    MaterialOverdrawn {
        act: String,
        specimen: String,
        requested: f64,
        available: f64,
    },

    /// The role that performed the act is not permitted to perform it.
    #[error("role {role:?} is not authorised for act {act_kind:?}")]
    RoleNotAuthorised { role: String, act_kind: String },

    /// 25.15: "Acts update explicit ledgers rather than only transcript text."
    #[error("act {act:?} is a {act_kind} and must post a ledger entry, but declares none")]
    ActWithoutLedgerEntry { act: String, act_kind: String },
}

/// Validation failures for the BioContext Capsule IR (25.16).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CapsuleError {
    /// 25.16: "Capsules obey data-use and clearance labels."
    #[error("capsule for {recipient:?} carries item {item:?} requiring label {label:?}, which the recipient does not hold")]
    LabelNotHeld {
        recipient: String,
        item: String,
        label: String,
    },

    /// 25.16: "Omissions are explicit."
    #[error("capsule for {recipient:?} omits {item:?} without a stated reason")]
    OmissionWithoutReason { recipient: String, item: String },

    /// 25.16: "Derived summaries point to source evidence."
    #[error("summary {summary:?} cites no source evidence")]
    SummaryWithoutSource { summary: String },

    /// An evidence item filed under two contradictory stances.
    #[error("evidence {item:?} appears as both {left} and {right}")]
    ContradictoryStance {
        item: String,
        left: String,
        right: String,
    },
}

/// Validation failures for the BioCapability Molecule IR (25.17).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MoleculeError {
    /// 25.17: "Nested molecules do not broaden authority."
    #[error("nested molecule {nested:?} requires capability {capability:?}, which {parent:?} does not hold")]
    NestedAuthorityBroadened {
        parent: String,
        nested: String,
        capability: String,
    },

    /// 25.17: "Guarantees are backed by evaluation evidence."
    #[error("guarantee {guarantee:?} cites no capability evidence")]
    UnbackedGuarantee { guarantee: String },

    /// A choreography step bound to no role.
    #[error("choreography step {step:?} is bound to no role")]
    UnboundStep { step: String },

    /// 25.17: "Internal attribution is preserved."
    #[error("molecule {molecule:?} publishes a result with no internal attribution")]
    AttributionErased { molecule: String },
}

/// Validation failures for the BioOracle Mesh IR (25.18).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum OracleIrError {
    /// 25.18: "Model judges cannot silently override stronger oracles."
    #[error("oracle {judge:?} at tier {judge_tier} may not override {stronger:?} at tier {stronger_tier}")]
    WeakerTierOverrides {
        judge: String,
        judge_tier: String,
        stronger: String,
        stronger_tier: String,
    },

    /// 25.18: "Oracle disagreement is retained."
    #[error("mesh resolved a disagreement between {left:?} and {right:?} without retaining the losing position")]
    DisagreementDiscarded { left: String, right: String },

    /// An oracle that claims a plane it also disclaims.
    #[error("oracle {oracle:?} both establishes and disclaims the {plane} plane")]
    PlaneClaimedAndDisclaimed { oracle: String, plane: String },

    /// 25.18 requires an independence declaration; an oracle sharing everything is not one.
    #[error("oracle {oracle:?} is not independent of the system it evaluates")]
    CircularIndependence { oracle: String },
}

/// Validation failures for the BioMutation IR (25.19).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MutationIrError {
    /// 25.19: "Every descendant retains parent lineage."
    #[error("mutation {mutation:?} declares no parent; a descendant without lineage cannot be audited")]
    LineageBroken { mutation: String },

    /// 25.19: "Controlled semantic changes update the oracle."
    #[error("mutation {mutation:?} declares relation {relation:?} but changes no oracle")]
    SemanticChangeWithoutOracleUpdate { mutation: String, relation: String },

    /// The mirror: a semantics-preserving mutation that moved the oracle.
    #[error("mutation {mutation:?} claims to preserve semantics but changes oracle {oracle:?}")]
    PreservingMutationChangesOracle { mutation: String, oracle: String },

    /// 25.19 requires a seed. A generator without one is not replayable.
    #[error("mutation {mutation:?} was generated without a recorded seed")]
    UnseededGenerator { mutation: String },
}

/// Validation failures for the BioResult Bundle IR (25.20).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BundleIrError {
    /// 25.20: "A published score resolves to a complete bundle."
    #[error("score {score:?} resolves to no bundle entry")]
    ScoreWithoutBundle { score: String },

    /// 25.20: "Result amendments create new versions."
    #[error("amendment of run {run:?} reuses the original version {version:?}")]
    AmendmentReusesVersion { run: String, version: String },

    /// 25.20: "Attestations identify signer and evidence scope."
    #[error("attestation on run {run:?} declares no {missing}")]
    AttestationWithout { run: String, missing: String },

    /// An oracle verdict in the bundle for an oracle the run never invoked.
    #[error("bundle carries a verdict from {oracle:?}, which does not appear in the run's actions")]
    VerdictFromUninvokedOracle { oracle: String },
}

/// Lexical failures.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LexError {
    #[error("unexpected character {found:?} at {span}")]
    UnexpectedCharacter { found: char, span: Span },

    #[error("string literal starting at {span} is never closed")]
    UnterminatedString { span: Span },

    #[error("{text:?} at {span} is not a number: {detail}")]
    MalformedNumber {
        text: String,
        span: Span,
        detail: String,
    },

    /// A backslash escape the lexer does not define.
    #[error("unknown escape {escape:?} in the string at {span}")]
    UnknownEscape { escape: String, span: Span },
}

/// Parse failures.
///
/// Every variant names the token that broke the parse, because "syntax error near line 3" makes the
/// author re-derive what the parser already knew.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ParseError {
    #[error("expected {expected} but found {found:?} at {span}")]
    UnexpectedToken {
        expected: String,
        found: String,
        span: Span,
    },

    #[error("expected {expected} but the query ends at {span}")]
    UnexpectedEnd { expected: String, span: Span },

    #[error("clause {clause} appears twice; the second is at {span}")]
    DuplicateClause { clause: String, span: Span },

    #[error("clause {clause} at {span} comes after {after}; BioQL clauses are ordered")]
    ClauseOutOfOrder {
        clause: String,
        after: String,
        span: Span,
    },

    /// A unit suffix that `bioprism-standards` does not know.
    ///
    /// Deliberately a parse error rather than a type error: `bioprism-standards` ships a closed unit
    /// table on purpose, so an unknown symbol is a token this language cannot read, not a value this
    /// language cannot check.
    #[error("{symbol:?} at {span} is not a unit this platform knows: {detail}")]
    UnknownUnit {
        symbol: String,
        span: Span,
        detail: String,
    },

    #[error("{text:?} at {span} is not a valid timestamp: {detail}")]
    MalformedTimestamp {
        text: String,
        span: Span,
        detail: String,
    },
}

/// Type-checking failures.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TypeError {
    /// The rejection this language exists for.
    ///
    /// Carries the whole [`Incomparability`] rather than a message, so a caller can match on the
    /// blocking dimension. `dimension` is the human name of that dimension and `class` is the
    /// `bioprism-scope` class it belongs to; both are derived from `reason`, never asserted
    /// separately.
    #[error("{left} and {right} at {span} are not comparable; blocking dimension is {dimension}: {reason}")]
    Incomparable {
        left: String,
        right: String,
        dimension: String,
        class: ScopeClass,
        /// Boxed because it is the largest payload in the enum, and an error type wide enough to
        /// trip `clippy::result_large_err` makes every `Result` in the checker expensive to move.
        reason: Box<Incomparability>,
        span: Span,
    },

    #[error("no field named {path:?} at {span}; the schema declares {declared} fields")]
    UnknownField {
        path: String,
        span: Span,
        declared: usize,
    },

    #[error("no collection named {name:?} at {span}")]
    UnknownCollection { name: String, span: Span },

    #[error("expected a boolean at {span} but found {found}")]
    NotBoolean { found: String, span: Span },

    #[error("{operator} at {span} needs a measured operand but the left side is {left} and the right side is {right}")]
    NotMeasured {
        operator: String,
        left: String,
        right: String,
        span: Span,
    },

    /// The temporal-leakage rule of 25.09, enforced at type-check time.
    #[error("{span} orders {left} against {right}; ordering two different clocks is the temporal leak 25.09 forbids")]
    ClockMismatch {
        left: Clock,
        right: Clock,
        span: Span,
    },

    #[error("set at {span} mixes {first} and {other}")]
    HeterogeneousSet {
        first: String,
        other: String,
        span: Span,
    },

    /// 25.21: "access labels | Required". Absence is refused, not defaulted to public.
    #[error("the query declares no access labels; 25.21 requires them and an absent declaration is not a public one")]
    AccessLabelsNotDeclared,

    /// 25.21: "ontology expansion policy | Required", enforced only where it can change the answer.
    #[error("field {field:?} at {span} is bound to ontology {ontology:?} but the query declares no expansion policy")]
    OntologyExpansionNotDeclared {
        field: String,
        ontology: String,
        span: Span,
    },

    /// 25.21: "time semantics | Required".
    #[error("collection {collection:?} is longitudinal and the query does not say which clock it means")]
    TimeSemanticsNotDeclared { collection: String },

    /// 25.21: "cost estimate | Required".
    #[error("the query declares no cost bound; 25.21 requires a cost estimate")]
    CostBoundNotDeclared,

    #[error("the static cost estimate is {estimate} but the query's declared bound is {limit}")]
    CostBoundExceeded { estimate: u64, limit: u64 },

    /// 25.21: "aggregation provenance | Required" and "Aggregates retain source lineage".
    #[error("aggregate {function} at {span} declares no source lineage; 25.21 requires aggregation provenance")]
    AggregationWithoutProvenance { function: String, span: Span },

    #[error("aggregate {function} at {span} is applied to {found}, which is not a measured value")]
    AggregateOverNonMeasured {
        function: String,
        found: String,
        span: Span,
    },

    /// The query's scope must sit inside the collection's declared scope.
    #[error("query scope does not refine the collection's declared scope on dimension {dimension:?}")]
    ScopeNotRefining { dimension: String },

    /// A scope dimension the `bioprism-scope` registry cannot classify.
    #[error("scope dimension {dimension:?} at {span} is unclassified; protected closure is defined per class")]
    UnclassifiedScopeDimension { dimension: String, span: Span },

    #[error("aggregate function {name:?} at {span} is not defined")]
    UnknownFunction { name: String, span: Span },

    /// `mg/kg * kg`, and the rest of the compositions the closed unit table refuses.
    #[error("{operator} at {span} cannot compose {left} with {right}: {detail}")]
    UnitComposition {
        operator: String,
        left: String,
        right: String,
        detail: String,
        span: Span,
    },
}

impl TypeError {
    /// Builds the incomparability variant, deriving the dimension name and class from the reason.
    ///
    /// A constructor rather than a struct literal so the three fields cannot drift apart: there is
    /// no way to report `UnstatedFrame` while naming "unit identity" as the blocker.
    pub fn incomparable(
        left: impl Into<String>,
        right: impl Into<String>,
        reason: Incomparability,
        span: Span,
    ) -> TypeError {
        TypeError::Incomparable {
            left: left.into(),
            right: right.into(),
            dimension: blocking_dimension(&reason).to_string(),
            class: reason.blocking_class(),
            reason: Box::new(reason),
            span,
        }
    }
}

/// The human name of the dimension an [`Incomparability`] blocked on.
///
/// `bioprism-standards` publishes [`bioprism_standards::CHECK_ORDER`] as the order the checks run
/// in, but at six entries it is coarser than the variants — `UnstatedFrame`, `FrameMismatch`,
/// `OrientationMismatch` and `SpaceMismatch` all fall under its single "coordinate frame or
/// reference build" heading. This table splits them, because "orientation" and "genome build" are
/// different things to go and fix.
pub fn blocking_dimension(reason: &Incomparability) -> &'static str {
    match reason {
        Incomparability::KindMismatch { .. } => "observable kind",
        Incomparability::DimensionMismatch { .. } => "physical dimension",
        Incomparability::NotCommensurable { .. } => "unit commensurability",
        Incomparability::ConversionRequired { .. } => "unit identity",
        Incomparability::UnstatedFrame { .. } => "coordinate frame (undeclared)",
        Incomparability::FrameMismatch { .. } => "coordinate frame",
        Incomparability::OrientationMismatch { .. } => "orientation",
        Incomparability::SpaceMismatch { .. } => "coordinate space",
        Incomparability::UnstatedBuild { .. } => "reference build (undeclared)",
        Incomparability::BuildMismatch { .. } => "reference build",
        Incomparability::ConventionMismatch { .. } => "coordinate convention",
        Incomparability::ContigMismatch { .. } => "contig",
        Incomparability::UnboundTerm { .. } => "ontology binding (undeclared)",
        Incomparability::UnmappedTerm { .. } => "ontology mapping",
        Incomparability::AmbiguousTerm { .. } => "ontology mapping",
        Incomparability::NamespaceMismatch { .. } => "ontology namespace",
        Incomparability::OntologyVersionDrift { .. } => "ontology release",
        Incomparability::GranularityMismatch { .. } => "ontology granularity",
        Incomparability::TermMismatch { .. } => "ontology term",
    }
}

/// Everything BioQL can refuse, in one type.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum QueryError {
    #[error(transparent)]
    Lex(#[from] LexError),
    #[error(transparent)]
    Parse(#[from] ParseError),
    #[error(transparent)]
    Type(#[from] TypeError),
}

impl QueryError {
    /// The source span, when the failure has one.
    ///
    /// The clause-level type errors have none: a missing `labels` clause is not at a position, it is
    /// the absence of one, and inventing a span pointing at the end of the query would be a
    /// diagnostic that says something false.
    pub fn span(&self) -> Option<Span> {
        match self {
            QueryError::Lex(error) => Some(match error {
                LexError::UnexpectedCharacter { span, .. }
                | LexError::UnterminatedString { span }
                | LexError::MalformedNumber { span, .. }
                | LexError::UnknownEscape { span, .. } => *span,
            }),
            QueryError::Parse(error) => Some(match error {
                ParseError::UnexpectedToken { span, .. }
                | ParseError::UnexpectedEnd { span, .. }
                | ParseError::DuplicateClause { span, .. }
                | ParseError::ClauseOutOfOrder { span, .. }
                | ParseError::UnknownUnit { span, .. }
                | ParseError::MalformedTimestamp { span, .. } => *span,
            }),
            QueryError::Type(error) => match error {
                TypeError::Incomparable { span, .. }
                | TypeError::UnknownField { span, .. }
                | TypeError::UnknownCollection { span, .. }
                | TypeError::NotBoolean { span, .. }
                | TypeError::NotMeasured { span, .. }
                | TypeError::ClockMismatch { span, .. }
                | TypeError::HeterogeneousSet { span, .. }
                | TypeError::OntologyExpansionNotDeclared { span, .. }
                | TypeError::AggregationWithoutProvenance { span, .. }
                | TypeError::AggregateOverNonMeasured { span, .. }
                | TypeError::UnclassifiedScopeDimension { span, .. }
                | TypeError::UnknownFunction { span, .. }
                | TypeError::UnitComposition { span, .. } => Some(*span),
                TypeError::AccessLabelsNotDeclared
                | TypeError::TimeSemanticsNotDeclared { .. }
                | TypeError::CostBoundNotDeclared
                | TypeError::CostBoundExceeded { .. }
                | TypeError::ScopeNotRefining { .. } => None,
            },
        }
    }
}
