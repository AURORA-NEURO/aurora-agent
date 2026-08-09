//! 27.01–27.04. Freezing a parent, importing an observed world, grafting, and simulating.

use bioprism_scope::Timestamp;
use bioprism_worldfactory::authoring::{
    freeze, CandidateParent, DecisionPoint, RequiredArtifact, ReviewRecord, Reviewed, Tier,
};
use bioprism_worldfactory::coverage::{coverage, owned_here, unclaimed, Owner};
use bioprism_worldfactory::error::{
    FreezeRefusal, GraftRefusal, ObservedRefusal, SimulatorRefusal,
};
use bioprism_worldfactory::mechanistic::{
    cite_for_result, declare_simulator, in_envelope, uncalibrated_parameters, CalibratedInterval,
};
use bioprism_worldfactory::observed::{declare, Access, SourceRef, Stratum, StudyDesign};
use bioprism_worldfactory::provenance::{Rung, Selection};
use bioprism_worldfactory::semisynthetic::{
    apply, parse_fact, shortcut_scan, Graft, Origin, SemiSyntheticWorld,
};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

fn at(text: &str) -> Timestamp {
    Timestamp::parse(text).expect("fixture timestamps are well formed")
}

fn consecutive() -> Selection {
    Selection::Consecutive {
        criterion: "every case meeting the entry criterion, in order".to_string(),
    }
}

fn observed_world() -> bioprism_worldfactory::observed::ObservedWorld {
    declare(
        "cohort-2026",
        vec![SourceRef::new("public-archive", "v3")],
        StudyDesign::new(100, consecutive())
            .with_stratum(Stratum::new("group-a", 60))
            .with_stratum(Stratum::new("group-b", 40))
            .not_identifying("what would have happened under the other arm"),
        BTreeSet::from(["observed outcome".to_string()]),
    )
    .expect("the declaration reconciles")
}

fn passing_reviews() -> ReviewRecord {
    ReviewRecord {
        plausibility: Reviewed::Passed {
            reviewer: "panel-1".to_string(),
        },
        exploit: Reviewed::Passed {
            reviewer: "panel-2".to_string(),
        },
        licence: Reviewed::Passed {
            reviewer: "counsel".to_string(),
        },
        clean_rebuild: true,
    }
}

fn gold_candidate() -> CandidateParent {
    CandidateParent::new("parent-1")
        .with_all_artifacts()
        .reviewed(passing_reviews())
        .with_decision(
            DecisionPoint::new("choose-analysis", at("2026-05-01T00:00:00Z"))
                .allowing("path-a")
                .allowing("path-b")
                .seeing("baseline-report"),
        )
        .available("baseline-report", at("2026-04-01T00:00:00Z"))
}

#[test]
fn a_world_whose_every_decision_admits_one_action_scores_agreement_with_its_author() {
    let single_path = CandidateParent::new("parent-1")
        .with_all_artifacts()
        .reviewed(passing_reviews())
        .with_decision(DecisionPoint::new("d1", at("2026-05-01T00:00:00Z")).allowing("only-path"))
        .with_decision(DecisionPoint::new("d2", at("2026-05-01T00:00:00Z")).allowing("only-path"));
    assert!(matches!(
        freeze(&single_path, Tier::Gold).expect_err("no alternative valid path anywhere"),
        FreezeRefusal::SinglePathAuthoring { points: 2 }
    ));
}

#[test]
fn evidence_that_became_available_after_the_decision_is_a_leak_however_genuine_the_value_is() {
    let leaking = gold_candidate().available("baseline-report", at("2026-06-01T00:00:00Z"));
    assert!(matches!(
        freeze(&leaking, Tier::Gold).expect_err("the decider could not have had it"),
        FreezeRefusal::FutureInformation { .. }
    ));
}

#[test]
fn a_parent_that_cannot_be_rebuilt_is_refused_before_anything_else_is_examined() {
    let mut reviews = passing_reviews();
    reviews.clean_rebuild = false;
    let unrebuildable = CandidateParent::new("parent-1").reviewed(reviews);
    assert!(matches!(
        freeze(&unrebuildable, Tier::Gold).expect_err("nothing else matters about a one-off"),
        FreezeRefusal::NoCleanRebuild
    ));
}

