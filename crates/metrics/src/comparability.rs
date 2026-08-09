//! Whether two scores may be set side by side at all.
//!
//! `bioprism-standards` asks this question about physical measurements and answers it with
//! `comparable(a, b) -> Result<(), Incomparability>` returning the *first* blocking dimension. This
//! module is the same shape for capability scores, and the shape is reused deliberately: a reader
//! who has learned to read one refusal reads the other, and the first-blocking discipline is right
//! for the same reason it is right there — a caller told "the pack versions differ" has one thing
//! to fix, while a caller handed six simultaneous complaints has to guess which one matters, and
//! the later checks are frequently meaningless until the earlier one is resolved.
//!
//! # The one place this diverges from `bioprism-standards`, and why
//!
//! There, two measurements that are both unbound to an ontology pass by default: an uncoded
//! millimetre is still a millimetre. Here, **silence does not match silence**. Two scores that each
//! failed to record a pack version are not thereby known to share one, and the default that says
//! otherwise is the single cheapest way to publish a comparison between systems that were never
//! evaluated on the same thing. So [`Condition::Unrecorded`] blocks on either side, and
//! [`ScoreIncomparability::ConditionUnrecorded`] names which side was silent.
//!
//! A caller with out-of-band knowledge — "both of these ran on pack 4, we simply did not stamp it"
//! — can waive a named dimension through [`ComparabilityPolicy`]. The waiver then appears in
//! [`ComparisonReport::waived`], so the assumption is printed rather than assumed. That is the same
//! trade this crate makes everywhere: an arbitrary step is allowed when it is recorded.
//!
//! # Not implemented
//!
//! No reconciliation. `bioprism-standards` can convert millimetres to centimetres and hand back the
//! `ConversionRecord`; there is no analogous act for capability scores. Rescoring a pack under a
//! different oracle floor is a re-evaluation, not a conversion, and offering
//! `reconcile(pack_3, pack_4)` would imply a mapping between evaluation regimes that nobody has
//! justified.

use crate::aggregate::CoveredAggregate;
use crate::conditions::{compare_condition, Condition, MeasurementConditions};
use crate::error::{MetricsError, ScoreIncomparability};
use crate::grid::CapabilityGrid;
use bioprism_ids::ContentHash;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// The dimensions checked, in the order they are checked.
///
/// Subject first because the commonest illegitimate comparison is not two systems under different
/// packs but two different questions on one axis. Scoring rule second because a disagreeing
/// direction inverts the meaning of every later check. Ontology version before pack version because
/// a capability identifier drawn from a different vocabulary may not denote the same capability, so
/// "the packs agree" would be a statement about two different things.
pub const CHECK_ORDER: &[&str] = &[
    "subject",
    "scoring rule",
    "ontology version",
    "pack version",
    "evidence base",
    "oracle floor",
    "budget",
    "stratification key",
];

/// Which checks a caller has explicitly waived.
///
/// A waiver is a claim the caller is making on the record — "these were the same, we just did not
/// stamp it" — not a relaxation of the rule. It appears in every report it affects.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ComparabilityPolicy {
    waived: BTreeSet<String>,
}

impl ComparabilityPolicy {
    /// Nothing waived. The correct default and the one [`comparable`] uses.
    pub fn strict() -> Self {
        ComparabilityPolicy::default()
    }

    /// Waives one dimension by name. Names come from [`CHECK_ORDER`] or from a stratification
    /// dimension; an unknown name simply never matches, so a typo weakens nothing.
    pub fn waiving(mut self, dimension: impl Into<String>) -> Self {
        self.waived.insert(dimension.into());
        self
    }

    pub fn waives(&self, dimension: &str) -> bool {
        self.waived.contains(dimension)
    }

    pub fn waived(&self) -> impl Iterator<Item = &str> {
        self.waived.iter().map(String::as_str)
    }

    pub fn is_strict(&self) -> bool {
        self.waived.is_empty()
    }

