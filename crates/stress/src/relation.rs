//! What must stay put, what must move, and which of the two is being tested.
//!
//! Blueprint 32.22 lists the relation vocabulary — invariance, equivariance, monotonicity, bounded
//! sensitivity, rank preservation — and names *"relation is under-specified"* as the failure that
//! makes a mutation program worthless. The under-specification this module refuses is subtler than
//! a missing relation: it is failing to distinguish a relation that **must** hold from one that is
//! **being tested**.
//!
//! Both look like `assert_eq!` and they mean opposite things. When a discriminative ranking moves
//! under a pure reweighting, nothing has been discovered about robustness — the ranking is a
//! function of measurements that were not touched, so something is broken. When the same ranking
//! moves under a batch offset, that *is* the finding, and the useful output is the offset at which
//! it happened. [`Obligation`] carries the difference, and [`crate::profile`] routes the two to
//! different sections of the report.
//!
//! The strongest relation here is [`StressRelation::MovesBy`]. 32.07's worked relation says
//! calibration *"must change"* under a prevalence shift but does not say by how much, which leaves
//! the check unfalsifiable in practice — almost any movement satisfies "must change". Under a
//! shift from base rate `p₀` to `p₁`, a posterior whose prior enters additively in log-odds must
//! move by exactly `logit(p₁) − logit(p₀)`, subject by subject. That is checkable to nine decimal
//! places, and a procedure that moves by anything else is not calibrated, however confidently it
//! is described as such.

use crate::cohort::Cohort;
use crate::conclusion::{logit, Character, Conclusion, Procedure};
use crate::error::StressError;
use crate::family::{Knob, Stress};
use crate::invariant::PostconditionResult;
use crate::perturb::{interpolated_target, ARITHMETIC_TOLERANCE};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

/// Whether a relation is a rule or a hypothesis.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Obligation {
    /// Must hold at every magnitude. A violation indicts the procedure or the generator, and is
    /// never a robustness finding.
    Required,
    /// The robustness claim under test. A violation is the finding, and the magnitude at which it
    /// first occurs is the number worth reporting.
    Probed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    Increases,
    Decreases,
}

impl Direction {
    pub fn as_str(self) -> &'static str {
        match self {
            Direction::Increases => "increases",
            Direction::Decreases => "decreases",
        }
    }
}

/// A constraint on how a conclusion may respond to a stress.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "relation", rename_all = "snake_case")]
pub enum StressRelation {
    /// Bit-for-bit unchanged. Reserved for values that are functions of untouched inputs.
    Invariant,
    /// A scalar may move, by no more than this.
    BoundedDrift { tolerance: f64 },
    /// A scalar must move by this much. Equivariance, with the amount computed in advance.
    MovesBy { expected: f64, tolerance: f64 },
    /// A scalar must move, and in this direction.
    Monotone { direction: Direction },
    /// The ordering must be identical.
    OrderPreserved,
    /// Pairs whose parent values differ by more than this ratio must not swap.
    ///
    /// The exact consequence of bounded multiplicative jitter: if no value moves by more than a
    /// factor `1 ± c`, then a pair separated by more than `(1 + c) / (1 − c)` cannot cross. Pairs
    /// closer than that may cross, and the relation says nothing about them — because nothing true
    /// can be said about them.
    OrderPreservedBeyondRatio { ratio: f64 },
    /// The flagged set may gain or lose at most this many members.
    MembershipStable { max_flips: usize },
}

impl StressRelation {
    pub fn describe(&self) -> String {
        match self {
            StressRelation::Invariant => "unchanged".into(),
            StressRelation::BoundedDrift { tolerance } => format!("moves at most {tolerance:.6}"),
            StressRelation::MovesBy {
                expected,
                tolerance,
            } => format!("moves by {expected:+.6} within {tolerance:.6}"),
            StressRelation::Monotone { direction } => direction.as_str().to_string(),
            StressRelation::OrderPreserved => "order unchanged".into(),
            StressRelation::OrderPreservedBeyondRatio { ratio } => {
                format!("no pair separated by more than a factor of {ratio:.6} swaps")
            }
            StressRelation::MembershipStable { max_flips } => {
                format!("at most {max_flips} membership change(s)")
            }
        }
    }

