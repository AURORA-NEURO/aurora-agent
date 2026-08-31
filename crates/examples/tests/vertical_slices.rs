//! What the reference examples establish, asserted.
//!
//! Every claim in `crates/examples` is a [`bioprism_examples::VerticalSlice`] carrying its own
//! expected outcome, so the first test below is the whole suite in one line: run every registered
//! slice and require that each still produces what it says it produces. The rest exist because
//! that test alone is not enough — it would pass on an empty registry, on a registry of slices
//! that assert nothing, and on a catalogue that quietly dropped the claims it could not meet.
//!
//! So the tests here also assert things *about* the suite: that the catalogue enumerates claims no
//! slice exercises, that every such claim names a concrete obstacle, that a broken expectation
//! surfaces as a recorded failure rather than an error, and that a broken example surfaces as an
//! error rather than a recorded failure. The two are different findings and the suite must not be
//! able to confuse them.

use bioprism_examples::catalog::{ALL_WITNESS_KINDS, DECISIVE_FACTS, DEFERRED_PASSES};
use bioprism_examples::{
    catalog, walk, Compiled, ExampleError, Expectation, Property, QueryOverlay, RefusalCode,
    SliceRegistry, SliceReport, SliceWorld, VerticalSlice,
};
use bioprism_section::{LeakageWitness, OracleStatus};
use bioprism_worldgen::WorldSpec;
use std::collections::BTreeSet;

fn registry() -> SliceRegistry {
    SliceRegistry::standard()
}

fn run(id: &str) -> SliceReport {
    registry()
        .run(id)
        .unwrap_or_else(|error| panic!("slice {id} is broken: {error}"))
}

fn compiled(id: &str) -> bioprism_examples::CompiledObservation {
    run(id)
        .observations
        .compiled
        .unwrap_or_else(|| panic!("slice {id} was expected to compile"))
}

// ---------------------------------------------------------------------------
// The suite itself
// ---------------------------------------------------------------------------

/// The claim this whole crate exists to make checkable.
#[test]
fn every_registered_slice_still_demonstrates_the_property_it_claims() {
    let report = registry().run_all().expect("every slice must be runnable");
    assert!(
        report.holds(),
        "one or more reference examples stopped holding:\n{}",
        report.render()
    );
}

#[test]
fn the_registry_registers_more_than_the_canonical_slice() {
    let registry = registry();
    assert!(
        registry.len() >= 5,
        "one passing example cannot separate a working oracle from a constant verdict"
    );
    assert!(registry.ids().contains(&"radiogenomic-integrity-v1"));
}

#[test]
fn slice_ids_are_unique_so_a_report_names_exactly_one_slice() {
    registry()
        .validate()
        .expect("standard registry is well formed");

    let duplicated = vec![catalog::clean_cohort(), catalog::clean_cohort()];
    let error = SliceRegistry::from_slices(duplicated)
        .expect_err("two slices sharing an id must be rejected");
    assert!(matches!(error, ExampleError::DuplicateSlice(id) if id == "clean-cohort-v1"));
}

#[test]
fn asking_for_an_unregistered_slice_is_a_typed_error() {
    let error = registry()
        .run("no-such-slice")
        .expect_err("an unknown id must not silently return an empty report");
    assert!(matches!(error, ExampleError::UnknownSlice(id) if id == "no-such-slice"));
}

#[test]
fn every_slice_cites_a_blueprint_module_and_explains_itself() {
    for slice in registry().slices() {
        assert!(
            !slice.blueprint_modules.is_empty(),
            "{} cites no blueprint module, so a reader cannot check it against the spec",
            slice.id
        );
        assert!(
            slice.narrative.len() > 200,
            "{} has no narrative; an example without one is a fixture, not a reference",
            slice.id
        );
    }
}

// ---------------------------------------------------------------------------
// Determinism
// ---------------------------------------------------------------------------

