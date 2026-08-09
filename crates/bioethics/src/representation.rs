//! Blueprint 36.13 — fairness, representation and global resource context.
//!
//! 36.13's purpose sentence carries the constraint that shapes this whole module: "evaluate
//! performance and utility across populations, sites, languages, assay access, and resource levels
//! **without essentializing groups**".
//!
//! # The anti-essentialism is a missing enum variant
//!
//! [`Attribution`] has two variants. A finding is attributable to a *context axis*, or it is
//! attributable to nothing yet. There is no `Attribution::ToGroup`, no `Attribution::ToAncestry`
//! and no field anywhere in this module that would let a caller record "this system is worse for
//! this population" as a property of the population rather than as a coordinate of the
//! measurement. That is registered as
//! [`crate::safeguard::Impossibility::NoFindingIsAttributedToAGroupRatherThanAContext`], and it is
//! the strongest form the blueprint's sentence can take in a type.
//!
//! # The resource axes are confounders, and the rule says so
//!
//! Three of 36.13's six scope axes — site resources, scanner and laboratory availability, and
//! follow-up and referral patterns — describe *access to measurement* rather than the subject of
//! it. [`ContextAxis::is_resource_context`] is that split, and [`attribute`] returns
//! [`Attribution::Unattributable`] when any of them differs across the strata being compared,
//! naming which. A difference in outcome between two sites that do not have the same instruments
//! is a statement about the instruments until somebody matches on them.
//!
//! The split is this crate's, derived from the module's own list; 36.13 does not classify its own
//! axes. It is written here rather than buried so that a reader who would place an axis
//! differently can see what they are disagreeing with.
//!
//! # An unmeasured stratum is not a passing stratum
//!
//! [`RepresentationSummary`] has private fields and one constructor, and every summary carries its
//! unmeasured strata beside its measured ones. There is no accessor that returns only the good
//! news and no way to build a summary that forgot to partition. This is the workspace's
//! unmeasured-is-not-zero rule in the currency of population coverage, and it is registered as
//! [`crate::safeguard::Impossibility::NoRepresentationSummaryOmitsItsUnmeasuredStrata`].
//!
//! # Where the arithmetic lives, and why not here
//!
//! `bioprism-metrics` owns aggregation over a capability grid, the worst-measured cell, intervals,
//! and the rule that an aggregate over a grid containing an unmeasured cell is not an aggregate
//! over the grid. This module computes no calibration, no interval, no disparity statistic and no
//! ranking. It holds the *vocabulary* of strata and the *attribution rule*, which is the part
//! 36.13 asks for and no sibling has.
//!
//! Small-group protection is `bioprism-policy`'s: [`StratumCoverage::from_cell`] takes a
//! `bioprism_policy::SmallCellRule` and reads its verdict. The threshold belongs to the rule the
//! caller supplies. No threshold is written in this crate, and 36.13 states none.
//!
//! # What 36.13 names and never specifies
//!
//! * **"worst-group calibration"** — no calibration metric, no binning, and no definition of
//!   "group" beyond the six scope axes. Not computed here.
//! * **"coverage and abstention"** — no coverage floor and no abstention rule.
//!   `bioprism-evalengine` owns coverage floors over posteriors; this module reports the partition.
//! * **"resource-constrained architecture"** — no resource model, no cost, no tier.
//! * **"small-group protection"** — no threshold, in this module or anywhere in §36.
//! * **"community review"** — no community, no composition, no cadence. Perimeter, declared only.
//! * **"languages"** appears in the purpose sentence and in no scope bullet, so there is no
//!   language axis here; adding one would be reading a taxonomy into a single word.
//!
//! # Not implemented
//!
//! No demographic ontology, no population reference panel, no ancestry inference, no site
//! registry, no statistics and no clock. A [`Stratum`] label is an opaque string this crate never
//! interprets.

use crate::error::BioethicsError;
use bioprism_policy::redaction::{CellRelease, SmallCellRule};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::fmt;

/// The six axes of 36.13's Scope, transcribed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextAxis {
    AncestryAndPopulationStructure,
    AgeAndSex,
    Geography,
    SiteResources,
    ScannerAndLaboratoryAvailability,
    FollowUpAndReferralPatterns,
}

