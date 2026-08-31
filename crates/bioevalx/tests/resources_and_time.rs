//! Nonrenewable resources across forks (26.06) and the four-clock availability audit (26.07).

use bioprism_bioevalx::burden::{Draw, Ledger, Resource, ResourceClass};
use bioprism_bioevalx::error::{BurdenError, WorldlineError};
use bioprism_bioevalx::worldline::{Clock, Decision, Observation, Worldline};
use bioprism_scope::Timestamp;
use serde_json::json;

fn at(rfc3339: &str) -> Timestamp {
    Timestamp::parse(rfc3339).expect("fixture timestamp parses")
}

fn ledger_with_one_biopsy() -> Ledger {
    let mut ledger = Ledger::new("root");
    ledger
        .declare(Resource::new(
            "biopsy-aliquot",
            ResourceClass::TissueAliquot,
            40,
            "uL",
        ))
        .expect("first declaration");
    ledger
        .declare(Resource::new(
            "gpu",
            ResourceClass::ComputeAndMoney,
            100,
            "hour",
        ))
        .expect("first declaration");
    ledger
}

#[test]
fn two_forks_cannot_both_spend_the_last_aliquot() {
    let mut ledger = ledger_with_one_biopsy();
    ledger.fork("root", "arm-a").expect("root exists");
    ledger.fork("root", "arm-b").expect("root exists");
    ledger
        .draw("arm-a", Draw::spent("rnaseq", "biopsy-aliquot", 40, "uL"))
        .expect("within the pool");
    ledger
        .draw(
            "arm-b",
            Draw::spent("proteomics", "biopsy-aliquot", 40, "uL"),
        )
        .expect("each branch sees the full pool");

    match ledger.joint_feasibility(&["arm-a", "arm-b"]) {
        Err(BurdenError::ForkDoubleSpend { resource, .. }) => {
            assert_eq!(resource, "biopsy-aliquot");
        }
        other => panic!("expected a double-spend refusal, got {other:?}"),
    }
}

#[test]
fn two_forks_may_both_spend_a_renewable_resource() {
    let mut ledger = ledger_with_one_biopsy();
    ledger.fork("root", "arm-a").expect("root exists");
    ledger.fork("root", "arm-b").expect("root exists");
    ledger
        .draw("arm-a", Draw::spent("train", "gpu", 20, "hour"))
        .expect("within the pool");
    ledger
        .draw("arm-b", Draw::spent("train", "gpu", 20, "hour"))
        .expect("within the pool");

    ledger
        .joint_feasibility(&["arm-a", "arm-b"])
        .expect("compute is not a contested aliquot");
}

#[test]
fn a_branch_inherits_what_its_parent_already_spent() {
    let mut ledger = ledger_with_one_biopsy();
    ledger
        .draw("root", Draw::spent("screen", "biopsy-aliquot", 30, "uL"))
        .expect("within the pool");
    ledger.fork("root", "child").expect("root exists");

    assert_eq!(
        ledger
            .remaining("child", "biopsy-aliquot")
            .expect("declared"),
        10
    );
    match ledger.draw("child", Draw::spent("confirm", "biopsy-aliquot", 20, "uL")) {
        Err(BurdenError::Overdraw { remaining, .. }) => assert_eq!(remaining, 10),
        other => panic!("expected an overdraw refusal, got {other:?}"),
    }
}

#[test]
fn material_spent_on_an_action_that_failed_is_still_gone() {
    let mut ledger = ledger_with_one_biopsy();
    ledger
        .draw(
            "root",
            Draw::spent("rnaseq", "biopsy-aliquot", 25, "uL").wasted(),
        )
        .expect("within the pool");

    assert_eq!(
        ledger
            .remaining("root", "biopsy-aliquot")
            .expect("declared"),
        15
    );
    let wasted = ledger.wasted_nonrenewable("root").expect("branch exists");
    assert_eq!(wasted.len(), 1);
    assert_eq!(wasted[0].action, "rnaseq");
}

#[test]
fn a_draw_quoted_in_a_different_unit_is_refused_rather_than_converted() {
    let mut ledger = ledger_with_one_biopsy();

    match ledger.draw("root", Draw::spent("rnaseq", "biopsy-aliquot", 1, "mL")) {
        Err(BurdenError::UnitMismatch { left, right, .. }) => {
            assert_eq!(left, "uL");
            assert_eq!(right, "mL");
        }
        other => panic!("expected a unit refusal, got {other:?}"),
    }
}

#[test]
fn duplicate_branches_are_refused_without_overwriting_existing_spend() {
    let mut ledger = ledger_with_one_biopsy();
    ledger.fork("root", "arm").expect("root exists");
    ledger
        .draw("arm", Draw::spent("screen", "biopsy-aliquot", 5, "uL"))
        .expect("within the pool");

    let refusal = ledger
        .fork("root", "arm")
        .expect_err("a branch id cannot replace an existing branch");

    assert!(matches!(refusal, BurdenError::DuplicateBranch(id) if id == "arm"));
    assert_eq!(ledger.branch("arm").expect("branch remains").draws().len(), 1);
}

