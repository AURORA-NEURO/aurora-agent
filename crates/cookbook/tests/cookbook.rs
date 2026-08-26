//! Acceptance tests for the cookbook.
//!
//! Three groups, and they check different kinds of thing. The first group checks that a recipe
//! *cannot* be vacuous — those are tests about the type, and they would still be worth writing if
//! the catalogue were empty. The second group checks that the catalogue is not lying about the
//! workspace, by resolving every name it uses against the working tree. The third checks that a
//! recipe really is a task route: that it goes through `bioprism-docgraph`'s compiler and comes out
//! with the guarantees that compiler makes, none of them weakened here.

use bioprism_cookbook::graph::{
    anti_recipe_route, asserted_crate_dependencies, compile_anti_recipe_route,
    compile_recipe_route, cookbook_doc_graph, declared_crate_nodes, declared_doc_nodes,
    every_omission_supports_sufficiency, omissions_by_influence, recipe_route, HOUSE_RULES,
};
use bioprism_cookbook::verify::{exported_items, test_function_present};
use bioprism_cookbook::{
    standard_cookbook, AntiRecipe, Check, CheckableProperty, Claim, Cookbook, CookbookError,
    CookbookReport, CrateName, EntryPoint, Pitfall, Recipe, RecipeId, ReferenceStatus, RouteError,
    Step, TestStatus, Workspace, WorkspaceTest, ROUTE_BUDGET,
};
use bioprism_docgraph::registry::ModuleId;
use bioprism_docgraph::{lint, BundleError, Sufficiency, TraversalPolicy};
use bioprism_section::InfluenceClass;

fn book() -> Cookbook {
    standard_cookbook().expect("the shipped catalogue is well formed")
}

fn workspace() -> Workspace {
    Workspace::here().expect("the workspace this crate was compiled in is readable")
}

fn id(value: &str) -> RecipeId {
    RecipeId::parse(value).expect("well-formed test id")
}

fn krate(value: &str) -> CrateName {
    CrateName::parse(value).expect("well-formed crate name")
}

fn entry(value: &str) -> EntryPoint {
    EntryPoint::parse(value).expect("well-formed entry point")
}

fn a_step() -> Step {
    Step::new(krate("bioprism-fiber"), "compile the query")
        .calling(entry("bioprism_fiber::compile"))
}

fn a_property() -> CheckableProperty {
    CheckableProperty::new(
        "the compile refuses rather than truncating",
        Check::EnforcedByTest(WorkspaceTest::new(
            krate("bioprism-docgraph"),
            "crates/docgraph/tests/acceptance.rs",
            "a_bundle_that_cannot_close_its_mandatory_set_fails_rather_than_truncating",
        )),
    )
}

fn a_pitfall() -> Pitfall {
    Pitfall::new("retrying at a larger budget", "the refusal is the finding")
}

// ---------------------------------------------------------------------------
// A recipe cannot be vacuous
// ---------------------------------------------------------------------------

#[test]
fn a_recipe_without_a_checkable_property_cannot_be_constructed() {
    let outcome = Recipe::draft(id("no-property"), "do a thing")
        .step(a_step())
        .demonstrating(Claim::new("something is true"))
        .easy_to_get_wrong(a_pitfall())
        .seal();
    assert!(matches!(
        outcome,
        Err(CookbookError::NoCheckableProperty(_))
    ));
}

#[test]
fn a_recipe_without_steps_is_a_title_and_is_refused() {
    let outcome = Recipe::draft(id("no-steps"), "do a thing")
        .demonstrating(Claim::new("something is true"))
        .checked_by(a_property())
        .easy_to_get_wrong(a_pitfall())
        .seal();
    assert!(matches!(outcome, Err(CookbookError::NoSteps(_))));
}

#[test]
fn a_recipe_that_demonstrates_no_claim_is_refused() {
    let outcome = Recipe::draft(id("no-claim"), "do a thing")
        .step(a_step())
        .checked_by(a_property())
        .easy_to_get_wrong(a_pitfall())
        .seal();
    assert!(matches!(outcome, Err(CookbookError::NoClaim(_))));
}

#[test]
fn a_recipe_that_names_nothing_easy_to_get_wrong_is_refused() {
    let outcome = Recipe::draft(id("no-pitfall"), "do a thing")
        .step(a_step())
        .demonstrating(Claim::new("something is true"))
        .checked_by(a_property())
        .seal();
    assert!(matches!(outcome, Err(CookbookError::NoPitfall(_))));
}

#[test]
fn a_recipe_with_a_blank_goal_is_refused() {
    let outcome = Recipe::draft(id("blank-goal"), "   ")
        .step(a_step())
        .demonstrating(Claim::new("something is true"))
        .checked_by(a_property())
        .easy_to_get_wrong(a_pitfall())
        .seal();
    assert!(matches!(
        outcome,
        Err(CookbookError::EmptyField { field: "goal", .. })
    ));
}

