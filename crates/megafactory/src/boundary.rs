//! The BioDecision compiler and boundary detection.
//!
//! Blueprint 35.07: "compile long workflows into meaningful decision cells rather than arbitrary
//! message chunks."
//!
//! `bioprism-trace` already segments a trajectory and ranks candidate cells, and `bioprism-prism`
//! owns the frozen `DecisionCell` and the reviewer approval that is the only path to one. Neither
//! is duplicated. This module supplies the two things 35.07 asks for that neither has: the
//! taxonomy of *why* a point is a decision, and the measurement `bioprism-trace` states in its own
//! documentation has never been run — whether a detector's boundaries agree with a human's.
//!
//! ## The seven kinds are about why, not what
//!
//! [`BoundaryKind`] is 35.07's required-components list read as an enum: a goal or obligation
//! changed, evidence was acquired, an analysis was selected, a claim was created or revised,
//! resources were spent, work was handed off or escalated, or something irreversible happened.
//! That is orthogonal to `bioprism_trace::EventKind`, which classifies what the transcript *shows*
//! — an action, an observation, a claim. A single tool call can be any of the seven or none.
//!
//! ## A single agreement number is refused when an irreversible boundary was missed
//!
//! The seventh kind is not exchangeable with the other six. Missing a routine evidence-acquisition
//! boundary costs a benchmark one instance; missing the point where an agent did something it
//! cannot undo means the compiled suite contains no test of the decision that mattered most.
//! [`BoundaryAgreement`] therefore reports recall per kind, lists
//! [`BoundaryAgreement::missed_high_regret`] separately, and
//! [`BoundaryAgreement::is_publishable`] is false whenever that list is non-empty **no matter how
//! high the overall figure is**. An F-score that averages an irreversible miss into six routine
//! hits is the arithmetic form of hiding it.
//!
//! ## Tolerance is required and travels with the number
//!
//! A boundary detected one span early is not obviously a miss, and how early is "one" is a choice
//! nobody can make for a caller. [`BoundaryAgreement::measure`] takes `tolerance` as a required
//! argument, never defaults it, and stores it in the result — the same discipline
//! `bioprism_scale::SimilarityRelation` applies to effective size, for the same reason: the number
//! is meaningless without the definition that produced it, so they travel in one object.
//!
//! Matching is greedy in sequence order and one-to-one: a single proposed boundary cannot be
//! credited against two reference boundaries, which is how a detector that fires on everything
//! scores well on a lenient matcher.
//!
//! ## What this produces, and what it deliberately does not
//!
//! [`compile_cell_spans`] cuts a complete session into half-open span ranges at the boundaries and
//! reports, per range, whether it is replayable. It emits [`CellSpan`], **not** a decision cell:
//! minting one of those requires `bioprism_prism::DecisionCell`'s reviewer approval, and a crate
//! that produced cells without review would route around the gate that exists to stop exactly
//! that. A gapped session is refused outright by `crate::trajectory::CaptureSession::require_complete`.
//!
//! There is no detector here. Ranking candidate boundaries is `bioprism-trace`'s, and its own
//! documentation says the ranking is unvalidated; what this module adds is the apparatus to
//! validate any such detector against annotated truth.

use crate::error::BoundaryError;
use crate::trajectory::{CaptureSession, Completeness};
use bioprism_scale::audit::ReleaseAudit;
use bioprism_scale::QualityGate;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// Why a point in a workflow is a decision. 35.07's seven required components.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BoundaryKind {
    /// The goal or an obligation changed.
    GoalOrObligationChange,
    /// Evidence entered the workflow.
    EvidenceAcquisition,
    /// An analysis was chosen among alternatives.
    AnalysisSelection,
    /// A claim was created or revised.
    ClaimCreationOrRevision,
    /// Resources were spent.
    ResourceExpenditure,
    /// Work was handed off or escalated.
    HandoffOrEscalation,
    /// Something irreversible or high-regret happened. Not exchangeable with the others.
    IrreversibleOrHighRegretAction,
}

impl BoundaryKind {
    pub const ALL: [BoundaryKind; 7] = [
        BoundaryKind::GoalOrObligationChange,
        BoundaryKind::EvidenceAcquisition,
        BoundaryKind::AnalysisSelection,
        BoundaryKind::ClaimCreationOrRevision,
        BoundaryKind::ResourceExpenditure,
        BoundaryKind::HandoffOrEscalation,
        BoundaryKind::IrreversibleOrHighRegretAction,
    ];