    fn filter(
        &self,
        outcome: Result<(), ScoreIncomparability>,
    ) -> Result<(), ScoreIncomparability> {
        match outcome {
            Err(reason) if self.waives(&reason.dimension()) => Ok(()),
            other => other,
        }
    }
}

/// Whether two sets of conditions describe the same measurement regime.
pub fn comparable(
    left: &MeasurementConditions,
    right: &MeasurementConditions,
) -> Result<(), ScoreIncomparability> {
    comparable_under(left, right, &ComparabilityPolicy::strict())
}

/// [`comparable`] with an explicit waiver policy.
pub fn comparable_under(
    left: &MeasurementConditions,
    right: &MeasurementConditions,
    policy: &ComparabilityPolicy,
) -> Result<(), ScoreIncomparability> {
    policy.filter(check_subject(left, right))?;
    policy.filter(check_scoring_rule(left, right))?;
    policy.filter(compare_condition(
        "ontology version",
        &left.ontology_version,
        &right.ontology_version,
        |left, right| ScoreIncomparability::DifferentOntologyVersion { left, right },
    ))?;
    policy.filter(compare_condition(
        "pack version",
        &left.pack_version,
        &right.pack_version,
        |left, right| ScoreIncomparability::DifferentPackVersion { left, right },
    ))?;
    policy.filter(compare_condition(
        "evidence base",
        &left.evidence_base,
        &right.evidence_base,
        |left, right| ScoreIncomparability::DifferentEvidenceBase { left, right },
    ))?;
    policy.filter(compare_condition(
        "oracle floor",
        &oracle_display(&left.oracle_floor),
        &oracle_display(&right.oracle_floor),
        |left, right| ScoreIncomparability::DifferentOracleFloor { left, right },
    ))?;
    policy.filter(compare_condition(
        "budget",
        &left.budget,
        &right.budget,
        |left, right| ScoreIncomparability::DifferentBudget { left, right },
    ))?;
    policy.filter(left.compare_stratum(right))
}

/// Whether two grids may be compared cell by cell.
pub fn grids_comparable(
    left: &CapabilityGrid,
    right: &CapabilityGrid,
    policy: &ComparabilityPolicy,
) -> Result<(), ScoreIncomparability> {
    comparable_under(
        &left.conditions.about(right.conditions.subject.clone()),
        &right.conditions,
        policy,
    )
}

/// Whether two aggregates are two readings of one quantity.
///
/// Note what this does *not* check: coverage. Two aggregates over the same conditions with 40% and
/// 95% coverage are formally comparable and substantively not, which is why
/// [`ComparisonReport`] raises coverage as a caveat rather than a block — a block would be a
/// judgement call this crate has no basis for, and a silent pass would be worse.
pub fn aggregates_comparable(
    left: &CoveredAggregate,
    right: &CoveredAggregate,
    policy: &ComparabilityPolicy,
) -> Result<(), ScoreIncomparability> {
    comparable_under(left.conditions(), right.conditions(), policy)
}

/// A comparison and everything that had to be true, waived, or noticed to reach it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ComparisonReport {
    pub left: String,
    pub right: String,
    pub verdict: Verdict,
    /// Dimensions the caller waived. Empty under [`ComparabilityPolicy::strict`].
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub waived: Vec<String>,
    /// Things that did not block and that a reader must not be allowed to miss.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub caveats: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "verdict", rename_all = "snake_case")]
pub enum Verdict {
    Comparable,
    Blocked { reason: ScoreIncomparability },
}

impl Verdict {
    pub fn is_comparable(&self) -> bool {
        matches!(self, Verdict::Comparable)
    }

    /// Whether the block is an absence of recorded conditions rather than a disagreement between
    /// them. A release gate treats the two differently: one is a labelling defect the evaluation
    /// can fix, the other is a fact about the systems.
    pub fn blocked_by_absence(&self) -> bool {
        matches!(self, Verdict::Blocked { reason } if reason.is_absence())
    }
}