#[test]
fn a_review_that_never_ran_is_not_a_review_that_passed() {
    let mut reviews = passing_reviews();
    reviews.exploit = Reviewed::NotPerformed;
    let unreviewed = gold_candidate().reviewed(reviews);

    assert!(matches!(
        freeze(&unreviewed, Tier::Gold).expect_err("Gold requires every review"),
        FreezeRefusal::ReviewNotPerformed { .. }
    ));

    let silver = freeze(
        &unreviewed,
        Tier::Silver {
            relaxing: BTreeSet::new(),
            without_reviews: BTreeSet::from(["exploit".to_string()]),
        },
    )
    .expect("Silver freezes the gap after the author states it");
    assert!(silver.waived_reviews().contains("exploit"));
}

#[test]
fn a_review_that_ran_and_failed_is_refused_at_every_tier() {
    let mut reviews = passing_reviews();
    reviews.plausibility = Reviewed::Failed {
        finding: "the lesion trajectory is not one a tumour produces".to_string(),
    };
    let defective = gold_candidate().reviewed(reviews);
    for tier in [
        Tier::Gold,
        Tier::Silver {
            relaxing: BTreeSet::new(),
            without_reviews: BTreeSet::from(["biological plausibility".to_string()]),
        },
    ] {
        assert!(matches!(
            freeze(&defective, tier).expect_err("a known defect is not a waivable gap"),
            FreezeRefusal::ReviewFailed { .. }
        ));
    }
}

#[test]
fn silver_publishes_exactly_the_artifacts_it_was_frozen_without() {
    let missing_traces = CandidateParent::new("parent-1")
        .reviewed(passing_reviews())
        .with_decision(
            DecisionPoint::new("d1", at("2026-05-01T00:00:00Z"))
                .allowing("a")
                .allowing("b"),
        );
    let mut candidate = missing_traces;
    for artifact in RequiredArtifact::ALL {
        if artifact != RequiredArtifact::ReferenceTraces {
            candidate = candidate.with_artifact(artifact);
        }
    }

    assert!(matches!(
        freeze(&candidate, Tier::Gold).expect_err("Gold needs all seven"),
        FreezeRefusal::MissingArtifact { .. }
    ));

    let silver = freeze(
        &candidate,
        Tier::Silver {
            relaxing: BTreeSet::from([RequiredArtifact::ReferenceTraces]),
            without_reviews: BTreeSet::new(),
        },
    )
    .expect("a declared relaxation is a stated limitation");
    assert_eq!(silver.relaxations().len(), 1);
    assert_eq!(silver.multi_path_points(), 1);
}

#[test]
fn a_parent_that_bundles_a_controlled_asset_may_not_be_frozen() {
    let redistributing = gold_candidate().from_source(
        SourceRef::new("restricted-cohort", "v1")
            .under(Access::Controlled {
                policy: "institutional agreement".to_string(),
            })
            .embedded(),
    );
    assert!(matches!(
        freeze(&redistributing, Tier::Gold).expect_err("controlled data stay controlled"),
        FreezeRefusal::ControlledAssetEmbedded { .. }
    ));
}

#[test]
fn a_cohort_whose_strata_do_not_sum_to_its_size_has_one_number_that_is_wrong() {
    let refusal = declare(
        "cohort",
        vec![SourceRef::new("archive", "v1")],
        StudyDesign::new(100, consecutive()).with_stratum(Stratum::new("only-group", 90)),
        BTreeSet::new(),
    )
    .expect_err("90 is not 100");
    assert!(matches!(
        refusal,
        ObservedRefusal::CohortCountUnreconciled {
            declared: 100,
            strata_total: 90
        }
    ));
}

#[test]
fn an_unpinned_source_changes_underneath_every_result_computed_against_it() {
    let refusal = declare(
        "cohort",
        vec![SourceRef::unpinned("moving-target")],
        StudyDesign::new(10, consecutive()),
        BTreeSet::new(),
    )
    .expect_err("a version is not optional");
    assert!(matches!(refusal, ObservedRefusal::UnpinnedSource { .. }));
}

#[test]
fn a_world_may_only_stand_for_a_population_once_its_selection_is_declared() {
    let refusal = declare(
        "cohort",
        vec![SourceRef::new("archive", "v1")],
        StudyDesign::new(10, Selection::Undeclared).standing_for("all cases"),
        BTreeSet::new(),
    )
    .expect_err("an undeclared procedure represents nothing");
    assert!(matches!(refusal, ObservedRefusal::UndeclaredSelection));

    declare(
        "cohort",
        vec![SourceRef::new("archive", "v1")],
        StudyDesign::new(10, Selection::Undeclared),
        BTreeSet::new(),
    )
    .expect("a world that claims no population is fine evidence about itself");
}

