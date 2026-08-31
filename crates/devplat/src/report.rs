//! One evidence state, four audiences, and no number that changes between them.
//!
//! Implements blueprint 11.23, Reporting and Export Formats. Its detailed design is unusual for
//! what remains of the section: three of its six clauses are predicates rather than descriptions.
//!
//! - *"Audience renderers change explanation depth and visualization but not values, uncertainty,
//!   lineage, or status."* — a statement about a function, and therefore testable for all
//!   audiences at once.
//! - *"Every number has a machine-readable source pointer."* — a construction precondition.
//! - *"Reports state pack/evaluator/resource mismatch ... **before** showing headline
//!   differences."* — an ordering constraint on the rendered document.
//!
//! Those three are what this module is. The rest of 11.23 — nine report types, seven formats, a
//! signed bundle, offline HTML — is a list of artifacts, and the ones this workspace already has
//! belong to `bioprism-bundle` and `bioprism-ledger`.
//!
//! # The projection
//!
//! [`render`] is a *projection*, not a transformation. A [`Rendering`] holds the same [`Figure`]
//! values the [`EvidenceState`] holds, and an [`Audience`] chooses only how much explanation
//! travels with them. The invariant is stated as `render(state, a).figures == state.figures` for
//! every `a`, which is a strange-looking claim until you notice that the natural implementation —
//! an executive renderer that rounds, or drops an interval to fit a slide — violates it, and that
//! this is precisely how two reports of one run come to disagree.
//!
//! [`Depth`] therefore governs *prose*: the explanation attached to a figure, the lineage
//! narrative, the methodological caveats. It never governs the figure.
//!
//! # What is not here
//!
//! No renderer for any concrete format. No HTML, no Markdown, no Parquet, no PDF; nothing writes a
//! file. A [`Rendering`] is an ordered list of sections and the figures each one carries, which is
//! the level at which the ordering constraint and the value-preservation constraint can both be
//! checked. A formatter that consumed one would be a seventh format, and 11.23's own point is that
//! the formats are downstream of the evidence state.
//!
//! No signature and no bundle: `bioprism-ids` supplies the digest, and what to do with it is the
//! bundle crate's question.

use bioprism_ids::{CanonicalError, ContentHash};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeSet;

use crate::error::ReportError;

/// A pointer from a number back to the thing that produced it.
///
/// Both halves are required. A locator without a digest names a moving target, and a digest
/// without a locator is unopenable; 11.23 asks for "a machine-readable source pointer", and either
/// half alone fails to be one.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct SourcePointer {
    /// Where the value came from: a run id, an artifact path, a trial record.
    pub locator: String,
    /// The content hash of that source.
    pub digest: String,
}

impl SourcePointer {
    pub fn new(locator: impl Into<String>, digest: impl Into<String>) -> Self {
        SourcePointer {
            locator: locator.into(),
            digest: digest.into(),
        }
    }

    fn is_complete(&self) -> bool {
        !self.locator.trim().is_empty() && !self.digest.trim().is_empty()
    }
}

/// What is known about the spread of a value.
///
/// [`Uncertainty::NotReported`] carries a reason because 11.23's comparability clause is about
/// stating limitations, and "no interval" is a limitation. The scientific-reproduction case in the
/// reference-example section makes the same point from the other side: it lists a missing
/// uncertainty interval as an unmet evidence obligation rather than as a blank cell.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "uncertainty", rename_all = "snake_case")]
pub enum Uncertainty {
    /// A two-sided interval at a stated confidence level, in whole percent.
    Interval { low: f64, high: f64, level: u8 },
    /// No interval, and why.
    NotReported { because: String },
}

/// What kind of statement a figure is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum FigureStatus {
    /// Computed from evidence in the run.
    Measured,
    /// Derived under an assumption, which the figure states.
    Estimated { assuming: String },
    /// Deliberately not shown to this audience, and why.
    ///
    /// A withheld figure keeps its slot in every rendering. Dropping it would make the executive
    /// view and the machine view disagree about how many figures exist, which is the failure the
    /// value-preservation invariant is meant to exclude.
    Withheld { because: String },
}

impl FigureStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            FigureStatus::Measured => "measured",
            FigureStatus::Estimated { .. } => "estimated",
            FigureStatus::Withheld { .. } => "withheld",
        }
    }
}

/// One number, with everything that must travel with it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Figure {
    name: String,
    value: f64,
    uncertainty: Uncertainty,
    /// The chain that produced the value: pack version, evaluator version, run id.
    lineage: Vec<String>,
    status: FigureStatus,
    source: SourcePointer,
    /// Prose. The only part of a figure an audience may lose.
    explanation: String,
}

