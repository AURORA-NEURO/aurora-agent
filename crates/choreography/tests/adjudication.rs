//! Blueprint 23.18: the procedure is the object, and an unsettleable dispute says so.

use bioprism_choreography::{
    open_challenges, AdjudicationRecord, Assurance, Dispute, DisputeKind, Evidence, EvidenceKind,
    EvidenceRequirement, Position, Procedure, Role, Ruling, Side,
};
use bioprism_weave::{Act, ActKind, Ledger};
use serde_json::json;

fn requirement() -> EvidenceRequirement {
    EvidenceRequirement::new(
        "transaction-commit-order",
        "the event log showing the order in which the two writes committed",
        "inspect event-log",
    )
}

fn bare_dispute() -> Dispute {
    Dispute::new(
        "ch-18",
        DisputeKind::Factual,
        Position::new("analyst", "the writes committed in order"),
        Position::new("skeptic", "the writes could have interleaved"),
        Side::Claimant,
    )
    .requiring(requirement())
}

#[test]
fn a_ruling_names_the_rule_that_fired_and_why() {
    let dispute = Dispute::new(
        "ch-01",
        DisputeKind::Factual,
        Position::new("analyst", "the pipeline is deterministic").with_evidence(Evidence::new(
            "ev-1",
            EvidenceKind::TestResult,
            Assurance::DeterministicTest,
            Side::Claimant,
            "the replay test passes on every seed",
        )),
        Position::new("skeptic", "it looked flaky to me"),
        Side::Claimant,
    );

    let ruling = Procedure::evidence_first().adjudicate(&dispute);
    let Ruling::Resolved {
        rule,
        prevailing,
        holder,
        basis,
        ..
    } = &ruling
    else {
        panic!("expected a resolution, got {ruling:?}");
    };
    assert_eq!(rule, "deterministic-evidence-for-claimant");
    assert_eq!(*prevailing, Side::Claimant);
    assert_eq!(holder, &Role::new("analyst"));
    assert!(basis.contains("reproducible"));
}

#[test]
fn a_dispute_the_evidence_cannot_settle_is_unresolved_with_the_missing_evidence_named() {
    let ruling = Procedure::evidence_first().adjudicate(&bare_dispute());

    let Ruling::Unresolved {
        missing_evidence,
        positions,
        ..
    } = &ruling
    else {
        panic!("expected unresolved, got {ruling:?}");
    };
    assert_eq!(missing_evidence.len(), 1);
    assert_eq!(missing_evidence[0].id, "transaction-commit-order");
    assert_eq!(missing_evidence[0].acquisition, "inspect event-log");
    assert_eq!(
        positions.len(),
        2,
        "both positions survive an unresolved dispute"
    );
    assert!(!ruling.is_settled());
    assert_eq!(ruling.prevailing(), None);
}

#[test]
fn an_unsettleable_dispute_does_not_default_to_the_higher_authority_party() {
    let mut dispute = bare_dispute();
    dispute.claimant = Position::new("principal-investigator", "the writes committed in order");
    dispute.challenger = Position::new("junior-analyst", "the writes could have interleaved");

    let ruling = Procedure::evidence_first().adjudicate(&dispute);
    assert!(
        matches!(ruling, Ruling::Unresolved { .. }),
        "seniority is not evidence; got {ruling:?}"
    );
}

#[test]
fn an_unsettleable_dispute_gives_the_same_answer_whichever_order_the_parties_are_in() {
    let forward = Procedure::evidence_first().adjudicate(&bare_dispute());

    let mut swapped = bare_dispute();
    std::mem::swap(&mut swapped.claimant, &mut swapped.challenger);
    swapped.burden = Side::Challenger;
    let reversed = Procedure::evidence_first().adjudicate(&swapped);

    assert!(matches!(forward, Ruling::Unresolved { .. }));
    assert!(
        matches!(reversed, Ruling::Unresolved { .. }),
        "no tiebreak exists in either direction"
    );
}

