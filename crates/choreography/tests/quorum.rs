//! Blueprint 23.19: five agents sharing one evidence source are one vote, not five.

use bioprism_choreography::{
    CorrelationReason, EvidenceBasis, Finding, Juror, Jury, Question, QuestionKind, QuorumOutcome,
    QuorumRule, Role, Vote,
};

fn independent(id: &str, role: &str, vote: Vote, source: &str) -> Juror {
    Juror::new(id, role, vote).declaring(EvidenceBasis::from_sources([source]))
}

fn question() -> Question {
    Question::new("q-1", QuestionKind::Underdetermined).satisfying("tests.pass")
}

#[test]
fn five_jurors_sharing_one_evidence_source_are_one_vote() {
    let jury = Jury::new(
        (0..5)
            .map(|index| {
                Juror::new(format!("reviewer-{index}"), "reviewer", Vote::Approve)
                    .declaring(EvidenceBasis::from_sources(["report-A"]))
            })
            .collect(),
    );

    let outcome = jury.decide(&QuorumRule::two_of_three_with_safety_veto(), &question());

    let QuorumOutcome::NotReached {
        raw_approvals,
        effective_approvals,
        independence,
        ..
    } = &outcome
    else {
        panic!("expected the count to fall short, got {outcome:?}");
    };
    assert_eq!(*raw_approvals, 5);
    assert_eq!(*effective_approvals, 1);
    assert_eq!(independence.independent_voices(), 1);
    assert!(!outcome.is_quorum());
}

#[test]
fn a_quorum_over_unverified_independence_is_not_a_quorum() {
    let jury = Jury::new(vec![
        independent("a", "reviewer", Vote::Approve, "report-A"),
        independent("b", "reviewer", Vote::Approve, "report-B"),
        Juror::new("c", "reviewer", Vote::Approve),
    ]);

    let outcome = jury.decide(&QuorumRule::two_of_three_with_safety_veto(), &question());

    let QuorumOutcome::IndependenceUnverified { jurors, .. } = &outcome else {
        panic!("expected the tally to refuse, got {outcome:?}");
    };
    assert_eq!(jurors, &vec!["c".to_string()]);
    assert!(!outcome.is_quorum());
    assert_eq!(
        outcome.effective_approvals(),
        None,
        "no count exists, because no clustering was possible"
    );
}

#[test]
fn independent_jurors_reach_quorum_and_the_outcome_carries_the_clustering() {
    let jury = Jury::new(vec![
        independent("a", "reviewer", Vote::Approve, "report-A"),
        independent("b", "reviewer", Vote::Approve, "report-B"),
        independent("c", "reviewer", Vote::Reject, "report-C"),
    ]);

    let outcome = jury.decide(&QuorumRule::two_of_three_with_safety_veto(), &question());

    let QuorumOutcome::Reached {
        effective_approvals,
        required,
        independence,
        dissent,
        ..
    } = &outcome
    else {
        panic!("expected a quorum, got {outcome:?}");
    };
    assert_eq!(*effective_approvals, 2);
    assert_eq!(*required, 2);
    assert_eq!(independence.independent_voices(), 3);
    assert!(independence.correlations.is_empty());
    assert_eq!(dissent.len(), 1, "the rejecting reviewer survives the tally");
    assert_eq!(dissent[0].juror, "c");
    assert!(outcome.is_quorum());
}

#[test]
fn a_shared_model_family_collapses_two_jurors_into_one_voice() {
    let jury = Jury::new(vec![
        Juror::new("a", "reviewer", Vote::Approve)
            .declaring(EvidenceBasis::from_sources(["report-A"]).on_model("family-1")),
        Juror::new("b", "reviewer", Vote::Approve)
            .declaring(EvidenceBasis::from_sources(["report-B"]).on_model("family-1")),
        independent("c", "reviewer", Vote::Reject, "report-C"),
    ]);

    let outcome = jury.decide(&QuorumRule::two_of_three_with_safety_veto(), &question());
    assert!(!outcome.is_quorum());

    let independence = outcome.independence().expect("clustering ran");
    assert_eq!(independence.independent_voices(), 2);
    assert!(matches!(
        independence.correlations[0].reason,
        CorrelationReason::SameModelFamily { .. }
    ));
}

