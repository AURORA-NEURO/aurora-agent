//! The shipped recipes and anti-recipes.
//!
//! Blueprint 19's twenty-two modules are worked examples of a *product* — a decision cell, an
//! oracle, a result bundle, a CLI session. This catalogue is worked examples of a *workspace*: the
//! six or seven things somebody actually arrives wanting to do, each mapped onto the crates that
//! do them, in order, with the claim it demonstrates and the test that already checks it.
//!
//! # How §19 was read, and the measurement behind that reading
//!
//! Section 41 could be discharged with one implementation because its sixteen modules are 72.6%
//! shared scaffolding — `bioprism-docgraph` measured that and said so. Section 19 is the opposite
//! and the same measurement says it plainly. Over the 22 modules of `19_REFERENCE_EXAMPLES`,
//! counting non-blank line occurrences (1264 of them across the section):
//!
//! | lines appearing in ≥ N of 22 modules | occurrences | share |
//! |---|---:|---:|
//! | 22 / 22 | 66 | 5.22% |
//! | 21 / 22 (§41's relative threshold, 15/16) | 66 | 5.22% |
//! | 15 / 22 (§41's absolute threshold) | 107 | 8.47% |
//! | 11 / 22 (half) | 124 | 9.81% |
//!
//! The number is stable under normalisation: trimming whitespace and lowercasing moves nothing at
//! the thresholds above (it changes only the ≥2 and ≥3 tails, 13.84% → 14.48%). It is *not* stable
//! under changing what counts as boilerplate, which is the sensitivity that matters: counting
//! **structural** scaffolding instead of repeated lines — YAML front-matter delimiters, `title:`,
//! `last_updated:`, every ATX heading and every code fence — gives **21.44%** (271 of 1264). So the
//! honest statement is a range with its method attached: **5.2% by exact repetition across all
//! modules, 8.5% at §41's threshold, 21.4% if markdown furniture counts.**
//!
//! The 66 fully-shared occurrences are exactly the front matter: `---` twice per module plus one
//! identical `last_updated: "2026-08-07"`. Mean pairwise line-set Jaccard between modules is 0.036;
//! mean lines unique to a single module is 48.3 of a mean 57.5.
//!
//! Two consequences, and they shaped this crate. First, §19 cannot be discharged by one generic
//! implementation the way §41 was — the modules genuinely differ, so a cookbook has to be authored
//! rather than generated. Second, §19 supplies almost no shared contract, so nearly every decision
//! here is this crate's own and is defended in these docs rather than attributed to the blueprint.
//!
//! # What is deliberately not here
//!
//! No recipe *runs*. See [`crate::recipe`]. And no recipe for the six §19 modules whose subject
//! this workspace has no implementation of — federated registry flow (19.20), the GitHub action
//! (19.08), the capability profile (19.07), the pack directory (19.10), the private-to-public
//! derivative (19.18) and the agent architecture IR (19.02). Those are absences, listed in
//! [`crate::report::CookbookReport::unwritten`], not silences.

use crate::antirecipe::AntiRecipe;
use crate::error::CookbookError;
use crate::quotes::agents;
use crate::recipe::{
    Check, CheckableProperty, Claim, CrateName, EntryPoint, Pitfall, Recipe, RecipeId, Step,
    WorkspaceTest,
};

/// The estimated-token ceiling every shipped recipe route declares.
///
/// One number for the whole catalogue rather than a per-recipe guess. A per-recipe budget would be
/// a set of magic constants nobody could justify, and the moment one of them was tuned to make a
/// route fit it would stop being a budget and start being a record of what the route happened to
/// cost. This one is checked in `tests/` against the largest mandatory set the catalogue produces,
/// so if a recipe grows past it the failure is a test, not a truncated bundle.
pub const ROUTE_BUDGET: u32 = 4096;

fn id(value: &str) -> RecipeId {
    RecipeId::parse(value).expect("catalogue recipe ids are well formed")
}

fn krate(value: &str) -> CrateName {
    CrateName::parse(value).expect("catalogue crate names are well formed")
}

fn entry(value: &str) -> EntryPoint {
    EntryPoint::parse(value).expect("catalogue entry points are well formed")
}

fn test(crate_name: &str, path: &str, name: &str) -> WorkspaceTest {
    WorkspaceTest::new(krate(crate_name), path, name)
}

fn enforced(statement: &str, crate_name: &str, path: &str, name: &str) -> CheckableProperty {
    CheckableProperty::new(statement, Check::EnforcedByTest(test(crate_name, path, name)))
}

fn observable(statement: &str, observe: &str, expect: &str) -> CheckableProperty {
    CheckableProperty::new(
        statement,
        Check::Observable {
            observe: observe.to_string(),
            expect: expect.to_string(),
        },
    )
}

/// Every recipe this crate ships, in the order a newcomer should meet them.
pub fn recipes() -> Result<Vec<Recipe>, CookbookError> {
    Ok(vec![
        compile_a_context()?,
        check_a_split_for_leakage()?,
        compare_context_strategies()?,
        generate_and_characterise_a_world()?,
        run_a_mutation_family()?,
        audit_what_cannot_be_demonstrated()?,
        route_documentation_for_a_task()?,
        size_a_benchmark_by_equivalence_classes()?,
        fork_two_architectures()?,
        hand_a_result_to_a_third_party()?,
        verify_this_cookbook()?,
    ])
}