#[test]
fn an_observable_check_with_no_expectation_is_refused() {
    let outcome = Recipe::draft(id("empty-observable"), "do a thing")
        .step(a_step())
        .demonstrating(Claim::new("something is true"))
        .checked_by(CheckableProperty::new(
            "it holds",
            Check::Observable {
                observe: "the report".to_string(),
                expect: "  ".to_string(),
            },
        ))
        .easy_to_get_wrong(a_pitfall())
        .seal();
    assert!(matches!(
        outcome,
        Err(CookbookError::EmptyField {
            field: "observable check",
            ..
        })
    ));
}

#[test]
fn a_recipe_whose_property_is_dropped_in_transit_fails_to_deserialise() {
    let recipe = book().recipes()[0].clone();
    let mut wire = serde_json::to_value(&recipe).expect("a recipe serialises");
    wire.as_object_mut()
        .expect("the wire form is an object")
        .insert("properties".to_string(), serde_json::json!([]));
    let reloaded: Result<Recipe, _> = serde_json::from_value(wire);
    assert!(
        reloaded.is_err(),
        "a recipe that lost its checkable property must not parse back into a Recipe"
    );
}

#[test]
fn a_recipe_round_trips_through_serde_without_losing_a_check() {
    for recipe in book().recipes() {
        let bytes = serde_json::to_vec(recipe).expect("a recipe serialises");
        let reloaded: Recipe = serde_json::from_slice(&bytes).expect("a recipe reloads");
        assert_eq!(&reloaded, recipe);
        assert_eq!(reloaded.properties().len(), recipe.properties().len());
    }
}

#[test]
fn an_entry_point_must_be_rooted_at_a_crate() {
    assert!(EntryPoint::parse("compile").is_err());
    assert!(EntryPoint::parse("bioprism_fiber::compile").is_ok());
}

#[test]
fn an_entry_point_with_a_non_identifier_segment_is_refused() {
    assert!(EntryPoint::parse("bioprism_fiber::compile()").is_err());
    assert!(EntryPoint::parse("bioprism_fiber::").is_err());
    assert!(EntryPoint::parse("bioprism-fiber::compile").is_err());
}

#[test]
fn an_entry_point_reports_the_package_name_not_the_path_segment() {
    assert_eq!(
        entry("bioprism_docgraph::fixture::repository_doc_graph")
            .crate_name()
            .as_str(),
        "bioprism-docgraph"
    );
}

#[test]
fn a_recipe_id_with_whitespace_or_capitals_is_refused() {
    assert!(RecipeId::parse("two words").is_err());
    assert!(RecipeId::parse("NotLowercase").is_err());
    assert!(RecipeId::parse("").is_err());
    assert!(RecipeId::parse("compile-a-context").is_ok());
}

// ---------------------------------------------------------------------------
// An anti-recipe cannot be an unrefuted opinion
// ---------------------------------------------------------------------------

#[test]
fn an_anti_recipe_cannot_be_constructed_without_a_test_that_already_refutes_it() {
    let wire = serde_json::json!({
        "id": "unenforced-warning",
        "attempt": "do the wrong thing",
        "why_it_fails": "because it is wrong",
        "instead": "do the right thing",
        "enforced_by": []
    });
    let reloaded: Result<AntiRecipe, _> = serde_json::from_value(wire);
    assert!(
        reloaded.is_err(),
        "an anti-recipe with no enforcing test is an opinion and must not parse"
    );
}

#[test]
fn an_anti_recipe_round_trips_through_serde_with_its_house_rule() {
    for anti in book().anti_recipes() {
        let bytes = serde_json::to_vec(anti).expect("an anti-recipe serialises");
        let reloaded: AntiRecipe = serde_json::from_slice(&bytes).expect("it reloads");
        assert_eq!(&reloaded, anti);
    }
}

#[test]
fn every_anti_recipe_names_a_test_and_an_alternative() {
    for anti in book().anti_recipes() {
        assert!(
            !anti.enforced_by().is_empty(),
            "{} states no test",
            anti.id()
        );
        assert!(
            !anti.instead().trim().is_empty(),
            "{} scolds without offering a move",
            anti.id()
        );
    }
}

#[test]
fn the_three_distinctions_the_house_rules_refuse_to_lose_each_have_an_anti_recipe() {
    let book = book();
    for required in [
        "cheapness-is-not-admissibility",
        "a-right-answer-from-an-incomplete-closure-is-not-a-pass",
        "instance-count-is-not-benchmark-count",
        "an-unreached-node-is-not-a-proven-zero",
        "an-estimate-is-not-a-measurement",
    ] {
        assert!(
            book.anti_recipe(required).is_ok(),
            "no anti-recipe for `{required}`"
        );
    }
}

// ---------------------------------------------------------------------------
// The shape of the catalogue
// ---------------------------------------------------------------------------

