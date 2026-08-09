//! The distinction the workspace refuses to let slide, expressed as types.
//!
//! `AGENTS.md`: "'Provably cannot matter' and 'nobody checked' are different states and must never
//! share a representation." These tests assert that there is no value of any public type in this
//! crate that blurs them.

use bioprism_influence::{
    manifest, structural_zero, Approximation, BoundMethod, InfluenceAnalysis, InfluenceBound,
    InfluenceEstimate, InfluenceMetric, Perturbation, UnknownReason,
};
use bioprism_section::{InfluenceClass, OmissionManifest};

fn bound(value: f64) -> InfluenceBound {
    InfluenceBound::new(
        value,
        InfluenceMetric::TotalVariationOnNormalisedAnswer,
        BoundMethod::DynamicRange,
        Approximation::ConservativeUpperBound,
        "fixture",
    )
    .expect("value is a total-variation distance")
}

#[test]
fn an_uncomputable_bound_is_unknown_and_not_infinity() {
    let estimate = InfluenceEstimate::Unknown(UnknownReason::NoFactorTable {
        factor: "f".into(),
    });
    assert!(estimate.bound().is_none());
    assert!(manifest::certificate_bound(&estimate).is_none());
    assert!(!estimate.supports_sufficiency());
}

#[test]
fn a_bound_of_infinity_cannot_be_constructed() {
    assert!(InfluenceBound::new(
        f64::INFINITY,
        InfluenceMetric::TotalVariationOnNormalisedAnswer,
        BoundMethod::DynamicRange,
        Approximation::ConservativeUpperBound,
        "fixture",
    )
    .is_err());
}

#[test]
fn a_bound_outside_the_unit_interval_cannot_be_constructed() {
    for value in [-0.001, 1.001, f64::NAN, f64::NEG_INFINITY] {
        assert!(
            InfluenceBound::new(
                value,
                InfluenceMetric::TotalVariationOnNormalisedAnswer,
                BoundMethod::DynamicRange,
                Approximation::ConservativeUpperBound,
                "fixture",
            )
            .is_err(),
            "{value} was accepted as a total-variation bound"
        );
    }
}

#[test]
fn an_unknown_group_carries_no_numeric_bound_on_the_certificate() {
    let estimate = InfluenceEstimate::Unknown(UnknownReason::NotAnalysed);
    let group = manifest::omission_group("nobody checked", 12, &estimate, Vec::new());
    assert_eq!(group.influence, InfluenceClass::Unknown);
    assert_eq!(group.bound, None);
}

#[test]
fn a_computed_zero_maps_to_bounded_rather_than_to_zero() {
    let estimate = InfluenceEstimate::Bounded(bound(0.0));
    let group = manifest::omission_group("a path exists and this perturbation did not use it", 1, &estimate, Vec::new());
    assert_eq!(group.influence, InfluenceClass::Bounded);
    assert_eq!(group.bound, Some(0.0));
}

#[test]
fn a_structural_zero_maps_to_influence_class_zero() {
    let estimate = InfluenceEstimate::Bounded(structural_zero("no path reaches the target").unwrap());
    let group = manifest::omission_group("unreached", 750, &estimate, Vec::new());
    assert_eq!(group.influence, InfluenceClass::Zero);
    assert_eq!(group.bound, Some(0.0));
}

#[test]
fn a_bounded_group_supports_a_sufficiency_claim_and_an_unknown_one_voids_it() {
    let mut sufficient = OmissionManifest::default();
    sufficient.push(manifest::omission_group(
        "bounded",
        3,
        &InfluenceEstimate::Bounded(bound(0.02)),
        Vec::new(),
    ));
    assert!(sufficient.supports_sufficiency_claim());

    sufficient.push(manifest::omission_group(
        "unchecked",
        1,
        &InfluenceEstimate::Unknown(UnknownReason::NotAnalysed),
        Vec::new(),
    ));
    assert!(!sufficient.supports_sufficiency_claim());
    assert_eq!(sufficient.blocking_groups().count(), 1);
}

#[test]
fn the_tightest_of_two_sound_bounds_is_the_smaller_one() {
    let loose = bound(0.4);
    let tight = bound(0.05);
    assert_eq!(loose.clone().tightest(tight.clone()).value(), 0.05);
    assert_eq!(tight.tightest(loose).value(), 0.05);
}

#[test]
fn a_tie_between_two_bounds_keeps_the_first_so_the_winner_is_deterministic() {
    let first = bound(0.3);
    let second = InfluenceBound::new(
        0.3,
        InfluenceMetric::TotalVariationOnNormalisedAnswer,
        BoundMethod::ChainContraction,
        Approximation::ConservativeUpperBound,
        "fixture",
    )
    .unwrap();
    assert_eq!(first.tightest(second).method(), BoundMethod::DynamicRange);
}

#[test]
fn a_vacuous_bound_is_flagged_even_though_it_is_formally_sufficient() {
    let estimate = InfluenceEstimate::Bounded(bound(1.0));
    let group = manifest::omission_group("everything", 5, &estimate, Vec::new());
    assert_eq!(group.influence, InfluenceClass::Bounded);
    assert!(group.influence.supports_sufficiency());
    assert!(!manifest::is_informative(&group));
    assert!(estimate.bound().unwrap().is_vacuous());
}

