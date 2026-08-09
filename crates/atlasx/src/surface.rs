//! What a browsing surface may be asked, and what it must say when it cannot answer.
//!
//! Shared by the two surfaces this crate builds: the coverage-debt statement of blueprint 34.08
//! and the failure browse of 34.09. Both of those modules carry the same paragraph, and it is the
//! only paragraph in either that is a predicate rather than a description:
//!
//! > The surface must explicitly support unavailable, controlled, stale, under-review, disputed,
//! > withdrawn, non-reproducible, and not-comparable states. It must not replace them with zero,
//! > empty, or hidden values.
//!
//! # The cell that has no blank
//!
//! [`SurfaceCell`] is the only way a number leaves a surface in this crate, and it has no `Empty`,
//! no `Default`, no `unwrap_or(0)` and no arm that carries an absent number. Its five arms are a
//! count, a share that carries its own denominator, a score, a hole, and a withheld state. The two
//! non-numeric arms borrow their vocabularies rather than minting new ones:
//!
//! - a hole carries `bioprism_atlas::UnmeasuredReason`, so "nobody measured" reads identically here
//!   and in the atlas;
//! - a withheld cell carries `bioprism_hub::PublicationState`, which already enumerates all eight
//!   states the paragraph above demands.
//!
//! Minting a ninth vocabulary would have been the failure the paragraph is warning about, one level
//! up: two enums for one distinction is how a surface ends up rendering `Withdrawn` as blank
//! because the renderer only learned about the other enum.
//!
//! # Browsing is not evidence
//!
//! `bioprism-lens` makes accessibility a type bound: a lens must produce a `Witness`, and `Witness`
//! has no method returning anything drawable, so a lens that can only draw does not compile. The
//! same idea is reused here against a different failure. A browsing aggregation is built to be
//! looked at, and the thing it is looked at *for* is usually a number the underlying records cannot
//! support — most often a rate, because a set of failures has no denominator of attempts in it.
//!
//! So [`Surface::answer`] returns an [`Answer`], which is either an [`Answer::Answered`] carrying a
//! cell or an [`Answer::Unanswerable`] carrying a reason. There is no third arm and no
//! `Answer::or_zero`. A surface must also [`Surface::declared`] the questions it offers, and
//! [`audit`] checks the two halves against each other: everything declared answers, and everything
//! undeclared refuses. A surface cannot quietly grow an answer it has no evidence for, because
//! growing one makes its own audit fail.
//!
//! # What this module is not
//!
//! Not a lens, and deliberately not built on one. `bioprism-lens` answers *what question is this a
//! view of, and did anyone check*, over compiled Decision Sections; this module answers *may this
//! aggregate be shown as a number*, over already-aggregated hub objects. Reusing `Lens` would have
//! required an evidence model these surfaces do not have. The idea is reused; the code is not.

use bioprism_atlas::UnmeasuredReason;
use bioprism_hub::PublicationState;
use serde::{Deserialize, Serialize};

use crate::error::AtlasxError;

/// A fraction that carries its own denominator.
///
/// Constructed only through [`Share::new`], which refuses a zero denominator and a numerator above
/// it, and re-validated on deserialization through [`ShareFields`]. The denominator is public
/// because a share whose denominator a reader cannot see is a percentage, and section 33's
/// statistical-reporting paragraph — identical in all nineteen of its modules — requires the
/// numerator and denominator to be reported, not the ratio.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "ShareFields")]
pub struct Share {
    numerator: usize,
    denominator: usize,
}

/// The deserialization form of [`Share`], which exists so the validation runs on the way in.
#[derive(Debug, Clone, Copy, Deserialize)]
pub struct ShareFields {
    pub numerator: usize,
    pub denominator: usize,
}

impl TryFrom<ShareFields> for Share {
    type Error = AtlasxError;

    fn try_from(fields: ShareFields) -> Result<Self, Self::Error> {
        Share::new(fields.numerator, fields.denominator)
    }
}

impl Share {
    /// A share, or a refusal.
    ///
    /// Zero over zero is [`AtlasxError::EmptyDenominator`] rather than `0.0`: an empty grid has no
    /// coverage, which is not the same claim as zero coverage.
    pub fn new(numerator: usize, denominator: usize) -> Result<Self, AtlasxError> {
        if denominator == 0 {
            return Err(AtlasxError::EmptyDenominator { numerator });
        }
        if numerator > denominator {
            return Err(AtlasxError::ShareAboveOne {
                numerator,
                denominator,
            });
        }
        Ok(Share {
            numerator,
            denominator,
        })
    }

    pub fn numerator(self) -> usize {
        self.numerator
    }

    pub fn denominator(self) -> usize {
        self.denominator
    }

    /// The ratio, for a caller that has already read the denominator off the same value.
    pub fn ratio(self) -> f64 {
        self.numerator as f64 / self.denominator as f64
    }
}