/// The first recipe anybody needs: compile a decision context and read its receipt.
fn compile_a_context() -> Result<Recipe, CookbookError> {
    Recipe::draft(
        id("compile-a-context-and-read-its-certificate"),
        "I have a world of facts and a decision to make, and I want the smallest evidence region \
         that is sufficient for it — plus a receipt saying what was left out.",
    )
    .step(
        Step::new(krate("bioprism-world"), "Load the world and validate it before compiling; a diagnostic here is cheaper than an omission later.")
            .calling(entry("bioprism_world::World"))
            .calling(entry("bioprism_world::validate")),
    )
    .step(
        Step::new(krate("bioprism-fiber"), "State the decision as a typed query, including the decision time and the protected tags.")
            .calling(entry("bioprism_fiber::Query")),
    )
    .step(
        Step::new(krate("bioprism-fiber"), "Compile. Protected closure runs first, then the backward slice, then the temporal cut, then the oracle.")
            .calling(entry("bioprism_fiber::compile"))
            .calling(entry("bioprism_fiber::CompileOutput")),
    )
    .step(
        Step::new(krate("bioprism-section"), "Read the Decision Section's layers, then read the certificate — the omission manifest before the verdict.")
            .calling(entry("bioprism_section::DecisionSection"))
            .calling(entry("bioprism_section::ContextCertificate"))
            .calling(entry("bioprism_section::OmissionManifest")),
    )
    .step(
        Step::new(krate("bioprism-ids"), "Recompute the certificate's digest over canonical bytes and compare. A consumer must be able to do this without linking the compiler.")
            .calling(entry("bioprism_ids::ContentHash"))
            .calling(entry("bioprism_ids::to_canonical_bytes")),
    )
    .demonstrating(
        Claim::new(
            "a compiled context arrives with a receipt that states what was omitted and with what \
             influence class, and the receipt verifies itself without the compiler that produced it",
        )
        .citing("43.13")
        .citing("43.25")
        .citing("43.26")
        .citing("40.05"),
    )
    .checked_by(enforced(
        "the certificate recomputes its own digest, so a consumer can verify it independently",
        "bioprism-examples",
        "crates/examples/tests/vertical_slices.rs",
        "every_compiled_slice_emits_a_certificate_that_verifies_its_own_digest",
    ))
    .checked_by(enforced(
        "the compiler names every pass it could not run rather than omitting it from the receipt",
        "bioprism-examples",
        "crates/examples/tests/vertical_slices.rs",
        "every_compile_declares_the_passes_it_could_not_run",
    ))
    .checked_by(enforced(
        "no omission group is left without an influence class",
        "bioprism-examples",
        "crates/examples/tests/vertical_slices.rs",
        "no_omission_group_in_the_suite_is_left_unclassified",
    ))
    .easy_to_get_wrong(Pitfall::new(
        "reading the oracle verdict and stopping there",
        "the verdict is one bit and the manifest is the basis it was reached from. A manifest \
         carrying any Unknown group does not support a sufficiency claim no matter what the verdict \
         says, and `OmissionManifest::supports_sufficiency_claim` is the function that says so.",
    ))
    .also_reading("docs/ARCHITECTURE.md")
    .with_budget(ROUTE_BUDGET)
    .seal()
}

fn check_a_split_for_leakage() -> Result<Recipe, CookbookError> {
    Recipe::draft(
        id("check-a-split-for-leakage"),
        "I have a cohort split into train and test and I need to know whether it leaks — and to be \
         handed something I can check by hand rather than a similarity score.",
    )
    .step(
        Step::new(krate("bioprism-world"), "Encode the cohort as facts scoped to subjects, with the split assignment and the preprocessing fit scope as first-class variables.")
            .calling(entry("bioprism_world::Fact"))
            .calling(entry("bioprism_world::Factor")),
    )
    .step(
        Step::new(krate("bioprism-fiber"), "Compile the integrity query. Protect the cohort-structure tags so closure runs before any relevance step.")
            .calling(entry("bioprism_fiber::compile")),
    )
    .step(
        Step::new(krate("bioprism-section"), "Read the verdict's witnesses. Each names the subjects a reader would have to refute.")
            .calling(entry("bioprism_section::OracleVerdict"))
            .calling(entry("bioprism_section::LeakageWitness"))
            .calling(entry("bioprism_section::OracleStatus")),
    )
    .step(
        Step::new(krate("bioprism-worldgen"), "Before trusting a clean verdict, inject one mechanism at a time into a clean world and confirm the oracle names exactly that mechanism.")
            .calling(entry("bioprism_worldgen::WorldSpec"))
            .calling(entry("bioprism_worldgen::LeakageMechanism"))
            .calling(entry("bioprism_worldgen::generate")),
    )
    .demonstrating(
        Claim::new(
            "the split-integrity oracle returns concrete witnesses a human can check by hand, and it \
             is not a constant `invalid` — a world with no injected defect returns valid with no \
             witnesses",
        )
        .citing("43.41")
        .citing("38.01"),
    )
    .checked_by(enforced(
        "the witnesses name the subjects a reader would have to refute, rather than scoring similarity",
        "bioprism-examples",
        "crates/examples/tests/vertical_slices.rs",
        "the_radiogenomic_witnesses_name_the_subjects_a_reader_would_have_to_refute",
    ))
    .checked_by(enforced(
        "a world with no injected defect returns valid, so the oracle is not a constant invalid",
        "bioprism-examples",
        "crates/examples/tests/vertical_slices.rs",
        "a_world_without_injected_defects_returns_valid_so_the_oracle_is_not_a_constant_invalid",
    ))
    .checked_by(enforced(
        "each leakage mechanism can be injected alone and produces exactly its own witness kind",
        "bioprism-examples",
        "crates/examples/tests/vertical_slices.rs",
        "each_leakage_mechanism_produces_exactly_its_own_witness_kind",
    ))
    .easy_to_get_wrong(Pitfall::new(
        "treating an empty witness list as proof that the split is clean",
        "the oracle detects the mechanisms the world encodes. An empty list means none of *those* \
         fired; it says nothing about a mechanism nobody modelled. `bioprism-examples` records that \
         `WorldSpec::LeakageMechanism` has four members where 38.01 names six families, so two of \
         the specified families cannot be injected and therefore cannot be detected here at all.",
    ))
    .also_reading("docs/FINDINGS.md")
    .with_budget(ROUTE_BUDGET)
    .seal()
}