#[test]
fn an_identical_context_capsule_collapses_two_jurors_into_one_voice() {
    let jury = Jury::new(vec![
        Juror::new("a", "reviewer", Vote::Approve)
            .declaring(EvidenceBasis::from_sources(["report-A"]).with_context("capsule-9")),
        Juror::new("b", "reviewer", Vote::Approve)
            .declaring(EvidenceBasis::from_sources(["report-B"]).with_context("capsule-9")),
    ]);

    let independence = jury.cluster();
    assert_eq!(independence.independent_voices(), 1);
    assert!(matches!(
        independence.correlations[0].reason,
        CorrelationReason::IdenticalContext { .. }
    ));
}

#[test]
fn correlation_is_transitive_so_a_chain_of_overlaps_is_one_voice() {
    let jury = Jury::new(vec![
        Juror::new("a", "reviewer", Vote::Approve)
            .declaring(EvidenceBasis::from_sources(["report-A"])),
        Juror::new("b", "reviewer", Vote::Approve)
            .declaring(EvidenceBasis::from_sources(["report-A", "report-B"])),
        Juror::new("c", "reviewer", Vote::Approve)
            .declaring(EvidenceBasis::from_sources(["report-B"])),
    ]);

    assert_eq!(jury.cluster().independent_voices(), 1);
}

#[test]
fn shared_tools_alone_do_not_collapse_jurors() {
    let mut left = EvidenceBasis::from_sources(["report-A"]);
    left.tools.insert("pytest".into());
    let mut right = EvidenceBasis::from_sources(["report-B"]);
    right.tools.insert("pytest".into());

    let jury = Jury::new(vec![
        Juror::new("a", "reviewer", Vote::Approve).declaring(left),
        Juror::new("b", "reviewer", Vote::Approve).declaring(right),
    ]);

    assert_eq!(
        jury.cluster().independent_voices(),
        2,
        "two reviewers using the same test runner are not thereby one reviewer"
    );
}

#[test]
fn a_cluster_whose_members_disagree_counts_for_neither_side() {
    let jury = Jury::new(vec![
        Juror::new("a", "reviewer", Vote::Approve)
            .declaring(EvidenceBasis::from_sources(["report-A"])),
        Juror::new("b", "reviewer", Vote::Reject)
            .declaring(EvidenceBasis::from_sources(["report-A"])),
        independent("c", "reviewer", Vote::Approve, "report-C"),
    ]);

    let independence = jury.cluster();
    assert_eq!(independence.independent_voices(), 2);
    let split = independence
        .clusters
        .iter()
        .find(|cluster| cluster.jurors.len() == 2)
        .expect("the correlated pair");
    assert_eq!(split.effective_vote, None);
}

#[test]
fn a_veto_is_not_outvoted() {
    let jury = Jury::new(vec![
        independent("a", "reviewer", Vote::Approve, "report-A"),
        independent("b", "reviewer", Vote::Approve, "report-B"),
        independent("c", "reviewer", Vote::Approve, "report-C"),
        independent("s", "safety", Vote::Veto, "report-D"),
    ]);

    let outcome = jury.decide(&QuorumRule::two_of_three_with_safety_veto(), &question());
    let QuorumOutcome::Vetoed { by, role, .. } = &outcome else {
        panic!("expected a veto, got {outcome:?}");
    };
    assert_eq!(by, "s");
    assert_eq!(role, &Role::new("safety"));
    assert!(!outcome.is_quorum());
}

#[test]
fn a_veto_from_a_role_without_veto_power_is_only_a_vote() {
    let jury = Jury::new(vec![
        independent("a", "reviewer", Vote::Approve, "report-A"),
        independent("b", "reviewer", Vote::Approve, "report-B"),
        independent("c", "reviewer", Vote::Veto, "report-C"),
    ]);

    let outcome = jury.decide(&QuorumRule::two_of_three_with_safety_veto(), &question());
    assert!(outcome.is_quorum());
}