#[test]
fn running_the_whole_registry_twice_produces_the_same_digest() {
    let first = registry().run_all().expect("first run");
    let second = registry().run_all().expect("second run");
    assert_eq!(
        first.digest, second.digest,
        "these are the artifacts a newcomer runs first; two runs must agree byte for byte"
    );
    assert_eq!(first, second);
}

#[test]
fn every_slice_report_digest_covers_its_own_body() {
    let report = registry().run_all().expect("runs");
    for slice in &report.slices {
        assert!(
            slice.digest_is_intact(),
            "{} carries a digest that does not match its own body",
            slice.slice_id
        );
    }
    assert!(report.digest_is_intact());
}

#[test]
fn editing_a_report_breaks_its_digest() {
    let mut report = run("clean-cohort-v1");
    assert!(report.digest_is_intact());
    report.failures.push("invented after the fact".into());
    assert!(
        !report.digest_is_intact(),
        "a report whose findings can be edited without breaking its digest is not a receipt"
    );
}

#[test]
fn a_slice_definition_survives_a_round_trip_through_serde() {
    let slice = catalog::temporal_firewall();
    let encoded = serde_json::to_string(&slice).expect("slices serialise");
    let decoded: VerticalSlice = serde_json::from_str(&encoded).expect("slices deserialise");

    let original = slice.run().expect("original runs");
    let replayed = decoded.run().expect("decoded runs");
    assert_eq!(
        original.digest, replayed.digest,
        "a slice must be fully described by its serialised form, or it cannot be published"
    );
}

// ---------------------------------------------------------------------------
// 43.41 — the canonical slice
// ---------------------------------------------------------------------------

#[test]
fn the_radiogenomic_slice_compiles_eleven_facts_out_of_seven_hundred_and_sixty_two() {
    let report = run("radiogenomic-integrity-v1");
    assert_eq!(report.observations.total_facts, 762);
    assert_eq!(
        report
            .observations
            .compiled
            .as_ref()
            .expect("compiles")
            .selected_facts,
        DECISIVE_FACTS.map(String::from).to_vec()
    );
}

/// A witness is only checkable if it names the objects it accuses.
#[test]
fn the_radiogenomic_witnesses_name_the_subjects_a_reader_would_have_to_refute() {
    let observed = compiled("radiogenomic-integrity-v1");
    assert_eq!(observed.status, OracleStatus::Invalid);
    assert_eq!(observed.witness_kinds, ALL_WITNESS_KINDS.to_vec());

    let mut seen_identity = false;
    let mut seen_site = false;
    let mut seen_temporal = false;
    for witness in &observed.witnesses {
        match witness {
            LeakageWitness::IdentityLeakage {
                alias,
                subjects,
                splits,
            } => {
                seen_identity = true;
                assert_eq!(alias, "ALT-77");
                assert_eq!(subjects, &["S001".to_string(), "S004".to_string()]);
                assert_eq!(splits, &["test".to_string(), "train".to_string()]);
            }
            LeakageWitness::SiteLeakage { site_by_split } => {
                seen_site = true;
                assert_eq!(site_by_split["train"], vec!["A".to_string()]);
                assert_eq!(site_by_split["test"], vec!["B".to_string()]);
            }
            LeakageWitness::TemporalLeakage {
                decision_time,
                future_label_sources,
            } => {
                seen_temporal = true;
                assert_eq!(decision_time, "2025-01-01");
                assert_eq!(future_label_sources["S004"], "2025-06-01");
            }
            LeakageWitness::PreprocessingLeakage { .. } => {}
            LeakageWitness::DomainCheck { check, .. } => {
                panic!(
                    "the reference split-integrity oracle emits no domain_check witness, yet the \
                     radiogenomic slice produced one for rule {check:?}"
                )
            }
        }
    }
    assert!(seen_identity && seen_site && seen_temporal);
}