fn compare_context_strategies() -> Result<Recipe, CookbookError> {
    Recipe::draft(
        id("compare-context-strategies-on-equal-engineering"),
        "I want to know whether compiling a context actually beats retrieving one, on a comparison \
         that would have embarrassed me if it had gone the other way.",
    )
    .step(
        Step::new(krate("bioprism-baseline"), "Build the panel. Every strategy gets the same world, the same query and the same budget; none gets privileged access.")
            .calling(entry("bioprism_baseline::default_panel"))
            .calling(entry("bioprism_baseline::ContextStrategy")),
    )
    .step(
        Step::new(krate("bioprism-baseline"), "Tune each baseline to its best setting before comparing. A baseline measured only where it degenerates is a strawman.")
            .calling(entry("bioprism_baseline::KHopIncidence"))
            .calling(entry("bioprism_baseline::LexicalTopK")),
    )
    .step(
        Step::new(krate("bioprism-baseline"), "Run the comparison and read the per-strategy result: selection size, verdict, and protected-closure fraction.")
            .calling(entry("bioprism_baseline::compare"))
            .calling(entry("bioprism_baseline::Comparison"))
            .calling(entry("bioprism_baseline::StrategyResult")),
    )
    .step(
        Step::new(krate("bioprism-worldgen"), "If the panel does not separate, change the structure rather than the tuning, and say which structure you changed.")
            .calling(entry("bioprism_worldgen::DistractorAttachment"))
            .calling(entry("bioprism_worldgen::TagStyle")),
    )
    .demonstrating(
        Claim::new(
            "on the world the distribution ships, a correctly tuned neighbourhood walk and a lexical \
             retriever select exactly the same eleven facts as the compiler — so that benchmark \
             measures nothing about the method, and the workspace publishes that",
        )
        .citing("43.38")
        .citing("43.39")
        .citing("43.41"),
    )
    .checked_by(enforced(
        "a correctly tuned graph walk matches the compiled selection exactly on the reference world",
        "bioprism-baseline",
        "crates/baseline/tests/equal_engineering.rs",
        "a_correctly_tuned_graph_walk_matches_fiber_exactly",
    ))
    .checked_by(enforced(
        "no strategy in the panel receives privileged access to the world or the query",
        "bioprism-baseline",
        "crates/baseline/tests/equal_engineering.rs",
        "no_strategy_receives_privileged_access",
    ))
    .checked_by(enforced(
        "on a world built to vary structure, no walk depth is simultaneously sound, closed and compact",
        "bioprism-examples",
        "crates/examples/tests/vertical_slices.rs",
        "no_graph_walk_depth_is_sound_closed_and_compact_on_the_discriminating_world",
    ))
    .easy_to_get_wrong(Pitfall::new(
        "ranking the panel by verdict, or by selection size",
        "on the discriminating world the lexical retriever reaches the right verdict from a 91% \
         protected closure — right by luck, having dropped a protected fact that happened not to \
         matter. Ranking on verdict crowns it; ranking on size crowns it twice. Rank on \
         admissibility: right verdict *and* full closure.",
    ))
    .also_reading("docs/FINDINGS.md")
    .also_reading("docs/BASELINE_COMPARISON.md")
    .with_budget(ROUTE_BUDGET)
    .seal()
}

fn generate_and_characterise_a_world() -> Result<Recipe, CookbookError> {
    Recipe::draft(
        id("generate-a-world-and-characterise-its-structure"),
        "I need a world whose structure I chose rather than inherited, and I need to be able to say \
         what makes it hard before I report anything measured on it.",
    )
    .step(
        Step::new(krate("bioprism-worldgen"), "Choose the structural knobs explicitly: where distractors attach, how deep the relay chain is, whether distractor tags are camouflaged.")
            .calling(entry("bioprism_worldgen::WorldSpec"))
            .calling(entry("bioprism_worldgen::DistractorAttachment"))
            .calling(entry("bioprism_worldgen::TagStyle")),
    )
    .step(
        Step::new(krate("bioprism-worldgen"), "Generate. The seed is part of the spec, so the world is reproducible from the spec alone.")
            .calling(entry("bioprism_worldgen::generate"))
            .calling(entry("bioprism_worldgen::Generated")),
    )
    .step(
        Step::new(krate("bioprism-world"), "Validate the generated document before measuring anything on it.")
            .calling(entry("bioprism_world::validate"))
            .calling(entry("bioprism_world::ValidationReport")),
    )
    .step(
        Step::new(krate("bioprism-bioworlds"), "Characterise it: separating depth, camouflage fraction, elimination width, and the directed closure versus the undirected neighbourhood.")
            .calling(entry("bioprism_bioworlds::StructuralProfile"))
            .calling(entry("bioprism_bioworlds::profile"))
            .calling(entry("bioprism_bioworlds::DependencyClosure")),
    )
    .demonstrating(
        Claim::new(
            "structure is a parameter, not a given: a world can be built with no separating depth at \
             all, and the profile says which knob did it",
        )
        .citing("43.39")
        .citing("38.01"),
    )
    .checked_by(enforced(
        "near-target attachment removes the separating depth that hub attachment leaves behind",
        "bioprism-bioworlds",
        "crates/bioworlds/tests/structure_invariants.rs",
        "near_target_attachment_removes_the_separating_depth_that_hub_attachment_leaves",
    ))
    .checked_by(enforced(
        "the generated discriminating world has no separating depth, while the reference world's is five",
        "bioprism-bioworlds",
        "crates/bioworlds/tests/reference_world_calibration.rs",
        "the_generated_discriminating_world_has_no_separating_depth",
    ))
    .checked_by(enforced(
        "regenerating from the same spec reproduces the same world, and a new seed leaves the decisive skeleton alone",
        "bioprism-bioworlds",
        "crates/bioworlds/tests/structure_invariants.rs",
        "a_different_seed_changes_the_distractors_and_leaves_the_decisive_skeleton_alone",
    ))
    .easy_to_get_wrong(Pitfall::new(
        "reporting a comparison run on a world you generated as though it were a general result",
        "the discriminating world was built to expose the failure modes it exposes, exactly as the \
         reference world was built to expose hub expansion. Each is one point. A result on a world \
         whose structure you chose is a statement about that structure until a sweep says otherwise, \
         and the sweep is not done.",
    ))
    .also_reading("docs/FINDINGS.md")
    .also_reading("docs/DISCRIMINATING_COMPARISON.md")
    .with_budget(ROUTE_BUDGET)
    .seal()
}

