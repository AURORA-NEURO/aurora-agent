//! Site, population, era shift and global equity (30.27).
//!
//! Blueprint 30.27 evaluates "robustness across institutions, scanners, pathology practice,
//! molecular access, treatment eras, age, ancestry, geography, and resource settings".
//! `crates/stress` owns the perturbation side — what happens when you *move* a cohort. This module
//! owns the standing structural fact: some cohorts were never one cohort to begin with.
//!
//! # A classification revision splits a cohort
//!
//! 30.27's required state includes "calendar era and classification version", and 30.23 names
//! "pooling historical and current labels without mapping" as a characteristic failure. Two
//! cohorts classified under different criteria are not comparable until somebody states what the
//! entity labels map onto, and [`comparable_cohorts`] refuses until they do. The mapping is
//! per-label and may legitimately say [`LabelFate::NoEquivalent`] — a label that was split, merged
//! or retired is not silently carried across.
//!
//! [`ClassificationVersion`] and [`EntityLabel`] are **opaque strings**. This crate names no WHO
//! edition and no entity, because 30.27 names none and printing a specific edition year here would
//! be a clinical fact this crate invented.
//!
//! # Resource absence is not biological absence
//!
//! Ladder item 4 is "separate resource absence from biological absence", and the matching failure
//! is "assuming missing molecular tests are negative". An assay that a site cannot run yields
//! `bioprism_onco::ObservationStatus::NotCollected` — silence — and [`as_negative_call`] refuses
//! to turn it into anything else. The site's inability is a fact about the site.
//!
//! # A descriptor is not a mechanism
//!
//! "Using race as biological essence" is a named failure. [`PopulationDescriptor`] separates
//! self-reported categories, administrative categories and genetic ancestry, and
//! [`use_descriptor`] refuses the administrative ones in a mechanistic role. Stratifying by them
//! is always allowed and is frequently the point: 30.27 exists to make subgroup performance
//! visible.
//!
//! # A subgroup result without an interval is not a result
//!
//! "Publishing unstable tiny-subgroup results" is the last named failure. There is no minimum
//! subgroup size here — 30.27 states none, and any number would be this crate legislating. What
//! [`subgroup_claim`] requires instead is that a claim carry its own `n` and an uncertainty
//! interval, so that a reader can see the instability rather than being protected from it by a
//! cutoff somebody guessed.
//!
//! # Not implemented
//!
//! No shift detection, no calibration estimation, no re-weighting, no fairness metric. Nothing
//! here computes a subgroup performance; it constrains what a computed one may be published as.

use crate::error::ShiftRefusal;
use bioprism_onco::{MarkerCall, ObservationStatus, Observed};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// A classification criteria version. Opaque; see the module header.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ClassificationVersion(String);

