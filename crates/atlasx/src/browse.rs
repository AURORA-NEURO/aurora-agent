//! The failure-atlas browsing surface — blueprint 34.09.
//!
//! 34.09 asks the hub to "aggregate minimized failures by mechanism and allow developers to
//! reproduce them locally". `bioprism-atlas` owns the failures: the closed mechanism taxonomy, the
//! unflattened causal chain, the first-divergence step, and label distributions that keep reviewer
//! disagreement instead of resolving it. This module owns the one thing aggregating them for a page
//! adds, which is a new way to be wrong.
//!
//! # Browsing is not evidence
//!
//! A grouped bar chart of failures invites exactly one question — *how often does it do that?* — and
//! a set of failure records cannot answer it. The set says how many failures were **recorded**. It
//! is silent on how many times the thing was attempted, so any rate computed inside the browse has
//! the numerator's own population as its denominator. The number that comes out is well-formed,
//! stable, and meaningless, which is the worst combination a hub page can ship.
//!
//! So [`FailureBrowse`] *declares* [`Question::RatePerAttempt`] — users will ask it, and pretending
//! the question does not exist is not an answer — and refuses it with
//! [`Unanswerable::NoAttemptDenominator`]. The rate becomes available through exactly one door:
//! [`FailureBrowse::rate_against`], which takes a `bioprism_metrics::CapabilityGrid`, refuses if the
//! grid is a reading of a different subject, refuses if the capability's cell is a hole, and
//! otherwise divides by the cell's **effective size** — independent clustering units, not trials,
//! which is the denominator section 33's stratification paragraph requires and the only one the
//! grid actually carries.
//!
//! This is `bioprism-lens`'s move against a different failure. There, a lens that can only produce a
//! rendering has no type to satisfy the `Witness` bound and does not compile. Here, an aggregation
//! that can only be looked at cannot manufacture the denominator it would need to be evidence, and
//! has to borrow one from a measurement that names its own subject.
//!
//! # Withheld records leave the buckets, never the denominator
//!
//! The paragraph 34.09 shares with the rest of its section requires the surface to support
//! unavailable, controlled, stale, under-review, disputed, withdrawn, non-reproducible and
//! not-comparable states, and forbids replacing them with "zero, empty, or hidden values".
//!
//! A withheld failure cannot sit in its mechanism bucket, because the bucket label would disclose
//! the diagnosis the state is withholding. It also cannot leave the denominator, because then the
//! visible shares would sum to one and the page would look complete. So a withheld record moves to
//! a bucket keyed by its state — [`BucketKey::Withheld`] — and stays in
//! [`FailureBrowse::records_browsed`]. The consequence is deliberate and is what
//! [`FailureBrowse::shares_sum_to_one`] reports: **withholding a record shrinks every visible share
//! and shrinks none of the denominator.** The gap is the disclosure.
//!
//! # What is aggregated but never resolved
//!
//! - A record whose reviewers disagree goes to [`BucketKey::Contested`], not to its modal
//!   mechanism. `bioprism_atlas::LabelDistribution` preserves the disagreement through evidence,
//!   diagnosis and reporting; taking the mode at render time would undo all of it at the last step.
//! - [`Question::ModalBucket`] refuses on a tie rather than letting the sort order decide.
//! - `bioprism_atlas::FailureMechanism::Unclassified` keeps its own bucket. It is a defect report
//!   against the taxonomy, and merging it into a neighbour is how the taxonomy stops being closed.
//! - A failure with no architecture component goes to [`BucketKey::Unattributed`], not to a
//!   plausible one.
//!
//! # Metrics 34.09 names and this module does not compute
//!
//! *Failure localization precision* — needs a ground truth for the true divergence point, which no
//! `bioprism_atlas::FailureRecord` carries; [`FailureBrowse::undiagnosed`] reports the count of
//! records that did not localize, which is the part the evidence supports.
//! *Download-to-replay success* and *regression closure* are workflow outcomes with no artifact
//! here. *Cross-system transfer* is a comparison across subjects and this surface refuses those.
//! *Duplicate failure reduction* has no baseline: [`FailureBrowse::distinct_families`] reports how
//! many distinct causal chains the browsed set contains, and no reduction is computed, because a
//! reduction is a difference against a "before" the module never says how to obtain.

use std::collections::{BTreeMap, BTreeSet};

use bioprism_atlas::{
    CapabilityId, FailureLabel, FailureMechanism, FailureRecord, FailureStage, Inducement, Severity,
};
use bioprism_hub::PublicationState;
use bioprism_metrics::CapabilityGrid;
use serde::{Deserialize, Serialize};