    /// Runs the relation against the parent and descendant conclusions.
    pub fn check(&self, before: &Conclusion, after: &Conclusion) -> PostconditionResult {
        match self {
            StressRelation::Invariant => {
                if before.value == after.value {
                    PostconditionResult::Held
                } else {
                    PostconditionResult::violated(
                        before.value.describe(),
                        after.value.describe(),
                    )
                }
            }
            StressRelation::BoundedDrift { tolerance } => {
                scalar_pair(before, after, |start, end| {
                    if (end - start).abs() <= *tolerance {
                        PostconditionResult::Held
                    } else {
                        PostconditionResult::violated(
                            format!("within {tolerance:.6} of {start:.6}"),
                            format!("{end:.6}, a move of {:+.6}", end - start),
                        )
                    }
                })
            }
            StressRelation::MovesBy {
                expected,
                tolerance,
            } => scalar_pair(before, after, |start, end| {
                let observed = end - start;
                if (observed - expected).abs() <= *tolerance {
                    PostconditionResult::Held
                } else {
                    PostconditionResult::violated(
                        format!("a move of {expected:+.9} within {tolerance:.9}"),
                        format!("a move of {observed:+.9}"),
                    )
                }
            }),
            StressRelation::Monotone { direction } => scalar_pair(before, after, |start, end| {
                let observed = end - start;
                let correct = match direction {
                    Direction::Increases => observed >= ARITHMETIC_TOLERANCE,
                    Direction::Decreases => observed <= -ARITHMETIC_TOLERANCE,
                };
                if correct {
                    PostconditionResult::Held
                } else {
                    PostconditionResult::violated(
                        format!("a value that {} from {start:.6}", direction.as_str()),
                        format!("{end:.6}, a move of {observed:+.9}"),
                    )
                }
            }),
            StressRelation::OrderPreserved => {
                let start = before.value.ids();
                let end = after.value.ids();
                if start == end {
                    return PostconditionResult::Held;
                }
                match start
                    .iter()
                    .zip(end.iter())
                    .position(|(start, end)| start != end)
                {
                    Some(index) => PostconditionResult::violated(
                        format!("{} at rank {}", start[index], index + 1),
                        format!("{} at rank {}", end[index], index + 1),
                    ),
                    None => PostconditionResult::violated(
                        format!("{} ranked subjects", start.len()),
                        format!("{} ranked subjects", end.len()),
                    ),
                }
            }
            StressRelation::OrderPreservedBeyondRatio { ratio } => {
                order_beyond_ratio(before, after, *ratio)
            }
            StressRelation::MembershipStable { max_flips } => {
                let start: BTreeSet<&str> = before.value.ids().into_iter().collect();
                let end: BTreeSet<&str> = after.value.ids().into_iter().collect();
                let flips = start.symmetric_difference(&end).count();
                if flips <= *max_flips {
                    PostconditionResult::Held
                } else {
                    PostconditionResult::violated(
                        format!("at most {max_flips} membership change(s)"),
                        format!(
                            "{flips} change(s): {}",
                            start
                                .symmetric_difference(&end)
                                .copied()
                                .collect::<Vec<_>>()
                                .join(", ")
                        ),
                    )
                }
            }
        }
    }
}

fn scalar_pair(
    before: &Conclusion,
    after: &Conclusion,
    check: impl Fn(f64, f64) -> PostconditionResult,
) -> PostconditionResult {
    match (before.value.as_scalar(), after.value.as_scalar()) {
        (Some(start), Some(end)) => check(start, end),
        _ => PostconditionResult::violated(
            "a scalar conclusion on both sides",
            format!(
                "{} then {}",
                before.value.describe(),
                after.value.describe()
            ),
        ),
    }
}