#[test]
fn ancestor_and_child_draws_are_one_continuation_not_a_fork_double_spend() {
    let mut ledger = ledger_with_one_biopsy();
    ledger
        .draw("root", Draw::spent("screen", "biopsy-aliquot", 10, "uL"))
        .expect("within the pool");
    ledger.fork("root", "child").expect("root exists");
    ledger
        .draw("child", Draw::spent("confirm", "biopsy-aliquot", 10, "uL"))
        .expect("child inherits the remaining pool");

    ledger
        .joint_feasibility(&["root", "child"])
        .expect("an ancestor and its continuation are one world");
}

#[test]
fn malformed_resource_and_draw_labels_are_refused_before_spending() {
    let mut ledger = Ledger::new("root");
    let resource_refusal = ledger
        .declare(Resource::new(" ", ResourceClass::TissueAliquot, 1, "uL"))
        .expect_err("resource identity must be bounded");
    assert!(matches!(
        resource_refusal,
        BurdenError::InvalidResource { .. }
    ));

    ledger
        .declare(Resource::new(
            "biopsy-aliquot",
            ResourceClass::TissueAliquot,
            1,
            "uL",
        ))
        .expect("valid resource");
    let draw_refusal = ledger
        .draw("root", Draw::spent(" ", "biopsy-aliquot", 1, "uL"))
        .expect_err("draw action identity must be bounded");
    assert!(matches!(draw_refusal, BurdenError::InvalidDraw { .. }));
}

#[test]
fn a_joint_query_cannot_repeat_the_same_branch_as_two_worlds() {
    let ledger = ledger_with_one_biopsy();

    let refusal = ledger
        .joint_feasibility(&["root", "root"])
        .expect_err("one branch cannot stand in for two independent worlds");

    assert!(matches!(
        refusal,
        BurdenError::DuplicateBranchReference(branch) if branch == "root"
    ));
}

#[test]
fn a_nondestructive_draw_does_not_make_two_branches_infeasible() {
    let mut ledger = ledger_with_one_biopsy();
    ledger.fork("root", "arm-a").expect("root exists");
    ledger.fork("root", "arm-b").expect("root exists");
    for branch in ["arm-a", "arm-b"] {
        ledger
            .draw(
                branch,
                Draw::spent("image", "biopsy-aliquot", 5, "uL").nondestructive(),
            )
            .expect("within the pool");
    }

    ledger
        .joint_feasibility(&["arm-a", "arm-b"])
        .expect("imaging the block twice destroys nothing");
}

#[test]
fn residual_value_is_reported_per_branch() {
    let mut ledger = ledger_with_one_biopsy();
    ledger.fork("root", "arm").expect("root exists");
    ledger
        .draw("arm", Draw::spent("rnaseq", "biopsy-aliquot", 10, "uL"))
        .expect("within the pool");

    let residual = ledger.residual("arm").expect("branch exists");
    assert_eq!(residual["biopsy-aliquot"], 30);
    assert_eq!(residual["gpu"], 100);
    assert_eq!(
        ledger.residual("root").expect("branch exists")["biopsy-aliquot"],
        40,
        "a child's spending does not deplete its parent"
    );
}

#[test]
fn privacy_exposure_counts_as_nonrenewable() {
    assert!(ResourceClass::PrivacyAccess.is_nonrenewable());
    assert!(!ResourceClass::ComputeAndMoney.is_nonrenewable());
}

#[test]
fn a_measurement_cannot_precede_the_biology_it_measures() {
    let outcome = Observation::new(
        "path-1",
        at("2026-03-10T00:00:00Z"),
        at("2026-03-08T00:00:00Z"),
        at("2026-03-12T00:00:00Z"),
        at("2026-03-12T00:00:00Z"),
    );

    assert!(matches!(
        outcome,
        Err(WorldlineError::MeasuredBeforeOccurred { .. })
    ));
}

#[test]
fn a_record_cannot_precede_the_measurement_it_records() {
    let outcome = Observation::new(
        "path-1",
        at("2026-03-10T00:00:00Z"),
        at("2026-03-11T00:00:00Z"),
        at("2026-03-10T12:00:00Z"),
        at("2026-03-12T00:00:00Z"),
    );

    assert!(matches!(
        outcome,
        Err(WorldlineError::RecordedBeforeMeasured { .. })
    ));
}

#[test]
fn an_observation_requires_a_bounded_identity() {
    let outcome = Observation::new(
        " ",
        at("2026-03-10T00:00:00Z"),
        at("2026-03-10T00:00:00Z"),
        at("2026-03-10T00:00:00Z"),
        at("2026-03-10T00:00:00Z"),
    );

    assert!(matches!(
        outcome,
        Err(WorldlineError::InvalidObservation { .. })
    ));
}

