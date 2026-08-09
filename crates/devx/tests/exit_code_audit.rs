//! The exit-code audit as a published deliverable.
//!
//! The brief asks for the audit to be asserted in a test, not only written in prose. These tests
//! pin the whole finding set, so a change to either the transcribed registry or the taxonomy
//! surfaces as a failing assertion rather than as a quietly different report.

use bioprism_devx::exitaudit::{
    audit, code_meaning_covers, shipped_code_for, shipped_exit_codes, AuditSeverity,
    DivergenceKind, SHIPPED_REGISTRY_SOURCE, SHIPPED_REGISTRY_TRANSCRIBED_FIELDS,
};
use bioprism_devx::lint::lint;
use bioprism_devx::taxonomy::{DiagnosticClass, Retryability};

#[test]
fn the_audit_reports_two_defects_five_imprecisions_and_one_note() {
    let audit = audit();
    assert_eq!(audit.by_severity(AuditSeverity::Defect).len(), 2);
    assert_eq!(audit.by_severity(AuditSeverity::Imprecision).len(), 5);
    assert_eq!(audit.by_severity(AuditSeverity::Note).len(), 1);
    assert_eq!(audit.divergences.len(), 8);
}

#[test]
fn the_first_defect_services_named_is_that_five_classes_collapse_onto_exit_four() {
    let audit = audit();
    let collision = audit
        .divergences
        .iter()
        .find(|d| d.kind == DivergenceKind::ClassCollision && d.code == 4)
        .expect("the collision on exit 4 is reported");
    assert_eq!(collision.severity, AuditSeverity::Defect);
    assert_eq!(
        collision.classes,
        vec![
            DiagnosticClass::Stale,
            DiagnosticClass::Conflict,
            DiagnosticClass::PolicyDenied,
            DiagnosticClass::ContractViolation,
            DiagnosticClass::Indeterminate,
        ]
    );
}

#[test]
fn the_second_defect_services_named_is_that_stale_is_advertised_as_terminal() {
    let audit = audit();
    let inverted = audit
        .divergences
        .iter()
        .find(|d| d.kind == DivergenceKind::RetryabilityInverted)
        .expect("the stale inversion is reported");
    assert_eq!(inverted.severity, AuditSeverity::Defect);
    assert_eq!(inverted.classes, vec![DiagnosticClass::Stale]);
    assert_eq!(
        DiagnosticClass::Stale.retryability(),
        Retryability::RetryableAsIs
    );
    assert!(!shipped_exit_codes()
        .iter()
        .find(|row| row.code == shipped_code_for(DiagnosticClass::Stale))
        .expect("exit 4 is in the registry")
        .advertised_retryable);
}

#[test]
fn the_collision_on_exit_four_is_a_defect_because_the_classes_disagree_on_retryability() {
    let decisions: Vec<Retryability> = DiagnosticClass::ALL
        .into_iter()
        .filter(|c| shipped_code_for(*c) == 4)
        .map(|c| c.retryability())
        .collect();
    let mut distinct = decisions.clone();
    distinct.sort();
    distinct.dedup();
    assert!(
        distinct.len() > 1,
        "if every class on exit 4 retried alike, the collision would only be an imprecision"
    );
}

#[test]
fn every_imprecision_is_a_meaning_narrowing_or_the_retryability_preserving_collision() {
    for divergence in audit().by_severity(AuditSeverity::Imprecision) {
        assert!(
            matches!(
                divergence.kind,
                DivergenceKind::MeaningNarrowerThanClass | DivergenceKind::ClassCollision
            ),
            "unexpected imprecision kind: {:?}",
            divergence.kind
        );
    }
}

#[test]
fn the_four_classes_whose_exit_code_means_something_else_are_named() {
    let narrowed: Vec<DiagnosticClass> = audit()
        .divergences
        .iter()
        .filter(|d| d.kind == DivergenceKind::MeaningNarrowerThanClass)
        .flat_map(|d| d.classes.clone())
        .collect();
    assert_eq!(
        narrowed,
        vec![
            DiagnosticClass::Conflict,
            DiagnosticClass::PolicyDenied,
            DiagnosticClass::Indeterminate,
            DiagnosticClass::Internal,
        ]
    );
    for class in &narrowed {
        assert!(!code_meaning_covers(*class));
    }
}

#[test]
fn the_four_classes_whose_exit_code_means_what_it_says_produce_no_finding() {
    for class in [
        DiagnosticClass::Usage,
        DiagnosticClass::InvalidInput,
        DiagnosticClass::ContractViolation,
        DiagnosticClass::Unavailable,
    ] {
        assert!(code_meaning_covers(class));
        assert!(
            !audit()
                .divergences
                .iter()
                .filter(|d| matches!(
                    d.kind,
                    DivergenceKind::MeaningNarrowerThanClass | DivergenceKind::RetryabilityInverted
                ))
                .any(|d| d.classes.contains(&class)),
            "{class} produced a per-class finding it should not have"
        );
    }
}

#[test]
fn every_divergence_names_the_distinction_a_replacement_registry_must_preserve() {
    for divergence in &audit().divergences {
        assert!(
            divergence.required_distinction.len() > 30,
            "{:?} states no required distinction",
            divergence.kind
        );
        assert!(!divergence.consequence.trim().is_empty());
        assert!(!divergence.finding.trim().is_empty());
    }
}

#[test]
fn the_audit_renders_as_diagnostics_that_pass_this_crates_own_lint() {
    let diagnostics = audit().as_diagnostics();
    assert_eq!(diagnostics.len(), 8);
    let report = lint(&diagnostics);
    assert!(
        report.is_clean(),
        "the audit fails the lint it publishes: {:?}",
        report.errors()
    );
}

#[test]
fn every_audit_diagnostic_cites_the_transcribed_registry_as_its_site() {
    for diagnostic in audit().as_diagnostics() {
        assert_eq!(diagnostic.site.describe(), SHIPPED_REGISTRY_SOURCE);
        assert!(!diagnostic.remedies.is_empty());
    }
}

#[test]
fn only_the_two_defects_are_escalated_to_a_human() {
    let escalated = audit()
        .as_diagnostics()
        .into_iter()
        .filter(|d| d.human_decision_required)
        .count();
    assert_eq!(escalated, 2);
}

#[test]
fn the_transcription_declares_which_fields_it_copied_and_which_it_did_not() {
    assert_eq!(SHIPPED_REGISTRY_TRANSCRIBED_FIELDS.len(), 3);
    assert!(SHIPPED_REGISTRY_TRANSCRIBED_FIELDS.contains(&"ExitCode::is_retryable"));
    assert_eq!(SHIPPED_REGISTRY_SOURCE, "crates/cli/src/exit.rs");
    for row in shipped_exit_codes() {
        assert!(!row.slug.is_empty());
        assert!(!row.meaning.is_empty());
    }
}

#[test]
fn the_shipped_registry_cannot_express_the_taxonomy_and_the_shortfall_is_three() {
    let audit = audit();
    assert_eq!(audit.shipped_code_count, 6);
    assert_eq!(audit.class_count, 9);
    let failure_codes = audit.shipped_code_count - 2;
    assert!(
        failure_codes < audit.class_count,
        "four failure codes cannot carry nine classes"
    );
}