    pub fn as_str(self) -> &'static str {
        match self {
            BoundaryKind::GoalOrObligationChange => "goal_or_obligation_change",
            BoundaryKind::EvidenceAcquisition => "evidence_acquisition",
            BoundaryKind::AnalysisSelection => "analysis_selection",
            BoundaryKind::ClaimCreationOrRevision => "claim_creation_or_revision",
            BoundaryKind::ResourceExpenditure => "resource_expenditure",
            BoundaryKind::HandoffOrEscalation => "handoff_or_escalation",
            BoundaryKind::IrreversibleOrHighRegretAction => "irreversible_or_high_regret_action",
        }
    }

    /// Whether missing this boundary is a different kind of error from missing the others.
    pub fn is_high_regret(self) -> bool {
        self == BoundaryKind::IrreversibleOrHighRegretAction
    }
}

/// One decision boundary: where it is, and why it is one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Boundary {
    /// The span sequence number this boundary sits at.
    pub seq: u64,
    pub kind: BoundaryKind,
}

impl Boundary {
    pub fn new(seq: u64, kind: BoundaryKind) -> Self {
        Boundary { seq, kind }
    }
}

/// Human-annotated boundaries, attributed to the person who annotated them.
///
/// The annotator is required. 35.07 lists "author review agreement" as an operational metric and an
/// agreement figure against an anonymous reference is agreement with nobody, so
/// [`ReferenceBoundaries::new`] refuses an empty annotator.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceBoundaries {
    pub annotator: String,
    boundaries: Vec<Boundary>,
}

impl ReferenceBoundaries {
    pub fn new(
        annotator: impl Into<String>,
        boundaries: Vec<Boundary>,
    ) -> Result<Self, BoundaryError> {
        let annotator = annotator.into();
        if annotator.trim().is_empty() {
            return Err(BoundaryError::UnattributedReference);
        }
        check_unique(&boundaries, "reference")?;
        let mut boundaries = boundaries;
        boundaries.sort();
        Ok(ReferenceBoundaries {
            annotator,
            boundaries,
        })
    }

    pub fn boundaries(&self) -> &[Boundary] {
        &self.boundaries
    }

    /// Fails if any annotated boundary sits outside the session it claims to annotate.
    pub fn check_within(&self, session: &CaptureSession) -> Result<(), BoundaryError> {
        let (Some(first), Some(last)) = (session.spans().first(), session.spans().last()) else {
            return Ok(());
        };
        for boundary in &self.boundaries {
            if boundary.seq < first.seq || boundary.seq > last.seq {
                return Err(BoundaryError::BoundaryOutsideSession {
                    seq: boundary.seq,
                    first: first.seq,
                    last: last.seq,
                });
            }
        }
        Ok(())
    }
}

fn check_unique(boundaries: &[Boundary], set: &str) -> Result<(), BoundaryError> {
    let mut seen: BTreeSet<u64> = BTreeSet::new();
    for boundary in boundaries {
        if !seen.insert(boundary.seq) {
            return Err(BoundaryError::DuplicateBoundary {
                set: set.to_string(),
                seq: boundary.seq,
            });
        }
    }
    Ok(())
}

/// Agreement for one boundary kind.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct KindAgreement {
    pub kind: BoundaryKind,
    pub reference: usize,
    pub proposed: usize,
    pub matched: usize,
    pub recall: f64,
    pub precision: f64,
}

/// How well a proposed boundary set agrees with an annotated one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BoundaryAgreement {
    pub annotator: String,
    /// How far apart two boundaries may sit and still match. Required, never defaulted, and stored
    /// so the figures below are never read without it.
    pub tolerance: u64,
    pub reference: usize,
    pub proposed: usize,
    pub matched: usize,
    pub precision: f64,
    pub recall: f64,
    pub f1: f64,
    pub by_kind: Vec<KindAgreement>,
    /// Annotated irreversible boundaries with no proposed match. Reported on its own because it is
    /// not exchangeable with the rest.
    pub missed_high_regret: Vec<Boundary>,
}