#[test]
fn every_compile_declares_the_passes_it_could_not_run() {
    for slice in registry().slices() {
        let report = slice.run().expect("runs");
        let Some(observed) = report.observations.compiled else {
            continue;
        };
        let declared: Vec<&str> = observed
            .deferred_passes
            .iter()
            .map(|pass| pass.pass.as_str())
            .collect();
        for expected in DEFERRED_PASSES {
            assert!(
                declared.contains(&expected),
                "{} did not declare the deferred pass {expected:?}",
                slice.id
            );
        }
        for pass in &observed.deferred_passes {
            assert!(
                pass.reason.len() > 20,
                "deferred pass {:?} names no reason, which reads as 'coming soon'",
                pass.pass
            );
        }
    }
}

#[test]
fn every_compiled_slice_emits_a_certificate_that_verifies_its_own_digest() {
    for slice in registry().slices() {
        let report = slice.run().expect("runs");
        let Some(observed) = report.observations.compiled else {
            continue;
        };
        assert!(
            observed.certificate_verifies,
            "{} produced a certificate whose embedded digest does not match its body",
            slice.id
        );
        assert_ne!(
            observed.certificate_digest_reference, observed.certificate_digest_extended,
            "{}: the extended profile adds the omission manifest, so it must hash differently",
            slice.id
        );
    }
}

// ---------------------------------------------------------------------------
// The negative control
// ---------------------------------------------------------------------------

#[test]
fn a_world_without_injected_defects_returns_valid_so_the_oracle_is_not_a_constant_invalid() {
    let observed = compiled("clean-cohort-v1");
    assert_eq!(observed.status, OracleStatus::Valid);
    assert!(observed.witnesses.is_empty());
    assert!(observed.protected_closure_satisfied);
    assert!(observed.supports_sufficiency_claim);
}

#[test]
fn each_leakage_mechanism_produces_exactly_its_own_witness_kind() {
    for (id, expected) in [
        ("mutation-identity-v1", "identity_leakage"),
        ("mutation-site-v1", "site_leakage"),
        ("mutation-temporal-v1", "temporal_leakage"),
        ("mutation-preprocessing-v1", "preprocessing_leakage"),
    ] {
        let observed = compiled(id);
        assert_eq!(
            observed.witness_kinds,
            vec![expected.to_string()],
            "{id} should isolate one mechanism"
        );
        assert_eq!(
            observed.selected_facts,
            DECISIVE_FACTS.map(String::from).to_vec()
        );
    }
}

// ---------------------------------------------------------------------------
// 43.09 — the temporal cut
// ---------------------------------------------------------------------------

#[test]
fn an_early_decision_cut_withholds_the_protected_evidence_the_verdict_depends_on() {
    let observed = compiled("temporal-firewall-v1");
    assert_eq!(observed.selected_facts.len(), 8);
    assert_eq!(observed.protected_closure.len(), 11);
    assert_eq!(
        observed.dropped_protected,
        vec![
            "fact.decision_cut".to_string(),
            "fact.preprocess_fit".to_string(),
            "fact.split".to_string()
        ]
    );
    assert!(!observed.protected_closure_satisfied);
    assert_eq!(observed.unresolved_obligations.len(), 3);
}

/// The most dangerous state in the suite: a `valid` verdict that must not be published.
#[test]
fn a_valid_verdict_with_an_unsatisfied_closure_does_not_support_a_sufficiency_claim() {
    let observed = compiled("temporal-firewall-v1");
    assert_eq!(
        observed.status,
        OracleStatus::Valid,
        "the oracle genuinely finds nothing wrong in what it was allowed to read"
    );
    assert!(observed.witnesses.is_empty());
    assert!(
        !observed.supports_sufficiency_claim,
        "an omission classed deferred_acquisition must void the sufficiency claim"
    );
    assert!(
        observed
            .omission_influence_classes
            .contains(&"deferred_acquisition".to_string()),
        "withheld evidence is never zero influence: it may well change the decision"
    );
}

#[test]
fn an_undischarged_obligation_comes_with_a_move_that_would_discharge_it() {
    let observed = compiled("temporal-firewall-v1");
    assert_eq!(
        observed.refinement_frontier_actions,
        vec!["advance_time_cut_or_use_retrospective_mode".to_string()]
    );
}