#[test]
fn the_catalogue_ships_between_eight_and_twelve_recipes() {
    let book = book();
    let count = book.recipes().len();
    assert!(
        (8..=12).contains(&count),
        "{count} recipes: fewer honest recipes beat twenty thin ones, but eight is the floor"
    );
}

#[test]
fn recipe_ids_are_unique_across_recipes_and_anti_recipes() {
    let book = book();
    let mut ids: Vec<String> = book
        .recipes()
        .iter()
        .map(|recipe| recipe.id().to_string())
        .chain(book.anti_recipes().iter().map(|anti| anti.id().to_string()))
        .collect();
    let total = ids.len();
    ids.sort();
    ids.dedup();
    assert_eq!(ids.len(), total, "an id is registered twice");
}

#[test]
fn a_duplicate_id_across_the_two_lists_is_refused_rather_than_shadowed() {
    let book = book();
    let recipe = book.recipes()[0].clone();
    let clashing = AntiRecipe::new(
        recipe.id().clone(),
        "attempt",
        "why",
        "instead",
        WorkspaceTest::new(
            krate("bioprism-docgraph"),
            "crates/docgraph/tests/acceptance.rs",
            "only_an_exhaustive_walk_licenses_a_zero_influence_claim",
        ),
    )
    .expect("the anti-recipe itself is well formed");
    let outcome = Cookbook::from_parts(vec![recipe], vec![clashing]);
    assert!(matches!(outcome, Err(CookbookError::DuplicateRecipe(_))));
}

#[test]
fn asking_for_an_unregistered_recipe_is_a_typed_error() {
    assert!(matches!(
        book().recipe("no-such-recipe"),
        Err(CookbookError::UnknownRecipe(_))
    ));
}

#[test]
fn the_six_task_areas_the_catalogue_promises_each_have_a_recipe() {
    let book = book();
    for required in [
        "compile-a-context-and-read-its-certificate",
        "check-a-split-for-leakage",
        "compare-context-strategies-on-equal-engineering",
        "generate-a-world-and-characterise-its-structure",
        "run-a-mutation-family",
        "audit-what-the-platform-cannot-demonstrate",
    ] {
        assert!(book.recipe(required).is_ok(), "no recipe for `{required}`");
    }
}

#[test]
fn every_recipe_cites_a_blueprint_module_and_names_an_entry_point() {
    for recipe in book().recipes() {
        assert!(
            !recipe.claim().blueprint_modules.is_empty(),
            "{} cites no blueprint module",
            recipe.id()
        );
        assert!(
            !recipe.entry_points().is_empty(),
            "{} names no entry point, so nothing about it can be verified",
            recipe.id()
        );
    }
}

#[test]
fn a_recipe_never_lists_the_house_rules_as_optional_reading() {
    for recipe in book().recipes() {
        assert!(
            !recipe.reading().iter().any(|module| module == HOUSE_RULES),
            "{} lists the house rules as optional; a boundary is not optional",
            recipe.id()
        );
    }
}

#[test]
fn every_recipe_states_a_pitfall_distinct_from_its_claim() {
    for recipe in book().recipes() {
        assert_ne!(recipe.pitfall().mistake, recipe.claim().statement);
        assert!(!recipe.pitfall().why.trim().is_empty());
    }
}

// ---------------------------------------------------------------------------
// The catalogue is not lying about the workspace
// ---------------------------------------------------------------------------

#[test]
fn every_crate_a_recipe_names_is_a_workspace_member() {
    let workspace = workspace();
    for krate in book().crates() {
        assert!(
            workspace.contains_package(&krate),
            "`{krate}` is named by a recipe and is not a workspace member"
        );
    }
}

#[test]
fn every_entry_point_a_recipe_names_exists_in_the_workspace() {
    let workspace = workspace();
    let mut missing = Vec::new();
    for recipe in book().recipes() {
        for entry in recipe.entry_points() {
            let status = workspace.resolve(entry);
            if !status.is_present() {
                missing.push(format!("{} names {entry}: {status:?}", recipe.id()));
            }
        }
    }
    assert!(missing.is_empty(), "{}", missing.join("\n"));
}

#[test]
fn every_enforcing_test_a_recipe_names_still_exists_by_name() {
    let workspace = workspace();
    let mut missing = Vec::new();
    for test in book().enforcing_tests() {
        let status = workspace.resolve_test(&test);
        if !status.is_present() {
            missing.push(format!("{}::{} — {status:?}", test.path, test.name));
        }
    }
    assert!(missing.is_empty(), "{}", missing.join("\n"));
}

#[test]
fn every_quotation_this_cookbook_attributes_to_another_file_is_still_there() {
    let workspace = workspace();
    let mut reworded = Vec::new();
    for quote in book().quotes() {
        let status = workspace.resolve_quote(&quote);
        if !status.is_present() {
            reworded.push(format!("{}: `{}` — {status:?}", quote.source, quote.needle));
        }
    }
    assert!(reworded.is_empty(), "{}", reworded.join("\n"));
}