/// Checks that no sufficiently separated pair changed places.
fn order_beyond_ratio(
    before: &Conclusion,
    after: &Conclusion,
    ratio: f64,
) -> PostconditionResult {
    let (Some(start), Some(end)) = (before.value.as_ordering(), after.value.as_ordering()) else {
        return PostconditionResult::violated(
            "an ordering on both sides",
            format!(
                "{} then {}",
                before.value.describe(),
                after.value.describe()
            ),
        );
    };
    let position = |id: &str| end.iter().position(|ranked| ranked.id == id);

    for (higher_index, higher) in start.iter().enumerate() {
        for lower in start.iter().skip(higher_index + 1) {
            if lower.value <= 0.0 || higher.value < lower.value * ratio {
                continue;
            }
            let (Some(higher_after), Some(lower_after)) =
                (position(&higher.id), position(&lower.id))
            else {
                return PostconditionResult::violated(
                    format!("{} and {} still ranked", higher.id, lower.id),
                    "one of them left the ordering".to_string(),
                );
            };
            if higher_after > lower_after {
                return PostconditionResult::violated(
                    format!(
                        "{} ({:.4}) above {} ({:.4}), separated by more than a factor of {ratio:.6}",
                        higher.id, higher.value, lower.id, lower.value
                    ),
                    format!("{} above {}", lower.id, higher.id),
                );
            }
        }
    }
    PostconditionResult::Held
}

/// A relation, its obligation, and the reason it was declared.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DeclaredRelation {
    pub procedure: Procedure,
    pub relation: StressRelation,
    pub obligation: Obligation,
    pub rationale: String,
}

/// Declares what a procedure must do under a stress, before the stress is applied.
///
/// A function of the parent cohort and the stress alone — never of the result. A "postcondition"
/// computed after seeing the outcome is a description, not a test.
pub fn declare(
    stress: &Stress,
    procedure: &Procedure,
    parent: &Cohort,
) -> Result<DeclaredRelation, StressError> {
    let required = |relation: StressRelation, rationale: &str| DeclaredRelation {
        procedure: procedure.clone(),
        relation,
        obligation: Obligation::Required,
        rationale: rationale.to_string(),
    };
    let probed = |relation: StressRelation, rationale: &str| DeclaredRelation {
        procedure: procedure.clone(),
        relation,
        obligation: Obligation::Probed,
        rationale: rationale.to_string(),
    };
    let spread = parent.pooled_within_sd().unwrap_or(1.0).max(f64::MIN_POSITIVE);

    Ok(match &stress.knob {
        Knob::PrevalenceShift { target_prevalence } => {
            let after = interpolated_target(
                parent.prevalence(),
                *target_prevalence,
                stress.magnitude.fraction(),
            );
            prevalence_relation(procedure, parent, after, required, probed)?
        }
        Knob::BatchEffect { .. } => match procedure.character() {
            Character::Geometric => required(
                StressRelation::Invariant,
                "a batch offset moves markers, and no geometric summary reads a marker",
            ),
            _ => match procedure {
                Procedure::MarkerRanking => probed(
                    StressRelation::OrderPreserved,
                    "a ranking that survives a technical offset ordered subjects by biology",
                ),
                Procedure::MarkerSeparation => probed(
                    StressRelation::BoundedDrift { tolerance: 0.05 },
                    "separation that survives a technical offset was not the offset",
                ),
                Procedure::GroupContrast => probed(
                    StressRelation::BoundedDrift {
                        tolerance: 0.1 * spread,
                    },
                    "an effect size that moves with the batch was partly measuring the batch",
                ),
                Procedure::CalibratedLogOdds { .. } => probed(
                    StressRelation::BoundedDrift { tolerance: 0.2 },
                    "a risk estimate that shifts with the laboratory is a laboratory estimate",
                ),
                _ => probed(
                    StressRelation::BoundedDrift { tolerance: 0.05 },
                    "a decision quantity that flips with the batch was reading the batch",
                ),
            },
        },
        Knob::AssayDegradation {
            limit_of_detection, ..
        } => match procedure {
            Procedure::VolumeThreshold { .. } | Procedure::VolumeRanking => {
                if limit_of_detection.is_some() {
                    probed(
                        StressRelation::MembershipStable { max_flips: 0 },
                        "geometry is untouched, so any change is the assay removing subjects from \
                         the analysis rather than anatomy moving",
                    )
                } else {
                    required(
                        StressRelation::Invariant,
                        "widening the marker's error bars cannot reach a volume",
                    )
                }
            }
            Procedure::MarkerRanking => probed(
                StressRelation::OrderPreserved,
                "an ordering that survives the assay's own imprecision is not reading noise",
            ),
            Procedure::MarkerSeparation => probed(
                StressRelation::BoundedDrift { tolerance: 0.05 },
                "separation that survives widened error bars is not an artefact of precision",
            ),
            Procedure::GroupContrast => probed(
                StressRelation::BoundedDrift {
                    tolerance: 0.1 * spread,
                },
                "the class means are held fixed by construction, so drift here is the analysis \
                 reacting to spread alone",
            ),
            Procedure::CalibratedLogOdds { .. } => probed(
                StressRelation::BoundedDrift { tolerance: 0.2 },
                "a risk estimate that moves by more than a fifth of a log-odd when only the error \
                 bars widened was reading precision as signal",
            ),
            _ => probed(
                StressRelation::BoundedDrift { tolerance: 0.05 },
                "a decision quantity that vanishes under the assay's stated imprecision was \
                 resting on precision the assay does not have",
            ),
        },
        Knob::SegmentationJitter {
            reproducibility_cv, ..
        } => {
            let cv = stress.magnitude.fraction() * reproducibility_cv;
            match procedure {
                Procedure::VolumeRanking => required(
                    StressRelation::OrderPreservedBeyondRatio {
                        ratio: (1.0 + cv) / (1.0 - cv),
                    },
                    "bounded jitter cannot reorder volumes separated by more than the bound; a \
                     swap outside that band is a defective generator, not a fragile conclusion",
                ),
                Procedure::VolumeThreshold { .. } => probed(
                    StressRelation::MembershipStable { max_flips: 0 },
                    "a threshold that reclassifies a subject inside the segmentation's own \
                     test-retest band is reporting one contour, not an anatomy",
                ),
                _ => required(
                    StressRelation::Invariant,
                    "jitter touches volumes only, so a non-geometric summary that moves means the \
                     jitter leaked",
                ),
            }
        }
    })
}