impl BoundaryAgreement {
    /// Matches `proposed` against `reference` greedily, one-to-one, within `tolerance` and kind.
    ///
    /// A proposal matches an annotation when the kinds are equal and the sequence numbers are
    /// within `tolerance`. Ties go to the nearest, then to the lower sequence number, so the result
    /// does not depend on input order.
    pub fn measure(
        reference: &ReferenceBoundaries,
        proposed: &[Boundary],
        tolerance: u64,
    ) -> Result<Self, BoundaryError> {
        check_unique(proposed, "proposed")?;
        let mut proposals: Vec<Boundary> = proposed.to_vec();
        proposals.sort();

        let mut taken: Vec<bool> = vec![false; proposals.len()];
        let mut matched_pairs: Vec<(Boundary, Boundary)> = Vec::new();

        for annotated in reference.boundaries() {
            let mut best: Option<(u64, usize)> = None;
            for (index, proposal) in proposals.iter().enumerate() {
                if taken[index] || proposal.kind != annotated.kind {
                    continue;
                }
                let distance = annotated.seq.abs_diff(proposal.seq);
                if distance > tolerance {
                    continue;
                }
                let better = match best {
                    None => true,
                    Some((best_distance, _)) => distance < best_distance,
                };
                if better {
                    best = Some((distance, index));
                }
            }
            if let Some((_, index)) = best {
                taken[index] = true;
                matched_pairs.push((*annotated, proposals[index]));
            }
        }

        let matched = matched_pairs.len();
        let reference_count = reference.boundaries().len();
        let proposed_count = proposals.len();
        let precision = share(matched, proposed_count);
        let recall = share(matched, reference_count);
        let f1 = if precision + recall == 0.0 {
            0.0
        } else {
            2.0 * precision * recall / (precision + recall)
        };

        let matched_reference: BTreeSet<Boundary> = matched_pairs
            .iter()
            .map(|(annotated, _)| *annotated)
            .collect();
        let missed_high_regret: Vec<Boundary> = reference
            .boundaries()
            .iter()
            .filter(|boundary| {
                boundary.kind.is_high_regret() && !matched_reference.contains(boundary)
            })
            .copied()
            .collect();

        let mut by_kind: Vec<KindAgreement> = Vec::new();
        for kind in BoundaryKind::ALL {
            let reference_of_kind = reference
                .boundaries()
                .iter()
                .filter(|boundary| boundary.kind == kind)
                .count();
            let proposed_of_kind = proposals
                .iter()
                .filter(|boundary| boundary.kind == kind)
                .count();
            let matched_of_kind = matched_pairs
                .iter()
                .filter(|(annotated, _)| annotated.kind == kind)
                .count();
            if reference_of_kind == 0 && proposed_of_kind == 0 {
                continue;
            }
            by_kind.push(KindAgreement {
                kind,
                reference: reference_of_kind,
                proposed: proposed_of_kind,
                matched: matched_of_kind,
                recall: share(matched_of_kind, reference_of_kind),
                precision: share(matched_of_kind, proposed_of_kind),
            });
        }

        Ok(BoundaryAgreement {
            annotator: reference.annotator.clone(),
            tolerance,
            reference: reference_count,
            proposed: proposed_count,
            matched,
            precision,
            recall,
            f1,
            by_kind,
            missed_high_regret,
        })
    }

    /// Whether this agreement may stand behind a claim that the boundaries are meaningful.
    ///
    /// False whenever an irreversible boundary was missed, whatever the F-score says.
    pub fn is_publishable(&self) -> bool {
        self.missed_high_regret.is_empty() && self.matched > 0
    }

    /// The sentence that must accompany the figures.
    pub fn headline(&self) -> String {
        if !self.missed_high_regret.is_empty() {
            return format!(
                "{} of {} boundaries matched against {} at tolerance {} (F1 {:.3}) — but {} \
                 irreversible boundary/boundaries were missed at {:?}, so the aggregate does not \
                 stand",
                self.matched,
                self.reference,
                self.annotator,
                self.tolerance,
                self.f1,
                self.missed_high_regret.len(),
                self.missed_high_regret
                    .iter()
                    .map(|boundary| boundary.seq)
                    .collect::<Vec<_>>()
            );
        }
        format!(
            "{} of {} annotated boundaries matched against {} at tolerance {}: precision {:.3}, \
             recall {:.3}, F1 {:.3}",
            self.matched,
            self.reference,
            self.annotator,
            self.tolerance,
            self.precision,
            self.recall,
            self.f1
        )
    }

