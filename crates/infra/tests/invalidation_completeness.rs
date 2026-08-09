//! An invalidation that cannot be proved total must report itself partial, name the region it
//! could not see into, and cost hit rate rather than correctness.

use bioprism_infra::{
    Cache, CodeIdentity, Completeness, ComputationKey, DependencyDeclaration, DependencyGraph,
    EntryStatus, Epoch, InvalidationError, InvalidationPlan, KeySchema, MissReason, ResourceId,
    ReuseRule,
};
use serde_json::json;

fn resource(name: &str) -> ResourceId {
    ResourceId::parse(name).expect("a non-empty resource name")
}

fn schema() -> KeySchema {
    KeySchema::declare("derived-table", ["inputs", "code"], ReuseRule::AcrossBuilds)
        .expect("schema")
}

fn key(schema: &KeySchema, inputs: &str) -> ComputationKey {
    ComputationKey::build(schema, [("inputs", inputs), ("code", "v1")]).expect("key")
}

fn builder() -> CodeIdentity {
    CodeIdentity::parse("build-a").expect("build")
}

#[test]
fn a_change_invalidates_entries_that_declared_a_dependency_on_it() {
    let mut graph = DependencyGraph::new();
    graph.declare(resource("cohort"), []).expect("declare");

    let declarations = [
        (
            "entry-1".to_string(),
            DependencyDeclaration::on([resource("cohort")]),
        ),
        (
            "entry-2".to_string(),
            DependencyDeclaration::on([resource("atlas")]),
        ),
    ];
    graph.declare(resource("atlas"), []).expect("declare");

    let plan = InvalidationPlan::compute(
        &graph,
        resource("cohort"),
        declarations.iter().map(|(d, decl)| (d.clone(), decl)),
    );

    assert!(plan.invalid_entries.contains("entry-1"));
    assert!(plan.proved_unaffected.contains("entry-2"));
    assert_eq!(plan.completeness, Completeness::Complete);
}

#[test]
fn invalidation_is_transitive_through_declared_dependencies() {
    let mut graph = DependencyGraph::new();
    graph
        .declare(resource("report"), [resource("summary")])
        .expect("declare");
    graph
        .declare(resource("summary"), [resource("cohort")])
        .expect("declare");
    graph.declare(resource("cohort"), []).expect("declare");

    let declarations = [(
        "entry-report".to_string(),
        DependencyDeclaration::on([resource("report")]),
    )];
    let plan = InvalidationPlan::compute(
        &graph,
        resource("cohort"),
        declarations.iter().map(|(d, decl)| (d.clone(), decl)),
    );

    assert!(
        plan.invalid_entries.contains("entry-report"),
        "a two-hop dependency is still a dependency"
    );
    assert!(plan.is_complete());
}

#[test]
fn an_incomplete_invalidation_is_reported_as_partial_rather_than_assumed_total() {
    let mut graph = DependencyGraph::new();
    graph.declare(resource("cohort"), []).expect("declare");

    let declarations = [
        (
            "declared".to_string(),
            DependencyDeclaration::on([resource("cohort")]),
        ),
        ("legacy".to_string(), DependencyDeclaration::Undeclared),
    ];
    let plan = InvalidationPlan::compute(
        &graph,
        resource("cohort"),
        declarations.iter().map(|(d, decl)| (d.clone(), decl)),
    );

    assert!(!plan.is_complete());
    let region = plan
        .completeness
        .unknown_region()
        .expect("a partial plan names its unknown region");
    assert!(region
        .entries_without_declared_dependencies
        .contains("legacy"));
    assert!(plan.invalid_entries.contains("declared"));
}

#[test]
fn an_opaque_resource_puts_the_entries_that_reach_it_into_the_unknown_region() {
    let mut graph = DependencyGraph::new();
    graph
        .declare_opaque(resource("vendor-feed"))
        .expect("opaque");
    graph
        .declare(resource("cohort"), [resource("vendor-feed")])
        .expect("declare");
    graph.declare(resource("atlas"), []).expect("declare");

    let declarations = [
        (
            "via-vendor".to_string(),
            DependencyDeclaration::on([resource("cohort")]),
        ),
        (
            "clean".to_string(),
            DependencyDeclaration::on([resource("atlas")]),
        ),
    ];
    let plan = InvalidationPlan::compute(
        &graph,
        resource("something-else"),
        declarations.iter().map(|(d, decl)| (d.clone(), decl)),
    );

    let region = plan.completeness.unknown_region().expect("partial");
    assert!(region.opaque_resources.contains(&resource("vendor-feed")));
    assert!(region
        .entries_depending_on_opaque_resources
        .contains("via-vendor"));
    assert!(
        plan.proved_unaffected.contains("clean"),
        "an entry whose closure is fully declared is still provable"
    );
}

