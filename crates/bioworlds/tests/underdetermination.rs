//! What "underdetermined" means structurally, and what it does not.
//!
//! The interesting condition is not "evidence is missing" — that is a retrieval failure, and every
//! incomplete world has it. It is that the evidence is *complete and readable* and the verdict is
//! still open. These tests separate the two, and the resolved control shows the count moves with
//! the evidence set rather than with the factor graph.
//!
//! Nothing here constructs a verdict or an `OracleVerdict::abstain`; `bioprism-fiber` is not in
//! this crate's dependency set. What is asserted is the shape of the input such a path would take.

use bioprism_bioworlds::underdetermined::{
    analyse, build, evidence_variable, hypothesis_variable, query, target_depends_on_every_hypothesis,
    AbstentionStep, PostTreatmentSpec, DISCRIMINATING, HYPOTHESES, TARGET,
};
use bioprism_onco::response::DiscriminatingEvidence;

#[test]
fn three_mutually_exclusive_hypotheses_survive_the_available_evidence() {
    let spec = PostTreatmentSpec::underdetermined();
    let world = build(&spec).expect("builds");
    let found = analyse(&world, &query(&spec)).expect("analyse");

    assert_eq!(found.live_hypotheses.len(), 3);
    assert_eq!(found.settled_hypotheses.len(), 0);
    assert_eq!(found.exclusion_factors, vec!["factor.hypothesis_exclusion"]);
    assert!(found.is_underdetermined());
}

#[test]
fn no_hypothesis_support_input_is_unresolvable_so_the_world_is_not_merely_incomplete() {
    let spec = PostTreatmentSpec::underdetermined();
    let world = build(&spec).expect("builds");
    let found = analyse(&world, &query(&spec)).expect("analyse");
    assert!(
        found.unresolvable_support_inputs.is_empty(),
        "an input nothing provides would make this a retrieval failure, not an ambiguity: {:?}",
        found.unresolvable_support_inputs
    );
}

#[test]
fn no_hypothesis_support_input_is_withheld_so_the_ambiguity_is_not_temporal() {
    let spec = PostTreatmentSpec::underdetermined();
    let world = build(&spec).expect("builds");
    let found = analyse(&world, &query(&spec)).expect("analyse");
    assert!(
        found.support_inputs_withheld_at_cut.is_empty(),
        "a withheld support input would make this §38.08's property rather than §38.02's: {:?}",
        found.support_inputs_withheld_at_cut
    );
}

#[test]
fn every_discriminating_study_is_present_as_a_declared_absence_rather_than_omitted() {
    let spec = PostTreatmentSpec::underdetermined();
    let world = build(&spec).expect("builds");
    let found = analyse(&world, &query(&spec)).expect("analyse");

    let expected: Vec<String> = DISCRIMINATING
        .iter()
        .map(|evidence| evidence_variable(evidence).expect("nameable"))
        .collect();
    for variable in &expected {
        assert!(
            found.declared_unobserved_discriminating_evidence.contains(variable),
            "{variable} must be a fact recording its own absence, not a gap in the world"
        );
        assert!(
            world.world().fact_providing(variable).is_some(),
            "{variable} must actually be provided by a fact"
        );
    }
    assert!(found.observed_discriminating_evidence.is_empty());
}

#[test]
fn collecting_one_declared_absent_study_collapses_the_live_hypothesis_set_to_one() {
    let ambiguous = PostTreatmentSpec::underdetermined();
    let resolved = PostTreatmentSpec::resolved_control();

    let before = analyse(&build(&ambiguous).expect("builds"), &query(&ambiguous)).expect("analyse");
    let after = analyse(&build(&resolved).expect("builds"), &query(&resolved)).expect("analyse");

    assert_eq!(before.live_hypotheses.len(), 3);
    assert_eq!(after.live_hypotheses.len(), 1);
    assert!(after.is_determined());
    assert!(!after.is_underdetermined());
}

#[test]
fn the_two_post_treatment_worlds_differ_only_in_one_facts_value() {
    let ambiguous = build(&PostTreatmentSpec::underdetermined()).expect("builds");
    let resolved = build(&PostTreatmentSpec::resolved_control()).expect("builds");

    assert_eq!(ambiguous.world().factors.len(), resolved.world().factors.len());
    assert_eq!(ambiguous.world().events.len(), resolved.world().events.len());

    let differing: Vec<&str> = ambiguous
        .world()
        .facts
        .iter()
        .zip(resolved.world().facts.iter())
        .filter(|(left, right)| left.value != right.value)
        .map(|(left, _)| left.id.as_str())
        .collect();
    assert_eq!(differing, vec!["fact.perfusion_mri"]);
}

