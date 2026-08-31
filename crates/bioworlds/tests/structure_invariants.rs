//! Invariants of the structural measurements themselves.
//!
//! A characterisation nobody checks is a number, not a measurement. These tests fix the direction
//! each knob moves its statistic, so a future edit that silently stops varying structure fails
//! here rather than producing a table of plausible-looking constants.

use bioprism_bioworlds::structure::{profile, DependencyClosure, Neighbourhood};
use bioprism_bioworlds::{temporal, BioWorld, BioWorldError, WorldBuilder};
use bioprism_worldgen::spec::{DistractorAttachment, TagStyle};
use serde_json::json;

fn measure(spec: &temporal::TemporalFirewallSpec) -> bioprism_bioworlds::StructuralProfile {
    let world = temporal::build(spec).expect("builds");
    profile(&world, &temporal::query(spec), temporal::DISTRACTOR_TAG).expect("profile")
}

#[test]
fn near_target_attachment_removes_the_separating_depth_that_hub_attachment_leaves() {
    let hub = measure(&temporal::TemporalFirewallSpec::reference_shaped());
    let near = measure(&temporal::TemporalFirewallSpec::discriminating());
    assert_eq!(hub.separating_depth, Some(5));
    assert_eq!(near.separating_depth, None);
}

#[test]
fn a_world_with_no_distractors_always_has_a_separating_depth() {
    let mut spec = temporal::TemporalFirewallSpec::discriminating();
    spec.distractors = 0;
    let measured = measure(&spec);
    assert_eq!(measured.distractor_facts, 0);
    assert_eq!(
        measured.separating_depth, measured.max_hops_to_a_decisive_fact,
        "with nothing to exclude, the smallest sound radius is the furthest decisive fact"
    );
}

#[test]
fn raising_relay_depth_pushes_the_decisive_facts_further_from_the_target() {
    let mut shallow = temporal::TemporalFirewallSpec::discriminating();
    shallow.relay_depth = 0;
    let mut deep = temporal::TemporalFirewallSpec::discriminating();
    deep.relay_depth = 5;

    let shallow = measure(&shallow);
    let deep = measure(&deep);
    assert!(
        deep.max_hops_to_a_decisive_fact > shallow.max_hops_to_a_decisive_fact,
        "relays must move the decisive facts without changing what they mean: {:?} vs {:?}",
        shallow.max_hops_to_a_decisive_fact,
        deep.max_hops_to_a_decisive_fact
    );
    assert_eq!(deep.decisive_facts, shallow.decisive_facts);
}

#[test]
fn camouflaged_tags_raise_the_camouflage_fraction_and_distinct_tags_leave_it_at_zero() {
    let mut camouflaged = temporal::TemporalFirewallSpec::discriminating();
    camouflaged.tag_style = TagStyle::Camouflaged;
    let mut distinct = temporal::TemporalFirewallSpec::discriminating();
    distinct.tag_style = TagStyle::Distinct;

    assert_eq!(measure(&camouflaged).tag_camouflage_fraction, 1.0);
    assert_eq!(measure(&distinct).tag_camouflage_fraction, 0.0);
}

#[test]
fn a_camouflaged_tag_is_not_itself_a_protected_tag() {
    let spec = temporal::TemporalFirewallSpec::discriminating();
    let world = temporal::build(&spec).expect("builds");
    let query = temporal::query(&spec);

    for fact in world
        .world()
        .facts
        .iter()
        .filter(|fact| fact.has_tag(temporal::DISTRACTOR_TAG))
    {
        for tag in &fact.tags {
            assert!(
                !query.protects(tag),
                "camouflage must defeat lexical scoring without entering the closure; {tag} does both"
            );
        }
    }
}

#[test]
fn the_protected_and_unprotected_facts_partition_the_world() {
    let measured = measure(&temporal::TemporalFirewallSpec::discriminating());
    assert_eq!(
        measured.protected_facts + measured.unprotected_facts,
        measured.facts
    );
    assert!(measured.protected_facts > 0);
    assert!(measured.unprotected_facts > 0);
}

#[test]
fn the_elimination_width_is_at_least_the_largest_factor_arity_minus_one() {
    for spec in [
        temporal::TemporalFirewallSpec::discriminating(),
        temporal::TemporalFirewallSpec::reference_shaped(),
    ] {
        let measured = measure(&spec);
        assert!(
            measured.elimination_width + 1 >= measured.max_factor_arity,
            "a clique over a factor's inputs forces at least arity-1 width: {} vs {}",
            measured.elimination_width,
            measured.max_factor_arity
        );
    }
}

#[test]
fn the_dependency_closure_is_directed_and_excludes_downstream_distractors() {
    let spec = temporal::TemporalFirewallSpec::discriminating();
    let world = temporal::build(&spec).expect("builds");
    let closure = DependencyClosure::of_target(world.world(), temporal::TARGET);

    let distractor_variables: Vec<&str> = world
        .world()
        .facts
        .iter()
        .filter(|fact| fact.has_tag(temporal::DISTRACTOR_TAG))
        .map(|fact| fact.provides.as_str())
        .collect();
    for variable in distractor_variables {
        assert!(
            !closure.depends_on(variable),
            "{variable} is downstream of the target and must not enter its backward closure"
        );
    }
}

