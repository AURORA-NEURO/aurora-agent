//! 27.01–27.04. The authoring ladder: what each rung can and cannot be evidence for.

use bioprism_worldfactory::error::ClaimRefusal;
use bioprism_worldfactory::provenance::{
    support, Claim, ClaimKind, Provenance, Rung, Selection,
};
use std::collections::BTreeSet;

fn consecutive() -> Selection {
    Selection::Consecutive {
        criterion: "every case meeting the entry criterion, in order".to_string(),
    }
}

#[test]
fn a_simulation_cannot_be_evidence_for_what_the_simulator_assumed() {
    let simulated = Provenance::mechanistic(["growth rate", "diffusion coefficient"]);
    let refusal = support(
        &simulated,
        Claim::new(ClaimKind::SimulatorBehaviour, "growth rate"),
    )
    .expect_err("the model wrote the growth rate down");
    assert!(matches!(
        refusal,
        ClaimRefusal::AssumedByConstruction { quantity, .. } if quantity == "growth rate"
    ));
}

#[test]
fn circularity_is_reported_before_the_rung_so_the_author_fixes_the_right_thing() {
    let simulated = Provenance::mechanistic(["growth rate"]);
    let refusal = support(&simulated, Claim::new(ClaimKind::Biology, "growth rate"))
        .expect_err("both checks would fire; the sharper one must win");
    assert!(
        matches!(refusal, ClaimRefusal::AssumedByConstruction { .. }),
        "an author told only that the rung is wrong would move the claim to another world and \
         reproduce the circularity there"
    );
}

#[test]
fn detecting_an_injected_effect_is_the_one_claim_an_assumption_supports() {
    let simulated = Provenance::mechanistic(["growth rate"]);
    let grounded = support(
        &simulated,
        Claim::new(ClaimKind::DetectingInjectedStructure, "growth rate"),
    )
    .expect("the injected effect is the ground truth for a detection claim");
    assert_eq!(grounded.claim().quantity, "growth rate");
}

#[test]
fn a_simulated_world_cannot_make_a_claim_about_biology_at_all() {
    let simulated = Provenance::mechanistic(["growth rate"]);
    let refusal = support(&simulated, Claim::new(ClaimKind::Biology, "response rate"))
        .expect_err("everything in it was written down by a modeller");
    assert!(matches!(refusal, ClaimRefusal::ExceedsRung { .. }));
}

#[test]
fn an_observed_world_cannot_make_a_claim_about_detecting_injected_structure() {
    let observed = Provenance::observed(consecutive());
    let refusal = support(
        &observed,
        Claim::new(ClaimKind::DetectingInjectedStructure, "batch offset"),
    )
    .expect_err("there is nothing injected to have found");
    assert!(
        matches!(refusal, ClaimRefusal::ExceedsRung { .. }),
        "the ladder is not one-directional; observation is not simply the strongest rung"
    );
}

#[test]
fn every_rung_supports_a_claim_about_the_world_as_built() {
    let rungs = [
        Provenance::observed(consecutive()),
        Provenance::semi_synthetic(&Provenance::observed(consecutive()), ["injected shift"]),
        Provenance::mechanistic(["growth rate"]),
    ];
    for provenance in &rungs {
        support(
            provenance,
            Claim::new(ClaimKind::TheWorldAsBuilt, "pipeline reproducibility"),
        )
        .expect("reproducibility is a fact about the artefact, whatever built it");
    }
}

#[test]
fn a_graft_onto_a_simulation_stands_on_both_rungs_and_inherits_its_assumptions() {
    let simulated = Provenance::mechanistic(["growth rate"]);
    let grafted = Provenance::semi_synthetic(&simulated, ["batch offset"]);

    assert_eq!(
        grafted.stands_on(),
        &BTreeSet::from([Rung::Mechanistic, Rung::SemiSynthetic])
    );
    assert_eq!(grafted.furthest_from_observation(), Rung::Mechanistic);
    assert!(grafted.assumptions().contains("growth rate"));
    assert!(grafted.assumptions().contains("batch offset"));
}

#[test]
fn a_graft_onto_observed_data_cannot_answer_a_question_about_the_simulator_it_never_used() {
    let observed_graft =
        Provenance::semi_synthetic(&Provenance::observed(consecutive()), ["batch offset"]);
    assert!(support(
        &observed_graft,
        Claim::new(ClaimKind::SimulatorBehaviour, "parameter recovery")
    )
    .is_err());

    let simulated_graft =
        Provenance::semi_synthetic(&Provenance::mechanistic(["growth rate"]), ["batch offset"]);
    support(
        &simulated_graft,
        Claim::new(ClaimKind::SimulatorBehaviour, "parameter recovery"),
    )
    .expect("the mechanistic rung is in the ancestry, so the question is answerable");
}

#[test]
fn a_semi_synthetic_world_cannot_make_a_biological_claim_however_real_its_parent_was() {
    let grafted =
        Provenance::semi_synthetic(&Provenance::observed(consecutive()), ["batch offset"]);
    let refusal = support(&grafted, Claim::new(ClaimKind::Biology, "marker prevalence"))
        .expect_err("a result on a grafted world cannot be attributed to the observed part");
    assert!(matches!(refusal, ClaimRefusal::ExceedsRung { .. }));
}

#[test]
fn a_counterfactual_the_study_design_does_not_identify_is_refused_by_name() {
    let observed = Provenance::observed(consecutive())
        .declaring_unsupported(["what would have happened under the other arm"]);
    let claim = Claim::new(ClaimKind::Biology, "treatment effect")
        .resting_on_counterfactual("what would have happened under the other arm");
    assert!(matches!(
        support(&observed, claim).expect_err("declared unsupported"),
        ClaimRefusal::CounterfactualNotIdentified { .. }
    ));
}

#[test]
fn only_consecutive_enrolment_supports_a_claim_about_a_population() {
    let population_claim =
        || Claim::new(ClaimKind::Biology, "marker prevalence").about_population("all cases");

    support(&Provenance::observed(consecutive()), population_claim())
        .expect("consecutive enrolment is the one selection that generalises");

    for selection in [
        Selection::Convenience {
            because: "these were the blocks still in the freezer".to_string(),
        },
        Selection::Enriched {
            for_what: "high-grade cases".to_string(),
        },
        Selection::Undeclared,
    ] {
        assert!(matches!(
            support(&Provenance::observed(selection), population_claim())
                .expect_err("a selected cohort stands for itself"),
            ClaimRefusal::SelectedCohort { .. }
        ));
    }
}

#[test]
fn a_selected_cohort_is_still_perfectly_good_evidence_about_itself() {
    let enriched = Provenance::observed(Selection::Enriched {
        for_what: "high-grade cases".to_string(),
    });
    support(&enriched, Claim::new(ClaimKind::Biology, "marker prevalence"))
        .expect("a claim that names no population is a claim about the cohort");
}

#[test]
fn a_grounded_claim_carries_the_sentence_its_methods_section_must_contain() {
    let simulated = support(
        &Provenance::mechanistic(["growth rate"]),
        Claim::new(ClaimKind::SimulatorBehaviour, "parameter recovery"),
    )
    .expect("in scope for a simulator");
    assert!(simulated.caveat().contains("exact inside the model"));

    let grafted = support(
        &Provenance::semi_synthetic(&Provenance::observed(consecutive()), ["batch offset"]),
        Claim::new(ClaimKind::DetectingInjectedStructure, "batch offset"),
    )
    .expect("detection is what a graft is for");
    assert!(grafted.caveat().contains("was injected"));
}
