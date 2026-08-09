//! Metamorphic relations, lineage and the honesty of the diversity accounting.

use bioprism_fiber::FiberError;
use bioprism_mutation::{
    apply, generate, measure, standard_suite, verdict_of, Mechanism, Mutation, MutationError,
    MutationKind, PostconditionResult, Relation, RejectionReason,
};
use bioprism_world::WorldError;
use bioprism_worldgen::{generate as generate_world, WorldSpec};
use serde_json::{json, Value};

fn parent() -> Value {
    generate_world(&WorldSpec::reference_like(40)).world
}

fn fact_value_mut<'a>(world: &'a mut Value, id: &str) -> &'a mut Value {
    world["facts"]
        .as_array_mut()
        .expect("facts array")
        .iter_mut()
        .find(|fact| fact["id"] == json!(id))
        .unwrap_or_else(|| panic!("{id} is present"))
        .get_mut("value")
        .expect("a fact carries a value")
}

#[test]
fn the_parent_world_exhibits_all_four_mechanisms() {
    let verdict = verdict_of(&parent()).expect("parent evaluates");
    assert_eq!(verdict.status.as_str(), "invalid");
    assert_eq!(verdict.witness_kinds().len(), 4);
}

#[test]
fn invariance_mutations_preserve_the_verdict_exactly() {
    let world = parent();
    let before = verdict_of(&world).unwrap();

    for mutation in [
        Mutation::new("m", MutationKind::RenameSubjects { prefix: "X".into() }),
        Mutation::new("m", MutationKind::ReorderFacts { seed: 3 }),
        Mutation::new("m", MutationKind::AddDistractors { count: 10, seed: 5 }),
        Mutation::new("m", MutationKind::CamouflageTags),
    ] {
        let mutated = apply(&world, &mutation).expect("applies");
        let after = verdict_of(&mutated).expect("mutated world evaluates");
        assert_eq!(
            mutation.relation.check(&before, &after),
            PostconditionResult::Held,
            "{:?} should be verdict-preserving",
            mutation.kind
        );
        assert_ne!(mutated, world, "a mutation must actually change the world");
    }
}

#[test]
fn renaming_subjects_changes_the_bytes_but_not_the_conclusion() {
    let world = parent();
    let mutated = apply(
        &world,
        &Mutation::new("m", MutationKind::RenameSubjects { prefix: "X".into() }),
    )
    .unwrap();

    let text = serde_json::to_string(&mutated).unwrap();
    assert!(text.contains("X001"), "subjects were not renamed");
    assert!(!text.contains("\"S001\""), "an old identifier survived");
    assert!(
        text.contains("ALT-77"),
        "the shared alias must survive, or the identity witness disappears for the wrong reason"
    );

    let after = verdict_of(&mutated).unwrap();
    assert!(after.witness_kinds().contains(&"identity_leakage"));
}

#[test]
fn removing_a_mechanism_removes_exactly_that_witness() {
    let world = parent();
    let before = verdict_of(&world).unwrap();

    for mechanism in Mechanism::ALL {
        let mutation = Mutation::new("m", MutationKind::RemoveLeakage { mechanism });
        let mutated = apply(&world, &mutation).expect("applies");
        let after = verdict_of(&mutated).expect("evaluates");

        assert_eq!(
            mutation.relation.check(&before, &after),
            PostconditionResult::Held,
            "removing {mechanism:?} did not remove exactly its witness"
        );
        assert!(!after.witness_kinds().contains(&mechanism.witness_kind()));
        assert_eq!(
            after.witness_kinds().len(),
            3,
            "removing one mechanism must not disturb the other three"
        );
    }
}

#[test]
fn removing_every_mechanism_yields_a_valid_world() {
    let mut world = parent();
    for mechanism in Mechanism::ALL {
        world = apply(
            &world,
            &Mutation::new("m", MutationKind::RemoveLeakage { mechanism }),
        )
        .unwrap();
    }
    let verdict = verdict_of(&world).unwrap();
    assert_eq!(verdict.status.as_str(), "valid");
    assert!(verdict.witnesses.is_empty());
}

#[test]
fn injecting_into_a_clean_world_adds_exactly_that_witness() {
    let mut clean = parent();
    for mechanism in Mechanism::ALL {
        clean = apply(
            &clean,
            &Mutation::new("m", MutationKind::RemoveLeakage { mechanism }),
        )
        .unwrap();
    }
    let before = verdict_of(&clean).unwrap();
    assert_eq!(before.status.as_str(), "valid");

    for mechanism in Mechanism::ALL {
        let mutation = Mutation::new("m", MutationKind::InjectLeakage { mechanism });
        let mutated = apply(&clean, &mutation).unwrap();
        let after = verdict_of(&mutated).unwrap();
        assert_eq!(
            mutation.relation.check(&before, &after),
            PostconditionResult::Held,
            "injecting {mechanism:?} did not add exactly its witness"
        );
    }
}

