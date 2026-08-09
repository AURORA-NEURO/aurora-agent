//! 36.21: the one predicate the module supports.

use bioprism_bioethics::validation::{EvidenceKind, EvidenceRecord, Maturity, ValidationDossier};
use bioprism_bioethics::BioethicsError;

fn complete_dossier() -> ValidationDossier {
    let mut dossier = ValidationDossier::new("bioprism-fiber::compile", "author-a");
    for kind in EvidenceKind::ALL {
        let attested_by = if kind == EvidenceKind::IndependentReproduction {
            "reproducer-b"
        } else {
            "author-a"
        };
        dossier = dossier.with(EvidenceRecord::new(
            kind,
            "a checkable pointer",
            attested_by,
        ));
    }
    dossier
}

#[test]
fn a_dossier_missing_an_evidence_kind_cannot_be_verified() {
    let mut dossier = ValidationDossier::new("bioprism-fiber::compile", "author-a");
    for kind in EvidenceKind::ALL {
        if kind == EvidenceKind::ScientificValidation {
            continue;
        }
        dossier = dossier.with(EvidenceRecord::new(kind, "a checkable pointer", "author-a"));
    }
    assert_eq!(dossier.missing(), vec![EvidenceKind::ScientificValidation]);
    let error = dossier
        .verify()
        .expect_err("absent evidence is not satisfied evidence");
    match error {
        BioethicsError::UnmetValidationEvidence { missing, .. } => {
            assert_eq!(missing, "scientific_validation");
        }
        other => panic!("expected the missing kind to be named: {other}"),
    }
}

#[test]
fn a_blank_reference_counts_as_missing_rather_than_as_present() {
    let mut dossier = complete_dossier();
    dossier = dossier.with(EvidenceRecord::new(
        EvidenceKind::ChangeControl,
        "   ",
        "author-a",
    ));
    assert_eq!(dossier.missing(), vec![EvidenceKind::ChangeControl]);
    assert!(
        dossier.verify().is_err(),
        "a ticked row with no pointer is how a validation file passes without anyone looking"
    );
}

#[test]
fn a_module_reproduced_by_its_own_author_cannot_be_verified() {
    let mut dossier = complete_dossier();
    dossier = dossier.with(EvidenceRecord::new(
        EvidenceKind::IndependentReproduction,
        "a checkable pointer",
        "author-a",
    ));
    let error = dossier
        .verify()
        .expect_err("structural non-identity is the only independence this crate can check");
    assert!(matches!(error, BioethicsError::ReproducerIsAuthor { .. }));
}

#[test]
fn a_complete_dossier_with_an_independent_reproducer_verifies() {
    let verified = complete_dossier()
        .verify()
        .expect("every evidence kind is present and the reproducer is not the author");
    assert_eq!(verified.subject(), "bioprism-fiber::compile");
    assert_eq!(verified.author(), "author-a");
    assert_eq!(verified.reproduced_by(), "reproducer-b");
    assert_eq!(verified.maturity(), Maturity::Verified);
    assert_eq!(
        verified.evidence(EvidenceKind::DesignReview).reference,
        "a checkable pointer"
    );
}

#[test]
fn an_incomplete_dossier_reports_experimental_and_a_complete_one_reports_verified() {
    assert_eq!(
        ValidationDossier::new("nothing-filed", "author-a").maturity(),
        Maturity::Experimental
    );
    assert_eq!(complete_dossier().maturity(), Maturity::Verified);
}

#[test]
fn reading_the_maturity_is_not_the_same_as_holding_a_verified_module() {
    let mut dossier = complete_dossier();
    dossier = dossier.with(EvidenceRecord::new(
        EvidenceKind::IndependentReproduction,
        "a checkable pointer",
        "author-a",
    ));
    assert_eq!(
        dossier.maturity(),
        Maturity::Verified,
        "every evidence row is filled, so the coverage question passes"
    );
    assert!(
        dossier.verify().is_err(),
        "and the independence question still fails, which is why the two are separate calls"
    );
}

#[test]
fn the_missing_list_is_in_blueprint_order_rather_than_in_dossier_order() {
    let dossier = ValidationDossier::new("nothing-filed", "author-a").with(EvidenceRecord::new(
        EvidenceKind::ChangeControl,
        "a checkable pointer",
        "author-a",
    ));
    let missing = dossier.missing();
    assert_eq!(
        missing.first(),
        Some(&EvidenceKind::RequirementsAndRiskFile)
    );
    assert_eq!(missing.len(), 6);
    assert!(!missing.contains(&EvidenceKind::ChangeControl));
}

#[test]
fn the_ladder_has_only_the_two_rungs_the_module_names() {
    assert_eq!(
        EvidenceKind::ALL.len(),
        7,
        "36.21's scope lists seven kinds"
    );
    assert_eq!(Maturity::Experimental.as_str(), "experimental");
    assert_eq!(Maturity::Verified.as_str(), "verified");
    assert!(
        Maturity::Experimental < Maturity::Verified,
        "the two rungs are ordered and nothing sits between them"
    );
}
