//! Oracle and evaluator review as a record, not a rubber stamp.
//!
//! Implements blueprint 14.09 (Oracle and Evaluator Review) and the one sentence of 14.08 (Pack
//! Review and Acceptance) that is a rule rather than a workflow: *"Approval names dimensions and
//! expiry/review date; it is not universal endorsement."*
//!
//! # What `bioprism-benchcompiler` already settled, and what is left
//!
//! `bioprism_benchcompiler::oracle` makes the gate structural: `ProposedOracle` has no `grade`
//! method, `ReviewedOracle` has exactly one constructor, and review refuses an unnamed reviewer, a
//! proposal with no declared blind spots, a successful exploit, and a lone model judge. That is the
//! *gate*. This module is the *record of the review that passed through it* — a different object
//! with a different failure mode.
//!
//! The failure mode is silence. A review says "approved" and the reader infers that everything
//! 14.09 lists was examined, when in fact the reviewer had no adversarial cases and could not
//! assess exploitability at all. So the central rule here is the workspace's own
//! zero-versus-unknown distinction, applied to review:
//!
//! > **An unreviewed dimension is not a passed one.**
//!
//! [`Finding`] has three arms and no `Default`. [`Finding::NotChecked`] carries the reason it could
//! not be checked, survives into the issued approval, and is reported by
//! [`DimensionScopedApproval::unreviewed`]. There is no method on an approval that answers "is this
//! approved?" with a bare boolean, because the honest answer is always "for which dimension".
//!
//! # Three ways a review is refused rather than weakened
//!
//! 1. **Self-review.** 14.03: "Author cannot provide the only approval for scoring semantics."
//!    [`ReviewError::SelfReview`] compares the reviewer's identity against the packet's author, and
//!    [`ReviewError::NotIndependent`] rejects a reviewer whose declared role is
//!    [`Role::Author`] regardless of whose work it is.
//! 2. **A pass with no corpus behind it.** 14.09's test corpus lists nine case classes, and each
//!    review question depends on particular ones — you cannot conclude that reward is unobtainable
//!    without intent if the corpus contains no exploit attempts. [`ReviewDimension::supported_by`]
//!    is that dependency, and recording a `Passed` finding for a dimension whose supporting class
//!    is absent is [`ReviewError::PassedWithoutCorpus`] rather than a warning.
//! 3. **A mandatory dimension left unchecked.** [`ReviewDimension::mandatory_for`] returns the
//!    dimensions that must carry a finding, and it returns *more of them* for a nondeterministic
//!    evaluator: 14.09's judge-calibration paragraph — blinded labelled set, sensitivity and
//!    specificity, subgroup and architecture bias, repeated stability, adversarial injection —
//!    applies exactly when the evaluator is not deterministic.
//!
//! # Scoring semantics change by revision, not in place
//!
//! 14.09: *"Any scoring-semantic change creates a new evaluator revision and impact analysis.
//! Historical results are not silently recomputed under new meaning."* [`EvaluatorRevision`]
//! carries a `bioprism_ids::ContentHash` over the scoring semantics separately from the revision's
//! own version, and [`EvaluatorRevision::supersede`] runs the claimed version bump through
//! `bioprism_governance`, which already owns the rule that a change moving a digest is
//! [`bioprism_governance::CompatibilityClass::Breaking`]. A scoring change that ships as a minor
//! bump is [`ReviewError::ScoringSemanticsUnderVersioned`].
//!
//! The same digest is what makes an approval go stale without a clock:
//! [`DimensionScopedApproval::carries_to`] refuses to apply an approval to a revision whose scoring
//! digest differs. This is `bioprism-registry`'s core-digest mechanism — accumulating evidence about
//! an artifact must not invalidate the evidence already attached to it, while a change to the
//! *content* invalidates every review of it at once — transposed from packs to evaluators.
//!
//! # Not implemented
//!
//! No review workflow: nothing assigns a reviewer, tracks a queue, sends a notification, or expires
//! a review on a date. 14.08 asks for an expiry date; this crate has no clock, so staleness is
//! structural (the digest moved) rather than temporal. No sampling design for large generated packs
//! — 14.08 wants human samples "concentrated on mutation boundaries", which needs the mutation
//! lineage `bioprism-mutation` holds. No signatures on the review record. No drift monitoring:
//! 14.09's sentinel agents need a running evaluator, and nothing here executes one.