impl ContextAxis {
    pub const ALL: [ContextAxis; 6] = [
        ContextAxis::AncestryAndPopulationStructure,
        ContextAxis::AgeAndSex,
        ContextAxis::Geography,
        ContextAxis::SiteResources,
        ContextAxis::ScannerAndLaboratoryAvailability,
        ContextAxis::FollowUpAndReferralPatterns,
    ];

    /// Whether the axis describes access to measurement rather than the subject of it.
    ///
    /// This crate's classification, derived from 36.13's own list. Its consequence is that these
    /// three must be matched before a difference along any other axis is attributable.
    pub const fn is_resource_context(self) -> bool {
        match self {
            ContextAxis::SiteResources
            | ContextAxis::ScannerAndLaboratoryAvailability
            | ContextAxis::FollowUpAndReferralPatterns => true,
            ContextAxis::AncestryAndPopulationStructure
            | ContextAxis::AgeAndSex
            | ContextAxis::Geography => false,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            ContextAxis::AncestryAndPopulationStructure => "ancestry_and_population_structure",
            ContextAxis::AgeAndSex => "age_and_sex",
            ContextAxis::Geography => "geography",
            ContextAxis::SiteResources => "site_resources",
            ContextAxis::ScannerAndLaboratoryAvailability => "scanner_and_laboratory_availability",
            ContextAxis::FollowUpAndReferralPatterns => "follow_up_and_referral_patterns",
        }
    }

    /// The blueprint's own words. Not elaborated.
    pub const fn describe(self) -> &'static str {
        match self {
            ContextAxis::AncestryAndPopulationStructure => "ancestry and population structure",
            ContextAxis::AgeAndSex => "age and sex",
            ContextAxis::Geography => "geography",
            ContextAxis::SiteResources => "site resources",
            ContextAxis::ScannerAndLaboratoryAvailability => "scanner and laboratory availability",
            ContextAxis::FollowUpAndReferralPatterns => "follow-up and referral patterns",
        }
    }
}

impl fmt::Display for ContextAxis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A coordinate on one axis.
///
/// A `Stratum` is where a measurement was taken, not what a subject is. Nothing here reads or
/// validates `label`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct Stratum {
    pub axis: ContextAxis,
    pub label: String,
}

impl Stratum {
    pub fn new(axis: ContextAxis, label: impl Into<String>) -> Self {
        Stratum {
            axis,
            label: label.into(),
        }
    }
}

impl fmt::Display for Stratum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}={}", self.axis, self.label)
    }
}

/// What is known about a stratum.
///
/// Three states, and they are not orderable. Suppression is not a measurement and absence is not a
/// pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "coverage", rename_all = "snake_case")]
pub enum StratumCoverage {
    /// Somebody measured this stratum.
    Measured,
    /// Nobody measured this stratum. Categorically distinct from measured-and-poor.
    Unmeasured,
    /// The stratum was too small to release, per the caller's `bioprism-policy` rule.
    SuppressedSmallGroup { below: u32 },
}

impl StratumCoverage {
    /// Reads `bioprism-policy`'s small-cell verdict for a stratum of `count` members.
    ///
    /// The threshold is the rule's. This crate holds none and 36.13 states none.
    pub fn from_cell(rule: &SmallCellRule, count: u64) -> Self {
        match rule.release(count) {
            CellRelease::Exact { .. } => StratumCoverage::Measured,
            CellRelease::BoundedUnknown { below } => {
                StratumCoverage::SuppressedSmallGroup { below }
            }
        }
    }

    pub const fn is_measured(self) -> bool {
        matches!(self, StratumCoverage::Measured)
    }
}

/// One stratum and its coverage.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct StratumObservation {
    pub stratum: Stratum,
    pub coverage: StratumCoverage,
}

impl StratumObservation {
    pub fn new(stratum: Stratum, coverage: StratumCoverage) -> Self {
        StratumObservation { stratum, coverage }
    }
}

/// A partition of the strata a study looked at.
///
/// Private fields, one constructor, no `Deserialize`. Every summary carries all three lists, so
/// "there were no unmeasured strata" and "nobody recorded the unmeasured strata" cannot share a
/// representation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RepresentationSummary {
    subject: String,
    measured: Vec<Stratum>,
    unmeasured: Vec<Stratum>,
    suppressed: Vec<Stratum>,
}

