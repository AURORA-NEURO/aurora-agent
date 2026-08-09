//! The shipped bytes, and the backlog ids they are answering.
//!
//! Two independent things are pinned here.
//!
//! **Fixtures.** `crates/bioworlds/fixtures/` holds each world as committed bytes so a reviewer
//! without a Rust toolchain, or a consumer in another language, can read the world a claim is made
//! about. They are `include_str!`'d, so this compares the *committed* file against a fresh build
//! rather than against itself.
//!
//! **Backlog ids.** This crate names property ids from `crates/examples` as strings, because that
//! crate is not in the dependency set. A string that drifts from the catalogue it refers to is
//! worse than no reference at all, so these tests read `crates/examples/src/property.rs` as text
//! and check the ids are still there. It is a weaker check than a type, and it is the strongest
//! one available without taking the dependency.

use bioprism_bioworlds::catalog::{
    COHORT_SCALE_CONTRACT, NON_PROTECTED_TEMPORAL_WITHHOLDING,
    REFERENCE_WORLD_DOES_NOT_DISCRIMINATE, STRUCTURAL_DISCRIMINATION_NO_USABLE_DEPTH,
    TEMPORAL_ACCESSIBILITY_WITHHOLDS_EVIDENCE, UNDERDETERMINED_ABSTENTION,
};
use bioprism_bioworlds::fixtures::{self, SHIPPED};
use bioprism_bioworlds::knobs::{MissingGeneratorKnob, REUSED_GENERATOR_KNOBS};
use bioprism_bioworlds::{SliceCatalog, IMPLEMENTED_BLUEPRINT_MODULES, UNBUILT_BLUEPRINT_WORLDS};
use bioprism_worldgen::spec::{DistractorAttachment, TagStyle, WorldSpec};
use std::path::PathBuf;

fn examples_property_source() -> String {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../examples/src/property.rs");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()))
}

#[test]
fn every_shipped_world_fixture_matches_a_freshly_built_world_byte_for_byte() {
    let catalogue = SliceCatalog::standard().expect("catalogue builds");
    for (id, world_text, _) in SHIPPED {
        let slice = catalogue.get(id).expect("the slice is registered");
        assert_eq!(
            slice.world().to_pretty_json(),
            world_text,
            "the committed fixture for {id} is stale; rebuild it"
        );
    }
}

#[test]
fn every_shipped_query_fixture_matches_the_slice_query_shape() {
    let catalogue = SliceCatalog::standard().expect("catalogue builds");
    for (id, _, query_text) in SHIPPED {
        let slice = catalogue.get(id).expect("the slice is registered");
        let committed = fixtures::load_query(query_text).expect("the query fixture parses");
        assert_eq!(slice.query().to_document(), committed, "{id}");
    }
}

#[test]
fn a_shipped_fixture_loads_under_the_reference_schema_without_this_crates_builder() {
    for (id, world_text, _) in SHIPPED {
        let world = fixtures::load(world_text).expect("the fixture loads");
        assert!(
            world.world().validate_reference_compat().is_ok(),
            "{id} would be rejected by the reference runtime"
        );
        assert!(!world.world().facts.is_empty());
        assert!(!world.world().events.is_empty());
    }
}

#[test]
fn the_shipped_fixtures_carry_the_four_catalogue_worlds_and_no_others() {
    let catalogue = SliceCatalog::standard().expect("catalogue builds");
    let mut ids: Vec<&str> = SHIPPED.iter().map(|(id, _, _)| *id).collect();
    ids.sort_unstable();
    let mut registered = catalogue.ids();
    registered.sort_unstable();
    assert_eq!(ids, registered);
}

#[test]
fn the_property_ids_this_crate_names_still_exist_in_the_examples_catalogue() {
    let source = examples_property_source();
    for id in [
        NON_PROTECTED_TEMPORAL_WITHHOLDING,
        UNDERDETERMINED_ABSTENTION,
        TEMPORAL_ACCESSIBILITY_WITHHOLDS_EVIDENCE,
        STRUCTURAL_DISCRIMINATION_NO_USABLE_DEPTH,
        REFERENCE_WORLD_DOES_NOT_DISCRIMINATE,
        COHORT_SCALE_CONTRACT,
    ] {
        assert!(
            source.contains(&format!("\"{id}\"")),
            "{id} is no longer a property id in crates/examples"
        );
    }
}

