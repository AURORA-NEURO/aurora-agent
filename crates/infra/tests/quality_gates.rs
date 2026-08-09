//! A check that could not run is neither a pass nor a failure, and a gate that contains one has
//! not passed.

use bioprism_infra::{
    Check, CheckOutcome, Dataset, Gate, GateVerdict, NotRunnable, QualityError, ReferenceSets,
};
use serde_json::json;

fn cohort() -> Dataset {
    Dataset::new("cohort")
        .expect("named dataset")
        .with_column("patient", [json!("p-1"), json!("p-2"), json!("p-3")])
        .expect("column")
        .with_column("stage", [json!("II"), json!("III"), json!("IV")])
        .expect("column")
        .with_column("age", [json!(41), json!(58), json!(63)])
        .expect("column")
}

#[test]
fn a_ragged_column_is_refused_because_it_makes_every_row_indexed_witness_ambiguous() {
    let error = Dataset::new("cohort")
        .expect("dataset")
        .with_column("a", [json!(1), json!(2)])
        .expect("column")
        .with_column("b", [json!(1)])
        .expect_err("the second column is short");
    assert_eq!(
        error,
        QualityError::RaggedColumn {
            column: "b".to_string(),
            found: 1,
            expected: 2
        }
    );
}

#[test]
fn a_passing_check_reports_how_many_values_it_examined() {
    let outcome = Check::NotNull {
        column: "patient".to_string(),
    }
    .run(&cohort(), &ReferenceSets::new());
    assert_eq!(outcome, CheckOutcome::Pass { examined: 3 });
}

#[test]
fn a_failing_check_produces_a_witness_naming_the_row_the_value_and_the_expectation() {
    let data = Dataset::new("cohort")
        .expect("dataset")
        .with_column("stage", [json!("II"), json!("IV+")])
        .expect("column");
    let outcome = Check::OneOf {
        column: "stage".to_string(),
        allowed: ["I", "II", "III", "IV"]
            .into_iter()
            .map(String::from)
            .collect(),
    }
    .run(&data, &ReferenceSets::new());

    let witness = outcome.witness().expect("a failure carries a witness");
    assert_eq!(witness.row, 1);
    assert_eq!(witness.column, "stage");
    assert_eq!(witness.found, "IV+");
    assert!(witness.expected.contains("IV"));
}

#[test]
fn a_missing_column_is_not_runnable_rather_than_a_pass_or_a_failure() {
    let outcome = Check::NotNull {
        column: "absent".to_string(),
    }
    .run(&cohort(), &ReferenceSets::new());
    assert_eq!(
        outcome,
        CheckOutcome::NotRunnable {
            reason: NotRunnable::MissingColumn {
                column: "absent".to_string()
            }
        }
    );
    assert!(!outcome.is_pass());
    assert!(!outcome.is_fail());
}

#[test]
fn a_column_of_only_nulls_is_not_runnable_rather_than_a_vacuous_pass() {
    let data = Dataset::new("cohort")
        .expect("dataset")
        .with_column("age", [json!(null), json!(null)])
        .expect("column");
    let outcome = Check::InRange {
        column: "age".to_string(),
        min: 0.0,
        max: 120.0,
    }
    .run(&data, &ReferenceSets::new());
    assert_eq!(
        outcome,
        CheckOutcome::NotRunnable {
            reason: NotRunnable::AllValuesNull {
                column: "age".to_string()
            }
        }
    );
}

#[test]
fn a_non_numeric_value_in_a_range_check_is_not_runnable_rather_than_out_of_range() {
    let data = Dataset::new("cohort")
        .expect("dataset")
        .with_column("age", [json!(41), json!("unknown")])
        .expect("column");
    let outcome = Check::InRange {
        column: "age".to_string(),
        min: 0.0,
        max: 120.0,
    }
    .run(&data, &ReferenceSets::new());
    assert_eq!(
        outcome,
        CheckOutcome::NotRunnable {
            reason: NotRunnable::NotComparable {
                column: "age".to_string(),
                row: 1,
                found: "unknown".to_string()
            }
        }
    );
}

#[test]
fn a_foreign_key_check_without_its_reference_set_is_not_runnable_rather_than_failing_every_row() {
    let outcome = Check::ForeignKey {
        column: "patient".to_string(),
        reference: "registry".to_string(),
    }
    .run(&cohort(), &ReferenceSets::new());
    assert_eq!(
        outcome,
        CheckOutcome::NotRunnable {
            reason: NotRunnable::MissingReferenceSet {
                reference: "registry".to_string()
            }
        }
    );
}

#[test]
fn a_foreign_key_check_with_its_reference_set_runs_and_names_the_row_that_is_absent() {
    let references = ReferenceSets::new().with("registry", ["p-1", "p-2"]);
    let outcome = Check::ForeignKey {
        column: "patient".to_string(),
        reference: "registry".to_string(),
    }
    .run(&cohort(), &references);
    let witness = outcome.witness().expect("p-3 is not in the registry");
    assert_eq!(witness.found, "p-3");
    assert_eq!(witness.row, 2);
}

#[test]
fn nulls_are_skipped_by_every_check_except_the_one_that_is_about_nulls() {
    let data = Dataset::new("cohort")
        .expect("dataset")
        .with_column("age", [json!(41), json!(null), json!(63)])
        .expect("column");
    let range = Check::InRange {
        column: "age".to_string(),
        min: 0.0,
        max: 120.0,
    }
    .run(&data, &ReferenceSets::new());
    assert_eq!(range, CheckOutcome::Pass { examined: 2 });

    let not_null = Check::NotNull {
        column: "age".to_string(),
    }
    .run(&data, &ReferenceSets::new());
    assert_eq!(not_null.witness().map(|w| w.row), Some(1));
}