#[test]
fn a_world_cannot_forget_which_of_its_facts_were_synthetic() {
    let parent = observed_world();
    let mut world = SemiSyntheticWorld::from_observed(
        &parent,
        BTreeMap::from([
            ("prevalence".to_string(), json!(30)),
            ("site".to_string(), json!("site-1")),
        ]),
    );
    let graft = Graft::new("g-shift")
        .targeting("prevalence")
        .editing("prevalence", json!(5))
        .injecting("prevalence");
    apply(&mut world, &graft).expect("a declared, in-target edit");

    assert!(world.origin_of("prevalence").expect("present").is_injected());
    assert!(!world.origin_of("site").expect("present").is_injected());

    let round_tripped: serde_json::Value =
        serde_json::from_str(&serde_json::to_string(&world).expect("worlds serialise"))
            .expect("and parse back");
    let origin = &round_tripped["facts"]["prevalence"]["origin"];
    assert_eq!(origin["origin"], "injected", "the label survives packaging");
}

#[test]
fn a_fact_deserialised_without_a_declared_origin_is_refused_rather_than_assumed_observed() {
    let orphan = json!({ "key": "prevalence", "value": 5 });
    assert!(matches!(
        parse_fact(&orphan).expect_err("no default origin exists"),
        GraftRefusal::OriginNotDeclared { fact } if fact == "prevalence"
    ));

    let declared = json!({
        "key": "prevalence",
        "value": 5,
        "origin": { "origin": "observed", "source": "cohort-2026" }
    });
    let fact = parse_fact(&declared).expect("a declared origin parses");
    assert!(matches!(fact.origin, Origin::Observed { .. }));
}

#[test]
fn a_graft_that_edits_outside_its_declared_target_set_has_an_undocumented_blast_radius() {
    let parent = observed_world();
    let mut world = SemiSyntheticWorld::from_observed(
        &parent,
        BTreeMap::from([("prevalence".to_string(), json!(30))]),
    );
    let sloppy = Graft::new("g-sloppy").editing("prevalence", json!(5));
    assert!(matches!(
        apply(&mut world, &sloppy).expect_err("the edit was never declared"),
        GraftRefusal::OutsideTargetSet { .. }
    ));
    assert!(
        !world.origin_of("prevalence").expect("present").is_injected(),
        "a refused graft must not leave a half-applied world behind"
    );
}

#[test]
fn a_graft_leaves_everything_outside_its_target_set_byte_identical() {
    let parent = observed_world();
    let mut world = SemiSyntheticWorld::from_observed(
        &parent,
        BTreeMap::from([
            ("prevalence".to_string(), json!(30)),
            ("site".to_string(), json!("site-1")),
            ("platform".to_string(), json!("p-1")),
        ]),
    );
    let graft = Graft::new("g-shift")
        .targeting("prevalence")
        .targeting("site")
        .editing("prevalence", json!(5));
    let report = apply(&mut world, &graft).expect("in-target");
    assert!(report.respected_target_set());
    assert_eq!(
        report.declared_but_untouched,
        BTreeSet::from(["site".to_string()]),
        "a declared target the transformation never reached is worth showing the author"
    );
}

#[test]
fn a_graft_onto_a_fact_that_was_itself_injected_has_no_observed_structure_under_it() {
    let parent = observed_world();
    let mut world = SemiSyntheticWorld::from_observed(
        &parent,
        BTreeMap::from([("prevalence".to_string(), json!(30))]),
    );
    let first = Graft::new("g-1")
        .targeting("prevalence")
        .editing("prevalence", json!(5));
    apply(&mut world, &first).expect("first graft is fine");

    let second = Graft::new("g-2")
        .targeting("prevalence")
        .editing("prevalence", json!(1));
    assert!(matches!(
        apply(&mut world, &second).expect_err("grafting onto a graft"),
        GraftRefusal::TargetIsItselfInjected { .. }
    ));
}

#[test]
fn a_graft_whose_whole_footprint_is_the_fact_the_oracle_asks_about_is_a_lookup() {
    let parent = observed_world();
    let mut world = SemiSyntheticWorld::from_observed(
        &parent,
        BTreeMap::from([
            ("prevalence".to_string(), json!(30)),
            ("site".to_string(), json!("site-1")),
        ]),
    );
    let graft = Graft::new("g-tell")
        .targeting("prevalence")
        .editing("prevalence", json!(5));
    let report = apply(&mut world, &graft).expect("in-target");

    assert!(matches!(
        shortcut_scan(&report, "prevalence").expect_err("one fact changed, and it is the answer"),
        GraftRefusal::SingleFactTell { .. }
    ));
    shortcut_scan(&report, "site").expect("a one-fact graft is legal until the oracle points at it");
}

