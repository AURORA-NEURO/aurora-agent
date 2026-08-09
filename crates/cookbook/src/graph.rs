//! Recipes as task routes over the documentation graph (41.05, 41.09).
//!
//! 41.05 calls the thing an agent needs before starting a job a *task route*: an intent, a
//! must-read set, a set of boundaries that are non-omittable whatever the budget, and an estimated
//! token ceiling. That is what a recipe is. So a recipe does not get a bespoke delivery mechanism —
//! it compiles into a [`TaskRoute`] and goes through `bioprism-docgraph`'s bundle compiler like
//! every other route in the workspace.
//!
//! This is reuse rather than politeness. Everything that makes a documentation context honest is
//! already implemented there and would have to be reimplemented, wrongly, to avoid it: the
//! protected-closure pass that runs before selection, the refusal to truncate a mandatory set to
//! fit a budget, the rule that only an exhaustive walk licenses a zero-influence claim, and the
//! omission record that carries an influence class per group. **None of it is weakened here.** In
//! particular [`compile_recipe_route`] forwards
//! [`BundleError::MandatorySetExceedsBudget`](bioprism_docgraph::BundleError::MandatorySetExceedsBudget)
//! unchanged: a recipe whose route will not fit fails, and no code path in this crate catches that
//! error and retries at a larger budget.
//!
//! # The shape of the graph, and why each edge type was chosen
//!
//! | edge | meaning here | consequence it buys |
//! |---|---|---|
//! | `recipe → crate` `example_of` | the recipe illustrates the crate and is not normative about it | including the recipe obliges the crate's node; impact stops after one hop |
//! | `crate → crate` `requires` | a real Cargo dependency | the dependency closure of a recipe's crates is mandatory too |
//! | `crate → AGENTS.md` `governs` | the house rules constrain the crate | every bundle carries the rules |
//! | `anti-recipe → AGENTS.md` `refines` | the anti-recipe sharpens a rule without replacing it | the rule travels with its instance |
//! | `recipe → docs/*` `references` | further reading | navigational only; creates no obligation |
//!
//! The `requires` edges are checked against the crates' `Cargo.toml` files by
//! [`crate::verify`]-adjacent machinery in `tests/`, so an edge asserting a dependency that was
//! removed fails rather than quietly mis-shaping every bundle that closes over it.
//!
//! # `AGENTS.md` is protected, so it is mandatory whether or not a route asks
//!
//! The node for `AGENTS.md` carries two of blueprint 39.05's protected classes — claim kind and
//! uncertainty/quality — which is what its content is: the rules distinguishing observation from
//! interpretation, and measured from unmeasured. Tagging it makes it mandatory in *every* bundle
//! this module compiles, by the corpus rather than by the route. A route author can forget a
//! boundary; the compiler must not depend on them remembering.
//!
//! # One document that no recipe reaches, on purpose
//!
//! `README.md` has a node and no edge. Every recipe route therefore omits it, and — because the
//! walk that concluded so was exhaustive — omits it as
//! [`InfluenceClass::Zero`]: provably could not have
//! changed the answer, as opposed to nobody looked. That is the distinction the whole workspace is
//! built around, and having one node in the graph that actually exercises it is worth more than a
//! tidier graph in which every node is reachable and the zero is never earned. Cap the walk and the
//! same omission becomes `Unknown`; `tests/` checks both.
//!
//! # Not implemented
//!
//! No scan of the real Markdown. [`bioprism_docgraph::scan`] reads bytes from disk and this module
//! declares nodes instead, with abridged stand-in text, for the reason that crate's own fixture
//! gives: a graph built from live prose changes every time somebody edits a sentence, and the tests
//! over it get deleted rather than fixed. The *structure* — which documents exist, which crates
//! exist, which depends on which — is real and is verified.

use crate::antirecipe::AntiRecipe;
use crate::book::Cookbook;
use crate::error::{CookbookError, RouteError};
use crate::recipe::{CrateName, Recipe, RecipeId};
use bioprism_docgraph::registry::{
    ContextCard, DocGraph, ModuleId, ModuleNode, NodeStatus, ProtectedClass,
};
use bioprism_docgraph::route::{RouteId, TaskRoute};
use bioprism_docgraph::tokens::ProfileLevel;
use bioprism_docgraph::vocabulary::{DocEdge, DocEdgeType};
use bioprism_docgraph::{compile_bundle, ContextBundle, TraversalPolicy};
use bioprism_section::InfluenceClass;

