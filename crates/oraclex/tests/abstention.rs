//! The spine: an oracle that cannot decide must abstain, and abstention is not support.
//!
//! `crates/bioworlds` found a v0.1 oracle returning `valid` on a world that genuinely
//! underdetermines its question. These tests are the standing check that this crate cannot do the
//! same by accident.

use bioprism_oraclex::identity::{decide, IdentityClaim, IdentitySignal, SpecimenRef};
use bioprism_oraclex::verdict::{Contradiction, Determination, Missing, NotEvaluable, Unresolved};
use bioprism_oraclex::OracleXError;
use bioprism_oracle::{EvidenceTier, Position};

#[test]
fn an_unresolved_determination_cannot_be_built_without_naming_what_is_missing() {
    let empty: Vec<Missing> = Vec::new();
    assert_eq!(
        Unresolved::new(empty),
        Err(OracleXError::UnresolvedWithoutMissingEvidence)
    );
}

#[test]
fn a_not_evaluable_determination_cannot_be_built_without_a_reason() {
    assert_eq!(
        NotEvaluable::new("   "),
        Err(OracleXError::NotEvaluableWithoutReason)
    );
}

#[test]
fn a_contradiction_cannot_be_built_without_a_witness() {
    assert_eq!(
        Contradiction::new(EvidenceTier::Deterministic, []),
        Err(OracleXError::ContradictionWithoutFinding)
    );
}

#[test]
fn an_unresolved_determination_cannot_be_deserialised_empty() {
    let json = r#"{"determination":"unresolved","missing":[]}"#;
    let parsed: Result<Determination, _> = serde_json::from_str(json);
    assert!(
        parsed.is_err(),
        "an empty missing set must not survive deserialisation: {parsed:?}"
    );
}

#[test]
fn a_not_evaluable_determination_cannot_be_deserialised_without_a_reason() {
    let json = r#"{"determination":"not_evaluable","reason":""}"#;
    let parsed: Result<Determination, _> = serde_json::from_str(json);
    assert!(parsed.is_err(), "an empty reason must not deserialise");
}

#[test]
fn an_abstention_is_not_supported() {
    let unresolved = Determination::unresolved("a fingerprint", "none was run");
    let not_evaluable = Determination::not_evaluable("out of scope");
    assert!(!unresolved.is_supported());
    assert!(!not_evaluable.is_supported());
    assert!(unresolved.is_abstention());
    assert!(not_evaluable.is_abstention());
    assert!(!unresolved.decided());
}

#[test]
fn an_abstention_carries_no_evidence_tier() {
    assert_eq!(
        Determination::unresolved("a fingerprint", "none was run").tier(),
        None,
        "an abstention is the absence of evidence, not weak evidence"
    );
    assert_eq!(Determination::not_evaluable("out of scope").tier(), None);
}

#[test]
fn an_abstention_projects_onto_the_mesh_without_becoming_a_position_of_support() {
    assert_eq!(
        Determination::unresolved("a fingerprint", "none was run").position(),
        Position::Unresolved
    );
    assert_eq!(
        Determination::not_evaluable("out of scope").position(),
        Position::NotEvaluable
    );
}

#[test]
fn an_oracle_that_cannot_decide_does_not_return_valid() {
    let left = SpecimenRef::new("P1", "A1", "T0");
    let right = SpecimenRef::new("P1", "A2", "T1");
    let signals = [IdentitySignal::TextualCrosswalk {
        join_key: "truncated_barcode".into(),
        agrees: true,
    }];

    let determination = decide(IdentityClaim::SameSubject, &left, &right, &signals);

    assert!(
        !determination.is_supported(),
        "a textual join that agrees is the absence of a test, not confirmation"
    );
    assert_eq!(determination.position(), Position::Unresolved);
    assert!(
        determination
            .missing()
            .iter()
            .any(|gap| gap.evidence.contains("molecular")),
        "the abstention must name molecular evidence as the gap, got {:?}",
        determination.missing()
    );
}

#[test]
fn every_abstention_produced_by_the_crate_names_a_gap_or_a_reason() {
    let left = SpecimenRef::new("P1", "A1", "T0");
    let right = SpecimenRef::new("P1", "A2", "T1");

    let abstentions = [
        decide(IdentityClaim::SameSubject, &left, &right, &[]),
        decide(
            IdentityClaim::SameSubject,
            &left,
            &right,
            &[IdentitySignal::TextualCrosswalk {
                join_key: "id".into(),
                agrees: true,
            }],
        ),
        bioprism_oraclex::perturbation::positivity(&[]),
        bioprism_oraclex::missing::complete_case_admissible(
            &bioprism_oraclex::missing::MissingnessMechanism::Undeclared,
        ),
    ];

    for determination in abstentions {
        match &determination {
            Determination::Unresolved(unresolved) => {
                assert!(!unresolved.missing().is_empty());
            }
            Determination::NotEvaluable(not_evaluable) => {
                assert!(!not_evaluable.reason().trim().is_empty());
            }
            other => panic!("expected an abstention, got {other:?}"),
        }
    }
}

