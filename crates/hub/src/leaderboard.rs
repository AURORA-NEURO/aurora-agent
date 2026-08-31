//! Leaderboard discipline: comparability as a precondition of ordering.
//!
//! Blueprint 34.15 (Leaderboards, Pareto Fronts and Claim Policy) exists to "permit useful
//! comparison without rewarding unsafe aggregation or marketing overstatement", and its failure
//! states include `not-comparable` alongside a prohibition on replacing states "with zero, empty,
//! or hidden values". 43.43's non-negotiable invariant is blunter: **no claim of universal
//! superiority**. The executive summary adds that instance count is not benchmark count.
//!
//! # The one idea
//!
//! A number is not a rank. A rank is a number *plus the conditions under which it was produced*,
//! and two numbers produced under different conditions do not form an order — not a close one, not
//! a provisional one, none. So [`ComparabilityConditions`] travels with every [`Entry`], and
//! [`rank_order`] returns [`HubError::NotComparable`] rather than an [`Ordering`] whenever the
//! conditions differ, naming every dimension that differs.
//!
//! This is the defensive choice. The alternative — order them anyway and print a footnote — is how
//! every dishonest leaderboard is built, because the ordering is what gets screenshotted and the
//! footnote is what does not.
//!
//! # Ties are real
//!
//! Two entries whose confidence intervals overlap are *tied*, not ordered. 34.15 requires
//! "parent-level intervals" and lists "rank instability" as a product metric; an interval that is
//! rendered but not respected in the sort is decoration. [`RankedBoard`] therefore uses competition
//! ranking, and a tie gives both entries the same rank.
//!
//! # The claim lint is a lint
//!
//! [`lint_claim`] rejects phrasings that assert universal superiority or clinical validity. It
//! cannot prove a claim is honest — a scanner never can — and a submitter determined to overstate
//! will find wording it does not know. What it does is make the *known* overstatements cost
//! something, and make the generated headlines in this module provably free of them, which is
//! tested. Treat a pass as "contains no phrasing we have already seen go wrong", not as approval.
//!
//! # What is not implemented
//!
//! No score computation, no statistics, no Pareto-front geometry, no cost model. Intervals are
//! taken as given. This module decides what may be ordered and what a board may say about it.

use crate::attribution::AccessTier;
use crate::disclosure::{DisclosureLedger, HeadlineLabel};
use crate::error::HubError;
use crate::id::{BoardId, Epoch, SubmissionId};
use crate::moderation::{ModerationLedger, ModerationState};
use crate::submission::{EvidenceScale, VerificationStatus};
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::BTreeMap;

/// The resource envelope a run was allowed. Two systems given different budgets were not asked
/// the same question, so this is part of comparability rather than a display column.
#[derive(Debug, Clone, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct BudgetEnvelope {
    pub max_cost_units: Option<u64>,
    pub max_oracle_calls: Option<u64>,
    pub max_latency_ms: Option<u64>,
}

impl BudgetEnvelope {
    /// Unbounded. Explicit rather than implied, because "no recorded budget" and "no budget" are
    /// different facts and only one of them is comparable to another unbounded run.
    pub fn unbounded() -> BudgetEnvelope {
        BudgetEnvelope::default()
    }

    fn describe(&self) -> String {
        fn part(name: &str, value: Option<u64>) -> String {
            match value {
                Some(v) => format!("{name}={v}"),
                None => format!("{name}=unbounded"),
            }
        }
        format!(
            "{}, {}, {}",
            part("cost", self.max_cost_units),
            part("oracle-calls", self.max_oracle_calls),
            part("latency-ms", self.max_latency_ms)
        )
    }
}

/// Everything that must match before two scores may be ordered.
///
/// The field set is the answer to "what could differ between two runs and change the number".
/// `protocol` is a digest of the scoring procedure itself, so a silent change to the grader makes
/// old and new entries unrankable instead of quietly rebasing the board.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ComparabilityConditions {
    pub pack: ContentHash,
    pub pack_version: String,
    /// Which split: a public split and a hidden holdout of the same pack are different questions.
    pub split: String,
    pub metric: String,
    /// Direction of the metric. Part of the conditions because ordering two entries under opposite
    /// conventions produces a confident, inverted ranking.
    pub higher_is_better: bool,
    pub oracle_tier: String,
    pub access_mode: AccessTier,
    pub budget: BudgetEnvelope,
    pub protocol: ContentHash,
}