/// The module id of the house rules. Non-omittable from every route this module builds.
pub const HOUSE_RULES: &str = "AGENTS.md";

/// Documentation nodes the cookbook routes through: path, title, status, one-sentence decision.
///
/// A short list on purpose. These are the documents a recipe actually sends a reader to; the rest
/// of the corpus is `bioprism-docgraph`'s business, and duplicating its fixture here would be a
/// second transcription to keep in step with the first.
const DOC_NODES: &[(&str, &str, NodeStatus, &str)] = &[
    (
        HOUSE_RULES,
        "AURORA Agent",
        NodeStatus::Guide,
        "Context assembly is a compiler pass; honest labelling of what was omitted is the product.",
    ),
    (
        "README.md",
        "bioprism",
        NodeStatus::Guide,
        "What the measurements actually say, including the ones that do not favour the thesis.",
    ),
    (
        "docs/ARCHITECTURE.md",
        "Architecture",
        NodeStatus::Implementation,
        "How the crates fit together and why the boundaries fall where they do.",
    ),
    (
        "docs/FINDINGS.md",
        "Findings",
        NodeStatus::Implementation,
        "Measured results from the context comparison, negative ones included.",
    ),
    (
        "docs/COVERAGE.md",
        "Blueprint coverage",
        NodeStatus::Generated,
        "Which blueprint modules the workspace cites, and which have nothing standing in for them.",
    ),
    (
        "docs/BASELINE_COMPARISON.md",
        "Equal-engineering context comparison: radiogenomic-integrity-demo-v1",
        NodeStatus::Generated,
        "The reference world comparison, in which the panel does not separate.",
    ),
    (
        "docs/DISCRIMINATING_COMPARISON.md",
        "Equal-engineering context comparison: generated-discriminating-v1",
        NodeStatus::Generated,
        "The generated world comparison, in which three distinct failure modes appear.",
    ),
];

/// The invariants the house-rules card must restate, one per protected class it carries.
///
/// 41.04 requires that "mandatory invariants are not compressed away", and `bioprism-docgraph`'s
/// linter raises an *error* — not a warning — for a node that declares a 39.05 protected class and
/// whose card does not carry the semantics. That is the right severity and it caught this crate
/// during development: the card was written with a decision line and no invariants, which is
/// exactly the compression the rule exists to forbid.
const HOUSE_RULE_INVARIANTS: &[&str] = &[
    "Claim kind: observation, interpretation, hypothesis, causal claim and recommendation are \
     distinct, and implementation is never implied from specification.",
    "Uncertainty and quality: unmeasured is not zero, an estimate is not a measurement, and zero \
     influence is not unknown influence.",
];

/// Crate nodes: package name, and the one thing that crate decides.
const CRATE_NODES: &[(&str, &str)] = &[
    ("bioprism-ids", "Canonical bytes, content hashing and typed identifiers."),
    ("bioprism-scope", "Typed scopes, their refinement order and their meet lattice."),
    ("bioprism-world", "Facts as local sections, typed factors, causal events and indices."),
    ("bioprism-section", "The Decision Section IR, the Context Certificate and the omission manifest."),
    ("bioprism-fiber", "The compiler: protected closure, backward slice, temporal cut, oracle."),
    ("bioprism-baseline", "Equal-engineering comparators, so the central claim can lose."),
    ("bioprism-worldgen", "Structural families, so world structure is a parameter rather than a given."),
    ("bioprism-bioworlds", "Reference worlds, their structural profiles and the world-shaped backlog."),
    ("bioprism-mutation", "Metamorphic families with executable postconditions and structural dedup."),
    ("bioprism-examples", "Runnable vertical slices, and the register of claims nothing exercises."),
    ("bioprism-prism", "Decision cells, matched forks and minimized, attested result state."),
    ("bioprism-bundle", "Signed, replayable result bundles a third party can check."),
    ("bioprism-scale", "Effective size: equivalence classes rather than instance counts."),
    ("bioprism-benchcompiler", "Benchmark compilation, contamination ledgers and effective diversity."),
    ("bioprism-docgraph", "The documentation graph, its routes and the context bundle compiler."),
    ("bioprism-cookbook", "This catalogue: task-shaped recipes and the mistakes around them."),
];