use crate::error::ReviewError;
use crate::id::{Actor, ActorId};
use bioprism_governance::{SchemaVersion, VersionBump};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

/// The nine case classes of 14.09's test corpus, verbatim from the module.
///
/// Closed, because the point of the enumeration is that a review can be asked which of them it had.
/// An open corpus — a bag of strings — would make [`ReviewError::PassedWithoutCorpus`] unenforceable,
/// since any dimension could claim support from a case class nobody defined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CorpusClass {
    ReferenceSolutions,
    AlternateValidSolutions,
    PartialProgress,
    PlausibleWrongOutputs,
    ExploitAttempts,
    MalformedStates,
    Timeouts,
    UnsafeActions,
    EvaluatorEdgeCases,
}

impl CorpusClass {
    pub const ALL: [CorpusClass; 9] = [
        CorpusClass::ReferenceSolutions,
        CorpusClass::AlternateValidSolutions,
        CorpusClass::PartialProgress,
        CorpusClass::PlausibleWrongOutputs,
        CorpusClass::ExploitAttempts,
        CorpusClass::MalformedStates,
        CorpusClass::Timeouts,
        CorpusClass::UnsafeActions,
        CorpusClass::EvaluatorEdgeCases,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            CorpusClass::ReferenceSolutions => "reference_solutions",
            CorpusClass::AlternateValidSolutions => "alternate_valid_solutions",
            CorpusClass::PartialProgress => "partial_progress",
            CorpusClass::PlausibleWrongOutputs => "plausible_wrong_outputs",
            CorpusClass::ExploitAttempts => "exploit_attempts",
            CorpusClass::MalformedStates => "malformed_states",
            CorpusClass::Timeouts => "timeouts",
            CorpusClass::UnsafeActions => "unsafe_actions",
            CorpusClass::EvaluatorEdgeCases => "evaluator_edge_cases",
        }
    }
}

impl fmt::Display for CorpusClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What the reviewer actually had to look at.
///
/// A count per class rather than the cases themselves: this crate governs the review, it does not
/// hold the benchmark. A class present with a count of zero is treated as absent, because "we
/// created the category and put nothing in it" is not evidence.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TestCorpus {
    counts: BTreeMap<CorpusClass, usize>,
}

impl TestCorpus {
    pub fn new() -> Self {
        TestCorpus::default()
    }

    /// Add `count` cases of `class`. Adding zero is a no-op rather than a declaration.
    #[must_use]
    pub fn with(mut self, class: CorpusClass, count: usize) -> Self {
        if count > 0 {
            *self.counts.entry(class).or_insert(0) += count;
        }
        self
    }

    pub fn count(&self, class: CorpusClass) -> usize {
        self.counts.get(&class).copied().unwrap_or(0)
    }

    pub fn holds(&self, class: CorpusClass) -> bool {
        self.count(class) > 0
    }

    pub fn is_empty(&self) -> bool {
        self.counts.values().all(|n| *n == 0)
    }

    pub fn total(&self) -> usize {
        self.counts.values().sum()
    }

    /// The classes 14.09 names that this corpus does not hold.
    pub fn gaps(&self) -> Vec<CorpusClass> {
        CorpusClass::ALL
            .into_iter()
            .filter(|c| !self.holds(*c))
            .collect()
    }
}