#[test]
fn no_omission_group_in_the_suite_is_left_unclassified() {
    for slice in registry().slices() {
        let report = slice.run().expect("runs");
        let Some(observed) = report.observations.compiled else {
            continue;
        };
        assert!(
            !observed
                .omission_influence_classes
                .contains(&"unknown".to_string()),
            "{} emitted an unknown-influence omission group",
            slice.id
        );
    }
}

/// The separation 43.09 asks for: withheld evidence and a broken closure are different failures.
#[test]
fn a_withheld_unprotected_fact_leaves_the_closure_intact_where_a_protected_one_does_not() {
    let protected = compiled("temporal-firewall-v1");
    let unprotected = compiled("unprotected-temporal-withholding-v1");

    assert!(!protected.dropped_protected.is_empty());
    assert!(!protected.protected_closure_satisfied);

    assert!(
        unprotected.dropped_protected.is_empty(),
        "the withheld fact carries no protected tag, so nothing may be reported as dropped"
    );
    assert!(unprotected.protected_closure_satisfied);
    assert_eq!(
        unprotected.protected_closure.len(),
        11,
        "an empty closure would be satisfied trivially and prove nothing"
    );
    assert_eq!(
        unprotected.inaccessible_selected_before_cut,
        vec!["fact.central_lab".to_string()],
        "a complete closure must not be allowed to stand in for complete evidence"
    );
}

/// The withheld fact is one the target depends on, not a spare the selection could afford to lose.
#[test]
fn the_fact_the_cut_withheld_is_read_by_a_check_inside_the_compiled_slice() {
    let observed = compiled("unprotected-temporal-withholding-v1");
    assert!(observed
        .selected_factors
        .contains(&"factor.confirmation_check".to_string()));
    assert!(
        !observed.supports_sufficiency_claim,
        "evidence withheld from a compiled check cannot support a sufficiency claim"
    );
    assert!(observed
        .omission_influence_classes
        .contains(&"deferred_acquisition".to_string()));
}

// ---------------------------------------------------------------------------
// 43.33 / 39.05 — policy
// ---------------------------------------------------------------------------

#[test]
fn a_fact_withheld_by_policy_is_classed_as_policy_blocked_rather_than_as_irrelevant() {
    let observed = compiled("policy-blocked-omission-v1");
    assert!(
        observed
            .omission_influence_classes
            .contains(&"inaccessible_by_policy".to_string()),
        "observed classes were {:?}",
        observed.omission_influence_classes
    );
    assert!(
        !observed
            .selected_facts
            .contains(&"fact.local_lab".to_string()),
        "the screen must actually withhold the fact, not merely label it"
    );
    assert!(
        observed.protected_closure_satisfied,
        "a policy requirement over a protected variable is a refusal; this world puts it elsewhere"
    );
    assert!(!observed.supports_sufficiency_claim);
}

/// The pair argument, for the third time: one field differs, and the omission changes class.
#[test]
fn the_policy_pair_differs_only_in_the_clause_the_query_accepts() {
    let withheld = run("policy-blocked-omission-v1");
    let released = run("policy-released-control-v1");

    assert_eq!(
        withheld.observations.world_digest, released.observations.world_digest,
        "a requirement lives on the world and a grant lives on the query; the corpus must not move"
    );
    assert_ne!(
        withheld.observations.query_id,
        released.observations.query_id
    );

    let withheld = withheld.observations.compiled.expect("compiles");
    let released = released.observations.compiled.expect("compiles");

    let gained: Vec<&String> = released
        .selected_facts
        .iter()
        .filter(|id| !withheld.selected_facts.contains(id))
        .collect();
    assert_eq!(
        gained,
        vec!["fact.local_lab"],
        "accepting one clause must release exactly the fact that clause governs"
    );
    assert!(!released
        .omission_influence_classes
        .contains(&"inaccessible_by_policy".to_string()));
    assert_eq!(
        released.inaccessible_selected_before_cut, withheld.inaccessible_selected_before_cut,
        "no clause a caller accepts moves a release date"
    );
}

