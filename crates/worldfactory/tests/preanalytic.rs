//! 27.10. A handling fault degrades what can be measured and leaves the biology alone.

use bioprism_worldfactory::error::PreanalyticRefusal;
use bioprism_worldfactory::preanalytic::{
    apply, check_response, detectability_floor, validate_family, Edit, ExpectedResponse, FaultKind,
    Intensity, PreanalyticMutation, Specimen, Stage, UPSTREAM_STATE_FIELD,
};
use serde_json::json;
use std::collections::BTreeSet;

fn specimen() -> Specimen {
    Specimen::new("S1")
        .with_biology("marker_state", json!("present"))
        .with_biology("grade", json!(3))
        .with_qc("rna_integrity", 9_000)
        .with_measurability("transcriptome", 10_000)
        .with_handling(Stage::Collection, "recorded_by", json!("theatre log"))
}

fn cold_ischaemia(intensity: Intensity) -> PreanalyticMutation {
    PreanalyticMutation::new(
        "ci-full",
        "cold-ischaemia",
        FaultKind::ColdIschaemia { minutes: 120 },
        intensity,
        ExpectedResponse::Detect,
    )
    .editing(Edit::Qc {
        field: "rna_integrity".to_string(),
        delta: -4_000,
    })
    .editing(Edit::Measurability {
        axis: "transcriptome".to_string(),
        loss: 6_000,
    })
}

#[test]
fn a_preanalytic_fault_leaves_the_biological_state_byte_identical() {
    let before = specimen();
    let faulted = apply(&before, &cold_ischaemia(Intensity::FULL)).expect("a legal handling fault");
    assert_eq!(
        faulted.specimen.biology_digest(),
        before.biology_digest(),
        "the tissue is what it is; only the measurement of it degraded"
    );
    assert!(faulted.has_signature());
}

#[test]
fn a_mutation_that_edits_the_biology_is_a_semantic_mutation_wearing_a_preanalytic_label() {
    let sneaky = cold_ischaemia(Intensity::FULL).editing(Edit::Biology {
        field: "marker_state".to_string(),
        value: json!("absent"),
    });
    let refusal = apply(&specimen(), &sneaky).expect_err("27.09 is a different family");
    assert!(matches!(
        refusal,
        PreanalyticRefusal::BiologicalStateChanged { field, .. } if field == "marker_state"
    ));
}

#[test]
fn a_biology_edit_that_writes_the_value_already_there_is_not_a_change() {
    let harmless = cold_ischaemia(Intensity::FULL).editing(Edit::Biology {
        field: "marker_state".to_string(),
        value: json!("present"),
    });
    apply(&specimen(), &harmless)
        .expect("the postcondition is about the state, not about the attempt");
}

#[test]
fn a_fault_with_no_observable_signature_asks_for_something_the_world_does_not_contain() {
    let silent = PreanalyticMutation::new(
        "silent",
        "cold-ischaemia",
        FaultKind::ColdIschaemia { minutes: 120 },
        Intensity::FULL,
        ExpectedResponse::Detect,
    )
    .editing(Edit::Handling {
        stage: Stage::Collection,
        field: "ischaemia_minutes".to_string(),
        value: json!(120),
    });
    assert!(matches!(
        apply(&specimen(), &silent).expect_err("nothing an instrument could notice"),
        PreanalyticRefusal::NoQcSignature { .. }
    ));
}

#[test]
fn a_qc_field_that_names_the_fault_is_a_label_not_a_signal() {
    let leaky = PreanalyticMutation::new(
        "ci-full",
        "cold-ischaemia",
        FaultKind::ColdIschaemia { minutes: 120 },
        Intensity::FULL,
        ExpectedResponse::Detect,
    )
    .editing(Edit::Qc {
        field: "cold_ischaemia_flag".to_string(),
        delta: 1,
    });
    assert!(matches!(
        apply(&specimen(), &leaky).expect_err("the QC panel spelled out the answer"),
        PreanalyticRefusal::QcLabelLeaksAnswer { .. }
    ));
}

#[test]
fn a_handling_record_whose_value_names_the_fault_leaks_it_just_as_well() {
    let leaky = cold_ischaemia(Intensity::FULL).editing(Edit::Handling {
        stage: Stage::Collection,
        field: "note".to_string(),
        value: json!("cold_ischaemia injected by the harness"),
    });
    assert!(matches!(
        apply(&specimen(), &leaky).expect_err("free text leaks as readily as a field name"),
        PreanalyticRefusal::QcLabelLeaksAnswer { .. }
    ));
}