/// The questions 14.09 tells a reviewer to answer, plus its judge-calibration axes.
///
/// The first six come from the module's "Review questions" paragraph, one dimension per question.
/// The last four come from "Judge calibration" and apply only to a nondeterministic evaluator —
/// asking whether a deterministic state predicate is stable under repetition is not a finding, it
/// is a tautology.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewDimension {
    /// "What exact property is measured?"
    MeasuredProperty,
    /// "Can a task be solved legitimately outside the reference path?"
    AlternateSolutionPaths,
    /// "Can reward be obtained without intent?"
    RewardWithoutIntent,
    /// "What evidence is hidden?"
    HiddenEvidence,
    /// "What is unknown?" — the dimension whose honest answer is usually the most useful one.
    UnknownCases,
    /// "What fails safely?"
    FailureSafety,
    /// Blinded human-labelled set; sensitivity and specificity.
    JudgeCalibration,
    /// Subgroup and architecture bias.
    SubgroupBias,
    /// Repeated stability and abstention behaviour.
    RepeatedStability,
    /// Adversarial injection into the judge.
    AdversarialInjection,
}

impl ReviewDimension {
    pub const ALL: [ReviewDimension; 10] = [
        ReviewDimension::MeasuredProperty,
        ReviewDimension::AlternateSolutionPaths,
        ReviewDimension::RewardWithoutIntent,
        ReviewDimension::HiddenEvidence,
        ReviewDimension::UnknownCases,
        ReviewDimension::FailureSafety,
        ReviewDimension::JudgeCalibration,
        ReviewDimension::SubgroupBias,
        ReviewDimension::RepeatedStability,
        ReviewDimension::AdversarialInjection,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            ReviewDimension::MeasuredProperty => "measured_property",
            ReviewDimension::AlternateSolutionPaths => "alternate_solution_paths",
            ReviewDimension::RewardWithoutIntent => "reward_without_intent",
            ReviewDimension::HiddenEvidence => "hidden_evidence",
            ReviewDimension::UnknownCases => "unknown_cases",
            ReviewDimension::FailureSafety => "failure_safety",
            ReviewDimension::JudgeCalibration => "judge_calibration",
            ReviewDimension::SubgroupBias => "subgroup_bias",
            ReviewDimension::RepeatedStability => "repeated_stability",
            ReviewDimension::AdversarialInjection => "adversarial_injection",
        }
    }

    /// The corpus class a `Passed` finding on this dimension requires.
    ///
    /// `None` means the dimension is answerable from the evaluator's own specification — what
    /// property is measured is a question about the contract, not about cases.
    pub const fn supported_by(self) -> Option<CorpusClass> {
        match self {
            ReviewDimension::MeasuredProperty => None,
            ReviewDimension::AlternateSolutionPaths => Some(CorpusClass::AlternateValidSolutions),
            ReviewDimension::RewardWithoutIntent => Some(CorpusClass::ExploitAttempts),
            ReviewDimension::HiddenEvidence => None,
            ReviewDimension::UnknownCases => Some(CorpusClass::EvaluatorEdgeCases),
            ReviewDimension::FailureSafety => Some(CorpusClass::MalformedStates),
            ReviewDimension::JudgeCalibration => Some(CorpusClass::PlausibleWrongOutputs),
            ReviewDimension::SubgroupBias => Some(CorpusClass::ReferenceSolutions),
            ReviewDimension::RepeatedStability => Some(CorpusClass::PartialProgress),
            ReviewDimension::AdversarialInjection => Some(CorpusClass::ExploitAttempts),
        }
    }

    /// Whether this dimension only makes sense for a nondeterministic evaluator.
    pub const fn calibration_only(self) -> bool {
        matches!(
            self,
            ReviewDimension::JudgeCalibration
                | ReviewDimension::SubgroupBias
                | ReviewDimension::RepeatedStability
                | ReviewDimension::AdversarialInjection
        )
    }

    /// The dimensions that must carry a finding for an approval to issue.
    ///
    /// All six review questions are mandatory in both cases; the four calibration axes become
    /// mandatory exactly when the evaluator is nondeterministic. 14.09's priority paragraph is the
    /// reason: a model judge "never overrides hard failure without an explicit conflict rule", so
    /// an uncalibrated judge is not merely under-reviewed, it is authorised to overrule evidence.
    pub fn mandatory_for(nondeterministic: bool) -> Vec<ReviewDimension> {
        ReviewDimension::ALL
            .into_iter()
            .filter(|d| nondeterministic || !d.calibration_only())
            .collect()
    }
}

