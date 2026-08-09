//! Invariants of 06.08 oracle synthesis and review.
//!
//! The claim these tests defend is that an unreviewed oracle cannot grade. Part of that claim is
//! enforced by the type system and is therefore untestable at runtime — `ProposedOracle` has no
//! `grade` method, so a test that called it would not compile — and the rest is enforced at the
//! gate, which is what is exercised here.

use bioprism_benchcompiler::minimize::{minimize, ContextItem, InterestSignature, MinimizeBudget, Tier};
use bioprism_benchcompiler::oracle::{synthesise, ExploitAttempt, OracleStrength, ProposedOracle};
use bioprism_benchcompiler::OracleError;
use bioprism_prism::{Acceptance, InputRef};
use serde_json::json;
use std::collections::BTreeSet;

fn sound_proposal() -> ProposedOracle {
    ProposedOracle::new(
        "or_leakage",
        "step 7: chose the split without checking alias overlap",
        OracleStrength::ExactStatePredicate,
    )
    .accepting("invalid")
    .requiring_witness("identity_leakage")
    .seeing("the frozen world and the candidate's verdict")
    .blind_to("anything the candidate did after emitting its verdict")
}

fn witnesses(names: &[&str]) -> BTreeSet<String> {
    names.iter().map(|name| (*name).to_string()).collect()
}

#[test]
fn a_proposal_reaches_a_grade_only_through_review() {
    let reviewed = sound_proposal()
        .review("k.okafor")
        .expect("a complete proposal passes the gate");
    assert_eq!(reviewed.reviewer(), "k.okafor");
    assert_eq!(
        reviewed.grade("invalid", &witnesses(&["identity_leakage"]), true),
        Acceptance::Passed
    );
}

#[test]
fn review_without_a_named_reviewer_is_refused() {
    assert_eq!(
        sound_proposal().review("   ").unwrap_err(),
        OracleError::UnattributedReview
    );
}

#[test]
fn an_oracle_with_no_declared_blind_spots_cannot_be_reviewed() {
    let proposal = ProposedOracle::new("or_bare", "step 3", OracleStrength::ExactStatePredicate)
        .accepting("valid");
    assert!(matches!(
        proposal.review("k.okafor").unwrap_err(),
        OracleError::NoGapAnalysis { .. }
    ));
}

#[test]
fn a_successful_exploit_blocks_review_and_no_reviewer_can_clear_it() {
    let proposal = sound_proposal().attacked_with(
        ExploitAttempt::new(
            "hardcoded_verdict",
            "emit `invalid` without reading the world",
        )
        .scored_as_pass(),
    );
    assert_eq!(
        proposal.review("k.okafor").unwrap_err(),
        OracleError::UnrebuttedExploit {
            attack: "hardcoded_verdict".to_string()
        }
    );
}

#[test]
fn an_exploit_that_actually_did_the_task_does_not_block_review() {
    let proposal = sound_proposal().attacked_with(
        ExploitAttempt::new("shortcut", "reached the verdict by a cheaper but valid route")
            .scored_as_pass()
            .fulfils_task_intent(),
    );
    assert!(proposal.successful_exploits().is_empty());
    assert!(proposal.review("k.okafor").is_ok());
}

#[test]
fn a_model_judge_with_no_deterministic_companion_is_refused() {
    let proposal = ProposedOracle::new("or_judge", "step 7", OracleStrength::ModelJudge)
        .accepting("valid")
        .blind_to("the candidate's internal reasoning");
    assert_eq!(
        proposal.review("k.okafor").unwrap_err(),
        OracleError::WeakOracleAlone {
            oracle: "or_judge".to_string(),
            strength: "model_judge"
        }
    );
}

#[test]
fn a_model_judge_paired_with_a_deterministic_check_passes_review() {
    let proposal = ProposedOracle::new("or_judge", "step 7", OracleStrength::ModelJudge)
        .accepting("valid")
        .blind_to("the candidate's internal reasoning")
        .paired_with("or_leakage");
    assert!(proposal.review("k.okafor").is_ok());
}

#[test]
fn an_oracle_that_accepts_nothing_is_refused() {
    let proposal = ProposedOracle::new("or_empty", "step 7", OracleStrength::ExactStatePredicate)
        .blind_to("everything after the verdict");
    assert!(matches!(
        proposal.review("k.okafor").unwrap_err(),
        OracleError::EmptyAcceptanceSet { .. }
    ));
}