#[test]
fn a_summary_separates_informative_bounds_from_vacuous_ones() {
    let groups = vec![
        manifest::omission_group("a", 1, &InfluenceEstimate::Bounded(bound(0.1)), Vec::new()),
        manifest::omission_group("b", 1, &InfluenceEstimate::Bounded(bound(0.4)), Vec::new()),
        manifest::omission_group("c", 1, &InfluenceEstimate::Bounded(bound(1.0)), Vec::new()),
        manifest::omission_group(
            "d",
            1,
            &InfluenceEstimate::Unknown(UnknownReason::NotAnalysed),
            Vec::new(),
        ),
    ];
    let summary = manifest::summarise(&groups);
    assert_eq!(summary.bounded_groups, 3);
    assert_eq!(summary.informative_groups, 2);
    assert_eq!(summary.vacuous_groups, 1);
    assert_eq!(summary.unknown_groups, 1);
    assert_eq!(summary.worst_informative_bound, Some(0.4));
}

#[test]
fn the_group_examples_are_capped_like_fibers() {
    let group = manifest::omission_group(
        "many",
        100,
        &InfluenceEstimate::Bounded(bound(0.2)),
        (0..10).map(|index| format!("fact.{index}")),
    );
    assert_eq!(group.examples.len(), manifest::EXAMPLE_LIMIT);
}

#[test]
fn an_estimate_round_trips_through_serde_without_losing_its_kind() {
    for estimate in [
        InfluenceEstimate::Bounded(bound(0.123)),
        InfluenceEstimate::Unknown(UnknownReason::NoFactorTable { factor: "f".into() }),
        InfluenceEstimate::Unknown(UnknownReason::RegionOutsideMethodClass {
            method: "chain_contraction".into(),
            handles: "a Markov chain".into(),
            detail: "the region branches".into(),
        }),
    ] {
        let json = serde_json::to_string(&estimate).unwrap();
        let back: InfluenceEstimate = serde_json::from_str(&json).unwrap();
        assert_eq!(back, estimate, "round trip changed {json}");
    }
}

#[test]
fn an_unknown_reason_serialises_without_a_numeric_field_anywhere() {
    let estimate = InfluenceEstimate::Unknown(UnknownReason::BackendDeclined {
        detail: "width above budget".into(),
    });
    let json = serde_json::to_value(&estimate).unwrap();
    assert!(json.get("bound").is_none());
    assert_eq!(json["kind"], "unknown");
}

#[test]
fn an_exact_method_is_labelled_exact_and_a_structural_one_is_not() {
    assert!(BoundMethod::ExactRemoval.is_exact());
    assert!(BoundMethod::StructuralZero.is_exact());
    assert!(!BoundMethod::DynamicRange.is_exact());
    assert!(!BoundMethod::ChainContraction.is_exact());
    assert!(!BoundMethod::RatioComposition.is_exact());
}

#[test]
fn every_named_gap_states_a_reason_rather_than_only_a_title() {
    assert!(bioprism_influence::NOT_IMPLEMENTED.len() >= 6);
    for (title, reason) in bioprism_influence::NOT_IMPLEMENTED {
        assert!(!title.is_empty());
        assert!(
            reason.len() > 80,
            "the gap {title:?} is named but not explained"
        );
    }
}

#[test]
fn a_widened_bound_and_a_joined_one_are_different_methods_on_the_wire() {
    let joined = InfluenceBound::new(
        0.25,
        InfluenceMetric::TotalVariationOnNormalisedAnswer,
        BoundMethod::AbstractInterpretation,
        Approximation::ConservativeUpperBound,
        "least fixed point under join",
    )
    .unwrap();
    let widened = InfluenceBound::new(
        0.25,
        InfluenceMetric::TotalVariationOnNormalisedAnswer,
        BoundMethod::WidenedAbstractInterpretation,
        Approximation::ConservativeUpperBound,
        "post-fixpoint after widening",
    )
    .unwrap();

    assert_eq!(joined.value(), widened.value());
    assert_ne!(joined.method(), widened.method());
    assert_ne!(joined.method().as_str(), widened.method().as_str());
    assert!(!joined.method().used_widening());
    assert!(widened.method().used_widening());
    assert!(!joined.method().is_exact());
    assert!(!widened.method().is_exact());

    let encoded = serde_json::to_string(&widened).unwrap();
    assert!(encoded.contains("widened_abstract_interpretation"));
    let decoded: InfluenceBound = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded.method(), BoundMethod::WidenedAbstractInterpretation);
}

#[test]
fn a_group_reason_carries_the_widening_all_the_way_onto_the_certificate() {
    let analysis = InfluenceAnalysis {
        subject: vec!["f.c0".to_string()],
        perturbation: Perturbation::Removal,
        estimate: InfluenceEstimate::Bounded(
            InfluenceBound::new(
                0.0475,
                InfluenceMetric::TotalVariationOnNormalisedAnswer,
                BoundMethod::WidenedAbstractInterpretation,
                Approximation::ConservativeUpperBound,
                "post-fixpoint after widening and narrowing",
            )
            .unwrap(),
        ),
        attempted: Vec::new(),
    };
    let group = manifest::omission_group_from_analysis(&analysis, 1, ["fact.one".to_string()]);
    assert_eq!(group.influence, InfluenceClass::Bounded);
    assert!(
        group.reason.contains("widened_abstract_interpretation"),
        "the reason a reader sees must say the bound was widened, and said {:?}",
        group.reason
    );
}