impl fmt::Display for ReviewDimension {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// What a reviewer concluded about one dimension.
///
/// Three arms, no `Default`, and no ordering that would let a caller take a maximum. The absence of
/// a fourth arm meaning "probably fine" is deliberate; so is the absence of `impl From<bool>`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "finding", rename_all = "snake_case")]
pub enum Finding {
    /// Examined against the corpus and found sound. The evidence sentence is what a later reader
    /// checks; an empty one is refused at [`ReviewRecord::conclude`].
    Passed { evidence: String },
    /// Examined and found defective.
    Failed { defect: String },
    /// Not examined, and the reason. This is a first-class outcome, not a missing entry: a review
    /// that says "I could not test injection because the judge is behind an API I cannot script"
    /// is more useful than one that omits the dimension.
    NotChecked { why: String },
}

impl Finding {
    pub fn passed(evidence: impl Into<String>) -> Self {
        Finding::Passed {
            evidence: evidence.into(),
        }
    }

    pub fn failed(defect: impl Into<String>) -> Self {
        Finding::Failed {
            defect: defect.into(),
        }
    }

    pub fn not_checked(why: impl Into<String>) -> Self {
        Finding::NotChecked { why: why.into() }
    }

    pub const fn is_pass(&self) -> bool {
        matches!(self, Finding::Passed { .. })
    }
}

/// One version of an evaluator, identified by what it scores rather than by what it is called.
///
/// `scoring_digest` is over the scoring semantics alone. Keeping it separate from the revision's
/// version string is what lets an approval go stale precisely when meaning changed, and not when
/// somebody reformatted the source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvaluatorRevision {
    pub evaluator: String,
    pub version: SchemaVersion,
    pub scoring_digest: ContentHash,
    /// Whether grading involves a model judge, a sampled statistic, or anything else that can
    /// return two answers for one input.
    pub nondeterministic: bool,
}

impl EvaluatorRevision {
    pub fn new(
        evaluator: impl Into<String>,
        version: SchemaVersion,
        scoring_digest: ContentHash,
        nondeterministic: bool,
    ) -> Self {
        EvaluatorRevision {
            evaluator: evaluator.into(),
            version,
            scoring_digest,
            nondeterministic,
        }
    }

    /// Declare `next` to be the successor of this revision after a scoring-semantic change.
    ///
    /// Two refusals, and both are 14.09's sentence rather than this crate's opinion. A successor
    /// that carries the same scoring digest did not change scoring semantics
    /// ([`ReviewError::RevisionWithoutChange`]). A successor whose scoring digest moved but whose
    /// version did not take a major bump is under-versioned
    /// ([`ReviewError::ScoringSemanticsUnderVersioned`]) — the required bump is
    /// `bioprism_governance::CompatibilityClass::Breaking`'s, because a change that moves a digest
    /// is what that crate already defines as breaking.
    pub fn supersede(&self, next: &EvaluatorRevision) -> Result<(), ReviewError> {
        if next.scoring_digest == self.scoring_digest {
            return Err(ReviewError::RevisionWithoutChange {
                revision: next.version.to_string(),
            });
        }
        let observed = bioprism_governance::observed_bump(&self.version, &next.version);
        if observed != VersionBump::Major {
            return Err(ReviewError::ScoringSemanticsUnderVersioned {
                from: self.version.to_string(),
                to: next.version.to_string(),
                observed: format!("{observed:?}").to_lowercase(),
                required: "major".into(),
            });
        }
        Ok(())
    }
}

