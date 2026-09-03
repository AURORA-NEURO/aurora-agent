//! Typed refusals.
//!
//! Every error in this crate is a refusal to let a category mistake pass silently. Section 24
//! is written as prose obligations ("the system must not reward an agent for retrieving hidden
//! labels", "no renderer may silently collapse the stack"); prose cannot refuse. These enums
//! are what that prose looks like when it is given the power to say no.
//!
//! The errors are split by concern rather than pooled into one enum, so a caller that only
//! cares about provenance never has to match on assay vocabulary.

use thiserror::Error;

/// A system that cannot describe itself cannot be evaluated (blueprint 24.01).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EvaluabilityError {
    #[error("evaluated system has no identity; an anonymous system produces unattributable scores")]
    MissingIdentity,
    #[error("evaluated system '{system}' declares no version; results could not be reproduced against a known artifact")]
    MissingVersion { system: String },
    #[error("evaluated system '{system}' declares no capabilities; there is nothing to hold it to")]
    NoDeclaredCapabilities { system: String },
    #[error("evaluated system '{system}' declares no admissible action, not even abstention")]
    NoAdmissibleAction { system: String },
    #[error("evaluated system '{system}' was asked to perform '{action}', which it never declared")]
    UndeclaredAction { system: String, action: String },
    #[error("category property '{property}' was never declared; blueprint 24.01 requires all six")]
    UndeclaredCategoryProperty { property: &'static str },
}

/// Conflation of the five units of analysis (blueprint 24.01, mirroring the 40.05 invariant).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ProvenanceError {
    #[error("cannot attach a generated instance: no parent world is bound")]
    InstanceWithoutParentWorld,
    #[error("cannot attach an execution trial: no generated instance is bound")]
    TrialWithoutInstance,
    #[error("cannot attach a scored result: no execution trial is bound")]
    ResultWithoutTrial,
    #[error("provenance chain is incomplete at the {level} level; a score without a chain is unattributable")]
    Incomplete { level: &'static str },
    #[error("identifier for the {unit} unit is empty")]
    EmptyIdentifier { unit: &'static str },
    #[error("identifier for the {unit} unit contains a control character")]
    ControlCharacter { unit: &'static str },
}

/// Violations of the latent/observation/decision stack (blueprint 24.04).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum StackError {
    #[error("lineage edge '{edge}' is not defined between {from} and {to}")]
    IllegalEdge {
        edge: &'static str,
        from: &'static str,
        to: &'static str,
    },
    #[error("object '{object}' at {actual} was reported as {claimed}; the stack may not be collapsed")]
    LevelCollapse {
        object: String,
        actual: &'static str,
        claimed: &'static str,
    },
    #[error("cross-layer transition {from} -> {to} declares no uncertainty transformation")]
    UndeclaredUncertainty {
        from: &'static str,
        to: &'static str,
    },
    #[error("cross-layer transition {from} -> {to} declares no mapping character (deterministic, statistical, heuristic or expert-mediated)")]
    UndeclaredMappingCharacter {
        from: &'static str,
        to: &'static str,
    },
}

/// Inadmissible falsifiable biological contracts (blueprint 24.07).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ContractError {
    #[error("contract '{contract}' states no intent")]
    NoIntent { contract: String },
    #[error("contract '{contract}' lists no falsifier; a claim nothing could disconfirm is not a claim")]
    NoFalsifier { contract: String },
    #[error("contract '{contract}' offers no action, not even abstention")]
    NoAction { contract: String },
    #[error("contract '{contract}' declares no claim schema, so its output cannot be scored")]
    NoClaimSchema { contract: String },
    #[error("contract '{contract}' declares no reference standard, so 'wrong' is undefined")]
    NoReferenceStandard { contract: String },
    #[error("contract '{contract}' admits no terminal state")]
    NoTermination { contract: String },
    #[error("contract '{child}' cannot refine '{parent}': {reason}")]
    IllegalRefinement {
        child: String,
        parent: String,
        reason: RefinementViolation,
    },
}

