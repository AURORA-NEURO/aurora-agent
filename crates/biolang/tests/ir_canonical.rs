//! Canonical form: every IR round-trips and hashes stably through `bioprism_ids`.
//!
//! Blueprint 25.01–25.21 each repeat lifecycle step 4, "the object receives a canonical
//! serialization and content hash". These tests check the two properties that step is worth having
//! for: the bytes are a function of the value, and they come from the one encoder the workspace's
//! three-way parity rests on.

use bioprism_biolang::bioql::{compile, BioType, CollectionDecl, QuerySchema};
use bioprism_biolang::clock::Clock;
use bioprism_biolang::ids::{MutationId, StateId, WorldlineId};
use bioprism_biolang::mutation::{
    MutationProgram, Risk, SeedDeclaration, SemanticRelation, Transformation, TransformationTarget,
};
use bioprism_biolang::oracle::{EvidencePlane, EvidenceTier, Independence, OracleIr};
use bioprism_biolang::state::{BioState, Plane, ResourceLedger, UncertaintySummary};
use bioprism_biolang::worldline::{AlignmentConfidence, Censoring, Worldline};
use bioprism_biolang::{round_trips, Canonical};
use bioprism_ids::{to_canonical_bytes, ContentHash, WorldId};
use bioprism_scope::{ScopeKey, Timestamp};
use serde_json::json;
use std::collections::BTreeSet;

fn at(text: &str) -> Timestamp {
    Timestamp::parse(text).expect("RFC 3339")
}

fn digest(tag: &str) -> ContentHash {
    ContentHash::of_value(&json!({ "tag": tag })).expect("hashes")
}

fn state(id: &str) -> BioState {
    BioState::new(
        StateId::parse(id).expect("well-formed"),
        WorldId::parse("onco/gbm").expect("well-formed"),
        at("2026-03-01T00:00:00Z"),
        at("2026-03-01T06:00:00Z"),
        UncertaintySummary {
            budget_digest: digest("budget"),
            unaccounted_components: 1,
        },
    )
    .within(ScopeKey::new().exact("subject", "S1"))
    .with_plane(Plane::Biological, digest("tumor"))
    .with_plane(Plane::Knowledge, digest("belief"))
    .owing("population frequency")
    .granting("read:evidence")
    .having_consumed(ResourceLedger::new().consume("tissue_mg", 40.0))
}

fn worldline() -> Worldline {
    Worldline::new(
        WorldlineId::parse("wl-1").expect("well-formed"),
        WorldId::parse("onco/gbm").expect("well-formed"),
        ScopeKey::new().exact("subject", "S1"),
        Clock::Event,
        AlignmentConfidence::declared(0.875, "declared by the assembling adapter"),
        Censoring::RightCensored {
            at: at("2027-01-01T00:00:00Z"),
        },
    )
    .then(state("s1"))
}

fn oracle() -> OracleIr {
    OracleIr {
        oracle_id: "oracle/schema".to_string(),
        kind: "schema".to_string(),
        version: "1.0.0".to_string(),
        tier: EvidenceTier::Deterministic,
        inputs: BTreeSet::from(["document".to_string()]),
        outputs: BTreeSet::from(["verdict".to_string()]),
        establishes: BTreeSet::from([EvidencePlane::Artifact]),
        cannot_establish: BTreeSet::from([EvidencePlane::Biological]),
        evidence_basis: "a JSON schema".to_string(),
        failure_conditions: Vec::new(),
        priority: 0,
        calibration: "not applicable".to_string(),
        independence: Independence {
            from_evaluated_system: true,
            shared_resources: BTreeSet::new(),
        },
    }
}

fn mutation() -> MutationProgram {
    MutationProgram {
        mutation_id: MutationId::parse("mut-1").expect("well-formed"),
        parent: Some("world/base@1".to_string()),
        applicability: "any world with a lesion table".to_string(),
        seed: SeedDeclaration::Seeded { seed: 42 },
        transformations: vec![Transformation {
            target: TransformationTarget::World,
            locator: "$.facts[0].value".to_string(),
            description: "rescale a volume".to_string(),
        }],
        relation: SemanticRelation::Preserving,
        oracle_changes: BTreeSet::new(),
        validations: vec!["the world still parses".to_string()],
        risk: Risk::Cosmetic,
        generator_version: "bioprism-mutation 0.1.0".to_string(),
    }
}

fn schema() -> QuerySchema {
    QuerySchema::new().with(
        CollectionDecl::new("lesions")
            .costing(10)
            .declare(
                "tumor_volume",
                BioType::quantity(bioprism_standards::Unit::parse("mm3").expect("known unit")),
            ),
    )
}

#[test]
fn a_biostate_round_trips_through_its_canonical_form() {
    assert!(round_trips(&state("s1")).expect("encodes"));
}

#[test]
fn a_worldline_round_trips_through_its_canonical_form() {
    assert!(round_trips(&worldline()).expect("encodes"));
}

#[test]
fn an_oracle_manifest_round_trips_through_its_canonical_form() {
    assert!(round_trips(&oracle()).expect("encodes"));
}

#[test]
fn a_mutation_program_round_trips_through_its_canonical_form() {
    assert!(round_trips(&mutation()).expect("encodes"));
}

