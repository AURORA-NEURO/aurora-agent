//! Acceptance tests for blueprint 41.16.
//!
//! 41.16 asks for checks "proving that task routing and token-bounded retrieval work on a clean
//! extraction", route smoke tests, bundle tests, hash tests, search tests and broken-edge
//! fixtures. Each test name states the claim it defends rather than the function it calls.

use bioprism_docgraph::bundle::{compile_bundle, OmissionReason, Sufficiency};
use bioprism_docgraph::error::{BundleError, DocGraphError};
use bioprism_docgraph::fixture::{broken_graph, repository_doc_graph, repository_routes};
use bioprism_docgraph::impact::impact_of;
use bioprism_docgraph::lint::{lint, LintFinding, LintSeverity};
use bioprism_docgraph::markdown::{first_h1, headings, link_targets, parse_document};
use bioprism_docgraph::protocol::{
    check_receipt, Citation, Claim, ClaimKind, ProtocolViolation, ReadingReceipt,
};
use bioprism_docgraph::registry::{
    ContextCard, DocGraph, ModuleId, ModuleNode, NodeStatus, ProtectedClass,
};
use bioprism_docgraph::route::{RouteDefect, RouteId, TaskRoute};
use bioprism_docgraph::tokens::{estimate_tokens, ProfileLevel, TokenCost};
use bioprism_docgraph::traverse::{traverse, Completeness, TraversalPolicy};
use bioprism_docgraph::vocabulary::{CompanionRequirement, DocEdge, DocEdgeType, ImpactPropagation};
use bioprism_section::InfluenceClass;

fn id(value: &str) -> ModuleId {
    ModuleId::parse(value).expect("well formed test id")
}

fn spec(path: &str, title: &str) -> ModuleNode {
    ModuleNode::new(id(path), path, title, NodeStatus::Specification)
        .with_card(ContextCard {
            decision: format!("{title} decides one thing."),
            ..ContextCard::default()
        })
        .with_hashed_body(&format!("# {title}\n\nNormative text for {path}.\n"))
}

fn route(name: &str) -> TaskRoute {
    TaskRoute::new(RouteId::parse(name).expect("route id"), "test route")
}

// ---------------------------------------------------------------- 41.03 vocabulary

#[test]
fn an_untyped_related_edge_is_not_in_the_vocabulary() {
    assert!(matches!(
        DocEdgeType::parse("related"),
        Err(DocGraphError::UnknownEdgeType(_))
    ));
    assert!(!DocEdgeType::ALL
        .iter()
        .any(|kind| kind.as_str() == "related"));
}

#[test]
fn unknown_edge_types_fail_validation_rather_than_degrading_to_a_default() {
    let error = DocEdgeType::parse("sort_of_about").unwrap_err();
    assert_eq!(
        error,
        DocGraphError::UnknownEdgeType("sort_of_about".to_string())
    );
}

#[test]
fn every_edge_type_round_trips_through_its_canonical_name() {
    for kind in DocEdgeType::ALL {
        assert_eq!(DocEdgeType::parse(kind.as_str()).unwrap(), kind);
        assert!(!kind.gloss().is_empty());
    }
}

#[test]
fn depends_on_parses_to_requires_without_creating_a_second_member() {
    assert_eq!(
        DocEdgeType::parse("depends_on").unwrap(),
        DocEdgeType::Requires
    );
    assert_eq!(DocEdgeType::ALL.len(), 12);
}

#[test]
fn a_reversed_alias_swaps_the_endpoints_and_records_the_original_spelling() {
    let edge = DocEdge::parse(id("contract.md"), "tested_by", id("suite.md")).unwrap();
    assert_eq!(edge.kind, DocEdgeType::Evaluates);
    assert_eq!(edge.from, id("suite.md"));
    assert_eq!(edge.to, id("contract.md"));
    assert_eq!(edge.normalized_from.as_deref(), Some("tested_by"));
}

#[test]
fn contradicts_is_the_only_symmetric_edge_type() {
    let symmetric: Vec<DocEdgeType> = DocEdgeType::ALL
        .into_iter()
        .filter(|kind| kind.symmetric())
        .collect();
    assert_eq!(symmetric, vec![DocEdgeType::Contradicts]);
}

#[test]
fn example_of_obliges_its_contract_but_does_not_propagate_impact() {
    assert_eq!(
        DocEdgeType::ExampleOf.companion(),
        CompanionRequirement::TargetWithSource
    );
    assert_eq!(
        DocEdgeType::ExampleOf.propagation(),
        ImpactPropagation::DirectOnly
    );
    assert_eq!(
        DocEdgeType::Requires.propagation(),
        ImpactPropagation::Transitive
    );
    assert_eq!(
        DocEdgeType::Refines.propagation(),
        ImpactPropagation::Transitive
    );
}

