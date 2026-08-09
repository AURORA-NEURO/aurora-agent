//! Blueprint 35.16, run rather than described.
//!
//! Every number asserted here is produced by [`bioprism_scale::million_scale_example`]. Two of
//! them are the blueprint's own numbers failing to reconcile with each other.

use bioprism_scale::accounting::{blueprint_plan, million_scale_example, Reconciliation, REFERENCE_RHO};
use bioprism_scale::corpus::{Corpus, GeneratedItem};
use bioprism_scale::cost::{CostModel, FactoryPlan};
use bioprism_scale::effective::{EffectiveSize, SimilarityRelation};

#[test]
fn the_blueprints_descendant_count_does_not_follow_from_its_own_structure() {
    let plan = blueprint_plan();
    let reconciliation = Reconciliation::of(&plan, 1_000_000);

    assert_eq!(plan.parent_decisions, 12_000);
    assert_eq!(plan.mutation_families, 35);
    assert_eq!(
        reconciliation.implied_descendants_single_parameterization, 420_000,
        "12,000 decisions × 35 families"
    );
    assert_eq!(reconciliation.stated_descendants, 1_200_000);
    assert!(
        !reconciliation.reconciles,
        "1,200,000 / 420,000 = 2.857…, and no integer number of parameterizations per family \
         reproduces the stated descendant count"
    );
    assert!((reconciliation.required_parameterizations - 2.857_142_857).abs() < 1e-6);
    assert_eq!(reconciliation.descendants_at_two_parameterizations, 840_000);
    assert_eq!(reconciliation.descendants_at_three_parameterizations, 1_260_000);
}

#[test]
fn reaching_one_million_valid_instances_requires_a_yield_the_blueprint_never_states() {
    let reconciliation = Reconciliation::of(&blueprint_plan(), 1_000_000);
    assert!((reconciliation.implied_minimum_yield - 0.833_333).abs() < 1e-5);
    assert!(reconciliation.notes[1].contains("83.3%"));
}

#[test]
fn a_million_nominal_items_deliver_three_orders_of_magnitude_fewer_observations() {
    let accounting = million_scale_example().unwrap();

    assert_eq!(accounting.enumerable.nominal, 1_200_000);
    assert_eq!(
        accounting.enumerable.effective, 420_000,
        "parameterizations multiply the instance count and not the class ceiling"
    );
    assert_eq!(accounting.executed_release.nominal, 100_000);
    assert_eq!(
        accounting.executed_release.effective, 100_000,
        "the executed subset is class-limited by its own size, not by the ceiling"
    );
    assert_eq!(
        accounting.headline.effective, 7_435,
        "100,000 executed instances clustered on 400 parents at rho=0.05"
    );
    assert!(accounting.headline.inflation_ratio > 160.0);
    assert_eq!(accounting.parent_world_floor, 400);
    assert_eq!(
        1_200_000 / accounting.parent_world_floor,
        3_000,
        "the headline is three thousand times the number of independent worlds"
    );
}

#[test]
fn the_headline_effective_size_moves_an_order_of_magnitude_across_plausible_correlations() {
    let accounting = million_scale_example().unwrap();
    let points = &accounting.hierarchical;

    assert_eq!(points.len(), 3);
    assert!(points[0].effective_sample_size > points[1].effective_sample_size);
    assert!(points[1].effective_sample_size > points[2].effective_sample_size);
    assert!(
        points[0].effective_sample_size / points[2].effective_sample_size > 10.0,
        "quoting a single rho without the range hides a factor of ten"
    );
    assert_eq!(points[1].rho, REFERENCE_RHO);
    assert!(points[2].inflation_versus_nominal > 600.0);
}

#[test]
fn cost_per_effective_class_is_reported_beside_cost_per_instance() {
    let accounting = million_scale_example().unwrap();
    let forecast = &accounting.forecast;

    assert!(forecast.compute_per_nominal_instance < 0.1);
    assert!(
        forecast.compute_per_effective_class > 8.0,
        "a paraphrase factory optimizes the first number and worsens the second"
    );

    let encoded = serde_json::to_value(forecast).unwrap();
    assert!(
        encoded.get("effective_size").is_some(),
        "a CostForecast cannot be constructed or serialized without its effective size"
    );
    assert_eq!(encoded["effective_size"]["nominal"], 1_200_000);
}