impl ClassificationVersion {
    pub fn new(value: impl Into<String>) -> Self {
        ClassificationVersion(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// An entity label as written under one classification version.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EntityLabel(String);

impl EntityLabel {
    pub fn new(value: impl Into<String>) -> Self {
        EntityLabel(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// What became of a label across a criteria revision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "fate", rename_all = "snake_case")]
pub enum LabelFate {
    Renamed { to: EntityLabel },
    /// One label became several, so a case under the old label is not assignable to one new one.
    Split { into: BTreeSet<EntityLabel> },
    Merged { into: EntityLabel },
    /// The label is gone and nothing under the new criteria corresponds to it.
    NoEquivalent,
}

impl LabelFate {
    /// Whether a single case under the old label lands on a single new label.
    ///
    /// False for [`LabelFate::Split`]: knowing a case was called by the old name does not say
    /// which of the successors it would be called now, and picking one would be the invented
    /// certainty this crate exists to refuse.
    pub fn resolves_to_one_label(&self) -> bool {
        matches!(self, LabelFate::Renamed { .. } | LabelFate::Merged { .. })
    }
}

/// A stated mapping between two classification versions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityMapping {
    pub from: ClassificationVersion,
    pub to: ClassificationVersion,
    fates: BTreeMap<EntityLabel, LabelFate>,
}

impl EntityMapping {
    pub fn new(from: ClassificationVersion, to: ClassificationVersion) -> Self {
        EntityMapping {
            from,
            to,
            fates: BTreeMap::new(),
        }
    }

    pub fn mapping(mut self, label: EntityLabel, fate: LabelFate) -> Self {
        self.fates.insert(label, fate);
        self
    }

    pub fn fate_of(&self, label: &EntityLabel) -> Option<&LabelFate> {
        self.fates.get(label)
    }

    fn relates(&self, left: &ClassificationVersion, right: &ClassificationVersion) -> bool {
        (&self.from == left && &self.to == right) || (&self.from == right && &self.to == left)
    }
}

/// A cohort, with the classification version its labels were written under.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Cohort {
    pub name: String,
    pub site: String,
    pub classification_version: ClassificationVersion,
    pub entities: BTreeSet<EntityLabel>,
}

impl Cohort {
    pub fn new(
        name: impl Into<String>,
        site: impl Into<String>,
        classification_version: ClassificationVersion,
    ) -> Self {
        Cohort {
            name: name.into(),
            site: site.into(),
            classification_version,
            entities: BTreeSet::new(),
        }
    }

    pub fn containing(mut self, label: EntityLabel) -> Self {
        self.entities.insert(label);
        self
    }
}

/// Whether two cohorts may be compared, given whatever mapping was stated.
///
/// Same version: yes. Different version and no mapping: refused. Different version with a mapping
/// that does not cover a label actually present in the older cohort: refused, naming the label —
/// an incomplete mapping is worse than none, because it looks like diligence.
pub fn comparable_cohorts(
    left: &Cohort,
    right: &Cohort,
    mapping: Option<&EntityMapping>,
) -> Result<(), ShiftRefusal> {
    if left.classification_version == right.classification_version {
        return Ok(());
    }
    let Some(mapping) = mapping.filter(|mapping| {
        mapping.relates(&left.classification_version, &right.classification_version)
    }) else {
        return Err(ShiftRefusal::UnmappedClassificationChange {
            left: left.classification_version.as_str().to_string(),
            right: right.classification_version.as_str().to_string(),
        });
    };
    let older = if mapping.from == left.classification_version {
        left
    } else {
        right
    };
    for label in &older.entities {
        if mapping.fate_of(label).is_none() {
            return Err(ShiftRefusal::IncompleteMapping {
                entity: label.as_str().to_string(),
                version: older.classification_version.as_str().to_string(),
            });
        }
    }
    Ok(())
}

/// Whether an assay could be run at a site (30.27, "diagnostic and treatment resource
/// availability").
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "availability", rename_all = "snake_case")]
pub enum AssayAvailability {
    Available,
    UnavailableAtSite,
}

/// An assay, a site, and whether the one could be run at the other.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SiteAssayContext {
    pub site: String,
    pub assay: String,
    pub availability: AssayAvailability,
}

impl SiteAssayContext {
    pub fn new(
        site: impl Into<String>,
        assay: impl Into<String>,
        availability: AssayAvailability,
    ) -> Self {
        SiteAssayContext {
            site: site.into(),
            assay: assay.into(),
            availability,
        }
    }