#[test]
fn supersedes_obliges_the_successor_when_the_superseded_module_is_present() {
    assert_eq!(
        DocEdgeType::Supersedes.companion(),
        CompanionRequirement::SourceWithTarget
    );
}

// ---------------------------------------------------------------- 41.06 token profiles

#[test]
fn a_total_containing_one_estimate_is_never_reported_as_a_measurement() {
    let measured = TokenCost::measured(100, "test-tokenizer");
    let estimated = TokenCost::estimate("some prose");
    assert!(measured.is_measurement());
    let total = TokenCost::sum([&measured, &estimated]);
    assert!(!total.is_measurement());
    assert_eq!(total.tokens, 100 + estimated.tokens);
}

#[test]
fn totals_of_measurements_from_different_tokenizers_degrade_to_an_estimate() {
    let a = TokenCost::measured(10, "tokenizer-a");
    let b = TokenCost::measured(10, "tokenizer-b");
    assert!(!TokenCost::sum([&a, &b]).is_measurement());
    assert!(TokenCost::sum([&a, &a]).is_measurement());
}

#[test]
fn no_cost_this_crate_produces_from_a_document_is_a_measurement() {
    let graph = repository_doc_graph();
    let bundle = compile_bundle(
        &graph,
        &repository_routes()[1],
        &TraversalPolicy::exhaustive(),
    )
    .expect("route compiles");
    assert!(!bundle.cost.is_measurement());
    assert!(bundle.entries.iter().all(|entry| !entry.cost.is_measurement()));
}

#[test]
fn the_estimator_floors_at_one_token_per_whitespace_run() {
    assert_eq!(estimate_tokens(""), 0);
    assert_eq!(estimate_tokens("a b c d e f g h"), 8);
    assert_eq!(estimate_tokens("abcd"), 1);
    assert_eq!(estimate_tokens("abcdefgh"), 2);
    assert_eq!(estimate_tokens("a b"), estimate_tokens("a b"));
}

#[test]
fn a_card_is_not_a_level_an_obligation_may_be_read_from() {
    assert!(!ProfileLevel::Handle.is_normative());
    assert!(!ProfileLevel::Card.is_normative());
    assert!(!ProfileLevel::Brief.is_normative());
    assert!(ProfileLevel::Contract.is_normative());
    assert!(ProfileLevel::DeepReference.is_normative());
}

// ---------------------------------------------------------------- 41.02 registry

#[test]
fn a_graph_accepts_a_dangling_edge_so_that_the_linter_can_report_it() {
    let mut graph = DocGraph::new();
    graph.insert_node(spec("a.md", "A")).unwrap();
    graph.insert_edge(DocEdge::new(id("a.md"), id("gone.md"), DocEdgeType::Requires));
    assert_eq!(graph.dangling_endpoints().len(), 1);
    let report = lint(&graph, &[]);
    assert_eq!(report.with_code("dangling_edge").count(), 1);
}

#[test]
fn registering_a_module_id_twice_is_an_error_rather_than_an_overwrite() {
    let mut graph = DocGraph::new();
    graph.insert_node(spec("a.md", "A")).unwrap();
    let error = graph.insert_node(spec("a.md", "A again")).unwrap_err();
    assert_eq!(error, DocGraphError::DuplicateModuleId(id("a.md")));
    assert_eq!(graph.node(&id("a.md")).unwrap().title, "A");
}

#[test]
fn module_ids_reject_whitespace_and_emptiness() {
    assert!(ModuleId::parse("docs/A B.md").is_err());
    assert!(ModuleId::parse("").is_err());
    assert!(ModuleId::parse("docs/ARCHITECTURE.md").is_ok());
}

// ---------------------------------------------------------------- 41.05 routes

#[test]
fn a_route_that_lists_a_boundary_as_optional_is_reported_as_a_defect() {
    let mut graph = DocGraph::new();
    graph.insert_node(spec("a.md", "A")).unwrap();
    graph.insert_node(spec("boundary.md", "Boundary")).unwrap();
    let route = route("r")
        .must_read(id("a.md"))
        .optional(id("boundary.md"))
        .non_omittable(id("boundary.md"));
    let defects = route.check(&graph);
    assert!(defects.iter().any(|defect| matches!(
        defect,
        RouteDefect::NonOmittableListedAsOptional { module } if module == &id("boundary.md")
    )));
}

