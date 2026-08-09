//! The exit-code audit as a published deliverable.
//!
//! The brief asked for the audit to be asserted in a test rather than only written in prose, and
//! these tests pinned the whole finding set so that a change to either the transcribed registry or
//! the taxonomy surfaced as a failing assertion. `bioprism-cli` has since fixed both defects, so
//! what the same assertions now pin is that they *stay* fixed.
//!
//! Two of them are load bearing in a way the rest are not. A test asserting "the audit reports no
//! defects" is satisfied just as well by an audit that has stopped looking, so the finding set from
//! before the fix is retained in [`registry_before_the_split`] and asserted against the same
//! detector, and a registry synthesised here — one this workspace has never shipped — is asserted
//! to be reported as defective. Between them they say the clean result is a fact about the
//! registry, not about the audit.

use bioprism_devx::exitaudit::{
    audit, audit_registry, code_meaning_covers, registry_before_the_split,
    retry_decision_is_recoverable, shipped_code_for, shipped_exit_codes, shipped_registry,
    AuditSeverity, DivergenceKind, RegistryUnderAudit, SHIPPED_REGISTRY_SOURCE,
    SHIPPED_REGISTRY_TRANSCRIBED_FIELDS,
};
use bioprism_devx::lint::lint;
use bioprism_devx::taxonomy::{DiagnosticClass, Retryability};

/// A registry this workspace has never shipped, in which one code carries two retry decisions.
///
/// Built by rerouting a single class rather than by writing a table out, so that it differs from
/// the shipped registry in exactly the property under test and in nothing else.
fn a_registry_that_collapses_two_retry_decisions_onto_one_code() -> RegistryUnderAudit {
    let mut registry = shipped_registry();
    for routing in &mut registry.classification {
        if routing.class == DiagnosticClass::Stale {
            routing.code = 4;
            routing.meaning_covers = false;
        }
    }
    registry
}

#[test]
fn the_audit_reports_no_defects_two_imprecisions_and_one_note() {
    let audit = audit();
    assert_eq!(
        audit.by_severity(AuditSeverity::Defect).len(),
        0,
        "{:?}",
        audit
            .defects()
            .iter()
            .map(|d| &d.finding)
            .collect::<Vec<_>>()
    );
    assert_eq!(audit.by_severity(AuditSeverity::Imprecision).len(), 2);
    assert_eq!(audit.by_severity(AuditSeverity::Note).len(), 1);
    assert_eq!(audit.divergences.len(), 3);
    assert!(audit.is_clean());
}

#[test]
fn a_caller_can_recover_the_retry_decision_from_the_exit_code_alone() {
    assert!(audit().retry_decision_recoverable_from_the_code_alone);
    assert!(retry_decision_is_recoverable(&shipped_registry()));

    let mut decisions: Vec<(u8, Retryability)> = DiagnosticClass::ALL
        .into_iter()
        .map(|class| (shipped_code_for(class), class.retryability()))
        .collect();
    decisions.sort();
    decisions.dedup();
    let mut codes: Vec<u8> = decisions.iter().map(|(code, _)| *code).collect();
    let before = codes.len();
    codes.dedup();
    assert_eq!(
        before,
        codes.len(),
        "a code appears against two retry decisions, so the status alone does not determine one"
    );
}

#[test]
fn the_first_defect_services_named_is_fixed_and_exit_four_now_carries_one_class() {
    let audit = audit();
    assert!(
        !audit
            .divergences
            .iter()
            .any(|d| d.kind == DivergenceKind::ClassCollision && d.code == 4),
        "exit 4 still carries more than one class"
    );
    let on_four: Vec<DiagnosticClass> = DiagnosticClass::ALL
        .into_iter()
        .filter(|class| shipped_code_for(*class) == 4)
        .collect();
    assert_eq!(on_four, vec![DiagnosticClass::ContractViolation]);
}

#[test]
fn the_second_defect_services_named_is_fixed_and_stale_advertises_the_retry_it_permits() {
    let audit = audit();
    assert!(
        !audit
            .divergences
            .iter()
            .any(|d| d.kind == DivergenceKind::RetryabilityInverted),
        "a code still advertises the opposite of the retry decision it carries"
    );
    assert_eq!(
        DiagnosticClass::Stale.retryability(),
        Retryability::RetryableAsIs
    );
    let row = shipped_exit_codes()
        .into_iter()
        .find(|row| row.code == shipped_code_for(DiagnosticClass::Stale))
        .expect("the code Stale maps to is in the registry");
    assert!(row.advertised_retryable);
    assert_eq!(
        row.advertised_retryability,
        Some(Retryability::RetryableAsIs)
    );
}