    /// How an unavailable assay is represented: as silence, using `bioprism_onco`'s states.
    pub fn observation(&self) -> Observed<MarkerCall> {
        match self.availability {
            AssayAvailability::Available => Observed::Unobserved(ObservationStatus::Missing),
            AssayAvailability::UnavailableAtSite => {
                Observed::Unobserved(ObservationStatus::NotCollected)
            }
        }
    }
}

/// Always refuses, for either availability.
///
/// The signature exists so that the mistake has a name and a place to fail, rather than being an
/// implicit `unwrap_or(Absent)` in a cohort builder. It refuses for an *available* assay too: an
/// assay that could have been run and was not is silence for a different reason, and neither
/// reason is a negative call.
pub fn as_negative_call(
    context: &SiteAssayContext,
) -> Result<std::convert::Infallible, ShiftRefusal> {
    Err(ShiftRefusal::ResourceAbsenceReadAsBiology {
        assay: context.assay.clone(),
        site: context.site.clone(),
    })
}

/// How a population variable was recorded (30.27, "population characteristics with privacy
/// protection").
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PopulationDescriptor {
    SelfReportedRaceOrEthnicity,
    GeneticAncestry,
    GeographicSite,
    ResourceSetting,
    Age,
}

impl PopulationDescriptor {
    pub const fn as_str(self) -> &'static str {
        match self {
            PopulationDescriptor::SelfReportedRaceOrEthnicity => {
                "self-reported race or ethnicity"
            }
            PopulationDescriptor::GeneticAncestry => "genetic ancestry",
            PopulationDescriptor::GeographicSite => "geographic site",
            PopulationDescriptor::ResourceSetting => "resource setting",
            PopulationDescriptor::Age => "age",
        }
    }

    /// Whether the descriptor is a social or administrative category rather than a measured
    /// biological one.
    pub const fn is_administrative(self) -> bool {
        matches!(
            self,
            PopulationDescriptor::SelfReportedRaceOrEthnicity
                | PopulationDescriptor::GeographicSite
                | PopulationDescriptor::ResourceSetting
        )
    }
}

/// What a variable is being used for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DescriptorUse {
    /// Reporting results separately by group. Always permitted; often the point.
    Stratification,
    /// Treating the variable as the biological cause of an effect.
    MechanisticVariable,
}

impl DescriptorUse {
    pub const fn as_str(self) -> &'static str {
        match self {
            DescriptorUse::Stratification => "a stratification variable",
            DescriptorUse::MechanisticVariable => "a mechanistic variable",
        }
    }
}

/// Whether a descriptor may be used in this role.
pub fn use_descriptor(
    descriptor: PopulationDescriptor,
    use_: DescriptorUse,
) -> Result<(), ShiftRefusal> {
    if use_ == DescriptorUse::MechanisticVariable && descriptor.is_administrative() {
        return Err(ShiftRefusal::DescriptorUsedAsMechanism {
            descriptor: descriptor.as_str().to_string(),
            use_: use_.as_str().to_string(),
        });
    }
    Ok(())
}

/// An interval a caller computed. This crate does not compute one.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct UncertaintyInterval {
    pub low: f64,
    pub high: f64,
}

/// One subgroup's result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SubgroupResult {
    pub subgroup: String,
    pub n: usize,
    pub estimate: f64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub interval: Option<UncertaintyInterval>,
}

/// A subgroup result that carries its own instability.
///
/// No public constructor; produced only by [`subgroup_claim`].
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct SubgroupClaim {
    result: SubgroupResult,
}

impl SubgroupClaim {
    pub fn result(&self) -> &SubgroupResult {
        &self.result
    }
}

/// Whether a subgroup result may be published as a claim.
pub fn subgroup_claim(result: SubgroupResult) -> Result<SubgroupClaim, ShiftRefusal> {
    if result.n == 0 {
        return Err(ShiftRefusal::EmptySubgroup {
            subgroup: result.subgroup,
        });
    }
    if result.interval.is_none() {
        return Err(ShiftRefusal::UnquantifiedSubgroup {
            subgroup: result.subgroup,
            n: result.n,
        });
    }
    Ok(SubgroupClaim { result })
}

/// A pooled score and whatever breakdown accompanies it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PooledScore {
    pub value: f64,
    #[serde(default)]
    pub subgroups: Vec<SubgroupResult>,
}

/// A report that shows every subgroup, not just the pooled number.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct EquityReport {
    pooled: f64,
    subgroups: Vec<SubgroupClaim>,
}

impl EquityReport {
    pub fn pooled(&self) -> f64 {
        self.pooled
    }

    pub fn subgroups(&self) -> &[SubgroupClaim] {
        &self.subgroups
    }
}