/// One dimension in which two entries' conditions disagree.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct ConditionDifference {
    pub dimension: String,
    pub left: String,
    pub right: String,
}

impl ConditionDifference {
    fn new(dimension: &str, left: impl Into<String>, right: impl Into<String>) -> Self {
        ConditionDifference {
            dimension: dimension.to_string(),
            left: left.into(),
            right: right.into(),
        }
    }
}

/// The result of comparing two condition sets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "comparability")]
pub enum Comparability {
    Comparable,
    /// Carries every differing dimension, because "not comparable" with no reason is the same
    /// unexplained refusal 34.16's appeal metric exists to prevent.
    NotComparable {
        differences: Vec<ConditionDifference>,
    },
}

impl ComparabilityConditions {
    /// Every dimension in which `self` and `other` disagree, in a fixed order.
    pub fn differences(&self, other: &ComparabilityConditions) -> Vec<ConditionDifference> {
        let mut out = Vec::new();
        if self.pack != other.pack {
            out.push(ConditionDifference::new(
                "pack",
                self.pack.to_string(),
                other.pack.to_string(),
            ));
        }
        if self.pack_version != other.pack_version {
            out.push(ConditionDifference::new(
                "pack_version",
                &self.pack_version,
                &other.pack_version,
            ));
        }
        if self.split != other.split {
            out.push(ConditionDifference::new("split", &self.split, &other.split));
        }
        if self.metric != other.metric {
            out.push(ConditionDifference::new(
                "metric",
                &self.metric,
                &other.metric,
            ));
        }
        if self.higher_is_better != other.higher_is_better {
            out.push(ConditionDifference::new(
                "higher_is_better",
                self.higher_is_better.to_string(),
                other.higher_is_better.to_string(),
            ));
        }
        if self.oracle_tier != other.oracle_tier {
            out.push(ConditionDifference::new(
                "oracle_tier",
                &self.oracle_tier,
                &other.oracle_tier,
            ));
        }
        if self.access_mode != other.access_mode {
            out.push(ConditionDifference::new(
                "access_mode",
                self.access_mode.as_str(),
                other.access_mode.as_str(),
            ));
        }
        if self.budget != other.budget {
            out.push(ConditionDifference::new(
                "budget",
                self.budget.describe(),
                other.budget.describe(),
            ));
        }
        if self.protocol != other.protocol {
            out.push(ConditionDifference::new(
                "protocol",
                self.protocol.to_string(),
                other.protocol.to_string(),
            ));
        }
        out
    }

    pub fn assess(&self, other: &ComparabilityConditions) -> Comparability {
        let differences = self.differences(other);
        if differences.is_empty() {
            Comparability::Comparable
        } else {
            Comparability::NotComparable { differences }
        }
    }

    /// The sentence a board must print above its ranking. Every rank is relative to this.
    pub fn statement(&self) -> String {
        format!(
            "Ranked on pack {} ({}), split `{}`, metric `{}` ({}), oracle tier `{}`, {} access, \
             budget [{}], scoring protocol {}. Ranks hold under these conditions only.",
            &self.pack.as_str()[..12],
            self.pack_version,
            self.split,
            self.metric,
            if self.higher_is_better {
                "higher is better"
            } else {
                "lower is better"
            },
            self.oracle_tier,
            self.access_mode,
            self.budget.describe(),
            &self.protocol.as_str()[..12],
        )
    }
}

/// A confidence interval on a score.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Interval {
    pub low: f64,
    pub high: f64,
}

impl Interval {
    pub fn overlaps(&self, other: &Interval) -> bool {
        self.low <= other.high && other.low <= self.high
    }
}

/// A reported score.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Score {
    pub value: f64,
    /// Absent means "no interval was reported", which is a weaker claim than a point estimate and
    /// is treated as such: two point estimates order strictly, and any overlap ties.
    pub interval: Option<Interval>,
}

impl Score {
    pub fn point(value: f64) -> Score {
        Score {
            value,
            interval: None,
        }
    }

    pub fn with_interval(value: f64, low: f64, high: f64) -> Score {
        Score {
            value,
            interval: Some(Interval { low, high }),
        }
    }