/// Real Cargo dependencies between the crates above, as `(dependent, dependency)`.
///
/// Only edges whose endpoints are both in [`CRATE_NODES`] are listed, and only real ones — a test
/// reads each crate's `Cargo.toml` and fails if an edge here asserts a dependency that is not
/// declared. Encoding them matters because they are what makes a recipe's mandatory set correct:
/// naming `bioprism-prism` in a step and not delivering `bioprism-section`'s contract would be a
/// bundle that cannot be acted on.
const CRATE_DEPENDENCIES: &[(&str, &str)] = &[
    ("bioprism-fiber", "bioprism-ids"),
    ("bioprism-fiber", "bioprism-scope"),
    ("bioprism-fiber", "bioprism-section"),
    ("bioprism-fiber", "bioprism-world"),
    ("bioprism-baseline", "bioprism-fiber"),
    ("bioprism-baseline", "bioprism-ids"),
    ("bioprism-baseline", "bioprism-section"),
    ("bioprism-baseline", "bioprism-world"),
    ("bioprism-worldgen", "bioprism-world"),
    ("bioprism-worldgen", "bioprism-baseline"),
    ("bioprism-worldgen", "bioprism-section"),
    ("bioprism-worldgen", "bioprism-fiber"),
    ("bioprism-mutation", "bioprism-fiber"),
    ("bioprism-mutation", "bioprism-ids"),
    ("bioprism-mutation", "bioprism-section"),
    ("bioprism-mutation", "bioprism-world"),
    ("bioprism-mutation", "bioprism-worldgen"),
    ("bioprism-examples", "bioprism-ids"),
    ("bioprism-examples", "bioprism-world"),
    ("bioprism-examples", "bioprism-worldgen"),
    ("bioprism-examples", "bioprism-fiber"),
    ("bioprism-examples", "bioprism-section"),
    ("bioprism-bioworlds", "bioprism-ids"),
    ("bioprism-bioworlds", "bioprism-scope"),
    ("bioprism-bioworlds", "bioprism-world"),
    ("bioprism-bioworlds", "bioprism-worldgen"),
    ("bioprism-prism", "bioprism-baseline"),
    ("bioprism-prism", "bioprism-fiber"),
    ("bioprism-prism", "bioprism-ids"),
    ("bioprism-prism", "bioprism-section"),
    ("bioprism-prism", "bioprism-world"),
    ("bioprism-prism", "bioprism-worldgen"),
    ("bioprism-scale", "bioprism-ids"),
    ("bioprism-scale", "bioprism-worldgen"),
    ("bioprism-benchcompiler", "bioprism-ids"),
    ("bioprism-benchcompiler", "bioprism-prism"),
    ("bioprism-docgraph", "bioprism-ids"),
    ("bioprism-docgraph", "bioprism-section"),
    ("bioprism-cookbook", "bioprism-ids"),
    ("bioprism-cookbook", "bioprism-section"),
    ("bioprism-cookbook", "bioprism-docgraph"),
];

fn module(value: &str) -> Result<ModuleId, CookbookError> {
    ModuleId::parse(value).map_err(CookbookError::Graph)
}

/// The module id a recipe occupies in the documentation graph.
pub fn recipe_module_id(id: &RecipeId) -> Result<ModuleId, CookbookError> {
    module(&format!("recipe/{id}"))
}

/// The module id an anti-recipe occupies.
pub fn anti_recipe_module_id(id: &RecipeId) -> Result<ModuleId, CookbookError> {
    module(&format!("anti-recipe/{id}"))
}

/// The module id a crate occupies.
pub fn crate_module_id(name: &CrateName) -> Result<ModuleId, CookbookError> {
    module(&format!("crate/{name}"))
}