#[test]
fn the_five_classes_that_shared_exit_four_now_hold_four_codes_between_them() {
    let mut codes: Vec<u8> = [
        DiagnosticClass::Stale,
        DiagnosticClass::Conflict,
        DiagnosticClass::PolicyDenied,
        DiagnosticClass::ContractViolation,
        DiagnosticClass::Indeterminate,
    ]
    .into_iter()
    .map(shipped_code_for)
    .collect();
    codes.sort();
    codes.dedup();
    assert_eq!(codes, vec![4, 6, 7, 8, 9]);
}

#[test]
fn the_audit_still_reports_the_registry_that_had_the_defects_exactly_as_it_first_did() {
    let audit = audit_registry(&registry_before_the_split());
    assert_eq!(audit.by_severity(AuditSeverity::Defect).len(), 2);
    assert_eq!(audit.by_severity(AuditSeverity::Imprecision).len(), 5);
    assert_eq!(audit.by_severity(AuditSeverity::Note).len(), 1);
    assert_eq!(audit.divergences.len(), 8);
    assert!(!audit.is_clean());
    assert!(!audit.retry_decision_recoverable_from_the_code_alone);

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

    let inverted = audit
        .divergences
        .iter()
        .find(|d| d.kind == DivergenceKind::RetryabilityInverted)
        .expect("the stale inversion is reported");
    assert_eq!(inverted.severity, AuditSeverity::Defect);
    assert_eq!(inverted.classes, vec![DiagnosticClass::Stale]);
}

#[test]
fn the_audit_reports_a_synthetic_registry_that_collapses_two_retry_decisions_as_defective() {
    let registry = a_registry_that_collapses_two_retry_decisions_onto_one_code();
    assert!(!retry_decision_is_recoverable(&registry));

    let audit = audit_registry(&registry);
    assert!(!audit.is_clean());
    assert!(!audit.retry_decision_recoverable_from_the_code_alone);
    let collision = audit
        .divergences
        .iter()
        .find(|d| d.kind == DivergenceKind::ClassCollision && d.code == 4)
        .expect("the synthetic collision on exit 4 is reported");
    assert_eq!(collision.severity, AuditSeverity::Defect);
    assert_eq!(
        collision.classes,
        vec![DiagnosticClass::Stale, DiagnosticClass::ContractViolation]
    );
}

#[test]
fn both_surviving_imprecisions_are_internal_sharing_the_io_code_with_unavailable() {
    let audit = audit();
    assert_eq!(shipped_code_for(DiagnosticClass::Internal), 5);
    assert_eq!(shipped_code_for(DiagnosticClass::Unavailable), 5);
    assert!(!code_meaning_covers(DiagnosticClass::Internal));

    let narrowing = audit
        .divergences
        .iter()
        .find(|d| d.kind == DivergenceKind::MeaningNarrowerThanClass)
        .expect("the narrowing on Internal is reported");
    assert_eq!(narrowing.severity, AuditSeverity::Imprecision);
    assert_eq!(narrowing.classes, vec![DiagnosticClass::Internal]);
    assert_eq!(narrowing.code, 5);

    let collision = audit
        .divergences
        .iter()
        .find(|d| d.kind == DivergenceKind::ClassCollision)
        .expect("the collision on exit 5 is reported");
    assert_eq!(collision.severity, AuditSeverity::Imprecision);
    assert_eq!(collision.code, 5);
    assert_eq!(
        collision.classes,
        vec![DiagnosticClass::Unavailable, DiagnosticClass::Internal]
    );
    assert_eq!(
        DiagnosticClass::Unavailable.retryability(),
        DiagnosticClass::Internal.retryability(),
        "the collision is only an imprecision because the two classes retry alike"
    );
}