#[test]
fn a_duplicate_value_names_the_row_it_first_appeared_at() {
    let data = Dataset::new("cohort")
        .expect("dataset")
        .with_column("patient", [json!("p-1"), json!("p-2"), json!("p-1")])
        .expect("column");
    let outcome = Check::Unique {
        column: "patient".to_string(),
    }
    .run(&data, &ReferenceSets::new());
    let witness = outcome.witness().expect("duplicate");
    assert_eq!(witness.row, 2);
    assert!(witness.expected.contains("row 0"));
}

#[test]
fn a_decreasing_sequence_fails_and_names_the_earlier_row_it_fell_below() {
    let data = Dataset::new("timeline")
        .expect("dataset")
        .with_column("day", [json!(1), json!(4), json!(3)])
        .expect("column");
    let outcome = Check::NonDecreasing {
        column: "day".to_string(),
    }
    .run(&data, &ReferenceSets::new());
    let witness = outcome.witness().expect("3 follows 4");
    assert_eq!(witness.row, 2);
    assert!(witness.expected.contains("row 1"));
}

#[test]
fn a_row_count_below_the_minimum_is_a_failure_and_not_an_unrunnable_check() {
    let data = Dataset::new("cohort").expect("dataset");
    let outcome = Check::RowCountAtLeast { rows: 10 }.run(&data, &ReferenceSets::new());
    assert!(outcome.is_fail());
    assert_eq!(
        outcome.witness().map(|w| w.found.clone()),
        Some("0".to_string())
    );
}

#[test]
fn a_gate_passes_only_when_every_check_ran_and_every_check_held() {
    let gate = Gate::new("cohort-admission")
        .expect("gate")
        .with(
            "patient-present",
            Check::NotNull {
                column: "patient".to_string(),
            },
        )
        .expect("check")
        .with(
            "age-plausible",
            Check::InRange {
                column: "age".to_string(),
                min: 0.0,
                max: 120.0,
            },
        )
        .expect("check");

    let report = gate.run(&cohort(), &ReferenceSets::new());
    assert_eq!(report.verdict, GateVerdict::Passed { checks: 2 });
    assert!(report.verdict.is_passed());
}

#[test]
fn a_gate_with_an_unrunnable_check_is_indeterminate_and_not_passed() {
    let gate = Gate::new("cohort-admission")
        .expect("gate")
        .with(
            "patient-present",
            Check::NotNull {
                column: "patient".to_string(),
            },
        )
        .expect("check")
        .with(
            "linked-to-registry",
            Check::ForeignKey {
                column: "patient".to_string(),
                reference: "registry".to_string(),
            },
        )
        .expect("check");

    let report = gate.run(&cohort(), &ReferenceSets::new());
    assert!(!report.verdict.is_passed());
    match &report.verdict {
        GateVerdict::Indeterminate { not_runnable } => {
            assert!(not_runnable.contains("linked-to-registry"));
        }
        other => panic!("expected indeterminate, got {other:?}"),
    }
    assert_eq!(report.blocked().len(), 1);
}

#[test]
fn a_failure_does_not_hide_an_unrunnable_check_behind_it() {
    let data = Dataset::new("cohort")
        .expect("dataset")
        .with_column("patient", [json!(null)])
        .expect("column");
    let gate = Gate::new("cohort-admission")
        .expect("gate")
        .with(
            "patient-present",
            Check::NotNull {
                column: "patient".to_string(),
            },
        )
        .expect("check")
        .with(
            "stage-known",
            Check::OneOf {
                column: "stage".to_string(),
                allowed: ["I"].into_iter().map(String::from).collect(),
            },
        )
        .expect("check");

    let report = gate.run(&data, &ReferenceSets::new());
    match &report.verdict {
        GateVerdict::Failed {
            failing,
            not_runnable,
        } => {
            assert!(failing.contains("patient-present"));
            assert!(
                not_runnable.contains("stage-known"),
                "the unrun check must remain visible alongside the failure"
            );
        }
        other => panic!("expected failed, got {other:?}"),
    }
}

#[test]
fn two_checks_under_one_name_are_refused_so_a_gate_cannot_silently_drop_one() {
    let error = Gate::new("gate")
        .expect("gate")
        .with(
            "same",
            Check::NotNull {
                column: "a".to_string(),
            },
        )
        .expect("check")
        .with(
            "same",
            Check::NotNull {
                column: "b".to_string(),
            },
        )
        .expect_err("duplicate name");
    assert_eq!(error, QualityError::DuplicateCheckName("same".to_string()));
}

#[test]
fn a_gate_report_serializes_with_its_witnesses_and_its_reasons() {
    let gate = Gate::new("gate")
        .expect("gate")
        .with(
            "linked",
            Check::ForeignKey {
                column: "patient".to_string(),
                reference: "registry".to_string(),
            },
        )
        .expect("check");
    let report = gate.run(&cohort(), &ReferenceSets::new());
    let text = serde_json::to_string(&report).expect("report serializes");
    assert!(text.contains("MissingReferenceSet"));
    assert!(text.contains("Indeterminate"));
}

#[test]
fn a_dataset_column_declared_twice_is_refused() {
    let error = Dataset::new("d")
        .expect("dataset")
        .with_column("a", [json!(1)])
        .expect("column")
        .with_column("a", [json!(2)])
        .expect_err("duplicate column");
    assert_eq!(error, QualityError::DuplicateColumn("a".to_string()));
}
