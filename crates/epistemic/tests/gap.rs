//! Blueprint 43.50: what `fiber-query/0.2` cannot carry, measured against the real parser.

use bioprism_epistemic::gap::{
    audit, proposed_query_document, unknown_fields_are_refused, FieldState,
    CURRENT_SCHEMA_VERSION, PROPOSED_SCHEMA_VERSION, REQUIRED_FOR_RATE_DISTORTION,
};
use serde_json::json;

#[test]
fn fiber_query_0_2_refuses_a_decision_loss_field_by_name_rather_than_discarding_it() {
    let refused = unknown_fields_are_refused().expect("the parser refuses rather than erroring");
    assert!(
        refused.contains(&"decision_loss".to_string()),
        "0.1 accepted this document and dropped the field, so a caller got no error, no warning          and no effect while missing_contract_fields went on reporting it missing. 0.2 must name          the key it refused. refused: {refused:?}"
    );
    assert!(refused.contains(&"permitted_actions".to_string()));
    assert!(
        !refused.is_empty(),
        "an empty refusal would mean the boundary had gone quiet again, which is the defect"
    );
}

#[test]
fn the_proposed_fields_cover_everything_the_compiler_reports_missing() {
    let document = json!({
        "schema_version": CURRENT_SCHEMA_VERSION,
        "query_id": "q-coverage-0001",
        "targets": ["split_supports_external_validity"],
        "decision_time": "2026-01-01T00:00:00Z",
        "budgets": { "max_facts": 8 }
    });
    let query = bioprism_fiber::Query::from_json(document).expect("valid v0.1 query");

    for missing in query.missing_contract_fields() {
        assert!(
            REQUIRED_FOR_RATE_DISTORTION
                .iter()
                .any(|field| field.name == missing),
            "bioprism-fiber reports {missing:?} missing and this crate's proposal does not name it"
        );
    }
}

#[test]
fn the_proposed_document_is_not_parseable_by_the_shipped_schema() {
    let document = proposed_query_document();
    assert_eq!(document["schema_version"], PROPOSED_SCHEMA_VERSION);
    assert!(
        bioprism_fiber::Query::from_json(document).is_err(),
        "a version bump has to be visible at the boundary; if the proposed document parsed as \
         v0.1 the fields would be optional-and-ignorable, which is the failure being fixed"
    );
}

#[test]
fn every_required_field_names_a_pass_that_needs_it_and_what_to_do_without_it() {
    for field in REQUIRED_FOR_RATE_DISTORTION {
        assert!(!field.unblocks.is_empty(), "{} unblocks nothing", field.name);
        assert!(
            !field.required_by.is_empty(),
            "{} cites no blueprint module",
            field.name
        );
        assert!(
            !field.absent_behaviour.is_empty(),
            "{} does not say what happens when it is absent, which is where a default sneaks in",
            field.name
        );
        assert!(
            !field.absent_behaviour.contains("default to a uniform"),
            "no field may specify a substituted default"
        );
    }
}

#[test]
fn the_audit_distinguishes_absent_from_present_and_discarded() {
    let bare = json!({
        "schema_version": CURRENT_SCHEMA_VERSION,
        "query_id": "q-bare-0001",
        "targets": ["y"],
        "decision_time": "2026-01-01T00:00:00Z",
        "budgets": { "max_facts": 1 }
    });
    let states = audit(&bare).expect("object");
    assert!(states
        .iter()
        .all(|s| matches!(s, FieldState::Absent { .. })));

    let mut with_loss = bare.clone();
    with_loss["decision_loss"] = proposed_query_document()["decision_loss"].clone();
    with_loss["distortion_tolerance"] = json!(0.05);
    let states = audit(&with_loss).expect("object");

    assert!(
        states.iter().any(|s| matches!(
            s,
            FieldState::PresentAndRefused {
                field: "decision_loss"
            }
        )),
        "a written-but-ignored field is a third state, not a present one"
    );
    assert!(states.iter().any(|s| matches!(
        s,
        FieldState::PresentAndRead {
            field: "distortion_tolerance"
        }
    )));
}

#[test]
fn a_query_document_that_is_not_an_object_is_refused_by_the_audit() {
    assert!(audit(&json!([1, 2, 3])).is_err());
}