impl Figure {
    /// The only constructor. Refuses a figure without a complete source pointer.
    pub fn new(
        name: impl Into<String>,
        value: f64,
        uncertainty: Uncertainty,
        status: FigureStatus,
        source: SourcePointer,
    ) -> Result<Self, ReportError> {
        let name: String = name.into();
        if name.trim().is_empty() {
            return Err(ReportError::UnnamedFigure { name });
        }
        if !source.is_complete() {
            return Err(ReportError::FigureWithoutSource { name });
        }
        if let FigureStatus::Withheld { because } = &status {
            if because.trim().is_empty() {
                return Err(ReportError::WithheldWithoutReason { name });
            }
        }
        Ok(Figure {
            name,
            value,
            uncertainty,
            lineage: Vec::new(),
            status,
            source,
            explanation: String::new(),
        })
    }

    pub fn with_lineage(mut self, step: impl Into<String>) -> Self {
        self.lineage.push(step.into());
        self
    }

    pub fn explained_by(mut self, explanation: impl Into<String>) -> Self {
        self.explanation = explanation.into();
        self
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn value(&self) -> f64 {
        self.value
    }

    pub fn uncertainty(&self) -> &Uncertainty {
        &self.uncertainty
    }

    pub fn lineage(&self) -> &[String] {
        &self.lineage
    }

    pub fn status(&self) -> &FigureStatus {
        &self.status
    }

    pub fn source(&self) -> &SourcePointer {
        &self.source
    }

    pub fn explanation(&self) -> &str {
        &self.explanation
    }

    /// The part of a figure that no audience may change: value, uncertainty, lineage, status,
    /// source. Compared as canonical JSON so the check does not depend on field order.
    pub fn invariant_core(&self) -> Value {
        json!({
            "name": self.name,
            "value": self.value,
            "uncertainty": self.uncertainty,
            "lineage": self.lineage,
            "status": self.status,
            "source": self.source,
        })
    }
}

/// A stated reason why two things in this report may not be compared.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Limitation {
    /// The mismatch: different pack version, missing trials, a mutable model alias.
    pub mismatch: String,
    /// What it does to comparability. Required: a banner line with no consequence is decoration.
    pub effect: String,
}

impl Limitation {
    pub fn new(
        mismatch: impl Into<String>,
        effect: impl Into<String>,
    ) -> Result<Self, ReportError> {
        let mismatch: String = mismatch.into();
        let effect: String = effect.into();
        if effect.trim().is_empty() {
            return Err(ReportError::LimitationWithoutEffect {
                limitation: mismatch,
            });
        }
        Ok(Limitation { mismatch, effect })
    }
}

/// The evidence a report is made of. One state; the renderings are views onto it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EvidenceState {
    figures: Vec<Figure>,
    limitations: Vec<Limitation>,
    /// Figure names the report leads with. Every one must name a figure that exists.
    headline: Vec<String>,
}

impl EvidenceState {
    /// Assemble a state. Refuses duplicate figures and headlines that name nothing.
    pub fn new(
        figures: Vec<Figure>,
        limitations: Vec<Limitation>,
        headline: Vec<String>,
    ) -> Result<Self, ReportError> {
        let mut seen = BTreeSet::new();
        for figure in &figures {
            if !seen.insert(figure.name.clone()) {
                return Err(ReportError::DuplicateFigure {
                    name: figure.name.clone(),
                });
            }
        }
        for name in &headline {
            if !seen.contains(name) {
                return Err(ReportError::HeadlineWithoutFigure {
                    headline: name.clone(),
                });
            }
        }
        Ok(EvidenceState {
            figures,
            limitations,
            headline,
        })
    }

    pub fn figures(&self) -> &[Figure] {
        &self.figures
    }

    pub fn figure(&self, name: &str) -> Option<&Figure> {
        self.figures.iter().find(|figure| figure.name == name)
    }

    pub fn limitations(&self) -> &[Limitation] {
        &self.limitations
    }

    pub fn headline(&self) -> &[String] {
        &self.headline
    }

    /// A content hash over the evidence, not over any rendering of it.
    ///
    /// Deterministic and clock-free: the same state always hashes the same, and no audience
    /// appears in the input, so all four renderings of a state cite one digest.
    pub fn digest(&self) -> Result<ContentHash, ReportError> {
        let value = serde_json::to_value(self).map_err(|error| ReportError::NotCanonical {
            reason: error.to_string(),
        })?;
        ContentHash::of_value(&value).map_err(|error: CanonicalError| ReportError::NotCanonical {
            reason: error.to_string(),
        })
    }
}

/// Who the rendering is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Audience {
    /// Wants the method and the caveats.
    Reviewer,
    /// Wants the numbers and how to reproduce them.
    Engineer,
    /// Wants the headline and the limitations that bear on it.
    Executive,
    /// Wants everything, unformatted.
    Machine,
}