/// Refusals from the cross-surface research operating-system contracts.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ResearchContractError {
    #[error("missing required field {field}")]
    MissingField { field: &'static str },
    #[error("schema version mismatch: expected {expected}, found {found}")]
    SchemaVersion { expected: String, found: String },
    #[error("{capability} does not carry the exact preclinical research boundary")]
    BoundaryMismatch { capability: String },
    #[error("{item} names no downstream or researcher consumer")]
    NoConsumer { item: String },
    #[error("{item} has an incomplete typed input/output contract")]
    MissingTypedContract { item: String },
    #[error("{item} at autonomy tier {tier:?} names no authority requirement")]
    MissingAuthority { item: String, tier: crate::research::AutonomyTier },
    #[error("{item} at a physical-execution tier does not declare instrument preflight")]
    MissingInstrumentPreflight { item: String },
    #[error("duplicate {kind} identifier {id}")]
    DuplicateId { kind: &'static str, id: String },
    #[error("workflow edge {from} -> {to} refers to an unknown node")]
    UnknownWorkflowNode { from: String, to: String },
    #[error("workflow node {node} has a self-edge")]
    WorkflowSelfEdge { node: String },
    #[error("workflow {workflow} contains a cycle through node {node}")]
    WorkflowCycle { workflow: String, node: String },
    #[error("invalid resource budget for {resource}")]
    InvalidBudget { resource: String },
    #[error("workflow {workflow} has approval-gated nodes but no approval record")]
    MissingWorkflowApproval { workflow: String },
    #[error("cannot serialize {item}: {message}")]
    Serialization { item: String, message: String },
    #[error("artifact {item} digest mismatch: expected {expected}, found {found}")]
    DigestMismatch { item: String, expected: String, found: String },
    #[error("autonomy grant for {actor} is incomplete")]
    IncompleteGrant { actor: String },
    #[error("policy receipt {item} has no reason")]
    MissingReason { item: String },
    #[error("policy receipt {receipt} cannot allow unresolved evidence")]
    UnresolvedAllow { receipt: String },
    #[error("policy receipt {receipt} carries authority before approval")]
    PrematureAuthority { receipt: String },
    #[error("execution run {run} is already closed")]
    RunClosed { run: String },
    #[error("execution run {run} expected event sequence {expected}, found {found}")]
    EventSequence { run: String, expected: u64, found: u64 },
    #[error("execution run {run} has a physical effect without payload evidence")]
    MissingEffectEvidence { run: String },
    #[error("execution runs may finish only with succeeded, failed, or cancelled")]
    InvalidFinalStatus,
    #[error("evidence receipt {receipt} contains no source")]
    NoEvidence { receipt: String },
    #[error("evidence receipt {receipt} contains no derivation")]
    MissingDerivation { receipt: String },
    #[error("evidence receipt {receipt} contains a source with no identifier")]
    MissingSourceId { receipt: String },
    #[error("evidence receipt {receipt} cannot claim proven with a potentially material omission")]
    ProtectedOmissionBlocksConclusion { receipt: String },
    #[error("evaluation card for {capability} is incomplete")]
    IncompleteEvaluation { capability: String },
    #[error("evaluation card for {item} has no uncertainty statement")]
    MissingUncertainty { item: String },
    #[error("federation envelope {envelope} is incomplete")]
    IncompleteFederation { envelope: String },
    #[error("federation envelope {envelope} claims local raw data but does not say so")]
    LocalizationMismatch { envelope: String },
    #[error("federation envelope {envelope} is unsigned")]
    UnsignedFederation { envelope: String },
}

/// The specific way a proposed refinement broke the contract-inheritance rule (24.07).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum RefinementViolation {
    #[error("it widens scope rather than narrowing it")]
    WidensScope,
    #[error("it drops an evidence obligation the parent required")]
    DropsEvidenceObligation,
    #[error("it admits an action the parent forbade")]
    AdmitsForbiddenAction,
    #[error("it drops a falsifier the parent required")]
    DropsFalsifier,
    #[error("it relaxes the parent's uncertainty requirement")]
    RelaxesUncertainty,
    #[error("it weakens the parent's declared use restriction")]
    WeakensUseRestriction,
}

/// Refused cross-scale evidence transport (blueprint 24.06).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TransportError {
    #[error("no declared translation edge from {from} to {to}; the spine is a graph, not a guaranteed chain")]
    NoEdge {
        from: &'static str,
        to: &'static str,
    },
    #[error("translation edge {from} -> {to} declares no evidence maturity")]
    UngradedEdge {
        from: &'static str,
        to: &'static str,
    },
    #[error("claim transported across {count} edge(s) without a translation certificate")]
    MissingCertificate { count: usize },
    #[error("certificate omits traversed edge {from} -> {to}")]
    CertificateOmitsEdge {
        from: &'static str,
        to: &'static str,
    },
    #[error("extrapolation '{mismatch}' is prohibited on edge {from} -> {to}")]
    ProhibitedExtrapolation {
        mismatch: &'static str,
        from: &'static str,
        to: &'static str,
    },
}