#[test]
fn the_factor_graphs_of_the_two_post_treatment_worlds_are_identical() {
    let ambiguous = build(&PostTreatmentSpec::underdetermined()).expect("builds");
    let resolved = build(&PostTreatmentSpec::resolved_control()).expect("builds");

    for (left, right) in ambiguous
        .world()
        .factors
        .iter()
        .zip(resolved.world().factors.iter())
    {
        assert_eq!(left.id, right.id);
        assert_eq!(left.inputs, right.inputs);
        assert_eq!(left.outputs, right.outputs);
        assert_eq!(left.kind, right.kind);
    }
}

#[test]
fn the_target_depends_on_every_hypothesis() {
    let spec = PostTreatmentSpec::underdetermined();
    let world = build(&spec).expect("builds");
    let found = analyse(&world, &query(&spec)).expect("analyse");
    assert!(target_depends_on_every_hypothesis(&world, &found, TARGET));
    assert_eq!(found.hypothesis_variables.len(), HYPOTHESES.len());
}

#[test]
fn a_world_with_one_hypothesis_settled_is_not_reported_as_underdetermined() {
    let mut spec = PostTreatmentSpec::underdetermined();
    spec.collected = vec![
        DiscriminatingEvidence::PerfusionMri,
        DiscriminatingEvidence::Histopathology,
    ];
    let world = build(&spec).expect("builds");
    let found = analyse(&world, &query(&spec)).expect("analyse");
    assert_eq!(found.live_hypotheses.len(), 0);
    assert!(!found.is_underdetermined());
}

#[test]
fn every_abstention_step_needs_a_compiler_change_not_a_world_change() {
    assert_eq!(AbstentionStep::ALL.len(), 6);
    for step in AbstentionStep::ALL {
        assert!(
            step.requires_a_compiler_change(),
            "{step:?} is claimed to be closable by building a world, which would be a false \
             statement about what this crate delivered"
        );
        assert!(!step.describe().is_empty());
    }
}

#[test]
fn the_underdetermined_world_carries_no_leakage_defect_so_the_v0_1_oracle_would_call_it_valid() {
    let spec = PostTreatmentSpec::underdetermined();
    let world = build(&spec).expect("builds");
    let leakage_shaped: Vec<&str> = world
        .world()
        .factors
        .iter()
        .filter(|factor| factor.outputs.iter().any(|o| o.as_str().ends_with("_leakage")))
        .map(|factor| factor.id.as_str())
        .collect();
    assert!(
        leakage_shaped.is_empty(),
        "this world's ambiguity must not be reachable through the split-integrity witnesses, or \
         an abstention path could be faked with a leakage witness: {leakage_shaped:?}"
    );
}

#[test]
fn the_hypothesis_support_factors_each_take_at_least_one_discriminating_input() {
    let spec = PostTreatmentSpec::underdetermined();
    let world = build(&spec).expect("builds");
    let discriminating: Vec<String> = DISCRIMINATING
        .iter()
        .map(|evidence| evidence_variable(evidence).expect("nameable"))
        .collect();

    for factor in world
        .world()
        .factors
        .iter()
        .filter(|factor| factor.kind == "hypothesis_support_rule")
    {
        assert!(
            factor
                .inputs
                .iter()
                .any(|input| discriminating.contains(&input.as_str().to_string())),
            "{} takes no discriminating input, so nothing could ever settle it",
            factor.id
        );
    }
}

#[test]
fn the_exclusion_factor_takes_every_hypothesis_as_an_input() {
    let spec = PostTreatmentSpec::underdetermined();
    let world = build(&spec).expect("builds");
    let exclusion = world
        .world()
        .factors
        .iter()
        .find(|factor| factor.kind == "mutual_exclusion_rule")
        .expect("an exclusion factor exists");

    for hypothesis in HYPOTHESES {
        let variable = hypothesis_variable(&hypothesis).expect("nameable");
        assert!(
            exclusion.inputs.iter().any(|i| i.as_str() == variable),
            "{variable} is outside the declared exclusion, so several could hold at once"
        );
    }
}