#[test]
fn a_right_answer_from_an_incomplete_basis_is_not_a_pass() {
    let reviewed = sound_proposal().review("k.okafor").expect("reviewable");
    assert_eq!(
        reviewed.grade("invalid", &witnesses(&["identity_leakage"]), false),
        Acceptance::ClosureIncomplete
    );
}

#[test]
fn a_wrong_verdict_and_a_missing_witness_are_reported_differently() {
    let reviewed = sound_proposal().review("k.okafor").expect("reviewable");
    assert_eq!(
        reviewed.grade("valid", &witnesses(&["identity_leakage"]), true),
        Acceptance::WrongVerdict {
            observed: "valid".to_string()
        }
    );
    assert_eq!(
        reviewed.grade("invalid", &witnesses(&[]), true),
        Acceptance::MissingWitnesses(vec!["identity_leakage".to_string()])
    );
}

#[test]
fn the_review_digest_changes_when_the_reviewer_changes() {
    let first = sound_proposal().review("k.okafor").expect("reviewable");
    let second = sound_proposal().review("r.mensah").expect("reviewable");
    assert_ne!(first.review_digest(), second.review_digest());
}

#[test]
fn a_reviewed_oracle_publishes_its_reviewer_and_digest() {
    let reviewed = sound_proposal().review("k.okafor").expect("reviewable");
    let encoded = serde_json::to_value(&reviewed).expect("serialisable");
    assert_eq!(encoded["reviewer"], json!("k.okafor"));
    assert_eq!(
        encoded["review_digest"],
        json!(reviewed.review_digest().to_string())
    );
}

#[test]
fn a_reviewed_contract_is_carried_into_the_prism_cell_unchanged() {
    let reviewed = sound_proposal().review("k.okafor").expect("reviewable");
    let world = InputRef::new("world.json", &json!({"facts": []}));
    let query = InputRef::new("query.json", &json!({"variable": "split"}));
    let cell = reviewed.into_cell("dc_leakage", world, query);

    assert!(cell.acceptable_verdicts.contains("invalid"));
    assert!(cell.required_witnesses.contains("identity_leakage"));
    assert_eq!(
        cell.decision_point,
        "step 7: chose the split without checking alias overlap"
    );
}

#[test]
fn synthesis_drops_to_a_weaker_rung_when_the_preserved_signature_has_no_witnesses() {
    let items = vec![ContextItem::new("a", Tier::Field)];
    let mut probe = |kept: &BTreeSet<String>| {
        if kept.contains("a") {
            InterestSignature::new("invalid")
        } else {
            InterestSignature::new("valid")
        }
    };
    let minimization = minimize(&items, &mut probe, MinimizeBudget::default()).expect("interesting");

    let proposal = synthesise("or_weak", "step 4", &minimization);
    assert_eq!(proposal.strength, OracleStrength::TrajectoryConstraint);
    assert!(proposal
        .blind_spots
        .iter()
        .any(|spot| spot.contains("not the reason for it")));
}

#[test]
fn synthesis_records_the_context_minimization_removed_as_a_blind_spot() {
    let items = vec![
        ContextItem::new("keep", Tier::Field),
        ContextItem::new("drop_one", Tier::Field),
        ContextItem::new("drop_two", Tier::Field),
    ];
    let mut probe = |kept: &BTreeSet<String>| {
        if kept.contains("keep") {
            InterestSignature::new("invalid").with_witness("identity_leakage")
        } else {
            InterestSignature::new("valid")
        }
    };
    let minimization = minimize(&items, &mut probe, MinimizeBudget::default()).expect("interesting");

    let proposal = synthesise("or_strong", "step 4", &minimization);
    assert_eq!(proposal.strength, OracleStrength::ExactStatePredicate);
    assert!(proposal.required_witnesses.contains("identity_leakage"));
    assert!(
        proposal
            .blind_spots
            .iter()
            .any(|spot| spot.contains("2 context item(s) removed")),
        "an oracle grading the reduced context cannot see what the reduction threw away"
    );
    assert!(proposal.review("k.okafor").is_ok());
}