fn run_a_mutation_family() -> Result<Recipe, CookbookError> {
    Recipe::draft(
        id("run-a-mutation-family"),
        "I have one world and I want a family of related decisions to test against, without \
         fooling myself about how many independent tests I now have.",
    )
    .step(
        Step::new(krate("bioprism-mutation"), "Pick the relations. Each carries an executable postcondition saying what the mutation must and must not change.")
            .calling(entry("bioprism_mutation::Relation"))
            .calling(entry("bioprism_mutation::Mechanism"))
            .calling(entry("bioprism_mutation::standard_suite")),
    )
    .step(
        Step::new(krate("bioprism-mutation"), "Apply them and let the postconditions run. A mutation whose postcondition fails is rejected, not recorded.")
            .calling(entry("bioprism_mutation::apply"))
            .calling(entry("bioprism_mutation::PostconditionResult"))
            .calling(entry("bioprism_mutation::Rejection")),
    )
    .step(
        Step::new(krate("bioprism-mutation"), "Generate the family and read its lineage: which parent each instance came from, and which instances collapsed onto each other.")
            .calling(entry("bioprism_mutation::generate"))
            .calling(entry("bioprism_mutation::Family"))
            .calling(entry("bioprism_mutation::Instance")),
    )
    .step(
        Step::new(krate("bioprism-mutation"), "Measure diversity before quoting a size. The number to quote is the class count, not the instance count.")
            .calling(entry("bioprism_mutation::measure"))
            .calling(entry("bioprism_mutation::Diversity")),
    )
    .demonstrating(
        Claim::new(
            "a metamorphic family is generated with executable postconditions, deduplicated \
             structurally, and reported by independent equivalence classes rather than by instance \
             count",
        )
        .citing("03.08")
        .citing("32")
        .citing("19.05"),
    )
    .checked_by(enforced(
        "the standard suite generates a family in which every postcondition was checked",
        "bioprism-mutation",
        "crates/mutation/tests/metamorphic.rs",
        "the_standard_suite_generates_a_validated_family",
    ))
    .checked_by(enforced(
        "a violated postcondition is caught rather than recorded as an instance",
        "bioprism-mutation",
        "crates/mutation/tests/metamorphic.rs",
        "a_violated_postcondition_is_caught",
    ))
    .checked_by(enforced(
        "deduplication is not defeated by relabelling the subjects",
        "bioprism-mutation",
        "crates/mutation/tests/metamorphic.rs",
        "deduplication_is_not_defeated_by_relabelling",
    ))
    .easy_to_get_wrong(Pitfall::new(
        "quoting the family size as the benchmark size",
        "invariance mutations preserve the verdict by construction, so a thousand of them are a \
         robustness check on one item. `bioprism-mutation` and `bioprism-scale` both refuse to \
         serialise a nominal count without the effective count beside it, for this reason.",
    ))
    .with_budget(ROUTE_BUDGET)
    .seal()
}

fn audit_what_cannot_be_demonstrated() -> Result<Recipe, CookbookError> {
    Recipe::draft(
        id("audit-what-the-platform-cannot-demonstrate"),
        "Before I build on this, I want the list of claims the architecture makes that nothing here \
         actually exercises — and the concrete reason for each.",
    )
    .step(
        Step::new(krate("bioprism-examples"), "Read the property catalogue. It is deliberately larger than what any slice can run.")
            .calling(entry("bioprism_examples::Property"))
            .calling(entry("bioprism_examples::PropertyClaim")),
    )
    .step(
        Step::new(krate("bioprism-examples"), "Take the coverage split without running anything, and read the unexercised half first.")
            .calling(entry("bioprism_examples::SliceRegistry"))
            .calling(entry("bioprism_examples::CoverageReport")),
    )
    .step(
        Step::new(krate("bioprism-bioworlds"), "Cross-check against the world-shaped backlog: which generator knobs the specification names and the generator does not expose.")
            .calling(entry("bioprism_bioworlds::MissingGeneratorKnob"))
            .calling(entry("bioprism_bioworlds::BlockedProperty"))
            .calling(entry("bioprism_bioworlds::SliceCatalog")),
    )
    .step(
        Step::new(krate("bioprism-cookbook"), "Cross-check against this catalogue's own gap list: the recipes that could not be written, and why.")
            .calling(entry("bioprism_cookbook::CookbookReport"))
            .calling(entry("bioprism_cookbook::UnwrittenRecipe")),
    )
    .demonstrating(
        Claim::new(
            "the platform enumerates the claims nothing exercises, each with a concrete obstacle, \
             rather than presenting its passing tests as coverage",
        )
        .citing("19.07")
        .citing("38.01")
        .citing("43.16"),
    )
    .checked_by(enforced(
        "both world-shaped backlog properties appear in the still-blocked column with a stated reason",
        "bioprism-bioworlds",
        "crates/bioworlds/tests/catalog_contract.rs",
        "both_world_shaped_backlog_properties_appear_in_the_still_blocked_column_with_a_reason",
    ))
    .checked_by(enforced(
        "the unfavourable control ships its finding rather than hiding it",
        "bioprism-bioworlds",
        "crates/bioworlds/tests/catalog_contract.rs",
        "the_unfavourable_control_ships_its_finding_rather_than_hiding_it",
    ))
    .checked_by(observable(
        "every unexercised property names either a concrete obstacle or the fact that nothing structural prevents it",
        "`SliceRegistry::standard().coverage().render()`, second section",
        "a non-empty `CLAIMED BUT NOT EXERCISED` block in which every entry has a `blocked by:` line",
    ))
    .easy_to_get_wrong(Pitfall::new(
        "reading a green test suite as coverage of the architecture's claims",
        "a suite reports on the claims somebody wrote a test for. The claims nobody wrote a test for \
         are invisible in it, and those are exactly the ones a newcomer assumes are covered. That is \
         why the coverage report enumerates `Property::ALL` rather than the registered slices.",
    ))
    .also_reading("docs/COVERAGE.md")
    .with_budget(ROUTE_BUDGET)
    .seal()
}