#[test]
fn the_whole_catalogue_resolves_against_the_working_tree() {
    let report = book().verify(&workspace());
    assert!(report.is_clean(), "{}", report.render());
}

#[test]
fn an_entry_point_naming_a_deleted_function_is_reported_rather_than_ignored() {
    let status = workspace().resolve(&entry("bioprism_fiber::a_function_that_was_deleted"));
    assert!(matches!(status, ReferenceStatus::ItemNotExported { .. }));
}

#[test]
fn an_entry_point_in_a_crate_that_does_not_exist_is_reported_as_such() {
    let status = workspace().resolve(&entry("bioprism_not_a_crate::anything"));
    assert!(matches!(
        status,
        ReferenceStatus::CrateNotInWorkspace { .. }
    ));
}

#[test]
fn an_entry_point_through_a_private_module_is_reported_at_the_module_not_the_item() {
    let status = workspace().resolve(&entry("bioprism_docgraph::not_a_module::something"));
    assert!(matches!(status, ReferenceStatus::ModuleNotExported { .. }));
}

#[test]
fn a_renamed_test_is_reported_as_a_missing_function_not_a_missing_file() {
    let status = workspace().resolve_test(&WorkspaceTest::new(
        krate("bioprism-docgraph"),
        "crates/docgraph/tests/acceptance.rs",
        "a_test_that_was_renamed_last_week",
    ));
    assert_eq!(status, TestStatus::FunctionMissing);
}

#[test]
fn a_test_attributed_to_the_wrong_crate_is_reported_separately_from_a_missing_one() {
    let status = workspace().resolve_test(&WorkspaceTest::new(
        krate("bioprism-cookbook"),
        "crates/docgraph/tests/acceptance.rs",
        "only_an_exhaustive_walk_licenses_a_zero_influence_claim",
    ));
    assert!(matches!(
        status,
        TestStatus::PathIsNotInsideTheNamedCrate { .. }
    ));
}

#[test]
fn an_unreadable_workspace_is_an_error_rather_than_a_clean_cookbook() {
    assert!(Workspace::open("no/such/checkout").is_err());
}

#[test]
fn the_export_reader_takes_the_alias_a_caller_would_have_to_write() {
    let exports = exported_items("pub use inner::Thing as Renamed;\n");
    assert!(exports.contains("Renamed"));
    assert!(!exports.contains("Thing"));
}

#[test]
fn the_export_reader_reads_a_pub_use_that_spans_several_lines() {
    let exports = exported_items(
        "pub use bundle::{\n    compile_bundle, BundleEntry,\n    ContextBundle,\n};\n",
    );
    for name in ["compile_bundle", "BundleEntry", "ContextBundle"] {
        assert!(exports.contains(name), "{name} was not read");
    }
    assert!(!exports.contains("bundle"));
}

#[test]
fn the_export_reader_does_not_treat_a_glob_as_a_name() {
    let exports = exported_items("pub use inner::*;\n");
    assert!(exports.is_empty());
}

#[test]
fn a_test_reference_matches_the_declaration_and_not_a_call_to_it() {
    let source = "fn helper() {}\n\n#[test]\nfn the_real_one() {\n    helper();\n}\n";
    assert!(test_function_present(source, "the_real_one"));
    assert!(test_function_present(source, "helper"));
    assert!(!test_function_present(source, "the_real"));
}

#[test]
fn every_crate_dependency_edge_the_graph_asserts_is_a_real_cargo_dependency() {
    let workspace = workspace();
    let mut wrong = Vec::new();
    for (from, to) in asserted_crate_dependencies() {
        let Some(directory) = workspace.directory_of(&from) else {
            wrong.push(format!("`{from}` is not a workspace member"));
            continue;
        };
        let Ok(manifest) = workspace.read(&format!("{directory}/Cargo.toml")) else {
            wrong.push(format!("`{from}` has no readable manifest"));
            continue;
        };
        let declared = manifest
            .lines()
            .map(str::trim)
            .any(|line| line.starts_with(&format!("{to} = ")));
        if !declared {
            wrong.push(format!("`{from}` does not depend on `{to}`"));
        }
    }
    assert!(wrong.is_empty(), "{}", wrong.join("\n"));
}

#[test]
fn every_crate_the_graph_declares_a_node_for_is_a_workspace_member() {
    let workspace = workspace();
    for krate in declared_crate_nodes() {
        assert!(
            workspace.contains_package(&krate),
            "the graph declares a node for `{krate}`, which is not a workspace member"
        );
    }
}

#[test]
fn every_document_the_graph_declares_a_node_for_exists_on_disk() {
    let workspace = workspace();
    for path in declared_doc_nodes() {
        assert!(
            workspace.read(path).is_ok(),
            "the graph declares a node for `{path}`, which is not in the working tree"
        );
    }
}

// ---------------------------------------------------------------------------
// A recipe is a task route, with the compiler's guarantees intact
// ---------------------------------------------------------------------------