#[test]
fn a_disconnected_must_read_set_is_reported_component_by_component() {
    let mut graph = DocGraph::new();
    graph.insert_node(spec("a.md", "A")).unwrap();
    graph.insert_node(spec("b.md", "B")).unwrap();
    graph.insert_node(spec("z.md", "Z")).unwrap();
    graph.insert_edge(DocEdge::new(id("a.md"), id("b.md"), DocEdgeType::Requires));
    let route = route("r")
        .must_read(id("a.md"))
        .must_read(id("b.md"))
        .must_read(id("z.md"));
    let defects = route.check(&graph);
    let components = defects
        .iter()
        .find_map(|defect| match defect {
            RouteDefect::MustReadNotConnected { components } => Some(components.clone()),
            _ => None,
        })
        .expect("disconnected must-read set is reported");
    assert_eq!(components.len(), 2);
}

// ---------------------------------------------------------------- 41.07 traversal

#[test]
fn only_an_exhaustive_walk_licenses_a_zero_influence_claim() {
    let mut graph = DocGraph::new();
    for path in ["a.md", "b.md", "c.md", "island.md"] {
        graph.insert_node(spec(path, path)).unwrap();
    }
    graph.insert_edge(DocEdge::new(id("a.md"), id("b.md"), DocEdgeType::Requires));
    graph.insert_edge(DocEdge::new(id("b.md"), id("c.md"), DocEdgeType::Requires));

    let full = traverse(&graph, &[id("a.md")], &TraversalPolicy::exhaustive());
    assert_eq!(full.completeness(), Completeness::Exhaustive);
    assert!(full.completeness().licenses_zero_influence());
    assert!(!full.was_reached(&id("island.md")));

    let capped = traverse(
        &graph,
        &[id("a.md")],
        &TraversalPolicy::exhaustive().with_max_depth(1),
    );
    assert!(!capped.completeness().licenses_zero_influence());
    assert!(!capped.was_reached(&id("c.md")));
}

#[test]
fn a_traversal_does_not_cross_a_denied_policy_label() {
    let mut graph = DocGraph::new();
    graph.insert_node(spec("open.md", "Open")).unwrap();
    graph
        .insert_node(spec("restricted.md", "Restricted").with_policy_label("phi"))
        .unwrap();
    graph.insert_node(spec("behind.md", "Behind")).unwrap();
    graph.insert_edge(DocEdge::new(
        id("open.md"),
        id("restricted.md"),
        DocEdgeType::Requires,
    ));
    graph.insert_edge(DocEdge::new(
        id("restricted.md"),
        id("behind.md"),
        DocEdgeType::Requires,
    ));

    let walk = traverse(
        &graph,
        &[id("open.md")],
        &TraversalPolicy::exhaustive().denying("phi"),
    );
    assert!(!walk.was_reached(&id("restricted.md")));
    assert!(!walk.was_reached(&id("behind.md")));
    assert_eq!(
        walk.blocked_by_policy.get(&id("restricted.md")).map(String::as_str),
        Some("phi")
    );
    assert!(!walk.completeness().licenses_zero_influence());
}

#[test]
fn filtering_an_edge_type_the_graph_does_not_use_narrows_nothing() {
    let mut graph = DocGraph::new();
    graph.insert_node(spec("a.md", "A")).unwrap();
    graph.insert_node(spec("b.md", "B")).unwrap();
    graph.insert_edge(DocEdge::new(id("a.md"), id("b.md"), DocEdgeType::Requires));

    let policy = TraversalPolicy::exhaustive().not_following(DocEdgeType::Evaluates);
    let walk = traverse(&graph, &[id("a.md")], &policy);
    assert_eq!(walk.completeness(), Completeness::Exhaustive);
}

#[test]
fn a_dangling_edge_met_during_a_walk_voids_the_zero_influence_licence() {
    let mut graph = DocGraph::new();
    graph.insert_node(spec("a.md", "A")).unwrap();
    graph.insert_node(spec("island.md", "Island")).unwrap();
    graph.insert_edge(DocEdge::new(
        id("a.md"),
        id("deleted.md"),
        DocEdgeType::References,
    ));

    let walk = traverse(&graph, &[id("a.md")], &TraversalPolicy::exhaustive());
    assert!(!walk.dangling.is_empty());
    assert!(!walk.completeness().licenses_zero_influence());
}

// ---------------------------------------------------------------- 41.09 bundle compiler

#[test]
fn a_bundle_that_cannot_close_its_mandatory_set_fails_rather_than_truncating() {
    let graph = repository_doc_graph();
    let route = route("tight")
        .must_read(id("docs/ARCHITECTURE.md"))
        .must_read(id("docs/FINDINGS.md"))
        .with_budget(3);
    let error = compile_bundle(&graph, &route, &TraversalPolicy::normative()).unwrap_err();
    match error {
        BundleError::MandatorySetExceedsBudget {
            mandatory_cost,
            budget,
            shortfall,
            ..
        } => {
            assert_eq!(budget, 3);
            assert!(mandatory_cost > 3);
            assert_eq!(shortfall, mandatory_cost - 3);
        }
        other => panic!("expected an explicit budget failure, got {other:?}"),
    }
}