    fn ties_with(&self, other: &Score) -> bool {
        match (self.interval.as_ref(), other.interval.as_ref()) {
            (Some(a), Some(b)) => a.overlaps(b),
            _ => self.value == other.value,
        }
    }
}

/// One reported result, with the conditions that produced it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Entry {
    pub submission: SubmissionId,
    pub conditions: ComparabilityConditions,
    pub score: Score,
    pub computed_at: Epoch,
    /// The entry's own statement that it knows the pack is public. See [`DisclosureLedger`].
    pub acknowledges_disclosure: bool,
    pub scale: EvidenceScale,
}

/// Order two entries, or refuse.
///
/// [`Ordering::Less`] means `left` ranks **ahead of** `right`, which is the direction a sort
/// wants. Refuses with [`HubError::NotComparable`] whenever the conditions differ in any
/// dimension.
pub fn rank_order(left: &Entry, right: &Entry) -> Result<Ordering, HubError> {
    let differences = left.conditions.differences(&right.conditions);
    if !differences.is_empty() {
        return Err(HubError::NotComparable {
            left: left.submission.to_string(),
            right: right.submission.to_string(),
            differences,
        });
    }
    if left.score.ties_with(&right.score) {
        return Ok(Ordering::Equal);
    }
    let raw = left.score.value.total_cmp(&right.score.value);
    Ok(if left.conditions.higher_is_better {
        raw.reverse()
    } else {
        raw
    })
}

/// Group entries into comparability classes.
///
/// The classes are the boards that could honestly exist. Nothing is merged across classes, and
/// nothing is dropped: an entry alone in its class is a class of one, which is the correct
/// rendering of "we have one result under these conditions".
pub fn partition(entries: &[Entry]) -> Vec<(ComparabilityConditions, Vec<Entry>)> {
    let mut classes: BTreeMap<ComparabilityConditions, Vec<Entry>> = BTreeMap::new();
    for entry in entries {
        classes
            .entry(entry.conditions.clone())
            .or_default()
            .push(entry.clone());
    }
    classes.into_iter().collect()
}

/// Why an entry appears on a board without a rank.
///
/// Present on the board rather than filtered out of it: 34.15 forbids replacing a state with an
/// empty value, and an entry that silently vanishes is exactly that.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "reason")]
pub enum UnrankableReason {
    NotComparable {
        differences: Vec<ConditionDifference>,
    },
    /// Not currently accepted, or not on the ledger at all.
    NotPublished { state: Option<ModerationState> },
    BelowVerificationFloor {
        has: VerificationStatus,
        floor: VerificationStatus,
    },
    /// Refused by the disclosure ledger or the evidence-scale check. Carries the refusal text.
    Ineligible { detail: String },
}

/// An entry the board is showing but will not order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnrankedEntry {
    pub entry: Entry,
    pub reason: UnrankableReason,
}

/// An entry the board will order, with the label its score must be shown under.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RankedEntry {
    /// Competition ranking: tied entries share a rank and the next rank skips.
    pub rank: usize,
    pub entry: Entry,
    pub verification: VerificationStatus,
    pub label: HeadlineLabel,
}

/// A leaderboard: an identifier, one set of conditions, and a verification floor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Board {
    pub id: BoardId,
    pub conditions: ComparabilityConditions,
    /// Entries below this appear unranked with [`UnrankableReason::BelowVerificationFloor`].
    pub min_verification: VerificationStatus,
}

/// The rendered board.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RankedBoard {
    pub board: BoardId,
    pub conditions: ComparabilityConditions,
    pub ranked: Vec<RankedEntry>,
    pub unranked: Vec<UnrankedEntry>,
}