#[test]
fn a_deterministic_test_beats_an_unreproduced_claim_whichever_side_holds_it() {
    let dispute = Dispute::new(
        "ch-02",
        DisputeKind::Factual,
        Position::new("analyst", "the join is lossless").with_evidence(Evidence::new(
            "ev-doc",
            EvidenceKind::Document,
            Assurance::SourceDocument,
            Side::Claimant,
            "the design note says so",
        )),
        Position::new("skeptic", "row counts differ").with_evidence(Evidence::new(
            "ev-test",
            EvidenceKind::TestResult,
            Assurance::DeterministicTest,
            Side::Challenger,
            "the row-count assertion fails",
        )),
        Side::Claimant,
    );

    let ruling = Procedure::evidence_first().adjudicate(&dispute);
    assert_eq!(ruling.prevailing(), Some(Side::Challenger));
    assert_eq!(ruling.rule(), Some("deterministic-evidence-for-challenger"));
}

#[test]
fn two_contradictory_deterministic_results_escalate_rather_than_resolve() {
    let dispute = Dispute::new(
        "ch-03",
        DisputeKind::Factual,
        Position::new("analyst", "the metric is 0.81").with_evidence(Evidence::new(
            "ev-a",
            EvidenceKind::TestResult,
            Assurance::DeterministicTest,
            Side::Claimant,
            "harness A reports 0.81",
        )),
        Position::new("skeptic", "the metric is 0.62").with_evidence(Evidence::new(
            "ev-b",
            EvidenceKind::TestResult,
            Assurance::DeterministicTest,
            Side::Challenger,
            "harness B reports 0.62",
        )),
        Side::Claimant,
    );

    let ruling = Procedure::evidence_first().adjudicate(&dispute);
    let Ruling::Escalated { to, reason, .. } = &ruling else {
        panic!("expected escalation, got {ruling:?}");
    };
    assert_eq!(to, "human-adjudicator");
    assert!(reason.contains("defect"));
    assert!(!ruling.is_settled(), "a referral is not a decision");
}

#[test]
fn a_concession_resolves_before_any_evidence_rule_is_consulted() {
    let dispute = Dispute::new(
        "ch-04",
        DisputeKind::Factual,
        Position::new("analyst", "the pipeline is deterministic")
            .with_evidence(Evidence::new(
                "ev-1",
                EvidenceKind::TestResult,
                Assurance::DeterministicTest,
                Side::Claimant,
                "the replay test passes",
            ))
            .conceding(),
        Position::new("skeptic", "it looked flaky"),
        Side::Claimant,
    );

    let ruling = Procedure::evidence_first().adjudicate(&dispute);
    assert_eq!(ruling.rule(), Some("concession-by-claimant"));
    assert_eq!(ruling.prevailing(), Some(Side::Challenger));
}

#[test]
fn an_unmet_burden_only_decides_when_the_other_side_produced_something() {
    let neither = Dispute::new(
        "ch-05",
        DisputeKind::Factual,
        Position::new("analyst", "the model generalises"),
        Position::new("skeptic", "it memorised the split"),
        Side::Claimant,
    );
    assert!(
        matches!(
            Procedure::evidence_first().adjudicate(&neither),
            Ruling::Unresolved { .. }
        ),
        "an empty record must not resolve against whoever happens to carry the burden"
    );

    let one_sided = Dispute::new(
        "ch-06",
        DisputeKind::Factual,
        Position::new("analyst", "the model generalises"),
        Position::new("skeptic", "it memorised the split").with_evidence(Evidence::new(
            "ev-doc",
            EvidenceKind::Document,
            Assurance::SourceDocument,
            Side::Challenger,
            "the split manifest shows overlap",
        )),
        Side::Claimant,
    );
    let ruling = Procedure::evidence_first().adjudicate(&one_sided);
    assert_eq!(ruling.rule(), Some("documentary-evidence-for-challenger"));
}

