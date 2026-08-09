//! Typed failures for the residue register.
//!
//! Three enums, because they answer three different questions and a caller has to be able to tell
//! them apart without reading a message string.
//!
//! [`VerdictError`] means somebody tried to assert a classification without the evidence that
//! licenses it: no crate, no reasoning, no anchor, or a `discharged elsewhere` naming nobody. This
//! is the gate the whole crate is built around, so it is raised at construction *and* at
//! deserialisation — a verdict that lost its source in transit is a parse failure rather than a
//! silently weaker object.
//!
//! [`RegisterError`] means the register itself is malformed: a module out of the blueprint's range,
//! a module registered twice, a module with no verdict at all.
//!
//! [`ReconciliationError`] means the *workspace* could not be read. It is deliberately distinct
//! from "the backlog and the register disagree": an unreadable `docs/BACKLOG.md` and a backlog
//! listing a module this register does not explain are different states, and collapsing them would
//! let an unreadable checkout be reported as a register in perfect agreement with a file nobody
//! opened.

use bioprism_cookbook::CookbookError;
use thiserror::Error;

/// An attempt to record a classification that its evidence does not license.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum VerdictError {
    /// The reasoning is the verdict. A classification with a source but no argument records that
    /// somebody looked, not what they concluded, and the two are different facts.
    #[error(
        "a verdict's reasoning is {given} characters; at least {minimum} are required, because a \
         classification with no argument behind it records that somebody looked rather than what \
         they concluded"
    )]
    ReasoningTooThin { given: usize, minimum: usize },

    /// An anchor short enough to occur by accident proves nothing about the file it points at.
    #[error(
        "an anchor needle is {given} characters; at least {minimum} are required, because a \
         fragment short enough to occur by accident cannot witness that a judgement is still there"
    )]
    AnchorTooThin { given: usize, minimum: usize },

    /// A judgement recorded nowhere is a judgement nobody can check.
    #[error("a verdict's anchor names no file; the locus is where the judgement was recorded")]
    LocusMissing,

    /// Paths are compared against `Cargo.toml`'s member list, which is forward-slashed.
    #[error("locus `{path}` is not workspace-relative and forward-slashed")]
    LocusNotWorkspaceRelative { path: String },

    /// `Discharged elsewhere` is a claim with a referent. Without one it is a shrug.
    #[error(
        "`discharged elsewhere` names no crate; the whole content of the verdict is which crate \
         holds the substance under a different section's id"
    )]
    NoDischarger,

    /// The honest default must not be assertable by omission.
    #[error(
        "`nobody has read it` names no crate that was searched; a survey that found nothing is a \
         finding only if it says where it looked"
    )]
    EmptySurvey,

    /// A blocker with no statement is indistinguishable from nobody having tried.
    #[error(
        "`real work not yet done` states no blocker, so it cannot be told from an untouched module"
    )]
    NoBlocker,

    /// A block-level split is a claim that the division runs *inside* the module. Naming neither
    /// side of the division leaves nothing to check.
    #[error("a block-level split names neither the implemented blocks nor the residual ones")]
    NoBlocks,

    /// A register that transcribed its own text would be citing itself as the authority for its
    /// own conclusions, which is the shape of every unsourced claim.
    #[error(
        "`{name}` cannot transcribe a judgement from itself; a verdict this register reached by \
         reading another crate is inferred, not transcribed"
    )]
    TranscribedByItself { name: String },

    /// The one combination that cannot be true: if no crate recorded a judgement, no crate can
    /// have stated the verdict saying so.
    #[error(
        "a survey that found no recorded judgement cannot itself be transcribed from one; the \
         absence is this register's reading and its standing has to say so"
    )]
    AbsenceCannotBeTranscribed,

    /// Crate names are reused from `bioprism-cookbook` rather than minted again here, so the
    /// workspace has one spelling of a package name rather than two that drift.
    #[error(transparent)]
    CrateName(#[from] CookbookError),
}

/// A malformed register.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RegisterError {
    /// The blueprint has 44 sections and `tools/coverage.sh` matches 01 through 49. A section
    /// outside that range could never be a citation, so a module claiming one is a typo.
    #[error("section {section} is outside the range the coverage script recognises")]
    SectionOutOfRange { section: u8 },

    #[error("module index {index} is outside 01..=99")]
    IndexOutOfRange { index: u8 },

    /// The register is keyed by title precisely because ids may not be written; an untitled module
    /// therefore has no key at all.
    #[error("a module with no title cannot be registered, because the register is keyed by title")]
    TitleMissing,

    /// The point of the crate. A module in the register with no verdict is a module in the backlog.
    #[error(
        "module `{title}` carries no verdict; an entry with no explanation is the backlog line it \
         was supposed to replace"
    )]
    NoVerdict { title: String },

    #[error("module `{title}` is registered twice")]
    DuplicateTitle { title: String },

    /// Two entries with one id would make the reconciliation against `docs/BACKLOG.md` ambiguous.
    #[error("two registered modules share one blueprint id")]
    DuplicateKey,

    #[error(transparent)]
    Verdict(#[from] VerdictError),
}

/// This crate's own source could not be scanned, as distinct from it containing a forbidden token.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CitationError {
    #[error("`{path}` is not a directory, so there is nothing to scan")]
    NotADirectory { path: String },

    #[error("could not read `{path}`: {reason}")]
    Unreadable { path: String, reason: String },

    /// A scan that read nothing would report a clean crate, which is the same output a scan of a
    /// genuinely clean crate produces. The two must not be confusable, because this audit is the
    /// only thing standing between the register and a coverage figure of 100%.
    #[error("scanned zero files under `{path}`; an empty scan is not a clean one")]
    NothingScanned { path: String },
}

/// The workspace could not be read, as distinct from the workspace disagreeing with the register.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ReconciliationError {
    #[error("could not read `{path}`: {reason}")]
    Unreadable { path: String, reason: String },

    /// A backlog file with no module lines parsed is far more likely to be a changed format than a
    /// genuinely empty backlog, and reporting it as agreement would be the failure this crate
    /// exists to catch. `docs/BACKLOG.md` has already emptied itself once.
    #[error(
        "`{path}` yielded no module lines; an empty parse is a format change, not an empty backlog"
    )]
    BacklogParsedEmpty { path: String },
}