    /// Records the meaningful-boundaries release gate.
    ///
    /// This is the one gate a boundary measurement is entitled to speak to, and only because the
    /// reference is attributed to a named annotator. It passes only when no irreversible boundary
    /// was missed.
    pub fn contribute_to(&self, audit: &mut ReleaseAudit) {
        audit.record(
            QualityGate::MeaningfulBoundaries,
            self.is_publishable(),
            self.headline(),
        );
    }
}

fn share(part: usize, whole: usize) -> f64 {
    if whole == 0 {
        0.0
    } else {
        part as f64 / whole as f64
    }
}

/// Whether a compiled span can be replayed from what the session recorded.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "replayable", rename_all = "snake_case")]
pub enum Replayable {
    Yes,
    /// The prefix this span would resume from is not intact, and why.
    No {
        reason: String,
    },
}

impl Replayable {
    pub fn is_yes(&self) -> bool {
        matches!(self, Replayable::Yes)
    }
}

/// A half-open span range proposed as a decision cell.
///
/// Not a decision cell. `bioprism_prism::DecisionCell` is minted only through reviewer approval and
/// this type carries no such token; a caller wanting a cell must take this proposal through that
/// gate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CellSpan {
    pub boundary: Boundary,
    /// First span sequence in the cell, inclusive.
    pub start: u64,
    /// First span sequence after the cell.
    pub end: u64,
    /// Spans the cell resumes from — everything before `start` in the session.
    pub prefix_spans: usize,
    pub spans: usize,
    pub replayable: Replayable,
}

/// Cuts a session into cell spans at `boundaries`.
///
/// A gapped session does not fail here; every span it produces is marked [`Replayable::No`] with
/// the reason, so a caller who ignores the flag ships proposals that say plainly they cannot be
/// resumed from. `crate::trajectory::CaptureSession::require_complete` is the hard gate for callers
/// who want the refusal instead.
pub fn compile_cell_spans(
    session: &CaptureSession,
    boundaries: &[Boundary],
) -> Result<Vec<CellSpan>, BoundaryError> {
    if session.is_empty() || boundaries.is_empty() {
        return Ok(Vec::new());
    }
    check_unique(boundaries, "compile")?;
    let complete = matches!(session.completeness(), Completeness::Complete { .. });

    let last = session.spans().last().expect("session is non-empty").seq;
    let mut sorted: Vec<Boundary> = boundaries.to_vec();
    sorted.sort();

    let mut spans = Vec::new();
    for (index, boundary) in sorted.iter().enumerate() {
        let start = boundary.seq;
        let end = sorted
            .get(index + 1)
            .map(|next| next.seq)
            .unwrap_or(last + 1);
        if end <= start {
            return Err(BoundaryError::EmptyCellSpan { start, end });
        }
        let prefix_spans = session
            .spans()
            .iter()
            .filter(|span| span.seq < start)
            .count();
        let contained = session
            .spans()
            .iter()
            .filter(|span| span.seq >= start && span.seq < end)
            .count();
        spans.push(CellSpan {
            boundary: *boundary,
            start,
            end,
            prefix_spans,
            spans: contained,
            replayable: if complete {
                Replayable::Yes
            } else {
                Replayable::No {
                    reason: "the session has interior gaps; the prefix this cell resumes from is \
                             not intact"
                        .into(),
                }
            },
        });
    }
    Ok(spans)
}

/// How much a cell compilation reduces what has to be executed.
///
/// 35.07 lists cost reduction as an operational metric. This is the honest reading of it: spans in
/// the session over cells compiled from it. It says nothing about whether the cells are the right
/// ones — that is [`BoundaryAgreement`]'s question, and a compiler that emits one cell per session
/// scores best here and worst there.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CompressionReport {
    pub spans: usize,
    pub cells: usize,
    pub spans_per_cell: f64,
    pub replayable_cells: usize,
    pub by_kind: BTreeMap<String, usize>,
}

impl CompressionReport {
    pub fn of(session: &CaptureSession, cells: &[CellSpan]) -> Self {
        let mut by_kind: BTreeMap<String, usize> = BTreeMap::new();
        for cell in cells {
            *by_kind
                .entry(cell.boundary.kind.as_str().to_string())
                .or_default() += 1;
        }
        CompressionReport {
            spans: session.len(),
            cells: cells.len(),
            spans_per_cell: if cells.is_empty() {
                0.0
            } else {
                session.len() as f64 / cells.len() as f64
            },
            replayable_cells: cells.iter().filter(|cell| cell.replayable.is_yes()).count(),
            by_kind,
        }
    }
}