impl Board {
    /// Render the board.
    ///
    /// Takes the moderation and disclosure ledgers because publishability is not a property of an
    /// entry: a withdrawn submission's entry still exists, and a leaked pack's entry still has a
    /// number on it. Deciding here, from the ledgers, is what keeps those two facts from having to
    /// be copied onto the entry and then kept in sync.
    pub fn rank(
        &self,
        entries: &[Entry],
        moderation: &ModerationLedger,
        disclosure: &DisclosureLedger,
    ) -> RankedBoard {
        let mut eligible: Vec<(Entry, VerificationStatus, HeadlineLabel)> = Vec::new();
        let mut unranked: Vec<UnrankedEntry> = Vec::new();

        for entry in entries {
            let differences = self.conditions.differences(&entry.conditions);
            if !differences.is_empty() {
                unranked.push(UnrankedEntry {
                    entry: entry.clone(),
                    reason: UnrankableReason::NotComparable { differences },
                });
                continue;
            }

            let state = moderation.state(&entry.submission);
            if state != Some(ModerationState::Accepted) {
                unranked.push(UnrankedEntry {
                    entry: entry.clone(),
                    reason: UnrankableReason::NotPublished { state },
                });
                continue;
            }

            let verification = moderation
                .verification(&entry.submission)
                .unwrap_or(VerificationStatus::SelfReported);
            if verification < self.min_verification {
                unranked.push(UnrankedEntry {
                    entry: entry.clone(),
                    reason: UnrankableReason::BelowVerificationFloor {
                        has: verification,
                        floor: self.min_verification,
                    },
                });
                continue;
            }

            if let Err(err) = entry.scale.validate() {
                unranked.push(UnrankedEntry {
                    entry: entry.clone(),
                    reason: UnrankableReason::Ineligible {
                        detail: err.to_string(),
                    },
                });
                continue;
            }

            match disclosure.headline_eligibility(
                &entry.conditions.pack,
                entry.computed_at,
                entry.acknowledges_disclosure,
            ) {
                Ok(label) => eligible.push((entry.clone(), verification, label)),
                Err(err) => unranked.push(UnrankedEntry {
                    entry: entry.clone(),
                    reason: UnrankableReason::Ineligible {
                        detail: err.to_string(),
                    },
                }),
            }
        }

        eligible.sort_by(|a, b| {
            rank_order(&a.0, &b.0)
                .unwrap_or(Ordering::Equal)
                .then_with(|| a.0.submission.cmp(&b.0.submission))
        });

        let mut ranked: Vec<RankedEntry> = Vec::with_capacity(eligible.len());
        for (index, (entry, verification, label)) in eligible.iter().enumerate() {
            let rank = match ranked.last() {
                Some(previous)
                    if rank_order(&previous.entry, entry).map(|o| o == Ordering::Equal)
                        == Ok(true) =>
                {
                    previous.rank
                }
                _ => index + 1,
            };
            ranked.push(RankedEntry {
                rank,
                entry: entry.clone(),
                verification: *verification,
                label: label.clone(),
            });
        }

        RankedBoard {
            board: self.id.clone(),
            conditions: self.conditions.clone(),
            ranked,
            unranked,
        }
    }
}

impl RankedBoard {
    /// The only sentence this board is entitled to say.
    ///
    /// Scoped to the conditions, quantified by equivalence classes rather than instance count, and
    /// closed with the nonclaim 43.43 requires. Tested to pass [`lint_claim`].
    pub fn headline(&self) -> String {
        let mut out = self.conditions.statement();
        match self.ranked.first() {
            None => out.push_str(
                " No entry on this board is currently rankable; see the unranked list for the \
                 reason on each.",
            ),
            Some(top) => {
                out.push_str(&format!(
                    " Rank 1 of {} rankable entr(ies): {} ({}), {}.",
                    self.ranked.len(),
                    top.entry.submission,
                    top.verification,
                    top.entry.scale.headline()
                ));
                out.push(' ');
                out.push_str(&top.label.caveat());
            }
        }
        if !self.unranked.is_empty() {
            out.push_str(&format!(
                " {} further entr(ies) are shown without a rank and are not ordered against these.",
                self.unranked.len()
            ));
        }
        out.push_str(
            " This board establishes no superiority outside the stated conditions and no clinical \
             validity.",
        );
        out
    }

    /// Entries sharing rank 1. More than one is a tie, and a tie is a result.
    pub fn leaders(&self) -> Vec<&RankedEntry> {
        self.ranked.iter().filter(|e| e.rank == 1).collect()
    }
}