impl Audience {
    pub const ALL: [Audience; 4] = [
        Audience::Reviewer,
        Audience::Engineer,
        Audience::Executive,
        Audience::Machine,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            Audience::Reviewer => "reviewer",
            Audience::Engineer => "engineer",
            Audience::Executive => "executive",
            Audience::Machine => "machine",
        }
    }

    /// How much prose travels with the figures.
    pub fn depth(self) -> Depth {
        match self {
            Audience::Reviewer => Depth::Full,
            Audience::Engineer => Depth::Full,
            Audience::Executive => Depth::HeadlineOnly,
            Audience::Machine => Depth::None,
        }
    }
}

/// How much explanation a rendering carries. The only axis an audience moves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Depth {
    /// Every figure keeps its explanation.
    Full,
    /// Only headline figures keep theirs.
    HeadlineOnly,
    /// No prose at all. A machine consumer reads the fields.
    None,
}

/// A named part of a rendered report, in order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Section {
    /// The comparability banner. Present exactly when the state has limitations.
    Banner,
    /// The headline differences.
    Headline,
    /// Everything else.
    Detail,
}

/// One figure as it appears to an audience: the core, plus whatever prose survived.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RenderedFigure {
    pub figure: Figure,
    /// The explanation this audience receives. Empty when depth dropped it.
    pub explanation: String,
}

/// A report as seen by one audience.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rendering {
    pub audience: Audience,
    pub depth: Depth,
    /// In document order.
    pub sections: Vec<Section>,
    pub banner: Vec<Limitation>,
    pub figures: Vec<RenderedFigure>,
    pub headline: Vec<String>,
    /// The digest of the evidence state, identical across audiences.
    pub evidence_digest: String,
}

impl Rendering {
    /// Whether the comparability banner comes before the headline differences.
    ///
    /// True vacuously when there are no limitations, which is the only case in which no banner is
    /// rendered. 11.23 requires the banner "before showing headline differences", and the ordering
    /// is the requirement — a banner in a footnote satisfies a spellchecker and nothing else.
    pub fn banner_precedes_headline(&self) -> bool {
        let banner = self.sections.iter().position(|s| *s == Section::Banner);
        let headline = self.sections.iter().position(|s| *s == Section::Headline);
        match (banner, headline) {
            (Some(b), Some(h)) => b < h,
            (None, _) => self.banner.is_empty(),
            (Some(_), None) => true,
        }
    }

    pub fn figure(&self, name: &str) -> Option<&RenderedFigure> {
        self.figures
            .iter()
            .find(|rendered| rendered.figure.name() == name)
    }
}

/// Project an evidence state for one audience.
///
/// The only thing that varies with `audience` is [`Depth`], and the only thing [`Depth`] touches
/// is [`RenderedFigure::explanation`]. Every figure appears in every rendering, in the same order,
/// with the same value, uncertainty, lineage, status and source pointer.
pub fn render(state: &EvidenceState, audience: Audience) -> Result<Rendering, ReportError> {
    let depth = audience.depth();
    let headline: BTreeSet<&str> = state.headline.iter().map(String::as_str).collect();
    let figures = state
        .figures
        .iter()
        .map(|figure| {
            let explanation = match depth {
                Depth::Full => figure.explanation.clone(),
                Depth::HeadlineOnly if headline.contains(figure.name.as_str()) => {
                    figure.explanation.clone()
                }
                _ => String::new(),
            };
            RenderedFigure {
                figure: figure.clone(),
                explanation,
            }
        })
        .collect();
    let mut sections = Vec::new();
    if !state.limitations.is_empty() {
        sections.push(Section::Banner);
    }
    sections.push(Section::Headline);
    sections.push(Section::Detail);
    Ok(Rendering {
        audience,
        depth,
        sections,
        banner: state.limitations.clone(),
        figures,
        headline: state.headline.clone(),
        evidence_digest: state.digest()?.as_str().to_string(),
    })
}

/// Render for every audience at once, which is how the value-preservation rule is usually checked.
pub fn render_all(state: &EvidenceState) -> Result<Vec<Rendering>, ReportError> {
    Audience::ALL
        .into_iter()
        .map(|audience| render(state, audience))
        .collect()
}

/// Whether two renderings of the same state disagree about any figure.
///
/// Returns the names that differ. Empty is the expected answer, and a non-empty answer is a
/// factual drift between two views of one run — the failure 11.23 calls out by name.
pub fn drifted_figures(left: &Rendering, right: &Rendering) -> Vec<String> {
    let mut drifted = Vec::new();
    for rendered in &left.figures {
        let name = rendered.figure.name();
        match right.figure(name) {
            Some(other) if other.figure.invariant_core() == rendered.figure.invariant_core() => {}
            _ => drifted.push(name.to_string()),
        }
    }
    for rendered in &right.figures {
        let name = rendered.figure.name();
        if left.figure(name).is_none() {
            drifted.push(name.to_string());
        }
    }
    drifted.sort();
    drifted.dedup();
    drifted
}