/// The documentation graph a cookbook route runs over.
///
/// Deterministic: nodes are keyed in a `BTreeMap` and edges are kept sorted by
/// `bioprism-docgraph`, so two calls produce byte-identical graphs and therefore byte-identical
/// bundles. 41.16 requires exactly that of a clean extraction.
pub fn cookbook_doc_graph(cookbook: &Cookbook) -> Result<DocGraph, CookbookError> {
    let mut graph = DocGraph::new();
    let rules = module(HOUSE_RULES)?;

    for (path, title, status, decision) in DOC_NODES {
        let protected_invariants = if *path == HOUSE_RULES {
            HOUSE_RULE_INVARIANTS.iter().map(|r| (*r).to_string()).collect()
        } else {
            Vec::new()
        };
        let mut node = ModuleNode::new(module(path)?, *path, *title, *status)
            .with_cluster("repository")
            .with_profile(ProfileLevel::Brief)
            .with_card(ContextCard {
                decision: (*decision).to_string(),
                protected_invariants,
                ..ContextCard::default()
            })
            .with_brief(format!("{title}. {decision}"));
        if *path == HOUSE_RULES {
            node = node
                .with_protected(ProtectedClass::ClaimKind)
                .with_protected(ProtectedClass::UncertaintyAndQuality);
        }
        graph.insert_node(node)?;
    }

    for (name, decision) in CRATE_NODES {
        let id = module(&format!("crate/{name}"))?;
        graph.insert_node(
            ModuleNode::new(
                id.clone(),
                format!("crates/{}/src/lib.rs", name.trim_start_matches("bioprism-")),
                *name,
                NodeStatus::Implementation,
            )
            .with_cluster("crate")
            .with_profile(ProfileLevel::Brief)
            .with_card(ContextCard {
                decision: (*decision).to_string(),
                ..ContextCard::default()
            })
            .with_brief(format!("{name}. {decision}")),
        )?;
        graph.insert_edge(DocEdge::new(id, rules.clone(), DocEdgeType::Governs));
    }

    for (from, to) in CRATE_DEPENDENCIES {
        graph.insert_edge(DocEdge::new(
            module(&format!("crate/{from}"))?,
            module(&format!("crate/{to}"))?,
            DocEdgeType::Requires,
        ));
    }

    for recipe in cookbook.recipes() {
        let id = recipe_module_id(recipe.id())?;
        graph.insert_node(
            ModuleNode::new(
                id.clone(),
                "crates/cookbook/src/catalog.rs",
                recipe.goal(),
                NodeStatus::Guide,
            )
            .with_cluster("recipe")
            .with_profile(ProfileLevel::Contract)
            .with_card(ContextCard {
                decision: recipe.claim().statement.clone(),
                ..ContextCard::default()
            })
            .with_hashed_body(&recipe.render()),
        )?;
        graph.insert_edge(DocEdge::new(
            id.clone(),
            rules.clone(),
            DocEdgeType::Governs,
        ));
        for krate in recipe.crates() {
            let target = crate_module_id(&krate)?;
            if graph.contains(&target) {
                graph.insert_edge(DocEdge::new(
                    id.clone(),
                    target,
                    DocEdgeType::ExampleOf,
                ));
            }
        }
        for reading in recipe.reading() {
            let target = module(reading)?;
            if graph.contains(&target) {
                graph.insert_edge(DocEdge::new(
                    id.clone(),
                    target,
                    DocEdgeType::References,
                ));
            }
        }
    }

    for anti in cookbook.anti_recipes() {
        let id = anti_recipe_module_id(anti.id())?;
        graph.insert_node(
            ModuleNode::new(
                id.clone(),
                "crates/cookbook/src/catalog.rs",
                anti.attempt(),
                NodeStatus::Guide,
            )
            .with_cluster("anti-recipe")
            .with_profile(ProfileLevel::Contract)
            .with_card(ContextCard {
                decision: anti.why_it_fails().to_string(),
                ..ContextCard::default()
            })
            .with_hashed_body(&anti.render()),
        )?;
        graph.insert_edge(DocEdge::new(id.clone(), rules.clone(), DocEdgeType::Refines));
        for test in anti.enforced_by() {
            let target = crate_module_id(&test.krate)?;
            if graph.contains(&target) {
                graph.insert_edge(DocEdge::new(
                    id.clone(),
                    target,
                    DocEdgeType::References,
                ));
            }
        }
    }

    Ok(graph)
}

/// The 41.05 route a recipe compiles to.
///
/// `must_read` holds the recipe alone. The crates come in through the `example_of` companion
/// closure rather than being listed, which is the difference between a route that stays correct
/// when a step is added and one that has to be edited in two places. `non_omittable` holds the
/// house rules, which are also protected by the corpus — belt and braces, and the belt is the one
/// that survives someone rewriting this function.
pub fn recipe_route(recipe: &Recipe) -> Result<TaskRoute, CookbookError> {
    let id = RouteId::parse(recipe.id().as_str()).map_err(CookbookError::Graph)?;
    let mut route = TaskRoute::new(id, recipe.goal())
        .must_read(recipe_module_id(recipe.id())?)
        .non_omittable(module(HOUSE_RULES)?);
    for reading in recipe.reading() {
        route = route.optional(module(reading)?);
    }
    for property in recipe.properties() {
        route = route.expecting(property.statement.clone());
    }
    if let Some(budget) = recipe.budget() {
        route = route.with_budget(budget);
    }
    Ok(route)
}

