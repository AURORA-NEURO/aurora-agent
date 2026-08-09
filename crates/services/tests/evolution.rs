//! Contract evolution, checked by the same code that checks schema evolution.
//!
//! Blueprint 40.11 requires every response to identify its schema version and 40.37 requires a
//! change to be classified before it ships. The classification is not done here: every assertion
//! below goes through `bioprism-governance`, so a contract change that would move an artifact
//! digest is breaking for exactly the reason a wire change would be, decided by exactly the same
//! predicate.
//!
//! The point of these tests is that there is no second classifier to disagree with the first.

use bioprism_governance::{CompatibilityClass, CompatibilityMode, FieldSpec, FieldType, VersionBump};
use bioprism_services::catalog;
use bioprism_services::contract::{descriptor, required, ContractId, VERSION_FIELD};
use bioprism_services::error::{ErrorClass, Retryability};

#[test]
fn adding_a_required_output_to_a_contract_is_breaking_and_moves_a_digest() {
    let before = catalog::context_compiler().expect("transcribes");
    let mut after = before.clone();
    let mut fields: Vec<FieldSpec> = before
        .response
        .fields()
        .iter()
        .filter(|field| field.path != VERSION_FIELD)
        .cloned()
        .collect();
    fields.push(required("provenance_manifest"));
    after.response = descriptor(
        "bioprism-context-compile-response/0.2",
        CompatibilityMode::PreserveAndForward,
        fields,
    )
    .expect("well formed");

    let change = before.change_to(&after).expect("same contract");
    assert_eq!(change.class(), CompatibilityClass::Breaking);
    assert!(change.moves_a_digest());
    assert_eq!(change.required_bump(), VersionBump::Major);
    change
        .version_gate()
        .expect("0.1 -> 0.2 under a zero major is the breaking bump");
    assert!(change.assert_class(CompatibilityClass::Compatible).is_err());
}

#[test]
fn a_breaking_change_that_did_not_bump_far_enough_fails_the_version_gate() {
    let before = catalog::registry_backend().expect("transcribes");
    let mut after = before.clone();
    let mut fields: Vec<FieldSpec> = before
        .response
        .fields()
        .iter()
        .filter(|field| field.path != VERSION_FIELD)
        .cloned()
        .collect();
    fields.push(required("moderation_queue"));
    after.response = descriptor(
        "bioprism-publish-response/0.1",
        CompatibilityMode::PreserveAndForward,
        fields,
    )
    .expect("well formed");

    let change = before.change_to(&after).expect("same contract");
    assert_eq!(change.class(), CompatibilityClass::Breaking);
    assert!(
        change.version_gate().is_err(),
        "a required hashed field arrived and the version string did not move"
    );
}

#[test]
fn adding_an_optional_unhashed_output_is_not_breaking() {
    let before = catalog::adaptive_scheduler().expect("transcribes");
    let mut after = before.clone();
    let mut fields: Vec<FieldSpec> = before
        .response
        .fields()
        .iter()
        .filter(|field| field.path != VERSION_FIELD)
        .cloned()
        .collect();
    fields.push(FieldSpec::optional("wall_clock_hint", FieldType::Object).excluded_from_digest());
    after.response = descriptor(
        "bioprism-schedule-response/0.1.1",
        CompatibilityMode::PreserveAndForward,
        fields,
    )
    .expect("well formed");

    let change = before.change_to(&after).expect("same contract");
    assert!(!change.moves_a_digest());
    assert_eq!(change.class(), CompatibilityClass::Compatible);
    assert_eq!(change.required_bump(), VersionBump::Patch);
    change.version_gate().expect(
        "a compatible change still has to move the version: bioprism-governance requires a patch \
         bump for anything that is not a no-op, and the descriptor took one",
    );
}

#[test]
fn a_contract_compared_with_itself_reports_no_change() {
    for contract in catalog::all() {
        let change = contract.change_to(&contract).expect("same contract");
        assert_eq!(change.class(), CompatibilityClass::Compatible);
        assert!(!change.moves_a_digest());
        assert_eq!(change.required_bump(), VersionBump::None);
    }
}

#[test]
fn every_error_class_a_contract_can_raise_carries_a_retry_decision() {
    for contract in catalog::all() {
        for class in contract.error_classes() {
            let decision = class.retryability();
            assert!(
                matches!(
                    decision,
                    Retryability::Never
                        | Retryability::OnlyAfterCallerChange
                        | Retryability::Safe
                ),
                "{}: {class}",
                contract.id
            );
        }
    }
}

#[test]
fn the_taxonomy_is_total_over_every_declared_failure_in_every_contract() {
    let mut covered = 0;
    for contract in catalog::all() {
        for failure in &contract.failures {
            assert!(
                ErrorClass::ALL.contains(&failure.class),
                "{}: {:?} carries a class outside the taxonomy",
                contract.id,
                failure.label
            );
            assert_eq!(failure.retryability(), failure.class.retryability());
            covered += 1;
        }
    }
    assert_eq!(
        covered, 34,
        "the seven modules list 34 failure phrases between them, and every one is bound"
    );
}

#[test]
fn no_contract_declares_a_write_effect_while_claiming_a_retry_is_free() {
    for contract in catalog::all() {
        if contract.idempotency.is_pure() {
            assert!(
                !contract.effects.iter().any(|effect| effect.is_write()),
                "{}",
                contract.id
            );
        }
    }
}

#[test]
fn every_operation_contract_names_a_distinct_pair_of_schemas() {
    let mut ids = Vec::new();
    for contract in catalog::all() {
        ids.push(contract.request.id.to_string());
        ids.push(contract.response.id.to_string());
    }
    let mut unique = ids.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(
        unique.len(),
        ids.len(),
        "two contracts sharing a schema name would make their diffs comparable when they are not"
    );
    assert_eq!(ids.len(), 14);
}

#[test]
fn the_two_structural_modules_have_no_schemas_because_they_specify_no_operation() {
    for id in [ContractId::ServiceGraph, ContractId::DomainBoundaries] {
        assert!(catalog::contract_for(id).is_none());
        assert!(!id.is_operation());
    }
}
