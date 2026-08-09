//! A classification that cannot be asserted without the evidence licensing it.
//!
//! # The one idea
//!
//! `bioprism-atlas` made a capability with no evidence `Unmeasured` rather than zero, and
//! `bioprism-metrics` made a bare aggregate unserializable so a score cannot be published without
//! its coverage. This module is the same discipline applied to an *explanation*: a [`Verdict`] has
//! private fields and one constructor, and that constructor requires a [`Source`] — the crate whose
//! judgement it is, the file the judgement was recorded in, a fragment of that file which must
//! still be present, and the argument in prose.
//!
//! An entry in a backlog says a module is uncovered. An entry here says *why*, on somebody's
//! authority, with a pointer a reader can follow. Without the second half it is the first half with
//! more words, so the type refuses to be built.
//!
//! Deserialisation goes through the same gate ([`SourceFields`], [`VerdictFields`]), because the
//! wire form is the form a register travels in and a verdict that lost its source in transit is a
//! parse failure rather than a weaker object.
//!
//! # Two standings, never merged
//!
//! [`Standing::Transcribed`] means the classifying crate wrote this verdict down about this module.
//! [`Standing::InferredHere`] means this register read that crate's text and drew the conclusion —
//! usually because two sections carry the same content under different ids, so the crate that
//! discharged it never named the id this register is explaining. The distinction is
//! `AGENTS.md`'s "zero influence is not unknown influence" in a different currency: *somebody
//! decided this* and *we think this follows* are different states and must not share a
//! representation, because an inference presented as a transcription is a judgement attributed to
//! a crate that never made it.
//!
//! # Why the anchor is a fragment and not a whole sentence
//!
//! Reused wholesale from `bioprism_cookbook::PinnedQuote`, including the reasoning: a fragment
//! survives reflowing a paragraph and dies when the claim changes, which is the discrimination
//! wanted. A whole-line needle breaks on every line-wrap edit and gets deleted within a month.

use bioprism_cookbook::{CrateName, PinnedQuote};
use serde::{Deserialize, Serialize};

use crate::error::VerdictError;

/// Shortest reasoning a verdict may carry.
///
/// Set where it is because every rejected value below it was a label rather than an argument:
/// "process", "not code", "elsewhere". The bound does not make an argument good; it makes an
/// absent one visible.
pub const MINIMUM_REASONING: usize = 40;

/// Shortest anchor fragment a source may carry.
///
/// A needle short enough to occur by accident in an unrelated edit witnesses nothing.
pub const MINIMUM_ANCHOR: usize = 16;

/// Whether the classifying crate stated this verdict, or this register inferred it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Standing {
    /// The crate named in the source classified *this module* in these terms. The anchor is a
    /// fragment of the sentence that did it.
    Transcribed,
    /// The crate named in the source said something this register reads as deciding the module,
    /// without naming the module. The anchor is a fragment of what it did say.
    ///
    /// Every use of this standing is a place where a reader may reasonably disagree with the
    /// register rather than with the crate, and the report separates the two counts so the
    /// disagreement has somewhere to land.
    InferredHere,
}

impl Standing {
    pub fn as_str(&self) -> &'static str {
        match self {
            Standing::Transcribed => "transcribed",
            Standing::InferredHere => "inferred here",
        }
    }
}

/// Where a verdict came from: whose judgement, recorded where, witnessed by what, argued how.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "SourceFields", into = "SourceFields")]
pub struct Source {
    recorded_by: CrateName,
    anchor: PinnedQuote,
    reasoning: String,
    standing: Standing,
}

/// The wire form of a [`Source`], and its only construction path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceFields {
    pub recorded_by: String,
    pub anchor: PinnedQuote,
    pub reasoning: String,
    pub standing: Standing,
}

impl Source {
    /// A verdict the named crate wrote down about this module.
    pub fn transcribed(
        recorded_by: &str,
        locus: &str,
        needle: &str,
        reasoning: &str,
    ) -> Result<Self, VerdictError> {
        Source::build(recorded_by, locus, needle, reasoning, Standing::Transcribed)
    }

    /// A verdict this register drew from the named crate's text.
    pub fn inferred(
        recorded_by: &str,
        locus: &str,
        needle: &str,
        reasoning: &str,
    ) -> Result<Self, VerdictError> {
        Source::build(
            recorded_by,
            locus,
            needle,
            reasoning,
            Standing::InferredHere,
        )
    }

