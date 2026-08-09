//! Coverage debt as a derived claim — blueprint 34.08.
//!
//! 34.08 asks the hub to "render conditional capability profiles rather than one general score",
//! and lists *profile coverage* first among its five product metrics. It does not say what the
//! denominator is. This module says, and the saying is the point: a coverage number whose
//! denominator is unstated is the same artifact as an unmeasured cell rendered as a low bar.
//!
//! # The fact, stated from the other side
//!
//! `bioprism-metrics` made a bare aggregate unrepresentable: `BareScore` implements neither
//! `Serialize` nor `Deserialize`, so a score cannot leave the process without the coverage it was
//! computed over. A [`DebtStatement`] is the same fact stated from the other side — not *this score
//! covers 7 of 12* but *these 5 were never measured, and here is which* — and it inherits the same
//! rule in the same way: private fields, one constructor that reads a grid, and a deserialization
//! path that recomputes the summary and refuses a stored statement its own contents contradict.
//!
//! A debt is therefore never asserted. There is no `DebtStatement::new(total, unmeasured)`, because
//! a pair of integers is exactly the artifact this module exists to prevent: a "94% covered" badge
//! with nothing behind it. [`DebtStatement::of`] is the only way in, and it names every hole.
//!
//! # Why not reuse `bioprism_atlas::CoverageDebt`
//!
//! Because it is a different object with a deliberately different failure mode, and both should
//! exist. `CoverageDebt` is a *summary inside a coverage report*: it sits beside `holes`, which
//! already lists the capabilities, so its counts are never orphaned and it is free to be plain
//! data. Its `ratio` returns `0.0` for an empty ontology and documents that as "a vacuous atlas
//! rather than a complete one", telling the reader to check `total_capabilities`.
//!
//! A hub surface has no such reader. It has a renderer, and a renderer given `0.0` draws an empty
//! bar. So [`DebtStatement::profile_coverage`] refuses instead: zero of zero is
//! [`Unanswerable::EvidenceAbsent`], and the cell is a [`SurfaceCell::Hole`] with nothing to draw.
//! The two types disagree about the empty case on purpose, and this paragraph is the record of the
//! disagreement.
//!
//! # Not implemented
//!
//! - **No second atlas.** A [`DebtStatement`] holds no scores, no evidence and no ontology. It
//!   reads a `bioprism_metrics::CapabilityGrid` and keeps identifiers.
//! - **No debt over time.** [`DebtStatement::discharged_by`] compares two statements a caller
//!   already has; it does not store, schedule or version them.
//! - **No target.** 34.08 names "profile coverage" as a metric to move and never says what value is
//!   enough. Nothing here has a threshold, and a release gate that wants one belongs in
//!   `bioprism-metrics`, which owns gates.

use std::collections::BTreeMap;

use bioprism_atlas::{CapabilityId, UnmeasuredReason};
use bioprism_hub::PublicationState;
use bioprism_metrics::CapabilityGrid;
use serde::{Deserialize, Serialize};

use crate::error::AtlasxError;
use crate::surface::{Answer, Question, Share, Surface, SurfaceCell, Unanswerable};

/// One capability that was not measured, and why.
///
/// The reason is `bioprism_atlas::UnmeasuredReason` rather than a local enum, so "nobody looked"
/// and "policy forbids it" mean the same thing in the atlas, in the grid and on the page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Hole {
    pub capability: CapabilityId,
    pub reason: UnmeasuredReason,
}

impl Hole {
    /// How this hole renders.
    ///
    /// A policy-inaccessible hole is [`PublicationState::Controlled`] rather than a generic hole:
    /// the measurement exists as a possibility and is being withheld, which is a different sentence
    /// from "nobody looked" and the shared blueprint paragraph requires the surface to keep them
    /// apart. Every other reason renders as a hole carrying its own text.
    pub fn cell(&self) -> SurfaceCell {
        match self.reason {
            UnmeasuredReason::InaccessibleByPolicy => {
                SurfaceCell::withheld(PublicationState::Controlled)
            }
            other => SurfaceCell::hole(other),
        }
    }

    /// Whether this hole leaves a claim about the subject blocked.
    ///
    /// Delegates to `bioprism_atlas::UnmeasuredReason::supports_claim`, which is the single
    /// predicate the atlas, the grid and this surface all consult. A hole closed by a declared
    /// intended use is the only kind that does not block.
    pub fn blocks_claim(&self) -> bool {
        !self.reason.supports_claim()
    }
}

/// What was not measured about one reading of a grid.
///
/// Private fields with one constructor, for the reason `bioprism_metrics::CoveredAggregate` has
/// them: the summary counts are redundant against the hole list, and a redundant count that can be
/// set independently is a count that will eventually be set wrong.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "DebtStatementFields")]
pub struct DebtStatement {
    subject: String,
    total_capabilities: usize,
    holes: Vec<Hole>,
    /// Redundant against `holes`. Recomputed on the way in; see [`DebtStatementFields`].
    closed_by_declaration: usize,
}