fn route_documentation_for_a_task() -> Result<Recipe, CookbookError> {
    Recipe::draft(
        id("route-documentation-for-a-task"),
        "I am an agent about to do one specific job in this repository and I want the smallest set \
         of documents that makes it doable, plus a record of what I was not given.",
    )
    .step(
        Step::new(krate("bioprism-docgraph"), "Get the corpus as a typed graph — files as nodes, typed relations as edges.")
            .calling(entry("bioprism_docgraph::DocGraph"))
            .calling(entry("bioprism_docgraph::fixture::repository_doc_graph")),
    )
    .step(
        Step::new(krate("bioprism-docgraph"), "Write the route: what you are about to do, the must-reads, and the boundaries that are non-omittable whatever the budget.")
            .calling(entry("bioprism_docgraph::TaskRoute"))
            .calling(entry("bioprism_docgraph::RouteDefect")),
    )
    .step(
        Step::new(krate("bioprism-docgraph"), "Choose the traversal policy deliberately; only an exhaustive walk can license a zero-influence claim.")
            .calling(entry("bioprism_docgraph::TraversalPolicy"))
            .calling(entry("bioprism_docgraph::Completeness")),
    )
    .step(
        Step::new(krate("bioprism-docgraph"), "Compile the bundle and read the omission record before reading the bundle.")
            .calling(entry("bioprism_docgraph::compile_bundle"))
            .calling(entry("bioprism_docgraph::ContextBundle"))
            .calling(entry("bioprism_docgraph::OmissionReason")),
    )
    .demonstrating(
        Claim::new(
            "the compiler-pass argument applies to documentation as well as to evidence: a route \
             compiles to a bounded bundle, and a bundle that cannot close its mandatory set fails \
             instead of quietly shrinking",
        )
        .citing("41.05")
        .citing("41.07")
        .citing("41.09"),
    )
    .checked_by(enforced(
        "a bundle that cannot close its mandatory set within budget fails rather than truncating",
        "bioprism-docgraph",
        "crates/docgraph/tests/acceptance.rs",
        "a_bundle_that_cannot_close_its_mandatory_set_fails_rather_than_truncating",
    ))
    .checked_by(enforced(
        "only an exhaustive walk licenses classifying an unreached module as zero-influence",
        "bioprism-docgraph",
        "crates/docgraph/tests/acceptance.rs",
        "only_an_exhaustive_walk_licenses_a_zero_influence_claim",
    ))
    .checked_by(enforced(
        "a module carrying a protected class is mandatory even when no edge reaches it",
        "bioprism-docgraph",
        "crates/docgraph/tests/acceptance.rs",
        "a_module_carrying_a_protected_class_is_mandatory_even_when_no_edge_reaches_it",
    ))
    .easy_to_get_wrong(Pitfall::new(
        "listing a boundary among the route's optional modules so the route fits",
        "`TaskRoute` keeps must-read and non-omittable as two fields precisely so an author trimming \
         a route to fit cannot trim a boundary, and `check` reports the attempt as a defect rather \
         than silently promoting the module back.",
    ))
    .also_reading("docs/ARCHITECTURE.md")
    .with_budget(ROUTE_BUDGET)
    .seal()
}

fn size_a_benchmark_by_equivalence_classes() -> Result<Recipe, CookbookError> {
    Recipe::draft(
        id("size-a-benchmark-by-equivalence-classes"),
        "Somebody handed me a benchmark with a large instance count and I need to know how many \
         independent things it actually tests.",
    )
    .step(
        Step::new(krate("bioprism-scale"), "Load the corpus and take the nominal count. Note that it is a nominal count; the type will not let you forget.")
            .calling(entry("bioprism_scale::Corpus"))
            .calling(entry("bioprism_scale::NominalCount")),
    )
    .step(
        Step::new(krate("bioprism-scale"), "Choose the similarity relation and state what it refuses to merge. Every relation here declares that.")
            .calling(entry("bioprism_scale::SimilarityRelation"))
            .calling(entry("bioprism_scale::content_digest")),
    )
    .step(
        Step::new(krate("bioprism-scale"), "Compute the effective size and read the inflation ratio and the parent concentration.")
            .calling(entry("bioprism_scale::EffectiveSize"))
            .calling(entry("bioprism_scale::EffectiveSizeReport")),
    )
    .step(
        Step::new(krate("bioprism-benchcompiler"), "Check contamination separately: an instance nobody probed is unmeasured, not clean.")
            .calling(entry("bioprism_benchcompiler::analyse")),
    )
    .demonstrating(
        Claim::new(
            "benchmark size is reported as independent equivalence classes, and a nominal count is \
             never serialised without the effective count beside it",
        )
        .citing("19.03")
        .citing("32"),
    )
    .checked_by(enforced(
        "a thousand paraphrases of one parent are one equivalence class",
        "bioprism-scale",
        "crates/scale/tests/effective_size.rs",
        "a_thousand_paraphrases_of_one_parent_are_one_equivalence_class",
    ))
    .checked_by(enforced(
        "a nominal count is never serialised without the effective count",
        "bioprism-scale",
        "crates/scale/tests/effective_size.rs",
        "a_nominal_count_is_never_serialized_without_the_effective_count",
    ))
    .checked_by(enforced(
        "an instance no panel ran is reported unmeasured, not failed",
        "bioprism-benchcompiler",
        "crates/benchcompiler/tests/scale_and_contamination.rs",
        "an_instance_no_panel_ran_is_unmeasured_not_failed",
    ))
    .easy_to_get_wrong(Pitfall::new(
        "treating a low inflation ratio as evidence of independence",
        "the ratio is only as good as the relation that produced it, and a relation that merges too \
         little reports a corpus as diverse for the same reason a bad hash reports no collisions. \
         `RelationQuality` exists to measure the relation against labelled truth first.",
    ))
    .with_budget(ROUTE_BUDGET)
    .seal()
}