impl RepresentationSummary {
    pub fn subject(&self) -> &str {
        &self.subject
    }

    pub fn measured(&self) -> &[Stratum] {
        &self.measured
    }

    /// The strata nobody measured. Always present, possibly empty.
    pub fn unmeasured(&self) -> &[Stratum] {
        &self.unmeasured
    }

    /// The strata a small-cell rule withheld. Distinct from unmeasured: somebody measured these
    /// and the count could not be released.
    pub fn suppressed(&self) -> &[Stratum] {
        &self.suppressed
    }

    /// Whether every stratum the study declared was measured.
    ///
    /// False when anything is unmeasured *or* suppressed, because a summary that treated a
    /// withheld cell as covered would report coverage the reader cannot inspect.
    pub fn is_complete(&self) -> bool {
        self.unmeasured.is_empty() && self.suppressed.is_empty()
    }

    /// Which axes have at least one stratum that is not measured.
    pub fn incomplete_axes(&self) -> BTreeSet<ContextAxis> {
        self.unmeasured
            .iter()
            .chain(self.suppressed.iter())
            .map(|stratum| stratum.axis)
            .collect()
    }
}

/// The only constructor for a [`RepresentationSummary`].
///
/// Refuses a duplicated stratum: two observations of the same axis and label are either the same
/// stratum recorded twice or two different things sharing a name, and both make the partition a
/// lie.
pub fn summarise<I: IntoIterator<Item = StratumObservation>>(
    subject: impl Into<String>,
    observations: I,
) -> Result<RepresentationSummary, BioethicsError> {
    let mut seen: BTreeSet<Stratum> = BTreeSet::new();
    let mut measured = Vec::new();
    let mut unmeasured = Vec::new();
    let mut suppressed = Vec::new();

    for observation in observations {
        if !seen.insert(observation.stratum.clone()) {
            return Err(BioethicsError::DuplicateStratum {
                axis: observation.stratum.axis.to_string(),
                label: observation.stratum.label,
            });
        }
        match observation.coverage {
            StratumCoverage::Measured => measured.push(observation.stratum),
            StratumCoverage::Unmeasured => unmeasured.push(observation.stratum),
            StratumCoverage::SuppressedSmallGroup { .. } => suppressed.push(observation.stratum),
        }
    }

    Ok(RepresentationSummary {
        subject: subject.into(),
        measured,
        unmeasured,
        suppressed,
    })
}

/// What a difference between strata may be said to be about.
///
/// There is no variant assigning a finding to a group.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "attribution", rename_all = "snake_case")]
pub enum Attribution {
    /// The difference is a coordinate of the measurement on this axis, with the resource context
    /// held matched.
    ToContext { axis: ContextAxis },
    /// The resource context differs, so the difference is not attributable to the axis asked
    /// about — or to any other.
    Unattributable { unmatched: BTreeSet<ContextAxis> },
}

impl Attribution {
    /// The axis, or a typed refusal naming the unmatched resource axes.
    pub fn require_context(self, finding: &str) -> Result<ContextAxis, BioethicsError> {
        match self {
            Attribution::ToContext { axis } => Ok(axis),
            Attribution::Unattributable { unmatched } => {
                let names: Vec<&str> = unmatched.iter().map(|axis| axis.as_str()).collect();
                Err(BioethicsError::ResourceContextUnmatched {
                    finding: finding.to_string(),
                    unmatched: names.join(", "),
                })
            }
        }
    }
}

/// Decides what a difference along `axis` may be attributed to.
///
/// `matched` is the set of axes the comparison held constant. Every resource-context axis other
/// than `axis` itself must be in it; anything missing comes back in
/// [`Attribution::Unattributable`]. The function computes nothing about the finding and never sees
/// a number — it decides only whether the comparison had the standing to say anything at all.
pub fn attribute(axis: ContextAxis, matched: &BTreeSet<ContextAxis>) -> Attribution {
    let unmatched: BTreeSet<ContextAxis> = ContextAxis::ALL
        .into_iter()
        .filter(|candidate| {
            candidate.is_resource_context() && *candidate != axis && !matched.contains(candidate)
        })
        .collect();

    if unmatched.is_empty() {
        Attribution::ToContext { axis }
    } else {
        Attribution::Unattributable { unmatched }
    }
}