#[test]
fn opacity_is_transitive_so_a_one_hop_check_cannot_declare_an_entry_unaffected() {
    let mut graph = DependencyGraph::new();
    graph
        .declare_opaque(resource("deep-opaque"))
        .expect("opaque");
    graph
        .declare(resource("middle"), [resource("deep-opaque")])
        .expect("declare");
    graph
        .declare(resource("near"), [resource("middle")])
        .expect("declare");

    let declarations = [(
        "entry".to_string(),
        DependencyDeclaration::on([resource("near")]),
    )];
    let plan = InvalidationPlan::compute(
        &graph,
        resource("unrelated"),
        declarations.iter().map(|(d, decl)| (d.clone(), decl)),
    );

    assert!(
        !plan.is_complete(),
        "an opaque node three hops down still voids the proof"
    );
    assert!(plan.proved_unaffected.is_empty());
}

#[test]
fn a_dependency_on_a_resource_the_graph_never_heard_of_is_reported_separately_from_an_opaque_one() {
    let graph = DependencyGraph::new();
    let declarations = [(
        "entry".to_string(),
        DependencyDeclaration::on([resource("never-added")]),
    )];
    let plan = InvalidationPlan::compute(
        &graph,
        resource("cohort"),
        declarations.iter().map(|(d, decl)| (d.clone(), decl)),
    );

    let region = plan.completeness.unknown_region().expect("partial");
    assert!(region.unknown_resources.contains(&resource("never-added")));
    assert!(
        region.opaque_resources.is_empty(),
        "never-added was not declared opaque; the remedy is different"
    );
}

#[test]
fn a_declared_dependency_on_nothing_is_a_claim_and_is_kept_apart_from_no_declaration() {
    let empty = DependencyDeclaration::on([]);
    assert!(empty.is_declared());
    assert_eq!(empty.resources().map(|set| set.len()), Some(0));
    assert!(!DependencyDeclaration::Undeclared.is_declared());
    assert_eq!(DependencyDeclaration::Undeclared.resources(), None);
    assert_ne!(empty, DependencyDeclaration::Undeclared);
}

#[test]
fn a_resource_cannot_be_both_declared_and_opaque() {
    let mut graph = DependencyGraph::new();
    graph.declare(resource("cohort"), []).expect("declare");
    let error = graph
        .declare_opaque(resource("cohort"))
        .expect_err("contradiction");
    assert_eq!(
        error,
        InvalidationError::ContradictoryDeclaration("cohort".to_string())
    );
}

#[test]
fn a_declared_cycle_terminates_the_walk_instead_of_hanging() {
    let mut graph = DependencyGraph::new();
    graph
        .declare(resource("a"), [resource("b")])
        .expect("declare");
    graph
        .declare(resource("b"), [resource("a")])
        .expect("declare");

    let affected = graph.affected_by(&resource("a"));
    assert!(affected.contains(&resource("a")));
    assert!(affected.contains(&resource("b")));
    let cycle = graph.find_cycle().expect("the cycle is reported");
    assert!(cycle.len() >= 2);
}

#[test]
fn an_acyclic_graph_reports_no_cycle() {
    let mut graph = DependencyGraph::new();
    graph
        .declare(resource("a"), [resource("b")])
        .expect("declare");
    graph.declare(resource("b"), []).expect("declare");
    assert_eq!(graph.find_cycle(), None);
}

#[test]
fn applying_a_partial_plan_marks_the_unknown_region_unproven_rather_than_leaving_it_servable() {
    let schema = schema();
    let mut cache = Cache::new(schema.clone());
    let mut graph = DependencyGraph::new();
    graph.declare(resource("cohort"), []).expect("declare");

    let invalid = cache
        .insert(
            key(&schema, "a"),
            json!(1),
            builder(),
            Epoch::ZERO,
            DependencyDeclaration::on([resource("cohort")]),
        )
        .expect("insert");
    let unknown = cache
        .insert(
            key(&schema, "b"),
            json!(2),
            builder(),
            Epoch::ZERO,
            DependencyDeclaration::Undeclared,
        )
        .expect("insert");

    let plan = InvalidationPlan::compute(&graph, resource("cohort"), cache.declarations());
    assert!(!plan.is_complete());

    let report = cache
        .apply(&plan, Epoch::new(5))
        .expect("population matches");
    assert!(report.removed.contains(&invalid));
    assert!(report.marked_unproven.contains(&unknown));
    assert!(!report.invalidation_was_complete);

    let lookup = cache
        .lookup(&key(&schema, "b"), &builder())
        .expect("lookup");
    match lookup.miss_reason() {
        Some(MissReason::UnprovenAfterPartialInvalidation { since, cause }) => {
            assert_eq!(*since, Epoch::new(5));
            assert!(cause.contains("cohort"));
        }
        other => panic!("a stale-risk entry must not be served; got {other:?}"),
    }
}