#[test]
fn a_superseded_module_never_appears_in_a_bundle_without_its_successor() {
    let mut graph = DocGraph::new();
    graph.insert_node(spec("old.md", "Old")).unwrap();
    graph.insert_node(spec("new.md", "New")).unwrap();
    graph.insert_edge(DocEdge::new(
        id("new.md"),
        id("old.md"),
        DocEdgeType::Supersedes,
    ));
    let route = route("read-old").must_read(id("old.md"));
    let bundle = compile_bundle(&graph, &route, &TraversalPolicy::normative()).unwrap();
    assert!(bundle.contains(&id("old.md")));
    assert!(bundle.contains(&id("new.md")));
    assert!(bundle.mandatory_ids().any(|module| module == &id("new.md")));
}

#[test]
fn one_side_of_a_contradiction_never_appears_without_the_other_or_its_resolution() {
    let graph = repository_doc_graph();
    let route = route("one-side").must_read(id("blueprint/41.01"));
    let bundle = compile_bundle(&graph, &route, &TraversalPolicy::normative()).unwrap();
    assert!(bundle.contains(&id("blueprint/41.01")));
    assert!(bundle.contains(&id("blueprint/43.01")));
    assert!(bundle.contains(&id("blueprint/41.00")));
}

#[test]
fn a_module_carrying_a_protected_class_is_mandatory_even_when_no_edge_reaches_it() {
    let graph = repository_doc_graph();
    let route = route("unrelated").must_read(id("blueprint/41.03"));
    let bundle = compile_bundle(&graph, &route, &TraversalPolicy::normative()).unwrap();
    assert!(bundle.mandatory_ids().any(|module| module == &id("AGENTS.md")));
    assert!(bundle.protected_classes.contains(&ProtectedClass::ClaimKind));
}

#[test]
fn a_module_unreachable_only_because_an_edge_type_was_filtered_is_not_zero_influence() {
    let mut graph = DocGraph::new();
    graph.insert_node(spec("root.md", "Root")).unwrap();
    graph.insert_node(spec("cited.md", "Cited")).unwrap();
    graph.insert_edge(DocEdge::new(
        id("root.md"),
        id("cited.md"),
        DocEdgeType::References,
    ));
    let route = route("r").must_read(id("root.md"));

    let filtered = compile_bundle(&graph, &route, &TraversalPolicy::normative()).unwrap();
    let omission = filtered
        .omission_for(&id("cited.md"))
        .expect("cited.md is omitted when references are not followed");
    assert_eq!(omission.reason, OmissionReason::NotExamined);
    assert_eq!(omission.influence, InfluenceClass::Unknown);
    assert!(!filtered.is_sufficient());

    let complete = compile_bundle(&graph, &route, &TraversalPolicy::exhaustive()).unwrap();
    assert!(complete.contains(&id("cited.md")));
}

#[test]
fn the_same_unreached_module_is_zero_under_an_exhaustive_walk_and_unknown_under_a_capped_one() {
    let mut graph = DocGraph::new();
    for path in ["root.md", "island.md"] {
        graph.insert_node(spec(path, path)).unwrap();
    }
    let route = route("r").must_read(id("root.md"));

    let exhaustive = compile_bundle(&graph, &route, &TraversalPolicy::exhaustive()).unwrap();
    let proven = exhaustive.omission_for(&id("island.md")).unwrap();
    assert_eq!(proven.reason, OmissionReason::NoPathFromSeeds);
    assert_eq!(proven.influence, InfluenceClass::Zero);
    assert!(exhaustive.is_sufficient());

    let capped = compile_bundle(
        &graph,
        &route,
        &TraversalPolicy::exhaustive().with_max_depth(0),
    )
    .unwrap();
    let unknown = capped.omission_for(&id("island.md")).unwrap();
    assert_eq!(unknown.reason, OmissionReason::NotExamined);
    assert_eq!(unknown.influence, InfluenceClass::Unknown);
    assert!(!capped.is_sufficient());
}