/// Phrasings a hub will not publish, and the reason each one is refused.
const FORBIDDEN_PHRASES: &[(&str, &str)] = &[
    (
        "state of the art",
        "asserts superiority over systems not evaluated here (43.43)",
    ),
    (
        "state-of-the-art",
        "asserts superiority over systems not evaluated here (43.43)",
    ),
    (
        "sota",
        "asserts superiority over systems not evaluated here (43.43)",
    ),
    (
        "best in class",
        "a class this board did not evaluate (43.43)",
    ),
    (
        "best-in-class",
        "a class this board did not evaluate (43.43)",
    ),
    ("world's best", "no board has world scope (43.43)"),
    (
        "outperforms all",
        "quantifies over systems not evaluated (43.43)",
    ),
    ("beats all", "quantifies over systems not evaluated (43.43)"),
    (
        "superior to all",
        "quantifies over systems not evaluated (43.43)",
    ),
    (
        "universally better",
        "explicit universal-superiority claim (43.43)",
    ),
    (
        "universally superior",
        "explicit universal-superiority claim (43.43)",
    ),
    (
        "no other system",
        "quantifies over systems not evaluated (43.43)",
    ),
    ("unbeatable", "unfalsifiable superiority claim (43.43)"),
    (
        "clinically validated",
        "a research benchmark establishes no clinical validity (36.12)",
    ),
    (
        "clinical grade",
        "a research benchmark establishes no clinical validity (36.12)",
    ),
    (
        "clinical-grade",
        "a research benchmark establishes no clinical validity (36.12)",
    ),
    (
        "ready for clinical use",
        "a research benchmark establishes no clinical validity (36.12)",
    ),
    (
        "diagnostic accuracy in patients",
        "implies patient-level validity (36.12)",
    ),
];

fn contains_phrase(haystack: &str, needle: &str) -> bool {
    let bytes = haystack.as_bytes();
    let mut from = 0usize;
    while let Some(offset) = haystack[from..].find(needle) {
        let start = from + offset;
        let end = start + needle.len();
        let before_ok = start == 0 || !bytes[start - 1].is_ascii_alphanumeric();
        let after_ok = end == bytes.len() || !bytes[end].is_ascii_alphanumeric();
        if before_ok && after_ok {
            return true;
        }
        from = end;
        if from >= haystack.len() {
            break;
        }
    }
    false
}