#[test]
fn a_typed_query_round_trips_through_its_canonical_form() {
    let typed = compile(
        r#"select tumor_volume from lesions where tumor_volume > 1 mm3 labels {} cost limit 100"#,
        &schema(),
    )
    .expect("typechecks");
    assert!(round_trips(&typed).expect("encodes"));
}

#[test]
fn every_ir_digest_comes_from_the_workspace_encoder_and_not_a_private_one() {
    let value = state("s1");
    let via_trait = value.canonical_bytes().expect("encodes");
    let via_workspace =
        to_canonical_bytes(&serde_json::to_value(&value).expect("serializes")).expect("encodes");
    assert_eq!(
        via_trait, via_workspace,
        "there is exactly one canonical encoder in this workspace"
    );
    assert_eq!(
        value.digest().expect("hashes"),
        ContentHash::of_bytes(&via_workspace)
    );
}

#[test]
fn a_digest_is_a_function_of_the_value_and_not_of_construction_order() {
    let built_one_way = state("s1");
    let built_another_way = BioState::new(
        StateId::parse("s1").expect("well-formed"),
        WorldId::parse("onco/gbm").expect("well-formed"),
        at("2026-03-01T00:00:00Z"),
        at("2026-03-01T06:00:00Z"),
        UncertaintySummary {
            budget_digest: digest("budget"),
            unaccounted_components: 1,
        },
    )
    .having_consumed(ResourceLedger::new().consume("tissue_mg", 40.0))
    .granting("read:evidence")
    .owing("population frequency")
    .with_plane(Plane::Knowledge, digest("belief"))
    .with_plane(Plane::Biological, digest("tumor"))
    .within(ScopeKey::new().exact("subject", "S1"));
    assert_eq!(
        built_one_way.digest().expect("hashes"),
        built_another_way.digest().expect("hashes")
    );
}

#[test]
fn a_state_digest_moves_when_a_represented_plane_moves() {
    let before = state("s1");
    let after = state("s1").with_plane(Plane::Biological, digest("tumor-grown"));
    assert_ne!(
        before.digest().expect("hashes"),
        after.digest().expect("hashes")
    );
    assert_eq!(
        before.changed_planes(&after),
        BTreeSet::from([Plane::Biological])
    );
}

#[test]
fn a_state_digest_does_not_move_when_nothing_represented_moves() {
    assert_eq!(
        state("s1").digest().expect("hashes"),
        state("s1").digest().expect("hashes")
    );
    assert!(state("s1").changed_planes(&state("s1")).is_empty());
}

#[test]
fn a_non_finite_amount_is_indistinguishable_from_an_absent_one_once_serde_json_has_seen_it() {
    let broken = state("s1").having_consumed(ResourceLedger::new().consume("tissue_mg", f64::NAN));
    let encoded = serde_json::to_value(&broken).expect("serializes");
    assert_eq!(
        encoded["consumed"]["tissue_mg"],
        serde_json::Value::Null,
        "serde_json renders a non-finite float as null, so bioprism_ids' non-finite guard never fires"
    );
    assert!(
        broken.digest().is_ok(),
        "the digest succeeds, which is the hole; the state validator is what closes it"
    );
}

#[test]
fn a_state_carrying_a_non_finite_amount_is_refused_at_validation() {
    let broken = state("s1").having_consumed(ResourceLedger::new().consume("tissue_mg", f64::NAN));
    assert!(matches!(
        broken.validate().unwrap_err(),
        bioprism_biolang::error::StateError::NonFiniteAmount { .. }
    ));
    state("s1").validate().expect("a finite ledger validates");
}

#[test]
fn a_canonical_encoding_sorts_object_keys_so_two_orders_hash_alike() {
    let left = json!({ "b": 1, "a": 2 });
    let right = json!({ "a": 2, "b": 1 });
    assert_eq!(
        to_canonical_bytes(&left).expect("encodes"),
        to_canonical_bytes(&right).expect("encodes")
    );
    assert_eq!(
        String::from_utf8(to_canonical_bytes(&left).expect("encodes")).expect("utf-8"),
        r#"{"a":2,"b":1}"#
    );
}

#[test]
fn two_queries_differing_only_in_layout_produce_the_same_typed_digest() {
    let dense = compile(
        r#"select tumor_volume from lesions where tumor_volume > 1 mm3 labels {} cost limit 100"#,
        &schema(),
    )
    .expect("typechecks");
    let spaced = compile(
        "select tumor_volume\n  from lesions\n  -- a comment\n  where tumor_volume > 1 mm3\n  labels {}\n  cost limit 100",
        &schema(),
    )
    .expect("typechecks");
    assert_eq!(
        dense.digest().expect("hashes"),
        spaced.digest().expect("hashes"),
        "a bundle cites the typed query, not the source text"
    );
}

#[test]
fn two_queries_differing_in_declared_labels_do_not_share_a_digest() {
    let public = compile(
        r#"select tumor_volume from lesions labels {} cost limit 100"#,
        &schema(),
    )
    .expect("typechecks");
    let restricted = compile(
        r#"select tumor_volume from lesions labels { "phi:identified" } cost limit 100"#,
        &schema(),
    )
    .expect("typechecks");
    assert_ne!(
        public.digest().expect("hashes"),
        restricted.digest().expect("hashes"),
        "access labels are part of what a query is"
    );
}