#[test]
fn every_recipe_route_compiles_within_its_declared_budget() {
    let book = book();
    let graph = cookbook_doc_graph(&book).expect("the cookbook graph builds");
    let policy = TraversalPolicy::exhaustive();
    for recipe in book.recipes() {
        let bundle = compile_recipe_route(&graph, recipe, &policy)
            .unwrap_or_else(|error| panic!("{}: {error}", recipe.id()));
        assert_eq!(bundle.budget, Some(ROUTE_BUDGET));
        assert!(
            bundle.cost.tokens <= ROUTE_BUDGET,
            "{} costs {} against a budget of {ROUTE_BUDGET}",
            recipe.id(),
            bundle.cost.tokens
        );
    }
}

#[test]
fn every_recipe_route_passes_the_41_05_checks_without_a_defect() {
    let book = book();
    let graph = cookbook_doc_graph(&book).expect("the cookbook graph builds");
    for recipe in book.recipes() {
        let route = recipe_route(recipe).expect("a route builds");
        let defects = route.check(&graph);
        assert!(defects.is_empty(), "{}: {defects:?}", recipe.id());
    }
}

#[test]
fn a_recipe_route_whose_budget_is_below_its_mandatory_set_fails_rather_than_truncating() {
    let book = book();
    let graph = cookbook_doc_graph(&book).expect("the cookbook graph builds");
    let recipe = book
        .recipe("compile-a-context-and-read-its-certificate")
        .expect("the recipe is registered");
    let mut route = recipe_route(recipe).expect("a route builds");
    route.budget = Some(10);
    let outcome = bioprism_docgraph::compile_bundle(&graph, &route, &TraversalPolicy::exhaustive());
    assert!(matches!(
        outcome,
        Err(BundleError::MandatorySetExceedsBudget { .. })
    ));
}

#[test]
fn the_house_rules_are_in_every_recipe_bundle() {
    let book = book();
    let graph = cookbook_doc_graph(&book).expect("the cookbook graph builds");
    let rules = ModuleId::parse(HOUSE_RULES).expect("a well-formed id");
    let policy = TraversalPolicy::exhaustive();
    for recipe in book.recipes() {
        let bundle = compile_recipe_route(&graph, recipe, &policy).expect("it compiles");
        assert!(
            bundle.contains(&rules),
            "{} was delivered without the house rules",
            recipe.id()
        );
        assert!(!bundle.protected_classes.is_empty());
    }
}

#[test]
fn a_recipe_bundle_carries_every_crate_its_steps_name() {
    let book = book();
    let graph = cookbook_doc_graph(&book).expect("the cookbook graph builds");
    let policy = TraversalPolicy::exhaustive();
    for recipe in book.recipes() {
        let bundle = compile_recipe_route(&graph, recipe, &policy).expect("it compiles");
        let mandatory: Vec<String> = bundle.mandatory_ids().map(ToString::to_string).collect();
        for krate in recipe.crates() {
            let module = format!("crate/{krate}");
            assert!(
                mandatory.contains(&module),
                "{} names `{krate}` and its bundle does not carry `{module}` as mandatory",
                recipe.id()
            );
        }
    }
}

#[test]
fn a_recipe_bundle_closes_over_the_dependencies_of_the_crates_it_names() {
    let book = book();
    let graph = cookbook_doc_graph(&book).expect("the cookbook graph builds");
    let recipe = book
        .recipe("fork-two-architectures-from-one-decision-cell")
        .expect("the recipe is registered");
    let bundle =
        compile_recipe_route(&graph, recipe, &TraversalPolicy::exhaustive()).expect("it compiles");
    let mandatory: Vec<String> = bundle.mandatory_ids().map(ToString::to_string).collect();
    for transitive in [
        "crate/bioprism-section",
        "crate/bioprism-world",
        "crate/bioprism-ids",
    ] {
        assert!(
            mandatory.contains(&transitive.to_string()),
            "the prism recipe does not close over `{transitive}`"
        );
    }
}

#[test]
fn compiling_the_same_recipe_route_twice_selects_the_same_modules() {
    let book = book();
    let graph = cookbook_doc_graph(&book).expect("the cookbook graph builds");
    let recipe = book.recipes()[0].clone();
    let policy = TraversalPolicy::exhaustive();
    let first = compile_recipe_route(&graph, &recipe, &policy).expect("it compiles");
    let second = compile_recipe_route(&graph, &recipe, &policy).expect("it compiles");
    assert_eq!(first.entries, second.entries);
    assert_eq!(first.omissions, second.omissions);
    assert_eq!(first.cost, second.cost);
}

#[test]
fn the_cookbook_graph_is_deterministic_across_two_builds() {
    let book = book();
    let first = cookbook_doc_graph(&book).expect("it builds");
    let second = cookbook_doc_graph(&book).expect("it builds");
    assert_eq!(first, second);
}

