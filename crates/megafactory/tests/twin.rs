//! Invariants of the mechanistic simulation and assay-twin factory, blueprint 35.04.
//!
//! Every rate below is a test fixture. None of them is a claim about biology, and the crate under
//! test contains no rate of its own.

use bioprism_megafactory::{
    counterfactual, AssayReadout, DiscrepancyProbe, Intervention, MechanisticModel, TwinError,
    TwinTruth,
};

fn compartments() -> Vec<String> {
    vec!["a".into(), "b".into(), "c".into()]
}

/// `a` drains into `c`; nothing refills `a`.
fn draining(id: &str, a_to_c: f64) -> MechanisticModel {
    MechanisticModel::new(
        id,
        compartments(),
        vec![
            vec![0.0, 0.0, a_to_c],
            vec![0.0, 0.0, 0.0],
            vec![0.0, 0.0, 0.0],
        ],
        "no saturation: transfer stays linear at every mass",
    )
    .expect("a stated misspecification and a square rate matrix")
}

/// `a` drains into `c` and is refilled from a large `b`, so `a` grows without an intervention.
fn refilled(id: &str) -> MechanisticModel {
    MechanisticModel::new(
        id,
        compartments(),
        vec![
            vec![0.0, 0.0, 0.3],
            vec![0.9, 0.0, 0.0],
            vec![0.0, 0.0, 0.0],
        ],
        "the refill compartment is treated as a source with no upstream",
    )
    .expect("a stated misspecification and a square rate matrix")
}

const INITIAL: [f64; 3] = [1.0, 5.0, 0.0];

#[test]
fn a_model_that_states_no_known_misspecification_is_refused() {
    let error = MechanisticModel::new("m", compartments(), vec![vec![0.0; 3]; 3], "   ")
        .expect_err("a blank misspecification must be refused");
    assert_eq!(error, TwinError::NoStatedMisspecification("m".into()));
}

#[test]
fn a_rate_matrix_of_the_wrong_shape_is_refused() {
    let error = MechanisticModel::new("m", compartments(), vec![vec![0.0; 3]; 2], "linear")
        .expect_err("a non-square matrix must be refused");
    assert!(matches!(error, TwinError::RateShape { .. }));
}

#[test]
fn a_non_finite_rate_is_refused() {
    let error = MechanisticModel::new(
        "m",
        compartments(),
        vec![vec![0.0, f64::NAN, 0.0], vec![0.0; 3], vec![0.0; 3]],
        "linear",
    )
    .expect_err("a non-finite rate must be refused");
    assert!(matches!(
        error,
        TwinError::NonFiniteRate { row: 0, col: 1, .. }
    ));
}

#[test]
fn a_model_with_no_compartments_is_refused() {
    let error = MechanisticModel::new("m", Vec::new(), Vec::new(), "linear")
        .expect_err("an empty state space must be refused");
    assert!(matches!(error, TwinError::NoCompartments { .. }));
}

#[test]
fn an_initial_state_of_the_wrong_length_is_refused() {
    let model = draining("m", 0.3);
    let error = model
        .run(&[1.0, 2.0], 3, None)
        .expect_err("a short initial state must be refused");
    assert!(matches!(
        error,
        TwinError::InitialStateShape { given: 2, .. }
    ));
}

#[test]
fn an_intervention_on_an_unknown_compartment_is_refused() {
    let model = draining("m", 0.3);
    let error = model
        .run(&INITIAL, 3, Some(&Intervention::hold("d", 1.0)))
        .expect_err("an unknown compartment must be refused");
    assert!(matches!(error, TwinError::UnknownCompartment { .. }));
}

#[test]
fn an_uninterrupted_run_conserves_total_mass() {
    let model = refilled("m");
    let trajectory = model.run(&INITIAL, 6, None).expect("a well formed run");
    let start: f64 = INITIAL.iter().sum();
    for state in &trajectory.states {
        let total: f64 = state.iter().sum();
        assert!(
            (total - start).abs() < 1e-9,
            "transfer moves mass and never creates it: {total} against {start}"
        );
    }
}

#[test]
fn an_intervention_holds_its_compartment_for_the_whole_run_rather_than_nudging_it_once() {
    let model = refilled("m");
    let trajectory = model
        .run(&INITIAL, 5, Some(&Intervention::hold("a", 1.0)))
        .expect("a well formed run");
    for state in &trajectory.states {
        assert_eq!(state[0], 1.0, "the held compartment never moves");
    }
    let free = model.run(&INITIAL, 5, None).expect("a well formed run");
    assert!(
        free.final_state()[0] > 1.0,
        "without the intervention this compartment grows, which is what makes the contrast causal"
    );
}

#[test]
fn a_counterfactual_is_the_same_number_every_time_it_is_computed() {
    let model = draining("m", 0.3);
    let intervention = Intervention::hold("a", 1.0);
    let first = counterfactual(&model, &INITIAL, 4, &intervention, "c").expect("computable");
    let second = counterfactual(&model, &INITIAL, 4, &intervention, "c").expect("computable");
    assert_eq!(first, second);
}