use crate::error::AtlasxError;
use crate::surface::{Answer, Question, Surface, SurfaceCell, Unanswerable};

/// How the browsed failures are partitioned.
///
/// Five facets, each derived from a field `bioprism_atlas::FailureRecord` already carries. There is
/// no free-text facet, because a facet a caller can invent is a facet whose buckets cannot be
/// checked against a closed set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Facet {
    /// By diagnosed mechanism. Contested diagnoses do not land in a mechanism bucket.
    Mechanism,
    /// By the stage of the first divergence, which is derived from the chain, never declared.
    FirstDivergenceStage,
    Severity,
    /// Whether the failure is charged to the system at all. An evaluator-induced failure is a
    /// benchmark defect.
    Inducement,
    ArchitectureComponent,
}

/// What one bucket is keyed by.
///
/// The non-facet arms are the ones that matter: each exists so a record that does not fit the facet
/// has somewhere honest to go, instead of being dropped or placed in a plausible neighbour.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "key", rename_all = "snake_case")]
pub enum BucketKey {
    Mechanism {
        mechanism: FailureMechanism,
    },
    Stage {
        stage: FailureStage,
    },
    Severity {
        severity: Severity,
    },
    Inducement {
        inducement: Inducement,
    },
    Component {
        component: String,
    },
    /// A failure whose diagnosis points at no identified component.
    Unattributed,
    /// A failure whose reviewers disagree about the mechanism.
    Contested,
    /// A failure whose chain yields no stage, which happens when the taxonomy did not cover it.
    Unstaged,
    /// A failure that exists but may not be shown as part of a diagnosis bucket.
    Withheld {
        state: PublicationState,
    },
}

impl BucketKey {
    /// A stable label, used as the bucket's map key and as the string in a [`Question`].
    ///
    /// Formatting, not a vocabulary: every component of every label comes from an enum owned by
    /// `bioprism-atlas` or `bioprism-hub`.
    pub fn label(&self) -> String {
        match self {
            BucketKey::Mechanism { mechanism } => format!("mechanism:{}", mechanism.as_str()),
            BucketKey::Stage { stage } => format!("stage:{}", stage.as_str()),
            BucketKey::Severity { severity } => format!("severity:{}", severity_str(*severity)),
            BucketKey::Inducement { inducement } => {
                format!("inducement:{}", inducement_str(*inducement))
            }
            BucketKey::Component { component } => format!("component:{component}"),
            BucketKey::Unattributed => "component:unattributed".to_string(),
            BucketKey::Contested => "contested".to_string(),
            BucketKey::Unstaged => "stage:unstaged".to_string(),
            BucketKey::Withheld { state } => format!("withheld:{}", state_str(*state)),
        }
    }

    /// Whether this bucket holds records that were withheld rather than diagnosed.
    pub fn is_withheld(&self) -> bool {
        matches!(self, BucketKey::Withheld { .. })
    }
}

fn severity_str(severity: Severity) -> &'static str {
    match severity {
        Severity::Cosmetic => "cosmetic",
        Severity::Degraded => "degraded",
        Severity::WrongConclusion => "wrong_conclusion",
        Severity::UnsafeAction => "unsafe_action",
    }
}

fn inducement_str(inducement: Inducement) -> &'static str {
    match inducement {
        Inducement::TaskInduced => "task_induced",
        Inducement::EnvironmentInduced => "environment_induced",
        Inducement::ModelInduced => "model_induced",
        Inducement::EvaluatorInduced => "evaluator_induced",
    }
}

fn state_str(state: PublicationState) -> &'static str {
    match state {
        PublicationState::Available => "available",
        PublicationState::Unavailable => "unavailable",
        PublicationState::Controlled => "controlled",
        PublicationState::Stale => "stale",
        PublicationState::UnderReview => "under_review",
        PublicationState::Disputed => "disputed",
        PublicationState::Withdrawn => "withdrawn",
        PublicationState::NonReproducible => "non_reproducible",
        PublicationState::NotComparable => "not_comparable",
    }
}

/// One partition cell, holding the identifiers of the failures in it.
///
/// The members are named rather than counted for the reason `bioprism_atlas::CoverageReport::holes`
/// is never elided: 34.09's stated purpose is to "allow developers to reproduce them locally", and
/// a bar you cannot click through to a record is a bar you cannot reproduce.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bucket {
    pub key: BucketKey,
    /// Failure identifiers, in the order the caller supplied them.
    pub members: Vec<String>,
}

impl Bucket {
    pub fn len(&self) -> usize {
        self.members.len()
    }

