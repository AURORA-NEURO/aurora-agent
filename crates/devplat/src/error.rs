//! Typed failures.
//!
//! Every constructor in this crate that refuses returns one of these. There is no `bool` refusal
//! and no `Option` standing in for "it did not hold": the reason a claim, a walkthrough, a figure
//! or a reproduction report was rejected is the useful half of the rejection, and a caller that
//! wants to report it needs it as data.

use thiserror::Error;

/// A surface could not be described.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SurfaceError {
    /// A surface names the artifact it lives in. An unnamed artifact cannot be looked for.
    #[error("surface of kind `{kind}` has an empty artifact name")]
    UnnamedArtifact { kind: &'static str },
    /// The in-repository surface is a Rust crate, and this workspace's crates are `bioprism-*`.
    #[error("`{artifact}` is not a crate of this workspace: an in-repository surface must name one")]
    NotAWorkspaceCrate { artifact: String },
}

/// A claim about a named API could not be sealed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ClaimError {
    #[error("a claim must name an API")]
    UnnamedApi,
    /// The evidence says "I found it in the tree" about something that is not in the tree.
    #[error(
        "`{api}` lives on the {kind} surface `{artifact}`, which is not in this repository, so it \
         cannot carry in-tree evidence"
    )]
    ForeignSurfaceClaimsInTreeEvidence {
        api: String,
        kind: &'static str,
        artifact: String,
    },
    /// The evidence says "I could not check it here" about something that is in the tree.
    #[error(
        "`{api}` lives in this repository (crate `{artifact}`), so `outside the tree` is not a \
         reason: resolve it or record it as absent"
    )]
    InTreeSurfaceClaimsForeignEvidence { api: String, artifact: String },
    /// "Cannot be checked here" without saying why is indistinguishable from "was not checked".
    #[error("`{api}` is recorded as unverifiable here without a reason")]
    UnverifiableWithoutReason { api: String },
    /// A resolution has to say what was read.
    #[error("`{api}` is recorded as resolved without naming the file it was resolved against")]
    ResolvedWithoutFile { api: String },
}

/// A walkthrough could not be sealed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum WalkthroughError {
    #[error("walkthrough id `{id}` is not of the form `section-slug` in kebab case")]
    MalformedId { id: String },
    #[error("walkthrough `{id}` has no goal")]
    NoGoal { id: String },
    #[error("walkthrough `{id}` has no steps")]
    NoSteps { id: String },
    #[error("step {ordinal} of walkthrough `{id}` has no instruction")]
    EmptyInstruction { id: String, ordinal: usize },
    /// A step that names no API is narration. That is allowed, and it has to be declared, because
    /// an undeclared narration step is a step whose claims nothing looked for.
    #[error(
        "step {ordinal} of walkthrough `{id}` is narration but does not say why it names no API"
    )]
    UndeclaredNarration { id: String, ordinal: usize },
    #[error("walkthrough `{id}` is entirely narration: it claims nothing and so verifies nothing")]
    EntirelyNarration { id: String },
}

/// A report could not be assembled or rendered.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ReportError {
    #[error("figure `{name}` carries no machine-readable source pointer")]
    FigureWithoutSource { name: String },
    #[error("figure `{name}` has an empty name")]
    UnnamedFigure { name: String },
    #[error("figure `{name}` appears twice in one evidence state")]
    DuplicateFigure { name: String },
    #[error("evidence state has a headline that names no figure: `{headline}`")]
    HeadlineWithoutFigure { headline: String },
    #[error("limitation `{limitation}` has no effect on comparability stated")]
    LimitationWithoutEffect { limitation: String },
    #[error("figure `{name}` is withheld without a reason")]
    WithheldWithoutReason { name: String },
    #[error("could not canonicalise the evidence state: {reason}")]
    NotCanonical { reason: String },
}

/// A reproduction report could not be sealed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ReproError {
    #[error("the obligation ledger does not mention `{kind}`; it must be total over the ten kinds")]
    IncompleteLedger { kind: &'static str },
    #[error(
        "status `{status}` is a verification result, and obligations {blocking:?} are still open"
    )]
    ConcludedUnderOpenObligation {
        status: &'static str,
        blocking: Vec<&'static str>,
    },
    #[error("a reproduction report must restate the claim it is about")]
    NoClaim,
    #[error("effect `{effect}` is both required and forbidden by molecule `{molecule}`")]
    EffectBothRequiredAndForbidden { molecule: String, effect: String },
    #[error("molecule `{molecule}` requires effect `{effect}`, which its policy does not allow")]
    EffectNotPermitted { molecule: String, effect: String },
    #[error("molecule `{molecule}` declares no effects at all, so its policy constrains nothing")]
    NoEffectsDeclared { molecule: String },
}

/// A security cell could not be scored, or its gate could not be evaluated.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ExploitError {
    #[error("security cell `{cell}` has no intended state transition, so nothing defines success")]
    NoIntendedTransition { cell: String },
    #[error("security cell `{cell}` has an empty identifier")]
    UnnamedCell { cell: String },
    #[error("security cell `{cell}` names no remediation for a finding that blocks release")]
    BlockingFindingWithoutRemediation { cell: String },
}

/// The self-citation audit could not run.
#[derive(Debug, Error)]
pub enum CitationError {
    #[error("could not read `{path}`: {reason}")]
    Unreadable { path: String, reason: String },
    #[error("`{path}` is not a directory")]
    NotADirectory { path: String },
}

/// Anything this crate can refuse, for callers that do not want to match six enums.
#[derive(Debug, Error)]
pub enum DevPlatError {
    #[error(transparent)]
    Surface(#[from] SurfaceError),
    #[error(transparent)]
    Claim(#[from] ClaimError),
    #[error(transparent)]
    Walkthrough(#[from] WalkthroughError),
    #[error(transparent)]
    Report(#[from] ReportError),
    #[error(transparent)]
    Repro(#[from] ReproError),
    #[error(transparent)]
    Exploit(#[from] ExploitError),
    #[error(transparent)]
    Citation(#[from] CitationError),
}