/// One thing a surface renders.
///
/// Internally tagged on `kind`, so a hole and a withheld cell serialize with no numeric key at all
/// — the same shape `bioprism_atlas::CapabilityCell` and `bioprism_metrics::GridCell` use, so a
/// consumer that has learned to read one reads all three.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SurfaceCell {
    /// A count of artifacts the surface actually holds.
    Count { value: usize },
    /// A fraction, with its denominator attached.
    Share { value: Share },
    /// A number somebody else measured, passed through.
    Score { value: f64 },
    /// Nobody measured this. Carries the atlas's reason vocabulary.
    Hole { reason: UnmeasuredReason },
    /// Measured, but not showable as a number in this state.
    Withheld { state: PublicationState },
}

impl SurfaceCell {
    pub fn count(value: usize) -> Self {
        SurfaceCell::Count { value }
    }

    pub fn share(numerator: usize, denominator: usize) -> Result<Self, AtlasxError> {
        Ok(SurfaceCell::Share {
            value: Share::new(numerator, denominator)?,
        })
    }

    pub fn hole(reason: UnmeasuredReason) -> Self {
        SurfaceCell::Hole { reason }
    }

    pub fn withheld(state: PublicationState) -> Self {
        SurfaceCell::Withheld { state }
    }

    /// The number, when there is one. There is no `as_number_or_zero`, here or downstream.
    pub fn as_number(&self) -> Option<f64> {
        match self {
            SurfaceCell::Count { value } => Some(*value as f64),
            SurfaceCell::Share { value } => Some(value.ratio()),
            SurfaceCell::Score { value } => Some(*value),
            SurfaceCell::Hole { .. } | SurfaceCell::Withheld { .. } => None,
        }
    }

    /// Whether this cell carries a number a reader may compare with another cell's.
    pub fn is_numeric(&self) -> bool {
        self.as_number().is_some()
    }
}

/// A question a surface may be put.
///
/// A closed set, because the point of the set is that [`audit`] can probe a surface with the
/// questions it did *not* declare. An open-ended query string would make that check vacuous.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(tag = "question", rename_all = "snake_case")]
pub enum Question {
    /// How many artifacts fall in one bucket.
    BucketCount { bucket: String },
    /// What share of everything this surface aggregated falls in one bucket.
    ShareOfAggregated { bucket: String },
    /// Which bucket holds the most.
    ModalBucket,
    /// How much of its subject the surface covers.
    ProfileCoverage,
    /// How often the subject failed *per attempt*.
    ///
    /// The question this crate exists to refuse from a failure browse. See
    /// [`Unanswerable::NoAttemptDenominator`].
    RatePerAttempt { capability: String },
    /// How the subject stands on a capability — how often it succeeded.
    CapabilityStanding { capability: String },
    /// How this subject compares with another.
    ComparisonWith { subject: String },
}

/// Why a surface will not answer.
///
/// The three-state discipline of `bioprism-lens` — answered, refused, evidence-absent — with the
/// refusals separated by cause, because they have different remedies:
/// [`Unanswerable::EvidenceAbsent`] is fixed by measuring, [`Unanswerable::NoAttemptDenominator`]
/// is fixed by asking a different object, and [`Unanswerable::NotDeclared`] is not a defect at all.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Unanswerable {
    /// The surface does not offer this question. Distinct from being unable to answer it.
    NotDeclared,
    /// The aggregation counts events, not attempts, so the rate has no denominator in it.
    ///
    /// This is the whole of "browsing is not evidence" in one variant. A set of failure records
    /// says how many failures were *recorded*; it is silent on how many times the thing was tried,
    /// and dividing by the record count instead produces a number whose denominator is the
    /// numerator's own population.
    NoAttemptDenominator,
    /// Nothing was measured, so there is nothing to report.
    EvidenceAbsent,
    /// Answering would resolve a disagreement the evidence deliberately keeps.
    ///
    /// `bioprism_atlas::LabelDistribution` preserves reviewer disagreement instead of collapsing to
    /// a modal label. A browse that reported the mode would undo that at the last step.
    LabelContested,
    /// No such bucket. Not zero: a bucket that does not exist and a bucket holding nothing are
    /// different facts, and only the second is a measurement.
    NoSuchBucket,
    /// The question presupposes a unique answer the evidence does not single out — a tie.
    ///
    /// `bioprism_metrics::Dominance::Incomparable` is a first-class state that no arbitrary
    /// tiebreak collapses; this is the same refusal at the level of a bar chart, where the
    /// tiebreak would be whichever bar the sort happened to put first.
    NotUnique,
    /// The comparison is across subjects that were not measured under the same conditions.
    DifferentSubject,
}