/// Reject claim text containing a known overstatement.
///
/// A pass means "contains no phrasing already known to go wrong", not "this claim is honest". See
/// the module note; the limitation is the point of documenting it.
pub fn lint_claim(text: &str) -> Result<(), HubError> {
    let lowered = text.to_ascii_lowercase();
    for (phrase, why) in FORBIDDEN_PHRASES {
        if contains_phrase(&lowered, phrase) {
            return Err(HubError::ForbiddenClaimPhrase { phrase, why });
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::disclosure::DisclosureLedger;
    use crate::fixtures::{accepted_submission, submission_with_id};
    use crate::moderation::Decision;

    fn conditions() -> ComparabilityConditions {
        ComparabilityConditions {
            pack: ContentHash::of_bytes(b"pack-a"),
            pack_version: "1.2.0".into(),
            split: "hidden-holdout".into(),
            metric: "first-divergence-rate".into(),
            higher_is_better: false,
            oracle_tier: "deterministic".into(),
            access_mode: AccessTier::Public,
            budget: BudgetEnvelope::unbounded(),
            protocol: ContentHash::of_bytes(b"protocol-a"),
        }
    }

    fn entry(id: &str, value: f64) -> Entry {
        Entry {
            submission: SubmissionId::parse(id).unwrap(),
            conditions: conditions(),
            score: Score::point(value),
            computed_at: Epoch(2),
            acknowledges_disclosure: false,
            scale: EvidenceScale::new(120, 12),
        }
    }

    fn published(ids: &[&str]) -> ModerationLedger {
        let mut ledger = ModerationLedger::new();
        let mut epoch = 1u64;
        for id in ids {
            let submission = if *id == "sub-1" {
                accepted_submission()
            } else {
                submission_with_id(id)
            };
            let sid = submission.id.clone();
            ledger.open(submission, "hub", Epoch(epoch)).unwrap();
            epoch += 1;
            ledger
                .transition(
                    &sid,
                    ModerationState::UnderReview,
                    Decision::by("rev-1", Epoch(epoch)),
                )
                .unwrap();
            epoch += 1;
            ledger
                .transition(
                    &sid,
                    ModerationState::Accepted,
                    Decision::by("rev-1", Epoch(epoch)),
                )
                .unwrap();
            epoch += 1;
        }
        ledger
    }

    fn held_out() -> DisclosureLedger {
        let mut ledger = DisclosureLedger::new();
        ledger.declare_held_out(&conditions().pack).unwrap();
        ledger
    }

    fn board() -> Board {
        Board {
            id: BoardId::parse("first-divergence").unwrap(),
            conditions: conditions(),
            min_verification: VerificationStatus::SelfReported,
        }
    }

    #[test]
    fn two_entries_evaluated_under_different_conditions_are_unrankable() {
        let left = entry("sub-1", 0.10);
        let mut right = entry("sub-2", 0.30);
        right.conditions.split = "public".into();
        right.conditions.budget = BudgetEnvelope {
            max_oracle_calls: Some(50),
            ..BudgetEnvelope::default()
        };

        let err = rank_order(&left, &right).expect_err("different conditions");
        match err {
            HubError::NotComparable { differences, .. } => {
                let dims: Vec<&str> = differences.iter().map(|d| d.dimension.as_str()).collect();
                assert_eq!(dims, vec!["split", "budget"]);
            }
            other => panic!("expected NotComparable, got {other:?}"),
        }
    }

    #[test]
    fn identical_conditions_order_in_the_declared_metric_direction() {
        let better = entry("sub-1", 0.10);
        let worse = entry("sub-2", 0.30);
        assert_eq!(rank_order(&better, &worse), Ok(Ordering::Less));

        let mut higher_better = better.clone();
        let mut higher_worse = worse.clone();
        higher_better.conditions.higher_is_better = true;
        higher_worse.conditions.higher_is_better = true;
        assert_eq!(
            rank_order(&higher_better, &higher_worse),
            Ok(Ordering::Greater)
        );
    }

    #[test]
    fn overlapping_intervals_are_tied_rather_than_ordered() {
        let mut a = entry("sub-1", 0.10);
        let mut b = entry("sub-2", 0.12);
        a.score = Score::with_interval(0.10, 0.08, 0.14);
        b.score = Score::with_interval(0.12, 0.11, 0.16);
        assert_eq!(rank_order(&a, &b), Ok(Ordering::Equal));

        let board = board();
        let ranked = board.rank(&[a, b], &published(&["sub-1", "sub-2"]), &held_out());
        assert_eq!(ranked.ranked.len(), 2);
        assert_eq!(ranked.leaders().len(), 2, "an interval tie is two leaders");
        assert!(ranked.ranked.iter().all(|e| e.rank == 1));
    }

    #[test]
    fn a_board_shows_incomparable_entries_unranked_instead_of_dropping_them() {
        let mut foreign = entry("sub-2", 0.01);
        foreign.conditions.oracle_tier = "probabilistic".into();
        let board = board();
        let ranked = board.rank(
            &[entry("sub-1", 0.10), foreign],
            &published(&["sub-1", "sub-2"]),
            &held_out(),
        );
        assert_eq!(ranked.ranked.len(), 1);
        assert_eq!(ranked.unranked.len(), 1);
        assert!(matches!(
            ranked.unranked[0].reason,
            UnrankableReason::NotComparable { .. }
        ));
        assert_eq!(ranked.leaders()[0].entry.submission.as_str(), "sub-1");
    }

    #[test]
    fn a_withdrawn_submission_leaves_the_ranking_but_stays_on_the_board() {
        let mut moderation = published(&["sub-1", "sub-2"]);
        let withdrawn = SubmissionId::parse("sub-2").unwrap();
        moderation
            .transition(
                &withdrawn,
                ModerationState::Withdrawn,
                Decision::by("lab-a", Epoch(99)).because("consent revoked upstream"),
            )
            .unwrap();

        let ranked = board().rank(
            &[entry("sub-1", 0.30), entry("sub-2", 0.10)],
            &moderation,
            &held_out(),
        );
        assert_eq!(ranked.ranked.len(), 1);
        assert_eq!(
            ranked.unranked[0].reason,
            UnrankableReason::NotPublished {
                state: Some(ModerationState::Withdrawn)
            }
        );
        assert!(moderation.tombstone(&withdrawn).is_some());
    }

    #[test]
    fn an_entry_below_the_verification_floor_is_listed_without_a_rank() {
        let mut board = board();
        board.min_verification = VerificationStatus::Verified;
        let ranked = board.rank(&[entry("sub-1", 0.10)], &published(&["sub-1"]), &held_out());
        assert!(ranked.ranked.is_empty());
        assert_eq!(
            ranked.unranked[0].reason,
            UnrankableReason::BelowVerificationFloor {
                has: VerificationStatus::SelfReported,
                floor: VerificationStatus::Verified,
            }
        );
    }

    #[test]
    fn an_entry_on_a_contaminated_pack_is_never_ranked() {
        use crate::disclosure::{ContaminationKind, ContaminationWitness};
        let mut disclosure = held_out();
        disclosure
            .record_contamination(
                &conditions().pack,
                ContaminationWitness {
                    kind: ContaminationKind::SolutionsPublished,
                    detail: "reference verdicts posted to a public forum".into(),
                    observed_at: Epoch(7),
                    reported_by: "audit-1".into(),
                },
            )
            .unwrap();

        let ranked = board().rank(&[entry("sub-1", 0.10)], &published(&["sub-1"]), &disclosure);
        assert!(ranked.ranked.is_empty());
        match &ranked.unranked[0].reason {
            UnrankableReason::Ineligible { detail } => {
                assert!(detail.contains("contaminated"), "{detail}");
            }
            other => panic!("expected Ineligible, got {other:?}"),
        }
    }

    #[test]
    fn an_entry_reporting_instances_without_classes_is_not_ranked() {
        let mut inflated = entry("sub-1", 0.01);
        inflated.scale = EvidenceScale::new(1_000_000, 0);
        let ranked = board().rank(&[inflated], &published(&["sub-1"]), &held_out());
        assert!(ranked.ranked.is_empty());
        assert!(matches!(
            ranked.unranked[0].reason,
            UnrankableReason::Ineligible { .. }
        ));
    }

    #[test]
    fn partitioning_never_merges_two_comparability_classes() {
        let mut other = entry("sub-2", 0.20);
        other.conditions.pack_version = "1.3.0".into();
        let classes = partition(&[entry("sub-1", 0.10), other, entry("sub-3", 0.15)]);
        assert_eq!(classes.len(), 2);
        let sizes: Vec<usize> = classes.iter().map(|(_, e)| e.len()).collect();
        assert!(sizes.contains(&2) && sizes.contains(&1));
    }

    #[test]
    fn a_generated_headline_passes_its_own_claim_lint() {
        let ranked = board().rank(
            &[entry("sub-1", 0.10), entry("sub-2", 0.30)],
            &published(&["sub-1", "sub-2"]),
            &held_out(),
        );
        let headline = ranked.headline();
        lint_claim(&headline).expect("the hub's own headline must pass its own lint");
        assert!(headline.contains("Ranks hold under these conditions only."));
        assert!(headline.contains("no clinical validity"));
        assert!(headline.contains("independent equivalence class"));
    }

    #[test]
    fn an_empty_board_says_so_rather_than_showing_nothing() {
        let ranked = board().rank(&[], &ModerationLedger::new(), &held_out());
        let headline = ranked.headline();
        assert!(headline.contains("No entry on this board is currently rankable"));
        lint_claim(&headline).unwrap();
    }

    #[test]
    fn universal_superiority_phrasings_are_refused_and_lookalikes_are_not() {
        for text in [
            "Our system is state-of-the-art on BioAtlas.",
            "SOTA on the glioma board.",
            "It outperforms all published architectures.",
            "A clinically validated result.",
        ] {
            assert!(
                lint_claim(text).is_err(),
                "should have been refused: {text}"
            );
        }
        for text in [
            "Rank 1 of 4 under the stated conditions.",
            "Collected at a site in Minnesota.",
            "Best result we obtained under this budget on this split.",
        ] {
            lint_claim(text).unwrap_or_else(|e| panic!("false positive on {text:?}: {e}"));
        }
    }

    #[test]
    fn the_claim_lint_handles_non_ascii_text_without_panicking() {
        lint_claim("Résultats sur le jeu de données du Minnesota — état de l'art non revendiqué.")
            .expect("no forbidden phrase");
        assert!(lint_claim("Modèle SOTA sur BioAtlas").is_err());
        lint_claim("東京のサイトで収集").expect("no forbidden phrase");
    }

    #[test]
    fn a_board_round_trips_through_json() {
        let ranked = board().rank(&[entry("sub-1", 0.10)], &published(&["sub-1"]), &held_out());
        let encoded = serde_json::to_string(&ranked).unwrap();
        let decoded: RankedBoard = serde_json::from_str(&encoded).unwrap();
        assert_eq!(decoded, ranked);
    }
}