/// The review as submitted: what was reviewed, by whom, against what, with what findings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewRecord {
    pub revision: EvaluatorRevision,
    pub author: Actor,
    pub reviewer: Actor,
    pub corpus: TestCorpus,
    findings: BTreeMap<ReviewDimension, Finding>,
}

impl ReviewRecord {
    pub fn new(revision: EvaluatorRevision, author: Actor, reviewer: Actor) -> Self {
        ReviewRecord {
            revision,
            author,
            reviewer,
            corpus: TestCorpus::new(),
            findings: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn against(mut self, corpus: TestCorpus) -> Self {
        self.corpus = corpus;
        self
    }

    /// Record a finding. A dimension may be recorded once; a second attempt is
    /// [`ReviewError::DuplicateDimension`] rather than an overwrite, so a record cannot be walked
    /// back into a pass.
    pub fn finding(
        mut self,
        dimension: ReviewDimension,
        finding: Finding,
    ) -> Result<Self, ReviewError> {
        if self.findings.contains_key(&dimension) {
            return Err(ReviewError::DuplicateDimension {
                dimension: dimension.as_str(),
            });
        }
        self.findings.insert(dimension, finding);
        Ok(self)
    }

    pub fn finding_for(&self, dimension: ReviewDimension) -> Option<&Finding> {
        self.findings.get(&dimension)
    }

    /// Issue the approval, or say which rule stopped it.
    ///
    /// The order of checks is the order of severity, so the error a caller sees is the worst thing
    /// wrong with the review rather than the first thing alphabetically.
    pub fn conclude(self) -> Result<DimensionScopedApproval, ReviewError> {
        if self.reviewer.id == self.author.id {
            return Err(ReviewError::SelfReview {
                reviewer: self.reviewer.id,
            });
        }
        if !self.reviewer.role.may_approve_alone() {
            return Err(ReviewError::NotIndependent {
                role: self.reviewer.role,
            });
        }
        if self.corpus.is_empty() {
            return Err(ReviewError::EmptyCorpus);
        }

        for dimension in ReviewDimension::ALL {
            if let Some(Finding::Failed { defect }) = self.findings.get(&dimension) {
                return Err(ReviewError::DimensionFailed {
                    dimension: dimension.as_str(),
                    defect: defect.clone(),
                });
            }
        }

        for dimension in ReviewDimension::ALL {
            let Some(Finding::Passed { evidence }) = self.findings.get(&dimension) else {
                continue;
            };
            if evidence.trim().is_empty() {
                return Err(ReviewError::MandatoryDimensionNotChecked {
                    dimension: dimension.as_str(),
                    why: "recorded as passed with no evidence sentence".into(),
                });
            }
            if let Some(class) = dimension.supported_by() {
                if !self.corpus.holds(class) {
                    return Err(ReviewError::PassedWithoutCorpus {
                        dimension: dimension.as_str(),
                        missing: class.as_str(),
                    });
                }
            }
        }

        for dimension in ReviewDimension::mandatory_for(self.revision.nondeterministic) {
            match self.findings.get(&dimension) {
                Some(Finding::Passed { .. }) => {}
                Some(Finding::NotChecked { why }) => {
                    return Err(ReviewError::MandatoryDimensionNotChecked {
                        dimension: dimension.as_str(),
                        why: why.clone(),
                    })
                }
                Some(Finding::Failed { .. }) => unreachable!("failures rejected above"),
                None => {
                    return Err(ReviewError::MandatoryDimensionNotChecked {
                        dimension: dimension.as_str(),
                        why: "no finding recorded".into(),
                    })
                }
            }
        }

        let passed: BTreeSet<ReviewDimension> = self
            .findings
            .iter()
            .filter(|(_, f)| f.is_pass())
            .map(|(d, _)| *d)
            .collect();
        let unreviewed: BTreeMap<ReviewDimension, String> = ReviewDimension::ALL
            .into_iter()
            .filter_map(|d| match self.findings.get(&d) {
                Some(Finding::NotChecked { why }) => Some((d, why.clone())),
                None => Some((d, "no finding recorded".to_string())),
                _ => None,
            })
            .collect();

        Ok(DimensionScopedApproval {
            evaluator: self.revision.evaluator,
            version: self.revision.version,
            scoring_digest: self.revision.scoring_digest,
            reviewer: self.reviewer.id,
            corpus: self.corpus,
            passed,
            unreviewed,
        })
    }
}

/// An approval that names its dimensions, per 14.08, and cannot be read as more than it is.
///
/// Private fields, one constructor ([`ReviewRecord::conclude`]), `Serialize` but **not**
/// `Deserialize`. The pattern is `bioprism_lab::CleanMeasurement`'s and
/// `bioprism_benchcompiler::ReviewedOracle`'s, and the reason is the same: an approval that could be
/// parsed out of JSON is an approval anybody can write, and the checks above become decorative.
///
/// There is deliberately no `is_approved()`. The questions this type answers are
/// [`DimensionScopedApproval::covers`] and [`DimensionScopedApproval::unreviewed`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DimensionScopedApproval {
    evaluator: String,
    version: SchemaVersion,
    scoring_digest: ContentHash,
    reviewer: ActorId,
    corpus: TestCorpus,
    passed: BTreeSet<ReviewDimension>,
    unreviewed: BTreeMap<ReviewDimension, String>,
}

impl DimensionScopedApproval {
    pub fn evaluator(&self) -> &str {
        &self.evaluator
    }