#[test]
fn a_cache_never_serves_an_entry_from_the_unknown_region_after_a_partial_invalidation() {
    let schema = schema();
    let mut cache = Cache::new(schema.clone());
    let mut graph = DependencyGraph::new();
    graph.declare_opaque(resource("vendor")).expect("opaque");

    cache
        .insert(
            key(&schema, "a"),
            json!("stale?"),
            builder(),
            Epoch::ZERO,
            DependencyDeclaration::on([resource("vendor")]),
        )
        .expect("insert");

    assert!(
        cache
            .lookup(&key(&schema, "a"), &builder())
            .expect("lookup")
            .is_hit(),
        "before any invalidation the entry is servable"
    );

    let plan = InvalidationPlan::compute(&graph, resource("anything"), cache.declarations());
    cache.apply(&plan, Epoch::new(1)).expect("apply");

    assert!(!cache
        .lookup(&key(&schema, "a"), &builder())
        .expect("lookup")
        .is_hit());
}

#[test]
fn a_complete_invalidation_leaves_the_entries_it_proved_unaffected_servable() {
    let schema = schema();
    let mut cache = Cache::new(schema.clone());
    let mut graph = DependencyGraph::new();
    graph.declare(resource("cohort"), []).expect("declare");
    graph.declare(resource("atlas"), []).expect("declare");

    cache
        .insert(
            key(&schema, "a"),
            json!(1),
            builder(),
            Epoch::ZERO,
            DependencyDeclaration::on([resource("cohort")]),
        )
        .expect("insert");
    let kept = cache
        .insert(
            key(&schema, "b"),
            json!(2),
            builder(),
            Epoch::ZERO,
            DependencyDeclaration::on([resource("atlas")]),
        )
        .expect("insert");

    let plan = InvalidationPlan::compute(&graph, resource("cohort"), cache.declarations());
    let report = cache.apply(&plan, Epoch::new(1)).expect("apply");

    assert!(report.invalidation_was_complete);
    assert!(report.left_proven.contains(&kept));
    assert!(report.marked_unproven.is_empty());
    assert!(cache
        .lookup(&key(&schema, "b"), &builder())
        .expect("lookup")
        .is_hit());
}

#[test]
fn a_plan_computed_over_a_different_population_is_refused_rather_than_applied_in_part() {
    let schema = schema();
    let mut cache = Cache::new(schema.clone());
    let graph = DependencyGraph::new();
    cache
        .insert(
            key(&schema, "a"),
            json!(1),
            builder(),
            Epoch::ZERO,
            DependencyDeclaration::on([]),
        )
        .expect("insert");
    let plan = InvalidationPlan::compute(&graph, resource("cohort"), cache.declarations());

    cache
        .insert(
            key(&schema, "b"),
            json!(2),
            builder(),
            Epoch::ZERO,
            DependencyDeclaration::Undeclared,
        )
        .expect("insert");

    let error = cache
        .apply(&plan, Epoch::new(1))
        .expect_err("the population moved under the plan");
    assert_eq!(
        error,
        InvalidationError::PopulationChanged {
            planned: 1,
            actual: 2
        }
    );
}

#[test]
fn an_unproven_entry_becomes_servable_again_only_when_a_named_build_reproves_it() {
    let schema = schema();
    let mut cache = Cache::new(schema.clone());
    let mut graph = DependencyGraph::new();
    graph.declare_opaque(resource("vendor")).expect("opaque");

    let digest = cache
        .insert(
            key(&schema, "a"),
            json!(1),
            builder(),
            Epoch::ZERO,
            DependencyDeclaration::on([resource("vendor")]),
        )
        .expect("insert");
    let plan = InvalidationPlan::compute(&graph, resource("x"), cache.declarations());
    cache.apply(&plan, Epoch::new(2)).expect("apply");
    assert_eq!(cache.unproven().len(), 1);

    let reprover = CodeIdentity::parse("build-c").expect("build");
    assert_eq!(cache.reprove(&digest, &reprover), Some(&reprover));
    assert!(cache.unproven().is_empty());
    assert!(matches!(
        cache.get(&digest).map(|entry| &entry.status),
        Some(EntryStatus::Proven)
    ));
}

#[test]
fn a_malformed_resource_name_is_refused() {
    assert!(ResourceId::parse("  ").is_err());
    assert!(ResourceId::parse("cohort\nname").is_err());
}
