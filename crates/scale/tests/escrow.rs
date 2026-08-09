//! Blueprint 35.05. A reveal must prove the committed value was fixed before the run.

use bioprism_scale::error::EscrowError;
use bioprism_scale::escrow::{commit_digest, EscrowState, EscrowVault, RevealCondition, Sequence};
use serde_json::json;

const SYSTEM: &str = "system-digest-abc";

fn outcome() -> serde_json::Value {
    json!({ "endpoint": "responder", "months": 18 })
}

#[test]
fn an_escrow_revealed_before_its_condition_is_refused() {
    let mut vault = EscrowVault::new();
    vault
        .seal(
            "e1",
            outcome(),
            "salt-1",
            SYSTEM,
            RevealCondition::WhenRunCompleted { run: "r1".into() },
        )
        .unwrap();
    vault.register_run("r1");

    match vault.reveal("e1", "r1", SYSTEM) {
        Err(EscrowError::ConditionNotMet { escrow, condition, .. }) => {
            assert_eq!(escrow, "e1");
            assert!(condition.contains("r1"));
        }
        other => panic!("an early reveal must be refused: {other:?}"),
    }

    vault.complete_run("r1").unwrap();
    let reveal = vault.reveal("e1", "r1", SYSTEM).unwrap();
    assert_eq!(reveal.payload, outcome());
}

#[test]
fn an_escrow_sealed_after_the_run_began_cannot_be_revealed_against_it() {
    let mut vault = EscrowVault::new();
    vault.register_run("r1");
    vault.complete_run("r1").unwrap();
    vault
        .seal(
            "late",
            outcome(),
            "salt",
            SYSTEM,
            RevealCondition::WhenRunCompleted { run: "r1".into() },
        )
        .unwrap();

    match vault.reveal("late", "r1", SYSTEM) {
        Err(EscrowError::CommitmentNotPriorToRun {
            sealed_at, run_at, ..
        }) => assert!(sealed_at > run_at),
        other => panic!("a commitment that does not precede the run proves nothing: {other:?}"),
    }
}

#[test]
fn a_reveal_verifies_from_its_own_contents() {
    let mut vault = EscrowVault::new();
    let record = vault
        .seal(
            "e1",
            outcome(),
            "salt-1",
            SYSTEM,
            RevealCondition::WhenOutcomeRecorded {
                outcome: "18-month-follow-up".into(),
            },
        )
        .unwrap();
    vault.register_run("r1");
    vault.record_outcome("18-month-follow-up");

    let reveal = vault.reveal("e1", "r1", SYSTEM).unwrap();
    reveal.verify().expect("an honest reveal verifies");
    assert_eq!(reveal.commitment, record.commitment);
    assert!(reveal.sealed_at < reveal.run_registered_at);
    assert!(reveal.run_registered_at < reveal.revealed_at);
}

#[test]
fn a_tampered_payload_fails_verification() {
    let mut vault = EscrowVault::new();
    vault
        .seal(
            "e1",
            outcome(),
            "salt-1",
            SYSTEM,
            RevealCondition::AtOrAfter(Sequence(1)),
        )
        .unwrap();
    vault.register_run("r1");

    let mut reveal = vault.reveal("e1", "r1", SYSTEM).unwrap();
    reveal.payload = json!({ "endpoint": "non-responder", "months": 18 });

    assert!(matches!(
        reveal.verify(),
        Err(EscrowError::CommitmentMismatch { .. })
    ));
}

#[test]
fn a_forged_ordering_fails_verification_even_with_a_valid_commitment() {
    let mut vault = EscrowVault::new();
    vault
        .seal(
            "e1",
            outcome(),
            "salt-1",
            SYSTEM,
            RevealCondition::AtOrAfter(Sequence(1)),
        )
        .unwrap();
    vault.register_run("r1");

    let mut reveal = vault.reveal("e1", "r1", SYSTEM).unwrap();
    reveal.run_registered_at = Sequence(0);

    assert!(matches!(
        reveal.verify(),
        Err(EscrowError::CommitmentNotPriorToRun { .. })
    ));
}