    pub fn version(&self) -> &SchemaVersion {
        &self.version
    }

    pub fn reviewer(&self) -> &ActorId {
        &self.reviewer
    }

    pub fn corpus(&self) -> &TestCorpus {
        &self.corpus
    }

    /// Whether this approval extends to `dimension`. The only affirmative question it answers.
    pub fn covers(&self, dimension: ReviewDimension) -> bool {
        self.passed.contains(&dimension)
    }

    pub fn covered(&self) -> Vec<ReviewDimension> {
        self.passed.iter().copied().collect()
    }

    /// The dimensions this approval says nothing about, and why. Never empty for a deterministic
    /// evaluator, because the four calibration axes are not mandatory there and therefore normally
    /// go unchecked — which is a fact about the approval a reader is entitled to.
    pub fn unreviewed(&self) -> Vec<(ReviewDimension, &str)> {
        self.unreviewed
            .iter()
            .map(|(d, why)| (*d, why.as_str()))
            .collect()
    }

    /// Whether this approval applies to `revision`.
    ///
    /// False as soon as the scoring digest moves, which is 14.09's "historical results are not
    /// silently recomputed under new meaning" read forwards: the approval does not follow the
    /// evaluator into its next revision, so the next revision needs its own review.
    pub fn carries_to(&self, revision: &EvaluatorRevision) -> Result<(), ReviewError> {
        if revision.scoring_digest != self.scoring_digest {
            return Err(ReviewError::ApprovalDoesNotCarry {
                approved: self.scoring_digest.as_str().to_string(),
                current: revision.scoring_digest.as_str().to_string(),
            });
        }
        Ok(())
    }
}

impl fmt::Display for DimensionScopedApproval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} reviewed by {} for {} of {} dimensions; {} unreviewed",
            self.evaluator,
            self.version,
            self.reviewer,
            self.passed.len(),
            ReviewDimension::ALL.len(),
            self.unreviewed.len()
        )
    }
}