/// The obstacle this crate was built against is gone, because the claim it blocked is now
/// demonstrated.
///
/// This assertion used to pin the blocker's exact wording, so that a reword would fail here rather
/// than silently stranding the worlds built against it. It fired when `crates/examples` gained a
/// slice compiling a protection-separated world: the property moved from blocked to demonstrated
/// and the sentence was removed entirely rather than reworded.
///
/// So the pin inverts. What is worth guarding now is that the property is still *named* and no
/// longer carries an obstacle — if an obstacle reappears, the claim regressed and the worlds here
/// are answering a question that is open again.
#[test]
fn the_obstacle_this_crate_answered_is_gone_because_the_claim_is_demonstrated() {
    let source = examples_property_source();
    assert!(
        source.contains("NonProtectedTemporalWithholding"),
        "the property this crate's worlds were built for is no longer in the registry at all"
    );
    assert!(
        !source
            .contains("withholds a decisive non-protected variable at the cut with an empty dropped_protected"),
        "the blocker sentence is back, so the claim has regressed from demonstrated to blocked and          the worlds here should be re-checked against whatever now blocks it"
    );
}

#[test]
fn the_abstention_blocker_still_names_the_compiler_rather_than_the_world() {
    let source = examples_property_source();
    assert!(source.contains("OracleVerdict::abstain exists in bioprism-section but no path in bioprism-fiber constructs it"));
}

/// Every knob this crate named as missing is now a field `WorldSpec` has.
///
/// This assertion used to run the other way. It listed six fields and failed when `worldgen` gained
/// one, so the gap list could not rot into a stale wishlist — and when the knobs landed it failed
/// on the first, which is the mechanism working rather than a regression. It is inverted here
/// rather than deleted, because the guarantee worth keeping is not "these are missing" but "this
/// crate's account of the generator matches the generator".
///
/// `policy` is a seventh that was never on the original list. It became necessary when
/// `bioprism-fiber` gained a pass screening facts by their policy scope dimension, at which point
/// no generated world could exercise it.
#[test]
fn every_knob_this_crate_named_is_now_a_field_worldgen_has() {
    let spec = serde_json::to_value(WorldSpec::discriminating(1)).expect("WorldSpec serialises");
    let map = spec.as_object().expect("an object");
    for landed in [
        "events",
        "protected_variables",
        "decision_time",
        "skeleton",
        "hypotheses",
        "declared_absent",
        "policy",
    ] {
        assert!(
            map.contains_key(landed),
            "WorldSpec has lost its `{landed}` field; the gap this crate recorded has reopened"
        );
    }
}

#[test]
fn the_knobs_worldgen_does_have_are_the_ones_this_crate_reuses() {
    assert_eq!(REUSED_GENERATOR_KNOBS.len(), 3);
    let _ = DistractorAttachment::NearTarget;
    let _ = TagStyle::Camouflaged;
    let spec = serde_json::to_value(WorldSpec::reference_like(1)).expect("serialises");
    let map = spec.as_object().expect("an object");
    for present in ["attachment", "tag_style", "relay_depth", "seed", "distractors"] {
        assert!(map.contains_key(present), "WorldSpec lost its `{present}` knob");
    }
}

#[test]
fn every_missing_knob_has_a_unique_snake_case_id_a_gap_and_a_proposed_field() {
    let mut seen = std::collections::BTreeSet::new();
    for knob in MissingGeneratorKnob::ALL {
        assert!(seen.insert(knob.id()), "duplicate knob id {}", knob.id());
        assert!(knob
            .id()
            .chars()
            .all(|c| c.is_ascii_lowercase() || c == '_'));
        assert!(knob.gap().len() > 40, "{} states no concrete gap", knob.id());
        assert!(
            knob.proposed_field().contains("WorldSpec"),
            "{} proposes no field on WorldSpec",
            knob.id()
        );
    }
    assert_eq!(seen.len(), 6);
}

#[test]
fn the_crate_names_the_fourteen_blueprint_worlds_it_did_not_build() {
    assert_eq!(IMPLEMENTED_BLUEPRINT_MODULES.len(), 2);
    assert_eq!(UNBUILT_BLUEPRINT_WORLDS.len(), 14);
    for world in UNBUILT_BLUEPRINT_WORLDS {
        assert!(world.starts_with("38."), "{world} is not a §38 world");
    }
    for module in IMPLEMENTED_BLUEPRINT_MODULES {
        assert!(!UNBUILT_BLUEPRINT_WORLDS
            .iter()
            .any(|unbuilt| unbuilt.starts_with(module)));
    }
}
