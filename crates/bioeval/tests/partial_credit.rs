//! Partial credit that names, and can re-derive, the rule that produced it (26.02, 26.14).

mod common;

use bioprism_bioeval::{
    BiologicalErrorClass, Credit, CreditError, CreditEvidence, CreditRule, CreditTerm,
};

fn term(id: &str, weight: f64) -> CreditTerm {
    CreditTerm {
        term_id: id.to_string(),
        weight,
        criterion: format!("the output satisfies {id}"),
    }
}

fn rubric(version: u32) -> CreditRule {
    CreditRule::new(
        "bioeval/effect-estimate/1",
        version,
        vec![
            term("names_the_gene", 3.0),
            term("right_direction", 2.0),
            term("magnitude_within_2x", 1.0),
        ],
    )
    .expect("the fixture rubric has positive weights")
}

#[test]
fn credit_carries_the_rule_that_produced_it() {
    let evidence = CreditEvidence::satisfying(["names_the_gene".to_string()]);
    let credit = rubric(1).award(&evidence).expect("no critical error");

    assert_eq!(credit.rule_id(), "bioeval/effect-estimate/1");
    assert_eq!(credit.rule_version(), 1);
    assert_eq!(
        credit.basis().len(),
        3,
        "every term is on the record, earned or not"
    );
    assert!(credit.basis().iter().any(|t| !t.satisfied));
}

#[test]
fn credit_is_the_weighted_share_of_the_terms_that_were_satisfied() {
    let evidence =
        CreditEvidence::satisfying(["names_the_gene".to_string(), "right_direction".to_string()]);
    let credit = rubric(1).award(&evidence).expect("no critical error");

    assert!((credit.fraction() - 5.0 / 6.0).abs() < 1e-12);
}

#[test]
fn the_same_rule_on_the_same_evidence_produces_the_same_digest() {
    let evidence = CreditEvidence::satisfying(["right_direction".to_string()]);

    let first = rubric(1).award(&evidence).expect("no critical error");
    let second = rubric(1).award(&evidence).expect("no critical error");

    assert_eq!(first.digest(), second.digest());
    assert_eq!(first, second);
    assert!(first.verify(&rubric(1), &evidence));
}

#[test]
fn a_credit_does_not_verify_against_a_different_version_of_its_rule() {
    let evidence = CreditEvidence::satisfying(["right_direction".to_string()]);
    let credit = rubric(1).award(&evidence).expect("no critical error");

    assert!(
        !credit.verify(&rubric(2), &evidence),
        "an edited rubric is a new rubric, and previously published credit does not carry over"
    );
}

#[test]
fn a_credit_does_not_verify_against_different_evidence() {
    let awarded_on = CreditEvidence::satisfying(["right_direction".to_string()]);
    let credit = rubric(1).award(&awarded_on).expect("no critical error");

    let claimed_on =
        CreditEvidence::satisfying(["right_direction".to_string(), "names_the_gene".to_string()]);
    assert!(!credit.verify(&rubric(1), &claimed_on));
}

#[test]
fn a_critical_error_forfeits_partial_credit_rather_than_scaling_it_down() {
    let evidence = CreditEvidence::satisfying([
        "names_the_gene".to_string(),
        "right_direction".to_string(),
        "magnitude_within_2x".to_string(),
    ])
    .with_error(BiologicalErrorClass::SpecimenIdentity);

    let refusal = rubric(1)
        .award(&evidence)
        .expect_err("a result about the wrong specimen has no meaningful remainder");

    assert!(matches!(
        refusal,
        CreditError::CriticalErrorForfeitsCredit {
            class: BiologicalErrorClass::SpecimenIdentity
        }
    ));
}

#[test]
fn an_unclassified_error_cannot_earn_credit() {
    let evidence = CreditEvidence::satisfying(["names_the_gene".to_string()])
        .with_error(BiologicalErrorClass::Unclassified);

    let refusal = rubric(1)
        .award(&evidence)
        .expect_err("credit for an unexamined failure is credit whose rule cannot be stated");

    assert!(matches!(refusal, CreditError::UnclassifiedError { .. }));
}

#[test]
fn a_benign_error_still_admits_credit() {
    let evidence =
        CreditEvidence::satisfying(["names_the_gene".to_string(), "right_direction".to_string()])
            .with_error(BiologicalErrorClass::MagnitudeRightDirection);

    let credit = rubric(1)
        .award(&evidence)
        .expect("right direction, wrong size, leaves the sign standing");

    assert!(credit.fraction() > 0.0);
}

#[test]
fn a_rule_with_no_terms_is_rejected_at_construction() {
    let refusal = CreditRule::new("bioeval/empty/1", 1, Vec::new())
        .expect_err("a rubric with nothing in it awards credit for nothing");

    assert!(matches!(refusal, CreditError::NoBasis { .. }));
}

#[test]
fn a_rule_with_a_non_positive_weight_is_rejected_at_construction() {
    let refusal = CreditRule::new("bioeval/bad-weight/1", 1, vec![term("t", 0.0)])
        .expect_err("a zero-weight term is a term that cannot be earned");

    assert!(matches!(refusal, CreditError::FractionOutOfRange { .. }));
}

#[test]
fn a_deserialised_credit_proves_nothing_until_its_digest_re_derives() {
    let evidence = CreditEvidence::satisfying(["names_the_gene".to_string()]);
    let credit = rubric(1).award(&evidence).expect("no critical error");

    let encoded = serde_json::to_string(&credit).expect("credit serialises");
    let forged = encoded.replace("\"fraction\":0.5", "\"fraction\":1.0");
    let decoded: Credit = serde_json::from_str(&forged).expect("credit deserialises");

    assert!(
        (decoded.fraction() - 1.0).abs() < 1e-12,
        "serde will happily reconstruct the inflated number"
    );
    assert!(
        !decoded.verify(&rubric(1), &evidence),
        "and the digest is what refuses it"
    );
}