#[test]
fn a_budget_excluded_module_never_counts_toward_a_sufficiency_claim() {
    let mut graph = DocGraph::new();
    graph.insert_node(spec("root.md", "Root")).unwrap();
    let bulky = ModuleNode::new(id("bulky.md"), "bulky.md", "Bulky", NodeStatus::Guide)
        .with_card(ContextCard {
            decision: "a very long decision sentence ".repeat(40),
            ..ContextCard::default()
        })
        .with_hashed_body("# Bulky\n\nlots of text\n");
    graph.insert_node(bulky).unwrap();
    graph.insert_edge(DocEdge::new(
        id("root.md"),
        id("bulky.md"),
        DocEdgeType::References,
    ));

    let root_cost = graph
        .node(&id("root.md"))
        .unwrap()
        .cost_at(ProfileLevel::Contract)
        .tokens;
    let route = route("r")
        .must_read(id("root.md"))
        .with_budget(root_cost + 5);
    let bundle = compile_bundle(&graph, &route, &TraversalPolicy::exhaustive()).unwrap();

    let omission = bundle.omission_for(&id("bulky.md")).expect("dropped");
    assert!(matches!(
        omission.reason,
        OmissionReason::BudgetExcluded { .. }
    ));
    assert_eq!(omission.influence, InfluenceClass::Unknown);
    assert!(matches!(bundle.sufficiency, Sufficiency::NotSufficient { .. }));
}

#[test]
fn a_mandatory_module_behind_a_denied_policy_label_fails_the_compile() {
    let mut graph = DocGraph::new();
    graph
        .insert_node(spec("restricted.md", "Restricted").with_policy_label("phi"))
        .unwrap();
    let route = route("r").must_read(id("restricted.md"));
    let error = compile_bundle(
        &graph,
        &route,
        &TraversalPolicy::exhaustive().denying("phi"),
    )
    .unwrap_err();
    assert!(matches!(
        error,
        BundleError::MandatoryModuleDeniedByPolicy { .. }
    ));
}

#[test]
fn a_route_naming_a_deleted_module_fails_rather_than_compiling_a_smaller_bundle() {
    let graph = repository_doc_graph();
    let route = route("stale").must_read(id("docs/DELETED.md"));
    let error = compile_bundle(&graph, &route, &TraversalPolicy::normative()).unwrap_err();
    assert!(matches!(
        error,
        BundleError::RouteReferencesMissingModule { .. }
    ));
}

#[test]
fn the_rendered_bundle_marks_every_source_boundary_explicitly() {
    let graph = repository_doc_graph();
    let bundle = compile_bundle(
        &graph,
        &repository_routes()[0],
        &TraversalPolicy::normative(),
    )
    .unwrap();
    let rendered = bundle.render_markdown(&graph);
    for module in bundle.module_ids() {
        assert!(rendered.contains(&format!("<!-- BEGIN {module} ")));
        assert!(rendered.contains(&format!("<!-- END {module} -->")));
    }
    assert!(rendered.contains("not a measurement"));
}

#[test]
fn a_rendered_bundle_carries_the_hash_of_every_hashed_module() {
    let graph = repository_doc_graph();
    let bundle = compile_bundle(
        &graph,
        &route("hashes").must_read(id("crate/bioprism-docgraph")),
        &TraversalPolicy::normative(),
    )
    .unwrap();
    let hash = graph
        .node(&id("crate/bioprism-docgraph"))
        .unwrap()
        .hash
        .clone()
        .expect("fixture hashes this node from bytes");
    assert!(bundle.render_markdown(&graph).contains(hash.as_str()));
}

#[test]
fn compiling_the_same_route_twice_selects_the_same_modules() {
    let graph = repository_doc_graph();
    let route = &repository_routes()[1];
    let first = compile_bundle(&graph, route, &TraversalPolicy::exhaustive()).unwrap();
    let second = compile_bundle(&graph, route, &TraversalPolicy::exhaustive()).unwrap();
    let first_ids: Vec<&ModuleId> = first.module_ids().collect();
    let second_ids: Vec<&ModuleId> = second.module_ids().collect();
    assert_eq!(first_ids, second_ids);
    assert_eq!(first.cost.tokens, second.cost.tokens);
}

#[test]
fn every_repository_route_bundles_successfully() {
    let graph = repository_doc_graph();
    for route in repository_routes() {
        let bundle = compile_bundle(&graph, &route, &TraversalPolicy::normative())
            .unwrap_or_else(|error| panic!("route {} failed: {error}", route.id));
        for module in &route.must_read {
            assert!(
                bundle.contains(module),
                "route {} dropped its must-read {module}",
                route.id
            );
        }
    }
}

#[test]
fn a_bundle_serialises_and_reloads_without_losing_its_omission_record() {
    let graph = repository_doc_graph();
    let bundle = compile_bundle(
        &graph,
        &repository_routes()[2],
        &TraversalPolicy::normative(),
    )
    .unwrap();
    let json = serde_json::to_string(&bundle).unwrap();
    let reloaded: bioprism_docgraph::ContextBundle = serde_json::from_str(&json).unwrap();
    assert_eq!(reloaded.omissions, bundle.omissions);
    assert_eq!(reloaded.manifest.groups.len(), bundle.manifest.groups.len());
    assert_eq!(reloaded.sufficiency, bundle.sufficiency);
}