/// The route an anti-recipe compiles to.
///
/// Unbudgeted. An anti-recipe is read when something has already gone wrong, and the reader wants
/// the rule it sharpens and every test that catches the mistake — which is 41.05's stated case for
/// an unbounded route, an audit that reads everything.
pub fn anti_recipe_route(anti: &AntiRecipe) -> Result<TaskRoute, CookbookError> {
    let id = RouteId::parse(anti.id().as_str()).map_err(CookbookError::Graph)?;
    Ok(
        TaskRoute::new(id, format!("understand why this does not work: {}", anti.attempt()))
            .must_read(anti_recipe_module_id(anti.id())?)
            .non_omittable(module(HOUSE_RULES)?)
            .expecting(anti.instead().to_string()),
    )
}

/// Compile a recipe's route into a context bundle.
///
/// Every failure `bioprism-docgraph` can raise is forwarded. Nothing here catches
/// `MandatorySetExceedsBudget` and retries: a recipe whose documentation will not fit its declared
/// budget is a defect in the recipe, and the caller has to see it.
pub fn compile_recipe_route(
    graph: &DocGraph,
    recipe: &Recipe,
    policy: &TraversalPolicy,
) -> Result<ContextBundle, RouteError> {
    let route = recipe_route(recipe)?;
    for declared in route.declared_modules() {
        if !graph.contains(&declared) {
            return Err(RouteError::UnroutableModule {
                recipe: recipe.id().to_string(),
                module: declared.to_string(),
            });
        }
    }
    Ok(compile_bundle(graph, &route, policy)?)
}

/// Compile an anti-recipe's route into a context bundle.
pub fn compile_anti_recipe_route(
    graph: &DocGraph,
    anti: &AntiRecipe,
    policy: &TraversalPolicy,
) -> Result<ContextBundle, RouteError> {
    let route = anti_recipe_route(anti)?;
    Ok(compile_bundle(graph, &route, policy)?)
}

/// A recipe bundle's omissions, counted by influence class.
///
/// The one summary worth putting in front of a reader before the bundle itself. `Zero` means the
/// modules provably could not have changed the answer; everything else is a caveat, and `Unknown`
/// is the one that voids the sufficiency claim outright. Returned as counts against
/// [`InfluenceClass::as_str`] rather than as a map keyed by the enum, because `bioprism-section`
/// deliberately does not give [`InfluenceClass`] an ordering and inventing one here would put a
/// comparison on that type its owner did not sanction — the same reasoning `bioprism-docgraph`
/// gives for grouping its manifest by reason alone.
pub fn omissions_by_influence(bundle: &ContextBundle) -> Vec<(&'static str, usize)> {
    let mut counts: Vec<(&'static str, usize)> = Vec::new();
    for class in [
        InfluenceClass::Zero,
        InfluenceClass::Bounded,
        InfluenceClass::InaccessibleByPolicy,
        InfluenceClass::DeferredAcquisition,
        InfluenceClass::Unknown,
    ] {
        let count = bundle
            .omissions
            .iter()
            .filter(|omission| omission.influence == class)
            .count();
        if count > 0 {
            counts.push((class.as_str(), count));
        }
    }
    counts
}

/// Whether every omission in a bundle is one that can support a sufficiency claim.
///
/// A convenience over [`InfluenceClass::supports_sufficiency`], kept as a separate function from
/// [`ContextBundle::is_sufficient`] because the two can in principle disagree: this one reads the
/// per-omission classes, that one reads the grouped manifest. A test compares them, so a
/// divergence is a failure rather than a silent difference of opinion between two summaries of the
/// same compile.
pub fn every_omission_supports_sufficiency(bundle: &ContextBundle) -> bool {
    bundle
        .omissions
        .iter()
        .all(|omission| omission.influence.supports_sufficiency())
}

/// The crate dependency edges the graph asserts, for the test that checks them against Cargo.
pub fn asserted_crate_dependencies() -> Vec<(CrateName, CrateName)> {
    CRATE_DEPENDENCIES
        .iter()
        .filter_map(|(from, to)| {
            Some((CrateName::parse(*from).ok()?, CrateName::parse(*to).ok()?))
        })
        .collect()
}

/// The crates this graph declares a node for.
pub fn declared_crate_nodes() -> Vec<CrateName> {
    CRATE_NODES
        .iter()
        .filter_map(|(name, _)| CrateName::parse(*name).ok())
        .collect()
}

/// The repository documents this graph declares a node for.
pub fn declared_doc_nodes() -> Vec<&'static str> {
    DOC_NODES.iter().map(|(path, _, _, _)| *path).collect()
}
