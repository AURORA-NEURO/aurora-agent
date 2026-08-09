//! 36.13: strata as context, and coverage that carries its own gaps.

use bioprism_bioethics::representation::{
    attribute, summarise, Attribution, ContextAxis, Stratum, StratumCoverage, StratumObservation,
};
use bioprism_bioethics::BioethicsError;
use bioprism_policy::redaction::SmallCellRule;
use std::collections::BTreeSet;

fn axes(items: impl IntoIterator<Item = ContextAxis>) -> BTreeSet<ContextAxis> {
    items.into_iter().collect()
}

fn every_resource_axis() -> BTreeSet<ContextAxis> {
    ContextAxis::ALL
        .into_iter()
        .filter(|axis| axis.is_resource_context())
        .collect()
}

#[test]
fn three_of_the_six_scope_axes_describe_access_to_measurement() {
    assert_eq!(every_resource_axis().len(), 3);
    for axis in [
        ContextAxis::SiteResources,
        ContextAxis::ScannerAndLaboratoryAvailability,
        ContextAxis::FollowUpAndReferralPatterns,
    ] {
        assert!(axis.is_resource_context(), "{axis} describes access");
    }
    for axis in [
        ContextAxis::AncestryAndPopulationStructure,
        ContextAxis::AgeAndSex,
        ContextAxis::Geography,
    ] {
        assert!(!axis.is_resource_context());
    }
}

#[test]
fn a_difference_is_not_attributable_while_a_resource_axis_is_unmatched() {
    let attribution = attribute(
        ContextAxis::AncestryAndPopulationStructure,
        &axes([ContextAxis::SiteResources]),
    );
    let Attribution::Unattributable { unmatched } = attribution.clone() else {
        panic!("two sites without the same instruments cannot support an ancestry claim");
    };
    assert!(unmatched.contains(&ContextAxis::ScannerAndLaboratoryAvailability));
    assert!(unmatched.contains(&ContextAxis::FollowUpAndReferralPatterns));

    let error = attribution
        .require_context("worse recall in one stratum")
        .expect_err("the refusal names the axes that were not held constant");
    assert!(matches!(
        error,
        BioethicsError::ResourceContextUnmatched { .. }
    ));
}

#[test]
fn a_difference_is_attributable_once_every_resource_axis_is_matched() {
    let attribution = attribute(
        ContextAxis::AncestryAndPopulationStructure,
        &every_resource_axis(),
    );
    assert_eq!(
        attribution
            .require_context("worse recall in one stratum")
            .expect("the comparison held the resource context constant"),
        ContextAxis::AncestryAndPopulationStructure
    );
}

#[test]
fn comparing_across_a_resource_axis_does_not_require_matching_that_axis_against_itself() {
    let matched = axes([
        ContextAxis::ScannerAndLaboratoryAvailability,
        ContextAxis::FollowUpAndReferralPatterns,
    ]);
    assert_eq!(
        attribute(ContextAxis::SiteResources, &matched),
        Attribution::ToContext {
            axis: ContextAxis::SiteResources
        },
        "a study about site resources may vary site resources"
    );
}

#[test]
fn a_summary_carries_its_unmeasured_strata_beside_its_measured_ones() {
    let summary = summarise(
        "recall-by-site",
        [
            StratumObservation::new(
                Stratum::new(ContextAxis::Geography, "region-a"),
                StratumCoverage::Measured,
            ),
            StratumObservation::new(
                Stratum::new(ContextAxis::Geography, "region-b"),
                StratumCoverage::Unmeasured,
            ),
        ],
    )
    .expect("no stratum is duplicated");

    assert_eq!(summary.measured().len(), 1);
    assert_eq!(summary.unmeasured().len(), 1);
    assert!(!summary.is_complete());
    assert!(summary.incomplete_axes().contains(&ContextAxis::Geography));
}

#[test]
fn a_suppressed_stratum_is_neither_measured_nor_unmeasured() {
    let rule = SmallCellRule::new(5);
    let suppressed = StratumCoverage::from_cell(&rule, 2);
    assert!(matches!(
        suppressed,
        StratumCoverage::SuppressedSmallGroup { below: 5 }
    ));
    assert!(!suppressed.is_measured());
    assert!(StratumCoverage::from_cell(&rule, 40).is_measured());

    let summary = summarise(
        "recall-by-site",
        [StratumObservation::new(
            Stratum::new(ContextAxis::AgeAndSex, "youngest band"),
            suppressed,
        )],
    )
    .expect("no duplicates");
    assert_eq!(summary.suppressed().len(), 1);
    assert!(summary.measured().is_empty());
    assert!(summary.unmeasured().is_empty());
    assert!(
        !summary.is_complete(),
        "a withheld cell is not coverage the reader can inspect"
    );
}

#[test]
fn the_small_cell_threshold_comes_from_the_policy_rule_rather_than_from_this_crate() {
    let strict = SmallCellRule::new(20);
    let permissive = SmallCellRule::new(2);
    assert!(!StratumCoverage::from_cell(&strict, 11).is_measured());
    assert!(StratumCoverage::from_cell(&permissive, 11).is_measured());
}

#[test]
fn a_duplicated_stratum_is_refused_rather_than_silently_partitioned_twice() {
    let error = summarise(
        "recall-by-site",
        [
            StratumObservation::new(
                Stratum::new(ContextAxis::Geography, "region-a"),
                StratumCoverage::Measured,
            ),
            StratumObservation::new(
                Stratum::new(ContextAxis::Geography, "region-a"),
                StratumCoverage::Unmeasured,
            ),
        ],
    )
    .expect_err("the same stratum cannot be both measured and unmeasured");
    assert!(matches!(error, BioethicsError::DuplicateStratum { .. }));
}

#[test]
fn a_complete_summary_reports_no_incomplete_axis() {
    let summary = summarise(
        "recall-by-site",
        ContextAxis::ALL.into_iter().map(|axis| {
            StratumObservation::new(
                Stratum::new(axis, "only stratum"),
                StratumCoverage::Measured,
            )
        }),
    )
    .expect("one stratum per axis");
    assert!(summary.is_complete());
    assert!(summary.incomplete_axes().is_empty());
    assert_eq!(summary.subject(), "recall-by-site");
}