#[test]
fn a_counterfactual_carries_the_model_and_its_misspecification_and_nothing_claims_to_be_true() {
    let model = draining("m", 0.3);
    let truth = counterfactual(&model, &INITIAL, 4, &Intervention::hold("a", 1.0), "c")
        .expect("computable");
    assert_eq!(truth.model(), "m");
    let TwinTruth::UnderModel {
        known_misspecification,
        ..
    } = &truth;
    assert!(!known_misspecification.is_empty());

    let json = serde_json::to_string(&truth).expect("serialisable");
    assert!(json.contains(r#""twin_truth":"under_model""#), "{json}");
    assert!(
        truth.qualification().contains("no saturation"),
        "the qualification names what the model gets wrong: {}",
        truth.qualification()
    );
}

#[test]
fn a_probe_over_an_empty_alternative_set_is_refused() {
    let model = draining("m", 0.3);
    let error = DiscrepancyProbe::run(&model, &[], &INITIAL, 4, &Intervention::hold("a", 1.0), "c")
        .expect_err("a model compared against itself is not a discrepancy measurement");
    assert_eq!(error, TwinError::NoAlternatives);
}

#[test]
fn an_alternative_with_a_different_state_space_is_refused() {
    let reference = draining("reference", 0.3);
    let other = MechanisticModel::new(
        "other",
        vec!["a".into(), "b".into()],
        vec![vec![0.0, 0.5], vec![0.0, 0.0]],
        "two compartments only",
    )
    .expect("well formed");
    let error = DiscrepancyProbe::run(
        &reference,
        &[other],
        &INITIAL,
        4,
        &Intervention::hold("a", 1.0),
        "c",
    )
    .expect_err("two effects over different state spaces are not the same quantity");
    assert!(matches!(error, TwinError::IncomparableAlternative { .. }));
}

#[test]
fn a_sign_flip_across_the_plausible_set_disqualifies_the_counterfactual() {
    let reference = draining("reference", 0.3);
    let probe = DiscrepancyProbe::run(
        &reference,
        &[refilled("refilled")],
        &INITIAL,
        3,
        &Intervention::hold("a", 1.0),
        "c",
    )
    .expect("comparable models");

    assert!(
        probe.reference.effect() > 0.0,
        "the reference effect is positive"
    );
    assert_eq!(probe.alternatives.len(), 1);
    assert!(
        probe.alternatives[0].effect() < 0.0,
        "under the refilled model, holding the compartment down reduces the outcome"
    );
    assert!(!probe.sign_stable);
    assert!(!probe.is_usable_as_an_oracle());
    assert_eq!(probe.models_disagreeing, vec!["refilled".to_string()]);
    assert!(probe.headline().contains("not benchmark ground truth"));
}

#[test]
fn a_sign_stable_probe_still_reports_the_range_it_survived() {
    let reference = draining("reference", 0.3);
    let probe = DiscrepancyProbe::run(
        &reference,
        &[draining("faster", 0.5), draining("slower", 0.1)],
        &INITIAL,
        3,
        &Intervention::hold("a", 1.0),
        "c",
    )
    .expect("comparable models");

    assert!(probe.sign_stable);
    assert!(probe.is_usable_as_an_oracle());
    assert!(probe.models_disagreeing.is_empty());
    assert!(
        probe.effect_span > 0.0,
        "agreeing on direction is not agreeing on magnitude"
    );
    assert!(probe.effect_min <= probe.reference.effect());
    assert!(probe.effect_max >= probe.reference.effect());
}

#[test]
fn a_null_effect_does_not_silently_agree_with_both_directions() {
    let inert = draining("inert", 0.0);
    let probe = DiscrepancyProbe::run(
        &draining("reference", 0.3),
        &[inert],
        &INITIAL,
        3,
        &Intervention::hold("a", 1.0),
        "c",
    )
    .expect("comparable models");
    assert!(
        !probe.sign_stable,
        "a zero effect is its own sign class and must not be counted as agreement"
    );
}

#[test]
fn an_assay_readout_names_its_forward_model_and_the_model_it_read_from() {
    let model = draining("m", 0.3);
    let trajectory = model.run(&INITIAL, 3, None).expect("a well formed run");
    let readout = AssayReadout::from_trajectory(&trajectory, "c", "log-linear gain", |mass| mass)
        .expect("the compartment exists");
    assert_eq!(readout.model, "m");
    assert_eq!(readout.forward_model, "log-linear gain");
    assert_eq!(readout.value, trajectory.final_mass("c").expect("present"));

    assert!(
        AssayReadout::from_trajectory(&trajectory, "absent", "gain", |mass| mass).is_none(),
        "a readout of a compartment that is not there is None, not zero"
    );
}