impl ComparisonReport {
    /// Builds the full report for two aggregates.
    ///
    /// The caveats are why the type exists. A verdict of `Comparable` reached with a waived pack
    /// version, or between a 95%-covered aggregate and a 40%-covered one, is a different object
    /// from one reached with everything recorded and both grids complete — and a receipt that
    /// flattened the difference would be doing the thing this crate exists to stop.
    pub fn of_aggregates(
        left: &CoveredAggregate,
        right: &CoveredAggregate,
        policy: &ComparabilityPolicy,
    ) -> Self {
        let mut caveats = Vec::new();
        for (side, aggregate) in [("left", left), ("right", right)] {
            if !aggregate.coverage().is_complete() {
                caveats.push(format!(
                    "{side} aggregate covers {:.2} of its declared scope; {} cells are unmeasured",
                    aggregate.coverage().fraction_of_scope,
                    aggregate.coverage().blocking_holes.len()
                ));
            }
            if aggregate.interval().is_none() {
                caveats.push(format!(
                    "{side} aggregate has no interval, so a difference between these numbers \
                     cannot be distinguished from noise"
                ));
            }
        }
        if left.rule() != right.rule() {
            caveats.push(format!(
                "aggregation rules differ: {} versus {}; the numbers answer different questions \
                 about the same grid",
                left.rule().as_str(),
                right.rule().as_str()
            ));
        }
        if let (Some(a), Some(b)) = (left.weighting_digest(), right.weighting_digest()) {
            if a != b {
                caveats.push(
                    "the two aggregates were produced under different declared weightings"
                        .to_string(),
                );
            }
        }
        let unrecorded = left.conditions().unrecorded_coordinates();
        if !unrecorded.is_empty() {
            caveats.push(format!(
                "left conditions leave {} of 33.01's stratification coordinates unrecorded: {}",
                unrecorded.len(),
                unrecorded.join(", ")
            ));
        }

        let verdict = match aggregates_comparable(left, right, policy) {
            Ok(()) => Verdict::Comparable,
            Err(reason) => Verdict::Blocked { reason },
        };
        ComparisonReport {
            left: left.grid().to_string(),
            right: right.grid().to_string(),
            verdict,
            waived: policy.waived().map(str::to_string).collect(),
            caveats,
        }
    }

    /// A content hash of the report, so a downstream artefact can cite this verdict rather than
    /// restate it. Mirrors `bioprism_standards::ComparabilityReport::digest`.
    pub fn digest(&self) -> Result<ContentHash, MetricsError> {
        let value = serde_json::to_value(self).map_err(|error| MetricsError::Encoding {
            subject: "comparison report".to_string(),
            detail: error.to_string(),
        })?;
        ContentHash::of_value(&value).map_err(|error| MetricsError::Encoding {
            subject: "comparison report".to_string(),
            detail: error.to_string(),
        })
    }
}

fn check_subject(
    left: &MeasurementConditions,
    right: &MeasurementConditions,
) -> Result<(), ScoreIncomparability> {
    if left.subject == right.subject {
        Ok(())
    } else {
        Err(ScoreIncomparability::DifferentSubject {
            left: left.subject.to_string(),
            right: right.subject.to_string(),
        })
    }
}

fn check_scoring_rule(
    left: &MeasurementConditions,
    right: &MeasurementConditions,
) -> Result<(), ScoreIncomparability> {
    if left.scoring_rule == right.scoring_rule {
        Ok(())
    } else {
        Err(ScoreIncomparability::DifferentScoringRule {
            left: left.scoring_rule.to_string(),
            right: right.scoring_rule.to_string(),
        })
    }
}

fn oracle_display(condition: &Condition<bioprism_atlas::OracleTier>) -> Condition<String> {
    match condition {
        Condition::Recorded { value } => Condition::recorded(value.to_string()),
        Condition::Unrecorded => Condition::Unrecorded,
    }
}