#[test]
fn every_omission_in_a_recipe_bundle_carries_an_influence_class() {
    let book = book();
    let graph = cookbook_doc_graph(&book).expect("the cookbook graph builds");
    let policy = TraversalPolicy::exhaustive();
    for recipe in book.recipes() {
        let bundle = compile_recipe_route(&graph, recipe, &policy).expect("it compiles");
        let counted: usize = bundle.manifest.groups.iter().map(|group| group.count).sum();
        assert_eq!(
            counted,
            bundle.omissions.len(),
            "{} has an omission outside the manifest",
            recipe.id()
        );
    }
}

#[test]
fn a_capped_walk_never_licenses_a_sufficiency_claim_that_an_exhaustive_one_would() {
    let book = book();
    let graph = cookbook_doc_graph(&book).expect("the cookbook graph builds");
    let recipe = book.recipes()[0].clone();
    let mut route = recipe_route(&recipe).expect("a route builds");
    route.budget = None;

    let exhaustive =
        bioprism_docgraph::compile_bundle(&graph, &route, &TraversalPolicy::exhaustive())
            .expect("it compiles");
    let capped = bioprism_docgraph::compile_bundle(
        &graph,
        &route,
        &TraversalPolicy::exhaustive().with_max_depth(1),
    )
    .expect("it compiles");

    assert_eq!(exhaustive.sufficiency, Sufficiency::Sufficient);
    assert!(matches!(
        capped.sufficiency,
        Sufficiency::NotSufficient { .. }
    ));
}

#[test]
fn the_token_cost_of_a_recipe_bundle_is_labelled_an_estimate() {
    let book = book();
    let graph = cookbook_doc_graph(&book).expect("the cookbook graph builds");
    let bundle = compile_recipe_route(&graph, &book.recipes()[0], &TraversalPolicy::exhaustive())
        .expect("it compiles");
    assert!(
        !bundle.cost.is_measurement(),
        "no tokenizer exists in this workspace, so no bundle cost may be a measurement"
    );
}

#[test]
fn every_anti_recipe_route_compiles_and_carries_the_rule_it_sharpens() {
    let book = book();
    let graph = cookbook_doc_graph(&book).expect("the cookbook graph builds");
    let rules = ModuleId::parse(HOUSE_RULES).expect("a well-formed id");
    for anti in book.anti_recipes() {
        let route = anti_recipe_route(anti).expect("a route builds");
        assert!(route.check(&graph).is_empty(), "{}", anti.id());
        let bundle = compile_anti_recipe_route(&graph, anti, &TraversalPolicy::exhaustive())
            .expect("it compiles");
        assert!(bundle.contains(&rules), "{}", anti.id());
    }
}

#[test]
fn a_route_naming_a_module_the_graph_does_not_hold_is_refused_before_compiling() {
    let book = book();
    let graph = cookbook_doc_graph(&book).expect("the cookbook graph builds");
    let stray = Recipe::draft(id("routes-nowhere"), "read a document that does not exist")
        .step(a_step())
        .demonstrating(Claim::new("nothing"))
        .checked_by(a_property())
        .easy_to_get_wrong(a_pitfall())
        .also_reading("docs/THIS_DOES_NOT_EXIST.md")
        .seal()
        .expect("the recipe itself is well formed");
    let outcome = compile_recipe_route(&graph, &stray, &TraversalPolicy::exhaustive());
    assert!(matches!(outcome, Err(RouteError::UnroutableModule { .. })));
}

#[test]
fn the_cookbook_graph_lints_without_errors() {
    let book = book();
    let graph = cookbook_doc_graph(&book).expect("the cookbook graph builds");
    let routes: Vec<_> = book
        .recipes()
        .iter()
        .map(|recipe| recipe_route(recipe).expect("a route builds"))
        .chain(
            book.anti_recipes()
                .iter()
                .map(|anti| anti_recipe_route(anti).expect("a route builds")),
        )
        .collect();
    let report = lint(&graph, &routes);
    let errors: Vec<String> = report
        .errors()
        .map(|finding| format!("{finding:?}"))
        .collect();
    assert!(errors.is_empty(), "{}", errors.join("\n"));
}

#[test]
fn the_mandatory_set_of_every_recipe_route_has_room_to_grow_inside_the_budget() {
    let book = book();
    let graph = cookbook_doc_graph(&book).expect("the cookbook graph builds");
    let ceiling = ROUTE_BUDGET / 4;
    for recipe in book.recipes() {
        let bundle = compile_recipe_route(&graph, recipe, &TraversalPolicy::exhaustive())
            .expect("it compiles");
        let mandatory: u32 = bundle
            .entries
            .iter()
            .filter(|entry| entry.is_mandatory())
            .map(|entry| entry.cost.tokens)
            .sum();
        assert!(
            mandatory <= ceiling,
            "{} has a mandatory set of {mandatory} estimated tokens against a {ceiling} ceiling; \
             the budget is what turns growth into a refusal rather than a truncation",
            recipe.id()
        );
    }
}