    pub fn is_empty(&self) -> bool {
        self.members.is_empty()
    }

    /// How this bucket renders.
    ///
    /// A withheld bucket renders as its state, never as a count: publishing "3 under review" for a
    /// diagnosis bucket would leak the diagnosis, and publishing nothing would be the hidden value
    /// the blueprint forbids.
    pub fn cell(&self) -> SurfaceCell {
        match &self.key {
            BucketKey::Withheld { state } => SurfaceCell::withheld(*state),
            _ => SurfaceCell::count(self.members.len()),
        }
    }
}

/// A visibility declaration for one browsed failure.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Visibility {
    pub failure_id: String,
    pub state: PublicationState,
}

impl Visibility {
    pub fn new(failure_id: impl Into<String>, state: PublicationState) -> Self {
        Visibility {
            failure_id: failure_id.into(),
            state,
        }
    }
}

/// Minimized failures, grouped for browsing.
///
/// Private fields with two constructors, both of which read records. There is no way to assemble a
/// browse from bucket counts, for the same reason there is no way to assemble a
/// [`crate::DebtStatement`] from a pair of integers.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct FailureBrowse {
    subject: String,
    facet: Facet,
    taxonomy_version: String,
    buckets: Vec<Bucket>,
    records_browsed: usize,
    withheld: usize,
    contested: usize,
    undiagnosed: usize,
    evaluator_induced: usize,
    distinct_families: usize,
    /// Capability each visible, system-charged failure implicates, with its count. The input to
    /// [`FailureBrowse::rate_against`], kept so the rate is computed from the browsed set rather
    /// than from a second pass over records the caller may have changed.
    charged_by_capability: BTreeMap<String, usize>,
}

/// Groups failures for browsing, with every record visible.
pub fn browse(
    subject: impl Into<String>,
    records: &[FailureRecord],
    facet: Facet,
) -> Result<FailureBrowse, AtlasxError> {
    browse_with_visibility(subject, records, facet, &[])
}

/// Groups failures for browsing, with some records withheld.
///
/// Refuses records spanning two taxonomy versions, a repeated failure identifier, a repeated
/// visibility declaration, and a visibility declaration for a record that is not being browsed. All
/// four are refusals rather than filters: each describes a caller whose inputs disagree with each
/// other, and quietly picking one reading is how a page ends up truthful about a set nobody asked
/// for.
pub fn browse_with_visibility(
    subject: impl Into<String>,
    records: &[FailureRecord],
    facet: Facet,
    visibility: &[Visibility],
) -> Result<FailureBrowse, AtlasxError> {
    let subject = subject.into();

    let mut taxonomy_version: Option<&str> = None;
    let mut ids: BTreeSet<&str> = BTreeSet::new();
    for record in records {
        match taxonomy_version {
            None => taxonomy_version = Some(record.ontology_version.as_str()),
            Some(seen) if seen != record.ontology_version => {
                return Err(AtlasxError::MixedTaxonomyVersions {
                    left: seen.to_string(),
                    right: record.ontology_version.clone(),
                })
            }
            Some(_) => {}
        }
        if !ids.insert(record.failure_id.as_str()) {
            return Err(AtlasxError::DuplicateRecord {
                failure_id: record.failure_id.clone(),
            });
        }
    }

    let mut withheld_states: BTreeMap<&str, PublicationState> = BTreeMap::new();
    for declaration in visibility {
        if !ids.contains(declaration.failure_id.as_str()) {
            return Err(AtlasxError::VisibilityForAbsentRecord {
                failure_id: declaration.failure_id.clone(),
            });
        }
        if withheld_states
            .insert(declaration.failure_id.as_str(), declaration.state)
            .is_some()
        {
            return Err(AtlasxError::DuplicateVisibility {
                failure_id: declaration.failure_id.clone(),
            });
        }
    }

    let mut buckets: BTreeMap<String, Bucket> = BTreeMap::new();
    let mut withheld = 0usize;
    let mut contested = 0usize;
    let mut undiagnosed = 0usize;
    let mut evaluator_induced = 0usize;
    let mut families: BTreeSet<Vec<FailureLabel>> = BTreeSet::new();
    let mut charged_by_capability: BTreeMap<String, usize> = BTreeMap::new();

    for record in records {
        let state = withheld_states
            .get(record.failure_id.as_str())
            .copied()
            .unwrap_or(PublicationState::Available);

        let key = if state == PublicationState::Available {
            if record.labels.is_contested() {
                contested += 1;
            }
            if !record.is_diagnosed() {
                undiagnosed += 1;
            }
            if record.axes.charges_the_system() {
                *charged_by_capability
                    .entry(record.implicates.as_str().to_string())
                    .or_insert(0) += 1;
            } else {
                evaluator_induced += 1;
            }
            families.insert(record.chain.labels());
            facet_key(record, facet)
        } else {
            withheld += 1;
            BucketKey::Withheld { state }
        };

        buckets
            .entry(key.label())
            .or_insert_with(|| Bucket {
                key,
                members: Vec::new(),
            })
            .members
            .push(record.failure_id.clone());
    }

    Ok(FailureBrowse {
        subject,
        facet,
        taxonomy_version: taxonomy_version.unwrap_or_default().to_string(),
        buckets: buckets.into_values().collect(),
        records_browsed: records.len(),
        withheld,
        contested,
        undiagnosed,
        evaluator_induced,
        distinct_families: families.len(),
        charged_by_capability,
    })
}