// ---------------------------------------------------------------- 41.10 change impact

#[test]
fn impact_travels_transitively_through_requires() {
    let mut graph = DocGraph::new();
    for path in ["base.md", "middle.md", "leaf.md"] {
        graph.insert_node(spec(path, path)).unwrap();
    }
    graph.insert_edge(DocEdge::new(
        id("middle.md"),
        id("base.md"),
        DocEdgeType::Requires,
    ));
    graph.insert_edge(DocEdge::new(
        id("leaf.md"),
        id("middle.md"),
        DocEdgeType::Requires,
    ));
    let report = impact_of(&graph, &id("base.md"), &[]);
    assert!(report.touches(&id("middle.md")));
    assert!(report.touches(&id("leaf.md")));
}

#[test]
fn impact_stops_after_one_hop_through_example_of_and_records_where_it_stopped() {
    let mut graph = DocGraph::new();
    for path in ["contract.md", "example.md", "cites-example.md"] {
        graph.insert_node(spec(path, path)).unwrap();
    }
    graph.insert_edge(DocEdge::new(
        id("example.md"),
        id("contract.md"),
        DocEdgeType::ExampleOf,
    ));
    graph.insert_edge(DocEdge::new(
        id("cites-example.md"),
        id("example.md"),
        DocEdgeType::References,
    ));
    let report = impact_of(&graph, &id("contract.md"), &[]);
    assert!(report.touches(&id("example.md")));
    assert!(!report.touches(&id("cites-example.md")));
    assert!(report
        .stopped_at
        .iter()
        .any(|stop| stop.module == id("example.md") && stop.via == DocEdgeType::ExampleOf));
}

#[test]
fn a_change_names_the_routes_that_read_the_changed_module() {
    let graph = repository_doc_graph();
    let routes = repository_routes();
    let report = impact_of(&graph, &id("docs/FINDINGS.md"), &routes);
    assert!(report
        .affected_routes
        .contains(&RouteId::parse("report-a-measurement").unwrap()));
}

#[test]
fn a_change_invalidates_every_bundle_that_carried_the_changed_module() {
    let graph = repository_doc_graph();
    let routes = repository_routes();
    let bundle = compile_bundle(&graph, &routes[1], &TraversalPolicy::normative()).unwrap();
    let report = impact_of(&graph, &id("docs/FINDINGS.md"), &routes);
    assert!(report.invalidates(&bundle));
    assert!(report
        .invalidated_entries(&bundle)
        .contains(&&id("docs/FINDINGS.md")));

    let unrelated = impact_of(&graph, &id(".agents/skills/check-parity/SKILL.md"), &routes);
    assert!(!unrelated.invalidates(&bundle));
}

// ---------------------------------------------------------------- 41.11 linting

#[test]
fn the_linter_finds_a_cycle_over_an_edge_type_whose_meaning_is_an_ordering() {
    let report = lint(&broken_graph(), &[]);
    let cycle = report
        .findings
        .iter()
        .find_map(|finding| match finding {
            LintFinding::CycleInAcyclicEdgeType { kind, cycle } => Some((*kind, cycle.clone())),
            _ => None,
        })
        .expect("the broken fixture contains a requires cycle");
    assert_eq!(cycle.0, DocEdgeType::Requires);
    assert!(cycle.1.len() >= 3);
}

#[test]
fn the_linter_finds_a_contradiction_that_names_nothing_to_resolve_it() {
    let report = lint(&broken_graph(), &[]);
    assert_eq!(report.with_code("unresolved_contradiction").count(), 1);
}

#[test]
fn a_contradiction_whose_resolution_is_named_is_not_reported() {
    let report = lint(&repository_doc_graph(), &[]);
    assert_eq!(report.with_code("unresolved_contradiction").count(), 0);
}

#[test]
fn the_linter_finds_a_link_to_a_file_that_is_not_in_the_registry() {
    let report = lint(&broken_graph(), &[]);
    assert!(report.with_code("dangling_edge").count() >= 1);
    assert!(report.has_errors());
}

#[test]
fn the_linter_finds_a_withdrawn_module_with_no_successor() {
    let report = lint(&broken_graph(), &[]);
    assert_eq!(report.with_code("withdrawn_without_successor").count(), 1);
}

#[test]
fn a_route_whose_budget_cannot_be_met_is_found_before_an_agent_asks_for_the_bundle() {
    let graph = repository_doc_graph();
    let route = route("impossible")
        .must_read(id("docs/ARCHITECTURE.md"))
        .with_budget(1);
    let report = lint(&graph, &[route]);
    assert_eq!(report.with_code("route_budget_unsatisfiable").count(), 1);
}