/// A mutation that lies about its relation must be rejected, not shipped.
#[test]
fn a_violated_postcondition_is_caught() {
    let world = parent();
    let before = verdict_of(&world).unwrap();

    // Claim that removing preprocessing leakage preserves the verdict. It does not.
    let liar = Mutation {
        id: "liar".into(),
        kind: MutationKind::RemoveLeakage {
            mechanism: Mechanism::Preprocessing,
        },
        relation: Relation::PreservesVerdict,
    };
    let mutated = apply(&world, &liar).unwrap();
    let after = verdict_of(&mutated).unwrap();

    match liar.relation.check(&before, &after) {
        PostconditionResult::Violated { expected, observed } => {
            assert!(expected.contains("preprocessing_leakage"));
            assert!(!observed.contains("preprocessing_leakage"));
        }
        PostconditionResult::Held => panic!("a false relation must not pass"),
    }

    let family = generate(&world, std::slice::from_ref(&liar)).unwrap();
    assert!(family.accepted.is_empty(), "a liar must not be admitted");
    assert_eq!(family.rejected.len(), 1);
    match &family.rejected[0].reason {
        RejectionReason::PostconditionViolated { expected, observed } => {
            assert!(expected.contains("preprocessing_leakage"));
            assert!(!observed.contains("preprocessing_leakage"));
        }
        other => panic!("a broken relation must be classified as such, got {other:?}"),
    }
}

/// The four rejection classes are findings about different code and must not share a shape.
#[test]
fn a_mutation_with_nothing_to_act_on_is_not_recorded_as_a_broken_postcondition() {
    // The operator looks its target up by fact id, so a world that provides the same variable
    // under a different id is well formed and still gives the mutation nothing to act on.
    let mut world = parent();
    for fact in world["facts"].as_array_mut().expect("facts array") {
        if fact["id"] == json!("fact.preprocess_fit") {
            fact["id"] = json!("fact.preprocess_fit.relabelled");
        }
    }

    let family = generate(
        &world,
        &[Mutation::new(
            "repair-preprocessing",
            MutationKind::RemoveLeakage {
                mechanism: Mechanism::Preprocessing,
            },
        )],
    )
    .expect("a world missing one fact still generates a family");

    assert_eq!(family.rejected.len(), 1);
    match &family.rejected[0].reason {
        RejectionReason::NotApplicable { detail } => {
            assert!(
                detail.contains("fact.preprocess_fit"),
                "the rejection must name what was missing: {detail}"
            );
        }
        other => panic!("expected an inapplicable mutation, got {other:?}"),
    }
}

/// A malformed parent is the caller's document to fix, and the error says which document.
#[test]
fn a_parent_that_does_not_load_is_a_typed_error_naming_the_world() {
    let error = generate(
        &json!({ "world_id": "not-a-world", "schema_version": "bioprism-world/0.0" }),
        &standard_suite(),
    )
    .expect_err("a parent that does not load cannot yield a family");

    match error {
        MutationError::DoesNotLoad { world_id, source } => {
            assert_eq!(world_id, "not-a-world");
            assert!(
                matches!(source, WorldError::UnsupportedSchema { .. }),
                "the loader's own error must survive, got {source:?}"
            );
        }
        other => panic!("expected a load failure, got {other:?}"),
    }
}

/// A world that loads and cannot be judged is a different failure with a different remedy.
#[test]
fn a_parent_the_oracle_refuses_is_not_reported_as_a_parent_that_does_not_load() {
    let mut world = parent();
    let split = fact_value_mut(&mut world, "fact.split")
        .as_object_mut()
        .expect("split assignments");
    // The last subject shares alias ALT-77 with the first, so dropping its arm leaves the oracle
    // with a group it cannot order.
    let aliased = split.keys().next_back().cloned().expect("a subject");
    split.remove(&aliased);
    let world_id = world["world_id"].as_str().expect("a label").to_string();

    let error = verdict_of(&world).expect_err("the oracle must refuse an unorderable split");
    match error {
        MutationError::NotEvaluable { world_id: named, source } => {
            assert_eq!(named, world_id);
            assert!(
                matches!(source, FiberError::UnorderableSplitGroups { .. }),
                "the oracle's own error must survive, got {source:?}"
            );
        }
        other => panic!("expected an oracle refusal, got {other:?}"),
    }

    assert!(
        generate(&world, &standard_suite()).is_err(),
        "a parent nobody can judge cannot anchor a validated family"
    );
}

#[test]
fn a_rejection_survives_serialization_with_its_class_intact() {
    let world = parent();
    let liar = Mutation {
        id: "liar".into(),
        kind: MutationKind::RemoveLeakage {
            mechanism: Mechanism::Preprocessing,
        },
        relation: Relation::PreservesVerdict,
    };
    let family = generate(&world, std::slice::from_ref(&liar)).unwrap();

    let encoded = serde_json::to_value(&family.rejected).unwrap();
    assert_eq!(
        encoded[0]["reason"]["rejection"],
        json!("postcondition_violated"),
        "the class must be readable without parsing prose: {encoded}"
    );

    let decoded: Vec<bioprism_mutation::Rejection> = serde_json::from_value(encoded).unwrap();
    assert_eq!(decoded, family.rejected);
}