/// The prevalence family's relations, which are the point of the family.
fn prevalence_relation(
    procedure: &Procedure,
    parent: &Cohort,
    after_prevalence: f64,
    required: impl Fn(StressRelation, &str) -> DeclaredRelation,
    probed: impl Fn(StressRelation, &str) -> DeclaredRelation,
) -> Result<DeclaredRelation, StressError> {
    let before_prevalence = parent.prevalence();
    Ok(match procedure {
        Procedure::MarkerRanking => required(
            StressRelation::OrderPreserved,
            "reweighting touches no measurement, so a discriminative ordering cannot legitimately \
             move",
        ),
        Procedure::MarkerSeparation => required(
            StressRelation::BoundedDrift {
                tolerance: ARITHMETIC_TOLERANCE,
            },
            "every positive-negative pair and the normaliser scale by the same factor under \
             class-uniform reweighting, so rank concordance is algebraically invariant",
        ),
        Procedure::GroupContrast => required(
            StressRelation::BoundedDrift {
                tolerance: ARITHMETIC_TOLERANCE,
            },
            "a within-class weighted mean is unchanged when every member of the class is \
             reweighted by the same factor",
        ),
        Procedure::VolumeThreshold { .. } | Procedure::VolumeRanking => required(
            StressRelation::Invariant,
            "sampling weight does not enter a geometric summary",
        ),
        Procedure::CalibratedLogOdds { .. } => {
            let (Some(before), Some(after)) =
                (logit(before_prevalence), logit(after_prevalence))
            else {
                return Err(StressError::ConclusionUndefined {
                    procedure: procedure.id(),
                    cohort: parent.id.clone(),
                    reason: "a base rate of exactly zero or one has no log-odds".into(),
                });
            };
            required(
                StressRelation::MovesBy {
                    expected: after - before,
                    tolerance: ARITHMETIC_TOLERANCE,
                },
                "the prior enters posterior log-odds additively, so a calibrated procedure must \
                 move by exactly the change in prior log-odds — not merely 'change'",
            )
        }
        Procedure::PositivePredictiveValue { .. } => {
            let moved = after_prevalence - before_prevalence;
            if moved.abs() <= ARITHMETIC_TOLERANCE {
                probed(
                    StressRelation::BoundedDrift {
                        tolerance: ARITHMETIC_TOLERANCE,
                    },
                    "the shift is a no-op at this magnitude, so nothing is required to move",
                )
            } else {
                required(
                    StressRelation::Monotone {
                        direction: if moved > 0.0 {
                            Direction::Increases
                        } else {
                            Direction::Decreases
                        },
                    },
                    "predictive value is a posterior quantity; it must follow the base rate, and a \
                     procedure whose predictive value ignores prevalence is mislabelled",
                )
            }
        }
    })
}