#[test]
fn a_published_card_reports_the_rung_the_world_actually_stands_on() {
    let parent = observed_world();
    let mut world = SemiSyntheticWorld::from_observed(
        &parent,
        BTreeMap::from([
            ("prevalence".to_string(), json!(30)),
            ("site".to_string(), json!("site-1")),
        ]),
    );
    let graft = Graft::new("g-shift")
        .targeting("prevalence")
        .editing("prevalence", json!(5))
        .injecting("prevalence");
    apply(&mut world, &graft).expect("in-target");

    let card = world.card();
    assert_eq!(card.furthest_from_observation, Rung::SemiSynthetic);
    assert_eq!(card.injected_facts, 1);
    assert_eq!(card.observed_facts, 1);
    assert!(world.provenance().assumptions().contains("prevalence"));
}

#[test]
fn grafting_onto_a_simulation_does_not_launder_the_simulation() {
    let simulator = declare_simulator(
        "sim",
        ["growth rate"],
        at("2026-01-01T00:00:00Z"),
        BTreeMap::new(),
    )
    .expect("assumptions declared");
    let mut world = SemiSyntheticWorld::from_parent(
        "sim-world",
        simulator.provenance(),
        BTreeMap::from([("prevalence".to_string(), json!(30))]),
    );
    let graft = Graft::new("g-shift")
        .targeting("prevalence")
        .editing("prevalence", json!(5))
        .injecting("prevalence");
    apply(&mut world, &graft).expect("in-target");

    let card = world.card();
    assert_eq!(
        card.furthest_from_observation,
        Rung::Mechanistic,
        "a downstream construction step never moves a world closer to measurement"
    );
    assert!(
        world.provenance().assumptions().contains("growth rate"),
        "the simulator's assumptions travel through the graft"
    );
}

#[test]
fn a_simulator_that_declares_no_assumptions_declares_no_limits() {
    let empty: [&str; 0] = [];
    assert!(matches!(
        declare_simulator("sim", empty, at("2026-01-01T00:00:00Z"), BTreeMap::new())
            .expect_err("every model assumes something"),
        SimulatorRefusal::NoDeclaredAssumptions { .. }
    ));
}

#[test]
fn a_simulator_calibrated_after_the_result_it_is_cited_for_was_fitted_to_it() {
    let simulator = declare_simulator(
        "sim",
        ["growth rate"],
        at("2026-06-01T00:00:00Z"),
        BTreeMap::new(),
    )
    .expect("assumptions declared");

    assert!(matches!(
        cite_for_result(&simulator, at("2026-03-01T00:00:00Z"))
            .expect_err("the calibration postdates the result"),
        SimulatorRefusal::CalibratedAfterResult { .. }
    ));
    cite_for_result(&simulator, at("2026-09-01T00:00:00Z")).expect("this ordering is fine");
}

#[test]
fn a_parameter_outside_the_calibrated_envelope_is_extrapolation_and_an_undeclared_one_is_a_gap() {
    let simulator = declare_simulator(
        "sim",
        ["growth rate"],
        at("2026-01-01T00:00:00Z"),
        BTreeMap::from([("diffusion".to_string(), CalibratedInterval::new(10, 90))]),
    )
    .expect("assumptions declared");

    let outside = BTreeMap::from([("diffusion".to_string(), 500)]);
    assert!(matches!(
        in_envelope(&simulator, &outside).expect_err("outside the calibrated interval"),
        SimulatorRefusal::OutOfCalibration { .. }
    ));

    let unlisted = BTreeMap::from([("porosity".to_string(), 500)]);
    in_envelope(&simulator, &unlisted)
        .expect("silence in the model-limit card is not the same as a failed check");
    assert_eq!(
        uncalibrated_parameters(&simulator, &unlisted),
        BTreeSet::from(["porosity".to_string()]),
        "the gap is reported rather than swallowed"
    );
}

#[test]
fn every_section_27_module_has_a_recorded_owner_or_a_stated_reason_for_having_none() {
    let table = coverage();
    assert_eq!(table.len(), 22);
    assert_eq!(
        owned_here().len(),
        7,
        "this crate claims exactly the seven modules its docs name"
    );
    for module in unclaimed() {
        match module.owner {
            Owner::Unclaimed { because } => assert!(
                because.len() > 20,
                "an unclaimed module must say why, not merely that"
            ),
            _ => unreachable!("filtered to unclaimed"),
        }
    }
    let ids: Vec<&str> = owned_here().iter().map(|m| m.id).collect();
    assert_eq!(
        ids,
        vec!["27.01", "27.02", "27.03", "27.04", "27.10", "27.11", "27.14"]
    );
}