// ---------------------------------------------------------------------------
// 34.14 / 19.06 — the result bundle
// ---------------------------------------------------------------------------

#[test]
fn a_compiled_certificate_survives_the_bundle_round_trip_and_still_self_verifies() {
    let report = run("attested-bundle-replay-v1");
    let bundle = report.observations.bundle.expect("the probe runs");
    assert!(bundle.survives_json_round_trip);
    assert_eq!(bundle.embedded_certificate, "self_verified");
    assert_eq!(
        bundle.recomputed_entries,
        vec![
            "certificate".to_string(),
            "query".to_string(),
            "section".to_string()
        ]
    );
}

/// An entry that travelled as a digest is unchecked, and the report has to keep saying so.
#[test]
fn the_world_travels_as_a_digest_and_is_reported_as_not_recomputed_rather_than_as_a_pass() {
    let report = run("attested-bundle-replay-v1");
    let bundle = report.observations.bundle.expect("the probe runs");
    assert_eq!(bundle.not_recomputed, vec!["world".to_string()]);
    assert!(
        bundle.honest_label.contains("recorded by digest only"),
        "{}",
        bundle.honest_label
    );
}

/// The limit that makes the claim's old wording unearned, asserted rather than footnoted.
#[test]
fn a_reviewer_without_the_producing_key_learns_nothing_and_one_holding_it_can_forge() {
    let report = run("attested-bundle-replay-v1");
    let bundle = report.observations.bundle.expect("the probe runs");
    assert_eq!(bundle.without_the_key, "wrong_key_offered");
    assert!(
        bundle.verifier_forgery_is_identical,
        "under a shared secret the set of parties that can check a tag is the set that can write it"
    );
    assert_eq!(bundle.repudiability, "forgeable-by-any-verifier");
    assert_eq!(bundle.scheme, "symmetric-shared-secret");
    assert!(bundle.tag.starts_with("hmac-sha256:"));
}

/// A tag authenticates a key. The catalogue's name for the claim must not outrun that.
#[test]
fn no_property_in_the_catalogue_claims_a_signature() {
    assert_eq!(
        Property::AttestedResultBundleReplay.id(),
        "attested_result_bundle_replay"
    );
    for property in Property::ALL {
        assert!(
            !property.id().contains("signed"),
            "{} claims a signature, and the workspace has no asymmetric scheme to back one",
            property.id()
        );
        assert!(
            !property.claim().contains("signed"),
            "{} claims a signature in its statement",
            property.id()
        );
    }
}

// ---------------------------------------------------------------------------
// 43.13 / 43.25 — closure and refusal
// ---------------------------------------------------------------------------

#[test]
fn a_budget_below_the_protected_closure_is_refused_rather_than_truncated() {
    let report = run("budget-refusal-v1");
    assert!(report.observations.compiled.is_none());
    let refusal = report.observations.refused.expect("must refuse");
    assert_eq!(refusal.code, RefusalCode::BudgetExceeded);
    assert_eq!(refusal.selected, Some(11));
    assert_eq!(refusal.max_facts, Some(5));
}

/// The pair argument: one field differs, and the answer flips.
#[test]
fn the_narrow_target_pair_differs_only_in_the_protected_tag_list() {
    let without = run("relevance-only-narrow-target-v1");
    let with = run("protected-closure-narrow-target-v1");

    assert_eq!(
        without.observations.world_digest, with.observations.world_digest,
        "the pair is only an argument if both ran against the same world"
    );
    assert_ne!(without.observations.query_id, with.observations.query_id);

    let without = without.observations.compiled.expect("compiles");
    let with = with.observations.compiled.expect("compiles");

    assert_eq!(without.selected_facts, vec!["fact.policy".to_string()]);
    assert_eq!(without.status, OracleStatus::Valid);
    assert!(without.witnesses.is_empty());

    assert_eq!(
        with.selected_facts,
        DECISIVE_FACTS.map(String::from).to_vec()
    );
    assert_eq!(with.status, OracleStatus::Invalid);
    assert_eq!(with.witness_kinds, ALL_WITNESS_KINDS.to_vec());
}