#[test]
fn the_burden_rule_fires_only_after_every_evidence_rule_has_failed_to_apply() {
    let dispute = Dispute::new(
        "ch-07",
        DisputeKind::Factual,
        Position::new("analyst", "the run is stable").with_evidence(Evidence::new(
            "ev-obs",
            EvidenceKind::Statement,
            Assurance::Opinion,
            Side::Claimant,
            "it looked fine to me",
        )),
        Position::new("skeptic", "the run drifts").with_evidence(Evidence::new(
            "ev-measure",
            EvidenceKind::Measurement,
            Assurance::Measurement,
            Side::Challenger,
            "the variance across seeds is 0.14",
        )),
        Side::Claimant,
    );

    let ruling = Procedure::evidence_first().adjudicate(&dispute);
    assert_eq!(
        ruling.rule(),
        Some("burden-unmet"),
        "no side is documented, so the rules that decide on present evidence do not apply and the \
         declared burden decides"
    );
    assert_eq!(ruling.prevailing(), Some(Side::Challenger));
}

#[test]
fn a_stronger_class_of_evidence_pre_empts_the_burden_rule() {
    let dispute = Dispute::new(
        "ch-07b",
        DisputeKind::Factual,
        Position::new("analyst", "the run is reproducible").with_evidence(Evidence::new(
            "ev-repro",
            EvidenceKind::Measurement,
            Assurance::Reproduction,
            Side::Claimant,
            "an independent rerun agreed",
        )),
        Position::new("skeptic", "the run drifts").with_evidence(Evidence::new(
            "ev-obs",
            EvidenceKind::Statement,
            Assurance::Opinion,
            Side::Challenger,
            "it felt different",
        )),
        Side::Challenger,
    );

    let ruling = Procedure::evidence_first().adjudicate(&dispute);
    assert_eq!(
        ruling.rule(),
        Some("documentary-evidence-for-claimant"),
        "a reproduction clears the documentary threshold, so evidence decides before the burden \
         rule is reached"
    );
    assert_eq!(ruling.prevailing(), Some(Side::Claimant));
}

#[test]
fn a_normative_question_is_referred_to_a_jury_only_after_the_evidence_rules_fail() {
    let procedure = Procedure::evidence_first().with_jury_referral("review-board");

    let normative = Dispute::new(
        "ch-08",
        DisputeKind::Policy,
        Position::new("lead", "publishing this is permitted"),
        Position::new("safety", "publishing this is not permitted"),
        Side::Claimant,
    );
    let ruling = procedure.adjudicate(&normative);
    assert_eq!(ruling.rule(), Some("refer-normative-question-to-jury"));

    let with_a_test = Dispute::new(
        "ch-09",
        DisputeKind::Policy,
        Position::new("lead", "publishing this is permitted"),
        Position::new("safety", "the redaction check fails").with_evidence(Evidence::new(
            "ev-redaction",
            EvidenceKind::TestResult,
            Assurance::DeterministicTest,
            Side::Challenger,
            "the redaction check fails on three fields",
        )),
        Side::Claimant,
    );
    assert_eq!(
        procedure.adjudicate(&with_a_test).rule(),
        Some("deterministic-evidence-for-challenger"),
        "a jury that can overrule a deterministic test is the failure mode, so the referral is last"
    );
}

#[test]
fn dissent_is_preserved_when_a_dispute_resolves() {
    let dispute = Dispute::new(
        "ch-10",
        DisputeKind::Methodological,
        Position::new("analyst", "the correction is appropriate").with_evidence(Evidence::new(
            "ev-1",
            EvidenceKind::TestResult,
            Assurance::DeterministicTest,
            Side::Claimant,
            "the simulation recovers the nominal rate",
        )),
        Position::new("skeptic", "the correction is anticonservative").blocking(),
        Side::Claimant,
    );

    let ruling = Procedure::evidence_first().adjudicate(&dispute);
    let Ruling::Resolved { dissent, .. } = &ruling else {
        panic!("expected a resolution");
    };
    assert_eq!(dissent.len(), 1);
    assert_eq!(dissent[0].side, Side::Challenger);
    assert_eq!(dissent[0].claim, "the correction is anticonservative");
    assert!(dissent[0].blocking);
}