/// The deserialization form of [`DebtStatement`].
///
/// Exists so that a stored debt is re-derived rather than trusted. A statement edited to say it has
/// fewer blocking holes than its own list contains fails to load.
#[derive(Debug, Clone, Deserialize)]
pub struct DebtStatementFields {
    pub subject: String,
    pub total_capabilities: usize,
    pub holes: Vec<Hole>,
    pub closed_by_declaration: usize,
}

impl TryFrom<DebtStatementFields> for DebtStatement {
    type Error = AtlasxError;

    fn try_from(fields: DebtStatementFields) -> Result<Self, Self::Error> {
        let mut seen: BTreeMap<&str, ()> = BTreeMap::new();
        for hole in &fields.holes {
            if seen.insert(hole.capability.as_str(), ()).is_some() {
                return Err(AtlasxError::DuplicateHole {
                    subject: fields.subject.clone(),
                    capability: hole.capability.as_str().to_string(),
                });
            }
        }
        if fields.holes.len() > fields.total_capabilities {
            return Err(AtlasxError::DebtExceedsGrid {
                subject: fields.subject.clone(),
                claimed: fields.total_capabilities,
                named: fields.holes.len(),
            });
        }
        let derived = fields.holes.iter().filter(|h| !h.blocks_claim()).count();
        if derived != fields.closed_by_declaration {
            return Err(AtlasxError::DebtNotDerivable {
                subject: fields.subject.clone(),
                asserted: fields.closed_by_declaration,
                derived,
            });
        }
        Ok(DebtStatement {
            subject: fields.subject,
            total_capabilities: fields.total_capabilities,
            holes: fields.holes,
            closed_by_declaration: derived,
        })
    }
}

impl DebtStatement {
    /// Reads a grid. The only constructor.
    ///
    /// The subject is the grid's label, not a caller's string, because
    /// `bioprism_metrics::CapabilityGrid::restricted_to` forces a relabel precisely so a restricted
    /// reading cannot masquerade as the whole. Taking the label here inherits that guarantee;
    /// taking a caller's string would discard it.
    pub fn of(grid: &CapabilityGrid) -> Self {
        let holes: Vec<Hole> = grid
            .holes()
            .map(|(capability, reason)| Hole {
                capability: capability.clone(),
                reason,
            })
            .collect();
        let closed_by_declaration = holes.iter().filter(|h| !h.blocks_claim()).count();
        DebtStatement {
            subject: grid.label.clone(),
            total_capabilities: grid.len(),
            holes,
            closed_by_declaration,
        }
    }

    pub fn total_capabilities(&self) -> usize {
        self.total_capabilities
    }

    pub fn measured(&self) -> usize {
        self.total_capabilities - self.holes.len()
    }

    pub fn unmeasured(&self) -> usize {
        self.holes.len()
    }

    /// Every hole, named. A debt that is only a count is not a debt, it is a mood.
    pub fn holes(&self) -> &[Hole] {
        &self.holes
    }

    /// The holes that still block a claim, i.e. all but those a declared intended use closes.
    pub fn blocking(&self) -> Vec<&Hole> {
        self.holes.iter().filter(|h| h.blocks_claim()).collect()
    }

    pub fn closed_by_declaration(&self) -> usize {
        self.closed_by_declaration
    }

    /// A grid with no capabilities in it. Vacuous, not complete.
    pub fn is_vacuous(&self) -> bool {
        self.total_capabilities == 0
    }

    /// 34.08's first product metric, with the denominator this crate supplies.
    ///
    /// Numerator: capabilities with a measured cell. Denominator: capabilities in the grid. Not
    /// capabilities in the ontology — a grid is a reading, and a reading of three capabilities is
    /// fully covered *as a reading of three capabilities*, which is why [`DebtStatement::subject`]
    /// travels with the number and why two subjects do not compare.
    ///
    /// Refuses on an empty grid. See this module's note on `bioprism_atlas::CoverageDebt::ratio`,
    /// which returns `0.0` there instead, deliberately and for a different audience.
    pub fn profile_coverage(&self) -> Answer {
        match Share::new(self.measured(), self.total_capabilities) {
            Ok(share) => Answer::answered(SurfaceCell::Share { value: share }),
            Err(_) => Answer::refused(Unanswerable::EvidenceAbsent),
        }
    }