fn fork_two_architectures() -> Result<Recipe, CookbookError> {
    Recipe::draft(
        id("fork-two-architectures-from-one-decision-cell"),
        "Two agent architectures disagree and I want to know where they first diverged, not which \
         one scored higher.",
    )
    .step(
        Step::new(krate("bioprism-prism"), "Freeze the decision state as a cell. Its inputs are bound by digest, so both forks provably resume from the same bytes.")
            .calling(entry("bioprism_prism::DecisionCell"))
            .calling(entry("bioprism_prism::InputRef")),
    )
    .step(
        Step::new(krate("bioprism-prism"), "Describe each architecture as a strategy spec rather than as code, so the comparison is over declared policies.")
            .calling(entry("bioprism_prism::Architecture"))
            .calling(entry("bioprism_prism::StrategySpec")),
    )
    .step(
        Step::new(krate("bioprism-prism"), "Run the matched fork and read the trial table. Acceptance is set-valued and names its failure mode.")
            .calling(entry("bioprism_prism::matched_fork"))
            .calling(entry("bioprism_prism::ForkResult"))
            .calling(entry("bioprism_prism::render_table")),
    )
    .step(
        Step::new(krate("bioprism-prism"), "Minimize the world down to the facts that are load-bearing for the divergence, then attest the bundle.")
            .calling(entry("bioprism_prism::minimize"))
            .calling(entry("bioprism_prism::ResultBundle"))
            .calling(entry("bioprism_prism::Attestation")),
    )
    .demonstrating(
        Claim::new(
            "two architectures fork from one frozen decision cell, and the difference between them is \
             attributed to a declared context policy rather than to an aggregate score",
        )
        .citing("19.16")
        .citing("03")
        .citing("06"),
    )
    .checked_by(enforced(
        "a cell binds its inputs by digest, so a fork cannot silently resume from different bytes",
        "bioprism-prism",
        "crates/prism/tests/matched_fork.rs",
        "a_cell_binds_its_inputs_by_digest",
    ))
    .checked_by(enforced(
        "on the discriminating world the context policy explains the difference between architectures",
        "bioprism-prism",
        "crates/prism/tests/matched_fork.rs",
        "on_the_discriminating_world_context_policy_explains_the_difference",
    ))
    .checked_by(enforced(
        "every fact left in the minimal set is load-bearing",
        "bioprism-prism",
        "crates/prism/tests/matched_fork.rs",
        "every_fact_in_the_minimal_set_is_load_bearing",
    ))
    .easy_to_get_wrong(Pitfall::new(
        "concluding that the architecture that won is better",
        "on the reference world the panel does not separate at all — `bioprism-prism`'s own test says \
         so — so a win there is a win on a world that cannot tell the two apart. A fork is evidence \
         about a divergence, not a ranking.",
    ))
    .also_reading("docs/FINDINGS.md")
    .with_budget(ROUTE_BUDGET)
    .seal()
}

fn hand_a_result_to_a_third_party() -> Result<Recipe, CookbookError> {
    Recipe::draft(
        id("hand-a-result-to-a-third-party"),
        "I have a compiled result and I need someone who does not trust me — and does not run my \
         runtime — to be able to check it.",
    )
    .step(
        Step::new(krate("bioprism-section"), "Start from the certificate. It is the object that already states what was omitted; the bundle wraps it rather than replacing it.")
            .calling(entry("bioprism_section::ContextCertificate"))
            .calling(entry("bioprism_section::CertificateVerification")),
    )
    .step(
        Step::new(krate("bioprism-bundle"), "Build the bundle: the manifest names every entry and its role, and each entry is bound by digest.")
            .calling(entry("bioprism_bundle::BundleBuilder"))
            .calling(entry("bioprism_bundle::BundleManifest"))
            .calling(entry("bioprism_bundle::ResultBundle")),
    )
    .step(
        Step::new(krate("bioprism-bundle"), "Attest it, and read what the attestation scheme actually promises before relying on it.")
            .calling(entry("bioprism_bundle::Attestation"))
            .calling(entry("bioprism_bundle::AuthenticationScheme"))
            .calling(entry("bioprism_bundle::Repudiability")),
    )
    .step(
        Step::new(krate("bioprism-bundle"), "Have the reviewer verify and then replay it, and read what the replay says it could not compare.")
            .calling(entry("bioprism_bundle::VerifiedBundle"))
            .calling(entry("bioprism_bundle::ReproductionAttempt"))
            .calling(entry("bioprism_bundle::Divergence")),
    )
    .demonstrating(
        Claim::new(
            "a result bundle verifies and replays without linking the runtime that produced it, and \
             a replay that diverges names the entry it diverged on rather than reporting a mismatch",
        )
        .citing("19.06")
        .citing("40.05")
        .citing("43.26"),
    )
    .checked_by(enforced(
        "a bundle verifies after transport without linking the runtime that produced it",
        "bioprism-bundle",
        "crates/bundle/tests/signed_result_bundle_replay.rs",
        "a_bundle_verifies_after_transport_without_linking_the_runtime_that_produced_it",
    ))
    .checked_by(enforced(
        "a replay that recompiles a different section diverges and names the entry",
        "bioprism-bundle",
        "crates/bundle/tests/signed_result_bundle_replay.rs",
        "a_replay_that_recompiles_a_different_section_diverges_and_names_the_entry",
    ))
    .checked_by(enforced(
        "a reviewer who can verify the attestation can also forge an identical one",
        "bioprism-bundle",
        "crates/bundle/tests/signed_result_bundle_replay.rs",
        "a_reviewer_who_can_verify_can_also_forge_an_identical_bundle",
    ))
    .easy_to_get_wrong(Pitfall::new(
        "calling the attestation a signature and treating verification as proof of authorship",
        "the scheme is a symmetric MAC. Any party who holds the key can verify *and* forge, so a \
         verified bundle proves that someone with the key produced it — not which one. \
         `Repudiability` is the type that says so, and a workflow that needs non-repudiation needs \
         an asymmetric scheme this workspace does not have.",
    ))
    .with_budget(ROUTE_BUDGET)
    .seal()
}