#[test]
fn a_downstream_stage_still_describing_the_pre_fault_specimen_is_internally_contradictory() {
    let with_stale_downstream = specimen().with_handling(
        Stage::Processing,
        UPSTREAM_STATE_FIELD,
        json!("a digest taken before the fault"),
    );
    assert!(matches!(
        apply(&with_stale_downstream, &cold_ischaemia(Intensity::FULL))
            .expect_err("the processing lab received a specimen that no longer exists"),
        PreanalyticRefusal::StagesInconsistent { .. }
    ));
}

#[test]
fn a_downstream_record_that_asserts_nothing_is_left_alone() {
    let quiet_downstream =
        specimen().with_handling(Stage::Processing, "operator", json!("tech-2"));
    apply(&quiet_downstream, &cold_ischaemia(Intensity::FULL))
        .expect("a record that made no claim about what it received cannot be stale");
}

#[test]
fn the_null_member_of_a_family_must_change_nothing_at_all() {
    let good_null = cold_ischaemia(Intensity::NULL);
    validate_family(&specimen(), "cold-ischaemia", &[good_null])
        .expect("scaled deltas vanish at zero intensity");

    let stamping_null = cold_ischaemia(Intensity::NULL).editing(Edit::Handling {
        stage: Stage::Collection,
        field: "protocol_variant".to_string(),
        value: json!("B"),
    });
    assert!(matches!(
        validate_family(&specimen(), "cold-ischaemia", &[stamping_null])
            .expect_err("a harness footprint at zero intensity destroys the control"),
        PreanalyticRefusal::NullMemberIsNotNull { .. }
    ));
}

#[test]
fn the_detectability_floor_is_the_smallest_intensity_that_crosses_the_callers_alert_level() {
    let sweep: Vec<PreanalyticMutation> = [0, 1_000, 2_500, 5_000, 10_000]
        .into_iter()
        .map(|i| cold_ischaemia(Intensity::per_ten_thousand(i)))
        .collect();
    let floor = detectability_floor(&specimen(), &sweep, "rna_integrity", 1_000)
        .expect("the full-strength member is well past the alert level");
    assert_eq!(floor, Intensity::per_ten_thousand(2_500));

    assert_eq!(
        detectability_floor(&specimen(), &sweep, "rna_integrity", 99_999),
        None,
        "a threshold nothing reaches is a finding about the family, not a saturating default"
    );
}

#[test]
fn a_response_the_world_cannot_offer_is_an_unanswerable_task_dressed_as_a_hard_one() {
    let needs_confirmation = PreanalyticMutation::new(
        "ci-confirm",
        "cold-ischaemia",
        FaultKind::ColdIschaemia { minutes: 120 },
        Intensity::FULL,
        ExpectedResponse::SelectConfirmatory {
            measurement: "repeat on the paired frozen aliquot".to_string(),
        },
    );
    let empty = BTreeSet::new();
    assert!(matches!(
        check_response(&needs_confirmation, &empty).expect_err("no such measurement exists"),
        PreanalyticRefusal::ResponseNotAvailable { .. }
    ));

    let available = BTreeSet::from(["repeat on the paired frozen aliquot".to_string()]);
    check_response(&needs_confirmation, &available).expect("now it is answerable");
}

#[test]
fn detection_and_abstention_need_no_action_to_be_available() {
    let empty = BTreeSet::new();
    for expected in [ExpectedResponse::Detect, ExpectedResponse::Abstain] {
        let mutation = PreanalyticMutation::new(
            "m",
            "f",
            FaultKind::FreezeThaw { cycles: 3 },
            Intensity::FULL,
            expected,
        );
        check_response(&mutation, &empty).expect("saying so is always available");
    }
}

#[test]
fn every_fault_kind_lands_on_one_of_the_seven_stages_the_blueprint_names() {
    let kinds = [
        FaultKind::ColdIschaemia { minutes: 1 },
        FaultKind::FixationDuration { hours: 1 },
        FaultKind::FixativeSubstitution {
            fixative: "x".to_string(),
        },
        FaultKind::FreezeThaw { cycles: 1 },
        FaultKind::StorageExcursion { hours: 1 },
        FaultKind::ReagentLotChange {
            lot: "L2".to_string(),
        },
        FaultKind::ProcessingDelay { hours: 1 },
    ];
    for kind in kinds {
        assert!(Stage::PIPELINE.contains(&kind.stage()));
    }
    assert_eq!(
        Stage::Collection.downstream().len(),
        6,
        "collection is first, so every other stage is downstream of it"
    );
    assert!(Stage::Processing.downstream().is_empty());
}
