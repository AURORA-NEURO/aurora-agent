//! The deliverable for the blocked `non_protected_temporal_withholding` property.
//!
//! `crates/examples` records the obstacle as: *"in the generated family every event-managed
//! variable is also protected, so an early cut cannot withhold evidence without simultaneously
//! breaking the closure; separating them needs an event over a non-protected variable that the
//! target depends on."* These tests assert that the world built here has exactly that event, and
//! that the generated family does not.
//!
//! None of them compiles a query. What they establish is that the property is now *exercisable*:
//! a compiler pointed at this world can withhold real evidence with the closure intact, which was
//! not previously possible against any world in the workspace.

use bioprism_bioworlds::structure::{profile, DependencyClosure};
use bioprism_bioworlds::{query, temporal, BioWorld, QueryShape};
use std::path::PathBuf;

fn firewall() -> (BioWorld, QueryShape) {
    let spec = temporal::TemporalFirewallSpec::discriminating();
    (
        temporal::build(&spec).expect("the firewall world builds"),
        temporal::query(&spec),
    )
}

fn repo_fixture(relative: &str) -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

fn generated_family_query() -> QueryShape {
    QueryShape {
        query_id: "generated-split-integrity".into(),
        targets: vec!["split_integrity_status".into()],
        protected_tags: query::tag_set(&[
            "identity",
            "split",
            "site",
            "scanner",
            "time",
            "specimen",
            "preprocessing",
            "policy",
            "negative_evidence",
            "protected",
        ]),
        decision_time: "2025-01-01T00:00:00Z".into(),
        max_facts: 64,
        max_tokens: 6000,
        role: "research-auditor".into(),
        policy: vec!["research-only".into()],
    }
}

#[test]
fn the_withheld_variable_is_reachable_from_the_target_and_is_not_protected() {
    let (world, query) = firewall();
    let closure = DependencyClosure::of_target(world.world(), temporal::TARGET);
    assert!(
        closure.depends_on(temporal::CENTRAL_LAB_CONFIRMATION),
        "the target must depend on the withheld variable, or withholding it withholds nothing"
    );

    let protected_tags: Vec<&String> = world
        .world()
        .facts
        .iter()
        .filter(|fact| fact.provides.as_str() == temporal::CENTRAL_LAB_CONFIRMATION)
        .flat_map(|fact| fact.tags.iter())
        .filter(|tag| query.protects(tag))
        .collect();
    assert!(
        protected_tags.is_empty(),
        "the withheld variable must carry no protected tag, found {protected_tags:?}"
    );
}

#[test]
fn the_withheld_variable_is_event_managed_and_its_event_is_released_after_the_cut() {
    let (world, query) = firewall();
    let measured = profile(&world, &query, temporal::DISTRACTOR_TAG).expect("profile");

    assert!(measured
        .temporal
        .event_managed
        .iter()
        .any(|v| v == temporal::CENTRAL_LAB_CONFIRMATION));
    assert!(measured
        .temporal
        .withheld
        .iter()
        .any(|v| v == temporal::CENTRAL_LAB_CONFIRMATION));
    assert_eq!(
        measured.temporal.withheld_not_protected_and_decisive,
        vec![
            temporal::CENTRAL_LAB_CONFIRMATION.to_string(),
            temporal::PROTOCOL_AMENDMENT_TEXT.to_string()
        ]
    );
}

#[test]
fn the_event_time_of_the_withheld_result_precedes_the_cut_while_its_release_does_not() {
    let (world, _) = firewall();
    let event = world
        .world()
        .events
        .iter()
        .find(|event| event.id.as_str() == "event.central_lab_release")
        .expect("the central lab release event exists");
    assert!(
        event.event_time < event.availability_time,
        "the specimen is assayed before the cut and released after it; collapsing the two fields \
         is the temporal-leakage bug 43.09 names"
    );
    assert!(!event.is_backdated());
}

#[test]
fn the_protected_closure_survives_the_temporal_cut() {
    let (world, query) = firewall();
    let measured = profile(&world, &query, temporal::DISTRACTOR_TAG).expect("profile");
    assert!(
        measured.temporal.protected_closure_survives_the_cut(),
        "withheld and protected: {:?}",
        measured.temporal.withheld_and_protected
    );
    assert!(measured.protected_facts > 0, "an empty closure survives trivially");
}

