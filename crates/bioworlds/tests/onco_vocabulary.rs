//! The post-treatment world speaks `bioprism-onco`'s vocabulary, not an invented one.
//!
//! §38's worlds must not present invented clinical detail as domain knowledge. The variable names
//! in [`bioprism_bioworlds::underdetermined`] are therefore *derived* from onco's serde
//! representation rather than retyped, and these tests pin the correspondence: a rename in onco
//! breaks them, which is the point. What remains illustrative — measurement values, the 44-day
//! interval, the criterion identifier — is labelled as such in the module documentation.

use bioprism_bioworlds::underdetermined::{
    build, evidence_variable, hypothesis_variable, vocabulary_name, PostTreatmentSpec,
    DISCRIMINATING, HYPOTHESES,
};
use bioprism_bioworlds::BioWorldError;
use bioprism_onco::response::{ChangeHypothesis, DiscriminatingEvidence, TreatmentModality};
use bioprism_onco::status::ObservationStatus;

#[test]
fn every_hypothesis_variable_is_named_by_an_onco_change_hypothesis() {
    let world = build(&PostTreatmentSpec::underdetermined()).expect("builds");
    let expected: Vec<String> = HYPOTHESES
        .iter()
        .map(|h| hypothesis_variable(h).expect("nameable"))
        .collect();

    let produced: Vec<String> = world
        .world()
        .factors
        .iter()
        .filter(|factor| factor.kind == "hypothesis_support_rule")
        .flat_map(|factor| factor.outputs.iter().map(|o| o.as_str().to_string()))
        .collect();

    for name in &expected {
        assert!(produced.contains(name), "{name} is not produced by any support factor");
    }
    assert_eq!(produced.len(), expected.len());
}

#[test]
fn the_hypothesis_variable_names_are_onco_serde_names_and_not_retyped_strings() {
    assert_eq!(
        hypothesis_variable(&ChangeHypothesis::Progression).expect("nameable"),
        "hypothesis_progression_support"
    );
    assert_eq!(
        hypothesis_variable(&ChangeHypothesis::TreatmentEffect).expect("nameable"),
        "hypothesis_treatment_effect_support"
    );
    assert_eq!(
        hypothesis_variable(&ChangeHypothesis::MixedProcess).expect("nameable"),
        "hypothesis_mixed_process_support"
    );
}

#[test]
fn every_discriminating_variable_is_named_by_an_onco_discriminating_evidence() {
    let world = build(&PostTreatmentSpec::underdetermined()).expect("builds");
    for evidence in DISCRIMINATING {
        let variable = evidence_variable(&evidence).expect("nameable");
        assert!(
            world.world().fact_providing(&variable).is_some(),
            "{variable} has no fact, so the world does not actually speak this vocabulary"
        );
    }
    assert_eq!(
        evidence_variable(&DiscriminatingEvidence::AminoAcidPet).expect("nameable"),
        "amino_acid_pet"
    );
}

#[test]
fn a_data_carrying_vocabulary_variant_is_refused_rather_than_stringified() {
    let other = ChangeHypothesis::Other("something_a_caller_invented".into());
    let refused = vocabulary_name(&other, "ChangeHypothesis");
    assert!(matches!(
        refused,
        Err(BioWorldError::VocabularyNotNameable { .. })
    ));
}

#[test]
fn the_post_treatment_window_is_the_value_onco_derives_for_chemoradiotherapy() {
    let world = build(&PostTreatmentSpec::underdetermined()).expect("builds");
    let fact = world
        .world()
        .fact_providing("post_treatment_window_days")
        .expect("the window fact exists");
    assert_eq!(
        fact.value.as_i64(),
        Some(TreatmentModality::ChemoRadiotherapy.post_treatment_window_days()),
        "the one non-illustrative number in this world must come from onco, not from here"
    );
}

#[test]
fn a_declared_absence_carries_onco_s_not_collected_status_verbatim() {
    let world = build(&PostTreatmentSpec::underdetermined()).expect("builds");
    let expected = vocabulary_name(&ObservationStatus::NotCollected, "ObservationStatus")
        .expect("nameable");
    let fact = world
        .world()
        .fact_providing(&evidence_variable(&DiscriminatingEvidence::PerfusionMri).expect("nameable"))
        .expect("the perfusion fact exists");

    assert_eq!(fact.value.get("observed").and_then(|v| v.as_bool()), Some(false));
    assert_eq!(
        fact.value.get("status").and_then(|v| v.as_str()),
        Some(expected.as_str()),
        "the absence must be typed by onco's six-state enumeration, not by an ad-hoc string"
    );
}

#[test]
fn the_collected_study_records_an_observation_rather_than_an_absence_status() {
    let world = build(&PostTreatmentSpec::resolved_control()).expect("builds");
    let fact = world
        .world()
        .fact_providing(&evidence_variable(&DiscriminatingEvidence::PerfusionMri).expect("nameable"))
        .expect("the perfusion fact exists");
    assert_eq!(fact.value.get("observed").and_then(|v| v.as_bool()), Some(true));
    assert!(
        fact.value.get("status").is_none(),
        "an observed study must not also carry an absence status"
    );
}

#[test]
fn the_treatment_modality_recorded_in_the_world_is_an_onco_serde_name() {
    let world = build(&PostTreatmentSpec::underdetermined()).expect("builds");
    let expected =
        vocabulary_name(&TreatmentModality::ChemoRadiotherapy, "TreatmentModality").expect("nameable");
    let fact = world
        .world()
        .fact_providing("treatment_timeline")
        .expect("the timeline fact exists");
    let first = fact
        .value
        .as_object()
        .and_then(|map| map.values().next())
        .expect("a per-subject entry");
    assert_eq!(
        first.get("modality").and_then(|v| v.as_str()),
        Some(expected.as_str())
    );
}