fn facet_key(record: &FailureRecord, facet: Facet) -> BucketKey {
    match facet {
        Facet::Mechanism => match record.labels.modal() {
            Some(mechanism) => BucketKey::Mechanism { mechanism },
            None => BucketKey::Contested,
        },
        Facet::FirstDivergenceStage => match record.first_divergence_stage() {
            Some(stage) => BucketKey::Stage { stage },
            None => BucketKey::Unstaged,
        },
        Facet::Severity => BucketKey::Severity {
            severity: record.axes.severity,
        },
        Facet::Inducement => BucketKey::Inducement {
            inducement: record.axes.inducement,
        },
        Facet::ArchitectureComponent => match &record.axes.architecture_component {
            Some(component) => BucketKey::Component {
                component: component.clone(),
            },
            None => BucketKey::Unattributed,
        },
    }
}

impl FailureBrowse {
    pub fn facet(&self) -> Facet {
        self.facet
    }

    /// The taxonomy version every browsed record shares. Empty for an empty browse.
    pub fn taxonomy_version(&self) -> &str {
        &self.taxonomy_version
    }

    pub fn buckets(&self) -> &[Bucket] {
        &self.buckets
    }

    pub fn bucket(&self, label: &str) -> Option<&Bucket> {
        self.buckets.iter().find(|b| b.key.label() == label)
    }

    /// Every record given to the browse, withheld ones included. The denominator.
    pub fn records_browsed(&self) -> usize {
        self.records_browsed
    }

    pub fn visible(&self) -> usize {
        self.records_browsed - self.withheld
    }

    pub fn withheld(&self) -> usize {
        self.withheld
    }

    /// Visible records whose reviewers disagree about the mechanism.
    pub fn contested(&self) -> usize {
        self.contested
    }

    /// Visible records that record a failure without localizing it.
    ///
    /// The honest part of 34.09's "failure localization precision", which needs a ground-truth
    /// divergence point no record carries.
    pub fn undiagnosed(&self) -> usize {
        self.undiagnosed
    }

    /// Visible records that are benchmark defects rather than system failures.
    pub fn evaluator_induced(&self) -> usize {
        self.evaluator_induced
    }

    /// Distinct causal chains among the visible records.
    ///
    /// Not a deduplication: no record is dropped and every bucket still names all its members. This
    /// is the count 34.09's "duplicate failure reduction" would be a difference of, and the
    /// difference is not computed because the module states no baseline.
    pub fn distinct_families(&self) -> usize {
        self.distinct_families
    }

    /// Whether the visible shares account for everything browsed.
    ///
    /// False whenever anything is withheld, and that is the disclosure: a page whose bars sum to
    /// less than the total is telling the reader that something is being held back, which a page
    /// that silently shrank its denominator would not.
    pub fn shares_sum_to_one(&self) -> bool {
        self.withheld == 0
    }