fn verify_this_cookbook() -> Result<Recipe, CookbookError> {
    Recipe::draft(
        id("verify-a-recipe-against-the-workspace"),
        "I want to know whether the recipes in this cookbook still point at things that exist, \
         before I follow one.",
    )
    .step(
        Step::new(krate("bioprism-cookbook"), "Take the catalogue. Every entry is already known to have a goal, steps, a claim, a checkable property and a pitfall — the type has no other state.")
            .calling(entry("bioprism_cookbook::Cookbook"))
            .calling(entry("bioprism_cookbook::Recipe")),
    )
    .step(
        Step::new(krate("bioprism-cookbook"), "Open the workspace as text — no crate is linked — and resolve every crate, entry point and enforcing test the catalogue names.")
            .calling(entry("bioprism_cookbook::Workspace"))
            .calling(entry("bioprism_cookbook::verify_cookbook")),
    )
    .step(
        Step::new(krate("bioprism-cookbook"), "Read the defects. A reference that no longer resolves is reported with what it was and where it was named.")
            .calling(entry("bioprism_cookbook::VerificationReport"))
            .calling(entry("bioprism_cookbook::ReferenceStatus")),
    )
    .step(
        Step::new(krate("bioprism-cookbook"), "Compile each recipe's documentation route and read its token profile and omission record.")
            .calling(entry("bioprism_cookbook::cookbook_doc_graph"))
            .calling(entry("bioprism_cookbook::compile_recipe_route")),
    )
    .demonstrating(
        Claim::new(
            "a reference example that names a deleted function fails a test rather than becoming \
             stale prose, and it does so without linking the crate it names",
        )
        .citing("19.09")
        .citing("41.11"),
    )
    .checked_by(enforced(
        "every entry point a recipe names still exists in the workspace",
        "bioprism-cookbook",
        "crates/cookbook/tests/cookbook.rs",
        "every_entry_point_a_recipe_names_exists_in_the_workspace",
    ))
    .checked_by(enforced(
        "every test a recipe or anti-recipe leans on still exists, by name, in the file it names",
        "bioprism-cookbook",
        "crates/cookbook/tests/cookbook.rs",
        "every_enforcing_test_a_recipe_names_still_exists_by_name",
    ))
    .checked_by(enforced(
        "every recipe's documentation route compiles within its declared budget",
        "bioprism-cookbook",
        "crates/cookbook/tests/cookbook.rs",
        "every_recipe_route_compiles_within_its_declared_budget",
    ))
    .easy_to_get_wrong(Pitfall::new(
        "reading a clean verification as evidence that the recipes work",
        "verification resolves names. It proves that the crates, entry points and tests exist and \
         that the documentation routes compile; it does not execute a single step. What proves a \
         recipe's claim is the test named in its `Check::EnforcedByTest`, and that test lives in the \
         crate that owns the behaviour, not here.",
    ))
    .with_budget(ROUTE_BUDGET)
    .seal()
}