#[test]
fn the_undirected_neighbourhood_reaches_distractors_the_directed_closure_does_not() {
    let spec = temporal::TemporalFirewallSpec::discriminating();
    let world = temporal::build(&spec).expect("builds");
    let hops = Neighbourhood::from_target(world.world(), temporal::TARGET);
    let reachable = hops.facts_within(usize::MAX);
    let distractors = world
        .world()
        .facts
        .iter()
        .filter(|fact| fact.has_tag(temporal::DISTRACTOR_TAG))
        .count();
    assert!(
        reachable.len() > distractors,
        "an undirected walk reaches the distractors, which is precisely why attachment matters"
    );
}

#[test]
fn an_unknown_target_is_a_typed_error_rather_than_an_empty_profile() {
    let spec = temporal::TemporalFirewallSpec::discriminating();
    let world = temporal::build(&spec).expect("builds");
    let mut query = temporal::query(&spec);
    query.targets = vec!["no_such_variable".into()];
    assert!(matches!(
        profile(&world, &query, temporal::DISTRACTOR_TAG),
        Err(BioWorldError::UnknownTarget { .. })
    ));
}

#[test]
fn a_query_with_several_targets_is_refused_rather_than_silently_using_the_first() {
    let spec = temporal::TemporalFirewallSpec::discriminating();
    let world = temporal::build(&spec).expect("builds");
    let mut query = temporal::query(&spec);
    query.targets = vec![temporal::TARGET.into(), "another_target".into()];
    assert!(matches!(
        profile(&world, &query, temporal::DISTRACTOR_TAG),
        Err(BioWorldError::UnknownTarget { .. })
    ));
}

#[test]
fn an_unparseable_decision_time_is_a_typed_error() {
    let spec = temporal::TemporalFirewallSpec::discriminating();
    let world = temporal::build(&spec).expect("builds");
    let mut query = temporal::query(&spec);
    query.decision_time = "the day before yesterday".into();
    assert!(matches!(
        profile(&world, &query, temporal::DISTRACTOR_TAG),
        Err(BioWorldError::Timestamp { .. })
    ));
}

#[test]
fn a_factor_referring_to_an_undefined_variable_is_refused_by_the_reference_checks() {
    let mut builder = WorldBuilder::new("broken-v1", "a factor with a dangling input", "X-001");
    builder
        .fact("fact.a", "a", json!(1), &["protected"], &["src"])
        .factor("factor.f", &["a", "missing"], &["t"], "rule", &[], 1.0);
    assert!(matches!(
        builder.build(),
        Err(BioWorldError::WorldRejected { .. })
    ));
}

#[test]
fn a_non_finite_factor_cost_is_refused_before_world_validation() {
    let mut builder = WorldBuilder::new("broken-cost-v1", "a non-finite factor", "X-001");
    builder.factor("factor.nan", &[], &[], "rule", &[], f64::NAN);

    let Err(BioWorldError::WorldRejected { message, .. }) = builder.build() else {
        panic!("non-finite factor costs must be rejected");
    };
    assert!(message.contains("factor.nan"), "{message}");
    assert!(message.contains("non-finite cost"), "{message}");
}

#[test]
fn a_document_that_is_not_fiber_world_0_1_is_refused() {
    let refused = BioWorld::from_document(json!({
        "schema_version": "fiber-world/0.2",
        "world_id": "w",
        "facts": [],
        "factors": [],
        "events": []
    }));
    assert!(matches!(refused, Err(BioWorldError::WorldRejected { .. })));
}

#[test]
fn the_worlds_are_deterministic_across_rebuilds() {
    for spec in [
        temporal::TemporalFirewallSpec::discriminating(),
        temporal::TemporalFirewallSpec::reference_shaped(),
    ] {
        let first = temporal::build(&spec).expect("builds");
        let second = temporal::build(&spec).expect("builds");
        assert_eq!(first.document(), second.document());
        assert_eq!(
            first.digest().expect("digest"),
            second.digest().expect("digest")
        );
    }
}

#[test]
fn a_different_seed_changes_the_distractors_and_leaves_the_decisive_skeleton_alone() {
    let mut other = temporal::TemporalFirewallSpec::discriminating();
    other.seed = 7;
    let base = measure(&temporal::TemporalFirewallSpec::discriminating());
    let reseeded = measure(&other);

    assert_eq!(base.decisive_facts, reseeded.decisive_facts);
    assert_eq!(base.decisive_variables, reseeded.decisive_variables);
    assert_eq!(base.separating_depth, reseeded.separating_depth);
    assert_ne!(
        temporal::build(&temporal::TemporalFirewallSpec::discriminating())
            .expect("builds")
            .digest()
            .expect("digest"),
        temporal::build(&other)
            .expect("builds")
            .digest()
            .expect("digest")
    );
}

#[test]
fn both_shipped_temporal_specs_sit_at_the_blueprint_cohort_scale() {
    assert!(temporal::TemporalFirewallSpec::discriminating().is_at_cohort_scale());
    assert!(temporal::TemporalFirewallSpec::reference_shaped().is_at_cohort_scale());
}

#[test]
fn hub_and_near_target_attachment_place_the_distractors_at_different_distances() {
    let hub = measure(&temporal::TemporalFirewallSpec::reference_shaped());
    let mut near = temporal::TemporalFirewallSpec::reference_shaped();
    near.attachment = DistractorAttachment::NearTarget;
    let near = measure(&near);

    assert!(
        near.min_hops_to_a_distractor_fact < hub.min_hops_to_a_distractor_fact,
        "near-target attachment must bring distractors closer: {:?} vs {:?}",
        near.min_hops_to_a_distractor_fact,
        hub.min_hops_to_a_distractor_fact
    );
}