    /// The rate 34.09's readers want, available only when a measurement supplies the denominator.
    ///
    /// Refuses when the grid is a reading of a different subject, because borrowing another
    /// system's attempt count is not a denominator, it is a coincidence of units. Refuses when the
    /// capability's cell is a hole, because there is nothing to divide by and zero attempts is not
    /// zero failures.
    ///
    /// The value is **failures per independent unit**, using
    /// `bioprism_metrics::GridCell::effective_size` — clustering units, not trials, per the
    /// stratification discipline section 33 repeats in every one of its modules. It is a
    /// [`SurfaceCell::Score`] and not a [`crate::Share`] because it may legitimately exceed one:
    /// a parent world can fail more than once.
    ///
    /// Only visible, system-charged failures are counted. An evaluator-induced failure is a
    /// benchmark defect and charging it to the system is the error
    /// `bioprism_atlas::FailureAxes::charges_the_system` exists to prevent.
    pub fn rate_against(&self, grid: &CapabilityGrid, capability: &CapabilityId) -> Answer {
        if grid.label != self.subject {
            return Answer::refused(Unanswerable::DifferentSubject);
        }
        let Some(cell) = grid.cell(capability) else {
            return Answer::refused(Unanswerable::EvidenceAbsent);
        };
        let Some(effective_size) = cell.effective_size() else {
            return Answer::refused(Unanswerable::EvidenceAbsent);
        };
        if effective_size == 0 {
            return Answer::refused(Unanswerable::EvidenceAbsent);
        }
        let failures = self
            .charged_by_capability
            .get(capability.as_str())
            .copied()
            .unwrap_or(0);
        Answer::answered(SurfaceCell::Score {
            value: failures as f64 / effective_size as f64,
        })
    }

    /// The largest bucket, when the evidence singles one out.
    fn modal(&self) -> Result<&Bucket, Unanswerable> {
        if self.visible() == 0 {
            return Err(Unanswerable::EvidenceAbsent);
        }
        if self.contested > 0 {
            return Err(Unanswerable::LabelContested);
        }
        let mut best: Option<&Bucket> = None;
        let mut tied = false;
        for bucket in self.buckets.iter().filter(|b| !b.key.is_withheld()) {
            match best {
                None => best = Some(bucket),
                Some(current) if bucket.len() > current.len() => {
                    best = Some(bucket);
                    tied = false;
                }
                Some(current) if bucket.len() == current.len() => tied = true,
                Some(_) => {}
            }
        }
        match best {
            Some(_) if tied => Err(Unanswerable::NotUnique),
            Some(bucket) => Ok(bucket),
            None => Err(Unanswerable::EvidenceAbsent),
        }
    }

    /// Which bucket holds the most visible records, when one does.
    pub fn modal_bucket(&self) -> Result<&BucketKey, Unanswerable> {
        self.modal().map(|bucket| &bucket.key)
    }
}

impl Surface for FailureBrowse {
    fn subject(&self) -> &str {
        &self.subject
    }

    fn declared(&self) -> Vec<Question> {
        let mut questions = vec![Question::ModalBucket];
        for bucket in &self.buckets {
            questions.push(Question::BucketCount {
                bucket: bucket.key.label(),
            });
            questions.push(Question::ShareOfAggregated {
                bucket: bucket.key.label(),
            });
        }
        // Declared and always refused. The question is the one every reader of a failure chart
        // actually has, and a surface that omits it from its declaration has not refused it — it
        // has left the reader to compute it themselves from the bucket counts, which is worse.
        for capability in self.charged_by_capability.keys() {
            questions.push(Question::RatePerAttempt {
                capability: capability.clone(),
            });
        }
        questions
    }

    fn answer(&self, question: &Question) -> Answer {
        match question {
            Question::ModalBucket => match self.modal() {
                Ok(bucket) => Answer::answered(bucket.cell()),
                Err(reason) => Answer::refused(reason),
            },
            Question::BucketCount { bucket } => match self.bucket(bucket) {
                Some(found) => Answer::answered(found.cell()),
                None => Answer::refused(Unanswerable::NoSuchBucket),
            },
            Question::ShareOfAggregated { bucket } => match self.bucket(bucket) {
                Some(found) if found.key.is_withheld() => Answer::answered(found.cell()),
                Some(found) => match SurfaceCell::share(found.len(), self.records_browsed) {
                    Ok(cell) => Answer::answered(cell),
                    Err(_) => Answer::refused(Unanswerable::EvidenceAbsent),
                },
                None => Answer::refused(Unanswerable::NoSuchBucket),
            },
            Question::RatePerAttempt { capability } => {
                if self.charged_by_capability.contains_key(capability) {
                    Answer::refused(Unanswerable::NoAttemptDenominator)
                } else {
                    Answer::refused(Unanswerable::NotDeclared)
                }
            }
            // Coverage is a statement about a grid, standing is a statement about scores, and a
            // comparison is a statement about two subjects. A pile of failures is none of those.
            Question::ProfileCoverage
            | Question::CapabilityStanding { .. }
            | Question::ComparisonWith { .. } => Answer::refused(Unanswerable::NotDeclared),
        }
    }

    fn cells(&self) -> Vec<(String, SurfaceCell)> {
        self.buckets
            .iter()
            .map(|bucket| (bucket.key.label(), bucket.cell()))
            .collect()
    }
}