/// Every anti-recipe this crate ships.
pub fn anti_recipes() -> Result<Vec<AntiRecipe>, CookbookError> {
    Ok(vec![
        AntiRecipe::new(
            id("cheapness-is-not-admissibility"),
            "rank context strategies by how few facts they select and declare the smallest one the winner",
            "on the discriminating world a lexical retriever selects as few facts as the compiler and \
             reaches the right verdict — from a 91% protected closure, having dropped a protected fact \
             that happened not to matter. Size is not a proxy for correctness and neither is the \
             verdict; ranking on either crowns the strategy that violated the mandatory closure and \
             got away with it.",
            "rank on admissibility: right verdict *and* full protected closure, with the closure \
             fraction reported next to the selection size so a reader can see both.",
            test(
                "bioprism-examples",
                "crates/examples/tests/vertical_slices.rs",
                "a_valid_verdict_with_an_unsatisfied_closure_does_not_support_a_sufficiency_claim",
            ),
        )?
        .also_enforced_by(test(
            "bioprism-examples",
            "crates/examples/tests/vertical_slices.rs",
            "no_graph_walk_depth_is_sound_closed_and_compact_on_the_discriminating_world",
        ))
        .sharpening(agents::admissibility()),
        AntiRecipe::new(
            id("a-right-answer-from-an-incomplete-closure-is-not-a-pass"),
            "accept a strategy because its verdict matched the reference, without checking whether the \
             protected closure was satisfied",
            "protected closure is computed before any relevance step precisely so that a strategy \
             cannot be credited for guessing correctly from evidence it never had. A verdict is one \
             bit; on a world with 750 distractors, agreeing on one bit is cheap. The closure is the \
             basis, and a basis with a hole in it supports nothing.",
            "read the omission manifest first: any group classified Unknown voids the sufficiency \
             claim regardless of the verdict, and `OmissionManifest::supports_sufficiency_claim` is \
             the function that decides it.",
            test(
                "bioprism-examples",
                "crates/examples/tests/vertical_slices.rs",
                "a_valid_verdict_with_an_unsatisfied_closure_does_not_support_a_sufficiency_claim",
            ),
        )?
        .also_enforced_by(test(
            "bioprism-examples",
            "crates/examples/tests/vertical_slices.rs",
            "protected_closure_admits_facts_no_dependency_path_reaches",
        ))
        .sharpening(agents::incomplete_basis()),
        AntiRecipe::new(
            id("instance-count-is-not-benchmark-count"),
            "generate a thousand paraphrases of one parent world and report a thousand-instance benchmark",
            "paraphrases of one parent are one equivalence class. A thousand of them measure \
             robustness on one item, and quoting the nominal count makes the benchmark look three \
             orders of magnitude more discriminating than it is. Relabelling the subjects does not \
             help either — the deduplicator is not defeated by renaming.",
            "report independent equivalence classes, and never serialise a nominal count without the \
             effective count beside it. `EffectiveSizeReport` and `Diversity` both exist to make that \
             the default rather than a discipline.",
            test(
                "bioprism-mutation",
                "crates/mutation/tests/metamorphic.rs",
                "instance_count_is_not_benchmark_count",
            ),
        )?
        .also_enforced_by(test(
            "bioprism-scale",
            "crates/scale/tests/effective_size.rs",
            "a_thousand_paraphrases_of_one_parent_are_one_equivalence_class",
        ))
        .also_enforced_by(test(
            "bioprism-benchcompiler",
            "crates/benchcompiler/tests/scale_and_contamination.rs",
            "effective_diversity_headlines_equivalence_classes_not_instance_count",
        ))
        .sharpening(agents::instance_count()),
        AntiRecipe::new(
            id("an-unreached-node-is-not-a-proven-zero"),
            "cap a traversal at a shallow depth for speed and classify everything it did not reach as \
             zero-influence",
            "zero influence means provably cannot matter. A depth cap means nobody looked past the \
             cap. The same unreached module is Zero under an exhaustive walk and Unknown under a \
             capped one, which is the proof that the classification is a property of the *search* and \
             not of the node — so a walk that was narrowed in any way, including by filtering an edge \
             type that the graph actually uses, cannot license the zero.",
            "read `Completeness::licenses_zero_influence` and let it decide. Under any narrowed walk \
             the omission is Unknown, and one Unknown group voids the sufficiency claim.",
            test(
                "bioprism-docgraph",
                "crates/docgraph/tests/acceptance.rs",
                "only_an_exhaustive_walk_licenses_a_zero_influence_claim",
            ),
        )?
        .also_enforced_by(test(
            "bioprism-docgraph",
            "crates/docgraph/tests/acceptance.rs",
            "the_same_unreached_module_is_zero_under_an_exhaustive_walk_and_unknown_under_a_capped_one",
        ))
        .also_enforced_by(test(
            "bioprism-docgraph",
            "crates/docgraph/tests/acceptance.rs",
            "a_module_unreachable_only_because_an_edge_type_was_filtered_is_not_zero_influence",
        ))
        .sharpening(agents::zero_is_not_unknown()),
        AntiRecipe::new(
            id("raising-the-budget-until-it-fits"),
            "catch the budget error, raise the budget, and retry until the bundle compiles — or drop \
             the cheapest mandatory module to make room",
            "the refusal is the finding. A truncated mandatory set is indistinguishable at the point of \
             use from a complete one: an agent handed nine of ten required contracts has no way to \
             notice, and neither has the reader of its output. Retrying at a larger budget is the \
             benign version and is still a decision nobody wrote down.",
            "write a smaller route — fewer must-reads, a narrower subject — or accept the larger budget \
             deliberately and record that you did. Both are decisions with an author; silent \
             truncation is not.",
            test(
                "bioprism-docgraph",
                "crates/docgraph/tests/acceptance.rs",
                "a_bundle_that_cannot_close_its_mandatory_set_fails_rather_than_truncating",
            ),
        )?
        .also_enforced_by(test(
            "bioprism-examples",
            "crates/examples/tests/vertical_slices.rs",
            "a_budget_below_the_protected_closure_is_refused_rather_than_truncated",
        )),
        AntiRecipe::new(
            id("an-estimate-is-not-a-measurement"),
            "quote the token number on a compiled bundle as the context size",
            "there is no tokenizer in this workspace. Every token number here is produced by a stated \
             heuristic, carries the estimator's id, and a sum containing one estimate degrades the \
             whole total to an estimate. Quoting it as a count is the same error as `score_or_zero`: \
             it turns an absence of measurement into a number that looks measured.",
            "quote it with its basis — `estimated, chars4-words-floor/v1` — or produce a real one \
             through `TokenCost::measured`, which cannot be called without naming the tokenizer that \
             did the measuring.",
            test(
                "bioprism-docgraph",
                "crates/docgraph/tests/acceptance.rs",
                "no_cost_this_crate_produces_from_a_document_is_a_measurement",
            ),
        )?
        .also_enforced_by(test(
            "bioprism-docgraph",
            "crates/docgraph/tests/acceptance.rs",
            "a_total_containing_one_estimate_is_never_reported_as_a_measurement",
        ))
        .sharpening(agents::unmeasured_is_not_zero()),
        AntiRecipe::new(
            id("an-obligation-read-out-of-a-card"),
            "answer a normative question from a module's context card, because the card was what the \
             bundle contained",
            "a card is a lossy rendering the registry wrote; it is sized at 60–180 tokens and exists so \
             an agent can decide whether to load the file. Reading an obligation out of it is reading \
             an obligation out of a summary, and the summary's author was a `render` function.",
            "check `ProfileLevel::is_normative` — only Contract and DeepReference qualify — and load \
             the module at Contract before citing it. The bundle compiler already delivers mandatory \
             modules at Contract for exactly this reason.",
            test(
                "bioprism-docgraph",
                "crates/docgraph/tests/acceptance.rs",
                "a_card_is_not_a_level_an_obligation_may_be_read_from",
            ),
        )?,
    ])
}