#[test]
fn the_one_document_no_recipe_reaches_is_a_proven_zero_and_not_an_unknown() {
    let book = book();
    let graph = cookbook_doc_graph(&book).expect("the cookbook graph builds");
    let mut route = recipe_route(&book.recipes()[0]).expect("a route builds");
    route.budget = None;
    let readme = ModuleId::parse("README.md").expect("a well-formed id");

    let exhaustive =
        bioprism_docgraph::compile_bundle(&graph, &route, &TraversalPolicy::exhaustive())
            .expect("it compiles");
    let omission = exhaustive
        .omission_for(&readme)
        .expect("README.md is not reached by any recipe route");
    assert_eq!(omission.influence, InfluenceClass::Zero);

    let capped = bioprism_docgraph::compile_bundle(
        &graph,
        &route,
        &TraversalPolicy::exhaustive().with_max_depth(1),
    )
    .expect("it compiles");
    assert_eq!(
        capped
            .omission_for(&readme)
            .expect("still omitted")
            .influence,
        InfluenceClass::Unknown,
        "the same unreached module must not be a proven zero under a narrowed walk"
    );
}

#[test]
fn the_two_sufficiency_summaries_of_one_compile_never_disagree() {
    let book = book();
    let graph = cookbook_doc_graph(&book).expect("the cookbook graph builds");
    for recipe in book.recipes() {
        for policy in [
            TraversalPolicy::exhaustive(),
            TraversalPolicy::exhaustive().with_max_depth(1),
            TraversalPolicy::normative(),
        ] {
            let bundle = compile_recipe_route(&graph, recipe, &policy).expect("it compiles");
            assert_eq!(
                bundle.is_sufficient(),
                every_omission_supports_sufficiency(&bundle),
                "{}: the grouped manifest and the per-omission classes disagree",
                recipe.id()
            );
            let counted: usize = omissions_by_influence(&bundle)
                .iter()
                .map(|(_, count)| count)
                .sum();
            assert_eq!(counted, bundle.omissions.len());
        }
    }
}

#[test]
fn a_check_backed_by_a_test_is_continuously_enforced_and_an_observable_is_not() {
    let enforced = Check::EnforcedByTest(WorkspaceTest::new(
        krate("bioprism-docgraph"),
        "crates/docgraph/tests/acceptance.rs",
        "only_an_exhaustive_walk_licenses_a_zero_influence_claim",
    ));
    let observed = Check::Observable {
        observe: "the rendered report".to_string(),
        expect: "a non-empty gap section".to_string(),
    };
    assert!(enforced.is_continuously_enforced());
    assert!(enforced.test().is_some());
    assert!(!observed.is_continuously_enforced());
    assert!(observed.test().is_none());
}

#[test]
fn a_recipe_lists_the_crate_that_owns_its_enforcing_test_among_its_crates() {
    let book = book();
    let recipe = book
        .recipe("run-a-mutation-family")
        .expect("the recipe is registered");
    let crates: Vec<String> = recipe.crates().iter().map(ToString::to_string).collect();
    assert!(crates.contains(&"bioprism-mutation".to_string()));
    for test in recipe.enforcing_tests() {
        assert!(
            crates.contains(&test.krate.to_string()),
            "the crate owning `{}` is not in the recipe's crate list",
            test.name
        );
    }
}

#[test]
fn an_entry_point_reports_the_item_path_beneath_its_crate() {
    let entry = entry("bioprism_docgraph::fixture::repository_doc_graph");
    assert_eq!(entry.item_path(), vec!["fixture", "repository_doc_graph"]);
}

#[test]
fn the_export_reader_finds_items_declared_directly() {
    let exports = exported_items(
        "pub mod inner;\npub fn thing() {}\npub struct Held;\npub const LIMIT: u32 = 1;\n\
         fn private_thing() {}\n",
    );
    for name in ["inner", "thing", "Held", "LIMIT"] {
        assert!(exports.contains(name), "{name} was not read");
    }
    assert!(!exports.contains("private_thing"));
}

#[test]
fn a_recipe_renders_the_same_text_on_every_call() {
    let book = book();
    let recipe = &book.recipes()[0];
    assert_eq!(recipe.render(), recipe.render());
    assert!(recipe.render().contains("easy to get wrong"));
    assert!(recipe.render().contains("claim:"));
}

#[test]
fn a_pinned_quotation_that_was_reworded_is_detected() {
    let quote = bioprism_cookbook::PinnedQuote::new("AGENTS.md", "a sentence nobody wrote");
    assert!(!quote.still_present_in("some other prose entirely"));
    assert!(quote.still_present_in("prose containing a sentence nobody wrote, oddly"));
}

#[test]
fn asking_for_an_unregistered_anti_recipe_is_a_typed_error() {
    assert!(matches!(
        book().anti_recipe("no-such-anti-recipe"),
        Err(CookbookError::UnknownRecipe(_))
    ));
}

