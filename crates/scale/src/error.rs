//! Typed failures for every gate in section 35.
//!
//! Each variant names the thing that went wrong and both sides of the conflict, because the
//! failures here are release-blocking and a release-blocking error that says only "invalid" costs
//! an engineer an afternoon. 35's failure-containment rule — quarantine, never silently delete —
//! is why [`ReleaseError::ImmutableReleaseModified`] refuses rather than overwrites.

use thiserror::Error;

/// Failures shared across the factory's content pipeline.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum ScaleError {
    #[error("item {0:?} is not in the corpus")]
    UnknownItem(String),

    #[error("item {0:?} was added twice; item ids must be unique within a corpus")]
    DuplicateItem(String),

    #[error("lineage cycle reached {0:?} again; an item cannot be its own ancestor")]
    LineageCycle(String),

    #[error("item {item:?} declares parent {parent:?}, which is not in the corpus")]
    DanglingParent { item: String, parent: String },

    #[error("could not canonicalize world content: {0}")]
    Canonical(String),

    #[error("intra-cluster correlation must lie in [0, 1), got {0}")]
    CorrelationOutOfRange(f64),
}

/// Blueprint 35.11. A split that separates a lineage family, or a contaminated release.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SplitError {
    /// The failure this crate exists to make impossible to ship.
    #[error(
        "family {family:?} straddles a split: item {left_item:?} is {left:?} but item \
         {right_item:?} is {right:?}; both descend from the same parent world"
    )]
    FamilyStraddlesSplit {
        family: String,
        left_item: String,
        left: String,
        right_item: String,
        right: String,
    },

    #[error("family {family:?} was already assigned to tier {existing:?}; refusing to reassign to {requested:?}")]
    FamilyAlreadyAssigned {
        family: String,
        existing: String,
        requested: String,
    },

    #[error("item {item:?} in family {family:?} has no tier assignment; an unassigned item cannot be released")]
    UnassignedItem { item: String, family: String },

    #[error("contamination: item {item:?} (digest {digest:?}) appears in declared training corpus {corpus:?}")]
    TrainingExposure {
        item: String,
        digest: String,
        corpus: String,
    },

    #[error("canary {canary:?} was reproduced verbatim; the hidden tier is exposed")]
    CanaryDetected { canary: String },
}

/// Blueprint 35.05. Escrow refuses far more often than it reveals; that is the point.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EscrowError {
    #[error("no escrow with id {0:?}")]
    UnknownEscrow(String),

    #[error("escrow {0:?} was already sealed; a commitment cannot be re-sealed")]
    AlreadySealed(String),

    #[error(
        "escrow {escrow:?} may not be revealed: its condition is {condition}, and the vault is at \
         sequence {now}"
    )]
    ConditionNotMet {
        escrow: String,
        condition: String,
        now: u64,
    },

    #[error(
        "escrow {escrow:?} was sealed at sequence {sealed_at} but run {run:?} began at sequence \
         {run_at}; a commitment that does not precede the run proves nothing"
    )]
    CommitmentNotPriorToRun {
        escrow: String,
        sealed_at: u64,
        run: String,
        run_at: u64,
    },

    #[error(
        "escrow {escrow:?} froze system {frozen:?} but the reveal names {presented:?}; a system \
         changed after the freeze is not the system that was evaluated blind"
    )]
    SystemNotFrozen {
        escrow: String,
        frozen: String,
        presented: String,
    },

    #[error("escrow {escrow:?} was voided ({reason}) and its payload is permanently unrevealable")]
    Voided { escrow: String, reason: String },

    #[error("escrow {0:?} has already been revealed; a blind reveal happens once")]
    AlreadyRevealed(String),

    #[error("run {0:?} is not registered with the vault, so there is no ordering witness")]
    UnknownRun(String),

    #[error("commitment does not verify: recomputed {recomputed:?}, recorded {recorded:?}")]
    CommitmentMismatch {
        recomputed: String,
        recorded: String,
    },
}

/// Blueprint 35.12. The cache refuses partial keys and unproven hits.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CacheError {
    #[error("incomplete semantic key: {0} is empty, and 35.12 caches only under complete keys")]
    IncompleteKey(&'static str),

    #[error("object {0:?} is not in the store")]
    MissingObject(String),

    #[error("snapshot {0:?} is not in the store")]
    MissingSnapshot(String),

    #[error("snapshot chain revisits {0:?}; a delta cannot be based on its own descendant")]
    SnapshotCycle(String),

    #[error(
        "cache key {key:?} matched but component {component} differs: stored {stored:?}, \
         presented {presented:?}"
    )]
    KeyCollision {
        key: String,
        component: &'static str,
        stored: String,
        presented: String,
    },

    #[error("object {id:?} does not hash to its address: content hashes to {actual:?}")]
    CorruptObject { id: String, actual: String },

    #[error("indexed store: {0}")]
    Store(String),
}

/// Blueprint 35.14. Budget shortfalls surface rather than silently dropping safety strata.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum AdaptiveError {
    #[error(
        "budget of {budget} instances is below the mandatory safety floor of {floor} \
         (strata: {strata}); the scheduler may not trade safety coverage for information gain"
    )]
    BudgetBelowMandatoryFloor {
        budget: usize,
        floor: usize,
        strata: String,
    },

    #[error("target confidence half-width must be positive, got {0}")]
    NonPositiveTarget(f64),
}

/// Blueprint 35.17.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ReleaseError {
    #[error(
        "release {version} is immutable: it was published with content {published:?} and cannot \
         be republished with {attempted:?}; issue a correction instead"
    )]
    ImmutableReleaseModified {
        version: String,
        published: String,
        attempted: String,
    },

    #[error("release {version} is not published, so it cannot be {action}")]
    UnknownRelease { version: String, action: &'static str },

    #[error("release {new} does not supersede {old}: {new} is not a later version")]
    SupersessionGoesBackwards { new: String, old: String },

    #[error("release {0} is already withdrawn")]
    AlreadyWithdrawn(String),
}

/// Blueprint 35.18.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AuditError {
    #[error(
        "auditor {auditor:?} produced the artefact under audit; 35.18 requires an independent \
         reproduction, and self-audit is not one"
    )]
    SelfAudit { auditor: String },

    #[error("quality gate {gate} was never evaluated; an unevaluated gate is not a passed gate")]
    GateUnevaluated { gate: String },

    #[error("release blocked by {failed} failed quality gate(s): {names}")]
    ReleaseBlocked { failed: usize, names: String },
}