/// Claims a world class cannot support (blueprint 24.03).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WorldClassError {
    #[error("world class '{class}' supports only {available} counterfactual strength; the claim requires {required}")]
    InsufficientCounterfactualStrength {
        class: &'static str,
        available: &'static str,
        required: &'static str,
    },
    #[error("world class '{class}' declares no reveal policy for information withheld for scoring")]
    NoRevealPolicy { class: &'static str },
    #[error("transition on the {plane} plane was recorded as changing latent biological state")]
    PlaneConfusion { plane: &'static str },
}

/// Temporal leakage and clock confusion (blueprint 24.09).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TemporalError {
    #[error("evidence '{evidence}' was recorded at {recorded} but the decision is taken at {decided}; it was unavailable at decision time")]
    Leakage {
        evidence: String,
        recorded: String,
        decided: String,
    },
    #[error("evidence '{evidence}' carries no {clock} stamp, so leakage cannot be checked")]
    UnstampedClock { evidence: String, clock: &'static str },
    #[error("branch '{branch}' is {kind}, which does not license a treatment counterfactual")]
    UnjustifiedCounterfactualBranch {
        branch: String,
        kind: &'static str,
    },
}

/// Violations of the affine tissue ledger (blueprint 24.10).
#[derive(Debug, Clone, PartialEq, Error)]
pub enum LedgerError {
    #[error("resource '{resource}' has {available} remaining; the action requests {requested}")]
    Exhausted {
        resource: String,
        available: String,
        requested: String,
    },
    #[error("branch would duplicate the material allocation of '{resource}'; consumed specimens are affine, not copyable")]
    DuplicatedMaterial { resource: String },
    #[error("a hypothetical branch attempted to consume real material '{resource}'")]
    HypotheticalConsumption { resource: String },
    #[error("resource quantity must be finite and non-negative, got {value}")]
    InvalidQuantity { value: f64 },
}

/// Misuse of a reference-standard distribution (blueprint 24.11).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ReferenceError {
    #[error("reference standard '{standard}' is a distribution over {raters} raters and must not be collapsed to a single label")]
    Collapsed { standard: String, raters: usize },
    #[error("reference standard '{standard}' declares no {field}; blueprint 24.11 requires it")]
    MissingMetadata { standard: String, field: &'static str },
    #[error("reference standard '{standard}' requires at least {required} reviewers, found {found}")]
    TooFewReviewers {
        standard: String,
        required: usize,
        found: usize,
    },
    #[error("reference standard '{standard}' holds no ratings at all")]
    Empty { standard: String },
}

/// Applicability and evidence-maturity failures (blueprint 24.12).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ApplicabilityError {
    #[error("claim '{claim}' declares no {field}; the applicability envelope is incomplete")]
    IncompleteEnvelope { claim: String, field: &'static str },
    #[error("claim '{claim}' was transported into an unsupported {dimension}: declared '{declared}', requested '{requested}'")]
    OutsideEnvelope {
        claim: String,
        dimension: &'static str,
        declared: String,
        requested: String,
    },
    #[error("claim '{claim}' at maturity '{maturity}' was presented as established")]
    OverstatedMaturity {
        claim: String,
        maturity: &'static str,
    },
}

/// Causal overclaiming (blueprint 24.13).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CausalError {
    #[error("causal contract '{contract}' declares no {field}")]
    IncompleteContract { contract: String, field: &'static str },
    #[error("contract '{contract}' claims an interventional effect from '{basis}' evidence; a high predictive score does not establish a mechanism")]
    ProhibitedShortcut { contract: String, basis: &'static str },
    #[error("contract '{contract}' declares no estimator acceptance set, so any estimator would pass")]
    NoEstimatorAcceptanceSet { contract: String },
    #[error("contract '{contract}' declares no falsification or sensitivity check")]
    NoSensitivityCheck { contract: String },
}

/// Assay-lens declaration and comparability failures (blueprint 24.08).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum LensError {
    #[error("assay lens '{lens}' declares no {field}; blueprint 24.08 requires it before the lens may be used to score")]
    Incomplete { lens: String, field: &'static str },
    #[error("measurements from '{left}' and '{right}' are not comparable: {dimension} differs ('{left_value}' vs '{right_value}')")]
    Incomparable {
        left: String,
        right: String,
        dimension: &'static str,
        left_value: String,
        right_value: String,
    },
    #[error("lens '{lens}' has no calibration evidence and declares no expert assumption in its place")]
    UncalibratedAndUndeclared { lens: String },
    #[error("a synthetic noise model in lens '{lens}' was presented as empirical truth")]
    SyntheticPresentedAsEmpirical { lens: String },
    #[error("cannot compose '{upstream}' into '{downstream}': the upstream output '{produces}' is not the downstream input '{consumes}'")]
    UncomposableStages {
        upstream: String,
        downstream: String,
        produces: String,
        consumes: String,
    },
}

/// Capability-profile failures (blueprint 24.05).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CapabilityError {
    #[error("a capability profile covering {cells} distinct cells cannot be reduced to one 'overall biological intelligence' number")]
    ScalarCollapse { cells: usize },
    #[error("capability cell for activity '{activity}' at scale '{scale}' carries no evidence")]
    UnevidencedCell {
        activity: &'static str,
        scale: &'static str,
    },
    #[error("no measurement exists for activity '{activity}' at scale '{scale}'; absence is not a low score")]
    Untested {
        activity: &'static str,
        scale: &'static str,
    },
}

/// Attempts to carry BioPRISM output across the clinical boundary (blueprint 24.15).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum BoundaryError {
    #[error("'{use_case}' is a declared non-goal of BioPRISM: {non_goal}")]
    NonGoal {
        use_case: String,
        non_goal: &'static str,
    },
    #[error("artifact '{artifact}' carries research-use-only provenance and cannot be relabelled as '{requested}'")]
    UseEscalation {
        artifact: String,
        requested: &'static str,
    },
    #[error("result card for '{artifact}' omits {field}, which blueprint 24.15 requires before any result leaves the system")]
    IncompleteResultCard { artifact: String, field: &'static str },
    #[error("action '{action}' in a {world} world requires institutional authorization that the runtime cannot manufacture")]
    UnauthorizedHighImpactAction { action: String, world: &'static str },
}