#[test]
fn the_audit_trail_never_contains_the_payload() {
    let mut vault = EscrowVault::new();
    vault
        .seal(
            "e1",
            json!({ "secret": "the-answer-is-responder" }),
            "salt-1",
            SYSTEM,
            RevealCondition::AtOrAfter(Sequence(9)),
        )
        .unwrap();

    let encoded = serde_json::to_string(&vault.audit_trail()).unwrap();
    assert!(!encoded.contains("the-answer-is-responder"));
    assert!(!encoded.contains("salt-1"));
    assert!(encoded.contains("commitment"));
    assert!(encoded.contains("sealed_at"));
}

#[test]
fn the_same_payload_under_different_salts_commits_differently() {
    let a = commit_digest(&outcome(), "salt-a").unwrap();
    let b = commit_digest(&outcome(), "salt-b").unwrap();
    assert_ne!(
        a, b,
        "an unsalted commitment to a one-bit outcome is a commitment to nothing"
    );
    assert_eq!(a, commit_digest(&outcome(), "salt-a").unwrap());
}

#[test]
fn a_system_changed_after_the_freeze_cannot_claim_the_blind_result() {
    let mut vault = EscrowVault::new();
    vault
        .seal(
            "e1",
            outcome(),
            "salt",
            SYSTEM,
            RevealCondition::AtOrAfter(Sequence(1)),
        )
        .unwrap();
    vault.register_run("r1");

    match vault.reveal("e1", "r1", "system-digest-retuned") {
        Err(EscrowError::SystemNotFrozen {
            frozen, presented, ..
        }) => {
            assert_eq!(frozen, SYSTEM);
            assert_eq!(presented, "system-digest-retuned");
        }
        other => panic!("the freeze is the point of a prospective tier: {other:?}"),
    }
}

#[test]
fn an_escrow_reveals_exactly_once() {
    let mut vault = EscrowVault::new();
    vault
        .seal(
            "e1",
            outcome(),
            "salt",
            SYSTEM,
            RevealCondition::AtOrAfter(Sequence(1)),
        )
        .unwrap();
    vault.register_run("r1");

    vault.reveal("e1", "r1", SYSTEM).unwrap();
    assert!(matches!(
        vault.reveal("e1", "r1", SYSTEM),
        Err(EscrowError::AlreadyRevealed(id)) if id == "e1"
    ));
    assert_eq!(vault.record("e1").unwrap().state, EscrowState::Revealed);
}

#[test]
fn a_commitment_cannot_be_re_sealed() {
    let mut vault = EscrowVault::new();
    vault
        .seal("e1", outcome(), "salt", SYSTEM, RevealCondition::AtOrAfter(Sequence(1)))
        .unwrap();
    assert!(matches!(
        vault.seal(
            "e1",
            json!({ "endpoint": "non-responder" }),
            "salt",
            SYSTEM,
            RevealCondition::AtOrAfter(Sequence(1))
        ),
        Err(EscrowError::AlreadySealed(id)) if id == "e1"
    ));
}

#[test]
fn a_voided_escrow_is_permanently_unrevealable_and_still_listed() {
    let mut vault = EscrowVault::new();
    vault
        .seal("e1", outcome(), "salt", SYSTEM, RevealCondition::AtOrAfter(Sequence(1)))
        .unwrap();
    vault.register_run("r1");
    vault.void("e1", "18-month follow-up never arrived").unwrap();

    match vault.reveal("e1", "r1", SYSTEM) {
        Err(EscrowError::Voided { reason, .. }) => assert!(reason.contains("follow-up")),
        other => panic!("a voided payload must stay sealed forever: {other:?}"),
    }

    let trail = vault.audit_trail();
    assert_eq!(trail.len(), 1, "35's failure containment marks, it does not delete");
    assert_eq!(trail[0].state, EscrowState::Voided);
    assert!(trail[0].void_reason.is_some());
}

#[test]
fn revealing_against_an_unregistered_run_has_no_ordering_witness() {
    let mut vault = EscrowVault::new();
    vault
        .seal("e1", outcome(), "salt", SYSTEM, RevealCondition::AtOrAfter(Sequence(1)))
        .unwrap();

    assert!(matches!(
        vault.reveal("e1", "never-registered", SYSTEM),
        Err(EscrowError::UnknownRun(run)) if run == "never-registered"
    ));
}

#[test]
fn the_vault_clock_is_monotone_and_owned_by_the_vault() {
    let mut vault = EscrowVault::new();
    let start = vault.now();
    let a = vault.advance();
    let b = vault.register_run("r1");
    let c = vault.record_outcome("follow-up");
    assert!(start < a && a < b && b < c);
}
