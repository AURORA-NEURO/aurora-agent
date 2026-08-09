//! Calibrating this crate's neighbourhood metric against a number somebody else measured.
//!
//! `docs/FINDINGS.md` reports, from `bioprism context compare` on the shipped reference world:
//! graph-4-hop selects **0** facts, graph-5-hop and graph-6-hop select **11**, graph-7-hop selects
//! all **761**. Those depths are the only external anchor available for
//! [`bioprism_bioworlds::structure`]'s hop convention, and without them the separating depths this
//! crate reports would be self-defined.
//!
//! If this file starts failing, the metric has drifted away from the published measurement and
//! every separating depth in the crate should be distrusted until it is re-anchored.

use bioprism_bioworlds::structure::{profile, Neighbourhood};
use bioprism_bioworlds::{query, BioWorld, QueryShape};
use std::path::PathBuf;

const TARGET: &str = "split_integrity_status";

fn repo_fixture(relative: &str) -> BioWorld {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join(relative);
    let text =
        std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display()));
    BioWorld::from_json_str(&text).expect("the shipped fixture loads")
}

fn reference_query() -> QueryShape {
    QueryShape {
        query_id: "reference-split-integrity".into(),
        targets: vec![TARGET.into()],
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
fn a_four_hop_ball_around_the_reference_target_contains_no_fact() {
    let world = repo_fixture("fixtures/fiber-v0.1/radiogenomic_world.json");
    let hops = Neighbourhood::from_target(world.world(), TARGET);
    assert_eq!(hops.facts_within(4).len(), 0);
}

#[test]
fn five_and_six_hop_balls_contain_exactly_the_eleven_decisive_facts() {
    let world = repo_fixture("fixtures/fiber-v0.1/radiogenomic_world.json");
    let hops = Neighbourhood::from_target(world.world(), TARGET);
    assert_eq!(hops.facts_within(5).len(), 11);
    assert_eq!(hops.facts_within(6).len(), 11);
}

#[test]
fn a_seven_hop_ball_contains_all_seven_hundred_and_sixty_one_facts() {
    let world = repo_fixture("fixtures/fiber-v0.1/radiogenomic_world.json");
    let hops = Neighbourhood::from_target(world.world(), TARGET);
    assert_eq!(hops.facts_within(7).len(), 761);
}

#[test]
fn the_reference_world_has_a_separating_depth_of_five() {
    let world = repo_fixture("fixtures/fiber-v0.1/radiogenomic_world.json");
    let measured = profile(&world, &reference_query(), "exploratory").expect("profile");
    assert_eq!(measured.separating_depth, Some(5));
    assert_eq!(measured.decisive_facts, 11);
    assert_eq!(measured.min_hops_to_a_distractor_fact, Some(7));
}

#[test]
fn the_generated_discriminating_world_has_no_separating_depth() {
    let world = repo_fixture("fixtures/generated/discriminating_world.json");
    let measured = profile(&world, &reference_query(), "exploratory").expect("profile");
    assert_eq!(measured.separating_depth, None);
    assert!(
        measured.min_hops_to_a_distractor_fact < measured.min_hops_to_a_decisive_fact,
        "distractors must sit strictly closer than the decisive facts for no depth to separate"
    );
}

#[test]
fn camouflage_is_zero_on_the_reference_world_and_total_on_the_generated_one() {
    let reference = profile(
        &repo_fixture("fixtures/fiber-v0.1/radiogenomic_world.json"),
        &reference_query(),
        "exploratory",
    )
    .expect("profile");
    let generated = profile(
        &repo_fixture("fixtures/generated/discriminating_world.json"),
        &reference_query(),
        "exploratory",
    )
    .expect("profile");

    assert_eq!(reference.tag_camouflage_fraction, 0.0);
    assert_eq!(generated.tag_camouflage_fraction, 1.0);
}

#[test]
fn the_reference_worlds_decisive_facts_all_sit_at_the_same_hop_distance() {
    let world = repo_fixture("fixtures/fiber-v0.1/radiogenomic_world.json");
    let measured = profile(&world, &reference_query(), "exploratory").expect("profile");
    assert_eq!(measured.min_hops_to_a_decisive_fact, Some(5));
    assert_eq!(measured.max_hops_to_a_decisive_fact, Some(5));
}