/// Whether a pooled score supports an equity claim.
///
/// "Hiding poor groups in pooled scores" is the first failure 30.27 names, and a pooled number with
/// no breakdown is that failure in one field.
pub fn equity_report(pooled: PooledScore) -> Result<EquityReport, ShiftRefusal> {
    if pooled.subgroups.is_empty() {
        return Err(ShiftRefusal::PooledScoreOnly);
    }
    let mut subgroups = Vec::new();
    for result in pooled.subgroups {
        subgroups.push(subgroup_claim(result)?);
    }
    Ok(EquityReport {
        pooled: pooled.value,
        subgroups,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn version(name: &str) -> ClassificationVersion {
        ClassificationVersion::new(name)
    }

    fn cohort(name: &str, version_name: &str, labels: &[&str]) -> Cohort {
        let mut cohort = Cohort::new(name, "site A", version(version_name));
        for label in labels {
            cohort = cohort.containing(EntityLabel::new(*label));
        }
        cohort
    }

    #[test]
    fn a_cohort_spanning_a_criteria_revision_is_not_one_cohort() {
        let historical = cohort("historical", "criteria-A", &["entity-1"]);
        let current = cohort("current", "criteria-B", &["entity-1a"]);
        let refusal = comparable_cohorts(&historical, &current, None).unwrap_err();
        assert!(matches!(
            refusal,
            ShiftRefusal::UnmappedClassificationChange { .. }
        ));
    }

    #[test]
    fn a_stated_mapping_makes_the_comparison_available() {
        let historical = cohort("historical", "criteria-A", &["entity-1"]);
        let current = cohort("current", "criteria-B", &["entity-1a"]);
        let mapping = EntityMapping::new(version("criteria-A"), version("criteria-B")).mapping(
            EntityLabel::new("entity-1"),
            LabelFate::Renamed {
                to: EntityLabel::new("entity-1a"),
            },
        );
        assert!(comparable_cohorts(&historical, &current, Some(&mapping)).is_ok());
    }

    #[test]
    fn a_mapping_that_misses_a_label_present_in_the_older_cohort_is_refused_by_name() {
        let historical = cohort("historical", "criteria-A", &["entity-1", "entity-2"]);
        let current = cohort("current", "criteria-B", &["entity-1a"]);
        let mapping = EntityMapping::new(version("criteria-A"), version("criteria-B")).mapping(
            EntityLabel::new("entity-1"),
            LabelFate::Renamed {
                to: EntityLabel::new("entity-1a"),
            },
        );
        assert_eq!(
            comparable_cohorts(&historical, &current, Some(&mapping)).unwrap_err(),
            ShiftRefusal::IncompleteMapping {
                entity: "entity-2".to_string(),
                version: "criteria-A".to_string()
            }
        );
    }

    #[test]
    fn a_mapping_between_other_versions_does_not_bridge_these_two() {
        let historical = cohort("historical", "criteria-A", &["entity-1"]);
        let current = cohort("current", "criteria-B", &["entity-1a"]);
        let unrelated = EntityMapping::new(version("criteria-C"), version("criteria-D"));
        assert!(matches!(
            comparable_cohorts(&historical, &current, Some(&unrelated)).unwrap_err(),
            ShiftRefusal::UnmappedClassificationChange { .. }
        ));
    }

    #[test]
    fn a_split_label_does_not_resolve_to_one_successor() {
        let split = LabelFate::Split {
            into: [EntityLabel::new("entity-1a"), EntityLabel::new("entity-1b")]
                .into_iter()
                .collect(),
        };
        assert!(!split.resolves_to_one_label());
        assert!(LabelFate::Renamed {
            to: EntityLabel::new("entity-1a")
        }
        .resolves_to_one_label());
        assert!(!LabelFate::NoEquivalent.resolves_to_one_label());
    }

    #[test]
    fn cohorts_under_one_version_need_no_mapping() {
        let left = cohort("left", "criteria-A", &["entity-1"]);
        let right = cohort("right", "criteria-A", &["entity-1"]);
        assert!(comparable_cohorts(&left, &right, None).is_ok());
    }

    #[test]
    fn an_unavailable_assay_at_a_site_is_not_a_negative_result() {
        let context = SiteAssayContext::new(
            "a site without the platform",
            "a molecular assay",
            AssayAvailability::UnavailableAtSite,
        );
        assert_eq!(
            context.observation(),
            Observed::Unobserved(ObservationStatus::NotCollected)
        );
        assert!(matches!(
            as_negative_call(&context).unwrap_err(),
            ShiftRefusal::ResourceAbsenceReadAsBiology { .. }
        ));
    }

    #[test]
    fn self_reported_race_may_stratify_a_report_but_not_explain_a_mechanism() {
        assert!(use_descriptor(
            PopulationDescriptor::SelfReportedRaceOrEthnicity,
            DescriptorUse::Stratification
        )
        .is_ok());
        assert!(matches!(
            use_descriptor(
                PopulationDescriptor::SelfReportedRaceOrEthnicity,
                DescriptorUse::MechanisticVariable
            )
            .unwrap_err(),
            ShiftRefusal::DescriptorUsedAsMechanism { .. }
        ));
    }

    #[test]
    fn site_and_resource_setting_are_administrative_categories_too() {
        for descriptor in [
            PopulationDescriptor::GeographicSite,
            PopulationDescriptor::ResourceSetting,
        ] {
            assert!(descriptor.is_administrative());
            assert!(use_descriptor(descriptor, DescriptorUse::MechanisticVariable).is_err());
        }
        assert!(!PopulationDescriptor::GeneticAncestry.is_administrative());
        assert!(use_descriptor(
            PopulationDescriptor::GeneticAncestry,
            DescriptorUse::MechanisticVariable
        )
        .is_ok());
    }

    #[test]
    fn a_subgroup_claim_without_an_interval_is_refused_whatever_its_size() {
        let large = SubgroupResult {
            subgroup: "a large subgroup".to_string(),
            n: 4_000,
            estimate: 0.81,
            interval: None,
        };
        assert!(matches!(
            subgroup_claim(large).unwrap_err(),
            ShiftRefusal::UnquantifiedSubgroup { .. }
        ));
    }

    #[test]
    fn a_tiny_subgroup_with_an_interval_is_publishable_because_the_interval_shows_it() {
        let tiny = SubgroupResult {
            subgroup: "a rare subgroup".to_string(),
            n: 3,
            estimate: 1.0,
            interval: Some(UncertaintyInterval {
                low: 0.29,
                high: 1.0,
            }),
        };
        let claim = subgroup_claim(tiny).expect("the interval carries the instability");
        assert_eq!(claim.result().n, 3);
    }

    #[test]
    fn an_empty_subgroup_is_refused_before_the_interval_is_examined() {
        let empty = SubgroupResult {
            subgroup: "a subgroup with no cases".to_string(),
            n: 0,
            estimate: f64::NAN,
            interval: None,
        };
        assert!(matches!(
            subgroup_claim(empty).unwrap_err(),
            ShiftRefusal::EmptySubgroup { .. }
        ));
    }

    #[test]
    fn a_pooled_score_alone_does_not_support_an_equity_claim() {
        let pooled = PooledScore {
            value: 0.91,
            subgroups: Vec::new(),
        };
        assert_eq!(equity_report(pooled).unwrap_err(), ShiftRefusal::PooledScoreOnly);
    }

    #[test]
    fn an_equity_report_carries_every_subgroup_including_the_poor_one() {
        let pooled = PooledScore {
            value: 0.91,
            subgroups: vec![
                SubgroupResult {
                    subgroup: "the large group".to_string(),
                    n: 900,
                    estimate: 0.93,
                    interval: Some(UncertaintyInterval {
                        low: 0.91,
                        high: 0.95,
                    }),
                },
                SubgroupResult {
                    subgroup: "the small group".to_string(),
                    n: 12,
                    estimate: 0.55,
                    interval: Some(UncertaintyInterval {
                        low: 0.28,
                        high: 0.80,
                    }),
                },
            ],
        };
        let report = equity_report(pooled).expect("both subgroups carry intervals");
        assert_eq!(report.subgroups().len(), 2);
        assert_eq!(report.pooled(), 0.91);
    }
}