#[test]
fn the_repository_fixture_reports_the_two_docs_that_share_one_heading() {
    let report = lint(&repository_doc_graph(), &[]);
    let duplicate = report
        .findings
        .iter()
        .find_map(|finding| match finding {
            LintFinding::DuplicateTitle { title, modules } => Some((title.clone(), modules.clone())),
            _ => None,
        })
        .expect("two comparison documents share an H1 in this repository");
    assert_eq!(duplicate.0, "Equal-engineering context comparison");
    assert_eq!(duplicate.1.len(), 2);
}

#[test]
fn the_repository_fixture_reports_the_document_with_no_heading() {
    let report = lint(&repository_doc_graph(), &[]);
    assert!(report.findings.iter().any(|finding| matches!(
        finding,
        LintFinding::MissingTitle { module } if module == &id("CLAUDE.md")
    )));
}

#[test]
fn the_repository_fixture_reports_the_architecture_document_nobody_links_to() {
    let report = lint(&repository_doc_graph(), &[]);
    let orphans: Vec<&ModuleId> = report
        .with_code("orphan_module")
        .filter_map(|finding| match finding {
            LintFinding::OrphanModule { module } => Some(module),
            _ => None,
        })
        .collect();
    assert!(
        orphans.contains(&&id("docs/ARCHITECTURE.md")),
        "neither README.md nor AGENTS.md links into docs/, so ARCHITECTURE.md has no inbound edge"
    );
}

#[test]
fn an_orphan_guide_is_not_reported_but_an_orphan_specification_is() {
    let mut graph = DocGraph::new();
    graph.insert_node(spec("contract.md", "Contract")).unwrap();
    graph
        .insert_node(ModuleNode::new(
            id("readme.md"),
            "readme.md",
            "Readme",
            NodeStatus::Guide,
        ))
        .unwrap();
    let report = lint(&graph, &[]);
    let orphans: Vec<&LintFinding> = report.with_code("orphan_module").collect();
    assert_eq!(orphans.len(), 1);
    assert!(matches!(
        orphans[0],
        LintFinding::OrphanModule { module } if module == &id("contract.md")
    ));
}

#[test]
fn a_module_declaring_a_protected_class_must_carry_it_in_its_card() {
    let mut graph = DocGraph::new();
    graph
        .insert_node(spec("boundary.md", "Boundary").with_protected(ProtectedClass::AccessAndConsent))
        .unwrap();
    let report = lint(&graph, &[]);
    assert_eq!(report.with_code("card_omits_protected_invariant").count(), 1);
    assert!(report
        .errors()
        .any(|finding| finding.code() == "card_omits_protected_invariant"));
}

#[test]
fn lint_findings_are_data_and_a_broken_corpus_still_produces_a_full_report() {
    let report = lint(&broken_graph(), &[]);
    assert!(report.findings.len() > 3);
    assert!(report.counts().len() > 2);
    assert!(report.warnings().count() > 0);
    assert!(report
        .errors()
        .all(|finding| finding.severity() == LintSeverity::Error));
}

// ---------------------------------------------------------------- markdown reader

#[test]
fn a_hash_inside_a_fenced_code_block_is_not_a_heading() {
    let text = "# Real\n\n```sh\n# not a heading\n```\n\n## Also real\n";
    let parsed = parse_document("t.md", text).unwrap();
    let found = headings(parsed.body);
    assert_eq!(found.len(), 2);
    assert_eq!(first_h1(parsed.body).as_deref(), Some("Real"));
}

#[test]
fn an_unterminated_front_matter_fence_is_an_error_not_an_absent_front_matter() {
    let error = parse_document("t.md", "---\ntitle: \"x\"\n\n# Body\n").unwrap_err();
    assert!(matches!(error, DocGraphError::MalformedFrontMatter { .. }));
    let none = parse_document("t.md", "# Body\n").unwrap();
    assert!(none.front_matter.is_none());
}

#[test]
fn front_matter_values_are_unquoted_and_the_body_starts_after_the_fence() {
    let parsed = parse_document(
        "t.md",
        "---\ntitle: \"Documentation Edge Vocabulary\"\nmodule_id: \"41.03\"\n---\n# Heading\n",
    )
    .unwrap();
    let matter = parsed.front_matter.expect("front matter present");
    assert_eq!(matter.get("module_id"), Some("41.03"));
    assert_eq!(matter.get("title"), Some("Documentation Edge Vocabulary"));
    assert_eq!(first_h1(parsed.body).as_deref(), Some("Heading"));
}

#[test]
fn link_targets_are_read_outside_code_fences_only() {
    let body = "See [a](docs/A.md) and [b](https://example.com).\n\n```\n[c](docs/C.md)\n```\n";
    let targets = link_targets(body);
    assert_eq!(targets, vec!["docs/A.md", "https://example.com"]);
}