#[test]
fn protected_closure_admits_facts_no_dependency_path_reaches() {
    let observed = compiled("protected-closure-narrow-target-v1");
    assert_eq!(
        observed.selected_factors,
        vec!["factor.policy_check".to_string()],
        "the dependency slice reaches exactly one factor"
    );
    assert_eq!(
        observed.protected_closure.len(),
        11,
        "closure contributes the other ten facts on its own"
    );
}

#[test]
fn a_protected_tag_that_matches_nothing_is_reported_rather_than_counted_as_satisfied() {
    let observed = compiled("protected-closure-narrow-target-v1");
    assert_eq!(
        observed.unmatched_protected_tags,
        vec!["consent".to_string()],
        "an empty closure for a requested tag would overstate what was protected"
    );
}

// ---------------------------------------------------------------------------
// 43.38 / 43.39 — equal-engineering comparison
// ---------------------------------------------------------------------------

#[test]
fn on_the_shipped_world_a_depth_five_walk_selects_the_identical_compiled_set() {
    let slice = catalog::reference_world_tie();
    let (world, query) = slice.world.build(&slice.id).expect("world builds");
    let walked = walk::facts_at(&world, &query, 5);
    let compiled: BTreeSet<String> = DECISIVE_FACTS.iter().map(|f| (*f).to_string()).collect();
    assert_eq!(
        walked, compiled,
        "the same eleven facts, not merely eleven facts: the compiler buys nothing here"
    );
}

#[test]
fn the_canonical_slice_and_its_baseline_slice_run_against_the_same_bytes() {
    let compiler = run("radiogenomic-integrity-v1");
    let baseline = run("reference-world-tie-v1");
    assert_eq!(
        compiler.observations.world_digest,
        baseline.observations.world_digest
    );
}

#[test]
fn no_graph_walk_depth_is_sound_closed_and_compact_on_the_discriminating_world() {
    let report = run("structural-discrimination-v1");
    let walk = report.observations.graph_walk.expect("probe runs");
    assert!(
        walk.usable_depths.is_empty(),
        "expected no usable depth, found {:?}",
        walk.usable_depths
    );
    let deep = walk
        .depths
        .iter()
        .find(|d| d.depth == 7)
        .expect("depth 7 observed");
    assert_eq!(deep.facts_selected, 750);
    assert!(
        !deep.verdict_preserving,
        "depth 7 takes 98% of the world and still misses every witness"
    );
}

/// The walk is not handicapped: given enough depth it reaches everything the compiler does.
#[test]
fn the_neighbourhood_walk_is_given_the_same_world_and_can_reach_the_whole_of_it() {
    let slice = catalog::structural_discrimination();
    let (world, query) = slice.world.build(&slice.id).expect("world builds");
    let deep = walk::facts_at(&world, &query, 16);
    for fact in DECISIVE_FACTS {
        assert!(
            deep.contains(fact),
            "a fair baseline must be able to reach {fact}"
        );
    }
    assert!(deep.len() >= world.facts.len() - 1);
}

#[test]
fn the_shipped_world_and_the_discriminating_world_differ_only_in_structure() {
    let tie = run("reference-world-tie-v1");
    let discriminating = run("structural-discrimination-v1");

    let tie_walk = tie.observations.graph_walk.expect("probe");
    let discriminating_walk = discriminating.observations.graph_walk.expect("probe");
    assert_eq!(tie_walk.usable_depths, vec![5, 6]);
    assert!(discriminating_walk.usable_depths.is_empty());

    assert_eq!(
        tie.observations.compiled.expect("compiles").selected_facts,
        discriminating
            .observations
            .compiled
            .expect("compiles")
            .selected_facts,
        "the compiled region is invariant to the structure the walk is sensitive to"
    );
}