#[test]
fn duplicate_decisions_and_context_references_are_refused() {
    let mut worldline = Worldline::new();
    let decision = Decision {
        id: "call".into(),
        at: at("2026-03-12T00:00:00Z"),
        context: vec!["never-recorded".into(), "never-recorded".into()],
    };
    let context_refusal = worldline
        .decide(decision)
        .expect_err("one context reference must be listed once");
    assert!(matches!(
        context_refusal,
        WorldlineError::DuplicateContextReference { .. }
    ));

    worldline
        .decide(Decision {
            id: "call".into(),
            at: at("2026-03-12T00:00:00Z"),
            context: vec![],
        })
        .expect("first decision");
    let duplicate = worldline
        .decide(Decision {
            id: "call".into(),
            at: at("2026-03-13T00:00:00Z"),
            context: vec![],
        })
        .expect_err("decision identity must be unique");
    assert!(matches!(duplicate, WorldlineError::DuplicateDecision(id) if id == "call"));
}

#[test]
fn persisted_worldlines_revalidate_context_invariants_before_audit() {
    let mut worldline = Worldline::new();
    worldline
        .decide(Decision {
            id: "call".into(),
            at: at("2026-03-12T00:00:00Z"),
            context: vec!["never-recorded".into()],
        })
        .expect("valid decision");
    let mut encoded = serde_json::to_value(&worldline).expect("worldline serialises");
    encoded["decisions"][0]["context"] = json!(["never-recorded", "never-recorded"]);

    let refusal = serde_json::from_value::<Worldline>(encoded)
        .expect_err("a persisted worldline must not bypass duplicate-context checks");

    assert!(refusal.to_string().contains("more than once"));
}

#[test]
fn evidence_about_an_early_day_that_was_signed_out_late_is_leakage_in_an_early_context() {
    let mut worldline = Worldline::new();
    worldline
        .observe(
            Observation::new(
                "path-77",
                at("2026-03-10T00:00:00Z"),
                at("2026-03-10T00:00:00Z"),
                at("2026-03-24T00:00:00Z"),
                at("2026-03-24T00:00:00Z"),
            )
            .expect("clocks are ordered"),
        )
        .expect("first observation");
    worldline
        .decide(Decision {
            id: "day-12-call".into(),
            at: at("2026-03-12T00:00:00Z"),
            context: vec!["path-77".into()],
        })
        .expect("valid decision");
    worldline
        .decide(Decision {
            id: "day-30-call".into(),
            at: at("2026-03-30T00:00:00Z"),
            context: vec!["path-77".into()],
        })
        .expect("valid decision");

    let leaks = worldline.audit();

    assert_eq!(leaks.len(), 1, "only the early decision leaks");
    assert_eq!(leaks[0].decision, "day-12-call");
    assert_eq!(leaks[0].clock, Clock::Accessible);
    assert_eq!(leaks[0].available_at, "2026-03-24T00:00:00Z");
}

#[test]
fn a_dangling_context_reference_is_reported_separately_from_leakage() {
    let mut worldline = Worldline::new();
    worldline
        .decide(Decision {
            id: "call".into(),
            at: at("2026-03-12T00:00:00Z"),
            context: vec!["never-recorded".into()],
        })
        .expect("valid decision");

    assert!(
        worldline.audit().is_empty(),
        "dropping an observation must not launder leakage into silence"
    );
    assert_eq!(worldline.dangling(), vec![("call", "never-recorded")]);
}

#[test]
fn what_was_admissible_is_reported_alongside_what_was_used() {
    let mut worldline = Worldline::new();
    for (id, accessible) in [
        ("early", "2026-03-01T00:00:00Z"),
        ("late", "2026-04-01T00:00:00Z"),
    ] {
        worldline
            .observe(
                Observation::new(
                    id,
                    at("2026-02-01T00:00:00Z"),
                    at("2026-02-01T00:00:00Z"),
                    at(accessible),
                    at(accessible),
                )
                .expect("clocks are ordered"),
            )
            .expect("distinct ids");
    }

    assert_eq!(
        worldline.admissible_at(at("2026-03-15T00:00:00Z")),
        vec!["early"]
    );
}

#[test]
fn the_availability_lag_is_the_gap_a_single_clock_model_cannot_see() {
    let observation = Observation::new(
        "path-77",
        at("2026-03-10T00:00:00Z"),
        at("2026-03-10T00:00:00Z"),
        at("2026-03-24T00:00:00Z"),
        at("2026-03-24T00:00:00Z"),
    )
    .expect("clocks are ordered");

    assert_eq!(
        observation.availability_lag_nanos(),
        14 * 24 * 60 * 60 * 1_000_000_000i128
    );
    assert_eq!(observation.at(Clock::Occurred), at("2026-03-10T00:00:00Z"));
}