#[test]
fn the_standard_suite_generates_a_validated_family() {
    let world = parent();
    let family = generate(&world, &standard_suite()).expect("family generates");

    assert_eq!(family.accepted.len(), 8, "all eight relations should hold");
    assert!(family.rejected.is_empty(), "unexpected rejections: {:?}", family.rejected);
    assert_eq!(family.yield_rate(), 1.0);

    for instance in &family.accepted {
        assert_eq!(instance.parent_id, family.parent_id);
        assert_eq!(instance.world_sha256.len(), 64);
        assert!(family.worlds.contains_key(&instance.id));
    }
}

#[test]
fn identical_mutations_are_deduplicated_rather_than_counted_twice() {
    let world = parent();
    let twice = vec![
        Mutation::new("first", MutationKind::ReorderFacts { seed: 42 }),
        Mutation::new("second", MutationKind::ReorderFacts { seed: 42 }),
    ];
    let family = generate(&world, &twice).unwrap();

    assert_eq!(family.accepted.len(), 1);
    assert_eq!(family.duplicates.len(), 1);
    assert!(family.yield_rate() < 1.0, "duplicates must lower the yield rate");
}

#[test]
fn a_mutation_that_reproduces_the_parent_is_a_duplicate() {
    let world = parent();
    // Removing a mechanism that is already absent leaves the world unchanged.
    let clean = apply(
        &world,
        &Mutation::new("prep", MutationKind::RemoveLeakage { mechanism: Mechanism::Preprocessing }),
    )
    .unwrap();

    let family = generate(
        &clean,
        &[Mutation::new(
            "again",
            MutationKind::RemoveLeakage {
                mechanism: Mechanism::Preprocessing,
            },
        )],
    )
    .unwrap();

    // Content-identical to its parent: caught by deduplication before the oracle ever runs, which
    // is cheaper and more accurate than letting the postcondition fail on it.
    assert!(family.accepted.is_empty());
    assert_eq!(family.duplicates.len(), 1);
    assert!(family.rejected.is_empty());
}

/// Renaming a world must not let a duplicate through.
#[test]
fn deduplication_is_not_defeated_by_relabelling() {
    let world = parent();
    let mut renamed = world.clone();
    renamed["world_id"] = Value::String("a-different-name".into());
    renamed["description"] = Value::String("and a different description".into());

    assert_eq!(
        bioprism_mutation::lineage::content_digest(&world).unwrap(),
        bioprism_mutation::lineage::content_digest(&renamed).unwrap(),
        "the label is not content"
    );
    assert_ne!(
        bioprism_ids::ContentHash::of_value(&world).unwrap(),
        bioprism_ids::ContentHash::of_value(&renamed).unwrap(),
        "but the full-document hash does differ, which is why dedup cannot use it"
    );
}

/// The headline claim of the diversity accounting.
#[test]
fn instance_count_is_not_benchmark_count() {
    let world = parent();

    // Twenty reorderings: twenty distinct worlds, all testing exactly the same thing.
    let padding: Vec<Mutation> = (0..20)
        .map(|seed| Mutation::new(format!("reorder-{seed}"), MutationKind::ReorderFacts { seed }))
        .collect();
    let padded = generate(&world, &padding).unwrap();
    let inflated = measure(std::slice::from_ref(&padded));

    assert!(inflated.instances >= 15, "expected many instances");
    assert_eq!(
        inflated.equivalence_classes, 1,
        "every reordering tests the same (parent, family, signature) triple"
    );
    assert!(inflated.inflation_ratio > 10.0);
    assert!(
        !inflated.is_publishable(),
        "a family that collapses to one class is a robustness check, not a benchmark"
    );
    assert!(inflated.headline().contains("Instance count is not benchmark count"));

    // The standard suite has fewer instances but far more independent information.
    let genuine = measure(&[generate(&world, &standard_suite()).unwrap()]);
    assert!(genuine.instances < inflated.instances, "fewer instances");
    assert!(
        genuine.equivalence_classes > inflated.equivalence_classes,
        "but more independent classes: {} vs {}",
        genuine.equivalence_classes,
        inflated.equivalence_classes
    );
    assert!(genuine.is_publishable());
    assert!(genuine.inflation_ratio < 1.5);
}

#[test]
fn diversity_accounts_for_multiple_parents() {
    let first = generate_world(&WorldSpec::reference_like(20)).world;
    let second = generate_world(&WorldSpec::discriminating(20)).world;

    let families = vec![
        generate(&first, &standard_suite()).unwrap(),
        generate(&second, &standard_suite()).unwrap(),
    ];
    let diversity = measure(&families);

    assert_eq!(diversity.parents, 2);
    assert!(diversity.equivalence_classes > diversity.families);
    assert!(diversity.caveat.contains("makes no claim about correlation"));
}

#[test]
fn generation_is_deterministic() {
    let world = parent();
    let first = generate(&world, &standard_suite()).unwrap();
    let second = generate(&world, &standard_suite()).unwrap();
    assert_eq!(first.accepted, second.accepted);
}
