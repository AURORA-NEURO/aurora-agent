//! Error-message quality held against everything this crate can emit.
//!
//! The lint is only worth having if it is run over the crate's whole emittable surface rather than
//! over the catalogue alone. These tests assemble every diagnostic `bioprism-devx` can produce —
//! the catalogue, the exit-code audit, the SDK error conversions and the compile-record
//! diagnostics — and grade the lot.

use bioprism_devx::catalogue::{catalogue, diagnose_record, from_manifest_error};
use bioprism_devx::diagnostic::{Certainty, Diagnostic, DiagnosticCode, Remedy, Site};
use bioprism_devx::exitaudit::audit;
use bioprism_devx::introspect::{CompileRecord, PassOutcome, PassRecord};
use bioprism_devx::lint::{lint, LintRule, LintSeverity};
use bioprism_devx::taxonomy::{ChangeRequired, DiagnosticClass};
use bioprism_sdk::ManifestError;
use bioprism_section::{InfluenceClass, OmissionGroup, OmissionManifest};

fn everything_this_crate_can_emit() -> Vec<Diagnostic> {
    let mut manifest = OmissionManifest::default();
    manifest.push(OmissionGroup {
        reason: "not analysed".into(),
        influence: InfluenceClass::Unknown,
        count: 3,
        bound: None,
        examples: Vec::new(),
    });
    manifest.push(OmissionGroup {
        reason: "withheld by consent".into(),
        influence: InfluenceClass::InaccessibleByPolicy,
        count: 1,
        bound: None,
        examples: Vec::new(),
    });
    let record = CompileRecord::new("world-1", "query-1")
        .with_manifest(manifest)
        .with_pass(PassRecord::new(
            "obstruction_tests",
            PassOutcome::Deferred {
                reason: String::new(),
            },
            "nothing to do",
        ));

    let mut all = catalogue();
    all.extend(audit().as_diagnostics());
    all.extend(diagnose_record(&record));
    all.push(from_manifest_error(
        &ManifestError::AdapterWithoutLossDeclaration {
            plugin: "acme-adapter".into(),
        },
        "acme-adapter",
    ));
    all
}

#[test]
fn nothing_this_crate_can_emit_fails_a_structural_lint_rule() {
    let diagnostics = everything_this_crate_can_emit();
    let report = lint(&diagnostics);
    assert!(
        report.is_clean(),
        "structural lint errors: {:?}",
        report.errors()
    );
    assert!(
        diagnostics.len() > catalogue().len(),
        "a lint graded over the catalogue alone is a lint for one module; the audit, the compile \
         record and the SDK conversions must all be in the set"
    );
    assert!(diagnostics.len() >= 25);
}

#[test]
fn every_emittable_diagnostic_states_at_least_one_remedy() {
    for diagnostic in everything_this_crate_can_emit() {
        assert!(
            !diagnostic.remedies.is_empty(),
            "{} states no remedy",
            diagnostic.code
        );
    }
}

#[test]
fn every_emittable_remedy_says_how_you_would_know_it_worked() {
    for diagnostic in everything_this_crate_can_emit() {
        for remedy in &diagnostic.remedies {
            assert!(
                remedy.verified_by.len() > 5,
                "{} offers an unverifiable remedy",
                diagnostic.code
            );
        }
    }
}

#[test]
fn no_emittable_remedy_is_asserted_more_confidently_than_the_finding_it_repairs() {
    for diagnostic in everything_this_crate_can_emit() {
        for remedy in &diagnostic.remedies {
            assert!(
                !(remedy.confidence.at_least_as_strong_as(diagnostic.certainty)
                    && remedy.confidence != diagnostic.certainty),
                "{} repairs a {} finding with an {} remedy",
                diagnostic.code,
                diagnostic.certainty,
                remedy.confidence
            );
        }
    }
}

#[test]
fn every_emittable_diagnostic_names_a_surface_that_must_change() {
    for diagnostic in everything_this_crate_can_emit() {
        assert_ne!(
            diagnostic.change_required(),
            ChangeRequired::Unknown,
            "{} does not say which surface must change",
            diagnostic.code
        );
    }
}

#[test]
fn a_diagnostic_with_no_stated_remedy_fails_the_catalogue_lint() {
    let bare = Diagnostic::new(
        DiagnosticCode::parse("DEVX-0099").expect("code"),
        DiagnosticClass::ContractViolation,
        "the mandatory closure is a subset of the selection",
        "3 protected facts were dropped",
        Site::Artifact {
            node_kind: "query".into(),
            id: "q-1".into(),
        },
    )
    .citing("43.13");
    let report = lint(&[bare]);
    assert!(!report.is_clean());
    assert_eq!(report.of_rule(LintRule::MissingRemedy).len(), 1);
}

#[test]
fn the_lint_rejects_a_remedy_that_cannot_be_verified_even_when_everything_else_is_present() {
    let diagnostic = Diagnostic::new(
        DiagnosticCode::parse("DEVX-0098").expect("code"),
        DiagnosticClass::InvalidInput,
        "every protected tag names at least one fact in the world",
        "the tag matched nothing",
        Site::Source {
            document: "query.json".into(),
            span: None,
        },
    )
    .citing("43.13")
    .with_remedy(Remedy::new(
        "remove the tag",
        Site::Source {
            document: "query.json".into(),
            span: None,
        },
        "",
        ChangeRequired::Payload,
        Certainty::Observed,
    ));
    let report = lint(&[diagnostic]);
    assert_eq!(
        report.of_rule(LintRule::RemedyWithoutVerification).len(),
        1
    );
    assert!(!report.is_clean());
}

#[test]
fn the_only_warnings_this_crate_accepts_are_the_two_documented_heuristic_false_positives() {
    let report = lint(&everything_this_crate_can_emit());
    let warnings = report.of_severity(LintSeverity::Warning);
    let codes: Vec<&str> = warnings.iter().map(|f| f.code.as_str()).collect();
    assert_eq!(codes, vec!["DEVX-0001", "DEVX-0014"]);
    assert!(warnings
        .iter()
        .all(|f| f.rule == LintRule::RemedyIsNotAnInstruction));
}

#[test]
fn the_lint_is_deterministic_and_order_independent_in_its_finding_set() {
    let mut forwards = everything_this_crate_can_emit();
    let mut backwards = forwards.clone();
    backwards.reverse();
    let a = lint(&forwards);
    let b = lint(&backwards);
    assert_eq!(a.findings, b.findings);
    forwards.truncate(1);
    assert_eq!(lint(&forwards).checked, 1);
}

#[test]
fn every_universal_2332_requirement_is_discharged_by_every_emittable_diagnostic() {
    for diagnostic in everything_this_crate_can_emit() {
        let unmet = diagnostic.unmet_universal_requirements();
        assert!(
            unmet.is_empty(),
            "{} leaves {:?} undischarged",
            diagnostic.code,
            unmet.iter().map(|r| r.phrase()).collect::<Vec<_>>()
        );
    }
}

#[test]
fn a_diagnostic_that_hedges_while_claiming_observation_is_warned_and_not_failed() {
    let mut diagnostic = catalogue()
        .into_iter()
        .next()
        .expect("the catalogue is non-empty");
    diagnostic.observed = "the budget is probably below the closure".into();
    let report = lint(&[diagnostic]);
    assert_eq!(report.of_rule(LintRule::CertaintyContradictsProse).len(), 1);
    assert!(report.is_clean());
}