// ---------------------------------------------------------------------------
// 38.01 — the world contract
// ---------------------------------------------------------------------------

#[test]
fn growing_the_cohort_thirtyfold_does_not_grow_the_compiled_region() {
    let small = compiled("mutation-identity-v1");
    let large = compiled("cohort-scale-v1");
    assert_eq!(large.selected_facts, small.selected_facts);
    assert_eq!(large.witness_kinds, ALL_WITNESS_KINDS.to_vec());
}

// ---------------------------------------------------------------------------
// Failure semantics
// ---------------------------------------------------------------------------

/// A claim that stops holding is a finding, not a crash.
#[test]
fn an_expectation_that_stops_holding_is_recorded_as_a_failure_not_an_error() {
    let mut slice = catalog::clean_cohort();
    slice.expectation = Expectation::compiles(Compiled::new().status(OracleStatus::Invalid));

    let report = slice
        .run()
        .expect("a broken claim must still produce a report");
    assert!(!report.holds());
    assert_eq!(report.failures.len(), 1);
    assert!(report.failures[0].contains("oracle status"));
    assert_eq!(
        report.observations.compiled.expect("compiles").status,
        OracleStatus::Valid,
        "the observation is recorded even though the expectation rejected it"
    );
}

#[test]
fn expecting_a_refusal_that_does_not_happen_is_a_failure() {
    let mut slice = catalog::clean_cohort();
    slice.expectation =
        Expectation::refuses(bioprism_examples::Refusal::new(RefusalCode::BudgetExceeded));
    let report = slice.run().expect("still reports");
    assert!(!report.holds());
    assert!(report.failures[0].contains("expected the slice to refuse"));
}

/// A broken example is a different finding from a broken claim, and must not masquerade as one.
#[test]
fn a_slice_whose_own_query_is_malformed_is_an_error_not_a_failing_report() {
    let slice = VerticalSlice::new(
        "malformed-query-probe",
        "A query the schema rejects",
        Property::ExactLeakageWitnesses,
        SliceWorld::new(WorldSpec::reference_like(4).with_world_id("malformed-query-probe"))
            .with_query(QueryOverlay::new().decision_time("the day before yesterday")),
        Expectation::compiles(Compiled::new()),
    );
    let error = slice
        .run()
        .expect_err("an example that cannot be built has demonstrated nothing");
    assert!(matches!(error, ExampleError::Query { .. }));
}

// ---------------------------------------------------------------------------
// Coverage — the half that reports what is *not* demonstrated
// ---------------------------------------------------------------------------

#[test]
fn the_coverage_report_accounts_for_every_claimed_property_exactly_once() {
    let coverage = registry().coverage();
    assert_eq!(coverage.total_claims(), Property::ALL.len());

    let mut seen: BTreeSet<Property> = BTreeSet::new();
    for claim in coverage.demonstrated.iter().chain(&coverage.unexercised) {
        assert!(
            seen.insert(claim.property),
            "{} appears twice in the coverage report",
            claim.id
        );
    }
    assert_eq!(seen.len(), Property::ALL.len());
}

/// A catalogue that lists only what passes is marketing.
#[test]
fn the_coverage_report_names_claims_that_no_slice_exercises() {
    let coverage = registry().coverage();
    assert!(
        !coverage.unexercised.is_empty(),
        "the architecture asserts more than this crate can run; a report claiming otherwise is false"
    );
    for claim in &coverage.unexercised {
        assert!(
            claim.exercised_by.is_empty(),
            "{} is listed as unexercised but names slices",
            claim.id
        );
    }
}