#[test]
fn the_eight_classes_whose_exit_code_means_what_it_says_produce_no_finding() {
    let audit = audit();
    for class in DiagnosticClass::ALL {
        if class == DiagnosticClass::Internal {
            continue;
        }
        assert!(code_meaning_covers(class));
        assert!(
            !audit
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
fn exit_one_is_still_reported_as_a_note_and_never_as_a_defect() {
    let audit = audit();
    let row = audit
        .divergences
        .iter()
        .find(|d| d.kind == DivergenceKind::CodeOutsideTheTaxonomy)
        .expect("exit 1 is reported");
    assert_eq!(row.code, 1);
    assert_eq!(row.severity, AuditSeverity::Note);
    assert!(row.classes.is_empty());
}

#[test]
fn every_imprecision_is_a_meaning_narrowing_or_the_retryability_preserving_collision() {
    for registry in [shipped_registry(), registry_before_the_split()] {
        for divergence in audit_registry(&registry).by_severity(AuditSeverity::Imprecision) {
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
}

#[test]
fn every_divergence_names_the_distinction_a_replacement_registry_must_preserve() {
    for registry in [shipped_registry(), registry_before_the_split()] {
        for divergence in &audit_registry(&registry).divergences {
            assert!(
                divergence.required_distinction.len() > 30,
                "{:?} states no required distinction",
                divergence.kind
            );
            assert!(!divergence.consequence.trim().is_empty());
            assert!(!divergence.finding.trim().is_empty());
        }
    }
}

#[test]
fn the_audit_renders_as_diagnostics_that_pass_this_crates_own_lint() {
    let diagnostics = audit().as_diagnostics();
    assert_eq!(diagnostics.len(), 3);
    let report = lint(&diagnostics);
    assert!(
        report.is_clean(),
        "the audit fails the lint it publishes: {:?}",
        report.errors()
    );

    let historical = audit_registry(&registry_before_the_split()).as_diagnostics();
    assert_eq!(historical.len(), 8);
    assert!(lint(&historical).is_clean());
}

#[test]
fn every_audit_diagnostic_cites_the_registry_it_audited_as_its_site() {
    for diagnostic in audit().as_diagnostics() {
        assert_eq!(diagnostic.site.describe(), SHIPPED_REGISTRY_SOURCE);
        assert!(!diagnostic.remedies.is_empty());
    }
    for diagnostic in audit_registry(&registry_before_the_split()).as_diagnostics() {
        assert_eq!(
            diagnostic.site.describe(),
            registry_before_the_split().source
        );
    }
}

#[test]
fn nothing_on_the_shipped_registry_is_escalated_to_a_human_and_both_old_defects_were() {
    let escalated = audit()
        .as_diagnostics()
        .into_iter()
        .filter(|d| d.human_decision_required)
        .count();
    assert_eq!(escalated, 0);

    let historical = audit_registry(&registry_before_the_split())
        .as_diagnostics()
        .into_iter()
        .filter(|d| d.human_decision_required)
        .count();
    assert_eq!(historical, 2);
}

#[test]
fn the_transcription_declares_which_fields_it_copied_and_which_it_did_not() {
    assert_eq!(SHIPPED_REGISTRY_TRANSCRIBED_FIELDS.len(), 5);
    assert!(SHIPPED_REGISTRY_TRANSCRIBED_FIELDS.contains(&"ExitCode::is_retryable"));
    assert!(SHIPPED_REGISTRY_TRANSCRIBED_FIELDS.contains(&"ExitCode::retryability"));
    assert!(SHIPPED_REGISTRY_TRANSCRIBED_FIELDS.contains(&"ExitCode::summary"));
    assert_eq!(SHIPPED_REGISTRY_SOURCE, "crates/cli/src/exit.rs");
    for row in shipped_exit_codes() {
        assert!(!row.slug.is_empty());
        assert!(!row.meaning.is_empty());
        assert_eq!(
            row.advertised_retryable,
            row.advertised_retryability
                .is_some_and(Retryability::permits_automatic_retry),
            "exit {} transcribes a boolean that disagrees with its own three-valued decision",
            row.code
        );
    }
}

#[test]
fn the_registry_still_has_fewer_failure_codes_than_classes_and_exit_five_is_where_that_shows() {
    let audit = audit();
    assert_eq!(audit.shipped_code_count, 10);
    assert_eq!(audit.class_count, 9);
    let failure_codes = audit.shipped_code_count - 2;
    assert_eq!(failure_codes, 8);
    assert!(
        failure_codes < audit.class_count,
        "eight failure codes cannot give nine classes one each"
    );
    let doubled: Vec<DiagnosticClass> = DiagnosticClass::ALL
        .into_iter()
        .filter(|class| shipped_code_for(*class) == 5)
        .collect();
    assert_eq!(
        doubled,
        vec![DiagnosticClass::Unavailable, DiagnosticClass::Internal]
    );
}