#[test]
fn a_second_withheld_variable_reaches_the_target_through_a_different_check() {
    let (world, _) = firewall();
    let inner = world.world();

    let consumers = |variable: &str| -> Vec<String> {
        inner
            .factors
            .iter()
            .filter(|factor| factor.inputs.iter().any(|input| input.as_str() == variable))
            .map(|factor| factor.id.as_str().to_string())
            .collect()
    };

    let lab = consumers(temporal::CENTRAL_LAB_CONFIRMATION);
    let amendment = consumers(temporal::PROTOCOL_AMENDMENT_TEXT);
    assert_eq!(lab, vec!["factor.lab_window_check".to_string()]);
    assert_eq!(amendment, vec!["factor.protocol_version_check".to_string()]);
}

#[test]
fn an_event_managed_unprotected_variable_released_before_the_cut_stays_readable() {
    let (world, query) = firewall();
    let measured = profile(&world, &query, temporal::DISTRACTOR_TAG).expect("profile");
    assert!(measured
        .temporal
        .event_managed_and_not_protected
        .iter()
        .any(|v| v == temporal::LOCAL_LAB_VALUE));
    assert!(
        !measured.temporal.withheld.contains(&temporal::LOCAL_LAB_VALUE.to_string()),
        "withholding must follow the release schedule, not the tag vocabulary"
    );
}

#[test]
fn every_protected_variable_is_unmanaged_or_released_at_or_before_the_cut() {
    let (world, query) = firewall();
    let measured = profile(&world, &query, temporal::DISTRACTOR_TAG).expect("profile");
    for variable in &measured.temporal.event_managed_and_protected {
        assert!(
            !measured.temporal.withheld.contains(variable),
            "{variable} is protected and withheld, which conflates the two failures"
        );
    }
}

#[test]
fn moving_the_cut_before_the_screening_release_breaks_the_closure_as_well() {
    let (world, mut query) = firewall();
    query.decision_time = "2025-01-01T00:00:00Z".into();
    let measured = profile(&world, &query, temporal::DISTRACTOR_TAG).expect("profile");
    assert!(
        !measured.temporal.protected_closure_survives_the_cut(),
        "an early cut should reproduce the generated family's failure mode"
    );
    assert!(measured
        .temporal
        .withheld_and_protected
        .contains(&"screening_decision_time".to_string()));
}

#[test]
fn the_generated_family_withholds_only_a_variable_the_target_does_not_depend_on() {
    let text = repo_fixture("fixtures/generated/discriminating_world.json");
    let world = BioWorld::from_json_str(&text).expect("the generated world loads");
    let measured = profile(&world, &generated_family_query(), "exploratory").expect("profile");

    assert_eq!(
        measured.temporal.withheld_and_not_protected,
        vec!["future_label_value".to_string()],
        "the generated family does have a non-protected withheld variable"
    );
    assert!(
        measured
            .temporal
            .withheld_not_protected_and_decisive
            .is_empty(),
        "and the target does not depend on it, so withholding it withholds nothing — which is a \
         sharper statement of the recorded blocker than the blocker itself makes"
    );
}

#[test]
fn the_shipped_reference_world_has_the_same_gap_as_the_generated_family() {
    let text = repo_fixture("fixtures/fiber-v0.1/radiogenomic_world.json");
    let world = BioWorld::from_json_str(&text).expect("the reference world loads");
    let measured = profile(&world, &generated_family_query(), "exploratory").expect("profile");
    assert!(measured
        .temporal
        .withheld_not_protected_and_decisive
        .is_empty());
    assert_eq!(
        measured.temporal.event_managed_and_protected.len(),
        3,
        "the three protected event-managed variables the blocker names"
    );
}

#[test]
fn the_control_world_withholds_the_same_variables_at_a_different_structural_corner() {
    let discriminating = temporal::TemporalFirewallSpec::discriminating();
    let control = temporal::TemporalFirewallSpec::reference_shaped();

    let left = profile(
        &temporal::build(&discriminating).expect("builds"),
        &temporal::query(&discriminating),
        temporal::DISTRACTOR_TAG,
    )
    .expect("profile");
    let right = profile(
        &temporal::build(&control).expect("builds"),
        &temporal::query(&control),
        temporal::DISTRACTOR_TAG,
    )
    .expect("profile");

    assert_eq!(
        left.temporal.withheld_not_protected_and_decisive,
        right.temporal.withheld_not_protected_and_decisive,
        "the temporal claim must not depend on the discrimination knobs"
    );
    assert_ne!(
        left.separating_depth, right.separating_depth,
        "and the discrimination knobs must actually move something"
    );
}