#[test]
fn every_unexercised_claim_names_a_concrete_obstacle() {
    let coverage = registry().coverage();
    let unblocked: Vec<&str> = coverage
        .unexercised_without_blocker()
        .iter()
        .map(|claim| claim.id.as_str())
        .collect();
    assert!(
        unblocked.is_empty(),
        "these claims are unexercised for no stated reason, which is indistinguishable from having been forgotten: {unblocked:?}"
    );
}

/// A blocker is the reason no slice runs a claim, so a demonstrated claim must not carry one.
///
/// Without this, closing a claim and forgetting to withdraw its obstacle would leave the registry
/// asserting both that a slice exercises the property and that nothing can — and the coverage table
/// prints the obstacle only in the unexercised half, so nobody would see the contradiction.
#[test]
fn a_claim_a_slice_demonstrates_no_longer_carries_an_obstacle() {
    for claim in &registry().coverage().demonstrated {
        assert_eq!(
            claim.blocker, None,
            "{} is demonstrated by {:?} and still records an obstacle",
            claim.id, claim.exercised_by
        );
    }
}

#[test]
fn a_blocked_claim_names_what_would_have_to_change_rather_than_that_something_is_missing() {
    for property in Property::ALL {
        let Some(blocker) = property.blocker() else {
            continue;
        };
        assert!(
            blocker.len() > 60,
            "{}: 'not implemented' is not a blocker description",
            property.id()
        );
    }
}

#[test]
fn the_four_passes_the_compiler_defers_are_all_registered_as_unexercised_claims() {
    let coverage = registry().coverage();
    for property in [
        Property::ObstructionAndGluing,
        Property::AbstractInterpretation,
        Property::DecisionEquivalenceQuotient,
        Property::RateDistortionOptimisation,
    ] {
        assert!(
            !coverage.contains(property),
            "{} is deferred by every compile and must not be reported as demonstrated",
            property.id()
        );
    }
}

#[test]
fn every_exercised_claim_is_backed_by_a_slice_that_actually_passes() {
    let report = registry().run_all().expect("runs");
    for claim in &report.coverage.demonstrated {
        for slice_id in &claim.exercised_by {
            let backing = report
                .slices
                .iter()
                .find(|report| &report.slice_id == slice_id)
                .unwrap_or_else(|| panic!("{claim:?} names a slice that did not run"));
            assert!(
                backing.holds(),
                "{} is reported as demonstrated by {slice_id}, which failed",
                claim.id
            );
        }
    }
}

#[test]
fn every_property_id_matches_its_serde_encoding() {
    for property in Property::ALL {
        let encoded = serde_json::to_string(&property).expect("serialises");
        assert_eq!(encoded, format!("\"{}\"", property.id()));
        assert!(
            !property.blueprint_modules().is_empty(),
            "{} cites no blueprint module",
            property.id()
        );
    }
}

/// A field the reader does not know is a field the digest cannot cover.
///
/// A registry report seals itself by re-serialising the parsed struct and hashing that, so
/// anything a reader silently discards is outside the seal by construction: the recomputation
/// cannot see it, the claimed digest still agrees, and a report carrying content nobody hashed
/// reads as intact. This report is the artefact that says which claims the reference examples
/// establish, so an invisible field in it is the difference between "these slices ran" and
/// "somebody wrote a slice into the file after it was sealed".
#[test]
fn a_registry_report_carrying_an_undeclared_field_is_refused_rather_than_silently_dropped() {
    let report = registry().run_all().expect("the standard registry runs");
    assert!(report.digest_is_intact());
    let sealed = serde_json::to_value(&report).expect("the report serialises");

    for pointer in ["", "/slices/0", "/coverage"] {
        let mut tampered = sealed.clone();
        tampered
            .pointer_mut(pointer)
            .expect("the position exists")
            .as_object_mut()
            .expect("the position is an object")
            .insert("injected".into(), serde_json::json!("added after sealing"));
        assert!(
            serde_json::from_value::<bioprism_examples::RegistryReport>(tampered).is_err(),
            "a key injected at {pointer:?} was read back into a report whose digest still verifies, \
             so the digest names less than the document does"
        );
    }
}