#[test]
fn expert_review_is_the_binding_resource_at_this_scale() {
    let accounting = million_scale_example().unwrap();
    assert_eq!(accounting.capacity.binding_resource, "expert review");
    assert!(accounting.capacity.expert_epochs > accounting.capacity.execution_epochs);
    assert_eq!(accounting.forecast.expert_minutes, 240_000.0);
    assert_eq!(
        accounting.forecast.executed_instances, 103_000,
        "100,000 release-suite plus 3,000 adaptive CI instances"
    );
}

#[test]
fn a_measured_corpus_agrees_with_the_analytic_class_ceiling() {
    let parents = 40;
    let decisions_per_parent = 3;
    let families = 35;
    let parameterizations = 2;

    let mut corpus = Corpus::new();
    for parent in 0..parents {
        let parent_id = format!("p{parent}");
        corpus
            .insert(GeneratedItem::parent(
                &parent_id,
                format!("d{parent}-0"),
                format!("digest-{parent_id}"),
            ))
            .unwrap();
        for decision in 0..decisions_per_parent {
            for family in 0..families {
                for parameterization in 0..parameterizations {
                    corpus
                        .insert(GeneratedItem::descendant(
                            format!("{parent_id}#d{decision}#f{family}#{parameterization}"),
                            &parent_id,
                            format!("f{family}"),
                            format!("sig-d{decision}"),
                            format!("digest-{parent_id}-{decision}-{family}-{parameterization}"),
                            format!("d{parent}-{decision}"),
                        ))
                        .unwrap();
                }
            }
        }
    }

    let descendants = parents * decisions_per_parent * families * parameterizations;
    let measured =
        EffectiveSize::measure(&corpus, SimilarityRelation::EquivalenceClass).unwrap();

    assert_eq!(measured.nominal, descendants + parents);
    assert_eq!(
        measured.effective,
        parents * decisions_per_parent * families + parents,
        "the second parameterization of every family adds instances and no classes"
    );
    assert!((measured.inflation_ratio - 2.0).abs() < 0.01);

    let analytic = FactoryPlan {
        parent_worlds: parents,
        parent_decisions: parents * decisions_per_parent,
        mutation_families: families,
        parameterizations_per_family: parameterizations,
        enumerable_descendants: descendants,
        release_suite_instances: descendants,
        ci_suite_instances: 0,
    };
    assert_eq!(analytic.implied_descendants(), descendants);
    assert_eq!(
        analytic.class_ceiling(),
        measured.effective - parents,
        "the analytic ceiling and the measured class count agree on materialized data"
    );
}

#[test]
fn the_replay_cache_hit_rate_reduces_execution_but_not_authoring() {
    let plan = blueprint_plan();
    let mut cold = CostModel {
        replay_cache_hit_rate: 0.0,
        ..bioprism_scale::accounting::assumed_cost_model()
    };
    let effective = EffectiveSize::from_counts(SimilarityRelation::EquivalenceClass, 1_200_000, 7_435);
    let cold_forecast = cold.forecast(&plan, effective.clone());

    cold.replay_cache_hit_rate = 0.9;
    let warm_forecast = cold.forecast(&plan, effective);

    assert!(warm_forecast.model_tokens < cold_forecast.model_tokens * 0.2);
    assert_eq!(
        warm_forecast.expert_minutes, cold_forecast.expert_minutes,
        "no cache reduces the expert review that authoring a parent world costs"
    );
    assert_eq!(warm_forecast.storage_bytes, cold_forecast.storage_bytes);
}

#[test]
fn the_headline_sentence_states_the_whole_ladder() {
    let accounting = million_scale_example().unwrap();
    let sentence = accounting.headline_sentence();

    for fragment in ["1200000", "420000", "100000", "400"] {
        assert!(sentence.contains(fragment), "missing {fragment} in: {sentence}");
    }
    assert!(accounting.findings.len() >= 5);
}
