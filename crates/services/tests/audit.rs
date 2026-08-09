//! The conformance audit as a standing test.
//!
//! Blueprint 40.32 asks for conformance to be machine-readable rather than a document somebody
//! updates. These assertions are the audit of `crates/services/src/audit.rs` stated as claims: if a
//! crate is repaired, or a contract transcription is corrected, one of them fails and the crate
//! docs have to move with it.
//!
//! They are not duplicates of the unit tests. The unit tests check that the machinery computes a
//! verdict; these check *which* verdict, per contract, by number, so the result cannot drift
//! quietly.

use bioprism_services::audit::{audit, markdown_table, AuditSummary};
use bioprism_services::conformance::Conformance;
use bioprism_services::contract::ContractId;
use bioprism_services::{catalog, implementations, workspace};

fn divergences_of(id: ContractId) -> Vec<String> {
    audit()
        .into_iter()
        .find(|entry| entry.contract == id)
        .unwrap_or_else(|| panic!("{id} is audited"))
        .divergences
}

#[test]
fn not_one_of_the_nine_contracts_is_satisfied_as_written() {
    let summary = AuditSummary::of(&audit());
    assert_eq!(summary.satisfied, 0);
    assert_eq!(summary.diverges, 9);
    assert_eq!(summary.not_implemented, 0);
    assert_eq!(
        summary.divergences, 59,
        "the total is asserted so that a repair or a correction has to move this number"
    );
}

#[test]
fn the_world_builder_contract_is_the_furthest_from_its_implementation() {
    assert_eq!(divergences_of(ContractId::WorldBuilder).len(), 16);
}

#[test]
fn the_registry_backend_contract_is_the_closest_to_its_implementation() {
    assert_eq!(divergences_of(ContractId::RegistryBackend).len(), 3);
}

#[test]
fn the_context_compiler_produces_neither_a_candidate_trace_nor_an_expansion_operation() {
    let divergences = divergences_of(ContractId::ContextCompiler);
    assert!(divergences.contains(&"output not produced: candidate_decision_trace".to_string()));
    assert!(divergences.contains(&"output not produced: expansion_api".to_string()));
}

#[test]
fn the_context_compiler_cannot_express_a_policy_denial_a_stale_graph_or_an_unresolvable_source() {
    let divergences = divergences_of(ContractId::ContextCompiler);
    for label in ["policy conflict", "stale graph", "unresolvable source"] {
        assert!(
            divergences.contains(&format!("failure mode with no typed error: {label}")),
            "{label} should have no typed representation in FiberError"
        );
    }
}

#[test]
fn the_mutation_runtime_identifies_parents_exactly_and_operator_versions_not_at_all() {
    assert!(divergences_of(ContractId::MutationRuntime)
        .iter()
        .any(|divergence| divergence.contains("operator versions")));
}

#[test]
fn the_matched_evaluator_cannot_tell_an_arm_that_failed_from_one_that_never_ran() {
    assert!(divergences_of(ContractId::MatchedEvaluator)
        .contains(&"failure mode with no typed error: partial arm failure".to_string()));
}

#[test]
fn the_adaptive_scheduler_has_no_change_fingerprint_to_score_regression_relevance_against() {
    assert!(divergences_of(ContractId::AdaptiveScheduler)
        .contains(&"input not accepted: architecture_change_fingerprint".to_string()));
}

#[test]
fn the_registry_cannot_enforce_the_invariant_that_private_objects_never_leak() {
    assert!(divergences_of(ContractId::RegistryBackend)
        .iter()
        .any(|divergence| divergence.contains("Private objects never leak")));
}

#[test]
fn six_of_the_seven_operation_contracts_place_a_write_no_crate_performs() {
    let count = audit()
        .into_iter()
        .filter(|entry| {
            entry
                .divergences
                .contains(&"declared effect not performed: WritesArtifactStore".to_string())
        })
        .count();
    assert_eq!(count, 6);
}

#[test]
fn every_contract_the_audit_reports_has_a_transcription_and_a_report_behind_it() {
    for entry in audit() {
        if entry.contract.is_operation() {
            assert!(catalog::contract_for(entry.contract).is_some());
            assert!(implementations::report_for(entry.contract).is_some());
            assert!(!entry.crates.is_empty(), "{}", entry.contract);
        } else {
            assert!(catalog::contract_for(entry.contract).is_none());
        }
    }
}

#[test]
fn the_published_table_agrees_with_the_computed_verdicts() {
    let table = markdown_table();
    for entry in audit() {
        assert!(
            table.contains(&format!("| {} |", entry.module_id)),
            "{} is missing from the table",
            entry.module_id
        );
    }
    assert_eq!(
        table.matches("diverges").count(),
        9,
        "the table must not soften a verdict the audit computed"
    );
}

#[test]
fn the_workspace_graph_reports_only_boundary_findings_and_no_structural_defects() {
    let graph = workspace::service_graph();
    let findings = graph.audit();
    assert_eq!(
        findings.len(),
        graph.undeclared_crossings().len() + graph.orphaned_concerns().len(),
        "every finding in the workspace graph is either an undeclared crossing or an orphaned \
         concern; there are no cycles, unknown services, wrong contracts or unpaired results"
    );
    assert_eq!(graph.undeclared_crossings().len(), 6);
    assert_eq!(graph.orphaned_concerns().len(), 6);
}

#[test]
fn every_audit_entry_is_reproducible_across_calls() {
    assert_eq!(
        audit(),
        audit(),
        "the audit reads no clock and no environment"
    );
}

#[test]
fn a_diverging_verdict_is_never_reported_without_a_named_divergence() {
    for entry in audit() {
        if entry.verdict == Conformance::Diverges {
            assert!(!entry.divergences.is_empty(), "{}", entry.contract);
        }
    }
}