#[test]
fn a_failing_prerequisite_stops_the_vote_before_anything_is_counted() {
    let jury = Jury::new(vec![
        independent("a", "reviewer", Vote::Approve, "report-A"),
        independent("b", "reviewer", Vote::Approve, "report-B"),
    ]);

    let outcome = jury.decide(
        &QuorumRule::two_of_three_with_safety_veto(),
        &Question::new("q-2", QuestionKind::Underdetermined),
    );
    let QuorumOutcome::PrerequisiteUnmet { missing, .. } = &outcome else {
        panic!("expected the prerequisite gate, got {outcome:?}");
    };
    assert_eq!(missing, &vec!["tests.pass".to_string()]);
}

#[test]
fn a_rule_may_not_decide_a_kind_of_question_it_does_not_claim() {
    let jury = Jury::new(vec![
        independent("a", "reviewer", Vote::Approve, "report-A"),
        independent("b", "reviewer", Vote::Approve, "report-B"),
    ]);

    let outcome = jury.decide(
        &QuorumRule::two_of_three_with_safety_veto(),
        &Question::new("q-3", QuestionKind::Factual).satisfying("tests.pass"),
    );
    let QuorumOutcome::NotDecidableByVote { kind, .. } = &outcome else {
        panic!("a factual question is not settled by preference, got {outcome:?}");
    };
    assert_eq!(*kind, QuestionKind::Factual);
}

#[test]
fn a_majority_does_not_override_a_deterministic_failure() {
    let jury = Jury::new(vec![
        independent("a", "reviewer", Vote::Approve, "report-A"),
        independent("b", "reviewer", Vote::Approve, "report-B"),
        independent("c", "reviewer", Vote::Approve, "report-C"),
    ]);

    let outcome = jury.decide(
        &QuorumRule::new("unanimous-ish", 2).requiring("tests.pass"),
        &question().with_finding(Finding {
            id: "ci-4711".into(),
            summary: "the integration suite fails on the merge commit".into(),
            supports: Vote::Reject,
        }),
    );

    let QuorumOutcome::OverriddenByDeterministicEvidence {
        finding,
        effective_approvals,
        ..
    } = &outcome
    else {
        panic!("expected the finding to dominate, got {outcome:?}");
    };
    assert_eq!(finding, "ci-4711");
    assert_eq!(*effective_approvals, 3);
    assert!(!outcome.is_quorum());
}

#[test]
fn a_deterministic_finding_that_agrees_with_the_majority_leaves_the_quorum_standing() {
    let jury = Jury::new(vec![
        independent("a", "reviewer", Vote::Approve, "report-A"),
        independent("b", "reviewer", Vote::Approve, "report-B"),
    ]);

    let outcome = jury.decide(
        &QuorumRule::new("two-independent", 2).requiring("tests.pass"),
        &question().with_finding(Finding {
            id: "ci-4712".into(),
            summary: "the integration suite passes".into(),
            supports: Vote::Approve,
        }),
    );
    assert!(outcome.is_quorum());
}

#[test]
fn abstentions_do_not_silence_the_cluster_they_share_a_basis_with() {
    let jury = Jury::new(vec![
        Juror::new("a", "reviewer", Vote::Abstain)
            .declaring(EvidenceBasis::from_sources(["report-A"])),
        Juror::new("b", "reviewer", Vote::Approve)
            .declaring(EvidenceBasis::from_sources(["report-A"])),
    ]);

    let independence = jury.cluster();
    assert_eq!(independence.independent_voices(), 1);
    assert_eq!(independence.clusters[0].effective_vote, Some(Vote::Approve));
}

#[test]
fn the_independence_analysis_survives_serialisation() {
    let jury = Jury::new(vec![
        independent("a", "reviewer", Vote::Approve, "report-A"),
        independent("b", "reviewer", Vote::Approve, "report-B"),
    ]);
    let outcome = jury.decide(&QuorumRule::new("two-independent", 2), &question());

    let encoded = serde_json::to_string(&outcome).expect("serialises");
    let decoded: QuorumOutcome = serde_json::from_str(&encoded).expect("round trips");
    assert!(decoded.is_quorum());
    assert_eq!(
        decoded.independence().map(|i| i.independent_voices()),
        Some(2),
        "a serialised quorum still carries the clustering that justified it"
    );
}