#[test]
fn a_recipe_that_lost_its_pitfall_in_transit_fails_to_deserialise() {
    let recipe = book().recipes()[0].clone();
    let mut wire = serde_json::to_value(&recipe).expect("a recipe serialises");
    wire.as_object_mut()
        .expect("the wire form is an object")
        .insert("pitfall".to_string(), serde_json::Value::Null);
    let reloaded: Result<Recipe, _> = serde_json::from_value(wire);
    assert!(reloaded.is_err());
}

// ---------------------------------------------------------------------------
// The report, and the half of it that is missing
// ---------------------------------------------------------------------------

#[test]
fn the_report_recomputes_its_own_digest() {
    let report = CookbookReport::of(&book()).expect("the report builds");
    assert!(report.digest_is_intact());
}

#[test]
fn editing_the_report_breaks_its_digest() {
    let mut report = CookbookReport::of(&book()).expect("the report builds");
    report.tests_leaned_on += 1;
    assert!(!report.digest_is_intact());
}

#[test]
fn two_runs_of_the_report_produce_the_same_digest() {
    let first = CookbookReport::of(&book()).expect("the report builds");
    let second = CookbookReport::of(&book()).expect("the report builds");
    assert_eq!(first.digest, second.digest);
}

#[test]
fn the_report_names_the_recipes_that_could_not_be_written_with_a_concrete_blocker() {
    let report = CookbookReport::of(&book()).expect("the report builds");
    assert!(
        !report.unwritten.is_empty(),
        "a cookbook claiming no gaps is a cookbook nobody audited"
    );
    for entry in &report.unwritten {
        assert!(!entry.blocker.trim().is_empty(), "{}", entry.goal);
        assert!(!entry.blueprint_modules.is_empty(), "{}", entry.goal);
    }
}

#[test]
fn every_gap_this_cookbook_attributes_to_another_crate_pins_that_crates_own_wording() {
    let workspace = workspace();
    let report = CookbookReport::of(&book()).expect("the report builds");
    for entry in &report.unwritten {
        if let Some(quote) = &entry.evidence {
            assert!(
                workspace.resolve_quote(quote).is_present(),
                "the obstacle quoted for `{}` is no longer in {}",
                entry.goal,
                quote.source
            );
        }
    }
}

#[test]
fn the_report_separates_recipes_nothing_continuously_enforces() {
    let book = book();
    let report = CookbookReport::of(&book).expect("the report builds");
    for id in &report.not_continuously_enforced {
        let recipe = book.recipe(id).expect("it is registered");
        assert!(
            !recipe.is_continuously_enforced(),
            "{id} is listed as unenforced and has an enforcing test"
        );
    }
}

#[test]
fn every_shipped_recipe_is_guarded_by_at_least_one_workspace_test() {
    let report = CookbookReport::of(&book()).expect("the report builds");
    assert!(
        report.not_continuously_enforced.is_empty(),
        "these recipes are checked only when somebody looks: {:?}",
        report.not_continuously_enforced
    );
}

#[test]
fn the_rendered_report_leads_with_the_gaps_rather_than_the_contents() {
    let report = CookbookReport::of(&book()).expect("the report builds");
    let rendered = report.render();
    let gaps = rendered
        .find("WANTED BUT NOT WRITTEN")
        .expect("the gap section is rendered");
    let contents = rendered
        .find("\nRECIPES")
        .expect("the recipe section is rendered");
    assert!(
        gaps < contents,
        "a gap list printed last is a gap list nobody reads"
    );
}

/// A field the reader does not know is a field the digest cannot cover.
///
/// `CookbookReport` seals itself by re-serialising the parsed struct and hashing that, so anything
/// a reader silently discards is outside the seal by construction: the recomputation cannot see
/// it, agrees with the claimed digest, and the report verifies with content in it that nobody
/// hashed. The report is the artefact a newcomer trusts to say what the platform can do, so an
/// invisible field in it is the difference between "these recipes exist" and "somebody wrote these
/// recipes into the file after it was sealed".
#[test]
fn a_report_carrying_a_field_the_reader_does_not_know_is_refused_rather_than_silently_dropped() {
    let report = CookbookReport::of(&book()).expect("the report builds");
    assert!(report.digest_is_intact());
    let sealed = serde_json::to_value(&report).expect("the report serialises");

    for pointer in ["", "/recipes/0", "/unwritten/0"] {
        let mut tampered = sealed.clone();
        tampered
            .pointer_mut(pointer)
            .expect("the position exists")
            .as_object_mut()
            .expect("the position is an object")
            .insert("injected".into(), serde_json::json!("added after sealing"));

        let reread = serde_json::from_value::<CookbookReport>(tampered);
        assert!(
            reread.is_err(),
            "a key injected at {pointer:?} was read back into a report that still verifies, so the \
             digest names less than the document does"
        );
    }
}