/// Convenience for a reviewer who checked everything mandatory against a full corpus.
///
/// Present because the tests and callers need one, and absent from the doc examples on purpose: a
/// helper that fills in evidence sentences would be a helper for writing reviews nobody performed.
pub fn full_corpus(cases_per_class: usize) -> TestCorpus {
    CorpusClass::ALL
        .into_iter()
        .fold(TestCorpus::new(), |c, class| c.with(class, cases_per_class))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::id::Role;

    fn revision(nondeterministic: bool) -> EvaluatorRevision {
        EvaluatorRevision::new(
            "structural-diff-oracle",
            SchemaVersion::parse("1.0.0").unwrap(),
            ContentHash::of_bytes(b"scoring-v1"),
            nondeterministic,
        )
    }

    fn complete(record: ReviewRecord, nondeterministic: bool) -> ReviewRecord {
        let mut record = record;
        for dimension in ReviewDimension::mandatory_for(nondeterministic) {
            record = record
                .finding(dimension, Finding::passed("checked against the corpus"))
                .unwrap();
        }
        record
    }

    fn record(nondeterministic: bool) -> ReviewRecord {
        ReviewRecord::new(
            revision(nondeterministic),
            Actor::author("pack-author"),
            Actor::independent_reviewer("reviewer-1"),
        )
        .against(full_corpus(3))
    }

    #[test]
    fn an_unreviewed_dimension_is_not_a_passed_one() {
        let approval = complete(record(false), false).conclude().unwrap();
        assert!(approval.covers(ReviewDimension::RewardWithoutIntent));
        assert!(!approval.covers(ReviewDimension::AdversarialInjection));
        let unreviewed: Vec<_> = approval.unreviewed().into_iter().map(|(d, _)| d).collect();
        assert!(unreviewed.contains(&ReviewDimension::AdversarialInjection));
    }

    #[test]
    fn an_author_cannot_review_their_own_evaluator() {
        let record = ReviewRecord::new(
            revision(false),
            Actor::author("same-person"),
            Actor::independent_reviewer("same-person"),
        )
        .against(full_corpus(1));
        assert!(matches!(
            complete(record, false).conclude(),
            Err(ReviewError::SelfReview { .. })
        ));
    }

    #[test]
    fn a_reviewer_acting_as_author_cannot_approve_alone() {
        let record = ReviewRecord::new(
            revision(false),
            Actor::author("author-1"),
            Actor::author("author-2"),
        )
        .against(full_corpus(1));
        assert!(matches!(
            complete(record, false).conclude(),
            Err(ReviewError::NotIndependent { role: Role::Author })
        ));
    }

    #[test]
    fn a_dimension_cannot_pass_without_corpus_material_behind_it() {
        let corpus = full_corpus(2);
        let corpus = CorpusClass::ALL
            .into_iter()
            .filter(|c| *c != CorpusClass::ExploitAttempts)
            .fold(TestCorpus::new(), |acc, c| acc.with(c, corpus.count(c)));
        let record = complete(record(false).against(corpus), false);
        assert!(matches!(
            record.conclude(),
            Err(ReviewError::PassedWithoutCorpus {
                dimension: "reward_without_intent",
                missing: "exploit_attempts"
            })
        ));
    }

    #[test]
    fn a_nondeterministic_evaluator_must_carry_the_four_calibration_findings() {
        let deterministic_only = complete(record(true), false);
        assert!(matches!(
            deterministic_only.conclude(),
            Err(ReviewError::MandatoryDimensionNotChecked { .. })
        ));
        let calibrated = complete(record(true), true);
        assert!(calibrated.conclude().is_ok());
    }

    #[test]
    fn a_deterministic_evaluator_does_not_need_judge_calibration() {
        assert_eq!(ReviewDimension::mandatory_for(false).len(), 6);
        assert_eq!(ReviewDimension::mandatory_for(true).len(), 10);
    }

    #[test]
    fn a_failed_dimension_refuses_the_whole_approval() {
        let record = complete(record(false), false);
        let record = ReviewRecord::new(
            record.revision.clone(),
            record.author.clone(),
            record.reviewer.clone(),
        )
        .against(full_corpus(2))
        .finding(
            ReviewDimension::FailureSafety,
            Finding::failed("a malformed state scores as a pass"),
        )
        .unwrap();
        assert!(matches!(
            record.conclude(),
            Err(ReviewError::DimensionFailed {
                dimension: "failure_safety",
                ..
            })
        ));
    }

    #[test]
    fn a_review_with_an_empty_corpus_is_refused() {
        let record = complete(
            ReviewRecord::new(
                revision(false),
                Actor::author("a"),
                Actor::independent_reviewer("b"),
            ),
            false,
        );
        assert!(matches!(record.conclude(), Err(ReviewError::EmptyCorpus)));
    }

    #[test]
    fn a_dimension_recorded_twice_is_refused_rather_than_overwritten() {
        let record = record(false)
            .finding(ReviewDimension::UnknownCases, Finding::failed("bad"))
            .unwrap();
        assert!(matches!(
            record.finding(ReviewDimension::UnknownCases, Finding::passed("good")),
            Err(ReviewError::DuplicateDimension { .. })
        ));
    }

    #[test]
    fn a_pass_with_no_evidence_sentence_is_not_a_pass() {
        let mut record = record(false);
        for dimension in ReviewDimension::mandatory_for(false) {
            let finding = if dimension == ReviewDimension::HiddenEvidence {
                Finding::passed("   ")
            } else {
                Finding::passed("checked")
            };
            record = record.finding(dimension, finding).unwrap();
        }
        assert!(matches!(
            record.conclude(),
            Err(ReviewError::MandatoryDimensionNotChecked {
                dimension: "hidden_evidence",
                ..
            })
        ));
    }

    #[test]
    fn an_approval_does_not_carry_to_a_revision_whose_scoring_digest_moved() {
        let approval = complete(record(false), false).conclude().unwrap();
        assert!(approval.carries_to(&revision(false)).is_ok());
        let moved = EvaluatorRevision::new(
            "structural-diff-oracle",
            SchemaVersion::parse("2.0.0").unwrap(),
            ContentHash::of_bytes(b"scoring-v2"),
            false,
        );
        assert!(matches!(
            approval.carries_to(&moved),
            Err(ReviewError::ApprovalDoesNotCarry { .. })
        ));
    }

    #[test]
    fn a_scoring_change_shipped_as_a_minor_bump_is_refused() {
        let v1 = revision(false);
        let minor = EvaluatorRevision::new(
            "structural-diff-oracle",
            SchemaVersion::parse("1.1.0").unwrap(),
            ContentHash::of_bytes(b"scoring-v2"),
            false,
        );
        assert!(matches!(
            v1.supersede(&minor),
            Err(ReviewError::ScoringSemanticsUnderVersioned { .. })
        ));
        let major = EvaluatorRevision::new(
            "structural-diff-oracle",
            SchemaVersion::parse("2.0.0").unwrap(),
            ContentHash::of_bytes(b"scoring-v2"),
            false,
        );
        assert!(v1.supersede(&major).is_ok());
    }

    #[test]
    fn a_successor_that_kept_the_scoring_digest_did_not_change_scoring() {
        let v1 = revision(false);
        let renamed = EvaluatorRevision::new(
            "structural-diff-oracle",
            SchemaVersion::parse("2.0.0").unwrap(),
            ContentHash::of_bytes(b"scoring-v1"),
            false,
        );
        assert!(matches!(
            v1.supersede(&renamed),
            Err(ReviewError::RevisionWithoutChange { .. })
        ));
    }

    #[test]
    fn an_approval_cannot_be_deserialised_back_into_existence() {
        let approval = complete(record(false), false).conclude().unwrap();
        let text = serde_json::to_string(&approval).unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert!(value.get("passed").is_some());
        assert!(value.get("unreviewed").is_some());
    }

    #[test]
    fn a_corpus_class_declared_with_zero_cases_is_absent() {
        let corpus = TestCorpus::new().with(CorpusClass::ExploitAttempts, 0);
        assert!(!corpus.holds(CorpusClass::ExploitAttempts));
        assert!(corpus.is_empty());
        assert_eq!(corpus.gaps().len(), 9);
    }
}