    fn build(
        recorded_by: &str,
        locus: &str,
        needle: &str,
        reasoning: &str,
        standing: Standing,
    ) -> Result<Self, VerdictError> {
        let recorded_by = CrateName::parse(recorded_by)?;
        if locus.trim().is_empty() {
            return Err(VerdictError::LocusMissing);
        }
        if locus.contains('\\') || locus.starts_with('/') {
            return Err(VerdictError::LocusNotWorkspaceRelative {
                path: locus.to_string(),
            });
        }
        if needle.chars().count() < MINIMUM_ANCHOR {
            return Err(VerdictError::AnchorTooThin {
                given: needle.chars().count(),
                minimum: MINIMUM_ANCHOR,
            });
        }
        if reasoning.chars().count() < MINIMUM_REASONING {
            return Err(VerdictError::ReasoningTooThin {
                given: reasoning.chars().count(),
                minimum: MINIMUM_REASONING,
            });
        }
        Ok(Source {
            recorded_by,
            anchor: PinnedQuote::new(locus, needle),
            reasoning: reasoning.to_string(),
            standing,
        })
    }

    pub fn recorded_by(&self) -> &CrateName {
        &self.recorded_by
    }

    /// The workspace-relative file the judgement was recorded in.
    pub fn locus(&self) -> &str {
        &self.anchor.source
    }

    /// The fragment of that file which must still be present for this verdict to stand.
    pub fn anchor(&self) -> &PinnedQuote {
        &self.anchor
    }

    pub fn reasoning(&self) -> &str {
        &self.reasoning
    }

    pub fn standing(&self) -> Standing {
        self.standing
    }
}

impl TryFrom<SourceFields> for Source {
    type Error = VerdictError;

    fn try_from(fields: SourceFields) -> Result<Self, Self::Error> {
        Source::build(
            &fields.recorded_by,
            &fields.anchor.source,
            &fields.anchor.needle,
            &fields.reasoning,
            fields.standing,
        )
    }
}

impl From<Source> for SourceFields {
    fn from(source: Source) -> Self {
        SourceFields {
            recorded_by: source.recorded_by.to_string(),
            anchor: source.anchor,
            reasoning: source.reasoning,
            standing: source.standing,
        }
    }
}

/// The crates that hold the substance of a module discharged under a different id.
///
/// A newtype rather than a bare `Vec` so that the empty case is unrepresentable: "discharged
/// elsewhere" with no referent is a shrug, and the whole content of the verdict is the referent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Vec<String>", into = "Vec<String>")]
pub struct Dischargers(Vec<CrateName>);

impl Dischargers {
    pub fn new<I, S>(crates: I) -> Result<Self, VerdictError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let names = crates
            .into_iter()
            .map(|name| CrateName::parse(name.as_ref()))
            .collect::<Result<Vec<_>, _>>()?;
        if names.is_empty() {
            return Err(VerdictError::NoDischarger);
        }
        Ok(Dischargers(names))
    }

    pub fn crates(&self) -> &[CrateName] {
        &self.0
    }
}

impl TryFrom<Vec<String>> for Dischargers {
    type Error = VerdictError;

    fn try_from(value: Vec<String>) -> Result<Self, Self::Error> {
        Dischargers::new(value)
    }
}

impl From<Dischargers> for Vec<String> {
    fn from(value: Dischargers) -> Self {
        value.0.iter().map(CrateName::to_string).collect()
    }
}

/// The crates that were read while looking for a judgement, and did not carry one.
///
/// The same shape as [`Dischargers`] and for the sharper reason: "genuinely uncovered" is the
/// honest default of this whole register, which makes it the one verdict an author can reach for
/// without doing any work. Requiring the survey makes the work visible. An empty survey is not a
/// wide search that found nothing; it is no search.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "Vec<String>", into = "Vec<String>")]
pub struct Survey(Vec<CrateName>);

impl Survey {
    pub fn new<I, S>(crates: I) -> Result<Self, VerdictError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let names = crates
            .into_iter()
            .map(|name| CrateName::parse(name.as_ref()))
            .collect::<Result<Vec<_>, _>>()?;
        if names.is_empty() {
            return Err(VerdictError::EmptySurvey);
        }
        Ok(Survey(names))
    }

    pub fn crates(&self) -> &[CrateName] {
        &self.0
    }
}

impl TryFrom<Vec<String>> for Survey {
    type Error = VerdictError;

    fn try_from(value: Vec<String>) -> Result<Self, Self::Error> {
        Survey::new(value)
    }
}

impl From<Survey> for Vec<String> {
    fn from(value: Survey) -> Self {
        value.0.iter().map(CrateName::to_string).collect()
    }
}