/// A surface's reply.
///
/// Two arms, no third. There is no `Answer::or_zero`, no `Default`, and no `From<usize>`: the only
/// route to an [`Answer::Answered`] is a [`SurfaceCell`], which itself has no blank.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "snake_case")]
pub enum Answer {
    Answered { cell: SurfaceCell },
    Unanswerable { reason: Unanswerable },
}

impl Answer {
    pub fn answered(cell: SurfaceCell) -> Self {
        Answer::Answered { cell }
    }

    pub fn refused(reason: Unanswerable) -> Self {
        Answer::Unanswerable { reason }
    }

    pub fn cell(&self) -> Option<&SurfaceCell> {
        match self {
            Answer::Answered { cell } => Some(cell),
            Answer::Unanswerable { .. } => None,
        }
    }

    pub fn refusal(&self) -> Option<&Unanswerable> {
        match self {
            Answer::Unanswerable { reason } => Some(reason),
            Answer::Answered { .. } => None,
        }
    }

    pub fn is_answered(&self) -> bool {
        matches!(self, Answer::Answered { .. })
    }
}

/// Something a hub page renders, and the questions it will stand behind.
///
/// The trait has no method returning a bare `f64` or `usize`. Every number a surface emits leaves
/// through a [`SurfaceCell`], so a renderer that wants a bar height must first hold a cell, and a
/// cell may be a hole. That is the whole mechanism; the rest is bookkeeping.
pub trait Surface {
    /// What this surface is a reading of. Two surfaces with different subjects do not compare.
    fn subject(&self) -> &str;

    /// The questions this surface offers, in a deterministic order.
    fn declared(&self) -> Vec<Question>;

    /// The reply. Must return [`Unanswerable::NotDeclared`] for anything not in
    /// [`Surface::declared`]; [`audit`] checks it.
    fn answer(&self, question: &Question) -> Answer;

    /// Every cell this surface would render, keyed by a stable label.
    ///
    /// A caller can therefore enumerate the non-numeric cells without rendering anything, which is
    /// what a publication check needs and what a chart library will not give it.
    fn cells(&self) -> Vec<(String, SurfaceCell)>;
}

/// The probe set [`audit`] uses for questions a surface did not declare.
///
/// Deliberately includes [`Question::RatePerAttempt`]: the check that a surface refuses it is
/// exactly the check that browsing has not become evidence.
fn probes() -> Vec<Question> {
    vec![
        Question::ModalBucket,
        Question::ProfileCoverage,
        Question::RatePerAttempt {
            capability: "atlasx.audit.probe".to_string(),
        },
        Question::CapabilityStanding {
            capability: "atlasx.audit.probe".to_string(),
        },
        Question::ComparisonWith {
            subject: "atlasx.audit.probe".to_string(),
        },
        Question::BucketCount {
            bucket: "atlasx.audit.probe".to_string(),
        },
        Question::ShareOfAggregated {
            bucket: "atlasx.audit.probe".to_string(),
        },
    ]
}

/// What an [`audit`] found.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Audit {
    pub subject: String,
    /// Declared questions the surface then refused. Not an error — a declared question can be
    /// unanswerable for want of evidence — but recorded, because a surface that declares much and
    /// answers little is a page of grey boxes and the reader should be told before it is built.
    pub declared_but_refused: Vec<(Question, Unanswerable)>,
    /// Undeclared questions the surface answered anyway. Always a defect.
    pub undeclared_but_answered: Vec<Question>,
    /// Cells that carry no number, with their labels. The eight states of the shared blueprint
    /// paragraph land here rather than in a numeric series.
    pub non_numeric_cells: Vec<String>,
}

impl Audit {
    /// Whether the surface answered only what it declared.
    ///
    /// Deliberately does not consider [`Audit::declared_but_refused`]: refusing a declared question
    /// is honest behaviour, not a violation.
    pub fn sound(&self) -> bool {
        self.undeclared_but_answered.is_empty()
    }
}

/// Checks a surface against its own declaration.
///
/// Generic over [`Surface`], so one check covers every surface in this crate and any a caller adds.
pub fn audit<S: Surface + ?Sized>(surface: &S) -> Audit {
    let declared = surface.declared();

    let mut declared_but_refused = Vec::new();
    for question in &declared {
        if let Answer::Unanswerable { reason } = surface.answer(question) {
            declared_but_refused.push((question.clone(), reason));
        }
    }

    let mut undeclared_but_answered = Vec::new();
    for probe in probes() {
        if declared.contains(&probe) {
            continue;
        }
        if surface.answer(&probe).is_answered() {
            undeclared_but_answered.push(probe);
        }
    }

    let non_numeric_cells = surface
        .cells()
        .into_iter()
        .filter(|(_, cell)| !cell.is_numeric())
        .map(|(label, _)| label)
        .collect();

    Audit {
        subject: surface.subject().to_string(),
        declared_but_refused,
        undeclared_but_answered,
        non_numeric_cells,
    }
}