#[test]
fn a_resolution_still_reports_the_evidence_it_never_needed() {
    let dispute = bare_dispute()
        .requiring(EvidenceRequirement::new(
            "concurrency-test",
            "a test that exercises the interleaving",
            "run concurrency-test",
        ));
    let mut settled = dispute;
    settled.claimant = settled.claimant.with_evidence(Evidence::new(
        "ev-log",
        EvidenceKind::EventLog,
        Assurance::DeterministicTest,
        Side::Claimant,
        "the event log shows a total order",
    ));

    let ruling = Procedure::evidence_first().adjudicate(&settled);
    assert!(ruling.is_settled());
    assert_eq!(
        ruling.missing_evidence().len(),
        2,
        "nothing was tagged as satisfying either requirement, and hiding that would overstate the result"
    );
}

#[test]
fn evidence_tagged_against_a_requirement_removes_it_from_the_missing_list() {
    let mut dispute = bare_dispute();
    dispute.challenger = dispute.challenger.with_evidence(
        Evidence::new(
            "ev-log",
            EvidenceKind::EventLog,
            Assurance::SourceDocument,
            Side::Challenger,
            "the event log is inconclusive about ordering",
        )
        .satisfying("transaction-commit-order"),
    );

    assert!(dispute.missing_evidence().is_empty());
    let ruling = Procedure::evidence_first().adjudicate(&dispute);
    assert_eq!(ruling.rule(), Some("documentary-evidence-for-challenger"));
}

#[test]
fn a_weave_ledger_challenge_becomes_a_dispute_the_kernel_refused_to_settle() {
    let mut ledger = Ledger::new();
    let claim = ledger.append(Act::new(
        ActKind::Claim,
        "analyst",
        "ledger",
        json!("MYC amplification drives the observed dependency"),
    ));
    ledger.append(Act::new(
        ActKind::Challenge,
        "skeptic",
        "ledger",
        json!("the dependency is explained by copy-number confounding"),
    ).replying_to(claim.id.clone()));

    let open = open_challenges(&ledger);
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].claimant, Role::new("analyst"));
    assert_eq!(open[0].challenger, Role::new("skeptic"));
    assert_eq!(open[0].claim_event, claim.id);

    let dispute = open[0].dispute("d-1", DisputeKind::Factual, Side::Claimant);
    assert_eq!(
        dispute.claimant.claim,
        "MYC amplification drives the observed dependency"
    );
    assert!(
        matches!(
            Procedure::evidence_first().adjudicate(&dispute),
            Ruling::Unresolved { .. }
        ),
        "a freshly opened challenge carries no evidence, so unresolved is the honest answer"
    );
}

#[test]
fn a_challenge_without_a_claim_antecedent_is_not_reported_as_a_dispute() {
    let mut ledger = Ledger::new();
    let proposal = ledger.append(Act::new(
        ActKind::Propose,
        "lead",
        "worker",
        json!("run the assay"),
    ));
    ledger.append(
        Act::new(ActKind::Challenge, "skeptic", "lead", json!("no"))
            .replying_to(proposal.id.clone()),
    );

    assert!(open_challenges(&ledger).is_empty());
}

#[test]
fn the_same_dispute_and_procedure_always_produce_the_same_record_digest() {
    let first = AdjudicationRecord::new(bare_dispute(), Procedure::evidence_first());
    let second = AdjudicationRecord::new(bare_dispute(), Procedure::evidence_first());

    assert_eq!(first.digest(), second.digest());
    assert!(first.digest().is_some());
    assert!(matches!(first.ruling, Ruling::Unresolved { .. }));
}

#[test]
fn a_different_procedure_over_the_same_dispute_produces_a_different_record() {
    let strict = AdjudicationRecord::new(bare_dispute(), Procedure::evidence_first());
    let referring = AdjudicationRecord::new(
        bare_dispute(),
        Procedure::evidence_first().with_jury_referral("review-board"),
    );

    assert_ne!(strict.digest(), referring.digest());
}
