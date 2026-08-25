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
//! - A *vacuous* bound does not map to `Bounded` at all. [`InfluenceBound::is_vacuous`] has always
//!   named the condition and [`is_informative`] has always reported it, but both were readable
//!   *after* the group was built, so a group whose bound excluded nothing could still fold into
//!   [`bioprism_section::OmissionManifest::supports_sufficiency_claim`] as if it had. The mapping
//!   now goes through [`bioprism_section::InformativeBound`], whose constructor is the refusal:
//!   a vacuous estimate produces [`bioprism_section::InfluenceClass::Unknown`] with no number,
//!   which is what "sound and constrains nothing" actually is.

use crate::analysis::InfluenceAnalysis;
use crate::bound::{BoundMethod, InfluenceBound, InfluenceEstimate};
use bioprism_section::{InfluenceClass, InformativeBound, OmissionGroup};

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
    let examples = examples.into_iter().take(EXAMPLE_LIMIT);
    match estimate {
        InfluenceEstimate::Bounded(bound) if bound.method() == BoundMethod::StructuralZero => {
            OmissionGroup {
                reason: reason.into(),
                influence: InfluenceClass::Zero,
                count,
                bound: Some(0.0),
                examples: examples.collect(),
            }
        }
        InfluenceEstimate::Bounded(bound) => match InformativeBound::new(bound.value()) {
            Some(informative) => OmissionGroup::bounded(reason, count, informative, examples),
            None => OmissionGroup {
                reason: format!(
                    "{}; the {} bound of {} permits every answer, so nothing was excluded",
                    reason.into(),
                    bound.method().as_str(),
                    bound.value()
                ),
                influence: InfluenceClass::Unknown,
                count,
                bound: None,
                examples: examples.collect(),
            },
        },
        InfluenceEstimate::Unknown(_) => OmissionGroup {
            reason: reason.into(),
            influence: InfluenceClass::Unknown,
            count,
            bound: None,
            examples: examples.collect(),
        },
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
///
/// Delegates to [`OmissionGroup::has_informative_bound`] rather than restating the comparison, so
/// there is one definition of the predicate and it is the one the constructor enforces. A group
/// built by [`omission_group`] can no longer fail it; a group built by struct literal still can,
/// and this is how such a group is detected.
pub fn is_informative(group: &OmissionGroup) -> bool {
    group.has_informative_bound()
}

/// A summary of what a manifest's bounded groups actually promise.
///
/// `vacuous_groups` is retained now that [`omission_group`] cannot produce one, because
/// [`OmissionGroup`]'s fields are public: a group assembled by struct literal can still claim
/// `Bounded` with a bound that excludes nothing, and this is the counter that finds it before
/// [`bioprism_section::OmissionManifest::push`] downgrades it.
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