    /// What a later reading of the same subject settled.
    ///
    /// The distinction this method exists for: a hole that moved from *not attempted* to *out of
    /// scope by declared use* was not measured, it was declared away. Both close the hole for
    /// claim purposes; only one is evidence, and a coverage percentage that rises because somebody
    /// narrowed the declared use is not a system that got better.
    pub fn discharged_by(&self, later: &DebtStatement) -> Result<Discharge, AtlasxError> {
        if self.subject != later.subject {
            return Err(AtlasxError::DebtSubjectsDiffer {
                left: self.subject.clone(),
                right: later.subject.clone(),
            });
        }
        let before: BTreeMap<&str, UnmeasuredReason> = self
            .holes
            .iter()
            .map(|h| (h.capability.as_str(), h.reason))
            .collect();
        let after: BTreeMap<&str, UnmeasuredReason> = later
            .holes
            .iter()
            .map(|h| (h.capability.as_str(), h.reason))
            .collect();

        let mut measured = Vec::new();
        let mut declared_away = Vec::new();
        let mut persisting = Vec::new();
        for (capability, reason) in &before {
            match after.get(capability) {
                None => measured.push(capability.to_string()),
                Some(UnmeasuredReason::OutOfScopeByDeclaredUse)
                    if *reason != UnmeasuredReason::OutOfScopeByDeclaredUse =>
                {
                    declared_away.push(capability.to_string())
                }
                Some(_) => persisting.push(capability.to_string()),
            }
        }
        let newly_unmeasured = after
            .keys()
            .filter(|capability| !before.contains_key(*capability))
            .map(|capability| capability.to_string())
            .collect();

        Ok(Discharge {
            subject: self.subject.clone(),
            measured,
            declared_away,
            persisting,
            newly_unmeasured,
        })
    }
}

/// What changed between two debt statements about one subject.
///
/// Four disjoint lists and no net number. A single "holes closed" integer would add
/// [`Discharge::measured`] to [`Discharge::declared_away`], and those are not the same currency.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Discharge {
    pub subject: String,
    /// Holes that became measured cells. The only entries that are evidence.
    pub measured: Vec<String>,
    /// Holes closed by narrowing the declared intended use. Not evidence.
    pub declared_away: Vec<String>,
    pub persisting: Vec<String>,
    /// Capabilities unmeasured in the later reading that the earlier one did not have at all —
    /// usually an ontology growing. Never counted against the earlier statement.
    pub newly_unmeasured: Vec<String>,
}

impl Discharge {
    /// Whether anything was actually measured. False when the only movement was declaratory.
    pub fn any_evidence(&self) -> bool {
        !self.measured.is_empty()
    }
}

const BUCKET_MEASURED: &str = "measured";
const BUCKET_UNMEASURED: &str = "unmeasured";
const BUCKET_BLOCKING: &str = "blocking";
const CELL_PROFILE_COVERAGE: &str = "profile_coverage";

impl Surface for DebtStatement {
    fn subject(&self) -> &str {
        &self.subject
    }

    fn declared(&self) -> Vec<Question> {
        vec![
            Question::ProfileCoverage,
            Question::BucketCount {
                bucket: BUCKET_MEASURED.to_string(),
            },
            Question::BucketCount {
                bucket: BUCKET_UNMEASURED.to_string(),
            },
            Question::BucketCount {
                bucket: BUCKET_BLOCKING.to_string(),
            },
            Question::ShareOfAggregated {
                bucket: BUCKET_UNMEASURED.to_string(),
            },
        ]
    }

    fn answer(&self, question: &Question) -> Answer {
        match question {
            Question::ProfileCoverage => self.profile_coverage(),
            Question::BucketCount { bucket } => match bucket.as_str() {
                BUCKET_MEASURED => Answer::answered(SurfaceCell::count(self.measured())),
                BUCKET_UNMEASURED => Answer::answered(SurfaceCell::count(self.unmeasured())),
                BUCKET_BLOCKING => Answer::answered(SurfaceCell::count(self.blocking().len())),
                _ => Answer::refused(Unanswerable::NotDeclared),
            },
            Question::ShareOfAggregated { bucket } if bucket == BUCKET_UNMEASURED => {
                match SurfaceCell::share(self.unmeasured(), self.total_capabilities) {
                    Ok(cell) => Answer::answered(cell),
                    Err(_) => Answer::refused(Unanswerable::EvidenceAbsent),
                }
            }
            // A debt statement records whether a capability was measured, never how the subject
            // performed. Standing, rates and comparisons are `bioprism-metrics`' objects and asking
            // this surface for one is asking the wrong thing, not asking it badly.
            _ => Answer::refused(Unanswerable::NotDeclared),
        }
    }

    fn cells(&self) -> Vec<(String, SurfaceCell)> {
        let mut cells = vec![
            (
                BUCKET_MEASURED.to_string(),
                SurfaceCell::count(self.measured()),
            ),
            (
                BUCKET_UNMEASURED.to_string(),
                SurfaceCell::count(self.unmeasured()),
            ),
            (
                BUCKET_BLOCKING.to_string(),
                SurfaceCell::count(self.blocking().len()),
            ),
        ];
        cells.push((
            CELL_PROFILE_COVERAGE.to_string(),
            match self.profile_coverage().cell() {
                Some(cell) => cell.clone(),
                None => SurfaceCell::hole(UnmeasuredReason::NotAttempted),
            },
        ));
        for hole in &self.holes {
            cells.push((hole.capability.as_str().to_string(), hole.cell()));
        }
        cells
    }
}