/// Where a code-bearing artifact that is not a Rust crate in this repository actually lives.
///
/// `bioprism_devplat::surface` holds a finer census over its own twenty modules — locale, package
/// names, importable paths. This axis is coarser and spans four sections, and this crate does not
/// depend on `bioprism-devplat` to get at the finer one: a register that could not be built while
/// a classifying crate was mid-edit would be unavailable exactly when somebody is changing a
/// classification, which is the argument `bioprism-cookbook` makes for reading the workspace as
/// text rather than linking it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ForeignSurface {
    /// A Python distribution: importable packages, decorators, a CLI.
    PythonPackage,
    /// A TypeScript client, usually generated from a schema this workspace does not emit.
    TypeScriptPackage,
    /// A network surface — REST, gRPC, an event stream, webhooks — for a server that does not
    /// exist here, because the service graph requires the same services to run in-process.
    NetworkApi,
    /// A composite action evaluated by a CI runner in somebody else's repository.
    GitHubAction,
    /// A workflow file. A workflow runner is not a library.
    CiWorkflow,
    /// Another organisation's evolving specification, whose disagreements would enter the evidence
    /// record as though they were properties of the run.
    ForeignSpecification,
}

impl ForeignSurface {
    pub fn as_str(&self) -> &'static str {
        match self {
            ForeignSurface::PythonPackage => "python package",
            ForeignSurface::TypeScriptPackage => "typescript package",
            ForeignSurface::NetworkApi => "network api",
            ForeignSurface::GitHubAction => "github action",
            ForeignSurface::CiWorkflow => "ci workflow",
            ForeignSurface::ForeignSpecification => "foreign specification",
        }
    }
}

/// Why a module has no implementation, on somebody's authority.
///
/// The five verdicts are the ones the workspace's classifying crates already established, and they
/// are reused rather than re-cut. A sixth vocabulary for the same distinctions is how two crates
/// end up disagreeing about a module for reasons that are entirely about wording.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum Classification {
    /// The detailed design describes what people do, not predicates over an artifact. Councils,
    /// cadence, appeals, onboarding, a dashboard somebody reads.
    ///
    /// `crates/stewardship`'s test, which every later crate reused: *is the detailed design a set
    /// of predicates over an artifact, or a description of what people do?*
    Process,
    /// Code-bearing, precise and testable, and not Rust and not in this repository.
    ///
    /// `crates/ops` had to add this bucket for §40 and `crates/devplat` found seven of twenty in
    /// its own scope. Calling these process would be as wrong as implementing them, and the
    /// difference matters to a contributor deciding what to work on.
    ForeignArtifact { surface: ForeignSurface },
    /// The content is implemented under a different section's id, by a crate that never names this
    /// one.
    ///
    /// `crates/worldfactory` found §27 modules discharged by five siblings, none of which cites a
    /// §27 id. That is a finding rather than a gap, and it is invisible to a coverage script that
    /// matches tokens.
    DischargedElsewhere { by: Dischargers },
    /// The prose/code division runs *inside* the module rather than between modules, so the module
    /// is simultaneously implemented and not.
    ///
    /// `crates/bioevalx`'s finding for §26 and §07: every module ends with the same four blocks
    /// describing what a study author must do, above a purpose, a target and a numbered protocol
    /// that are checkable. A per-module verdict cannot express that; this one says which side is
    /// which.
    BlockLevelSplit {
        /// The blocks somebody already implements, and who.
        implemented_by: Dischargers,
        /// What is left after those blocks are taken out.
        residue: String,
    },
    /// Nobody has read it, or it is real work not yet done.
    ///
    /// The honest default. Every other verdict here is a reason a module will *never* move; this
    /// is the one that says it still can, and quietly emptying it is the failure mode this whole
    /// register exists to make visible.
    GenuinelyUncovered { standing: UncoveredStanding },
}

/// The two ways a module is genuinely uncovered, which are not the same fact.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UncoveredStanding {
    /// No crate in the workspace records any position on it. The survey is what was read.
    NobodyHasRead { surveyed: Survey },
    /// Somebody stated why it cannot be or has not been done. A blocker is not a classification —
    /// the module may well be code-bearing — so this stays uncovered rather than becoming an
    /// excuse.
    RealWorkNotDone { blocked_by: String },
}

impl UncoveredStanding {
    /// A survey, or a refusal naming the empty one.
    pub fn nobody_has_read<I, S>(surveyed: I) -> Result<Self, VerdictError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Ok(UncoveredStanding::NobodyHasRead {
            surveyed: Survey::new(surveyed)?,
        })
    }

    /// A stated blocker, or a refusal.
    pub fn real_work_not_done(blocked_by: &str) -> Result<Self, VerdictError> {
        if blocked_by.chars().count() < MINIMUM_REASONING {
            return Err(VerdictError::NoBlocker);
        }
        Ok(UncoveredStanding::RealWorkNotDone {
            blocked_by: blocked_by.to_string(),
        })
    }
}

