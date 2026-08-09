//! Turning an estimate into the shape `bioprism-section` expects.
//!
//! [`bioprism_section::InfluenceClass::Bounded`] exists today and nothing in the workspace
//! constructs it. This module is the constructor. It is deliberately thin — the interesting work
//! is in deciding what number is defensible, and once that is decided the mapping is mechanical,
//! which is the point: integration should be a wiring change in `bioprism-fiber`, not a redesign.
//!
//! Two mappings are load-bearing and are asserted by tests rather than left to review:
//!
//! - [`crate::InfluenceEstimate::Unknown`] maps to [`bioprism_section::InfluenceClass::Unknown`]
//!   with `bound: None`. There is no path by which an unknown influence acquires a numeric bound,
//!   because there is no `f64` in the `Unknown` variant to leak.
//! - A bound of exactly zero maps to [`bioprism_section::InfluenceClass::Bounded`], *not* to
//!   `Zero`, unless the method was [`crate::BoundMethod::StructuralZero`]. The two claims differ:
//!   `Zero` says no dependency path reaches the evidence, while a computed `0.0` says a path
//!   exists and this particular perturbation happens not to travel down it. Collapsing them would
//!   overstate the structural claim, which is the exact error the omission manifest exists to
//!   avoid.

use crate::analysis::InfluenceAnalysis;
use crate::bound::{BoundMethod, InfluenceBound, InfluenceEstimate};
use bioprism_section::{InfluenceClass, OmissionGroup};

/// How many representative members a group carries onto the certificate.
///
/// `bioprism-fiber` uses three; matching it keeps certificates comparable.
pub const EXAMPLE_LIMIT: usize = 3;

/// Builds the omission group an estimate licenses.
///
/// `count` is the number of omitted members the group covers, which is not the number of factors
/// analysed: an omission group is a family of *facts* grouped by structural reason, and one factor
/// may carry many of them.
pub fn omission_group(
    reason: impl Into<String>,
    count: usize,
    estimate: &InfluenceEstimate,
    examples: impl IntoIterator<Item = String>,
) -> OmissionGroup {
    let (influence, bound) = match estimate {
        InfluenceEstimate::Bounded(bound) => match bound.method() {
            BoundMethod::StructuralZero => (InfluenceClass::Zero, Some(0.0)),
            _ => (InfluenceClass::Bounded, Some(bound.value())),
        },
        InfluenceEstimate::Unknown(_) => (InfluenceClass::Unknown, None),
    };
    OmissionGroup {
        reason: reason.into(),
        influence,
        count,
        bound,
        examples: examples.into_iter().take(EXAMPLE_LIMIT).collect(),
    }
}

/// The same mapping, with the reason string generated from the analysis.
///
/// The generated reason names the method and the perturbation class, because a bound whose
/// derivation is not on the certificate is a number a reader has to take on trust.
pub fn omission_group_from_analysis(
    analysis: &InfluenceAnalysis,
    count: usize,
    examples: impl IntoIterator<Item = String>,
) -> OmissionGroup {
    let reason = match &analysis.estimate {
        InfluenceEstimate::Bounded(bound) => format!(
            "influence bounded by {} in {} under {} perturbation, by {}",
            bound.value(),
            bound.metric().as_str(),
            analysis.perturbation.class_name(),
            bound.method().as_str()
        ),
        InfluenceEstimate::Unknown(reason) => {
            format!("influence not bounded by any implemented method: {reason}")
        }
    };
    omission_group(reason, count, &analysis.estimate, examples)
}

/// Whether a group's bound constrains anything at all.
///
/// A bound of one is sound and permits every answer, so a manifest full of vacuous bounds is
/// formally sufficient and practically empty. `AGENTS.md` requires that a measurement that
/// disagrees with the thesis be published rather than dressed up; counting vacuous groups
/// separately is how that is done here.
pub fn is_informative(group: &OmissionGroup) -> bool {
    group.influence == InfluenceClass::Bounded && group.bound.is_some_and(|value| value < 1.0)
}

/// A summary of what a manifest's bounded groups actually promise.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct BoundedSummary {
    pub bounded_groups: usize,
    pub informative_groups: usize,
    pub vacuous_groups: usize,
    pub unknown_groups: usize,
    /// The largest bound among informative groups, which is the one a sufficiency argument has to
    /// clear. `None` when there are none.
    pub worst_informative_bound: Option<f64>,
}

pub fn summarise<'a>(groups: impl IntoIterator<Item = &'a OmissionGroup>) -> BoundedSummary {
    let mut summary = BoundedSummary::default();
    for group in groups {
        match group.influence {
            InfluenceClass::Bounded => {
                summary.bounded_groups += 1;
                if is_informative(group) {
                    summary.informative_groups += 1;
                    let value = group.bound.unwrap_or(1.0);
                    summary.worst_informative_bound =
                        Some(summary.worst_informative_bound.map_or(value, |seen: f64| {
                            seen.max(value)
                        }));
                } else {
                    summary.vacuous_groups += 1;
                }
            }
            InfluenceClass::Unknown => summary.unknown_groups += 1,
            _ => {}
        }
    }
    summary
}

/// The `bound` field a section-level consumer will read, or `None`.
///
/// Exists so that no caller is tempted to write `estimate.bound().map(|b| b.value()).unwrap_or(1.0)`
/// and thereby turn an unknown influence into a vacuous bound, which would silently promote an
/// unanalysed group into one that supports a sufficiency claim.
pub fn certificate_bound(estimate: &InfluenceEstimate) -> Option<f64> {
    estimate.bound().map(InfluenceBound::value)
}