// ---------------------------------------------------------------- 41.12 reading protocol

#[test]
fn claiming_what_is_built_from_a_specification_node_is_a_violation() {
    let graph = repository_doc_graph();
    let bundle = compile_bundle(
        &graph,
        &route("r").must_read(id("blueprint/41.03")),
        &TraversalPolicy::normative(),
    )
    .unwrap();
    let mut receipt = ReadingReceipt::new(RouteId::parse("r").unwrap());
    for module in bundle.mandatory_ids() {
        receipt = receipt.loading(module.clone(), ProfileLevel::Brief);
    }
    receipt = receipt.claiming(Claim {
        statement: "the edge vocabulary is implemented".to_string(),
        sourced_from: id("blueprint/41.03"),
        kind: ClaimKind::WhatIsBuilt,
    });
    receipt.unresolved_obligations.push("none checked".to_string());
    let violations = check_receipt(&graph, &bundle, &receipt);
    assert!(violations.iter().any(|violation| matches!(
        violation,
        ProtocolViolation::ImpliedImplementationFromSpecification { module, .. }
            if module == &id("blueprint/41.03")
    )));
}

#[test]
fn citing_a_heading_the_module_does_not_have_is_a_violation() {
    let graph = repository_doc_graph();
    let bundle = compile_bundle(
        &graph,
        &route("r").must_read(id("crate/bioprism-docgraph")),
        &TraversalPolicy::normative(),
    )
    .unwrap();
    let mut receipt = ReadingReceipt::new(RouteId::parse("r").unwrap());
    for module in bundle.mandatory_ids() {
        receipt = receipt.loading(module.clone(), ProfileLevel::Card);
    }
    receipt = receipt.citing(Citation {
        module: id("crate/bioprism-docgraph"),
        heading: "A section that does not exist".to_string(),
        level: ProfileLevel::Contract,
    });
    let violations = check_receipt(&graph, &bundle, &receipt);
    assert!(violations
        .iter()
        .any(|violation| matches!(violation, ProtocolViolation::CitedMissingHeading { .. })));
}

#[test]
fn a_real_heading_cited_at_a_normative_level_is_not_a_violation() {
    let graph = repository_doc_graph();
    let bundle = compile_bundle(
        &graph,
        &route("r").must_read(id("crate/bioprism-docgraph")),
        &TraversalPolicy::normative(),
    )
    .unwrap();
    let receipt = ReadingReceipt::new(RouteId::parse("r").unwrap()).citing(Citation {
        module: id("crate/bioprism-docgraph"),
        heading: "What it implements".to_string(),
        level: ProfileLevel::Contract,
    });
    let violations = check_receipt(&graph, &bundle, &receipt);
    assert!(!violations
        .iter()
        .any(|violation| matches!(violation, ProtocolViolation::CitedMissingHeading { .. })));
}

#[test]
fn an_insufficient_bundle_read_without_reporting_any_gap_is_a_violation() {
    let mut graph = DocGraph::new();
    graph.insert_node(spec("root.md", "Root")).unwrap();
    graph.insert_node(spec("cited.md", "Cited")).unwrap();
    graph.insert_edge(DocEdge::new(
        id("root.md"),
        id("cited.md"),
        DocEdgeType::References,
    ));
    let bundle = compile_bundle(
        &graph,
        &route("r").must_read(id("root.md")),
        &TraversalPolicy::normative(),
    )
    .unwrap();
    assert!(!bundle.is_sufficient());
    let mut receipt = ReadingReceipt::new(RouteId::parse("r").unwrap());
    for module in bundle.mandatory_ids() {
        receipt = receipt.loading(module.clone(), ProfileLevel::Contract);
    }
    let violations = check_receipt(&graph, &bundle, &receipt);
    assert!(violations
        .iter()
        .any(|violation| matches!(violation, ProtocolViolation::SufficiencyGapNotReported { .. })));
}

#[test]
fn reading_a_module_the_bundle_did_not_deliver_is_a_violation() {
    let graph = repository_doc_graph();
    let bundle = compile_bundle(
        &graph,
        &route("r").must_read(id("blueprint/41.03")),
        &TraversalPolicy::normative(),
    )
    .unwrap();
    let receipt = ReadingReceipt::new(RouteId::parse("r").unwrap())
        .loading(id("docs/BASELINE_COMPARISON.md"), ProfileLevel::Contract);
    let violations = check_receipt(&graph, &bundle, &receipt);
    assert!(violations
        .iter()
        .any(|violation| matches!(violation, ProtocolViolation::LoadedOutsideBundle { .. })));
}