impl Classification {
    /// A short label, stable across releases because reports are diffed.
    pub fn as_str(&self) -> &'static str {
        match self {
            Classification::Process => "process",
            Classification::ForeignArtifact { .. } => "foreign artifact",
            Classification::DischargedElsewhere { .. } => "discharged elsewhere",
            Classification::BlockLevelSplit { .. } => "block-level split",
            Classification::GenuinelyUncovered { .. } => "genuinely uncovered",
        }
    }

    /// Whether this verdict leaves work on the table.
    ///
    /// Four of the five say the module will never move; one says it still can. Reports that
    /// collapse the five into a percentage lose exactly this distinction, which is why the
    /// function is named for the question rather than for the variant.
    pub fn is_work_remaining(&self) -> bool {
        matches!(self, Classification::GenuinelyUncovered { .. })
    }

    /// `Discharged elsewhere`, built through its gate.
    pub fn discharged_by<I, S>(crates: I) -> Result<Self, VerdictError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        Ok(Classification::DischargedElsewhere {
            by: Dischargers::new(crates)?,
        })
    }

    /// A block-level split, built through its gate.
    pub fn block_level_split<I, S>(implemented_by: I, residue: &str) -> Result<Self, VerdictError>
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        if residue.chars().count() < MINIMUM_ANCHOR {
            return Err(VerdictError::NoBlocks);
        }
        Ok(Classification::BlockLevelSplit {
            implemented_by: Dischargers::new(implemented_by)?,
            residue: residue.to_string(),
        })
    }

    /// Every crate this verdict names as holding substance, in the order given.
    pub fn named_crates(&self) -> Vec<&CrateName> {
        match self {
            Classification::DischargedElsewhere { by } => by.crates().iter().collect(),
            Classification::BlockLevelSplit { implemented_by, .. } => {
                implemented_by.crates().iter().collect()
            }
            Classification::GenuinelyUncovered {
                standing: UncoveredStanding::NobodyHasRead { surveyed },
            } => surveyed.crates().iter().collect(),
            _ => Vec::new(),
        }
    }
}

/// A classification together with the evidence that licenses it.
///
/// Private fields and one constructor. There is no `Verdict { classification }` anywhere, and
/// there is no `Default`, because a default verdict is an unsourced one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "VerdictFields", into = "VerdictFields")]
pub struct Verdict {
    classification: Classification,
    source: Source,
}

/// The wire form of a [`Verdict`], and its only construction path.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct VerdictFields {
    pub classification: Classification,
    pub source: Source,
}

/// This crate's own package name, which a transcription may not be attributed to.
pub const THIS_REGISTER: &str = "bioprism-residue";

impl Verdict {
    /// The only way to make one.
    ///
    /// It takes a [`Source`] by value rather than by option, so there is no shape of this call
    /// that omits it. That is the whole design: `bioprism-atlas` gates a measurement the same way
    /// and `bioprism-metrics` gates a score, and the reason is identical — a number, or here an
    /// explanation, that reaches a reader without its basis is indistinguishable from one that has
    /// none.
    ///
    /// Two combinations are refused rather than represented. This register may not *transcribe*
    /// itself, because a source that points back at the file making the claim is the shape of every
    /// unsourced assertion. And a survey that found no recorded judgement may not be transcribed
    /// from one, because that is the single self-contradictory pair the vocabulary admits.
    pub fn record(classification: Classification, source: Source) -> Result<Self, VerdictError> {
        if source.standing() == Standing::Transcribed {
            if source.recorded_by().as_str() == THIS_REGISTER {
                return Err(VerdictError::TranscribedByItself {
                    name: THIS_REGISTER.to_string(),
                });
            }
            if matches!(
                classification,
                Classification::GenuinelyUncovered {
                    standing: UncoveredStanding::NobodyHasRead { .. }
                }
            ) {
                return Err(VerdictError::AbsenceCannotBeTranscribed);
            }
        }
        Ok(Verdict {
            classification,
            source,
        })
    }

    pub fn classification(&self) -> &Classification {
        &self.classification
    }

    pub fn source(&self) -> &Source {
        &self.source
    }

    /// The crate whose judgement this is.
    pub fn recorded_by(&self) -> &CrateName {
        self.source.recorded_by()
    }

    pub fn standing(&self) -> Standing {
        self.source.standing()
    }

    /// One line for a report.
    pub fn describe(&self) -> String {
        format!(
            "{} [{} · {}]",
            self.classification.as_str(),
            self.source.recorded_by(),
            self.source.standing().as_str()
        )
    }
}

impl TryFrom<VerdictFields> for Verdict {
    type Error = VerdictError;

    fn try_from(fields: VerdictFields) -> Result<Self, Self::Error> {
        Verdict::record(fields.classification, fields.source)
    }
}

impl From<Verdict> for VerdictFields {
    fn from(verdict: Verdict) -> Self {
        VerdictFields {
            classification: verdict.classification,
            source: verdict.source,
        }
    }
}
